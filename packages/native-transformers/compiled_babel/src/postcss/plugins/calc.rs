use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    parse_value_to_components, serialize_component_values,
};
use crate::postcss::plugins::normalize_css_engine::calc::reduce_calc_value;
use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

#[derive(Debug, Clone, Copy)]
pub struct CalcOptions {
    precision: usize,
    preserve: bool,
    warn_when_cannot_resolve: bool,
    media_queries: bool,
    selectors: bool,
}

impl Default for CalcOptions {
    fn default() -> Self {
        Self {
            precision: 5,
            preserve: false,
            warn_when_cannot_resolve: false,
            media_queries: false,
            selectors: false,
        }
    }
}

pub fn postcss_calc() -> CalcPlugin {
    CalcPlugin {
        options: CalcOptions::default(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CalcPlugin {
    options: CalcOptions,
}

impl Plugin for CalcPlugin {
    fn name(&self) -> &'static str {
        "postcss-calc"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        for rule in &mut stylesheet.rules {
            process_rule(rule, &self.options);
        }
    }
}

fn process_rule(rule: &mut Rule, opts: &CalcOptions) {
    match rule {
        Rule::QualifiedRule(rule) => process_component_list(&mut rule.block.value, opts),
        Rule::AtRule(at) => {
            if let Some(block) = &mut at.block {
                process_component_list(&mut block.value, opts);
            }
        }
        Rule::ListOfComponentValues(list) => process_component_list(&mut list.children, opts),
    }
}

fn process_component_list(list: &mut Vec<ComponentValue>, opts: &CalcOptions) {
    let mut idx = 0;
    while idx < list.len() {
        match &mut list[idx] {
            ComponentValue::Declaration(decl) => {
                if let Some(clone) = process_declaration(decl, opts) {
                    // Insert preserved clone directly before the transformed declaration.
                    list.insert(idx, ComponentValue::Declaration(Box::new(clone)));
                    idx += 1;
                }
            }
            ComponentValue::QualifiedRule(rule) => {
                process_component_list(&mut rule.block.value, opts)
            }
            ComponentValue::AtRule(at) => {
                if let Some(block) = &mut at.block {
                    process_component_list(&mut block.value, opts);
                }
            }
            ComponentValue::SimpleBlock(block) => process_component_list(&mut block.value, opts),
            ComponentValue::ListOfComponentValues(list) => {
                process_component_list(&mut list.children, opts)
            }
            ComponentValue::KeyframeBlock(block) => {
                process_component_list(&mut block.block.value, opts)
            }
            _ => {}
        }
        idx += 1;
    }
}

fn process_declaration(decl: &mut Declaration, opts: &CalcOptions) -> Option<Declaration> {
    let Some(original) = serialize_component_values(&decl.value) else {
        return None;
    };

    let Some((next, unresolved)) = reduce_calc_value(&original, opts.precision) else {
        return None;
    };

    if unresolved && opts.warn_when_cannot_resolve {
        // TODO: surface warnings once TransformContext exposes a warning channel.
    }

    if next == original {
        return None;
    }

    if opts.preserve {
        let mut clone = decl.clone();
        clone.value = parse_value_to_components(&next);
        Some(clone)
    } else {
        decl.value = parse_value_to_components(&next);
        None
    }
}
