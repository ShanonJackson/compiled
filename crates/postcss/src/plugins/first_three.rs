//! Combined first-three-plugins: discardDuplicates + discardEmptyRules + parentOrphanedPseudos
//!
//! Implements the first three PostCSS plugins from `transform.ts` as a single
//! Rust plugin with two phases:
//!   Phase 1: Root-level declaration deduplication (keeps last occurrence per prop)
//!   Phase 2: Single depth-first walk that removes empty-value declarations
//!            (and their now-empty parent rules) and fixes orphaned pseudo-selectors
//!
//! Order matters: dedup must run before empty-removal. See plan for analysis.

use std::collections::HashMap;

use crate::ast::{CssNode, NodeId, Stylesheet};
use crate::list;
use crate::plugin::Plugin;

/// Combined plugin that replaces discardDuplicates + discardEmptyRules + parentOrphanedPseudos.
pub struct FirstThreePlugins;

impl Plugin for FirstThreePlugins {
    fn name(&self) -> &str {
        "first-three-combined"
    }

    fn once(&mut self, root: NodeId, ss: &mut Stylesheet) {
        discard_duplicates(root, ss);
        discard_empty_and_fix_pseudos(root, ss);
    }
}

// ── Phase 1: discardDuplicates ──────────────────────────────────────────────

/// Port of `discard-duplicates.ts`.
///
/// Scans root's direct children. Groups declarations by prop name.
/// Removes all but the last occurrence of each prop.
fn discard_duplicates(root: NodeId, ss: &mut Stylesheet) {
    // Collect root-level declarations grouped by prop.
    let children: Vec<NodeId> = ss
        .children_of(root)
        .map(|c| c.to_vec())
        .unwrap_or_default();

    let mut decls_by_prop: HashMap<String, Vec<NodeId>> = HashMap::new();

    for &child_id in &children {
        if let CssNode::Declaration(d) = ss.node(child_id) {
            decls_by_prop
                .entry(d.prop.clone())
                .or_default()
                .push(child_id);
        }
    }

    // Remove all but the last for each prop group.
    for (_prop, ids) in &decls_by_prop {
        if ids.len() > 1 {
            for &id in &ids[..ids.len() - 1] {
                ss.remove_node(id);
            }
        }
    }
}

// ── Phase 2: discardEmptyRules + parentOrphanedPseudos (single walk) ────────

/// Combined walk: removes empty-value declarations and fixes orphaned pseudo selectors.
fn discard_empty_and_fix_pseudos(root: NodeId, ss: &mut Stylesheet) {
    // Collect all nodes in a single depth-first walk. We separate them into
    // two lists to process after the walk (avoids mutation during traversal).
    let mut empty_decls: Vec<NodeId> = Vec::new();
    let mut rules_to_fix: Vec<NodeId> = Vec::new();

    collect_nodes(ss, root, &mut empty_decls, &mut rules_to_fix);

    // Process empty declarations (discardEmptyRules).
    for decl_id in empty_decls {
        let parent_id = ss.node(decl_id).parent();
        ss.remove_node(decl_id);

        // If parent is a rule that is now empty, remove it too.
        if let Some(pid) = parent_id {
            if let CssNode::Rule(_) = ss.node(pid) {
                let child_count = ss
                    .children_of(pid)
                    .map(|c| c.len())
                    .unwrap_or(0);
                if child_count == 0 {
                    ss.remove_node(pid);
                }
            }
        }
    }

    // Process rules (parentOrphanedPseudos).
    for rule_id in rules_to_fix {
        // Check the rule still has a parent (might have been removed above).
        if ss.node(rule_id).parent().is_none() {
            continue;
        }
        fix_rule_selectors(rule_id, ss);
    }
}

/// Depth-first collection pass — avoids borrow issues by not mutating during traversal.
fn collect_nodes(
    ss: &Stylesheet,
    node_id: NodeId,
    empty_decls: &mut Vec<NodeId>,
    rules_to_fix: &mut Vec<NodeId>,
) {
    let children: Vec<NodeId> = ss
        .node(node_id)
        .children()
        .map(|c| c.to_vec())
        .unwrap_or_default();

    for child_id in children {
        match ss.node(child_id) {
            CssNode::Declaration(d) => {
                if is_value_empty(&d.value) {
                    empty_decls.push(child_id);
                }
            }
            CssNode::Rule(_) => {
                rules_to_fix.push(child_id);
                // Recurse into rule's children.
                collect_nodes(ss, child_id, empty_decls, rules_to_fix);
            }
            CssNode::AtRule(_) => {
                // Recurse into at-rule's children.
                collect_nodes(ss, child_id, empty_decls, rules_to_fix);
            }
            _ => {}
        }
    }
}

// ── discardEmptyRules helpers ───────────────────────────────────────────────

/// Port of `isValueEmpty` from `discard-empty-rules.ts`.
fn is_value_empty(value: &str) -> bool {
    value == "undefined" || value == "null" || value.trim().is_empty()
}

// ── parentOrphanedPseudos helpers ───────────────────────────────────────────

/// Fix selectors on a rule node: split by comma, transform orphaned pseudos,
/// rejoin with the original separator, and update the rule.
fn fix_rule_selectors(rule_id: NodeId, ss: &mut Stylesheet) {
    let (selector, has_colon_selector) = {
        if let CssNode::Rule(d) = ss.node(rule_id) {
            let sel = d.selector.clone();
            // Quick check: if no individual selector starts with ':', nothing to do.
            let selectors = list::comma(&sel);
            let needs_fix = selectors.iter().any(|s| s.starts_with(':'));
            (sel, needs_fix)
        } else {
            return;
        }
    };

    if !has_colon_selector {
        return;
    }

    let separator = extract_selector_separator(&selector);
    let selectors = list::comma(&selector);

    let transformed: Vec<String> = selectors
        .into_iter()
        .map(|s| transform_orphaned_pseudo(&s))
        .collect();

    let new_selector = transformed.join(&separator);

    if let CssNode::Rule(d) = ss.node_mut(rule_id) {
        d.selector = new_selector;
        // Clear raws.selector so the stringifier uses the new value
        // (stringifier checks raws.selector.value == d.selector).
        d.raws.selector = None;
    }
}

/// Extract the separator pattern from a selector string.
///
/// Matches JS PostCSS `rule.selectors` setter which uses `/,[\s]*/` to find
/// the original separator and reuses it when joining.
fn extract_selector_separator(selector: &str) -> String {
    let bytes = selector.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b',' {
            // Capture comma + trailing whitespace.
            let mut end = i + 1;
            while end < bytes.len() && matches!(bytes[end], b' ' | b'\n' | b'\t' | b'\r' | 0x0c) {
                end += 1;
            }
            return selector[i..end].to_string();
        }
    }
    // No comma found — single selector, separator doesn't matter.
    ",".to_string()
}

/// Transform an orphaned pseudo-selector by prepending `&` before each
/// top-level pseudo.
///
/// Replaces `postcss-selector-parser`'s `walkPseudos` + `insertBefore(nesting)`.
///
/// Examples:
///   `:hover`           → `&:hover`
///   `::before`         → `&::before`
///   `:hover:focus`     → `&:hover&:focus`
///   `:not(:hover)`     → `&:not(&:hover)`
///   `:first-child &`   → `&:first-child &`
///   `div > :hover`     → unchanged (doesn't start with `:`)
///   `&:hover`          → unchanged (doesn't start with `:`)
fn transform_orphaned_pseudo(selector: &str) -> String {
    if !selector.starts_with(':') {
        return selector.to_string();
    }

    let bytes = selector.as_bytes();
    let mut result = String::with_capacity(selector.len() + 8);
    let mut i = 0;

    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_brackets = false; // [...] attribute selector
    let mut escape = false;

    while i < bytes.len() {
        let b = bytes[i];

        if escape {
            result.push(b as char);
            escape = false;
            i += 1;
            continue;
        }

        if b == b'\\' {
            escape = true;
            result.push('\\');
            i += 1;
            continue;
        }

        if in_single_quote {
            if b == b'\'' {
                in_single_quote = false;
            }
            result.push(b as char);
            i += 1;
            continue;
        }

        if in_double_quote {
            if b == b'"' {
                in_double_quote = false;
            }
            result.push(b as char);
            i += 1;
            continue;
        }

        if in_brackets {
            if b == b']' {
                in_brackets = false;
            }
            result.push(b as char);
            i += 1;
            continue;
        }

        match b {
            b'\'' => {
                in_single_quote = true;
                result.push('\'');
                i += 1;
            }
            b'"' => {
                in_double_quote = true;
                result.push('"');
                i += 1;
            }
            b'[' => {
                in_brackets = true;
                result.push('[');
                i += 1;
            }
            b':' => {
                // Insert `&` before the pseudo.
                result.push('&');
                result.push(':');
                i += 1;
                // If next char is also `:` (pseudo-element like ::before),
                // consume it as part of the same pseudo.
                if i < bytes.len() && bytes[i] == b':' {
                    result.push(':');
                    i += 1;
                }
            }
            _ => {
                result.push(b as char);
                i += 1;
            }
        }
    }

    result
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::Processor;

    fn process(css: &str) -> String {
        let mut proc = Processor::new(vec![Box::new(FirstThreePlugins)]);
        proc.process(css)
    }

    // ── transform_orphaned_pseudo unit tests ────────────────────────────

    #[test]
    fn pseudo_noop_no_colon() {
        assert_eq!(transform_orphaned_pseudo("div"), "div");
        assert_eq!(transform_orphaned_pseudo("&:hover"), "&:hover");
        assert_eq!(transform_orphaned_pseudo(".class"), ".class");
        assert_eq!(transform_orphaned_pseudo("div > :hover"), "div > :hover");
    }

    #[test]
    fn pseudo_simple() {
        assert_eq!(transform_orphaned_pseudo(":hover"), "&:hover");
        assert_eq!(transform_orphaned_pseudo(":focus"), "&:focus");
        assert_eq!(transform_orphaned_pseudo(":active"), "&:active");
        assert_eq!(transform_orphaned_pseudo(":first-child"), "&:first-child");
    }

    #[test]
    fn pseudo_element() {
        assert_eq!(transform_orphaned_pseudo("::before"), "&::before");
        assert_eq!(transform_orphaned_pseudo("::after"), "&::after");
        assert_eq!(transform_orphaned_pseudo("::placeholder"), "&::placeholder");
    }

    #[test]
    fn pseudo_multiple() {
        assert_eq!(
            transform_orphaned_pseudo(":hover:focus"),
            "&:hover&:focus"
        );
    }

    #[test]
    fn pseudo_functional() {
        assert_eq!(transform_orphaned_pseudo(":not(.foo)"), "&:not(.foo)");
        assert_eq!(transform_orphaned_pseudo(":nth-child(2)"), "&:nth-child(2)");
    }

    #[test]
    fn pseudo_with_nesting_after() {
        assert_eq!(
            transform_orphaned_pseudo(":first-child &"),
            "&:first-child &"
        );
    }

    // ── discardDuplicates (ported from JS tests) ────────────────────────

    #[test]
    fn dedup_no_duplicates() {
        let css = "\n      display: block;\n      margin: 0 auto;\n    ";
        let result = process(css);
        assert_eq!(result, css);
    }

    #[test]
    fn dedup_removes_duplicates() {
        let result = process("\n      display: block;\n      display: flex;\n    ");
        assert_eq!(result, "\n      display: flex;\n    ");
    }

    // ── discardEmptyRules (ported from JS tests) ────────────────────────

    #[test]
    fn empty_omits_undefined_value() {
        let result = process("\n      display: undefined;\n      color: red;\n    ");
        assert_eq!(result, "\n      color: red;\n    ");
    }

    #[test]
    fn empty_omits_null_value() {
        let result = process("\n      display: null;\n      color: red;\n    ");
        assert_eq!(result, "\n      color: red;\n    ");
    }

    #[test]
    fn empty_omits_empty_value() {
        let result = process("\n      display: ;\n      color: red;\n    ");
        assert_eq!(result, "\n      color: red;\n    ");
    }

    #[test]
    fn empty_inside_selector_keeps_siblings() {
        let result = process(
            "\n      :hover {        \n        display: undefined;\n        color: red;\n      }\n    ",
        );
        // Combined plugin also transforms :hover → &:hover (orphaned pseudo).
        assert_eq!(
            result,
            "\n      &:hover {\n        color: red;\n      }\n    "
        );
    }

    #[test]
    fn empty_removes_empty_rule() {
        let result = process(
            "\n      :hover {        \n        display: undefined;\n      }\n    ",
        );
        assert_eq!(result, "\n    ");
    }

    // ── parentOrphanedPseudos (ported from JS tests) ────────────────────

    #[test]
    fn orphan_noop_with_existing_nesting() {
        let css =
            "\n      div {\n        &:hover {\n          display: block;\n        }\n      }\n    ";
        let result = process(css);
        assert_eq!(result, css);
    }

    #[test]
    fn orphan_parents_orphaned_pseudo() {
        let result = process(
            "\n      div {\n        :hover {\n          display: block;\n        }\n      }\n    ",
        );
        assert_eq!(
            result,
            "\n      div {\n        &:hover {\n          display: block;\n        }\n      }\n    "
        );
    }

    #[test]
    fn orphan_noop_combinator_before_pseudo() {
        let css = "\n      div {\n        div > :hover {\n          display: block;\n        }\n      }\n    ";
        let result = process(css);
        assert_eq!(result, css);
    }

    #[test]
    fn orphan_top_level_pseudo() {
        let result = process(
            "\n      :hover {\n        display: block;\n      }\n    ",
        );
        assert_eq!(
            result,
            "\n      &:hover {\n        display: block;\n      }\n    "
        );
    }

    #[test]
    fn orphan_pseudo_with_appended_nesting() {
        let result = process(
            "\n      :first-child & {\n        color: hotpink;\n      }\n    ",
        );
        assert_eq!(
            result,
            "\n      &:first-child & {\n        color: hotpink;\n      }\n    "
        );
    }

    #[test]
    fn orphan_noop_nesting_appended_to_attr() {
        let css = "\n      [data-look='h100']& {\n        display: block;\n      }\n    ";
        let result = process(css);
        assert_eq!(result, css);
    }

    #[test]
    fn orphan_multiple_selector_groups_both_pseudo() {
        let result = process(
            "\n      :hover, :active {\n        display: block;\n      }\n    ",
        );
        assert_eq!(
            result,
            "\n      &:hover, &:active {\n        display: block;\n      }\n    "
        );
    }

    #[test]
    fn orphan_multiple_selector_groups_mixed() {
        let result = process(
            "\n      div, :active {\n        display: block;\n      }\n    ",
        );
        assert_eq!(
            result,
            "\n      div, &:active {\n        display: block;\n      }\n    "
        );
    }

    // ── Combined interaction tests ──────────────────────────────────────

    #[test]
    fn combined_dedup_then_empty() {
        // dedup keeps last `color: ;`, then empty-rules removes it.
        let result = process("\n      color: red;\n      color: ;\n    ");
        assert_eq!(result, "\n    ");
    }

    #[test]
    fn combined_empty_inside_pseudo_rule() {
        let result = process(
            "\n      :hover {\n        display: undefined;\n      }\n    ",
        );
        assert_eq!(result, "\n    ");
    }

    #[test]
    fn combined_all_three() {
        let result = process(
            "\n      display: block;\n      display: flex;\n      :hover {\n        color: null;\n        background: red;\n      }\n    ",
        );
        assert_eq!(
            result,
            "\n      display: flex;\n      &:hover {\n        background: red;\n      }\n    "
        );
    }
}
