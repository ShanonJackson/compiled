use swc_core::common::DUMMY_SP;
use swc_core::css::ast::{
    CombinatorValue, ComplexSelector, ComplexSelectorChildren, ComponentValue, NestingSelector,
    QualifiedRule, QualifiedRulePrelude, Rule, SimpleBlock, Stylesheet, SubclassSelector,
};

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
    if let Some(compound) = first_compound_selector(complex) {
        if selector_starts_with_pseudo(compound) && compound.nesting_selector.is_none() {
            compound.nesting_selector = Some(NestingSelector { span: DUMMY_SP });
        }
    }
}

fn first_compound_selector(
    complex: &mut ComplexSelector,
) -> Option<&mut swc_core::css::ast::CompoundSelector> {
    let mut iter = complex.children.iter_mut();
    while let Some(child) = iter.next() {
        match child {
            ComplexSelectorChildren::CompoundSelector(compound) => return Some(compound),
            ComplexSelectorChildren::Combinator(combinator) => {
                if !matches!(combinator.value, CombinatorValue::Descendant) {
                    return None;
                }
            }
        }
    }
    None
}

fn selector_starts_with_pseudo(compound: &swc_core::css::ast::CompoundSelector) -> bool {
    if compound.nesting_selector.is_some() || compound.type_selector.is_some() {
        return false;
    }

    match compound.subclass_selectors.first() {
        Some(SubclassSelector::PseudoClass(_) | SubclassSelector::PseudoElement(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postcss::transform::{TransformContext, TransformCssOptions};
    use swc_core::common::{input::StringInput, FileName, SourceMap};
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
