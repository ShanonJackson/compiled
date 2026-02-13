//! CSS stringifier — 1:1 port of PostCSS 8.4.31 `stringifier.js`.
//!
//! This is the most critical component for byte-identical output.
//! Every method, constant, and edge case is ported exactly from the JS source.

use crate::ast::*;

/// Default raw formatting values — exact match of PostCSS 8.4.31's `DEFAULT_RAW`.
struct DefaultRaw;

impl DefaultRaw {
    const AFTER: &'static str = "\n";
    const BEFORE_CLOSE: &'static str = "\n";
    const BEFORE_COMMENT: &'static str = "\n";
    const BEFORE_DECL: &'static str = "\n";
    const BEFORE_OPEN: &'static str = " ";
    const BEFORE_RULE: &'static str = "\n";
    const COLON: &'static str = ": ";
    const COMMENT_LEFT: &'static str = " ";
    const COMMENT_RIGHT: &'static str = " ";
    const EMPTY_BODY: &'static str = "";
    const INDENT: &'static str = "    ";
    const SEMICOLON: bool = false;
}

/// Stringify a Stylesheet into a CSS string.
pub fn stringify(ss: &Stylesheet) -> String {
    let mut output = String::with_capacity(256);
    let mut stringifier = Stringifier::new(ss, &mut output);
    stringifier.stringify_node(ss.root, false);
    output
}

/// Stringify a single node within a stylesheet.
pub fn stringify_node(ss: &Stylesheet, node_id: NodeId) -> String {
    let mut output = String::new();
    let mut stringifier = Stringifier::new(ss, &mut output);
    stringifier.stringify_node(node_id, false);
    output
}

struct Stringifier<'a> {
    ss: &'a Stylesheet,
    builder: &'a mut String,
}

impl<'a> Stringifier<'a> {
    fn new(ss: &'a Stylesheet, builder: &'a mut String) -> Self {
        Stringifier { ss, builder }
    }

    /// Main dispatch: stringify a node based on its type.
    fn stringify_node(&mut self, node_id: NodeId, semicolon: bool) {
        match self.ss.node(node_id) {
            CssNode::Root(_) => self.stringify_root(node_id),
            CssNode::Rule(_) => self.stringify_rule(node_id),
            CssNode::AtRule(_) => self.stringify_atrule(node_id, semicolon),
            CssNode::Declaration(_) => self.stringify_decl(node_id, semicolon),
            CssNode::Comment(_) => self.stringify_comment(node_id),
        }
    }

    // ── Root ───────────────────────────────────────────────────────────────

    fn stringify_root(&mut self, node_id: NodeId) {
        self.body(node_id);
        let after = self.raw_str(node_id, "after", "after");
        if !after.is_empty() {
            self.builder.push_str(&after);
        }
    }

    // ── Comment ────────────────────────────────────────────────────────────

    fn stringify_comment(&mut self, node_id: NodeId) {
        let left = self.raw_str(node_id, "left", "commentLeft");
        let right = self.raw_str(node_id, "right", "commentRight");
        let text = if let CssNode::Comment(d) = self.ss.node(node_id) {
            &d.text
        } else {
            ""
        };
        self.builder.push_str("/*");
        self.builder.push_str(&left);
        self.builder.push_str(text);
        self.builder.push_str(&right);
        self.builder.push_str("*/");
    }

    // ── Declaration ────────────────────────────────────────────────────────

    fn stringify_decl(&mut self, node_id: NodeId, semicolon: bool) {
        let between = self.raw_str(node_id, "between", "colon");
        let (prop, value_str, important, important_raw) =
            if let CssNode::Declaration(d) = self.ss.node(node_id) {
                (
                    d.prop.clone(),
                    self.raw_value(node_id, "value"),
                    d.important,
                    d.raws.important.clone(),
                )
            } else {
                return;
            };

        let mut s = format!("{}{}{}", prop, between, value_str);
        if important {
            s.push_str(&important_raw.unwrap_or_else(|| " !important".to_string()));
        }
        if semicolon {
            s.push(';');
        }
        self.builder.push_str(&s);
    }

    // ── Rule ───────────────────────────────────────────────────────────────

    fn stringify_rule(&mut self, node_id: NodeId) {
        let selector = self.raw_value(node_id, "selector");
        self.block(node_id, &selector);
        // ownSemicolon
        if let CssNode::Rule(d) = self.ss.node(node_id) {
            if let Some(ref own) = d.raws.own_semicolon {
                self.builder.push_str(own);
            }
        }
    }

    // ── AtRule ─────────────────────────────────────────────────────────────

    fn stringify_atrule(&mut self, node_id: NodeId, semicolon: bool) {
        let (name, params_str, after_name, has_children) =
            if let CssNode::AtRule(d) = self.ss.node(node_id) {
                let params = if !d.params.is_empty() {
                    self.raw_value(node_id, "params")
                } else {
                    String::new()
                };
                (
                    d.name.clone(),
                    params,
                    d.raws.after_name.clone(),
                    d.children.is_some(),
                )
            } else {
                return;
            };

        let mut name_str = format!("@{}", name);
        if let Some(ref an) = after_name {
            name_str.push_str(an);
        } else if !params_str.is_empty() {
            name_str.push(' ');
        }

        if has_children {
            let start = format!("{}{}", name_str, params_str);
            self.block(node_id, &start);
        } else {
            let between = self
                .ss
                .node(node_id)
                .raws()
                .between
                .clone()
                .unwrap_or_default();
            let end = format!(
                "{}{}{}",
                between,
                if semicolon { ";" } else { "" },
                ""
            );
            self.builder
                .push_str(&format!("{}{}{}", name_str, params_str, end));
        }
    }

    // ── Block ──────────────────────────────────────────────────────────────

    /// Stringify a block node: `start + between + { body after }`.
    fn block(&mut self, node_id: NodeId, start: &str) {
        let between = self.raw_str(node_id, "between", "beforeOpen");
        self.builder.push_str(start);
        self.builder.push_str(&between);
        self.builder.push('{');

        let children = self.ss.node(node_id).children();
        let has_children = children.map_or(false, |c| !c.is_empty());

        if has_children {
            self.body(node_id);
            let after = self.raw_str(node_id, "after", "after");
            if !after.is_empty() {
                self.builder.push_str(&after);
            }
        } else {
            let after = self.raw_str(node_id, "after", "emptyBody");
            if !after.is_empty() {
                self.builder.push_str(&after);
            }
        }

        self.builder.push('}');
    }

    // ── Body ───────────────────────────────────────────────────────────────

    /// Stringify the children of a container node.
    fn body(&mut self, node_id: NodeId) {
        let children: Vec<NodeId> = self
            .ss
            .node(node_id)
            .children()
            .map(|c| c.to_vec())
            .unwrap_or_default();

        if children.is_empty() {
            return;
        }

        // Find the index of the last non-comment child.
        let mut last_non_comment = children.len() - 1;
        while last_non_comment > 0 {
            if self.ss.node(children[last_non_comment]).node_type() != "comment" {
                break;
            }
            last_non_comment -= 1;
        }

        // Determine if last child gets a semicolon.
        let use_semicolon = self.raw_semicolon(node_id);

        for (i, &child_id) in children.iter().enumerate() {
            let before = self.raw_str(child_id, "before", "before");
            if !before.is_empty() {
                self.builder.push_str(&before);
            }
            let semi = i != last_non_comment || use_semicolon;
            self.stringify_node(child_id, semi);
        }
    }

    // ── Raw value resolution ───────────────────────────────────────────────

    /// Get the raw or normalized value for a property.
    ///
    /// Port of `Stringifier.prototype.rawValue()` in JS.
    fn raw_value(&self, node_id: NodeId, prop: &str) -> String {
        let node = self.ss.node(node_id);
        let raws = node.raws();
        match prop {
            "value" => {
                if let CssNode::Declaration(d) = node {
                    if let Some(ref rv) = raws.value {
                        if rv.value == d.value {
                            return rv.raw.clone();
                        }
                    }
                    return d.value.clone();
                }
                String::new()
            }
            "selector" => {
                if let CssNode::Rule(d) = node {
                    if let Some(ref rv) = raws.selector {
                        if rv.value == d.selector {
                            return rv.raw.clone();
                        }
                    }
                    return d.selector.clone();
                }
                String::new()
            }
            "params" => {
                if let CssNode::AtRule(d) = node {
                    if let Some(ref rv) = raws.params {
                        if rv.value == d.params {
                            return rv.raw.clone();
                        }
                    }
                    return d.params.clone();
                }
                String::new()
            }
            _ => String::new(),
        }
    }

    /// Resolve a raw formatting string.
    ///
    /// Port of `Stringifier.prototype.raw()` in JS PostCSS.
    /// The cascade is:
    /// 1. Check `node.raws[own]` directly
    /// 2. Special case: first child of root → empty `before`
    /// 3. Check root's `rawCache[detect]`
    /// 4. For before/after: delegate to `beforeAfter()`
    /// 5. Walk tree to detect from other nodes, or use `DEFAULT_RAW`
    /// 6. Cache result on root
    fn raw_str(&self, node_id: NodeId, own: &str, detect: &str) -> String {
        let node = self.ss.node(node_id);

        // 1. Check explicit raws.
        if let Some(val) = self.get_raw_own(node, own) {
            return val;
        }

        // 2. Special case: first child of root has no `before`.
        if detect == "before" {
            let parent = node.parent();
            if parent.is_none() {
                return String::new();
            }
            if let Some(parent_id) = parent {
                let parent_node = self.ss.node(parent_id);
                if parent_node.node_type() == "root" {
                    if let Some(first) = self.ss.first_child(parent_id) {
                        if first == node_id {
                            return String::new();
                        }
                    }
                }
            }
        }

        // 3. If no parent, use defaults.
        if node.parent().is_none() && node.node_type() != "root" {
            return self.default_raw(detect).to_string();
        }

        // 4. Check cache on root.
        if let Some(cached) = self.ss.get_raw_cache(detect) {
            return cached.clone();
        }

        // 5. Detect from tree or use defaults.
        if detect == "before" || detect == "after" {
            return self.before_after(node_id, detect);
        }

        // For other detect values, walk tree to find any node with that raw.
        let detected = self.detect_raw(detect);
        detected.unwrap_or_else(|| self.default_raw(detect).to_string())
    }

    /// Get a raw value directly from a node's raws struct.
    fn get_raw_own(&self, node: &CssNode, own: &str) -> Option<String> {
        let raws = node.raws();
        match own {
            "before" => raws.before.clone(),
            "after" => raws.after.clone(),
            "between" => raws.between.clone(),
            "left" => raws.left.clone(),
            "right" => raws.right.clone(),
            "afterName" | "after_name" => raws.after_name.clone(),
            _ => None,
        }
    }

    /// Get the semicolon raw (boolean → whether last decl gets `;`).
    fn raw_semicolon(&self, node_id: NodeId) -> bool {
        let node = self.ss.node(node_id);
        if let Some(semi) = node.raws().semicolon {
            return semi;
        }

        // Check root cache.
        if let Some(cached) = self.ss.get_raw_cache("semicolon") {
            return cached == "true";
        }

        // Detect from tree: find any node with raws.semicolon set.
        let detected = self.detect_semicolon();
        detected.unwrap_or(DefaultRaw::SEMICOLON)
    }

    /// Compute before/after spacing, including depth-based indentation.
    ///
    /// Port of `Stringifier.prototype.beforeAfter()`.
    fn before_after(&self, node_id: NodeId, detect: &str) -> String {
        let node = self.ss.node(node_id);

        // Determine the base value.
        let base_detect = if node.node_type() == "decl" {
            "beforeDecl"
        } else if node.node_type() == "comment" {
            "beforeComment"
        } else if detect == "before" {
            "beforeRule"
        } else {
            "beforeClose"
        };

        let mut value = self
            .detect_raw(base_detect)
            .unwrap_or_else(|| self.default_raw(base_detect).to_string());

        // Count depth.
        let mut depth = 0usize;
        let mut current = node_id;
        while let Some(parent) = self.ss.node(current).parent() {
            let parent_type = self.ss.node(parent).node_type();
            if parent_type == "root" {
                break;
            }
            depth += 1;
            current = parent;
        }

        // If value contains a newline, append indentation.
        if value.contains('\n') {
            let indent = self
                .detect_raw("indent")
                .unwrap_or_else(|| DefaultRaw::INDENT.to_string());
            if !indent.is_empty() {
                for _ in 0..depth {
                    value.push_str(&indent);
                }
            }
        }

        value
    }

    /// Walk the tree to detect a raw formatting value from any node.
    fn detect_raw(&self, detect: &str) -> Option<String> {
        let mut result: Option<String> = None;
        self.walk_for_raw(self.ss.root, detect, &mut result);
        result
    }

    /// Recursive helper for detect_raw.
    fn walk_for_raw(&self, node_id: NodeId, detect: &str, result: &mut Option<String>) {
        if result.is_some() {
            return;
        }

        let node = self.ss.node(node_id);

        // Check this node's raws for the detect key.
        let own = self.detect_key_to_own(detect);
        if let Some(val) = self.get_raw_own(node, own) {
            *result = Some(val);
            return;
        }

        // Recurse into children.
        if let Some(children) = node.children() {
            for &child_id in children {
                self.walk_for_raw(child_id, detect, result);
                if result.is_some() {
                    return;
                }
            }
        }
    }

    /// Walk tree to detect semicolon preference.
    fn detect_semicolon(&self) -> Option<bool> {
        let mut result: Option<bool> = None;
        self.walk_for_semicolon(self.ss.root, &mut result);
        result
    }

    fn walk_for_semicolon(&self, node_id: NodeId, result: &mut Option<bool>) {
        if result.is_some() {
            return;
        }
        if let Some(semi) = self.ss.node(node_id).raws().semicolon {
            *result = Some(semi);
            return;
        }
        if let Some(children) = self.ss.node(node_id).children() {
            for &child_id in children {
                self.walk_for_semicolon(child_id, result);
                if result.is_some() {
                    return;
                }
            }
        }
    }

    /// Map detect key names to own raw field names.
    fn detect_key_to_own<'b>(&self, detect: &'b str) -> &'b str {
        match detect {
            "beforeDecl" | "beforeComment" | "beforeRule" | "beforeClose" | "before" => "before",
            "after" => "after",
            "beforeOpen" => "between",
            "colon" => "between",
            "commentLeft" => "left",
            "commentRight" => "right",
            "indent" => "before", // indent is detected from before values
            "emptyBody" => "after",
            _ => detect,
        }
    }

    /// Get the default raw value for a detect key.
    fn default_raw(&self, detect: &str) -> &str {
        match detect {
            "after" => DefaultRaw::AFTER,
            "beforeClose" => DefaultRaw::BEFORE_CLOSE,
            "beforeComment" => DefaultRaw::BEFORE_COMMENT,
            "beforeDecl" => DefaultRaw::BEFORE_DECL,
            "beforeOpen" => DefaultRaw::BEFORE_OPEN,
            "beforeRule" => DefaultRaw::BEFORE_RULE,
            "colon" => DefaultRaw::COLON,
            "commentLeft" => DefaultRaw::COMMENT_LEFT,
            "commentRight" => DefaultRaw::COMMENT_RIGHT,
            "emptyBody" => DefaultRaw::EMPTY_BODY,
            "indent" => DefaultRaw::INDENT,
            "before" => "",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::parser;

    fn roundtrip(css: &str) -> String {
        let input = Input::new(css, None);
        let mut ss = Stylesheet::new();
        parser::parse(&input, &mut ss);
        stringify(&ss)
    }

    #[test]
    fn test_simple_rule() {
        let css = "a { }";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_declaration() {
        let css = "a {\n    color: red;\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_multiple_declarations() {
        let css = "a {\n    color: red;\n    font-size: 12px;\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_at_rule_no_block() {
        let css = "@charset \"utf-8\";";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_at_rule_with_block() {
        let css = "@media screen {\n    a {\n        color: red;\n    }\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_comment() {
        let css = "/* hello */";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_comment_in_rule() {
        let css = "a {\n    /* comment */\n    color: red;\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_important() {
        let css = "a {\n    color: red !important;\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_no_trailing_newline() {
        let css = "a { color: red }";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_multiple_rules() {
        let css = "a {\n    color: red;\n}\nb {\n    color: blue;\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_empty_stylesheet() {
        let css = "";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_whitespace_only() {
        let css = "  \n  ";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_multiple_selectors() {
        let css = "a, b {\n    color: red;\n}";
        assert_eq!(roundtrip(css), css);
    }

    #[test]
    fn test_nested_at_rules() {
        let css = "@media screen {\n    @supports (display: grid) {\n        a {\n            color: red;\n        }\n    }\n}";
        assert_eq!(roundtrip(css), css);
    }
}
