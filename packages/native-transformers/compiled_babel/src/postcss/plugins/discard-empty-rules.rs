use swc_core::css::ast::{
    AtRule, AtRulePrelude, ComponentValue, Declaration, DeclarationName, QualifiedRule,
    QualifiedRulePrelude, Rule, Stylesheet,
};
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardEmptyRules;

impl Plugin for DiscardEmptyRules {
    fn name(&self) -> &'static str {
        "discard-empty-rules"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        prune_stylesheet(stylesheet);
    }
}

pub fn discard_empty_rules() -> DiscardEmptyRules {
    DiscardEmptyRules
}

fn prune_stylesheet(stylesheet: &mut Stylesheet) {
    prune_rule_list(&mut stylesheet.rules);
}

fn prune_rule_list(rules: &mut Vec<Rule>) {
    let mut index = 0;

    while index < rules.len() {
        let remove = match &mut rules[index] {
            Rule::QualifiedRule(rule) => {
                prune_component_values(&mut rule.block.value);
                qualified_rule_is_empty(rule)
            }
            Rule::AtRule(rule) => {
                if let Some(block) = &mut rule.block {
                    prune_component_values(&mut block.value);
                }

                at_rule_is_empty(rule)
            }
            Rule::ListOfComponentValues(list) => {
                prune_component_values(&mut list.children);
                list.children.is_empty()
            }
        };

        if remove {
            rules.remove(index);
        } else {
            index += 1;
        }
    }
}

fn prune_component_values(values: &mut Vec<ComponentValue>) {
    let mut index = 0;

    while index < values.len() {
        let remove = match &mut values[index] {
            ComponentValue::Declaration(declaration) => declaration_is_empty(declaration),
            ComponentValue::QualifiedRule(rule) => {
                prune_component_values(&mut rule.block.value);
                qualified_rule_is_empty(rule)
            }
            ComponentValue::AtRule(rule) => {
                if let Some(block) = &mut rule.block {
                    prune_component_values(&mut block.value);
                }

                at_rule_is_empty(rule)
            }
            ComponentValue::SimpleBlock(block) => {
                prune_component_values(&mut block.value);
                block.value.is_empty()
            }
            ComponentValue::KeyframeBlock(block) => {
                prune_component_values(&mut block.block.value);
                block.block.value.is_empty()
            }
            ComponentValue::Function(function) => {
                prune_component_values(&mut function.value);
                function.value.is_empty()
            }
            ComponentValue::ListOfComponentValues(list) => {
                prune_component_values(&mut list.children);
                list.children.is_empty()
            }
            _ => false,
        };

        if remove {
            values.remove(index);
        } else {
            index += 1;
        }
    }
}

fn declaration_is_empty(declaration: &Declaration) -> bool {
    declaration.value.is_empty() && !matches!(declaration.name, DeclarationName::DashedIdent(_))
}

fn qualified_rule_is_empty(rule: &QualifiedRule) -> bool {
    if rule.block.value.is_empty() {
        return true;
    }

    serialize_qualified_rule_prelude(&rule.prelude).map_or(false, |selector| selector.is_empty())
}

fn at_rule_is_empty(rule: &AtRule) -> bool {
    let params = match serialize_at_rule_prelude(rule.prelude.as_deref()) {
        Some(serialized) => serialized,
        None => return false,
    };

    let has_block = rule.block.is_some();
    let block_is_empty = matches!(&rule.block, Some(block) if block.value.is_empty());
    let params_are_empty = params.is_empty();

    block_is_empty || (params_are_empty && !has_block)
}

fn serialize_qualified_rule_prelude(prelude: &QualifiedRulePrelude) -> Option<String> {
    let mut output = String::new();

    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });

        if generator.emit(prelude).is_err() {
            return None;
        }
    }

    Some(output)
}

fn serialize_at_rule_prelude(prelude: Option<&AtRulePrelude>) -> Option<String> {
    match prelude {
        Some(prelude) => {
            let mut output = String::new();

            {
                let writer = BasicCssWriter::new(&mut output, None, Default::default());
                let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });

                if generator.emit(prelude).is_err() {
                    return None;
                }
            }

            Some(output)
        }
        None => Some(String::new()),
    }
}
