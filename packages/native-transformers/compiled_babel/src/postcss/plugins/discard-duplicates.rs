use swc_core::css::ast::{
    AtRule, AtRuleName, AtRulePrelude, ComponentValue, Declaration, DeclarationName, QualifiedRule,
    QualifiedRulePrelude, Rule, Stylesheet,
};
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardDuplicates;

impl Plugin for DiscardDuplicates {
    fn name(&self) -> &'static str {
        "discard-duplicates"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        dedupe(&mut stylesheet.rules);
    }
}

pub fn discard_duplicates() -> DiscardDuplicates {
    DiscardDuplicates
}

fn dedupe(rules: &mut Vec<Rule>) {
    let mut index = rules.len();

    while index > 0 {
        index -= 1;

        let action = {
            match &mut rules[index] {
                Rule::QualifiedRule(rule) => {
                    dedupe_component_values(&mut rule.block.value);
                    Some(RuleAction::Qualified(QualifiedRuleSnapshot::from_rule(
                        rule,
                    )))
                }
                Rule::AtRule(rule) => {
                    if let Some(block) = &mut rule.block {
                        dedupe_component_values(&mut block.value);
                    }

                    Some(RuleAction::AtRule(AtRuleSnapshot::from_rule(rule)))
                }
                Rule::ListOfComponentValues(list) => {
                    dedupe_component_values(&mut list.children);
                    None
                }
            }
        };

        match action {
            Some(RuleAction::Qualified(snapshot)) => dedupe_rule(&snapshot, rules, index),
            Some(RuleAction::AtRule(snapshot)) => dedupe_at_rule(&snapshot, rules, index),
            None => {}
        }
    }
}

fn dedupe_component_values(values: &mut Vec<ComponentValue>) {
    let mut index = values.len();

    while index > 0 {
        index -= 1;

        let action = {
            match &mut values[index] {
                ComponentValue::QualifiedRule(rule) => {
                    dedupe_component_values(&mut rule.block.value);
                    Some(ValueAction::Qualified(QualifiedRuleSnapshot::from_rule(
                        rule,
                    )))
                }
                ComponentValue::AtRule(rule) => {
                    if let Some(block) = &mut rule.block {
                        dedupe_component_values(&mut block.value);
                    }

                    Some(ValueAction::AtRule(AtRuleSnapshot::from_rule(rule)))
                }
                ComponentValue::Declaration(decl) => Some(ValueAction::Declaration(
                    DeclarationSnapshot::from_decl(decl),
                )),
                ComponentValue::SimpleBlock(block) => {
                    dedupe_component_values(&mut block.value);
                    None
                }
                ComponentValue::Function(function) => {
                    dedupe_component_values(&mut function.value);
                    None
                }
                ComponentValue::ListOfComponentValues(list) => {
                    dedupe_component_values(&mut list.children);
                    None
                }
                ComponentValue::KeyframeBlock(block) => {
                    dedupe_component_values(&mut block.block.value);
                    None
                }
                _ => None,
            }
        };

        match action {
            Some(ValueAction::Qualified(snapshot)) => {
                dedupe_rule_in_values(&snapshot, values, index)
            }
            Some(ValueAction::AtRule(snapshot)) => {
                dedupe_at_rule_in_values(&snapshot, values, index)
            }
            Some(ValueAction::Declaration(snapshot)) => {
                dedupe_declaration_in_values(&snapshot, values, index)
            }
            None => {}
        }
    }
}

fn dedupe_rule(snapshot: &QualifiedRuleSnapshot, nodes: &mut Vec<Rule>, last_index: usize) {
    let mut index = last_index as isize - 1;

    while index >= 0 {
        let remove = match &mut nodes[index as usize] {
            Rule::QualifiedRule(other) => {
                if snapshot.selectors_match(other) {
                    dedupe_rule_children(&snapshot.declarations, &mut other.block.value);
                    is_rule_empty(other)
                } else {
                    false
                }
            }
            _ => false,
        };

        if remove {
            nodes.remove(index as usize);
        }

        index -= 1;
    }
}

fn dedupe_rule_in_values(
    snapshot: &QualifiedRuleSnapshot,
    nodes: &mut Vec<ComponentValue>,
    last_index: usize,
) {
    let mut index = last_index as isize - 1;

    while index >= 0 {
        let remove = match &mut nodes[index as usize] {
            ComponentValue::QualifiedRule(other) => {
                if snapshot.selectors_match(other) {
                    dedupe_rule_children(&snapshot.declarations, &mut other.block.value);
                    is_rule_empty(other)
                } else {
                    false
                }
            }
            _ => false,
        };

        if remove {
            nodes.remove(index as usize);
        }

        index -= 1;
    }
}

fn dedupe_rule_children(declarations: &[DeclarationSnapshot], targets: &mut Vec<ComponentValue>) {
    for decl in declarations {
        remove_matching_declarations(decl, targets);
    }
}

fn dedupe_at_rule(snapshot: &AtRuleSnapshot, nodes: &mut Vec<Rule>, last_index: usize) {
    let mut index = last_index as isize - 1;

    while index >= 0 {
        let remove = match &mut nodes[index as usize] {
            Rule::AtRule(other) => snapshot.matches(other),
            _ => false,
        };

        if remove {
            nodes.remove(index as usize);
        }

        index -= 1;
    }
}

fn dedupe_at_rule_in_values(
    snapshot: &AtRuleSnapshot,
    nodes: &mut Vec<ComponentValue>,
    last_index: usize,
) {
    let mut index = last_index as isize - 1;

    while index >= 0 {
        let remove = match &mut nodes[index as usize] {
            ComponentValue::AtRule(other) => snapshot.matches(other),
            _ => false,
        };

        if remove {
            nodes.remove(index as usize);
        }

        index -= 1;
    }
}

fn dedupe_declaration_in_values(
    snapshot: &DeclarationSnapshot,
    nodes: &mut Vec<ComponentValue>,
    last_index: usize,
) {
    let mut index = last_index as isize - 1;

    while index >= 0 {
        let remove = match nodes.get(index as usize) {
            Some(ComponentValue::Declaration(other)) => snapshot.matches(other),
            _ => false,
        };

        if remove {
            nodes.remove(index as usize);
        }

        index -= 1;
    }
}

fn remove_matching_declarations(snapshot: &DeclarationSnapshot, nodes: &mut Vec<ComponentValue>) {
    let mut index = nodes.len();

    while index > 0 {
        index -= 1;

        let remove = match nodes.get(index) {
            Some(ComponentValue::Declaration(other)) => snapshot.matches(other),
            _ => false,
        };

        if remove {
            nodes.remove(index);
        }
    }
}

fn is_rule_empty(rule: &QualifiedRule) -> bool {
    !rule.block.value.iter().any(|component| {
        matches!(
            component,
            ComponentValue::QualifiedRule(_)
                | ComponentValue::AtRule(_)
                | ComponentValue::Declaration(_)
                | ComponentValue::KeyframeBlock(_)
                | ComponentValue::ListOfComponentValues(_)
                | ComponentValue::Function(_)
                | ComponentValue::SimpleBlock(_)
        )
    })
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

fn serialize_at_rule_name(name: &AtRuleName) -> Option<String> {
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });

        if generator.emit(name).is_err() {
            return None;
        }
    }

    Some(output)
}

fn serialize_at_rule_prelude(prelude: &AtRulePrelude) -> Option<String> {
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

fn serialize_at_rule_prelude_option(prelude: Option<&AtRulePrelude>) -> Option<String> {
    prelude.and_then(serialize_at_rule_prelude)
}

fn serialize_declaration_name(name: &DeclarationName) -> Option<String> {
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });

        if generator.emit(name).is_err() {
            return None;
        }
    }

    Some(output)
}

fn serialize_component_value(value: &ComponentValue) -> Option<String> {
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });

        if generator.emit(value).is_err() {
            return None;
        }
    }

    Some(output)
}

fn serialize_component_values(values: &[ComponentValue]) -> Option<String> {
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });

        for component in values {
            if generator.emit(component).is_err() {
                return None;
            }
        }
    }

    Some(output)
}

#[derive(Debug, Clone)]
struct DeclarationSnapshot {
    name: Option<String>,
    important: bool,
    value: Option<String>,
}

impl DeclarationSnapshot {
    fn from_decl(decl: &Declaration) -> Self {
        DeclarationSnapshot {
            name: serialize_declaration_name(&decl.name),
            important: decl.important.is_some(),
            value: serialize_component_values(&decl.value),
        }
    }

    fn matches(&self, decl: &Declaration) -> bool {
        self.name == serialize_declaration_name(&decl.name)
            && self.important == decl.important.is_some()
            && self.value == serialize_component_values(&decl.value)
    }
}

#[derive(Debug, Clone)]
struct QualifiedRuleSnapshot {
    selector: Option<String>,
    declarations: Vec<DeclarationSnapshot>,
}

impl QualifiedRuleSnapshot {
    fn from_rule(rule: &QualifiedRule) -> Self {
        QualifiedRuleSnapshot {
            selector: serialize_qualified_rule_prelude(&rule.prelude),
            declarations: rule
                .block
                .value
                .iter()
                .filter_map(|component| match component {
                    ComponentValue::Declaration(decl) => Some(DeclarationSnapshot::from_decl(decl)),
                    _ => None,
                })
                .collect(),
        }
    }

    fn selectors_match(&self, rule: &QualifiedRule) -> bool {
        self.selector == serialize_qualified_rule_prelude(&rule.prelude)
    }
}

#[derive(Debug, Clone)]
struct AtRuleSnapshot {
    name: Option<String>,
    prelude: Option<String>,
    block: Option<String>,
}

impl AtRuleSnapshot {
    fn from_rule(rule: &AtRule) -> Self {
        AtRuleSnapshot {
            name: serialize_at_rule_name(&rule.name),
            prelude: serialize_at_rule_prelude_option(rule.prelude.as_deref()),
            block: rule
                .block
                .as_ref()
                .and_then(|block| serialize_component_values(&block.value)),
        }
    }

    fn matches(&self, rule: &AtRule) -> bool {
        self.name == serialize_at_rule_name(&rule.name)
            && self.prelude == serialize_at_rule_prelude_option(rule.prelude.as_deref())
            && self.block
                == rule
                    .block
                    .as_ref()
                    .and_then(|block| serialize_component_values(&block.value))
    }
}

#[derive(Debug)]
enum RuleAction {
    Qualified(QualifiedRuleSnapshot),
    AtRule(AtRuleSnapshot),
}

#[derive(Debug)]
enum ValueAction {
    Qualified(QualifiedRuleSnapshot),
    AtRule(AtRuleSnapshot),
    Declaration(DeclarationSnapshot),
}
