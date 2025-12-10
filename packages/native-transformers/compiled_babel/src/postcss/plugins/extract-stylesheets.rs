use swc_core::common::Spanned;
use swc_core::css::ast::{Rule, Stylesheet};
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct ExtractStyleSheets;

impl Plugin for ExtractStyleSheets {
  fn name(&self) -> &'static str {
    "extract-style-sheets"
  }

  fn run(&self, stylesheet: &mut Stylesheet, ctx: &mut TransformContext<'_>) {
    if stylesheet.rules.is_empty() {
      return;
    }

    for rule in &stylesheet.rules {
      let Some(serialized) = serialize_rule(rule) else {
        continue;
      };

      ctx.push_sheet(serialized);
      if std::env::var("COMPILED_CSS_TRACE").is_ok() {
        if let Some(last) = ctx.sheets.last() {
          eprintln!("[extract] sheet='{}'", last);
        }
      }
    }
  }
}

pub fn extract_stylesheets() -> ExtractStyleSheets {
  ExtractStyleSheets
}

fn serialize_rule(rule: &Rule) -> Option<String> {
  serialize_with_codegen(rule).map(|css| normalize_block_value_spacing(&css))
}

fn serialize_with_codegen<T>(node: &T) -> Option<String>
where
  T: Spanned,
  for<'writer> CodeGenerator<BasicCssWriter<'writer, &'writer mut String>>: Emit<T>,
{
  let mut output = String::new();
  {
    let writer = BasicCssWriter::new(&mut output, None, Default::default());
    let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: true });
    if generator.emit(node).is_err() {
      return None;
    }
  }
  Some(output)
}

pub(crate) fn normalize_block_value_spacing(input: &str) -> String {
  let mut output = String::with_capacity(input.len());
  let mut chars = input.chars().peekable();
  let mut inside_block = false;
  let mut calc_stack: Vec<bool> = Vec::new();
  let mut current_ident = String::new();

  while let Some(ch) = chars.next() {
    if ch == '{' {
      inside_block = true;
      calc_stack.clear();
      current_ident.clear();
    } else if ch == '}' {
      inside_block = false;
      calc_stack.clear();
      current_ident.clear();
    }

    if ch == '(' {
      let is_calc = current_ident.eq_ignore_ascii_case("calc");
      calc_stack.push(is_calc);
      current_ident.clear();
    } else if ch == ')' {
      calc_stack.pop();
    } else if ch.is_alphabetic() || ch == '-' {
      current_ident.push(ch);
    } else {
      current_ident.clear();
    }

    output.push(ch);
    if inside_block && ch == ',' {
      let inside_calc = calc_stack.iter().any(|flag| *flag);
      if inside_calc {
        if matches!(chars.peek(), Some(c) if !c.is_whitespace()) {
          output.push(' ');
        }
      } else {
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
          chars.next();
        }
      }
      continue;
    }

    if inside_block && ch == ')' {
      if let Some(&next) = chars.peek() {
        if needs_space_after_token(next) {
          output.push(' ');
        }
      }
    }
  }

  output
}

fn needs_space_after_token(next: char) -> bool {
  matches!(next, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_') && !next.is_whitespace()
}

#[cfg(test)]
mod tests {
  use super::*;
  use swc_core::common::{input::StringInput, FileName, SourceMap};
  use swc_core::css::parser::{parse_string_input, parser::ParserConfig};

  fn parse_stylesheet(css: &str) -> Stylesheet {
    let cm: std::sync::Arc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom("inline.css".into()).into(), css.into());
    let mut errors = vec![];
    parse_string_input::<Stylesheet>(
      StringInput::from(&*fm),
      None,
      ParserConfig::default(),
      &mut errors,
    )
    .expect("failed to parse css")
  }

  #[test]
  fn serializes_padding_shorthand_with_spacing() {
    let css = "._a{padding:5 var(--foo) 0 0}";
    let stylesheet = parse_stylesheet(css);
    assert_eq!(stylesheet.rules.len(), 1);
    let serialized = serialize_rule(&stylesheet.rules[0]).expect("serialize rule");
    assert_eq!(serialized, css);
  }

  #[test]
  fn normalizes_missing_space_after_var_value() {
    let raw = "._a{padding:5 var(--foo)0 0}";
    let normalized = normalize_block_value_spacing(raw);
    assert_eq!(normalized, "._a{padding:5 var(--foo) 0 0}");
  }

  #[test]
  fn trims_var_fallback_outside_calc() {
    let raw = "._a{box-shadow:var(--foo, 10px)}";
    let normalized = normalize_block_value_spacing(raw);
    assert_eq!(normalized, "._a{box-shadow:var(--foo,10px)}");
  }

  #[test]
  fn preserves_var_fallback_inside_calc() {
    let raw = "._a{height:calc(100vh - var(--foo, 10px))}";
    let normalized = normalize_block_value_spacing(raw);
    assert_eq!(normalized, raw);
  }
}
