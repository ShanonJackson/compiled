use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    parse_value_to_components, serialize_component_values,
};
use crate::postcss::utils::selector_stringifier;
use crate::postcss::value_parser as vp;
use swc_core::css::ast::{ComponentValue, QualifiedRulePrelude, Rule, SelectorList, Stylesheet};

fn parse_string_ast(input: &str) -> (Vec<(String, &'static str)>, bool) {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut nodes: Vec<(String, &'static str)> = Vec::new();
    let mut quotes = false;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        match ch {
            ' ' | '\t' | '\r' | '\x0C' => {
                let start = i;
                let mut j = i;
                while j < bytes.len() {
                    let c = bytes[j] as char;
                    if c == ' ' || c == '\n' || c == '\t' || c == '\r' || c == '\x0C' {
                        j += 1;
                    } else {
                        break;
                    }
                }
                nodes.push((input[start..j].to_string(), "space"));
                i = j;
                continue;
            }
            '\'' => {
                nodes.push(("'".to_string(), "squote"));
                quotes = true;
                i += 1;
                continue;
            }
            '"' => {
                nodes.push(('"'.to_string(), "dquote"));
                quotes = true;
                i += 1;
                continue;
            }
            '\\' => {
                let next = if i + 1 < bytes.len() {
                    bytes[i + 1] as char
                } else {
                    '\0'
                };
                if next == '\'' {
                    nodes.push(("\\'".to_string(), "esc_squote"));
                    quotes = true;
                    i += 2;
                    continue;
                }
                if next == '"' {
                    nodes.push(("\\\"".to_string(), "esc_dquote"));
                    quotes = true;
                    i += 2;
                    continue;
                }
                if next == '\n' {
                    nodes.push(("\\\n".to_string(), "newline"));
                    i += 2;
                    continue;
                }
            }
            _ => {}
        }
        let start = i;
        let mut j = i + 1;
        while j < bytes.len() {
            let c = bytes[j] as char;
            if c == ' '
                || c == '\n'
                || c == '\t'
                || c == '\r'
                || c == '\x0C'
                || c == '\\'
                || c == '\''
                || c == '"'
            {
                break;
            }
            j += 1;
        }
        nodes.push((input[start..j].to_string(), "string"));
        i = j;
    }
    (nodes, quotes)
}

fn stringify_ast(nodes: &[(String, &'static str)]) -> String {
    let mut out = String::new();
    for (v, kind) in nodes.iter() {
        if *kind == "newline" {
            continue;
        }
        out.push_str(v);
    }
    out
}

fn change_wrapping_quotes(node_quote: &mut char, nodes: &mut Vec<(String, &'static str)>) {
    let mut has_squote = 0;
    let mut has_dquote = 0;
    let mut esc_s = 0;
    let mut esc_d = 0;
    for (_, k) in nodes.iter() {
        match *k {
            "squote" => has_squote += 1,
            "dquote" => has_dquote += 1,
            "esc_squote" => esc_s += 1,
            "esc_dquote" => esc_d += 1,
            _ => {}
        }
    }
    if has_squote == 0 && has_dquote == 0 {
        if *node_quote == '\'' && esc_s > 0 && esc_d == 0 {
            *node_quote = '"';
        } else if *node_quote == '"' && esc_d > 0 && esc_s == 0 {
            *node_quote = '\'';
        }
    }
    let parent = *node_quote;
    let mut replaced: Vec<(String, &'static str)> = Vec::with_capacity(nodes.len());
    for (v, k) in nodes.drain(..) {
        if k == "esc_dquote" && parent == '\'' {
            replaced.push(('"'.to_string(), "dquote"));
        } else if k == "esc_squote" && parent == '"' {
            replaced.push(("'".to_string(), "squote"));
        } else {
            replaced.push((v, k));
        }
    }
    *nodes = replaced;
}

fn normalize_value(value: &str, preferred_quote: char) -> String {
    if !value.contains('\'') && !value.contains('"') && !value.contains('\\') && !value.contains('\n') {
        return value.to_string();
    }
    if let Ok(max_str) = std::env::var("COMPILED_STRING_NORM_MAXLEN") {
        if let Ok(max) = max_str.parse::<usize>() {
            if value.len() > max {
                return value.to_string();
            }
        }
    }

    let mut parsed = vp::parse(value);
    vp::walk(
        &mut parsed.nodes[..],
        &mut |n| {
            match n {
                vp::Node::String { value: s, quote, .. } => {
                    let (mut nodes, quotes) = parse_string_ast(s);
                    let mut q = *quote;
                    if q == '\0' {
                        q = preferred_quote;
                    }
                    if quotes {
                        change_wrapping_quotes(&mut q, &mut nodes);
                    } else {
                        q = preferred_quote;
                    }
                    *quote = q;
                    *s = stringify_ast(&nodes);
                }
                _ => {}
            }
            true
        },
        false,
    );
    vp::stringify(&parsed.nodes)
}

fn rewrite_declaration_value(values: &mut Vec<ComponentValue>, preferred_quote: char) {
    let Some(current) = serialize_component_values(values) else { return; };
    if !current.contains('\'') && !current.contains('"') && !current.contains('\\') && !current.contains('\n') {
        return;
    }
    let next = normalize_value(&current, preferred_quote);
    if next != current {
        *values = parse_value_to_components(&next);
    }
}

fn rewrite_selector_list(list: &mut SelectorList, preferred_quote: char) {
    let serialized = selector_stringifier::serialize_selector_list(list);
    if !serialized.contains('\'') && !serialized.contains('"') && !serialized.contains('\\') && !serialized.contains('\n') {
        return;
    }
    let next = normalize_value(&serialized, preferred_quote);
    if next == serialized {
        return;
    }
    if let Some(parsed) = selector_stringifier::parse_selector_list_from_str(&next) {
        *list = parsed;
    }
}

fn walk_rule(rule: &mut Rule, preferred_quote: char) {
    match rule {
        Rule::QualifiedRule(rule) => {
            if let QualifiedRulePrelude::SelectorList(list) = &mut rule.prelude {
                rewrite_selector_list(list, preferred_quote);
            }
            walk_components(&mut rule.block.value, preferred_quote);
        }
        Rule::AtRule(at) => {
            if let Some(block) = &mut at.block {
                walk_components(&mut block.value, preferred_quote);
            }
        }
        Rule::ListOfComponentValues(list) => walk_components(&mut list.children, preferred_quote),
    }
}

fn walk_components(values: &mut [ComponentValue], preferred_quote: char) {
    for value in values.iter_mut() {
        match value {
            ComponentValue::Declaration(decl) => rewrite_declaration_value(&mut decl.value, preferred_quote),
            ComponentValue::QualifiedRule(rule) => {
                if let QualifiedRulePrelude::SelectorList(list) = &mut rule.prelude {
                    rewrite_selector_list(list, preferred_quote);
                }
                walk_components(&mut rule.block.value, preferred_quote);
            }
            ComponentValue::AtRule(at) => {
                if let Some(block) = &mut at.block {
                    walk_components(&mut block.value, preferred_quote);
                }
            }
            ComponentValue::SimpleBlock(block) => walk_components(&mut block.value, preferred_quote),
            ComponentValue::ListOfComponentValues(list) => walk_components(&mut list.children, preferred_quote),
            ComponentValue::KeyframeBlock(block) => walk_components(&mut block.block.value, preferred_quote),
            ComponentValue::Function(fun) => walk_components(&mut fun.value, preferred_quote),
            _ => {}
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NormalizeString;

pub fn normalize_string() -> NormalizeString {
    NormalizeString
}

impl Plugin for NormalizeString {
    fn name(&self) -> &'static str {
        "postcss-normalize-string"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        let preferred_quote = '"';
        for rule in &mut stylesheet.rules {
            walk_rule(rule, preferred_quote);
        }
    }
}
