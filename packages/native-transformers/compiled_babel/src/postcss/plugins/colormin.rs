use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

use super::super::transform::{Plugin, TransformContext};
use super::normalize_css_engine::colormin::{
    default_options_with_browsers, transform_value, ColorminOptions,
};
use crate::postcss::plugins::expand_shorthands::types::{
    declaration_property_name, parse_value_to_components, serialize_component_values,
};

#[derive(Debug)]
pub struct ColorMin {
    options: ColorminOptions,
    browsers: Vec<String>,
    cache: HashMap<String, String>,
}

impl Default for ColorMin {
    fn default() -> Self {
        let (options, browsers) = default_options_with_browsers();
        Self {
            options,
            browsers,
            cache: HashMap::new(),
        }
    }
}

impl Plugin for ColorMin {
    fn name(&self) -> &'static str {
        "postcss-colormin"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        // Mutable cache but immutable plugin instance: rebuild a small cache map to mirror JS behaviour.
        let mut cache = self.cache.clone();
        normalize_stylesheet(stylesheet, &self.options, &self.browsers, &mut cache);
    }
}

pub fn colormin() -> ColorMin {
    ColorMin::default()
}

static SKIP_PROP: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(composes|font|src$|filter|-webkit-tap-highlight-color)").unwrap()
});

fn normalize_stylesheet(
    stylesheet: &mut Stylesheet,
    options: &ColorminOptions,
    browsers: &[String],
    cache: &mut HashMap<String, String>,
) {
    normalize_rules(&mut stylesheet.rules, options, browsers, cache);
}

fn normalize_rules(
    rules: &mut Vec<Rule>,
    options: &ColorminOptions,
    browsers: &[String],
    cache: &mut HashMap<String, String>,
) {
    for rule in rules.iter_mut() {
        match rule {
            Rule::QualifiedRule(rule) => {
                normalize_values(&mut rule.block.value, options, browsers, cache)
            }
            Rule::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    normalize_values(&mut block.value, options, browsers, cache);
                }
            }
            Rule::ListOfComponentValues(list) => {
                normalize_components(&mut list.children, options, browsers, cache);
            }
        }
    }
}

fn normalize_components(
    values: &mut [ComponentValue],
    options: &ColorminOptions,
    browsers: &[String],
    cache: &mut HashMap<String, String>,
) {
    for v in values {
        match v {
            ComponentValue::Declaration(decl) => {
                normalize_declaration(decl, options, browsers, cache)
            }
            ComponentValue::QualifiedRule(rule) => {
                normalize_values(&mut rule.block.value, options, browsers, cache);
            }
            ComponentValue::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    normalize_values(&mut block.value, options, browsers, cache);
                }
            }
            ComponentValue::SimpleBlock(block) => {
                normalize_values(&mut block.value, options, browsers, cache);
            }
            ComponentValue::ListOfComponentValues(list) => {
                normalize_components(&mut list.children, options, browsers, cache);
            }
            ComponentValue::Function(fun) => {
                normalize_components(&mut fun.value, options, browsers, cache)
            }
            ComponentValue::KeyframeBlock(block) => {
                normalize_values(&mut block.block.value, options, browsers, cache);
            }
            _ => {}
        }
    }
}

fn normalize_values(
    values: &mut Vec<ComponentValue>,
    options: &ColorminOptions,
    browsers: &[String],
    cache: &mut HashMap<String, String>,
) {
    for v in values.iter_mut() {
        match v {
            ComponentValue::Declaration(decl) => {
                normalize_declaration(decl, options, browsers, cache)
            }
            ComponentValue::QualifiedRule(rule) => {
                normalize_values(&mut rule.block.value, options, browsers, cache);
            }
            ComponentValue::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    normalize_values(&mut block.value, options, browsers, cache);
                }
            }
            ComponentValue::SimpleBlock(block) => {
                normalize_values(&mut block.value, options, browsers, cache);
            }
            ComponentValue::ListOfComponentValues(list) => {
                normalize_components(&mut list.children, options, browsers, cache);
            }
            ComponentValue::Function(fun) => {
                normalize_components(&mut fun.value, options, browsers, cache)
            }
            ComponentValue::KeyframeBlock(block) => {
                normalize_values(&mut block.block.value, options, browsers, cache);
            }
            _ => {}
        }
    }
}

fn normalize_declaration(
    declaration: &mut Declaration,
    options: &ColorminOptions,
    browsers: &[String],
    cache: &mut HashMap<String, String>,
) {
    let prop_name = declaration_property_name(&declaration.name);
    if SKIP_PROP.is_match(&prop_name) {
        return;
    }

    let Some(original_value) = serialize_component_values(&declaration.value) else {
        return;
    };

    if original_value.is_empty() {
        return;
    }

    let cache_key = format!(
        "{:?}",
        (
            &original_value,
            options.transparent,
            options.alpha_hex,
            options.name,
            browsers
        )
    );

    if let Some(cached) = cache.get(&cache_key) {
        declaration.value = parse_value_to_components(cached);
        return;
    }

    let new_value = transform_value(&original_value, options);
    declaration.value = parse_value_to_components(&new_value);
    cache.insert(cache_key, new_value);
}
