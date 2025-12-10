use regex::Regex;
use swc_core::css::ast::{ComponentValue, Declaration, Rule, Stylesheet};

use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    declaration_property_name, parse_value_to_components, serialize_component_values,
};

fn merge_range_bounds(left: &str, right: &str) -> Option<String> {
    let lchars: Vec<char> = left.chars().collect();
    let rchars: Vec<char> = right.chars().collect();
    if lchars.len() != rchars.len() {
        return None;
    }
    let mut question = 0usize;
    let mut group = String::from("u+");
    for i in 0..lchars.len() {
        let lc = lchars[i];
        let rc = rchars[i];
        if lc == rc && question == 0 {
            group.push(lc);
        } else if lc == '0' && rc == 'f' {
            question += 1;
            group.push('?');
        } else {
            return None;
        }
    }
    if question < 6 {
        Some(group)
    } else {
        None
    }
}

fn normalize_single_range(range: &str) -> String {
    let r = range.to_lowercase();
    let mut parts = r[2..].splitn(2, '-');
    let a = parts.next().unwrap_or("");
    if let Some(b) = parts.next() {
        if let Some(merged) = merge_range_bounds(a, b) {
            return merged;
        }
    }
    r
}

fn normalize_unicode_range(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    let re = Regex::new(r"(?i)u\+[0-9a-f?]+(?:-[0-9a-f?]+)?").unwrap();
    let next = re
        .replace_all(value, |caps: &regex::Captures| {
            normalize_single_range(&caps[0])
        })
        .to_string();
    if next != value {
        Some(next)
    } else {
        None
    }
}

fn process_declaration(decl: &mut Declaration) {
    if !declaration_property_name(&decl.name).eq_ignore_ascii_case("unicode-range") {
        return;
    }

    let Some(current) = serialize_component_values(&decl.value) else {
        return;
    };
    let Some(next) = normalize_unicode_range(&current) else {
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

#[derive(Debug, Default, Clone, Copy)]
pub struct NormalizeUnicode;

pub fn normalize_unicode() -> NormalizeUnicode {
    NormalizeUnicode
}

impl Plugin for NormalizeUnicode {
    fn name(&self) -> &'static str {
        "postcss-normalize-unicode"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        for rule in &mut stylesheet.rules {
            walk_rule(rule);
        }
    }
}
