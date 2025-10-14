use crate::hash::hash;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const INCREASE_SPECIFICITY_SELECTOR: &str = ":not(#\\#)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssOptions {
    #[serde(default)]
    pub class_hash_prefix: Option<String>,
    #[serde(default)]
    pub class_name_compression_map: BTreeMap<String, String>,
    #[serde(default)]
    pub increase_specificity: bool,
    #[serde(default)]
    pub sort_at_rules: bool,
    #[serde(default)]
    pub sort_shorthand: bool,
    #[serde(default)]
    pub flatten_multiple_selectors: bool,
}

impl Default for CssOptions {
    fn default() -> Self {
        Self {
            class_hash_prefix: None,
            class_name_compression_map: BTreeMap::new(),
            increase_specificity: false,
            sort_at_rules: true,
            sort_shorthand: true,
            flatten_multiple_selectors: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicRule {
    pub class_name: String,
    pub css: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CssArtifacts {
    pub rules: Vec<AtomicRule>,
    pub raw_rules: Vec<String>,
}

impl CssArtifacts {
    pub fn push(&mut self, rule: AtomicRule) {
        self.rules.push(rule);
    }

    pub fn push_raw(&mut self, css: String) {
        self.raw_rules.push(css);
    }

    pub fn merge(&mut self, other: CssArtifacts) {
        self.rules.extend(other.rules);
        self.raw_rules.extend(other.raw_rules);
    }

    pub fn class_names(&self) -> impl Iterator<Item = &str> {
        self.rules.iter().map(|rule| rule.class_name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AtRuleInput {
    pub name: String,
    pub params: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CssRuleInput {
    pub selectors: Vec<String>,
    pub at_rules: Vec<AtRuleInput>,
    pub property: String,
    pub value: String,
    pub important: bool,
}

pub fn normalize_selector(selector: Option<&str>) -> String {
    match selector {
        None => "&".to_string(),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.contains('&') {
                trimmed.to_string()
            } else if trimmed.is_empty() {
                "&".to_string()
            } else {
                format!("& {}", trimmed)
            }
        }
    }
}

pub fn add_unit_if_needed(name: &str, value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }

    if let Ok(number) = value.parse::<f64>() {
        if number == 0.0 || is_unitless_property(name) {
            return trim_numeric(value);
        }

        return format!("{}px", trim_numeric(value));
    }

    trim_numeric(value)
}

fn trim_numeric(value: &str) -> String {
    value.trim().to_string()
}

fn is_unitless_property(name: &str) -> bool {
    matches!(
        name,
        "animationIterationCount"
            | "basePalette"
            | "borderImageOutset"
            | "borderImageSlice"
            | "borderImageWidth"
            | "boxFlex"
            | "boxFlexGroup"
            | "boxOrdinalGroup"
            | "columnCount"
            | "columns"
            | "flex"
            | "flexGrow"
            | "flexPositive"
            | "flexShrink"
            | "flexNegative"
            | "flexOrder"
            | "fontSizeAdjust"
            | "fontWeight"
            | "gridArea"
            | "gridRow"
            | "gridRowEnd"
            | "gridRowSpan"
            | "gridRowStart"
            | "gridColumn"
            | "gridColumnEnd"
            | "gridColumnSpan"
            | "gridColumnStart"
            | "lineClamp"
            | "lineHeight"
            | "opacity"
            | "order"
            | "orphans"
            | "tabSize"
            | "WebkitLineClamp"
            | "widows"
            | "zIndex"
            | "zoom"
            | "fillOpacity"
            | "floodOpacity"
            | "stopOpacity"
            | "strokeDasharray"
            | "strokeDashoffset"
            | "strokeMiterlimit"
            | "strokeOpacity"
            | "strokeWidth"
    )
}

fn replace_nesting(selector: &str, class_name: &str) -> String {
    selector.replace('&', &format!(".{}", class_name))
}

fn join_selectors(selectors: &[String], class_name: &str) -> String {
    selectors
        .iter()
        .map(|selector| replace_nesting(selector, class_name))
        .collect::<Vec<_>>()
        .join(",")
}

fn apply_increase_specificity(selector: &str) -> String {
    if !selector.contains("._") {
        return selector.to_string();
    }

    let mut result = String::with_capacity(selector.len() + 16);
    let chars: Vec<(usize, char)> = selector.char_indices().collect();
    let mut index = 0usize;

    while index < chars.len() {
        let (byte_index, ch) = chars[index];
        result.push(ch);

        if ch == '.' {
            let mut class_chars = String::new();
            let mut next = index + 1;
            while next < chars.len() {
                let (_, next_ch) = chars[next];
                if next_ch.is_ascii_alphanumeric() || next_ch == '_' || next_ch == '-' {
                    result.push(next_ch);
                    class_chars.push(next_ch);
                    next += 1;
                } else {
                    break;
                }
            }

            if !class_chars.is_empty() && class_chars.starts_with('_') {
                let class_end_byte = if next < chars.len() {
                    chars[next].0
                } else {
                    selector.len()
                };
                if !selector[class_end_byte..].starts_with(INCREASE_SPECIFICITY_SELECTOR) {
                    result.push_str(INCREASE_SPECIFICITY_SELECTOR);
                }
            }

            index = next;
            continue;
        }

        if ch.is_ascii() {
            // advance using prepared index when we didn't consume additional chars.
            index += 1;
        } else {
            // fallback for multi-byte characters: find next index by matching byte offset.
            let mut next_index = index + 1;
            while next_index < chars.len() && chars[next_index].0 == byte_index {
                next_index += 1;
            }
            index = next_index;
        }
    }

    result
}

fn wrap_at_rules(mut css: String, at_rules: &[AtRuleInput]) -> String {
    for at_rule in at_rules.iter().rev() {
        let name = at_rule.name.trim();
        let params = at_rule.params.trim();
        if params.is_empty() {
            css = format!("@{}{{{}}}", name, css);
        } else {
            css = format!("@{} {}{{{}}}", name, params, css);
        }
    }
    css
}

pub fn atomicize_rules(rules: &[CssRuleInput], options: &CssOptions) -> CssArtifacts {
    let mut artifacts = CssArtifacts::default();

    for rule in rules {
        let normalized_selectors = if rule.selectors.is_empty() {
            vec![normalize_selector(None)]
        } else {
            rule.selectors
                .iter()
                .map(|selector| normalize_selector(Some(selector)))
                .collect::<Vec<_>>()
        };

        let declaration = if rule.important {
            format!("{}:{}!important", rule.property, rule.value)
        } else {
            format!("{}:{}", rule.property, rule.value)
        };

        let mut per_selector_outputs = Vec::new();
        for selector in &normalized_selectors {
            let selectors_hash = selector.to_string();
            let at_rule_label: String = rule
                .at_rules
                .iter()
                .map(|input| format!("{}{}", input.name.trim(), input.params.trim()))
                .collect();
            let prefix = options.class_hash_prefix.as_deref().unwrap_or("");
            let group_hash = hash(
                &format!(
                    "{}{}{}{}",
                    prefix, at_rule_label, selectors_hash, rule.property
                ),
                0,
            );
            let group = &group_hash[..group_hash.len().min(4)];

            let value_with_flag = if rule.important {
                format!("{}:{}!important", rule.property, rule.value)
            } else {
                format!("{}:{}", rule.property, rule.value)
            };
            let value_hash = hash(&value_with_flag, 0);
            let value_segment = &value_hash[..value_hash.len().min(4)];
            let full_class = format!("_{}{}", group, value_segment);
            let (class_name, selector_target) =
                match options.class_name_compression_map.get(&full_class[1..]) {
                    Some(compressed) => (
                        format!("_{}_{}", &full_class[1..5], compressed),
                        compressed.clone(),
                    ),
                    None => (full_class.clone(), full_class.clone()),
                };
            let mut selector_output = join_selectors(&[selector.clone()], &selector_target);
            if options.increase_specificity {
                selector_output = apply_increase_specificity(&selector_output);
            }
            let css = wrap_at_rules(
                format!("{}{{{}}}", selector_output.clone(), declaration.clone()),
                &rule.at_rules,
            );
            per_selector_outputs.push((class_name, selector_output, css));
        }

        if !options.flatten_multiple_selectors && per_selector_outputs.len() > 1 {
            let combined_selector = per_selector_outputs
                .iter()
                .map(|(_, selector, _)| selector.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let combined_css = wrap_at_rules(
                format!("{}{{{}}}", combined_selector, declaration),
                &rule.at_rules,
            );
            for (class_name, _, _) in per_selector_outputs {
                artifacts.push(AtomicRule {
                    class_name,
                    css: combined_css.clone(),
                });
            }
        } else {
            for (class_name, _, css) in per_selector_outputs {
                artifacts.push(AtomicRule { class_name, css });
            }
        }
    }

    artifacts
}

/// A very small subset of the Babel CSS pipeline that focuses on atomicising flat
/// declarations. This is intentionally conservative but produces the same class
/// name hashing scheme as the JavaScript implementation so the outputs can be
/// compared in integration tests.
pub fn atomicize_literal(css: &str, options: &CssOptions) -> CssArtifacts {
    let mut artifacts = CssArtifacts::default();
    for declaration in css.split(';') {
        let trimmed = declaration.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((prop, value)) = trimmed.split_once(':') {
            let property = prop.trim();
            let mut value = value.trim();
            let important_suffix;
            if let Some(stripped) = value.strip_suffix("!important") {
                important_suffix = Some("!important");
                value = stripped.trim_end();
            } else {
                important_suffix = None;
            }
            let prefix = options
                .class_hash_prefix
                .as_ref()
                .map(String::as_str)
                .unwrap_or("");
            let group_hash = hash(&format!("{}{}", prefix, property), 0);
            let group = &group_hash[..group_hash.len().min(4)];
            let value_with_flag = match important_suffix {
                Some(flag) => format!("{}{}", value, flag),
                None => value.to_string(),
            };
            let value_hash = hash(&value_with_flag, 0);
            let value_segment = &value_hash[..value_hash.len().min(4)];
            let full_class = format!("_{}{}", group, value_segment);
            let (class_name, selector_target) =
                match options.class_name_compression_map.get(&full_class[1..]) {
                    Some(compressed) => (
                        format!("_{}_{}", &full_class[1..5], compressed),
                        compressed.clone(),
                    ),
                    None => (full_class.clone(), full_class.clone()),
                };
            let css_value = if let Some(flag) = important_suffix {
                format!("{}:{}{}", property, value, flag)
            } else {
                format!("{}:{}", property, value)
            };
            let mut selector = format!(".{}", selector_target);
            if options.increase_specificity {
                selector = apply_increase_specificity(&selector);
            }
            let css_rule = format!("{}{{{}}}", selector, css_value);
            artifacts.push(AtomicRule {
                class_name,
                css: css_rule,
            });
        }
    }
    artifacts
}

#[cfg(test)]
mod tests {
    use super::{atomicize_literal, CssOptions};

    #[test]
    fn generates_atomic_rules() {
        let artifacts = atomicize_literal("color: red; background: blue;", &CssOptions::default());
        assert_eq!(artifacts.rules.len(), 2);
        assert!(artifacts.class_names().any(|name| name.starts_with('_')));
    }

    #[test]
    fn compresses_class_names_when_map_present() {
        let mut options = CssOptions::default();
        options
            .class_name_compression_map
            .insert("syaz13q2".into(), "a".into());
        let artifacts = atomicize_literal("color: blue;", &options);
        assert_eq!(artifacts.rules.len(), 1);
        let rule = &artifacts.rules[0];
        assert_eq!(rule.class_name, "_syaz_a");
        assert!(rule.css.contains(".a{"));
    }
}
