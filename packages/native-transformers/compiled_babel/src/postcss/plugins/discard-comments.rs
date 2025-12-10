use crate::postcss::plugins::expand_shorthands::types::{
  declaration_property_name, parse_value_to_components, serialize_component_values,
};
use crate::postcss::transform::{Plugin, TransformContext};
use swc_core::css::ast::{AtRule, ComponentValue, Declaration, QualifiedRule, Rule, SimpleBlock, Stylesheet};

/// Native translation of `postcss-discard-comments` that operates on the raw
/// CSS source prior to parsing. The SWC parser drops comment nodes, so the
/// plugin's primary responsibility in Rust is to capture the comments that the
/// PostCSS stack would have preserved (notably `/*! … */` license banners when
/// optimisations are enabled, or _all_ comments when optimisations are
/// disabled) so they can be re-emitted after transformation.
#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardComments;

impl Plugin for DiscardComments {
  fn name(&self) -> &'static str {
    "postcss-discard-comments"
  }

  fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
    // The SWC parser drops comment nodes up front, so at runtime there are no
    // comment AST nodes to discard. However, the JS plugin also normalizes the
    // whitespace around comments via `list.space().join(' ')` which collapses
    // newlines and tabs into single spaces for declaration values (even when no
    // comments are present). We mirror that effect here so the value seeds used
    // for hashing match Babel.
    normalize_stylesheet(stylesheet);
  }
}

pub fn discard_comments() -> DiscardComments {
  DiscardComments
}

fn normalize_stylesheet(stylesheet: &mut Stylesheet) {
  for rule in &mut stylesheet.rules {
    normalize_rule(rule);
  }
}

fn normalize_rule(rule: &mut Rule) {
  match rule {
    Rule::QualifiedRule(rule) => normalize_qualified_rule(rule),
    Rule::AtRule(rule) => normalize_at_rule(rule),
    Rule::ListOfComponentValues(list) => {
      normalize_components(&mut list.children);
    }
  }
}

fn normalize_qualified_rule(rule: &mut Box<QualifiedRule>) {
  normalize_block(&mut rule.block);
}

fn normalize_at_rule(rule: &mut Box<AtRule>) {
  if let Some(block) = &mut rule.block {
    normalize_block(block);
  }
}

fn normalize_block(block: &mut SimpleBlock) {
  for component in &mut block.value {
    match component {
      ComponentValue::Declaration(decl) => normalize_declaration(decl),
      ComponentValue::QualifiedRule(rule) => normalize_qualified_rule(rule),
      ComponentValue::AtRule(rule) => normalize_at_rule(rule),
      ComponentValue::SimpleBlock(inner) => normalize_block(inner),
      ComponentValue::ListOfComponentValues(list) => {
        normalize_components(&mut list.children);
      }
      _ => {}
    }
  }
}

fn normalize_components(components: &mut Vec<ComponentValue>) {
  for component in components {
    match component {
      ComponentValue::Declaration(decl) => normalize_declaration(decl),
      ComponentValue::QualifiedRule(rule) => normalize_qualified_rule(rule),
      ComponentValue::AtRule(rule) => normalize_at_rule(rule),
      ComponentValue::SimpleBlock(block) => normalize_block(block),
      ComponentValue::ListOfComponentValues(list) => {
        normalize_components(&mut list.children);
      }
      _ => {}
    }
  }
}

fn normalize_declaration(declaration: &mut Declaration) {
  let prop_name = declaration_property_name(&declaration.name);
  if std::env::var("COMPILED_CLI_TRACE").is_ok() && prop_name == "grid-template-areas" {
    eprintln!("[discard-comments] visit prop=grid-template-areas");
  }
  let original_value = match serialize_component_values(&declaration.value) {
    Some(value) => value,
    None => return,
  };

  let normalized = collapse_whitespace(&original_value);

  if std::env::var("COMPILED_CLI_TRACE").is_ok()
    && prop_name == "grid-template-areas"
  {
    eprintln!(
      "[discard-comments] prop=grid-template-areas before='{}' after='{}'",
      original_value.replace('\n', "\\n"),
      normalized
    );
  }

  if normalized != original_value {
    declaration.value = parse_value_to_components(&normalized);
  }
}

/// Mirrors `postcss.list.space()` splitting with a join of `' '`, which trims
/// the segments and collapses whitespace that isn't inside quotes or
/// parentheses.
fn collapse_whitespace(input: &str) -> String {
  if input.is_empty() {
    return String::new();
  }

  let separators = [' ', '\n', '\t'];
  let mut parts: Vec<String> = Vec::new();
  let mut current = String::new();
  let mut split = false;
  let mut func_depth = 0i32;
  let mut in_quote: Option<char> = None;
  let mut escape = false;

  for ch in input.chars() {
    if escape {
      escape = false;
    } else if ch == '\\' {
      escape = true;
    } else if let Some(q) = in_quote {
      if ch == q {
        in_quote = None;
      }
    } else if ch == '"' || ch == '\'' {
      in_quote = Some(ch);
    } else if ch == '(' {
      func_depth += 1;
    } else if ch == ')' {
      if func_depth > 0 {
        func_depth -= 1;
      }
    } else if func_depth == 0 && separators.contains(&ch) {
      split = true;
    }

    if split {
      if !current.is_empty() {
        parts.push(current.trim().to_string());
      }
      current.clear();
      split = false;
    } else {
      current.push(ch);
    }
  }

  if !current.is_empty() {
    parts.push(current.trim().to_string());
  }

  parts.join(" ")
}

/// Collect the comments that should be preserved according to
/// `postcss-discard-comments`' default semantics.
///
/// When `optimize_css` is enabled we keep only "important" comments – those
/// whose contents begin with `!`. When optimisations are disabled we retain all
/// comments so the non-minified output mirrors Babel's behaviour.
pub fn collect_preserved_comments(css: &str, optimize_css: Option<bool>) -> Vec<String> {
  let mut preserved = Vec::new();
  let keep_only_important = optimize_css.unwrap_or(true);

  let bytes = css.as_bytes();
  let mut index = 0;

  while index + 1 < bytes.len() {
    if bytes[index] == b'/' && bytes[index + 1] == b'*' {
      if let Some(end) = find_comment_end(bytes, index + 2) {
        let body = &css[index + 2..end];
        let is_important = body.starts_with('!');

        if !keep_only_important || is_important {
          let mut comment = format!("/*{}*/", body);
          let mut cursor = end + 2;
          while cursor < bytes.len() {
            let ch = bytes[cursor];
            if matches!(ch, b' ' | b'\t' | b'\n' | b'\r') {
              comment.push(ch as char);
              cursor += 1;
            } else {
              break;
            }
          }
          preserved.push(comment);
          index = cursor;
          continue;
        }

        index = end + 2;
        continue;
      } else {
        // Unterminated comment – conservatively capture the rest.
        let body = &css[index + 2..];
        if !keep_only_important || body.starts_with('!') {
          preserved.push(format!("/*{}", body));
        }
        break;
      }
    }

    index += 1;
  }

  preserved
}

fn find_comment_end(bytes: &[u8], mut index: usize) -> Option<usize> {
  while index + 1 < bytes.len() {
    if bytes[index] == b'*' && bytes[index + 1] == b'/' {
      return Some(index);
    }
    index += 1;
  }
  None
}

#[cfg(test)]
mod tests {
  use super::collect_preserved_comments;

  #[test]
  fn collects_important_comments_when_optimising() {
    let css = "/*! keep */ .a { color: red; } /* drop */";
    let preserved = collect_preserved_comments(css, Some(true));
    assert_eq!(preserved, vec!["/*! keep */ ".to_string()]);
  }

  #[test]
  fn collects_all_comments_when_not_optimising() {
    let css = "/* first */ .a { /* second */ color: red; }";
    let preserved = collect_preserved_comments(css, Some(false));
    assert_eq!(
      preserved,
      vec!["/* first */ ".to_string(), "/* second */ ".to_string()]
    );
  }
}
