use once_cell::sync::Lazy;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::css::shorthand::shorthand_bucket;

const STYLE_ORDER: &[&str] = &[
    ":link",
    ":visited",
    ":focus-within",
    ":focus",
    ":focus-visible",
    ":hover",
    ":active",
];

/// Configuration toggles that influence how style sheets are ordered.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct SortConfig {
    pub sort_at_rules_enabled: bool,
    pub sort_shorthand_enabled: bool,
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            sort_at_rules_enabled: true,
            sort_shorthand_enabled: true,
        }
    }
}

/// Sorts an atomic style sheet into a deterministic order mirroring the
/// JavaScript implementation.
#[allow(dead_code)]
pub fn sort_atomic_style_sheet(rules: &[String], config: SortConfig) -> String {
    if rules.is_empty() {
        return String::new();
    }

    let mut sorted = rules.to_vec();
    sorted.sort();
    let css = sorted.join("\n");

    let nodes = merge_duplicate_at_rules(parse_stylesheet(&css));

    let mut catch_all: Vec<Node> = Vec::new();
    let mut rule_nodes: Vec<RuleNode> = Vec::new();
    let mut at_rule_infos: Vec<AtRuleInfo> = Vec::new();

    for node in nodes {
        match node {
            Node::Rule(rule) => rule_nodes.push(rule),
            Node::AtRule(at_rule) => {
                let params = at_rule.params.as_str();
                let parsed = if config.sort_at_rules_enabled {
                    parse_media_query(params)
                } else {
                    Vec::new()
                };

                at_rule_infos.push(AtRuleInfo {
                    parsed,
                    query: at_rule.params.clone(),
                    at_rule_name: at_rule.name.clone(),
                    node: at_rule,
                });
            }
            Node::Other(other) => catch_all.push(Node::Other(other)),
        }
    }

    if config.sort_shorthand_enabled {
        sort_shorthand_nodes(&mut catch_all);
        sort_shorthand_rules(&mut rule_nodes);
        for info in &mut at_rule_infos {
            sort_shorthand_nodes(&mut info.node.children);
        }
    }

    sort_pseudo_selectors(&mut rule_nodes);

    if config.sort_at_rules_enabled {
        at_rule_infos.sort_by(sort_at_rules);
    }

    for info in &mut at_rule_infos {
        sort_at_rule_pseudo_selectors(&mut info.node);
    }

    let mut output: Vec<String> = Vec::new();

    output.extend(catch_all.into_iter().map(|node| format_node(&node)));
    output.extend(rule_nodes.into_iter().map(|rule| format_rule(&rule)));
    output.extend(
        at_rule_infos
            .into_iter()
            .map(|info| format_at_rule(&info.node)),
    );

    output.join("\n")
}

#[derive(Clone, Debug)]
struct RuleNode {
    selector: String,
    declarations: Vec<Declaration>,
}

#[derive(Clone, Debug)]
struct Declaration {
    property: String,
    value: String,
}

#[derive(Clone, Debug)]
struct AtRuleNode {
    name: String,
    params: String,
    children: Vec<Node>,
}

#[derive(Clone, Debug)]
enum Node {
    Rule(RuleNode),
    AtRule(AtRuleNode),
    Other(String),
}

fn parse_stylesheet(source: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut index = 0;
    let bytes = source.as_bytes();
    let len = bytes.len();

    while index < len {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }

        if bytes[index] == b'@' {
            let name_start = index + 1;
            index += 1;
            while index < len && is_name_char(bytes[index]) {
                index += 1;
            }

            let name = source[name_start..index].to_string();
            let params_start = index;

            while index < len && bytes[index] != b'{' && bytes[index] != b';' {
                index += 1;
            }

            if index >= len {
                break;
            }

            if bytes[index] == b';' {
                let raw = source[name_start - 1..=index].trim();
                if !raw.is_empty() {
                    nodes.push(Node::Other(raw.to_string()));
                }
                index += 1;
                continue;
            }

            let params = source[params_start..index].to_string();
            let brace_index = index;

            if let Some(body_end) = find_matching_brace(bytes, brace_index) {
                let body = &source[brace_index + 1..body_end];
                let children = parse_stylesheet(body);
                nodes.push(Node::AtRule(AtRuleNode {
                    name,
                    params,
                    children,
                }));
                index = body_end + 1;
            } else {
                break;
            }

            continue;
        }

        let selector_start = index;
        while index < len && bytes[index] != b'{' {
            index += 1;
        }

        if index >= len {
            break;
        }

        let selector = source[selector_start..index].trim().to_string();
        let brace_index = index;

        if let Some(body_end) = find_matching_brace(bytes, brace_index) {
            let body = &source[brace_index + 1..body_end];
            let declarations = parse_declarations(body);
            nodes.push(Node::Rule(RuleNode {
                selector,
                declarations,
            }));
            index = body_end + 1;
        } else {
            break;
        }
    }

    nodes
}

fn is_name_char(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'-'
}

fn find_matching_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().enumerate().skip(start) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_declarations(body: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    for part in body.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((property, value)) = trimmed.split_once(':') {
            declarations.push(Declaration {
                property: property.trim().to_string(),
                value: value.trim().to_string(),
            });
        }
    }

    declarations
}

fn merge_duplicate_at_rules(nodes: Vec<Node>) -> Vec<Node> {
    struct Bucket {
        node: AtRuleNode,
        seen: HashSet<String>,
    }

    let mut merged = Vec::new();
    let mut buckets: HashMap<(String, String), Bucket> = HashMap::new();
    let mut order: Vec<(String, String)> = Vec::new();

    for node in nodes {
        match node {
            Node::AtRule(mut at_rule) => {
                let key = (at_rule.name.clone(), at_rule.params.clone());
                let bucket = buckets.entry(key.clone()).or_insert_with(|| {
                    order.push(key.clone());
                    Bucket {
                        node: AtRuleNode {
                            name: key.0.clone(),
                            params: key.1.clone(),
                            children: Vec::new(),
                        },
                        seen: HashSet::new(),
                    }
                });

                for child in at_rule.children.drain(..) {
                    let rendered = format_node(&child);
                    if bucket.seen.insert(rendered.clone()) {
                        bucket.node.children.push(child);
                    }
                }
            }
            other => merged.push(other),
        }
    }

    for key in order {
        if let Some(bucket) = buckets.remove(&key) {
            merged.push(Node::AtRule(bucket.node));
        }
    }

    merged
}

fn sort_shorthand_nodes(nodes: &mut Vec<Node>) {
    for node in nodes.iter_mut() {
        if let Node::AtRule(at_rule) = node {
            sort_shorthand_nodes(&mut at_rule.children);
        }
    }

    nodes.sort_by(|a, b| {
        let a_bucket = first_shorthand_bucket_node(a);
        let b_bucket = first_shorthand_bucket_node(b);
        a_bucket.cmp(&b_bucket)
    });
}

fn sort_shorthand_rules(rules: &mut Vec<RuleNode>) {
    rules.sort_by(|a, b| {
        let a_bucket = first_shorthand_bucket_rule(a);
        let b_bucket = first_shorthand_bucket_rule(b);
        a_bucket.cmp(&b_bucket)
    });
}

fn first_shorthand_bucket_node(node: &Node) -> usize {
    first_declaration_property(node)
        .and_then(|property| shorthand_bucket(property))
        .unwrap_or(usize::MAX)
}

fn first_shorthand_bucket_rule(rule: &RuleNode) -> usize {
    rule.declarations
        .first()
        .and_then(|decl| shorthand_bucket(&decl.property))
        .unwrap_or(usize::MAX)
}

fn first_declaration_property(node: &Node) -> Option<&str> {
    match node {
        Node::Rule(rule) => rule.declarations.first().map(|decl| decl.property.as_str()),
        Node::AtRule(at_rule) => {
            for child in &at_rule.children {
                if let Some(prop) = first_declaration_property(child) {
                    return Some(prop);
                }
            }
            None
        }
        Node::Other(_) => None,
    }
}

fn sort_pseudo_selectors(rules: &mut Vec<RuleNode>) {
    rules.sort_by(|a, b| pseudo_score(&a.selector).cmp(&pseudo_score(&b.selector)));
}

fn pseudo_score(selector: &str) -> usize {
    let primary = selector
        .split(',')
        .next()
        .map(|part| part.trim())
        .unwrap_or(selector);

    for (index, pseudo) in STYLE_ORDER.iter().enumerate() {
        if primary.ends_with(pseudo) {
            return index + 1;
        }
    }

    0
}

#[derive(Clone)]
struct AtRuleInfo {
    parsed: Vec<ParsedAtRule>,
    node: AtRuleNode,
    at_rule_name: String,
    query: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Property {
    Width,
    Height,
    DeviceWidth,
    DeviceHeight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComparisonOperator {
    Less,
    LessEq,
    Equal,
    Greater,
    GreaterEq,
}

#[derive(Clone, Debug)]
struct ParsedAtRule {
    property: Property,
    comparison_operator: ComparisonOperator,
    length: f64,
    index: usize,
}

fn sort_at_rules(first: &AtRuleInfo, second: &AtRuleInfo) -> Ordering {
    let name_order = first.at_rule_name.cmp(&second.at_rule_name);
    if name_order != Ordering::Equal {
        return name_order;
    }

    let limit = first.parsed.len().min(second.parsed.len());
    for idx in 0..limit {
        let lhs = &first.parsed[idx];
        let rhs = &second.parsed[idx];
        let lhs_key = sort_key(lhs);
        let rhs_key = sort_key(rhs);

        if lhs_key != rhs_key {
            return lhs_key.cmp(&rhs_key);
        }

        let lhs_weight = length_sort_value(lhs);
        let rhs_weight = length_sort_value(rhs);
        if (lhs_weight - rhs_weight).abs() > f64::EPSILON {
            return lhs_weight
                .partial_cmp(&rhs_weight)
                .unwrap_or(Ordering::Equal);
        }
    }

    if first.parsed.len() + second.parsed.len() > 0 && first.parsed.len() != second.parsed.len() {
        return first.parsed.len().cmp(&second.parsed.len());
    }

    first.query.cmp(&second.query)
}

fn sort_key(rule: &ParsedAtRule) -> usize {
    property_order(rule.property) + operator_order(rule.comparison_operator)
}

fn property_order(property: Property) -> usize {
    match property {
        Property::Width => 1,
        Property::Height => 2,
        Property::DeviceWidth => 101,
        Property::DeviceHeight => 102,
    }
}

fn operator_order(operator: ComparisonOperator) -> usize {
    match operator {
        ComparisonOperator::Greater => 10,
        ComparisonOperator::GreaterEq => 20,
        ComparisonOperator::Less => 30,
        ComparisonOperator::LessEq => 40,
        ComparisonOperator::Equal => 50,
    }
}

fn length_sort_value(rule: &ParsedAtRule) -> f64 {
    match rule.comparison_operator {
        ComparisonOperator::Greater | ComparisonOperator::GreaterEq | ComparisonOperator::Equal => {
            rule.length
        }
        ComparisonOperator::Less | ComparisonOperator::LessEq => -rule.length,
    }
}

fn sort_at_rule_pseudo_selectors(at_rule: &mut AtRuleNode) {
    let mut nested_rules = Vec::new();
    let mut others = Vec::new();

    for child in at_rule.children.drain(..) {
        match child {
            Node::Rule(rule) => nested_rules.push(rule),
            Node::AtRule(mut nested_at_rule) => {
                sort_at_rule_pseudo_selectors(&mut nested_at_rule);
                others.push(Node::AtRule(nested_at_rule));
            }
            Node::Other(other) => others.push(Node::Other(other)),
        }
    }

    sort_pseudo_selectors(&mut nested_rules);

    at_rule.children = others;
    for rule in nested_rules {
        at_rule.children.push(Node::Rule(rule));
    }
}

fn parse_media_query(params: &str) -> Vec<ParsedAtRule> {
    let mut parsed = Vec::new();

    for caps in MIN_MAX_SYNTAX.captures_iter(params) {
        if let Some(parsed_rule) = parse_min_max_syntax(&caps) {
            parsed.push(parsed_rule);
        }
    }

    for caps in REVERSED_RANGE_SYNTAX.captures_iter(params) {
        if let Some(parsed_rule) = parse_reversed_range_syntax(&caps) {
            parsed.push(parsed_rule);
        }
    }

    for caps in RANGE_SYNTAX.captures_iter(params) {
        if let Some(parsed_rule) = parse_range_syntax(&caps) {
            parsed.push(parsed_rule);
        }
    }

    parsed.sort_by_key(|entry| entry.index);
    parsed
}

static MIN_MAX_SYNTAX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?P<property>(?:min|max)-?(?:device-)?(?:width|height))\s*:\s*(?P<length>-?\d*\.?\d+)(?P<lengthUnit>ch|em|ex|px|rem)?"
    )
    .expect("valid min/max syntax regex")
});

static REVERSED_RANGE_SYNTAX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?P<length>-?\d*\.?\d+)(?P<lengthUnit>ch|em|ex|px|rem)?\s*(?P<operator><=?|>=?|=)\s*(?P<property>(?:device-)?(?:width|height))"
    )
    .expect("valid reversed range syntax regex")
});

static RANGE_SYNTAX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?P<property>(?:device-)?(?:width|height))\s*(?P<operator><=?|>=?|=)\s*(?P<length>-?\d*\.?\d+)(?P<lengthUnit>ch|em|ex|px|rem)?"
    )
    .expect("valid range syntax regex")
});

fn parse_min_max_syntax(caps: &regex::Captures<'_>) -> Option<ParsedAtRule> {
    let property = caps.name("property")?.as_str();
    let (property, operator) = convert_min_max_property(property)?;
    let length = parse_length(caps)?;
    let index = caps.get(0)?.start();

    Some(ParsedAtRule {
        property,
        comparison_operator: operator,
        length,
        index,
    })
}

fn convert_min_max_property(value: &str) -> Option<(Property, ComparisonOperator)> {
    match value {
        "min-width" => Some((Property::Width, ComparisonOperator::GreaterEq)),
        "max-width" => Some((Property::Width, ComparisonOperator::LessEq)),
        "min-height" => Some((Property::Height, ComparisonOperator::GreaterEq)),
        "max-height" => Some((Property::Height, ComparisonOperator::LessEq)),
        "min-device-width" => Some((Property::DeviceWidth, ComparisonOperator::GreaterEq)),
        "max-device-width" => Some((Property::DeviceWidth, ComparisonOperator::LessEq)),
        "min-device-height" => Some((Property::DeviceHeight, ComparisonOperator::GreaterEq)),
        "max-device-height" => Some((Property::DeviceHeight, ComparisonOperator::LessEq)),
        _ => None,
    }
}

fn parse_reversed_range_syntax(caps: &regex::Captures<'_>) -> Option<ParsedAtRule> {
    let property = parse_property(caps.name("property")?.as_str())?;
    let operator = parse_operator(caps.name("operator")?.as_str()).map(reverse_operator)?;
    let length = parse_length(caps)?;
    let index = caps.get(0)?.start();

    Some(ParsedAtRule {
        property,
        comparison_operator: operator,
        length,
        index,
    })
}

fn parse_range_syntax(caps: &regex::Captures<'_>) -> Option<ParsedAtRule> {
    let property = parse_property(caps.name("property")?.as_str())?;
    let operator = parse_operator(caps.name("operator")?.as_str())?;
    let length = parse_length(caps)?;
    let index = caps.get(0)?.start();

    Some(ParsedAtRule {
        property,
        comparison_operator: operator,
        length,
        index,
    })
}

fn parse_property(value: &str) -> Option<Property> {
    match value {
        "width" => Some(Property::Width),
        "height" => Some(Property::Height),
        "device-width" => Some(Property::DeviceWidth),
        "device-height" => Some(Property::DeviceHeight),
        _ => None,
    }
}

fn parse_operator(value: &str) -> Option<ComparisonOperator> {
    match value {
        "<" => Some(ComparisonOperator::Less),
        "<=" => Some(ComparisonOperator::LessEq),
        "=" => Some(ComparisonOperator::Equal),
        ">" => Some(ComparisonOperator::Greater),
        ">=" => Some(ComparisonOperator::GreaterEq),
        _ => None,
    }
}

fn reverse_operator(operator: ComparisonOperator) -> ComparisonOperator {
    match operator {
        ComparisonOperator::Less => ComparisonOperator::Greater,
        ComparisonOperator::LessEq => ComparisonOperator::GreaterEq,
        ComparisonOperator::Greater => ComparisonOperator::Less,
        ComparisonOperator::GreaterEq => ComparisonOperator::LessEq,
        ComparisonOperator::Equal => ComparisonOperator::Equal,
    }
}

fn parse_length(caps: &regex::Captures<'_>) -> Option<f64> {
    let length_match = caps.name("length")?.as_str();
    let unit = caps.name("lengthUnit").map(|value| value.as_str());

    if length_match == "0" {
        return Some(0.0);
    }

    let length_value: f64 = length_match.parse().ok()?;
    let unit = unit?;

    match unit {
        "px" => Some(length_value),
        "em" | "rem" => Some(length_value * 16.0),
        "ch" | "ex" => Some(length_value * 0.5 * 16.0),
        _ => None,
    }
}

fn format_node(node: &Node) -> String {
    match node {
        Node::Rule(rule) => format_rule(rule),
        Node::AtRule(at_rule) => format_at_rule(at_rule),
        Node::Other(text) => text.clone(),
    }
}

fn format_rule(rule: &RuleNode) -> String {
    let mut body = String::new();
    for (index, decl) in rule.declarations.iter().enumerate() {
        if index > 0 {
            body.push(';');
        }
        body.push_str(&decl.property);
        body.push(':');
        body.push_str(&decl.value);
    }

    format!("{}{{{}}}", rule.selector, body)
}

fn format_at_rule(at_rule: &AtRuleNode) -> String {
    let mut body = String::new();
    for child in &at_rule.children {
        body.push_str(&format_node(child));
    }

    format!("@{}{}{{{}}}", at_rule.name, at_rule.params, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_duplicate_at_rules() {
        let input = vec![
            "@media (min-width:500px){._a{color:red}}".to_string(),
            "@media (min-width:500px){._b{color:blue}}".to_string(),
        ];

        let sorted = sort_atomic_style_sheet(&input, SortConfig::default());
        assert_eq!(
            sorted,
            "@media (min-width:500px){._a{color:red}._b{color:blue}}"
        );
    }

    #[test]
    fn sorts_min_width_breakpoints() {
        let input = vec![
            "@media (min-width:400px){._a{color:yellow}}".to_string(),
            "@media (min-width:200px){._b{color:red}}".to_string(),
        ];

        let sorted = sort_atomic_style_sheet(&input, SortConfig::default());
        assert_eq!(
            sorted,
            "@media (min-width:200px){._b{color:red}}\n@media (min-width:400px){._a{color:yellow}}"
        );
    }

    #[test]
    fn sorts_max_width_breakpoints() {
        let input = vec![
            "@media (max-width:200px){._a{color:blue}}".to_string(),
            "@media (max-width:400px){._b{color:red}}".to_string(),
        ];

        let sorted = sort_atomic_style_sheet(&input, SortConfig::default());
        assert_eq!(
            sorted,
            "@media (max-width:400px){._b{color:red}}\n@media (max-width:200px){._a{color:blue}}"
        );
    }

    #[test]
    fn parses_max_width_media_queries() {
        let parsed = super::parse_media_query("(max-width:200px)");
        assert_eq!(parsed.len(), 1);
        let rule = &parsed[0];
        assert!(matches!(
            rule.comparison_operator,
            ComparisonOperator::LessEq
        ));
        assert!((rule.length - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sorts_pseudo_selectors() {
        let input = vec![
            "._a:hover{color:red}".to_string(),
            "._b:focus{color:blue}".to_string(),
        ];

        let sorted = sort_atomic_style_sheet(&input, SortConfig::default());
        assert_eq!(sorted, "._b:focus{color:blue}\n._a:hover{color:red}");
    }

    #[test]
    fn sorts_nested_pseudo_selectors_inside_media() {
        let input =
            vec!["@media (min-width:200px){._a:hover{color:red}._b:focus{color:blue}}".to_string()];

        let sorted = sort_atomic_style_sheet(&input, SortConfig::default());
        assert_eq!(
            sorted,
            "@media (min-width:200px){._b:focus{color:blue}._a:hover{color:red}}"
        );
    }
}
