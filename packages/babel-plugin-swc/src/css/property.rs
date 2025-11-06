//! Helpers ported from `packages/css/src/utils/css-property.ts`.
//!
//! Only the subset required by the initial SWC port is included. The module can
//! be expanded incrementally as additional features are translated from the
//! Babel implementation.

use once_cell::sync::Lazy;
use std::collections::HashSet;

use crate::options::PluginConfig;
use crate::postcss::{
    minify_color, normalise_content_value, normalise_current_color, normalise_ordered_value,
    normalise_timing_function, normalise_zero_unit, reduce_initial_value,
};
use crate::utils::kebab_case::kebab_case;

#[derive(Debug, Clone, PartialEq)]
pub struct CssObject(Vec<(String, CssValue)>);

impl CssObject {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn insert(&mut self, key: String, value: CssValue) {
        self.0.push((key, value));
    }

    pub fn iter(&self) -> CssObjectIter<'_> {
        CssObjectIter(self.0.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&CssValue> {
        self.0
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }
}

impl Default for CssObject {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for CssObject {
    type Item = (String, CssValue);
    type IntoIter = std::vec::IntoIter<(String, CssValue)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a CssObject {
    type Item = (&'a String, &'a CssValue);
    type IntoIter = CssObjectIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct CssObjectIter<'a>(std::slice::Iter<'a, (String, CssValue)>);

impl<'a> Iterator for CssObjectIter<'a> {
    type Item = (&'a String, &'a CssValue);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, value)| (key, value))
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CssValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Object(CssObject),
    Array(Vec<CssValue>),
}

impl CssValue {
    pub fn into_css_string(self) -> Option<String> {
        match self {
            CssValue::String(text) => Some(text),
            CssValue::Number(num) => Some(trim_number(num)),
            CssValue::Bool(value) => Some(value.to_string()),
            CssValue::Null => None,
            CssValue::Object(_) | CssValue::Array(_) => None,
        }
    }

    pub fn to_css_string(&self) -> Option<String> {
        self.clone().into_css_string()
    }

    pub fn into_number(self) -> Option<f64> {
        match self {
            CssValue::Number(num) => Some(num),
            _ => None,
        }
    }

    pub fn to_number(&self) -> Option<f64> {
        self.clone().into_number()
    }

    pub fn into_bool(self) -> Option<bool> {
        match self {
            CssValue::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub fn to_bool(&self) -> Option<bool> {
        self.clone().into_bool()
    }

    pub fn as_object(&self) -> Option<&CssObject> {
        match self {
            CssValue::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn into_object(self) -> Option<CssObject> {
        match self {
            CssValue::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[CssValue]> {
        match self {
            CssValue::Array(values) => Some(values),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            CssValue::Null => false,
            CssValue::Bool(value) => *value,
            CssValue::Number(value) => !value.is_nan() && *value != 0.0,
            CssValue::String(text) => !text.is_empty(),
            CssValue::Object(_) | CssValue::Array(_) => true,
        }
    }

    pub fn is_nullish(&self) -> bool {
        matches!(self, CssValue::Null)
    }
}

static UNITLESS_PROPERTIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "animationIterationCount",
        "basePalette",
        "borderImageOutset",
        "borderImageSlice",
        "borderImageWidth",
        "boxFlex",
        "boxFlexGroup",
        "boxOrdinalGroup",
        "columnCount",
        "columns",
        "flex",
        "flexGrow",
        "flexPositive",
        "flexShrink",
        "flexNegative",
        "flexOrder",
        "fontSizeAdjust",
        "fontWeight",
        "gridArea",
        "gridRow",
        "gridRowEnd",
        "gridRowSpan",
        "gridRowStart",
        "gridColumn",
        "gridColumnEnd",
        "gridColumnSpan",
        "gridColumnStart",
        "lineClamp",
        "lineHeight",
        "opacity",
        "order",
        "orphans",
        "tabSize",
        "WebkitLineClamp",
        "widows",
        "zIndex",
        "zoom",
        "fillOpacity",
        "floodOpacity",
        "stopOpacity",
        "strokeDasharray",
        "strokeDashoffset",
        "strokeMiterlimit",
        "strokeOpacity",
        "strokeWidth",
    ])
});

#[allow(dead_code)]
pub fn add_unit_if_needed(name: &str, value: CssValue) -> Option<String> {
    match value {
        CssValue::Null => None,
        CssValue::Bool(_) => None,
        CssValue::Number(num) => {
            if num == 0.0 || is_unitless_property(name) {
                Some(trim_number(num))
            } else {
                Some(format!("{}px", trim_number(num)))
            }
        }
        CssValue::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        CssValue::Object(_) | CssValue::Array(_) => None,
    }
}

fn is_unitless_property(name: &str) -> bool {
    UNITLESS_PROPERTIES.contains(name)
}

pub fn trim_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        let mut repr = value.to_string();
        if repr.contains('.') {
            while repr.ends_with('0') {
                repr.pop();
            }
            if repr.ends_with('.') {
                repr.pop();
            }
        }
        repr
    }
}

const TIMING_FUNCTION_PROPERTIES: &[&str] = &[
    "animation",
    "animation-delay",
    "animation-duration",
    "animation-timing-function",
    "transition",
    "transition-delay",
    "transition-duration",
    "transition-timing-function",
];

fn should_normalise_timing_property(property: &str) -> bool {
    TIMING_FUNCTION_PROPERTIES
        .iter()
        .any(|candidate| candidate == &property)
}

fn extract_important(value: &str) -> (String, bool) {
    let trimmed = value.trim();

    if let Some(index) = trimmed.rfind("!important") {
        let (head, tail) = trimmed.split_at(index);
        if tail.trim() == "!important" {
            return (head.trim_end().to_owned(), true);
        }
    }

    (trimmed.to_owned(), false)
}

pub fn normalise_important(value: &str) -> String {
    let trimmed = value.trim();

    if let Some(index) = trimmed.rfind("!important") {
        let (head, tail) = trimmed.split_at(index);
        if tail.trim() == "!important" {
            let base = head.trim_end();
            let mut result = String::with_capacity(base.len() + "!important".len());
            result.push_str(base);
            result.push_str("!important");
            return result;
        }
    }

    trimmed.to_owned()
}

pub fn array_to_css_string(values: &[CssValue]) -> Option<String> {
    let mut parts = Vec::new();

    for value in values {
        match value {
            CssValue::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                parts.push(trimmed.to_owned());
            }
            CssValue::Number(num) => parts.push(trim_number(*num)),
            CssValue::Bool(value) => parts.push(value.to_string()),
            _ => return None,
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

pub fn normalise_property_pair(
    key: &str,
    value: CssValue,
    config: &PluginConfig,
) -> Option<(String, String)> {
    let property_name = if key.starts_with("--") {
        key.to_owned()
    } else {
        kebab_case(key)
    };

    let normalized_value = match value {
        CssValue::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed.to_owned()
        }
        CssValue::Number(num) => add_unit_if_needed(key, CssValue::Number(num))?,
        CssValue::Bool(_) => return None,
        CssValue::Array(values) => array_to_css_string(&values)?,
        CssValue::Null | CssValue::Object(_) => return None,
    };

    let value = normalized_value;
    let (base_value, has_important) = extract_important(&value);

    let mut value = if property_name == "content" {
        normalise_content_value(&base_value).into_owned()
    } else if should_normalise_timing_property(&property_name) {
        normalise_timing_function(&base_value).into_owned()
    } else {
        base_value
    };

    if config.optimize_css && !property_name.starts_with("--") {
        if let Some(ordered) = normalise_ordered_value(&property_name, &value) {
            value = ordered;
        }

        if let Some(replacement) = reduce_initial_value(&property_name, &value) {
            value = replacement;
        }

        if let Some(zero) = normalise_zero_unit(&value) {
            value = zero;
        }

        let normalised = normalise_current_color(&value);
        if normalised.as_ref() != value {
            value = normalised.into_owned();
        }

        if let Some(minified) = minify_color(&value) {
            value = minified;
        }
    } else if config.optimize_css {
        let normalised = normalise_current_color(&value);
        if normalised.as_ref() != value {
            value = normalised.into_owned();
        }
    }

    if has_important {
        value.push_str("!important");
    }

    let value = normalise_important(&value);
    Some((property_name, value))
}

pub fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalise_at_rule_key(key: &str) -> String {
    collapse_whitespace(key)
        .replace(": ", ":")
        .replace(" )", ")")
}

pub fn serialize_css_object(map: &CssObject, config: &PluginConfig) -> Option<String> {
    #[derive(Debug)]
    enum Part {
        Declaration(String),
        Block(String),
    }

    let mut parts: Vec<Part> = Vec::new();

    for (key, value) in map.iter() {
        match value {
            CssValue::Object(nested) => {
                let selector = collapse_whitespace(key);
                if key.starts_with('@') {
                    if let Some(body) = serialize_css_object(nested, config) {
                        parts.push(Part::Block(format!(
                            "{}{{{body}}}",
                            normalise_at_rule_key(&selector)
                        )));
                    }
                } else if let Some(body) = serialize_css_object(nested, config) {
                    parts.push(Part::Block(format!("{selector}{{{body}}}")));
                }
            }
            CssValue::Array(items) => {
                if let Some(joined) = array_to_css_string(items) {
                    if let Some((name, value)) =
                        normalise_property_pair(key, CssValue::String(joined), config)
                    {
                        parts.push(Part::Declaration(format!("{name}:{value}")));
                    }
                } else {
                    for item in items {
                        if let CssValue::Object(obj) = item {
                            let selector = collapse_whitespace(key);
                            if key.starts_with('@') {
                                if let Some(body) = serialize_css_object(obj, config) {
                                    parts.push(Part::Block(format!(
                                        "{}{{{body}}}",
                                        normalise_at_rule_key(&selector)
                                    )));
                                }
                            } else if let Some(body) = serialize_css_object(obj, config) {
                                parts.push(Part::Block(format!("{selector}{{{body}}}")));
                            }
                        } else if let Some((name, value)) =
                            normalise_property_pair(key, item.clone(), config)
                        {
                            parts.push(Part::Declaration(format!("{name}:{value}")));
                        }
                    }
                }
            }
            CssValue::Bool(flag) => {
                if let Some((name, value)) =
                    normalise_property_pair(key, CssValue::String(flag.to_string()), config)
                {
                    parts.push(Part::Declaration(format!("{name}:{value}")));
                }
            }
            _ => {
                if let Some((name, value)) = normalise_property_pair(key, value.clone(), config) {
                    parts.push(Part::Declaration(format!("{name}:{value}")));
                }
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        let mut result = String::new();
        let mut prev_was_declaration = false;

        for part in parts {
            match part {
                Part::Declaration(text) => {
                    if !result.is_empty() && prev_was_declaration {
                        result.push(';');
                    }
                    result.push_str(&text);
                    prev_was_declaration = true;
                }
                Part::Block(text) => {
                    if !result.is_empty() && prev_was_declaration {
                        result.push(';');
                    }
                    result.push_str(&text);
                    prev_was_declaration = false;
                }
            }
        }

        Some(result)
    }
}

impl CssValue {
    pub fn into_template_segment(self, config: &PluginConfig) -> Option<String> {
        match self {
            CssValue::Object(map) => serialize_css_object(&map, config),
            CssValue::Array(values) => array_to_css_string(&values),
            other => other.into_css_string(),
        }
    }

    pub fn to_template_segment(&self, config: &PluginConfig) -> Option<String> {
        self.clone().into_template_segment(config)
    }
}

#[cfg(test)]
mod tests {
    use super::{add_unit_if_needed, normalise_property_pair, CssValue};
    use crate::options::PluginConfig;

    #[test]
    fn appends_px_for_lengths() {
        assert_eq!(
            add_unit_if_needed("fontSize", CssValue::Number(12.0)),
            Some("12px".into())
        );
    }

    #[test]
    fn keeps_unitless_properties() {
        assert_eq!(
            add_unit_if_needed("opacity", CssValue::Number(0.5)),
            Some("0.5".into())
        );
    }

    #[test]
    fn skips_empty_values() {
        assert_eq!(add_unit_if_needed("color", CssValue::Null), None);
        assert_eq!(add_unit_if_needed("color", CssValue::Bool(false)), None);
        assert_eq!(
            add_unit_if_needed("color", CssValue::String(String::new())),
            None
        );
    }

    #[test]
    fn timing_normalisation_runs_without_optimisation() {
        let mut config = PluginConfig::default();
        config.optimize_css = false;

        let value = CssValue::String("200ms".into());

        assert_eq!(
            normalise_property_pair("animationDuration", value.clone(), &config),
            Some(("animation-duration".into(), "0.200s".into()))
        );

        config.optimize_css = true;
        assert_eq!(
            normalise_property_pair("animationDuration", value, &config),
            Some(("animation-duration".into(), "0.200s".into()))
        );
    }

    #[test]
    fn current_color_normalisation_respects_optimize_flag() {
        let mut config = PluginConfig::default();
        let value = CssValue::String("currentcolor".into());

        config.optimize_css = true;
        assert_eq!(
            normalise_property_pair("color", value.clone(), &config),
            Some(("color".into(), "currentColor".into()))
        );

        config.optimize_css = false;
        assert_eq!(
            normalise_property_pair("color", value, &config),
            Some(("color".into(), "currentcolor".into()))
        );
    }

    #[test]
    fn reduces_initial_values_when_optimised() {
        let mut config = PluginConfig::default();
        config.optimize_css = true;

        let value = CssValue::String("initial".into());
        assert_eq!(
            normalise_property_pair("marginLeft", value, &config),
            Some(("margin-left".into(), "0".into()))
        );
    }

    #[test]
    fn converts_zero_units() {
        let mut config = PluginConfig::default();
        config.optimize_css = true;

        let value = CssValue::String("0px".into());
        assert_eq!(
            normalise_property_pair("padding", value, &config),
            Some(("padding".into(), "0".into()))
        );
    }

    #[test]
    fn minifies_color_values() {
        let mut config = PluginConfig::default();
        config.optimize_css = true;

        let value = CssValue::String("rebeccapurple".into());
        assert_eq!(
            normalise_property_pair("color", value, &config),
            Some(("color".into(), "#639".into()))
        );
    }
}
