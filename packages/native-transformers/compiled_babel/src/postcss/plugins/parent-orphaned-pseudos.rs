use swc_core::common::{input::StringInput, FileName, SourceMap};
use swc_core::css::ast::{
    ComplexSelector, ComponentValue, QualifiedRule, QualifiedRulePrelude, Rule, SimpleBlock,
    Stylesheet,
};
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};
use swc_core::css::parser::{parse_string_input, parser::ParserConfig};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct ParentOrphanedPseudos;

impl Plugin for ParentOrphanedPseudos {
    fn name(&self) -> &'static str {
        "parent-orphaned-pseudos"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        normalize_stylesheet(stylesheet);
    }
}

pub fn parent_orphaned_pseudos() -> ParentOrphanedPseudos {
    ParentOrphanedPseudos
}

fn normalize_stylesheet(stylesheet: &mut Stylesheet) {
    for rule in &mut stylesheet.rules {
        normalize_rule(rule);
    }
}

fn normalize_rule(rule: &mut Rule) {
    match rule {
        Rule::QualifiedRule(rule) => {
            normalize_qualified_rule(rule);
            normalize_simple_block(&mut rule.block);
        }
        Rule::AtRule(at_rule) => {
            if let Some(block) = &mut at_rule.block {
                normalize_simple_block(block);
            }
        }
        Rule::ListOfComponentValues(list) => normalize_component_values(&mut list.children),
    }
}

fn normalize_simple_block(block: &mut SimpleBlock) {
    normalize_component_values(&mut block.value);
}

fn normalize_component_values(values: &mut [ComponentValue]) {
    for value in values {
        match value {
            ComponentValue::QualifiedRule(rule) => {
                normalize_qualified_rule(rule);
                normalize_simple_block(&mut rule.block);
            }
            ComponentValue::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    normalize_simple_block(block);
                }
            }
            ComponentValue::SimpleBlock(block) => normalize_simple_block(block),
            ComponentValue::ListOfComponentValues(list) => {
                normalize_component_values(&mut list.children)
            }
            ComponentValue::Function(function) => normalize_component_values(&mut function.value),
            ComponentValue::KeyframeBlock(block) => normalize_simple_block(&mut block.block),
            _ => {}
        }
    }
}

fn normalize_qualified_rule(rule: &mut QualifiedRule) {
    match &mut rule.prelude {
        QualifiedRulePrelude::SelectorList(selector_list) => {
            for complex in &mut selector_list.children {
                normalize_complex_selector(complex);
            }
        }
        QualifiedRulePrelude::RelativeSelectorList(selector_list) => {
            for relative in &mut selector_list.children {
                normalize_complex_selector(&mut relative.selector);
            }
        }
        _ => {}
    }
}

fn normalize_complex_selector(complex: &mut ComplexSelector) {
    let serialized = serialize_selector(complex);
    if !serialized.starts_with(':') {
        return;
    }

    // Mirror JS plugin: insert a nesting selector before **every** pseudo in orphaned selectors.
    if let Some(parsed) = rewrite_orphaned_selector(&serialized) {
        *complex = parsed;
    }
}

/// Serialize a complex selector to a string.
fn serialize_selector(complex: &ComplexSelector) -> String {
    let mut output = String::new();
    {
        let wr = BasicCssWriter::new(&mut output, None, Default::default());
        let mut gen = CodeGenerator::new(wr, CodegenConfig { minify: true });
        gen.emit(complex).expect("write selector");
    }
    output
}

/// Parse a complex selector from a string.
fn parse_selector(selector: &str) -> Option<ComplexSelector> {
    let css = format!("{selector}{{}}");
    let cm: std::sync::Arc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom("selector.css".into()).into(), css.into());
    let mut errors = vec![];
    let stylesheet = parse_string_input::<Stylesheet>(
        StringInput::from(&*fm),
        None,
        ParserConfig::default(),
        &mut errors,
    )
    .ok()?;
    if !errors.is_empty() {
        return None;
    }
    for rule in stylesheet.rules {
        if let Rule::QualifiedRule(rule) = rule {
            if let QualifiedRulePrelude::SelectorList(list) = rule.prelude {
                if let Some(first) = list.children.into_iter().next() {
                    return Some(first);
                }
            }
        }
    }
    None
}

/// Insert a nesting selector before each pseudo in an orphaned selector string.
fn add_nesting_before_pseudos(selector: &str) -> String {
    let mut out = String::with_capacity(selector.len() + 4);
    let mut chars = selector.chars().peekable();
    let mut _saw_pseudo = false;
    while let Some(ch) = chars.next() {
        if ch == ':' {
            // Consume a run of ':' characters (handles ::before)
            let mut colons = String::from(":");
            while let Some(':') = chars.peek() {
                colons.push(':');
                chars.next();
            }
            if !selector.starts_with('&') {
                out.push('&');
            }
            out.push_str(&colons);
            _saw_pseudo = true;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Attempt to rewrite an orphaned selector (starts with pseudo, no nesting) to match
/// JS behaviour by inserting a nesting selector before each pseudo component.
fn rewrite_orphaned_selector(serialized: &str) -> Option<ComplexSelector> {
    // Insert nesting before each pseudo so `:focus::before` becomes `&:focus&::before`.
    let rewritten = add_nesting_before_pseudos(serialized);
    if std::env::var("COMPILED_CSS_TRACE").is_ok() {
        eprintln!(
            "[parent-orphaned] orphan selector serialized='{}' rewritten='{}'",
            serialized, rewritten
        );
    }
    parse_selector(&rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postcss::transform::{TransformContext, TransformCssOptions};
    use swc_core::common::{input::StringInput, FileName, SourceMap};
    use swc_core::css::ast::ComplexSelectorChildren;
    use swc_core::css::parser::{parse_string_input, parser::ParserConfig};

    fn parse_stylesheet(css: &str) -> Stylesheet {
        let cm: std::sync::Arc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom("test.css".into()).into(), css.into());
        let mut errors = vec![];
        parse_string_input::<Stylesheet>(
            StringInput::from(&*fm),
            None,
            ParserConfig::default(),
            &mut errors,
        )
        .expect("failed to parse test stylesheet")
    }

    fn first_nested_rule(stylesheet: &Stylesheet) -> Option<&QualifiedRule> {
        for rule in &stylesheet.rules {
            if let Rule::QualifiedRule(rule) = rule {
                for component in &rule.block.value {
                    if let ComponentValue::QualifiedRule(nested) = component {
                        return Some(nested);
                    }
                }
            }
        }

        None
    }

    fn first_root_rule(stylesheet: &Stylesheet) -> Option<&QualifiedRule> {
        for rule in &stylesheet.rules {
            if let Rule::QualifiedRule(rule) = rule {
                return Some(rule);
            }
        }
        None
    }

    #[test]
    fn adds_nesting_to_orphaned_pseudo_rules() {
        let mut stylesheet =
            parse_stylesheet(".a {\n  :hover { color: red; }\n  ::before { content: ''; }\n }");
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);

        ParentOrphanedPseudos.run(&mut stylesheet, &mut ctx);

        let nested_rule = first_nested_rule(&stylesheet).expect("nested rule");
        assert_nesting_present(nested_rule);
    }

    #[test]
    fn keeps_existing_nesting_selectors() {
        let mut stylesheet = parse_stylesheet(".a { &::before { color: blue; } }");
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);

        ParentOrphanedPseudos.run(&mut stylesheet, &mut ctx);

        let nested_rule = first_nested_rule(&stylesheet).expect("nested rule");
        assert_nesting_present(nested_rule);
    }

    #[test]
    fn adds_nesting_when_pseudo_precedes_nesting_selector() {
        let mut stylesheet = parse_stylesheet(".a { :focus & { color: red; } }");
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);

        ParentOrphanedPseudos.run(&mut stylesheet, &mut ctx);

        let nested_rule = first_nested_rule(&stylesheet).expect("nested rule");
        assert_nesting_present(nested_rule);
    }

    #[test]
    fn does_not_add_nesting_to_regular_pseudos() {
        let mut stylesheet = parse_stylesheet(".a:hover { color: red; }");
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);

        ParentOrphanedPseudos.run(&mut stylesheet, &mut ctx);

        let root_rule = first_root_rule(&stylesheet).expect("root rule");
        assert_nesting_absent(root_rule);
    }

    fn assert_nesting_present(rule: &QualifiedRule) {
        match &rule.prelude {
            QualifiedRulePrelude::SelectorList(list) => {
                let compound = match list.children.first().unwrap().children.first().unwrap() {
                    ComplexSelectorChildren::CompoundSelector(compound) => compound,
                    _ => panic!("expected compound selector"),
                };

                assert!(compound.nesting_selector.is_some());
            }
            QualifiedRulePrelude::RelativeSelectorList(list) => {
                let relative = list.children.first().expect("relative selector");
                let compound = match relative.selector.children.first().unwrap() {
                    ComplexSelectorChildren::CompoundSelector(compound) => compound,
                    _ => panic!("expected compound selector"),
                };

                assert!(compound.nesting_selector.is_some());
            }
            _ => panic!("unexpected prelude variant"),
        }
    }

    fn assert_nesting_absent(rule: &QualifiedRule) {
        match &rule.prelude {
            QualifiedRulePrelude::SelectorList(list) => {
                let compound = match list.children.first().unwrap().children.first().unwrap() {
                    ComplexSelectorChildren::CompoundSelector(compound) => compound,
                    _ => panic!("expected compound selector"),
                };
                assert!(compound.nesting_selector.is_none());
            }
            QualifiedRulePrelude::RelativeSelectorList(list) => {
                let relative = list.children.first().expect("relative selector");
                let compound = match relative.selector.children.first().unwrap() {
                    ComplexSelectorChildren::CompoundSelector(compound) => compound,
                    _ => panic!("expected compound selector"),
                };
                assert!(compound.nesting_selector.is_none());
            }
            _ => panic!("unexpected prelude variant"),
        }
    }
}
