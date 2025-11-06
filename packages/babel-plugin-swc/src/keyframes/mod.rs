//! Port of `src/keyframes` from the Babel plugin.
//!
//! The original implementation exposes helpers for generating stable
//! `@keyframes` names and serialising their bodies. The SWC port keeps the same
//! responsibilities but translates them into a Rust-friendly API that the
//! transformer can invoke.

use swc_core::common::{sync::Lrc, SourceMap, DUMMY_SP};
use swc_core::ecma::ast::{
    CallExpr, EsVersion, Expr, ExprStmt, Module, ModuleItem, Stmt, TaggedTpl,
};
use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};

use crate::{
    css::property::{
        collapse_whitespace, normalise_property_pair, serialize_css_object, CssObject, CssValue,
    },
    options::PluginConfig,
    utils::hash::murmur2_hash,
};

/// Builds the hashed keyframes name and full `@keyframes` rule string for the
/// provided call expression and already-evaluated CSS object.
pub fn build_keyframes_rule(
    call: &CallExpr,
    map: &CssObject,
    config: &PluginConfig,
) -> Option<(String, String)> {
    let body = serialize_css_object(map, config)?;
    let name = generate_keyframes_name(&Expr::Call(call.clone()));
    let rule = format!("@keyframes {name}{{{body}}}");

    Some((name, rule))
}

pub fn build_keyframes_rule_from_template(
    tagged: &TaggedTpl,
    raw_value: &str,
    config: &PluginConfig,
) -> Option<(String, String)> {
    let body = normalise_keyframes_body(raw_value, config)?;
    let name = generate_keyframes_name(&Expr::TaggedTpl(tagged.clone()));
    let rule = format!("@keyframes {name}{{{body}}}");

    Some((name, rule))
}

fn generate_keyframes_name(expr: &Expr) -> String {
    let code = emit_expression(expr);
    let hash = murmur2_hash(&code, 0);
    format!("k{hash}")
}

fn emit_expression(expr: &Expr) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buf = Vec::new();

    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: CodegenConfig::default().with_target(EsVersion::Es2022),
            comments: None,
            cm,
            wr: writer,
        };

        let module = Module {
            span: DUMMY_SP,
            body: vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(expr.clone()),
            }))],
            shebang: None,
        };

        emitter
            .emit_module(&module)
            .expect("emit expression module");
    }

    let code = String::from_utf8(buf).expect("utf8 expression");
    let trimmed = code.trim();
    let without_semicolon = trimmed.strip_suffix(';').unwrap_or(trimmed);
    without_semicolon.trim().to_string()
}

fn normalise_keyframes_body(raw: &str, config: &PluginConfig) -> Option<String> {
    let mut output = String::new();
    let mut rest = raw;

    loop {
        let Some(start_brace) = rest.find('{') else {
            break;
        };

        let (selector_raw, remainder) = rest.split_at(start_brace);
        let selector = selector_raw.trim();

        if remainder.len() <= 1 {
            break;
        }

        let mut depth = 1;
        let mut body_end = None;
        let body_source = &remainder[1..];

        for (index, ch) in body_source.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        body_end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }

        let Some(body_end) = body_end else {
            return None;
        };

        let block_body = &body_source[..body_end];
        let declarations = normalise_keyframe_declarations(block_body, config)?;

        if !declarations.is_empty() {
            let selector = normalise_keyframe_selector(selector);
            output.push_str(&selector);
            output.push('{');
            output.push_str(&declarations);
            output.push('}');
        }

        rest = &body_source[body_end + 1..];
    }

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn normalise_keyframe_selector(selector: &str) -> String {
    let collapsed = collapse_whitespace(selector);
    let mut parts = Vec::new();

    for part in collapsed.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.eq_ignore_ascii_case("from") {
            parts.push(String::from("0%"));
        } else if trimmed.eq_ignore_ascii_case("to") {
            parts.push(String::from("to"));
        } else {
            parts.push(trimmed.to_string());
        }
    }

    parts.join(",")
}

fn normalise_keyframe_declarations(body: &str, config: &PluginConfig) -> Option<String> {
    let mut declarations = Vec::new();

    for declaration in body.split(';') {
        let trimmed = declaration.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((prop_raw, value_raw)) = trimmed.split_once(':') else {
            continue;
        };

        let property = prop_raw.trim();
        let value = value_raw.trim();

        if property.is_empty() || value.is_empty() {
            continue;
        }

        if let Some((name, value)) =
            normalise_property_pair(property, CssValue::String(value.to_string()), config)
        {
            declarations.push(format!("{name}:{value}"));
        }
    }

    if declarations.is_empty() {
        None
    } else {
        Some(declarations.join(";"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> PluginConfig {
        PluginConfig::default()
    }

    #[test]
    fn normalises_simple_keyframes_body() {
        let body = normalise_keyframes_body(
            "\n  from { opacity: 0; }\n  to { opacity: 1; }\n",
            &config(),
        )
        .expect("normalise body");

        assert_eq!(body, "0%{opacity:0}to{opacity:1}");
    }
}
