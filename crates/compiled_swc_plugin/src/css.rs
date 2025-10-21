use crate::hash::hash;
use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use swc_core::ecma::ast::Expr;

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
      flatten_multiple_selectors: false,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicRule {
  pub class_name: String,
  pub css: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeCssVariable {
  pub name: String,
  pub expression: Expr,
  pub prefix: Option<String>,
  pub suffix: Option<String>,
}

impl RuntimeCssVariable {
  pub fn new(
    name: String,
    expression: Expr,
    prefix: Option<String>,
    suffix: Option<String>,
  ) -> Self {
    Self {
      name,
      expression,
      prefix,
      suffix,
    }
  }
}

#[derive(Debug, Clone)]
pub struct CssArtifacts {
  pub rules: Vec<AtomicRule>,
  pub raw_rules: Vec<String>,
  pub runtime_variables: Vec<RuntimeCssVariable>,
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
    self.runtime_variables.extend(other.runtime_variables);
  }

  pub fn push_variable(&mut self, variable: RuntimeCssVariable) {
    self.runtime_variables.push(variable);
  }

  pub fn class_names(&self) -> impl Iterator<Item = &str> {
    self.rules.iter().map(|rule| rule.class_name.as_str())
  }
}

impl Default for CssArtifacts {
  fn default() -> Self {
    Self {
      rules: Vec::new(),
      raw_rules: Vec::new(),
      runtime_variables: Vec::new(),
    }
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

fn normalize_pseudo_element_colons(selector: &str) -> Cow<'_, str> {
  if !selector.contains("::") {
    return Cow::Borrowed(selector);
  }

  let mut output = String::with_capacity(selector.len());
  let mut chars = selector.chars().peekable();
  let mut in_single_quote = false;
  let mut in_double_quote = false;
  let mut escape_next = false;

  while let Some(ch) = chars.next() {
    if escape_next {
      output.push(ch);
      escape_next = false;
      continue;
    }

    match ch {
      '\\' => {
        escape_next = true;
        output.push(ch);
      }
      '\'' if !in_double_quote => {
        in_single_quote = !in_single_quote;
        output.push(ch);
      }
      '"' if !in_single_quote => {
        in_double_quote = !in_double_quote;
        output.push(ch);
      }
      ':' if !in_single_quote && !in_double_quote => {
        output.push(':');
        if matches!(chars.peek(), Some(':')) {
          chars.next();
        }
      }
      _ => output.push(ch),
    }
  }

  Cow::Owned(output)
}

pub fn normalize_selector(selector: Option<&str>) -> String {
  match selector {
    None => "&".to_string(),
    Some(raw) => {
      let trimmed = raw.trim();
      let pseudo_normalized = normalize_pseudo_element_colons(trimmed);
      let normalized = minify_selector(pseudo_normalized.as_ref());
      if normalized.contains('&') {
        if normalized.starts_with(':') {
          return format!("&{}", normalized);
        }
        normalized
      } else if normalized.is_empty() {
        "&".to_string()
      } else if normalized.starts_with(':') || normalized.starts_with('[') {
        format!("&{}", normalized)
      } else {
        format!("& {}", normalized)
      }
    }
  }
}

fn named_color_hex(value: &str) -> Option<&'static str> {
  match value {
    "aliceblue" => Some("#f0f8ff"),
    "antiquewhite" => Some("#faebd7"),
    "aqua" => Some("#00ffff"),
    "aquamarine" => Some("#7fffd4"),
    "azure" => Some("#f0ffff"),
    "beige" => Some("#f5f5dc"),
    "bisque" => Some("#ffe4c4"),
    "black" => Some("#000000"),
    "blanchedalmond" => Some("#ffebcd"),
    "blue" => Some("#0000ff"),
    "blueviolet" => Some("#8a2be2"),
    "brown" => Some("#a52a2a"),
    "burlywood" => Some("#deb887"),
    "cadetblue" => Some("#5f9ea0"),
    "chartreuse" => Some("#7fff00"),
    "chocolate" => Some("#d2691e"),
    "coral" => Some("#ff7f50"),
    "cornflowerblue" => Some("#6495ed"),
    "cornsilk" => Some("#fff8dc"),
    "crimson" => Some("#dc143c"),
    "darkblue" => Some("#00008b"),
    "darkcyan" => Some("#008b8b"),
    "darkgoldenrod" => Some("#b8860b"),
    "darkgray" => Some("#a9a9a9"),
    "darkgreen" => Some("#006400"),
    "darkkhaki" => Some("#bdb76b"),
    "darkmagenta" => Some("#8b008b"),
    "darkolivegreen" => Some("#556b2f"),
    "darkorange" => Some("#ff8c00"),
    "darkorchid" => Some("#9932cc"),
    "darkred" => Some("#8b0000"),
    "darksalmon" => Some("#e9967a"),
    "darkseagreen" => Some("#8fbc8f"),
    "darkslateblue" => Some("#483d8b"),
    "darkslategray" => Some("#2f4f4f"),
    "darkturquoise" => Some("#00ced1"),
    "darkviolet" => Some("#9400d3"),
    "deeppink" => Some("#ff1493"),
    "deepskyblue" => Some("#00bfff"),
    "dimgray" => Some("#696969"),
    "dodgerblue" => Some("#1e90ff"),
    "firebrick" => Some("#b22222"),
    "floralwhite" => Some("#fffaf0"),
    "forestgreen" => Some("#228b22"),
    "fuchsia" => Some("#ff00ff"),
    "gainsboro" => Some("#dcdcdc"),
    "ghostwhite" => Some("#f8f8ff"),
    "gold" => Some("#ffd700"),
    "goldenrod" => Some("#daa520"),
    "gray" => Some("#808080"),
    "green" => Some("#008000"),
    "greenyellow" => Some("#adff2f"),
    "honeydew" => Some("#f0fff0"),
    "hotpink" => Some("#ff69b4"),
    "indianred" => Some("#cd5c5c"),
    "indigo" => Some("#4b0082"),
    "ivory" => Some("#fffff0"),
    "khaki" => Some("#f0e68c"),
    "lavender" => Some("#e6e6fa"),
    "lavenderblush" => Some("#fff0f5"),
    "lawngreen" => Some("#7cfc00"),
    "lemonchiffon" => Some("#fffacd"),
    "lightblue" => Some("#add8e6"),
    "lightcoral" => Some("#f08080"),
    "lightcyan" => Some("#e0ffff"),
    "lightgoldenrodyellow" => Some("#fafad2"),
    "lightgray" => Some("#d3d3d3"),
    "lightgreen" => Some("#90ee90"),
    "lightpink" => Some("#ffb6c1"),
    "lightsalmon" => Some("#ffa07a"),
    "lightseagreen" => Some("#20b2aa"),
    "lightskyblue" => Some("#87cefa"),
    "lightslategray" => Some("#778899"),
    "lightsteelblue" => Some("#b0c4de"),
    "lightyellow" => Some("#ffffe0"),
    "lime" => Some("#00ff00"),
    "limegreen" => Some("#32cd32"),
    "linen" => Some("#faf0e6"),
    "maroon" => Some("#800000"),
    "mediumaquamarine" => Some("#66cdaa"),
    "mediumblue" => Some("#0000cd"),
    "mediumorchid" => Some("#ba55d3"),
    "mediumpurple" => Some("#9370db"),
    "mediumseagreen" => Some("#3cb371"),
    "mediumslateblue" => Some("#7b68ee"),
    "mediumspringgreen" => Some("#00fa9a"),
    "mediumturquoise" => Some("#48d1cc"),
    "mediumvioletred" => Some("#c71585"),
    "midnightblue" => Some("#191970"),
    "mintcream" => Some("#f5fffa"),
    "mistyrose" => Some("#ffe4e1"),
    "moccasin" => Some("#ffe4b5"),
    "navajowhite" => Some("#ffdead"),
    "navy" => Some("#000080"),
    "oldlace" => Some("#fdf5e6"),
    "olive" => Some("#808000"),
    "olivedrab" => Some("#6b8e23"),
    "orange" => Some("#ff8000"),
    "orangered" => Some("#ff4500"),
    "orchid" => Some("#da70d6"),
    "palegoldenrod" => Some("#eee8aa"),
    "palegreen" => Some("#98fb98"),
    "paleturquoise" => Some("#afeeee"),
    "palevioletred" => Some("#db7093"),
    "papayawhip" => Some("#ffefd5"),
    "peachpuff" => Some("#ffdab9"),
    "peru" => Some("#cd853f"),
    "pink" => Some("#ffc0cb"),
    "plum" => Some("#dda0dd"),
    "powderblue" => Some("#b0e0e6"),
    "purple" => Some("#800080"),
    "rebeccapurple" => Some("#663399"),
    "red" => Some("#ff0000"),
    "rosybrown" => Some("#bc8f8f"),
    "royalblue" => Some("#4169e1"),
    "saddlebrown" => Some("#8b4513"),
    "salmon" => Some("#fa8072"),
    "sandybrown" => Some("#f4a460"),
    "seagreen" => Some("#2e8b57"),
    "seashell" => Some("#fff5ee"),
    "sienna" => Some("#a0522d"),
    "silver" => Some("#c0c0c0"),
    "skyblue" => Some("#87ceeb"),
    "slateblue" => Some("#6a5acd"),
    "slategray" => Some("#708090"),
    "snow" => Some("#fffafa"),
    "springgreen" => Some("#00ff7f"),
    "steelblue" => Some("#4682b4"),
    "tan" => Some("#d2b48c"),
    "teal" => Some("#008080"),
    "thistle" => Some("#d8bfd8"),
    "tomato" => Some("#ff6347"),
    "turquoise" => Some("#40e0d0"),
    "violet" => Some("#ee82ee"),
    "wheat" => Some("#f5deb3"),
    "white" => Some("#ffffff"),
    "whitesmoke" => Some("#f5f5f5"),
    "yellow" => Some("#ffff00"),
    "yellowgreen" => Some("#9acd32"),
    _ => None,
  }
}

pub fn add_unit_if_needed(name: &str, value: &str) -> String {
  if value.is_empty() {
    return String::new();
  }

  if let Ok(number) = value.parse::<f64>() {
    let unitless = if is_unitless_property(name) {
      true
    } else if let Some(camel) = crate::to_camel_case(name) {
      is_unitless_property(&camel)
    } else {
      false
    };

    if number == 0.0 || unitless {
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
    if ch == '#' {
      if index + 8 < chars.len() {
        let slice = &chars[index + 1..index + 9];
        if slice.iter().all(|c| c.is_ascii_hexdigit()) {
          if slice[0] == slice[1]
            && slice[2] == slice[3]
            && slice[4] == slice[5]
            && slice[6] == slice[7]
          {
            output.push('#');
            output.push(slice[0]);
            output.push(slice[2]);
            output.push(slice[4]);
            output.push(slice[6]);
            index += 9;
            continue;
          }
        }
      }
      if index + 6 < chars.len()
        && !(index + 7 < chars.len() && chars[index + 7].is_ascii_hexdigit())
      {
        let slice = &chars[index + 1..index + 7];
        if slice.iter().all(|c| c.is_ascii_hexdigit())
          && slice[0] == slice[1]
          && slice[2] == slice[3]
          && slice[4] == slice[5]
        {
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

  let mut semantic = trimmed.to_string();
  let lower_trimmed = trimmed.to_ascii_lowercase();
  if let Some(hex) = named_color_hex(&lower_trimmed) {
    let shortened = shorten_hex_literals(hex);
    let candidate = if shortened.len() < hex.len() {
      shortened
    } else {
      hex.to_string()
    };
    if candidate.len() < semantic.len() {
      semantic = candidate;
    } else if hex.len() < semantic.len() {
      semantic = hex.to_string();
    }
  }
  if lower_trimmed == "currentcolor" || lower_trimmed == "current-color" {
    semantic = "currentColor".to_string();
  }
  semantic = lowercase_hex_literals(&semantic);
  semantic = shorten_hex_literals(&semantic);
  semantic = strip_decimal_leading_zeros(&semantic);
  semantic = convert_length_units(&semantic);
  semantic = convert_color_functions_to_hex(&semantic);
  semantic = lowercase_hex_literals(&semantic);
  semantic = shorten_hex_literals(&semantic);
  semantic = strip_zero_units(&semantic);
  let output = minify_whitespace(&semantic);
  let hash_value = output.clone();

  NormalizedCssValue {
    hash_value,
    output_value: output,
  }
}

fn strip_decimal_leading_zeros(value: &str) -> String {
  let mut output = String::with_capacity(value.len());
  let mut chars = value.chars().peekable();
  let mut prev: Option<char> = None;

  while let Some(ch) = chars.next() {
    if ch == '0' {
      if let Some('.') = chars.peek().copied() {
        if !prev
          .map(|c| c.is_ascii_digit() || c == '.')
          .unwrap_or(false)
        {
          let mut lookahead = chars.clone();
          lookahead.next(); // skip the dot
          let mut digits = 0usize;
          while let Some(next) = lookahead.peek() {
            if next.is_ascii_digit() {
              digits += 1;
              lookahead.next();
            } else {
              break;
            }
          }
          if digits > 0 && digits <= 2 {
            chars.next(); // consume dot
            output.push('.');
            prev = Some('.');
            continue;
          }
        }
      }
    }

    output.push(ch);
    prev = Some(ch);
  }

  output
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

    let mut number_start = cursor;
    while number_start < px_index {
      let ch = core.as_bytes()[number_start] as char;
      if ch.is_ascii_whitespace() {
        number_start += 1;
      } else {
        break;
      }
    }

    if number_start >= px_index {
      search_start = px_index + 2;
      continue;
    }

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

fn parse_rgb_component(component: &str) -> Option<u8> {
  let trimmed = component.trim();
  if trimmed.is_empty() {
    return None;
  }
  if let Some(stripped) = trimmed.strip_suffix('%') {
    let value: f64 = stripped.trim().parse().ok()?;
    let clamped = value.max(0.0).min(100.0);
    Some((clamped * 255.0 / 100.0).round().max(0.0).min(255.0) as u8)
  } else {
    let value: f64 = trimmed.parse().ok()?;
    let clamped = value.max(0.0).min(255.0);
    Some(clamped.round().max(0.0).min(255.0) as u8)
  }
}

fn parse_alpha_component(component: &str) -> Option<u8> {
  let trimmed = component.trim();
  if trimmed.is_empty() {
    return None;
  }
  if let Some(stripped) = trimmed.strip_suffix('%') {
    let value: f64 = stripped.trim().parse().ok()?;
    let clamped = value.max(0.0).min(100.0);
    Some((clamped * 255.0 / 100.0).round().max(0.0).min(255.0) as u8)
  } else {
    let value: f64 = trimmed.parse().ok()?;
    let clamped = value.max(0.0).min(1.0);
    Some((clamped * 255.0).round().max(0.0).min(255.0) as u8)
  }
}

fn convert_rgb_like_to_hex(segment: &str, is_rgba: bool) -> Option<(String, usize)> {
  let prefix_len = if is_rgba { 4 } else { 3 };
  if segment.len() <= prefix_len || !segment.as_bytes()[prefix_len].eq(&b'(') {
    return None;
  }
  let start = prefix_len + 1;
  let end_rel = segment[start..].find(')')?;
  let end = start + end_rel;
  let inner = &segment[start..end];
  let consumed = end + 1;
  let components: Vec<&str> = inner.split(',').map(|part| part.trim()).collect();
  if is_rgba && components.len() != 4 {
    return None;
  }
  if !is_rgba && components.len() != 3 {
    return None;
  }

  let r = parse_rgb_component(components.get(0)?.trim())?;
  let g = parse_rgb_component(components.get(1)?.trim())?;
  let b = parse_rgb_component(components.get(2)?.trim())?;

  if is_rgba {
    let a = parse_alpha_component(components.get(3)?.trim())?;
    if a == 255 {
      Some((format!("#{:02x}{:02x}{:02x}", r, g, b), consumed))
    } else {
      Some((format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a), consumed))
    }
  } else {
    Some((format!("#{:02x}{:02x}{:02x}", r, g, b), consumed))
  }
}

fn convert_color_functions_to_hex(value: &str) -> String {
  let mut output = String::with_capacity(value.len());
  let mut index = 0usize;

  while index < value.len() {
    let rest = &value[index..];
    let mut consumed = 0usize;

    if rest.len() >= 5 && rest[..4].eq_ignore_ascii_case("rgba") {
      if let Some(result) = convert_rgb_like_to_hex(rest, true) {
        output.push_str(&result.0);
        consumed = result.1;
      }
    } else if rest.len() >= 4 && rest[..3].eq_ignore_ascii_case("rgb") {
      if let Some(result) = convert_rgb_like_to_hex(rest, false) {
        output.push_str(&result.0);
        consumed = result.1;
      }
    }

    if consumed > 0 {
      index += consumed;
      continue;
    }

    let mut chars = rest.chars();
    if let Some(ch) = chars.next() {
      output.push(ch);
      index += ch.len_utf8();
    } else {
      break;
    }
  }

  output
}

fn canonicalize_selector_key(selector: &str) -> String {
  let mut output = String::with_capacity(selector.len());
  let mut chars = selector.chars().peekable();

  while let Some(ch) = chars.next() {
    if ch == '.' && matches!(chars.peek(), Some('_')) {
      output.push_str("._HASH");
      chars.next();
      while let Some(next) = chars.peek() {
        if next.is_ascii_alphanumeric() || *next == '_' {
          chars.next();
        } else {
          break;
        }
      }
      continue;
    }
    output.push(ch);
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

fn should_skip_shorthand_expansion(raw_value: &str) -> bool {
  raw_value.to_ascii_lowercase().contains("var(")
}

fn split_css_value_components(raw_value: &str) -> Vec<String> {
  let mut parts = Vec::new();
  let mut current = String::new();
  let mut paren_depth = 0usize;
  let mut bracket_depth = 0usize;
  let mut brace_depth = 0usize;
  let mut string_delim: Option<char> = None;
  let mut escape_next = false;

  for ch in raw_value.chars() {
    if let Some(delim) = string_delim {
      current.push(ch);
      if escape_next {
        escape_next = false;
        continue;
      }
      if ch == '\\' {
        escape_next = true;
      } else if ch == delim {
        string_delim = None;
      }
      continue;
    }

    match ch {
      '"' | '\'' => {
        string_delim = Some(ch);
        current.push(ch);
      }
      '(' => {
        paren_depth += 1;
        current.push(ch);
      }
      ')' => {
        if paren_depth > 0 {
          paren_depth -= 1;
        }
        current.push(ch);
      }
      '[' => {
        bracket_depth += 1;
        current.push(ch);
      }
      ']' => {
        if bracket_depth > 0 {
          bracket_depth -= 1;
        }
        current.push(ch);
      }
      '{' => {
        brace_depth += 1;
        current.push(ch);
      }
      '}' => {
        if brace_depth > 0 {
          brace_depth -= 1;
        }
        current.push(ch);
      }
      ' ' | '\t' | '\n' | '\r' => {
        if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
          if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
            current.clear();
          }
        } else {
          current.push(ch);
        }
      }
      _ => current.push(ch),
    }
  }

  if !current.trim().is_empty() {
    parts.push(current.trim().to_string());
  }

  parts
}

fn expand_box_shorthand(property: &str, raw_value: &str) -> Vec<PropertyExpansion> {
  if should_skip_shorthand_expansion(raw_value) {
    return vec![PropertyExpansion {
      name: property.to_string(),
      raw_value: raw_value.to_string(),
    }];
  }

  let values = split_css_value_components(raw_value);
  if values.is_empty() || values.len() > 4 {
    return vec![PropertyExpansion {
      name: property.to_string(),
      raw_value: raw_value.to_string(),
    }];
  }

  let top = values[0].clone();
  let right = values.get(1).cloned().unwrap_or_else(|| top.clone());
  let bottom = values.get(2).cloned().unwrap_or_else(|| top.clone());
  let left = values.get(3).cloned().unwrap_or_else(|| right.clone());

  vec![
    PropertyExpansion {
      name: format!("{}-top", property),
      raw_value: top,
    },
    PropertyExpansion {
      name: format!("{}-right", property),
      raw_value: right,
    },
    PropertyExpansion {
      name: format!("{}-bottom", property),
      raw_value: bottom,
    },
    PropertyExpansion {
      name: format!("{}-left", property),
      raw_value: left,
    },
  ]
}

const OUTLINE_STYLE_VALUES: &[&str] = &[
  "auto", "none", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset",
];

const OUTLINE_GLOBAL_VALUES: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];

const OUTLINE_WIDTH_UNITS: &[&str] = &[
  "%", "cap", "ch", "cm", "em", "ex", "fr", "ic", "in", "lh", "mm", "pc", "pt", "px", "q", "rem",
  "rlh", "vb", "vh", "vi", "vmax", "vmin", "vw",
];

fn is_outline_style_value(value: &str) -> bool {
  let lower = value.trim().to_ascii_lowercase();
  OUTLINE_STYLE_VALUES
    .iter()
    .any(|candidate| lower == *candidate)
    || OUTLINE_GLOBAL_VALUES
      .iter()
      .any(|candidate| lower == *candidate)
}

fn is_outline_width_value(value: &str) -> bool {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return false;
  }
  let lower = trimmed.to_ascii_lowercase();
  if OUTLINE_GLOBAL_VALUES
    .iter()
    .any(|candidate| lower == *candidate)
  {
    return true;
  }
  if matches!(
    lower.as_str(),
    "auto" | "thin" | "medium" | "thick" | "min-content" | "max-content" | "fit-content"
  ) {
    return true;
  }
  if trimmed.contains('(') {
    return true;
  }
  let mut chars = trimmed.chars().peekable();
  if matches!(chars.peek(), Some(c) if *c == '+' || *c == '-') {
    chars.next();
  }
  let mut has_digit = false;
  while let Some(ch) = chars.peek() {
    if ch.is_ascii_digit() {
      has_digit = true;
      chars.next();
      continue;
    }
    if *ch == '.' {
      chars.next();
      continue;
    }
    break;
  }
  if !has_digit {
    return false;
  }
  let unit: String = chars.collect();
  if unit.is_empty() {
    return true;
  }
  let lower_unit = unit.trim().to_ascii_lowercase();
  OUTLINE_WIDTH_UNITS
    .iter()
    .any(|candidate| lower_unit == *candidate)
}

fn is_outline_color_value(value: &str) -> bool {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return false;
  }
  let lower = trimmed.to_ascii_lowercase();
  if OUTLINE_GLOBAL_VALUES
    .iter()
    .any(|candidate| lower == *candidate)
  {
    return true;
  }
  if lower == "transparent" || lower == "currentcolor" {
    return true;
  }
  if lower.starts_with('#') {
    return lower.chars().skip(1).all(|ch| ch.is_ascii_hexdigit());
  }
  if named_color_hex(&lower).is_some() {
    return true;
  }
  if let Some(index) = lower.find('(') {
    let func = &lower[..index];
    return matches!(
      func,
      "rgb"
        | "rgba"
        | "hsl"
        | "hsla"
        | "hwb"
        | "lab"
        | "lch"
        | "color"
        | "device-cmyk"
        | "oklab"
        | "oklch"
    );
  }
  false
}

fn expand_outline_shorthand(raw_value: &str) -> Option<Vec<PropertyExpansion>> {
  if should_skip_shorthand_expansion(raw_value) {
    return None;
  }

  let values = split_css_value_components(raw_value);
  if values.len() > 3 {
    return None;
  }

  let mut color_value: Option<String> = None;
  let mut style_value: Option<String> = None;
  let mut width_value: Option<String> = None;

  for value in values {
    if is_outline_color_value(&value) {
      if color_value.is_some() {
        return None;
      }
      color_value = Some(value);
      continue;
    }
    if is_outline_style_value(&value) {
      if style_value.is_some() {
        return None;
      }
      style_value = Some(value);
      continue;
    }
    if is_outline_width_value(&value) {
      if width_value.is_some() {
        return None;
      }
      width_value = Some(value);
      continue;
    }
    return None;
  }

  let color = color_value.unwrap_or_else(|| "currentColor".to_string());
  let style = style_value.unwrap_or_else(|| "none".to_string());
  let width = width_value.unwrap_or_else(|| "medium".to_string());

  Some(vec![
    PropertyExpansion {
      name: "outline-color".into(),
      raw_value: color,
    },
    PropertyExpansion {
      name: "outline-style".into(),
      raw_value: style,
    },
    PropertyExpansion {
      name: "outline-width".into(),
      raw_value: width,
    },
  ])
}

fn is_text_decoration_color_token(value: &str) -> bool {
  let lower = value.to_ascii_lowercase();
  if lower.starts_with('#') && lower.len() > 1 && lower[1..].chars().all(|c| c.is_ascii_hexdigit())
  {
    return true;
  }
  if named_color_hex(lower.as_str()).is_some() {
    return true;
  }
  if matches!(lower.as_str(), "transparent" | "currentcolor") {
    return true;
  }
  if lower.starts_with("rgb(")
    || lower.starts_with("rgba(")
    || lower.starts_with("hsl(")
    || lower.starts_with("hsla(")
    || lower.starts_with("hwb(")
    || lower.starts_with("lab(")
    || lower.starts_with("lch(")
    || lower.starts_with("oklab(")
    || lower.starts_with("oklch(")
    || lower.starts_with("color(")
    || lower.starts_with("var(")
  {
    return true;
  }
  false
}

fn expand_text_decoration(raw_value: &str) -> Option<Vec<PropertyExpansion>> {
  let trimmed = raw_value.trim();
  if trimmed.is_empty() {
    return Some(Vec::new());
  }

  const GLOBAL_VALUES: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];
  const LINE_KEYWORDS: &[&str] = &["none", "underline", "overline", "line-through", "blink"];
  const STYLE_KEYWORDS: &[&str] = &["solid", "double", "dotted", "dashed", "wavy"];

  let mut line_values: Vec<String> = Vec::new();
  let mut style_value: Option<String> = None;
  let mut color_value: Option<String> = None;

  for token in trimmed.split_whitespace() {
    let lower = token.to_ascii_lowercase();
    if LINE_KEYWORDS.contains(&lower.as_str()) || GLOBAL_VALUES.contains(&lower.as_str()) {
      if line_values.contains(&lower) {
        return Some(Vec::new());
      }
      line_values.push(lower);
      continue;
    }
    if STYLE_KEYWORDS.contains(&lower.as_str()) || GLOBAL_VALUES.contains(&lower.as_str()) {
      if style_value.is_some() {
        return Some(Vec::new());
      }
      style_value = Some(lower);
      continue;
    }
    if is_text_decoration_color_token(&lower) {
      if color_value.is_some() {
        return Some(Vec::new());
      }
      let normalized = if lower == "currentcolor" {
        "currentColor".to_string()
      } else {
        lower
      };
      color_value = Some(normalized);
      continue;
    }
    return Some(Vec::new());
  }

  line_values.sort();
  let resolved_line = if line_values.is_empty() {
    "none".to_string()
  } else {
    line_values.join(" ")
  };
  let color = color_value.unwrap_or_else(|| "initial".into());
  let style = style_value.unwrap_or_else(|| "solid".into());

  Some(vec![
    PropertyExpansion {
      name: "text-decoration-color".into(),
      raw_value: color,
    },
    PropertyExpansion {
      name: "text-decoration-line".into(),
      raw_value: resolved_line,
    },
    PropertyExpansion {
      name: "text-decoration-style".into(),
      raw_value: style,
    },
  ])
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
    if let Some(expanded) = expand_text_decoration(raw_value) {
      return expanded;
    }
  }

  if property == "padding" || property == "margin" {
    return expand_box_shorthand(property, raw_value);
  }

  if property == "outline" {
    if let Some(expanded) = expand_outline_shorthand(raw_value) {
      return expanded;
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

fn minify_selector(selector: &str) -> String {
  let mut result = String::with_capacity(selector.len());
  let mut chars = selector.chars().peekable();

  while let Some(ch) = chars.next() {
    if ch.is_ascii_whitespace() {
      while matches!(chars.peek(), Some(next) if next.is_ascii_whitespace()) {
        chars.next();
      }
      let mut prev_non_whitespace = None;
      for ch in result.chars().rev() {
        if !ch.is_ascii_whitespace() {
          prev_non_whitespace = Some(ch);
          break;
        }
      }
      let prev_is_combinator = prev_non_whitespace
        .map(|c| matches!(c, '>' | '+' | '~' | ','))
        .unwrap_or(false);
      let next_is_combinator = chars
        .peek()
        .map(|c| matches!(c, '>' | '+' | '~' | ','))
        .unwrap_or(false);
      if prev_non_whitespace == Some('&') && next_is_combinator {
        result.push(' ');
        continue;
      }
      if prev_is_combinator || next_is_combinator {
        continue;
      }
      result.push(' ');
    } else if matches!(ch, '>' | '+' | '~' | ',') {
      while result.ends_with(' ') {
        let trimmed = result.trim_end_matches(' ');
        if trimmed.chars().last() == Some('&') {
          break;
        }
        result.pop();
      }
      result.push(ch);
      while matches!(chars.peek(), Some(next) if next.is_ascii_whitespace()) {
        chars.next();
      }
    } else {
      result.push(ch);
    }
  }

  result
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

pub(crate) fn minify_at_rule_params(raw: &str) -> String {
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

    let mut at_rule_label = if rule.at_rules.is_empty() {
      "undefined".to_string()
    } else {
      String::new()
    };
    if !rule.at_rules.is_empty() {
      for input in &rule.at_rules {
        at_rule_label.push_str(input.name.trim());
        at_rule_label.push_str(&minify_at_rule_params(&input.params));
      }
    }
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
        let mut selector_parts = per_selector_outputs
          .iter()
          .map(|(_, selector, _)| {
            let canonical = canonicalize_selector_key(selector.as_str());
            (canonical, selector.as_str())
          })
          .collect::<Vec<_>>();
        selector_parts.sort_by(|a, b| a.0.cmp(&b.0));
        let combined_selector = selector_parts
          .iter()
          .map(|(_, original)| *original)
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
    minify_selector, normalize_css_value, normalize_selector, vendor_prefixed_values,
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
  fn lowercases_hex_fallbacks_inside_var() {
    let normalized = normalize_css_value("var(--ds-surface-overlay, #FFFFFF)");
    assert_eq!(normalized.output_value, "var(--ds-surface-overlay,#fff)");
  }

  #[test]
  fn converts_rgba_to_hex() {
    let normalized = normalize_css_value("rgba(10, 20, 30, 0.8)");
    assert_eq!(normalized.output_value, "#0a141ecc");
  }

  #[test]
  fn normalize_selector_preserves_combinator_space() {
    assert_eq!(
      normalize_selector(Some("> button")),
      "& >button".to_string()
    );
    assert_eq!(normalize_selector(Some(">button")), "& >button".to_string());
    assert_eq!(
      normalize_selector(Some(" >button")),
      "& >button".to_string()
    );
    assert_eq!(minify_selector("& >button"), "& >button".to_string());
    assert_eq!(
      normalize_selector(Some("& >button")),
      "& >button".to_string()
    );
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

  #[test]
  fn normalize_selector_collapses_pseudo_element_colons() {
    assert_eq!(normalize_selector(Some("span::before")), "& span:before");
    assert_eq!(normalize_selector(Some("::after")), "&:after");
    assert_eq!(normalize_selector(Some("&::before")), "&:before");
    assert_eq!(
      normalize_selector(Some("[data-attr=\"a::b\"]")),
      "&[data-attr=\"a::b\"]"
    );
  }
}
