use crate::css_map::{visit_css_map_path_with_builder, CssMapUsage};
use crate::types::{Metadata, MetadataContext};
use crate::utils_ast::build_code_frame_error;
use crate::utils_css::kebab_case;
use crate::utils_evaluate_expression::evaluate_expression;
use crate::utils_hash::hash;
use crate::utils_is_compiled::{
    is_compiled_css_call_expression, is_compiled_css_map_call_expression,
    is_compiled_css_tagged_template_expression,
};
use crate::utils_resolve_binding::resolve_binding;
use crate::utils_types::{
    BindingSource, ConditionalCssItem, CssItem, CssMapItem, CssOutput, LogicalCssItem,
    LogicalOperator, PartialBindingWithMeta, SheetCssItem, UnconditionalCssItem, Variable,
};
use swc_core::common::sync::Lrc;
use swc_core::common::{SourceMap, Spanned, DUMMY_SP};
use swc_core::ecma::ast::{
    ArrayLit, ArrowExpr, BinExpr, BlockStmtOrExpr, CallExpr, Callee, CondExpr, Expr, ExprOrSpread,
    Ident, Lit, MemberExpr, TaggedTpl, UnaryExpr, UnaryOp,
};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::{Config, Emitter, Node};

fn print_expression(expr: &Expr) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buffer = Vec::new();

    {
        let mut writer = JsWriter::new(cm.clone(), "\n", &mut buffer, None);
        writer.set_indent_str("  ");
        let mut emitter = Emitter {
            cfg: Config::default(),
            comments: None,
            cm,
            wr: writer,
        };

        expr.emit_with(&mut emitter).expect("emit expression");
    }

    String::from_utf8(buffer).expect("expression to utf8 string")
}

fn call_arguments_as_array(call: &CallExpr) -> Expr {
    let elements = call
        .args
        .iter()
        .map(|arg| {
            if arg.spread.is_some() {
                panic!("Spread elements are not supported in keyframes arguments");
            }

            Some(ExprOrSpread {
                spread: None,
                expr: arg.expr.clone(),
            })
        })
        .collect();

    Expr::Array(ArrayLit {
        span: call.span,
        elems: elements,
    })
}

/// Merge consecutive unconditional CSS items while preserving the position of
/// any sheet entries. This mirrors the behaviour of the Babel helper and is
/// relied upon when normalising conditional CSS branches.
pub fn merge_subsequent_unconditional_css_items(items: Vec<CssItem>) -> Vec<CssItem> {
    let mut merged: Vec<CssItem> = Vec::new();
    let mut sheets: Vec<CssItem> = Vec::new();

    let mut index = 0usize;
    while index < items.len() {
        match &items[index] {
            CssItem::Sheet(_) => sheets.push(items[index].clone()),
            CssItem::Unconditional(_) => {
                let mut css = get_item_css(&items[index]);
                let mut last_index = index;

                let mut lookahead = index + 1;
                while lookahead < items.len() {
                    match &items[lookahead] {
                        CssItem::Unconditional(_) => {
                            css.push_str(&get_item_css(&items[lookahead]));
                            last_index = lookahead;
                        }
                        CssItem::Sheet(_) => sheets.push(items[lookahead].clone()),
                        _ => break,
                    }
                    lookahead += 1;
                }

                merged.push(CssItem::unconditional(css));
                index = last_index;
            }
            _ => merged.push(items[index].clone()),
        }

        index += 1;
    }

    sheets.into_iter().chain(merged.into_iter()).collect()
}

/// Helper that serialises a `CssItem` into the raw CSS string it represents.
/// This matches the behaviour of the Babel helper so downstream utilities can
/// reuse it during native transformations.
pub fn get_item_css(item: &CssItem) -> String {
    match item {
        CssItem::Conditional(conditional) => {
            let mut css = get_item_css(&conditional.consequent);
            css.push_str(&get_item_css(&conditional.alternate));
            css
        }
        CssItem::Unconditional(unconditional) => unconditional.css.clone(),
        CssItem::Logical(logical) => logical.css.clone(),
        CssItem::Sheet(sheet) => sheet.css.clone(),
        CssItem::Map(map) => map.css.clone(),
    }
}

/// Mirrors the Babel `generateCacheForCSSMap` helper by warming the cssMap cache for a given
/// identifier when possible. Returns `true` when the cache was populated.
pub fn generate_cache_for_css_map_with_builder<F>(
    identifier: &Ident,
    meta: &Metadata,
    build_css: &mut F,
) -> bool
where
    F: FnMut(&Expr, &Metadata) -> CssOutput,
{
    let name = identifier.sym.as_ref().to_string();

    {
        let state = meta.state();
        if state.css_map.contains_key(&name) || state.ignore_member_expressions.contains(&name) {
            return false;
        }
    }

    let resolved = resolve_binding(name.as_str(), meta.clone(), evaluate_expression);

    if let Some(PartialBindingWithMeta {
        node: Some(node),
        meta: binding_meta,
        ..
    }) = resolved
    {
        let is_css_map_call = {
            let state_ref = binding_meta.state();
            is_compiled_css_map_call_expression(&node, &state_ref)
        };

        if is_css_map_call {
            if let Expr::Call(call) = &node {
                visit_css_map_path_with_builder(
                    CssMapUsage::Call(call),
                    Some(identifier),
                    &binding_meta,
                    |expr, metadata| build_css(expr, metadata),
                );

                let has_cache = meta.state().css_map.contains_key(&name);
                if !has_cache {
                    meta.state_mut()
                        .ignore_member_expressions
                        .insert(name.clone());
                }

                return has_cache;
            }
        }
    }

    meta.state_mut().ignore_member_expressions.insert(name);
    false
}

fn find_binding_identifier(expr: &Expr) -> Option<Ident> {
    match expr {
        Expr::Ident(ident) => Some(ident.clone()),
        Expr::Call(call) => match &call.callee {
            Callee::Expr(callee) => find_binding_identifier(callee),
            _ => None,
        },
        Expr::Member(member) => find_binding_identifier(member.obj.as_ref()),
        _ => None,
    }
}

fn callback_if_file_included(meta: &Metadata, next: &Metadata) {
    let should_include = {
        let current_filename = meta.state().filename.clone();
        let next_filename = next.state().filename.clone();
        current_filename != next_filename
    };

    if should_include {
        if let Some(location) = next.state().file().loc.as_ref() {
            meta.state_mut()
                .included_files
                .push(location.filename.clone());
        }
    }
}

fn assert_no_imported_css_variables(
    reference: &Expr,
    meta: &Metadata,
    binding: &PartialBindingWithMeta,
    result: &CssOutput,
) {
    if binding.source == BindingSource::Import && !result.variables.is_empty() {
        let error = build_code_frame_error(
            "Identifier contains values that can't be statically evaluated",
            Some(reference.span()),
            meta,
        );
        panic!("{error}");
    }
}

enum ConditionalBranch {
    Consequent,
    Alternate,
}

fn logical_items_from_conditional_expression(
    css: Vec<CssItem>,
    node: &CondExpr,
    branch: ConditionalBranch,
) -> Vec<CssItem> {
    css.into_iter()
        .map(|item| match item {
            CssItem::Conditional(_) => item,
            CssItem::Logical(logical) => {
                let mut span = logical.expression.span();
                if span == DUMMY_SP {
                    span = node.test.span();
                }

                let expression = Expr::Bin(BinExpr {
                    span,
                    op: logical.operator.to_binary_op(),
                    left: Box::new((*node.test).clone()),
                    right: Box::new(logical.expression.clone()),
                });

                CssItem::Logical(LogicalCssItem {
                    expression,
                    operator: logical.operator,
                    css: logical.css,
                })
            }
            _ => {
                let expression = match branch {
                    ConditionalBranch::Consequent => (*node.test).clone(),
                    ConditionalBranch::Alternate => Expr::Unary(UnaryExpr {
                        span: node.test.span(),
                        op: UnaryOp::Bang,
                        arg: Box::new((*node.test).clone()),
                    }),
                };

                CssItem::Logical(LogicalCssItem {
                    expression,
                    operator: LogicalOperator::And,
                    css: get_item_css(&item),
                })
            }
        })
        .collect()
}

pub fn extract_member_expression_with_builder<F>(
    member: &MemberExpr,
    meta: &Metadata,
    fallback_to_evaluate: bool,
    build_css: &mut F,
) -> Option<CssOutput>
where
    F: FnMut(&Expr, &Metadata) -> CssOutput,
{
    if let Some(identifier) = find_binding_identifier(&Expr::Member(member.clone())) {
        let _ = generate_cache_for_css_map_with_builder(&identifier, meta, build_css);

        let has_cache = {
            let state = meta.state();
            state.css_map.contains_key(identifier.sym.as_ref())
        };

        if has_cache {
            let name = identifier.sym.as_ref().to_string();
            return Some(CssOutput {
                css: vec![CssItem::Map(CssMapItem {
                    name,
                    expression: Expr::Member(member.clone()),
                    css: String::new(),
                })],
                variables: Vec::new(),
            });
        }
    }

    if fallback_to_evaluate {
        let pair = evaluate_expression(&Expr::Member(member.clone()), meta.clone());
        return Some(build_css(&pair.value, &pair.meta));
    }

    None
}

pub fn extract_logical_expression_with_builder<F>(
    arrow: &ArrowExpr,
    meta: &Metadata,
    build_css: &mut F,
) -> CssOutput
where
    F: FnMut(&Expr, &Metadata) -> CssOutput,
{
    let mut css: Vec<CssItem> = Vec::new();
    let mut variables: Vec<Variable> = Vec::new();

    if let BlockStmtOrExpr::Expr(body_expr) = arrow.body.as_ref() {
        let pair = evaluate_expression(body_expr, meta.clone());
        let result = build_css(&pair.value, &pair.meta);

        callback_if_file_included(meta, &pair.meta);

        css.extend(result.css);
        variables.extend(result.variables);
    }

    CssOutput {
        css: merge_subsequent_unconditional_css_items(css),
        variables,
    }
}

pub fn extract_conditional_expression_with_builder<F>(
    node: &CondExpr,
    meta: &Metadata,
    build_css: &mut F,
) -> CssOutput
where
    F: FnMut(&Expr, &Metadata) -> CssOutput,
{
    let mut css: Vec<CssItem> = Vec::new();
    let mut variables: Vec<Variable> = Vec::new();

    let process_branch = |expr: &Expr,
                          meta: &Metadata,
                          build_css: &mut F,
                          variables: &mut Vec<Variable>|
     -> Option<CssItem> {
        let mut css_output: Option<CssOutput> = None;

        let looks_like_css_literal = if matches!(expr, Expr::Object(_)) {
            true
        } else if let Expr::Lit(Lit::Str(str_lit)) = expr {
            str_lit.value.contains(':')
        } else if let Expr::Tpl(tpl) = expr {
            tpl.quasis
                .iter()
                .any(|quasi| quasi.raw.as_ref().contains(':'))
        } else {
            false
        };

        if looks_like_css_literal {
            css_output = Some(build_css(expr, meta));
        } else {
            let is_compiled_css = {
                let state = meta.state();
                let compiled = is_compiled_css_tagged_template_expression(expr, &state)
                    || is_compiled_css_call_expression(expr, &state);
                compiled
            };

            if is_compiled_css {
                css_output = Some(build_css(expr, meta));
            } else if let Expr::Ident(identifier) = expr {
                if let Some(binding) =
                    resolve_binding(identifier.sym.as_ref(), meta.clone(), evaluate_expression)
                {
                    if let Some(node) = binding.node.clone() {
                        let compiled = {
                            let state = binding.meta.state();
                            let compiled =
                                is_compiled_css_tagged_template_expression(&node, &state)
                                    || is_compiled_css_call_expression(&node, &state);
                            compiled
                        };

                        if compiled {
                            let result = build_css(&node, &binding.meta);
                            assert_no_imported_css_variables(expr, meta, &binding, &result);
                            css_output = Some(result);
                        }
                    }
                }
            } else if let Expr::Cond(inner_conditional) = expr {
                css_output = Some(extract_conditional_expression_with_builder(
                    inner_conditional,
                    meta,
                    build_css,
                ));
            } else if let Expr::Member(member_expr) = expr {
                css_output =
                    extract_member_expression_with_builder(member_expr, meta, false, build_css);
            }
        }

        if let Some(mut output) = css_output {
            variables.append(&mut output.variables);
            let merged = merge_subsequent_unconditional_css_items(output.css);

            if merged.len() > 1 {
                let error = build_code_frame_error(
                    "Conditional branch contains unexpected expression",
                    Some(expr.span()),
                    meta,
                );
                panic!("{error}");
            }

            return merged.into_iter().next();
        }

        None
    };

    let consequent_css = process_branch(&node.cons, meta, build_css, &mut variables);
    let alternate_css = process_branch(&node.alt, meta, build_css, &mut variables);

    match (consequent_css, alternate_css) {
        (Some(consequent), Some(alternate)) => {
            css.push(CssItem::Conditional(ConditionalCssItem {
                test: (*node.test).clone(),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            }));
        }
        (Some(consequent), None) => css.extend(logical_items_from_conditional_expression(
            vec![consequent],
            node,
            ConditionalBranch::Consequent,
        )),
        (None, Some(alternate)) => css.extend(logical_items_from_conditional_expression(
            vec![alternate],
            node,
            ConditionalBranch::Alternate,
        )),
        (None, None) => {}
    }

    CssOutput { css, variables }
}

/// Extracts CSS rules from a keyframes expression while reusing the provided builder
/// for nested evaluation. Mirrors the Babel `extractKeyframes` helper by hashing the
/// expression source to produce a deterministic animation name and wrapping the
/// generated output in an `@keyframes` rule.
pub fn extract_keyframes_with_builder<F>(
    expression: &Expr,
    meta: &Metadata,
    prefix: &str,
    suffix: &str,
    build_css: &mut F,
) -> CssOutput
where
    F: FnMut(&Expr, &Metadata) -> CssOutput,
{
    let code = print_expression(expression);
    let name = format!("k{}", hash(&code));
    let selector = format!("@keyframes {name}");

    let keyframe_meta = meta.with_context(MetadataContext::Keyframes {
        keyframe: name.clone(),
    });

    let inner_output = match expression {
        Expr::Call(call) => {
            let array_expr = call_arguments_as_array(call);
            build_css(&array_expr, &keyframe_meta)
        }
        Expr::TaggedTpl(TaggedTpl { tpl, .. }) => {
            build_css(&Expr::Tpl((**tpl).clone()), &keyframe_meta)
        }
        Expr::Tpl(tpl) => build_css(&Expr::Tpl(tpl.clone()), &keyframe_meta),
        _ => build_css(expression, &keyframe_meta),
    };

    let wrapped = to_css_rule(&selector, &inner_output);

    if wrapped
        .css
        .iter()
        .any(|item| !matches!(item, CssItem::Unconditional(_)))
    {
        let error = build_code_frame_error(
            "Keyframes contains unexpected CSS",
            Some(expression.span()),
            meta,
        );
        panic!("{error}");
    }

    let sheet_css = wrapped
        .css
        .iter()
        .map(|item| get_item_css(item))
        .collect::<String>();

    CssOutput {
        css: vec![
            CssItem::Sheet(SheetCssItem { css: sheet_css }),
            CssItem::Unconditional(UnconditionalCssItem {
                css: format!("{prefix}{name}{suffix}"),
            }),
        ],
        variables: wrapped.variables,
    }
}

fn wrap_with_selector(selector: &str, css: String) -> String {
    format!("{selector} {{ {css} }}")
}

fn to_css_rule_internal(selector: &str, item: &CssItem) -> CssItem {
    match item {
        CssItem::Conditional(conditional) => CssItem::Conditional(ConditionalCssItem {
            test: conditional.test.clone(),
            consequent: Box::new(to_css_rule_internal(selector, &conditional.consequent)),
            alternate: Box::new(to_css_rule_internal(selector, &conditional.alternate)),
        }),
        CssItem::Unconditional(unconditional) => CssItem::Unconditional(UnconditionalCssItem {
            css: wrap_with_selector(selector, unconditional.css.clone()),
        }),
        CssItem::Logical(logical) => CssItem::Logical(LogicalCssItem {
            expression: logical.expression.clone(),
            operator: logical.operator,
            css: wrap_with_selector(selector, logical.css.clone()),
        }),
        CssItem::Sheet(sheet) => CssItem::Sheet(SheetCssItem {
            css: wrap_with_selector(selector, sheet.css.clone()),
        }),
        CssItem::Map(map) => CssItem::Map(CssMapItem {
            name: map.name.clone(),
            expression: map.expression.clone(),
            css: wrap_with_selector(selector, map.css.clone()),
        }),
    }
}

/// Map the CSS output to rule blocks for the provided selector. Mirrors the
/// behaviour of the Babel helper which recursively wraps each item while
/// preserving conditional branches.
pub fn to_css_rule(selector: &str, result: &CssOutput) -> CssOutput {
    let css = result
        .css
        .iter()
        .map(|item| to_css_rule_internal(selector, item))
        .collect();

    CssOutput {
        css,
        variables: result.variables.clone(),
    }
}

fn declaration_css(key: &str, css: String) -> String {
    format!("{}: {};", kebab_case(key), css)
}

fn to_css_declaration_internal(key: &str, item: &CssItem) -> CssItem {
    match item {
        CssItem::Sheet(sheet) => CssItem::Sheet(sheet.clone()),
        CssItem::Conditional(conditional) => CssItem::Conditional(ConditionalCssItem {
            test: conditional.test.clone(),
            consequent: Box::new(to_css_declaration_internal(key, &conditional.consequent)),
            alternate: Box::new(to_css_declaration_internal(key, &conditional.alternate)),
        }),
        CssItem::Unconditional(unconditional) => CssItem::Unconditional(UnconditionalCssItem {
            css: declaration_css(key, unconditional.css.clone()),
        }),
        CssItem::Logical(logical) => CssItem::Logical(LogicalCssItem {
            expression: logical.expression.clone(),
            operator: logical.operator,
            css: declaration_css(key, logical.css.clone()),
        }),
        CssItem::Map(map) => CssItem::Map(CssMapItem {
            name: map.name.clone(),
            expression: map.expression.clone(),
            css: declaration_css(key, map.css.clone()),
        }),
    }
}

/// Convert the CSS output into property declarations for the provided key,
/// mirroring the Babel helper that kebab-cases the property name and reuses the
/// existing item shape.
pub fn to_css_declaration(key: &str, result: &CssOutput) -> CssOutput {
    let css = result
        .css
        .iter()
        .map(|item| to_css_declaration_internal(key, item))
        .collect();

    CssOutput {
        css,
        variables: result.variables.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assert_no_imported_css_variables, callback_if_file_included,
        extract_conditional_expression_with_builder, extract_keyframes_with_builder,
        extract_logical_expression_with_builder, extract_member_expression_with_builder,
        find_binding_identifier, generate_cache_for_css_map_with_builder, get_item_css,
        merge_subsequent_unconditional_css_items, to_css_declaration, to_css_rule,
    };
    use crate::types::{
        CompiledImports, Metadata, MetadataContext, PluginOptions, TransformFile,
        TransformFileOptions, TransformState,
    };
    use crate::utils_types::{
        BindingSource, ConditionalCssItem, CssItem, CssOutput, LogicalCssItem, LogicalOperator,
        PartialBindingWithMeta, SheetCssItem, UnconditionalCssItem, Variable,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use swc_core::common::sync::Lrc;
    use swc_core::common::{FileName, SourceMap, SyntaxContext, DUMMY_SP};
    use swc_core::ecma::ast::{
        CallExpr, Callee, Expr, ExprOrSpread, Ident, KeyValueProp, ObjectLit, Prop, PropName,
        PropOrSpread,
    };
    use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax};

    fn ident_expr(name: &str) -> Expr {
        Expr::Ident(Ident::new(name.into(), DUMMY_SP, SyntaxContext::empty()))
    }

    fn create_metadata() -> Metadata {
        let cm: Lrc<SourceMap> = Default::default();
        cm.new_source_file(FileName::Custom("test.js".into()).into(), String::new());
        let file = TransformFile::with_options(
            cm.clone(),
            Vec::new(),
            TransformFileOptions {
                filename: Some("test.js".into()),
                loc_filename: Some("test.js".into()),
                ..TransformFileOptions::default()
            },
        );
        let state = Rc::new(RefCell::new(TransformState::new(
            file,
            PluginOptions::default(),
        )));

        Metadata::new(state)
    }

    fn create_metadata_with_filename(filename: &str) -> Metadata {
        let cm: Lrc<SourceMap> = Default::default();
        cm.new_source_file(FileName::Custom(filename.into()).into(), String::new());
        let file = TransformFile::with_options(
            cm,
            Vec::new(),
            TransformFileOptions {
                filename: Some(filename.into()),
                loc_filename: Some(filename.into()),
                ..TransformFileOptions::default()
            },
        );
        let state = Rc::new(RefCell::new(TransformState::new(
            file,
            PluginOptions::default(),
        )));

        Metadata::new(state)
    }

    fn parse_expression(code: &str) -> Expr {
        let cm: Lrc<SourceMap> = Default::default();
        parse_expression_with_source_map(&cm, code)
    }

    fn parse_expression_with_source_map(cm: &Lrc<SourceMap>, code: &str) -> Expr {
        let source_file =
            cm.new_source_file(FileName::Custom("test.js".into()).into(), code.into());
        let lexer = Lexer::new(
            Syntax::Es(Default::default()),
            Default::default(),
            StringInput::from(&*source_file),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        *parser.parse_expr().expect("parse expression")
    }

    fn assert_keyframe_sheet(
        css_output: &CssOutput,
        expected_name: &str,
        prefix: &str,
        suffix: &str,
    ) {
        assert_eq!(css_output.variables.len(), 0);
        assert_eq!(css_output.css.len(), 2);

        match &css_output.css[0] {
            CssItem::Sheet(SheetCssItem { css }) => {
                let normalized: String = css.chars().filter(|ch| !ch.is_whitespace()).collect();
                let expected =
                    format!("@keyframes{expected_name}{{0%{{opacity:1}}to{{opacity:0}}}}");
                assert_eq!(normalized, expected);
            }
            other => panic!("expected sheet css, found {other:?}"),
        }

        match &css_output.css[1] {
            CssItem::Unconditional(UnconditionalCssItem { css }) => {
                assert_eq!(css, &format!("{prefix}{expected_name}{suffix}"));
            }
            other => panic!("expected unconditional css, found {other:?}"),
        }
    }

    fn css_map_call() -> Expr {
        let selector_ident = Ident::new("primary".into(), DUMMY_SP, SyntaxContext::empty());
        let variant = ObjectLit {
            span: DUMMY_SP,
            props: Vec::new(),
        };

        let argument = ObjectLit {
            span: DUMMY_SP,
            props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                key: PropName::Ident(selector_ident.into()),
                value: Box::new(Expr::Object(variant)),
            })))],
        };

        Expr::Call(CallExpr {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(
                "cssMap".into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            )))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Object(argument)),
            }],
            type_args: None,
        })
    }

    #[test]
    fn extract_keyframes_from_call_expression() {
        let metadata = create_metadata();
        let expr = parse_expression("keyframes({ from: { opacity: 1 }, to: { opacity: 0 } })");
        let mut build_css = |value: &Expr, meta: &Metadata| {
            match &meta.context {
                MetadataContext::Keyframes { keyframe } => {
                    assert_eq!(keyframe.len(), 8);
                }
                other => panic!("expected keyframes context, found {other:?}"),
            }

            match value {
                Expr::Array(array) => {
                    assert_eq!(array.elems.len(), 1);
                    CssOutput {
                        css: vec![CssItem::unconditional("0%{opacity:1}to{opacity:0}")],
                        variables: Vec::new(),
                    }
                }
                other => panic!("unexpected expression {other:?}"),
            }
        };

        let output =
            extract_keyframes_with_builder(&expr, &metadata, "animation: ", ";", &mut build_css);

        assert_keyframe_sheet(&output, "k1m8j3od", "animation: ", ";");
    }

    #[test]
    fn extract_keyframes_from_tagged_template() {
        let metadata = create_metadata();
        let expr = parse_expression("keyframes`from { opacity: 1; } to { opacity: 0; }`");
        let mut build_css = |value: &Expr, meta: &Metadata| {
            match &meta.context {
                MetadataContext::Keyframes { keyframe } => {
                    assert_eq!(keyframe.len(), 7);
                }
                other => panic!("expected keyframes context, found {other:?}"),
            }

            match value {
                Expr::Tpl(_) => CssOutput {
                    css: vec![CssItem::unconditional("0%{opacity:1}to{opacity:0}")],
                    variables: Vec::new(),
                },
                other => panic!("unexpected expression {other:?}"),
            }
        };

        let output =
            extract_keyframes_with_builder(&expr, &metadata, "animation: ", ";", &mut build_css);

        assert_keyframe_sheet(&output, "kqbs1so", "animation: ", ";");
    }

    #[test]
    fn merges_adjacent_unconditional_items() {
        let items = vec![
            CssItem::unconditional("color: red;"),
            CssItem::unconditional("background: blue;"),
            CssItem::Logical(LogicalCssItem {
                css: "display: none;".into(),
                expression: ident_expr("flag"),
                operator: LogicalOperator::And,
            }),
            CssItem::unconditional("border: 0;"),
        ];

        let merged = merge_subsequent_unconditional_css_items(items);
        assert_eq!(merged.len(), 3);
        assert_eq!(get_item_css(&merged[0]), "color: red;background: blue;");
        assert!(matches!(merged[1], CssItem::Logical(_)));
        assert_eq!(get_item_css(&merged[2]), "border: 0;");
    }

    #[test]
    fn preserves_sheets_when_merging_unconditionals() {
        let items = vec![
            CssItem::Sheet(SheetCssItem {
                css: ".a{color:red;}".into(),
            }),
            CssItem::unconditional("margin: 0;"),
            CssItem::unconditional("padding: 0;"),
        ];

        let merged = merge_subsequent_unconditional_css_items(items);
        assert_eq!(merged.len(), 2);
        assert!(matches!(merged[0], CssItem::Sheet(_)));
        assert_eq!(get_item_css(&merged[1]), "margin: 0;padding: 0;");
    }

    #[test]
    fn get_item_css_serialises_variants() {
        let unconditional = CssItem::unconditional("color: red;");
        assert_eq!(get_item_css(&unconditional), "color: red;");

        let logical = CssItem::Logical(LogicalCssItem {
            css: "display: none;".into(),
            expression: ident_expr("flag"),
            operator: LogicalOperator::And,
        });
        assert_eq!(get_item_css(&logical), "display: none;");
    }

    #[test]
    fn to_css_rule_wraps_items_in_selector() {
        let item = CssItem::unconditional("color: red;");
        let output = CssOutput {
            css: vec![item],
            variables: Vec::new(),
        };

        let result = to_css_rule(".foo", &output);
        assert_eq!(result.css.len(), 1);

        match &result.css[0] {
            CssItem::Unconditional(unconditional) => {
                assert_eq!(unconditional.css, ".foo { color: red; }");
            }
            _ => panic!("expected unconditional item"),
        }
    }

    #[test]
    fn to_css_rule_recursively_wraps_conditionals() {
        let conditional = CssItem::Conditional(ConditionalCssItem {
            test: Expr::Ident(Ident::new("flag".into(), DUMMY_SP, SyntaxContext::empty())),
            consequent: Box::new(CssItem::unconditional("color: red;")),
            alternate: Box::new(CssItem::unconditional("color: blue;")),
        });

        let output = CssOutput {
            css: vec![conditional],
            variables: Vec::new(),
        };

        let result = to_css_rule(".foo", &output);
        match &result.css[0] {
            CssItem::Conditional(mapped) => {
                if let CssItem::Unconditional(unconditional) = mapped.consequent.as_ref() {
                    assert_eq!(unconditional.css, ".foo { color: red; }");
                } else {
                    panic!("expected unconditional consequent");
                }

                if let CssItem::Unconditional(unconditional) = mapped.alternate.as_ref() {
                    assert_eq!(unconditional.css, ".foo { color: blue; }");
                } else {
                    panic!("expected unconditional alternate");
                }
            }
            _ => panic!("expected conditional item"),
        }
    }

    #[test]
    fn to_css_declaration_maps_values() {
        let item = CssItem::Logical(LogicalCssItem {
            css: "var(--token)".into(),
            expression: ident_expr("flag"),
            operator: LogicalOperator::And,
        });

        let output = CssOutput {
            css: vec![item],
            variables: Vec::new(),
        };

        let result = to_css_declaration("fontWeight", &output);
        match &result.css[0] {
            CssItem::Logical(logical) => {
                assert_eq!(logical.css, "font-weight: var(--token);");
            }
            _ => panic!("expected logical item"),
        }
    }

    #[test]
    fn finds_binding_identifier_on_nested_member() {
        let expr = parse_expression("theme.colors.primary");
        if let Expr::Member(member) = expr {
            let ident = find_binding_identifier(&Expr::Member(member)).expect("identifier");
            assert_eq!(ident.sym.as_ref(), "theme");
        } else {
            panic!("expected member expression");
        }
    }

    #[test]
    fn callback_if_file_included_tracks_imports() {
        let meta = create_metadata_with_filename("root.tsx");
        let next = create_metadata_with_filename("imported.tsx");

        callback_if_file_included(&meta, &next);

        let state = meta.state();
        assert_eq!(state.included_files, vec!["imported.tsx".to_string()]);
    }

    #[test]
    #[should_panic(expected = "Identifier contains values that can't be statically evaluated")]
    fn assert_no_imported_css_variables_panics_for_imports() {
        let meta = create_metadata();
        let binding_meta = create_metadata();
        let binding = PartialBindingWithMeta::new(
            None,
            None,
            true,
            binding_meta.clone(),
            BindingSource::Import,
        );

        let css_output = CssOutput {
            css: Vec::new(),
            variables: vec![Variable {
                name: "--token".into(),
                expression: ident_expr("value"),
                prefix: None,
                suffix: None,
            }],
        };

        let source_map = {
            let state = meta.state();
            state.file().source_map.clone()
        };
        let reference = parse_expression_with_source_map(&source_map, "styles");
        assert!(matches!(reference, Expr::Ident(_)));
        assert_no_imported_css_variables(&reference, &meta, &binding, &css_output);
    }

    #[test]
    fn assert_no_imported_css_variables_allows_local_bindings() {
        let meta = create_metadata();
        let binding_meta = create_metadata();
        let binding = PartialBindingWithMeta::new(
            None,
            None,
            true,
            binding_meta.clone(),
            BindingSource::Module,
        );

        let css_output = CssOutput::new();
        let reference = ident_expr("styles");
        assert_no_imported_css_variables(&reference, &meta, &binding, &css_output);
    }

    #[test]
    fn extract_member_expression_returns_map_item() {
        let meta = create_metadata();
        {
            let mut state = meta.state_mut();
            state
                .css_map
                .insert("styles".into(), vec![".a{color:red;}".into()]);
        }

        let expr = parse_expression("styles.primary");
        if let Expr::Member(member) = expr {
            let output = extract_member_expression_with_builder(
                &member,
                &meta,
                false,
                &mut |_expr, _meta| CssOutput::new(),
            )
            .expect("map output");

            assert_eq!(output.css.len(), 1);
            match &output.css[0] {
                CssItem::Map(map) => assert_eq!(map.name, "styles"),
                _ => panic!("expected map item"),
            }
        } else {
            panic!("expected member expression");
        }
    }

    #[test]
    fn extract_member_expression_falls_back_to_evaluation() {
        let meta = create_metadata();
        let binding_meta = create_metadata();
        let binding = PartialBindingWithMeta::new(
            Some(parse_expression("({ primary: { color: 'red' } })")),
            None,
            true,
            binding_meta.clone(),
            BindingSource::Module,
        );
        meta.insert_parent_binding("theme", binding);

        let expr = parse_expression("theme.primary");
        let mut invoked = false;

        if let Expr::Member(member) = expr {
            let output =
                extract_member_expression_with_builder(&member, &meta, true, &mut |expr, _meta| {
                    invoked = true;
                    assert!(matches!(expr, Expr::Object(_)));
                    CssOutput {
                        css: vec![CssItem::unconditional("color: red;")],
                        variables: Vec::new(),
                    }
                })
                .expect("css output");

            assert!(invoked);
            assert_eq!(output.css.len(), 1);
        } else {
            panic!("expected member expression");
        }
    }

    #[test]
    fn extract_logical_expression_evaluates_body() {
        let meta = create_metadata();
        let expr = parse_expression("() => ({ color: 'red' })");
        let mut invoked = false;

        if let Expr::Arrow(arrow) = expr {
            let result =
                extract_logical_expression_with_builder(&arrow, &meta, &mut |expr, _meta| {
                    invoked = true;
                    assert!(matches!(expr, Expr::Object(_)));
                    CssOutput {
                        css: vec![CssItem::unconditional("color: red;")],
                        variables: Vec::new(),
                    }
                });

            assert!(invoked);
            assert_eq!(result.css.len(), 1);
            assert_eq!(get_item_css(&result.css[0]), "color: red;");
        } else {
            panic!("expected arrow expression");
        }
    }

    #[test]
    fn extract_conditional_expression_handles_both_branches() {
        let meta = create_metadata();
        let expr = parse_expression("flag ? { color: 'red' } : { color: 'blue' }");
        let mut call_count = 0;

        if let Expr::Cond(cond) = expr {
            let result = extract_conditional_expression_with_builder(&cond, &meta, &mut |_, _| {
                call_count += 1;
                let css = if call_count == 1 {
                    "color: red;"
                } else {
                    "color: blue;"
                };
                CssOutput {
                    css: vec![CssItem::unconditional(css)],
                    variables: Vec::new(),
                }
            });

            assert_eq!(call_count, 2);
            assert_eq!(result.css.len(), 1);
            match &result.css[0] {
                CssItem::Conditional(conditional) => {
                    assert!(matches!(
                        *conditional.consequent.clone(),
                        CssItem::Unconditional(_)
                    ));
                    assert!(matches!(
                        *conditional.alternate.clone(),
                        CssItem::Unconditional(_)
                    ));
                }
                _ => panic!("expected conditional css item"),
            }
        } else {
            panic!("expected conditional expression");
        }
    }

    #[test]
    fn extract_conditional_expression_converts_single_branch_to_logical() {
        let meta = create_metadata();
        let expr = parse_expression("flag ? { color: 'red' } : value");
        let mut invoked = false;

        if let Expr::Cond(cond) = expr {
            let result = extract_conditional_expression_with_builder(&cond, &meta, &mut |_, _| {
                if !invoked {
                    invoked = true;
                    CssOutput {
                        css: vec![CssItem::unconditional("color: red;")],
                        variables: Vec::new(),
                    }
                } else {
                    CssOutput::new()
                }
            });

            assert!(invoked);
            assert_eq!(result.css.len(), 1);
            match &result.css[0] {
                CssItem::Logical(logical) => {
                    assert_eq!(logical.css, "color: red;");
                    assert_eq!(logical.operator, LogicalOperator::And);
                }
                _ => panic!("expected logical css item"),
            }
        } else {
            panic!("expected conditional expression");
        }
    }

    #[test]
    fn to_css_declaration_preserves_sheets() {
        let sheet_css = ".a { color: red; }".to_string();
        let sheet = CssItem::Sheet(SheetCssItem {
            css: sheet_css.clone(),
        });

        let output = CssOutput {
            css: vec![sheet],
            variables: Vec::new(),
        };

        let result = to_css_declaration("color", &output);
        assert_eq!(result.css.len(), 1);

        match &result.css[0] {
            CssItem::Sheet(mapped) => assert_eq!(mapped.css, sheet_css),
            _ => panic!("expected sheet item"),
        }
    }

    #[test]
    fn generate_cache_populates_css_map() {
        let metadata = create_metadata();
        {
            let mut state = metadata.state_mut();
            state.compiled_imports = Some(CompiledImports {
                css_map: vec!["cssMap".into()],
                ..CompiledImports::default()
            });
        }

        let binding_meta = metadata.clone();
        let css_map_expr = css_map_call();
        let binding = PartialBindingWithMeta::new(
            Some(css_map_expr.clone()),
            None,
            true,
            binding_meta.clone(),
            BindingSource::Module,
        );
        binding_meta.insert_parent_binding("styles", binding);

        let ident = Ident::new("styles".into(), DUMMY_SP, SyntaxContext::empty());
        let mut calls = 0usize;
        let mut build_css = |expr: &Expr, _meta: &Metadata| {
            calls += 1;
            assert!(matches!(expr, Expr::Object(_)));
            CssOutput {
                css: vec![CssItem::Sheet(SheetCssItem {
                    css: ".a{color:red;}".into(),
                })],
                variables: Vec::new(),
            }
        };

        let populated = generate_cache_for_css_map_with_builder(&ident, &metadata, &mut build_css);

        assert!(populated);
        assert_eq!(calls, 1);

        let state = metadata.state();
        let sheets = state.css_map.get("styles").expect("cache entry");
        assert_eq!(sheets.len(), 1);
        assert!(state.ignore_member_expressions.is_empty());
    }

    #[test]
    fn generate_cache_marks_identifier_when_binding_missing() {
        let metadata = create_metadata();
        let ident = Ident::new("styles".into(), DUMMY_SP, SyntaxContext::empty());
        let mut build_css = |_expr: &Expr, _meta: &Metadata| CssOutput::new();

        let populated = generate_cache_for_css_map_with_builder(&ident, &metadata, &mut build_css);

        assert!(!populated);

        let state = metadata.state();
        assert!(state.ignore_member_expressions.contains("styles"));
        assert!(state.css_map.is_empty());
    }
}
