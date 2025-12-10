use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    parse_value_to_components, serialize_component_values,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MinifyGradients;

pub fn minify_gradients() -> MinifyGradients {
    MinifyGradients
}

fn tighten_commas(s: &str) -> String {
    s.replace(", ", ",")
}

fn tighten_slashes(s: &str) -> String {
    s.replace(" / ", "/")
}

fn trim_inner_spaces(s: &str) -> String {
    // Remove spaces after '(' and before ')'
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    let bytes: Vec<char> = s.chars().collect();
    while i < bytes.len() {
        let ch = bytes[i];
        out.push(ch);
        if ch == '(' {
            while i + 1 < bytes.len() && bytes[i + 1].is_whitespace() {
                i += 1;
            }
        } else if ch == ')' && out.ends_with(' ') {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(')');
        }
        i += 1;
    }
    out
}

fn minify_gradient_value(value: &str) -> Option<String> {
    if !value.contains("gradient(") {
        return None;
    }

    let mut next = tighten_commas(value);
    next = tighten_slashes(&next);
    next = trim_inner_spaces(&next);

    if let Some(idx) = next.find("linear-gradient(") {
        if let Some(end) = next[idx..].find(',') {
            let start = idx + "linear-gradient(".len();
            let dir = next[start..idx + end].trim();
            if dir.eq_ignore_ascii_case("to bottom") {
                let mut s = next.clone();
                s.replace_range(start..idx + end + 1, "");
                next = s;
            }
        }
    }

    if next != value {
        Some(next)
    } else {
        None
    }
}

fn try_replace_value(decl: &mut Declaration, current: &str, next: String) {
    if next == current {
        return;
    }
    decl.value = parse_value_to_components(&next);
}

fn process_declaration(decl: &mut Declaration) {
    let Some(serialized) = serialize_component_values(&decl.value) else {
        return;
    };
    let Some(next) = minify_gradient_value(&serialized) else {
        return;
    };
    try_replace_value(decl, &serialized, next);
}

fn walk_rule(rule: &mut Rule) {
    match rule {
        Rule::QualifiedRule(q) => walk_components(&mut q.block.value),
        Rule::AtRule(at) => {
            if let Some(block) = &mut at.block {
                walk_components(&mut block.value);
            }
        }
        Rule::ListOfComponentValues(list) => walk_components(&mut list.children),
    }
}

fn walk_components(values: &mut [ComponentValue]) {
    for value in values {
        match value {
            ComponentValue::Declaration(decl) => process_declaration(decl),
            ComponentValue::QualifiedRule(rule) => walk_components(&mut rule.block.value),
            ComponentValue::AtRule(at) => {
                if let Some(block) = &mut at.block {
                    walk_components(&mut block.value);
                }
            }
            ComponentValue::SimpleBlock(block) => walk_components(&mut block.value),
            ComponentValue::ListOfComponentValues(list) => walk_components(&mut list.children),
            ComponentValue::KeyframeBlock(block) => walk_components(&mut block.block.value),
            _ => {}
        }
    }
}

impl Plugin for MinifyGradients {
    fn name(&self) -> &'static str {
        "postcss-minify-gradients"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        for rule in &mut stylesheet.rules {
            walk_rule(rule);
        }
    }
}
