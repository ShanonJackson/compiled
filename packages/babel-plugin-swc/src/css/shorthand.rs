//! Helpers that mirror the subset of `expand-shorthands` PostCSS plugin used by
//! the Babel implementation.
//!
//! The goal is to expand shorthand declarations like `padding` and `outline`
//! into the corresponding longhand properties before hashing so the generated
//! class names, ordering, and style rules remain identical to the original
//! pipeline.

use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::postcss::is_color as is_color_value;

const GLOBAL_VALUES: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];
const OUTLINE_STYLE_VALUES: &[&str] = &[
    "auto", "none", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset",
];
const OUTLINE_WIDTH_KEYWORDS: &[&str] = &["thin", "medium", "thick"];

/// Expands supported shorthand properties into their longhand equivalents.
///
/// Returning `None` indicates the property should be handled as-is, while an
/// empty expansion (`Some(Vec::new())`) mirrors the behaviour of the Babel
/// plugin where invalid shorthand usage removes the declaration entirely.
pub fn expand_shorthand_property(property: &str, value: &str) -> Option<Vec<(String, String)>> {
    let (base_value, has_important) = split_important(value);

    if base_value.is_empty() || contains_css_variable(&base_value) {
        return None;
    }

    match property {
        "padding" => expand_padding(&base_value, has_important),
        "outline" => expand_outline(&base_value, has_important),
        "overflow" => expand_overflow(&base_value, has_important),
        _ => None,
    }
}

fn expand_padding(value: &str, important: bool) -> Option<Vec<(String, String)>> {
    let tokens = split_css_tokens(value);
    if tokens.is_empty() {
        return Some(Vec::new());
    }

    let top = tokens.get(0).cloned().unwrap_or_default();
    let right = tokens.get(1).cloned().unwrap_or_else(|| top.clone());
    let bottom = tokens.get(2).cloned().unwrap_or_else(|| top.clone());
    let left = tokens.get(3).cloned().unwrap_or_else(|| right.clone());

    Some(vec![
        ("padding-top".into(), apply_important(top, important)),
        ("padding-right".into(), apply_important(right, important)),
        ("padding-bottom".into(), apply_important(bottom, important)),
        ("padding-left".into(), apply_important(left, important)),
    ])
}

fn expand_outline(value: &str, important: bool) -> Option<Vec<(String, String)>> {
    let tokens = split_css_tokens(value);

    let mut color: Option<String> = None;
    let mut style: Option<String> = None;
    let mut width: Option<String> = None;

    for token in tokens {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if color.is_none() && is_outline_color(trimmed) {
            color = Some(trimmed.to_string());
            continue;
        }

        if width.is_none() && is_outline_width_keyword(&lower) {
            width = Some(trimmed.to_string());
            continue;
        }

        if width.is_none() && is_numeric_width(trimmed) {
            width = Some(trimmed.to_string());
            continue;
        }

        if style.is_none() && is_outline_style_keyword(&lower) {
            style = Some(trimmed.to_string());
            continue;
        }

        // Unrecognised token – mirror Babel by discarding the declaration.
        return Some(Vec::new());
    }

    let color_value = color.unwrap_or_else(|| "currentColor".to_string());
    let style_value = style.unwrap_or_else(|| "none".to_string());
    let width_value = width.unwrap_or_else(|| "medium".to_string());

    Some(vec![
        (
            "outline-color".into(),
            apply_important(color_value, important),
        ),
        (
            "outline-style".into(),
            apply_important(style_value, important),
        ),
        (
            "outline-width".into(),
            apply_important(width_value, important),
        ),
    ])
}

fn expand_overflow(value: &str, important: bool) -> Option<Vec<(String, String)>> {
    let tokens = split_css_tokens(value);

    if tokens.is_empty() {
        return Some(Vec::new());
    }

    if tokens.len() > 2 {
        return Some(Vec::new());
    }

    let x = tokens.get(0).cloned().unwrap_or_default();
    let y = tokens.get(1).cloned().unwrap_or_else(|| x.clone());

    Some(vec![
        ("overflow-x".into(), apply_important(x, important)),
        ("overflow-y".into(), apply_important(y, important)),
    ])
}

static SHORTHAND_BUCKETS: Lazy<HashMap<&'static str, usize>> = Lazy::new(|| {
    HashMap::from([
        ("all", 0),
        ("animation", 1),
        ("animation-range", 1),
        ("background", 1),
        ("border", 1),
        ("border-color", 2),
        ("border-style", 2),
        ("border-width", 2),
        ("border-block", 3),
        ("border-inline", 3),
        ("border-top", 4),
        ("border-right", 4),
        ("border-bottom", 4),
        ("border-left", 4),
        ("border-block-start", 5),
        ("border-block-end", 5),
        ("border-inline-start", 5),
        ("border-inline-end", 5),
        ("border-image", 1),
        ("border-radius", 1),
        ("column-rule", 1),
        ("columns", 1),
        ("contain-intrinsic-size", 1),
        ("container", 1),
        ("flex", 1),
        ("flex-flow", 1),
        ("font", 1),
        ("font-synthesis", 1),
        ("font-variant", 2),
        ("gap", 1),
        ("grid", 1),
        ("grid-area", 1),
        ("grid-column", 2),
        ("grid-row", 2),
        ("grid-template", 2),
        ("inset", 1),
        ("inset-block", 2),
        ("inset-inline", 2),
        ("list-style", 1),
        ("margin", 1),
        ("margin-block", 2),
        ("margin-inline", 2),
        ("mask", 1),
        ("mask-border", 1),
        ("offset", 1),
        ("outline", 1),
        ("overflow", 1),
        ("overscroll-behavior", 1),
        ("padding", 1),
        ("padding-block", 2),
        ("padding-inline", 2),
        ("place-content", 1),
        ("place-items", 1),
        ("place-self", 1),
        ("position-try", 1),
        ("scroll-margin", 1),
        ("scroll-margin-block", 2),
        ("scroll-margin-inline", 2),
        ("scroll-padding", 1),
        ("scroll-padding-block", 2),
        ("scroll-padding-inline", 2),
        ("scroll-timeline", 1),
        ("text-decoration", 1),
        ("text-emphasis", 1),
        ("text-wrap", 1),
        ("transition", 1),
        ("view-timeline", 1),
    ])
});

pub fn shorthand_bucket(property: &str) -> Option<usize> {
    SHORTHAND_BUCKETS.get(property).copied()
}

fn contains_css_variable(value: &str) -> bool {
    value.to_ascii_lowercase().contains("var(")
}

fn split_important(value: &str) -> (String, bool) {
    if let Some(stripped) = value.strip_suffix("!important") {
        return (stripped.trim_end().to_string(), true);
    }

    (value.to_string(), false)
}

fn split_css_tokens(value: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for ch in value.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                if depth > 0 {
                    depth -= 1;
                }
                current.push(ch);
            }
            ch if ch.is_whitespace() && depth == 0 => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    tokens
}

fn apply_important(value: String, important: bool) -> String {
    if important {
        format!("{value}!important")
    } else {
        value
    }
}

fn is_outline_color(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }

    let lower = token.to_ascii_lowercase();

    if GLOBAL_VALUES.contains(&lower.as_str()) {
        return true;
    }

    if lower == "transparent" || lower == "currentcolor" {
        return true;
    }

    is_color_value(token)
}

fn is_outline_style_keyword(value: &str) -> bool {
    GLOBAL_VALUES.contains(&value) || OUTLINE_STYLE_VALUES.contains(&value)
}

fn is_outline_width_keyword(value: &str) -> bool {
    GLOBAL_VALUES.contains(&value) || OUTLINE_WIDTH_KEYWORDS.contains(&value)
}

fn is_numeric_width(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.contains('(') && trimmed.ends_with(')') {
        return true;
    }

    let mut end = trimmed.len();
    let bytes = trimmed.as_bytes();
    while end > 0 {
        let ch = bytes[end - 1] as char;
        if ch.is_ascii_alphabetic() || ch == '%' {
            end -= 1;
        } else {
            break;
        }
    }

    let number_part = &trimmed[..end];
    if number_part.is_empty() {
        return false;
    }

    number_part.parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_padding_into_longhands() {
        let expanded = expand_shorthand_property("padding", "8px").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("padding-top".into(), "8px".into()),
                ("padding-right".into(), "8px".into()),
                ("padding-bottom".into(), "8px".into()),
                ("padding-left".into(), "8px".into()),
            ]
        );
    }

    #[test]
    fn expands_outline_defaults() {
        let expanded = expand_shorthand_property("outline", "none").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("outline-color".into(), "currentColor".into()),
                ("outline-style".into(), "none".into()),
                ("outline-width".into(), "medium".into()),
            ]
        );
    }

    #[test]
    fn skips_expansion_when_variables_present() {
        assert!(expand_shorthand_property("padding", "var(--x)").is_none());
    }

    #[test]
    fn expands_padding_with_mixed_units() {
        let expanded = expand_shorthand_property("padding", "1px 2em 3% 4rem").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("padding-top".into(), "1px".into()),
                ("padding-right".into(), "2em".into()),
                ("padding-bottom".into(), "3%".into()),
                ("padding-left".into(), "4rem".into()),
            ]
        );
    }

    #[test]
    fn expands_padding_with_calc_values() {
        let expanded = expand_shorthand_property("padding", "calc(10% - 5px) 2em").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("padding-top".into(), "calc(10% - 5px)".into()),
                ("padding-right".into(), "2em".into()),
                ("padding-bottom".into(), "calc(10% - 5px)".into()),
                ("padding-left".into(), "2em".into()),
            ]
        );
    }

    #[test]
    fn expands_padding_retains_important_flag() {
        let expanded = expand_shorthand_property("padding", "1px 2px!important").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("padding-top".into(), "1px!important".into()),
                ("padding-right".into(), "2px!important".into()),
                ("padding-bottom".into(), "1px!important".into()),
                ("padding-left".into(), "2px!important".into()),
            ]
        );
    }

    #[test]
    fn expands_outline_with_full_values() {
        let expanded =
            expand_shorthand_property("outline", "1px solid rgba(0, 0, 0, 0.5) !important")
                .unwrap();
        assert_eq!(
            expanded,
            vec![
                (
                    "outline-color".into(),
                    "rgba(0, 0, 0, 0.5)!important".into(),
                ),
                ("outline-style".into(), "solid!important".into()),
                ("outline-width".into(), "1px!important".into()),
            ]
        );
    }

    #[test]
    fn outline_with_unknown_token_is_dropped() {
        let expanded = expand_shorthand_property("outline", "1px groove banana").unwrap();
        assert!(expanded.is_empty());
    }

    #[test]
    fn expands_overflow_single_value() {
        let expanded = expand_shorthand_property("overflow", "hidden").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("overflow-x".into(), "hidden".into()),
                ("overflow-y".into(), "hidden".into()),
            ]
        );
    }

    #[test]
    fn expands_overflow_dual_values() {
        let expanded = expand_shorthand_property("overflow", "hidden visible").unwrap();
        assert_eq!(
            expanded,
            vec![
                ("overflow-x".into(), "hidden".into()),
                ("overflow-y".into(), "visible".into()),
            ]
        );
    }

    #[test]
    fn overflow_with_too_many_tokens_is_dropped() {
        let expanded = expand_shorthand_property("overflow", "hidden visible scroll").unwrap();
        assert!(expanded.is_empty());
    }
}
