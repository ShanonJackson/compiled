use std::collections::HashMap;

use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    declaration_property_name, parse_value_to_components, serialize_component_values,
};
use crate::postcss::plugins::normalize_css_engine::ordered_values::{
    lib,
    rules,
};
use crate::postcss::value_parser as vp;
use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

#[derive(Debug, Default, Clone, Copy)]
pub struct OrderedValues;

impl Plugin for OrderedValues {
    fn name(&self) -> &'static str {
        "postcss-ordered-values"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        let mut cache = HashMap::new();
        normalize_stylesheet(stylesheet, &mut cache);
    }
}

pub fn ordered_values() -> OrderedValues {
    OrderedValues
}

type ValueCache = HashMap<String, String>;

fn normalize_stylesheet(stylesheet: &mut Stylesheet, cache: &mut ValueCache) {
    normalize_rules(&mut stylesheet.rules, cache);
}

fn normalize_rules(rules: &mut Vec<Rule>, cache: &mut ValueCache) {
    for rule in rules.iter_mut() {
        match rule {
            Rule::QualifiedRule(rule) => {
                normalize_component_values(&mut rule.block.value, cache);
            }
            Rule::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    normalize_component_values(&mut block.value, cache);
                }
            }
            Rule::ListOfComponentValues(list) => {
                normalize_component_values(&mut list.children, cache);
            }
        }
    }
}

fn normalize_component_values(values: &mut Vec<ComponentValue>, cache: &mut ValueCache) {
    for value in values.iter_mut() {
        match value {
            ComponentValue::Declaration(declaration) => {
                normalize_declaration(declaration, cache);
            }
            ComponentValue::QualifiedRule(rule) => {
                normalize_component_values(&mut rule.block.value, cache);
            }
            ComponentValue::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    normalize_component_values(&mut block.value, cache);
                }
            }
            ComponentValue::SimpleBlock(block) => {
                normalize_component_values(&mut block.value, cache);
            }
            ComponentValue::ListOfComponentValues(list) => {
                normalize_component_values(&mut list.children, cache);
            }
            ComponentValue::Function(function) => {
                normalize_component_values(&mut function.value, cache);
            }
            ComponentValue::KeyframeBlock(block) => {
                normalize_component_values(&mut block.block.value, cache);
            }
            _ => {}
        }
    }
}

fn normalize_declaration(declaration: &mut Declaration, cache: &mut ValueCache) {
    let Some(original_value) = serialize_value_with_spacing(&declaration.value) else {
        return;
    };

    if original_value.trim().is_empty() {
        return;
    }

    let property = declaration_property_name(&declaration.name);
    let normalized_prop = vendor_unprefixed(&property.to_ascii_lowercase());

    if !is_supported_property(&normalized_prop) {
        return;
    }

    if let Some(cached) = cache.get(&original_value) {
        declaration.value = parse_value_to_components(cached);
        return;
    }

    let parsed = vp::parse(&original_value);

    if parsed.nodes.len() < 2 || should_abort(&parsed) {
        cache.insert(original_value.clone(), original_value.clone());
        return;
    }

    let output = process_value(&normalized_prop, &parsed);

    cache.insert(original_value.clone(), output.clone());
    declaration.value = parse_value_to_components(&output);
}

fn process_value(prop: &str, parsed: &vp::ParsedValue) -> String {
    match prop {
        "border"
        | "border-top"
        | "border-right"
        | "border-bottom"
        | "border-left"
        | "border-block"
        | "border-inline"
        | "border-block-start"
        | "border-block-end"
        | "border-inline-start"
        | "border-inline-end"
        | "outline"
        | "column-rule" => rules::border::normalize(parsed),
        "transition" => rules::transition::normalize(parsed),
        "list-style" => rules::list_style::normalize(parsed),
        "columns" => rules::columns::normalize(parsed),
        "flex-flow" => rules::flex_flow::normalize(parsed),
        "animation" => lib::get_value::get_value(lib::arguments::get_arguments(parsed)),
        "box-shadow" => rules::box_shadow::normalize(parsed).unwrap_or_else(|_| vp::stringify(&parsed.nodes)),
        "grid-auto-flow" => rules::grid::normalize_auto_flow(parsed),
        "grid-column-gap" | "grid-row-gap" => rules::grid::normalize_gap(parsed),
        "grid-column"
        | "grid-row"
        | "grid-row-start"
        | "grid-row-end"
        | "grid-column-start"
        | "grid-column-end" => rules::grid::normalize_line(parsed),
        _ => vp::stringify(&parsed.nodes),
    }
}

fn is_supported_property(prop: &str) -> bool {
    matches!(
        prop,
        "animation"
            | "outline"
            | "box-shadow"
            | "flex-flow"
            | "list-style"
            | "transition"
            | "border"
            | "border-top"
            | "border-right"
            | "border-bottom"
            | "border-left"
            | "border-block"
            | "border-inline"
            | "border-block-start"
            | "border-block-end"
            | "border-inline-start"
            | "border-inline-end"
            | "columns"
            | "column-rule"
            | "grid-auto-flow"
            | "grid-column-gap"
            | "grid-row-gap"
            | "grid-column"
            | "grid-row"
            | "grid-row-start"
            | "grid-row-end"
            | "grid-column-start"
            | "grid-column-end"
    )
}

fn vendor_unprefixed(value: &str) -> String {
    if let Some(rest) = value.strip_prefix('-') {
        if let Some(idx) = rest.find('-') {
            return rest[idx + 1..].to_string();
        }
    }
    value.to_string()
}

fn is_variable_function_node(node: &vp::Node) -> bool {
    match node {
        vp::Node::Function { value, .. } => {
            let name = value.to_lowercase();
            matches!(name.as_str(), "var" | "env" | "constant")
        }
        _ => false,
    }
}

fn should_abort(parsed: &vp::ParsedValue) -> bool {
    let mut abort = false;
    let mut nodes = parsed.nodes.clone();
    vp::walk(&mut nodes[..], &mut |node| {
        match node {
            vp::Node::Comment { .. } => abort = true,
            n if is_variable_function_node(n) => abort = true,
            vp::Node::Word { value } if value.contains("___CSS_LOADER_IMPORT___") => abort = true,
            _ => {}
        }
        !abort
    }, false);
    abort
}

fn serialize_value_with_spacing(components: &[ComponentValue]) -> Option<String> {
    let mut result = String::new();

    for component in components {
        let piece = serialize_component_values(&[component.clone()])?;
        if piece.is_empty() {
            continue;
        }

        if needs_space_between(&result, &piece) {
            result.push(' ');
        }

        result.push_str(&piece);
    }

    Some(result)
}

fn needs_space_between(existing: &str, next: &str) -> bool {
    if existing.is_empty() {
        return false;
    }

    let prev = existing.chars().rev().find(|ch| !ch.is_whitespace());
    let next_char = next.chars().find(|ch| !ch.is_whitespace());

    match (prev, next_char) {
        (Some(','), Some(_)) => true,
        (Some('/'), Some(_)) => true,
        (Some(prev), Some(next)) => {
            let prev_word = is_word_like_char(prev);
            let next_word = is_word_like_char(next);
            (prev_word && next_word) || (matches!(prev, ')' | '%') && next_word)
        }
        _ => false,
    }
}

fn is_word_like_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}
