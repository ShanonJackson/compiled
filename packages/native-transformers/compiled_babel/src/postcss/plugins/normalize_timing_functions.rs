use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    parse_value_to_components, serialize_component_values,
};

fn eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.0001
}

fn normalize_cubic_bezier(s: &str) -> Option<String> {
    let body = s.strip_prefix("cubic-bezier(")?.strip_suffix(")")?;
    let nums: Vec<f32> = body
        .split(',')
        .map(|p| p.trim())
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    if nums.len() != 4 {
        return None;
    }
    let (a, b, c, d) = (nums[0], nums[1], nums[2], nums[3]);
    if eq(a, 0.25) && eq(b, 0.1) && eq(c, 0.25) && eq(d, 1.0) {
        return Some("ease".to_string());
    }
    if eq(a, 0.0) && eq(b, 0.0) && eq(c, 1.0) && eq(d, 1.0) {
        return Some("linear".to_string());
    }
    if eq(a, 0.42) && eq(b, 0.0) && eq(c, 1.0) && eq(d, 1.0) {
        return Some("ease-in".to_string());
    }
    if eq(a, 0.0) && eq(b, 0.0) && eq(c, 0.58) && eq(d, 1.0) {
        return Some("ease-out".to_string());
    }
    if eq(a, 0.42) && eq(b, 0.0) && eq(c, 0.58) && eq(d, 1.0) {
        return Some("ease-in-out".to_string());
    }
    None
}

fn normalize_steps(s: &str) -> Option<String> {
    let body = s.strip_prefix("steps(")?.strip_suffix(")")?;
    let parts: Vec<&str> = body.split(',').map(|p| p.trim()).collect();
    if parts.len() == 2 && parts[0] == "1" {
        let kw = parts[1].to_ascii_lowercase();
        if kw == "start" {
            return Some("step-start".to_string());
        }
        if kw == "end" {
            return Some("step-end".to_string());
        }
    }
    None
}

fn normalize_timing_functions_value(value: &str) -> Option<String> {
    let mut next = value.to_string();
    let mut i = 0usize;
    let mut changed = false;
    while let Some(start) = next[i..].find("cubic-bezier(") {
        let idx = i + start;
        if let Some(end_rel) = next[idx..].find(')') {
            let end = idx + end_rel + 1;
            let seg = &next[idx..end];
            if let Some(name) = normalize_cubic_bezier(seg) {
                next.replace_range(idx..end, &name);
                i = idx + name.len();
                changed = true;
                continue;
            }
            i = end;
        } else {
            break;
        }
    }

    let mut j = 0usize;
    while let Some(start) = next[j..].find("steps(") {
        let idx = j + start;
        if let Some(end_rel) = next[idx..].find(')') {
            let end = idx + end_rel + 1;
            let seg = &next[idx..end];
            if let Some(name) = normalize_steps(seg) {
                next.replace_range(idx..end, &name);
                j = idx + name.len();
                changed = true;
                continue;
            }
            j = end;
        } else {
            break;
        }
    }

    if changed {
        Some(next)
    } else {
        None
    }
}

fn rewrite_declaration(decl: &mut Declaration) {
    let Some(current) = serialize_component_values(&decl.value) else {
        return;
    };
    let Some(next) = normalize_timing_functions_value(&current) else {
        return;
    };
    decl.value = parse_value_to_components(&next);
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
            ComponentValue::Declaration(decl) => rewrite_declaration(decl),
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

#[derive(Debug, Default, Clone, Copy)]
pub struct NormalizeTimingFunctions;

pub fn normalize_timing_functions() -> NormalizeTimingFunctions {
    NormalizeTimingFunctions
}

impl Plugin for NormalizeTimingFunctions {
    fn name(&self) -> &'static str {
        "postcss-normalize-timing-functions"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        for rule in &mut stylesheet.rules {
            walk_rule(rule);
        }
    }
}
