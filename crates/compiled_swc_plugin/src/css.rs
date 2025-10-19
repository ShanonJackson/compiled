use crate::hash::hash;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const INCREASE_SPECIFICITY_SELECTOR: &str = ":not(#\\#)";

#[derive(Debug, Clone)]
pub struct NormalizedCssValue {
  pub hash_value: String,
  pub output_value: String,
}

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
  pub raw_value: String,
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

fn lowercase_hex_literals(input: &str) -> String {
  let mut bytes = input.as_bytes().to_vec();
  let mut index = 0usize;
  while index < bytes.len() {
    if bytes[index] == b'#' {
      let mut inner = index + 1;
      while inner < bytes.len() {
        let ch = bytes[inner];
        if (ch as char).is_ascii_hexdigit() {
          bytes[inner] = ch.to_ascii_lowercase();
          inner += 1;
        } else {
          break;
        }
      }
      index = inner;
    } else {
      index += 1;
    }
  }

  String::from_utf8(bytes).expect("css value should be valid utf8")
}

fn shorten_hex_literals(input: &str) -> String {
  let mut output = String::with_capacity(input.len());
  let chars: Vec<char> = input.chars().collect();
  let mut index = 0usize;

  while index < chars.len() {
    let ch = chars[index];
    if ch == '#' && index + 6 < chars.len() {
      let slice = &chars[index + 1..index + 7];
      if slice.iter().all(|c| c.is_ascii_hexdigit()) {
        if slice[0] == slice[1] && slice[2] == slice[3] && slice[4] == slice[5] {
          output.push('#');
          output.push(slice[0]);
          output.push(slice[2]);
          output.push(slice[4]);
          index += 7;
          continue;
        }
      }
    }

    output.push(ch);
    index += 1;
  }

  output
}

fn minify_whitespace(value: &str) -> String {
  let mut output = String::with_capacity(value.len());
  let mut chars = value.chars().peekable();

  while let Some(ch) = chars.next() {
    if ch == ' ' {
      if matches!(chars.peek(), Some(next) if *next == ',') {
        continue;
      }
    }

    output.push(ch);

    if ch == ',' {
      while matches!(chars.peek(), Some(next) if next.is_ascii_whitespace()) {
        chars.next();
      }
    }
  }

  output
}

pub fn normalize_css_value(value: &str) -> NormalizedCssValue {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return NormalizedCssValue {
      hash_value: String::new(),
      output_value: String::new(),
    };
  }

  let mut semantic = lowercase_hex_literals(trimmed);
  semantic = shorten_hex_literals(&semantic);
  semantic = convert_length_units(&semantic);
  semantic = strip_zero_units(&semantic);
  let output = minify_whitespace(&semantic);

  NormalizedCssValue {
    hash_value: semantic,
    output_value: output,
  }
}

fn convert_length_units(value: &str) -> String {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return String::new();
  }

  let mut core = trimmed;
  let mut important_suffix = "";

  if let Some(stripped) = core.strip_suffix("!important") {
    core = stripped.trim_end();
    important_suffix = "!important";
  }

  let bytes = core.as_bytes();
  let mut search_start = 0usize;
  let mut last_written = 0usize;
  let mut output = String::with_capacity(core.len());

  while let Some(rel_pos) = core[search_start..].find("px") {
    let px_index = search_start + rel_pos;
    let mut cursor = px_index;
    let mut has_digit = false;
    let mut seen_decimal = false;

    while cursor > 0 {
      let ch = bytes[cursor - 1] as char;
      if ch.is_ascii_digit() {
        has_digit = true;
        cursor -= 1;
        continue;
      }
      if ch == '.' && !seen_decimal {
        seen_decimal = true;
        cursor -= 1;
        continue;
      }
      if (ch == '+' || ch == '-') && cursor - 1 < px_index {
        if cursor > 1 {
          let prev = bytes[cursor - 2] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            break;
          }
        }
        cursor -= 1;
        continue;
      }
      break;
    }

    if !has_digit {
      search_start = px_index + 2;
      continue;
    }

    let number_start = cursor;
    if number_start > 0 {
      let prev = bytes[number_start - 1] as char;
      let mut should_skip = prev.is_ascii_alphanumeric() || prev == '_';
      if !should_skip && prev == '-' && number_start >= 2 {
        let prev2 = bytes[number_start - 2] as char;
        if prev2.is_ascii_alphanumeric() || prev2 == '_' || prev2 == '-' {
          should_skip = true;
        }
      }
      if should_skip {
        search_start = px_index + 2;
        continue;
      }
    }

    let number_str = &core[number_start..px_index];
    if number_str.contains('.') || number_str.contains('e') || number_str.contains('E') {
      search_start = px_index + 2;
      continue;
    }

    if let Ok(px_value) = number_str.parse::<i64>() {
      let mut best: Option<String> = None;

      if px_value != 0 {
        if px_value % 16 == 0 {
          let converted_abs = px_value.abs() / 16;
          let mut candidate = String::new();
          if px_value < 0 {
            candidate.push('-');
          }
          candidate.push_str(&converted_abs.to_string());
          candidate.push_str("pc");
          best = Some(candidate);
        }

        if px_value % 4 == 0 {
          let pt_value = px_value * 3 / 4;
          let mut candidate = String::new();
          if pt_value < 0 {
            candidate.push('-');
          }
          candidate.push_str(&pt_value.abs().to_string());
          candidate.push_str("pt");
          best = match best {
            Some(existing) => {
              if candidate.len() < existing.len() {
                Some(candidate)
              } else {
                Some(existing)
              }
            }
            None => Some(candidate),
          };
        }
      }

      if let Some(candidate) = best {
        let original_len = px_index + 2 - number_start;
        if candidate.len() < original_len {
          output.push_str(&core[last_written..number_start]);
          output.push_str(&candidate);
          last_written = px_index + 2;
        }
      }
    }

    search_start = px_index + 2;
  }

  if last_written == 0 {
    if important_suffix.is_empty() {
      core.to_string()
    } else {
      let mut result = core.to_string();
      result.push_str(important_suffix);
      result
    }
  } else {
    output.push_str(&core[last_written..]);
    if important_suffix.is_empty() {
      output
    } else {
      output.push_str(important_suffix);
      output
    }
  }
}

fn strip_zero_units(value: &str) -> String {
  const UNITS: [&str; 7] = ["px", "em", "rem", "vw", "vh", "vmin", "vmax"];

  let bytes = value.as_bytes();
  let mut index = 0usize;
  let mut output = String::with_capacity(value.len());

  while index < bytes.len() {
    let current = bytes[index];
    if current == b'0' {
      let prev_byte = if index > 0 {
        Some(bytes[index - 1])
      } else {
        None
      };
      let prev_is_digit_or_dot = prev_byte
        .map(|byte| {
          let ch = byte as char;
          ch.is_ascii_digit() || ch == '.'
        })
        .unwrap_or(false);

      if !prev_is_digit_or_dot {
        let mut replaced = false;
        for unit in UNITS {
          let unit_bytes = unit.as_bytes();
          if index + 1 + unit_bytes.len() <= bytes.len()
            && &bytes[index + 1..index + 1 + unit_bytes.len()] == unit_bytes
          {
            if prev_byte == Some(b'-') && output.ends_with('-') {
              output.pop();
            }
            output.push('0');
            index += 1 + unit_bytes.len();
            replaced = true;
            break;
          }
        }

        if replaced {
          continue;
        }
      }
    }

    output.push(current as char);
    index += 1;
  }

  output
}

fn vendor_prefixed_values(property: &str, value: &str) -> Option<Vec<String>> {
  let normalized_value = value.trim();
  if normalized_value.is_empty() {
    return None;
  }

  let property_lower = property.to_ascii_lowercase();
  let applies_to_fit_content = matches!(
    property_lower.as_str(),
    "width" | "height" | "min-width" | "max-width" | "min-height" | "max-height"
  );

  if applies_to_fit_content && normalized_value == "fit-content" {
    if normalized_value.contains("-moz-fit-content") {
      return None;
    }
    return Some(vec!["-moz-fit-content".into(), "fit-content".into()]);
  }

  None
}

struct PropertyExpansion {
  name: String,
  raw_value: String,
}

fn expand_property(property: &str, raw_value: &str) -> Vec<PropertyExpansion> {
  if property == "flex" {
    let trimmed = raw_value.trim();
    if trimmed == "1" {
      return vec![
        PropertyExpansion {
          name: "flex-grow".into(),
          raw_value: "1".into(),
        },
        PropertyExpansion {
          name: "flex-shrink".into(),
          raw_value: "1".into(),
        },
        PropertyExpansion {
          name: "flex-basis".into(),
          raw_value: "0%".into(),
        },
      ];
    }
  }

  if property == "text-decoration" {
    let trimmed = raw_value.trim();
    if trimmed.eq_ignore_ascii_case("none") {
      return vec![
        PropertyExpansion {
          name: "text-decoration-line".into(),
          raw_value: "none".into(),
        },
        PropertyExpansion {
          name: "text-decoration-color".into(),
          raw_value: "initial".into(),
        },
        PropertyExpansion {
          name: "text-decoration-style".into(),
          raw_value: "solid".into(),
        },
      ];
    }
  }

  if let Some(names) = expand_shorthand_properties(property) {
    return names
      .into_iter()
      .map(|name| PropertyExpansion {
        raw_value: raw_value.to_string(),
        name,
      })
      .collect();
  }

  vec![PropertyExpansion {
    name: property.to_string(),
    raw_value: raw_value.to_string(),
  }]
}

fn expand_shorthand_properties(property: &str) -> Option<Vec<String>> {
  match property {
    "overflow" => Some(vec!["overflow-x".into(), "overflow-y".into()]),
    _ => None,
  }
}

pub(crate) fn shorthand_bucket(property: &str) -> Option<u16> {
  match property {
    "all" => Some(0),
    "animation"
    | "animation-range"
    | "background"
    | "border"
    | "border-image"
    | "border-radius"
    | "column-rule"
    | "columns"
    | "contain-intrinsic-size"
    | "container"
    | "flex"
    | "flex-flow"
    | "font"
    | "font-synthesis"
    | "gap"
    | "grid"
    | "grid-area"
    | "grid-template"
    | "inset"
    | "list-style"
    | "margin"
    | "mask"
    | "mask-border"
    | "offset"
    | "outline"
    | "overflow"
    | "overscroll-behavior"
    | "padding"
    | "place-content"
    | "place-items"
    | "place-self"
    | "position-try"
    | "scroll-margin"
    | "scroll-padding"
    | "scroll-timeline"
    | "text-decoration"
    | "text-emphasis"
    | "text-wrap"
    | "transition"
    | "view-timeline" => Some(1),
    "border-color"
    | "border-style"
    | "border-width"
    | "grid-column"
    | "grid-row"
    | "inset-block"
    | "inset-inline"
    | "margin-block"
    | "margin-inline"
    | "padding-block"
    | "padding-inline"
    | "scroll-margin-block"
    | "scroll-margin-inline"
    | "scroll-padding-block"
    | "scroll-padding-inline"
    | "font-variant" => Some(2),
    "border-block" | "border-inline" => Some(3),
    "border-top" | "border-right" | "border-bottom" | "border-left" => Some(4),
    "border-block-start" | "border-block-end" | "border-inline-start" | "border-inline-end" => {
      Some(5)
    }
    _ => None,
  }
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

fn minify_at_rule_params(raw: &str) -> String {
  let mut result = String::with_capacity(raw.len());
  let mut chars = raw.chars().peekable();
  let mut pending_space = false;

  while let Some(ch) = chars.next() {
    if ch.is_ascii_whitespace() {
      pending_space = true;
      continue;
    }
    if pending_space {
      if !result.ends_with('(') && !result.is_empty() {
        result.push(' ');
      }
      pending_space = false;
    }
    result.push(ch);
    if ch == ':' {
      while matches!(chars.peek(), Some(next) if next.is_ascii_whitespace()) {
        chars.next();
      }
    }
  }

  result.trim().to_string()
}

pub(crate) fn wrap_at_rules(mut css: String, at_rules: &[AtRuleInput]) -> String {
  for at_rule in at_rules.iter().rev() {
    let name = at_rule.name.trim();
    let params = minify_at_rule_params(&at_rule.params);
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
  let debug_hash = std::env::var_os("COMPILED_DEBUG_HASH").is_some();

  let mut indices: Vec<usize> = (0..rules.len()).collect();
  indices.sort_by(|&a, &b| {
    let rule_a = &rules[a];
    let rule_b = &rules[b];
    let bucket_a = shorthand_bucket(rule_a.property.as_str()).unwrap_or(u16::MAX);
    let bucket_b = shorthand_bucket(rule_b.property.as_str()).unwrap_or(u16::MAX);
    bucket_a.cmp(&bucket_b).then_with(|| a.cmp(&b))
  });

  for index in indices {
    let rule = &rules[index];
    let normalized_selectors = if rule.selectors.is_empty() {
      vec![normalize_selector(None)]
    } else {
      rule
        .selectors
        .iter()
        .map(|selector| normalize_selector(Some(selector)))
        .collect::<Vec<_>>()
    };

    let at_rule_label: String = if rule.at_rules.is_empty() {
      "undefined".to_string()
    } else {
      rule
        .at_rules
        .iter()
        .map(|input| {
          format!(
            "{}{}",
            input.name.trim(),
            minify_at_rule_params(&input.params)
          )
        })
        .collect()
    };
    let prefix = options.class_hash_prefix.as_deref().unwrap_or("");
    let expansions = expand_property(rule.property.as_str(), &rule.raw_value);

    for expansion in expansions {
      let NormalizedCssValue {
        hash_value,
        output_value,
      } = normalize_css_value(&expansion.raw_value);

      let vendor_values = vendor_prefixed_values(expansion.name.as_str(), &output_value)
        .unwrap_or_else(|| vec![output_value.clone()]);

      let declaration_values: Vec<String> = vendor_values
        .iter()
        .map(|value| {
          if rule.important {
            format!("{}:{}!important", expansion.name, value)
          } else {
            format!("{}:{}", expansion.name, value)
          }
        })
        .collect();
      let declaration = declaration_values.join(";");

      let mut hash_component = hash_value.clone();
      if rule.important {
        hash_component.push_str("!important");
      }
      let value_for_hash = hash_component.clone();
      let value_hash = hash(&value_for_hash, 0);
      let value_segment = &value_hash[..value_hash.len().min(4)];

      let mut per_selector_outputs = Vec::new();
      for selector in &normalized_selectors {
        let selectors_hash = selector.to_string();
        let group_hash = hash(
          &format!(
            "{}{}{}{}",
            prefix, at_rule_label, selectors_hash, expansion.name
          ),
          0,
        );
        let group = &group_hash[..group_hash.len().min(4)];
        if debug_hash {
          eprintln!(
            "[compiled-hash] group-input='{}{}{}{}' selector='{}' property='{}'",
            prefix, at_rule_label, selectors_hash, expansion.name, selector, expansion.name
          );
        }
        if debug_hash {
          eprintln!(
            "[compiled-hash] value-input='{}' important={}",
            value_for_hash, rule.important
          );
        }
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
          format!("{}{{{}}}", combined_selector, declaration.clone()),
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
      let mut raw_value = value.trim().to_string();
      let important_suffix;
      if let Some(stripped) = raw_value.strip_suffix("!important") {
        important_suffix = Some("!important");
        raw_value = stripped.trim_end().to_string();
      } else {
        important_suffix = None;
      }
      let NormalizedCssValue {
        hash_value,
        output_value,
      } = normalize_css_value(&raw_value);
      let prefix = options
        .class_hash_prefix
        .as_ref()
        .map(String::as_str)
        .unwrap_or("");
      let group_hash = hash(&format!("{}undefined{}", prefix, property), 0);
      let group = &group_hash[..group_hash.len().min(4)];
      let value_for_hash = match important_suffix {
        Some(flag) => format!("{}{}", hash_value, flag),
        None => hash_value.clone(),
      };
      let value_hash = hash(&value_for_hash, 0);
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
        format!("{}:{}{}", property, output_value, flag)
      } else {
        format!("{}:{}", property, output_value)
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
  use super::{
    CssOptions, CssRuleInput, atomicize_literal, atomicize_rules, minify_at_rule_params,
    normalize_css_value, vendor_prefixed_values,
  };

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
      .insert("1doq13q2".into(), "a".into());
    let artifacts = atomicize_literal("color: blue;", &options);
    assert_eq!(artifacts.rules.len(), 1);
    let rule = &artifacts.rules[0];
    assert_eq!(rule.class_name, "_1doq_a");
    assert!(rule.css.contains(".a{"));
  }

  #[test]
  fn converts_px_fallbacks_to_pc_inside_var() {
    let normalized = normalize_css_value("var(--ds-space-200, 16px)");
    assert_eq!(normalized.output_value, "var(--ds-space-200,1pc)");
    assert!(normalized.hash_value.contains("1pc"));
  }

  #[test]
  fn does_not_convert_px_inside_identifiers() {
    let normalized = normalize_css_value("url(/images/icon-16px.png)");
    assert_eq!(normalized.output_value, "url(/images/icon-16px.png)");
  }

  #[test]
  fn strips_zero_units_inside_var() {
    let normalized = normalize_css_value("var(--ds-space-0, 0px)");
    assert_eq!(normalized.output_value, "var(--ds-space-0,0)");
  }

  #[test]
  fn vendor_prefixed_values_for_fit_content() {
    let values = vendor_prefixed_values("width", "fit-content").expect("expected expansion");
    assert_eq!(values, vec!["-moz-fit-content", "fit-content"]);
    assert!(vendor_prefixed_values("width", "-moz-fit-content").is_none());
  }

  #[test]
  fn atomicize_rules_emits_vendor_prefixed_fit_content() {
    let rule = CssRuleInput {
      selectors: vec!["&".to_string()],
      at_rules: vec![],
      property: "width".into(),
      value: "fit-content".into(),
      raw_value: "fit-content".into(),
      important: false,
    };
    let artifacts = atomicize_rules(&[rule], &CssOptions::default());
    let css_rule = &artifacts.rules[0].css;
    assert!(
      css_rule.contains("width:-moz-fit-content;width:fit-content"),
      "css rule was {css_rule}"
    );
  }

  #[test]
  fn atomicize_rules_expands_overflow_shorthand() {
    let rule = CssRuleInput {
      selectors: vec!["&".to_string()],
      at_rules: vec![],
      property: "overflow".into(),
      value: "hidden".into(),
      raw_value: "hidden".into(),
      important: false,
    };
    let artifacts = atomicize_rules(&[rule], &CssOptions::default());
    let css_strings: Vec<&str> = artifacts
      .rules
      .iter()
      .map(|rule| rule.css.as_str())
      .collect();
    assert!(
      css_strings
        .iter()
        .any(|css| css.contains("overflow-x:hidden")),
      "css strings were {:?}",
      css_strings
    );
    assert!(
      css_strings
        .iter()
        .any(|css| css.contains("overflow-y:hidden")),
      "css strings were {:?}",
      css_strings
    );
  }

  #[test]
  fn atomicize_rules_expands_flex_shorthand_one() {
    let rule = CssRuleInput {
      selectors: vec!["&".to_string()],
      at_rules: vec![],
      property: "flex".into(),
      value: "1".into(),
      raw_value: "1".into(),
      important: false,
    };
    let artifacts = atomicize_rules(&[rule], &CssOptions::default());
    let css_strings: Vec<&str> = artifacts
      .rules
      .iter()
      .map(|rule| rule.css.as_str())
      .collect();
    assert!(
      css_strings.iter().any(|css| css.contains("flex-grow:1")),
      "css strings were {:?}",
      css_strings
    );
    assert!(
      css_strings.iter().any(|css| css.contains("flex-shrink:1")),
      "css strings were {:?}",
      css_strings
    );
    assert!(
      css_strings.iter().any(|css| css.contains("flex-basis:0%")),
      "css strings were {:?}",
      css_strings
    );
  }

  #[test]
  fn atomicize_rules_expands_text_decoration_none() {
    let rule = CssRuleInput {
      selectors: vec!["&".to_string()],
      at_rules: vec![],
      property: "text-decoration".into(),
      value: "none".into(),
      raw_value: "none".into(),
      important: false,
    };
    let artifacts = atomicize_rules(&[rule], &CssOptions::default());
    let css_strings: Vec<&str> = artifacts
      .rules
      .iter()
      .map(|rule| rule.css.as_str())
      .collect();
    assert!(
      css_strings
        .iter()
        .any(|css| css.contains("text-decoration-line:none")),
      "css strings were {:?}",
      css_strings
    );
    assert!(
      css_strings
        .iter()
        .any(|css| css.contains("text-decoration-color:initial")),
      "css strings were {:?}",
      css_strings
    );
    assert!(
      css_strings
        .iter()
        .any(|css| css.contains("text-decoration-style:solid")),
      "css strings were {:?}",
      css_strings
    );
  }

  #[test]
  fn minifies_media_query_parameters() {
    assert_eq!(
      minify_at_rule_params("(min-width: 90rem)"),
      "(min-width:90rem)"
    );
    assert_eq!(
      minify_at_rule_params("screen and (min-width: 90rem)"),
      "screen and (min-width:90rem)"
    );
  }
}
