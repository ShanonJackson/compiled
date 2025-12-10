use std::collections::HashMap;

use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    declaration_property_name, parse_value_to_components, serialize_component_values,
};
use crate::postcss::value_parser as vp;

#[derive(Debug, Default, Clone, Copy)]
pub struct NormalizePositions;

pub fn normalize_positions() -> NormalizePositions {
    NormalizePositions
}

impl Plugin for NormalizePositions {
    fn name(&self) -> &'static str {
        "postcss-normalize-positions"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        let mut cache: HashMap<String, String> = HashMap::new();
        for rule in &mut stylesheet.rules {
            walk_rule(rule, &mut cache);
        }
    }
}

fn walk_rule(rule: &mut Rule, cache: &mut HashMap<String, String>) {
    match rule {
        Rule::QualifiedRule(q) => walk_components(&mut q.block.value, cache),
        Rule::AtRule(at) => {
            if let Some(block) = &mut at.block {
                walk_components(&mut block.value, cache);
            }
        }
        Rule::ListOfComponentValues(list) => walk_components(&mut list.children, cache),
    }
}

fn walk_components(values: &mut [ComponentValue], cache: &mut HashMap<String, String>) {
    for value in values {
        match value {
            ComponentValue::Declaration(decl) => process_declaration(decl, cache),
            ComponentValue::QualifiedRule(rule) => walk_components(&mut rule.block.value, cache),
            ComponentValue::AtRule(at) => {
                if let Some(block) = &mut at.block {
                    walk_components(&mut block.value, cache);
                }
            }
            ComponentValue::SimpleBlock(block) => walk_components(&mut block.value, cache),
            ComponentValue::ListOfComponentValues(list) => walk_components(&mut list.children, cache),
            ComponentValue::KeyframeBlock(block) => walk_components(&mut block.block.value, cache),
            _ => {}
        }
    }
}

fn process_declaration(decl: &mut Declaration, cache: &mut HashMap<String, String>) {
    let prop = declaration_property_name(&decl.name);
    if !is_target_property(&prop) {
        return;
    }

    let Some(current) = serialize_component_values(&decl.value) else {
        return;
    };

    if let Some(next) = cache.get(&current) {
        if next != &current {
            decl.value = parse_value_to_components(next);
        }
        return;
    }

    let next = transform(&current);
    cache.insert(current.clone(), next.clone());

    if next != current {
        decl.value = parse_value_to_components(&next);
    }
}

fn transform(value: &str) -> String {
    let mut parsed = vp::parse(value);
    let mut ranges: Vec<Range> = Vec::new();
    let mut range_index = 0usize;
    let mut should_continue = true;

    for (idx, node) in parsed.nodes.iter().enumerate() {
        if is_comma(node) {
            range_index += 1;
            should_continue = true;
            continue;
        }

        if !should_continue {
            continue;
        }

        if is_slash(node) {
            should_continue = false;
            continue;
        }

        if range_index >= ranges.len() {
            ranges.push(Range::default());
        }

        if is_variable_function(node) {
            should_continue = false;
            ranges[range_index] = Range::default();
            continue;
        }

        let is_position_keyword =
            is_direction_keyword(node) || is_dimension(node) || is_number(node) || is_math_function(node);

        if ranges[range_index].start.is_none() && is_position_keyword {
            ranges[range_index].start = Some(idx);
            ranges[range_index].end = Some(idx);
            continue;
        }

        if ranges[range_index].start.is_some() {
            if matches!(node, vp::Node::Space { .. }) {
                continue;
            }

            if is_position_keyword {
                ranges[range_index].end = Some(idx);
            }
        }
    }

    for range in ranges {
        let Some(start) = range.start else {
            continue;
        };
        let Some(end) = range.end else {
            continue;
        };

        if start >= parsed.nodes.len() || end >= parsed.nodes.len() || start > end {
            continue;
        }

        let nodes_len = end - start + 1;
        if nodes_len > 3 {
            continue;
        }

        let mut slice = parsed.nodes[start..=end].iter_mut();
        let Some(first_node) = slice.next() else { continue };
        let first_value = node_value(first_node).to_ascii_lowercase();

        let mut second: Option<&mut vp::Node> = None;
        let mut third: Option<&mut vp::Node> = None;

        if nodes_len >= 2 {
            second = slice.next();
        }
        if nodes_len >= 3 {
            third = slice.next();
        }

        let second_value = third
            .as_ref()
            .map(|node| node_value(node).to_ascii_lowercase());

        if nodes_len == 1 || second_value.as_deref() == Some("center") {
            if let Some(node) = third.as_deref_mut() {
                set_node_value(node, "");
            }
            if let Some(node) = second.as_deref_mut() {
                set_node_value(node, "");
            }

            if let Some(mapped) = map_horizontal_or_center(&first_value) {
                set_node_value(first_node, mapped);
            }

            continue;
        }

        if let Some(second_node_value) = second_value.as_deref() {
            let second_lower = second_node_value.to_ascii_lowercase();
            if first_value == "center" && direction_keywords().contains(&second_lower.as_str()) {
                set_node_value(first_node, "");
                if let Some(node) = second.as_deref_mut() {
                    set_node_value(node, "");
                }

                if let Some(mapped) = map_horizontal(second_node_value) {
                    if let Some(node) = third.as_deref_mut() {
                        set_node_value(node, mapped);
                    }
                }

                continue;
            }

            if let Some(mapped_horizontal) = map_horizontal(&first_value) {
                if let Some(mapped_vertical) = map_vertical(second_node_value) {
                    set_node_value(first_node, mapped_horizontal);
                    if let Some(node) = third.as_deref_mut() {
                        set_node_value(node, mapped_vertical);
                    }

                    continue;
                }
            } else if let Some(mapped_vertical) = map_vertical(&first_value) {
                if let Some(mapped_horizontal) = map_horizontal(second_node_value) {
                    set_node_value(first_node, mapped_horizontal);
                    if let Some(node) = third.as_deref_mut() {
                        set_node_value(node, mapped_vertical);
                    }
                }
            }
        }
    }

    vp::stringify(&parsed.nodes)
}

#[derive(Debug, Default, Clone, Copy)]
struct Range {
    start: Option<usize>,
    end: Option<usize>,
}

fn is_target_property(prop: &str) -> bool {
    let lower = prop.to_ascii_lowercase();
    lower == "background" || lower == "background-position" || is_perspective_origin(&lower)
}

fn is_perspective_origin(prop: &str) -> bool {
    if prop.ends_with("perspective-origin") {
        let prefix = &prop[..prop.len() - "perspective-origin".len()];
        return prefix.is_empty() || (prefix.starts_with('-') && prefix.ends_with('-'));
    }
    false
}

fn is_comma(node: &vp::Node) -> bool {
    matches!(node, vp::Node::Div { value, .. } if value == ",")
}

fn is_slash(node: &vp::Node) -> bool {
    matches!(node, vp::Node::Div { value, .. } if value == "/")
}

fn is_variable_function(node: &vp::Node) -> bool {
    match node {
        vp::Node::Function { value, .. } => matches!(value.to_ascii_lowercase().as_str(), "var" | "env" | "constant"),
        _ => false,
    }
}

fn is_math_function(node: &vp::Node) -> bool {
    match node {
        vp::Node::Function { value, .. } => matches!(value.to_ascii_lowercase().as_str(), "calc" | "min" | "max" | "clamp"),
        _ => false,
    }
}

fn is_number(node: &vp::Node) -> bool {
    match node {
        vp::Node::Word { value } => value.parse::<f64>().is_ok(),
        _ => false,
    }
}

fn is_dimension(node: &vp::Node) -> bool {
    match node {
        vp::Node::Word { value } => vp::unit::unit(value).map_or(false, |u| !u.unit.is_empty()),
        _ => false,
    }
}

fn is_direction_keyword(node: &vp::Node) -> bool {
    match node {
        vp::Node::Word { value } => {
            let lower = value.to_ascii_lowercase();
            direction_keywords().contains(&lower.as_str())
        }
        _ => false,
    }
}

fn direction_keywords() -> &'static [&'static str] {
    &["top", "right", "bottom", "left", "center"]
}

fn map_horizontal(value: &str) -> Option<&'static str> {
    match value {
        "right" => Some("100%"),
        "left" => Some("0"),
        _ => None,
    }
}

fn map_vertical(value: &str) -> Option<&'static str> {
    match value {
        "bottom" => Some("100%"),
        "top" => Some("0"),
        _ => None,
    }
}

fn map_horizontal_or_center(value: &str) -> Option<&'static str> {
    match value {
        "center" => Some("50%"),
        other => map_horizontal(other),
    }
}

fn node_value(node: &vp::Node) -> String {
    match node {
        vp::Node::Word { value }
        | vp::Node::Space { value }
        | vp::Node::Div { value, .. }
        | vp::Node::Function { value, .. }
        | vp::Node::String { value, .. }
        | vp::Node::Comment { value, .. } => value.clone(),
    }
}

fn set_node_value(node: &mut vp::Node, value: &str) {
    match node {
        vp::Node::Word { value: v }
        | vp::Node::Space { value: v }
        | vp::Node::Div { value: v, .. }
        | vp::Node::Function { value: v, .. }
        | vp::Node::String { value: v, .. }
        | vp::Node::Comment { value: v, .. } => *v = value.to_string(),
    }
}
