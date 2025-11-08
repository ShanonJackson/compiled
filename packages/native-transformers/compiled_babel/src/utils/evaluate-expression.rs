use std::borrow::Cow;

use swc_core::common::{Span, Spanned, SyntaxContext};
use swc_core::ecma::ast::{BinExpr, Expr, Lit, Number, Str};
use swc_core::ecma::utils::{ExprCtx, ExprExt, Value};
use swc_core::ecma::visit::{noop_visit_type, Visit, VisitWith};

use crate::types::Metadata;
use crate::utils_create_result_pair::{create_result_pair, ResultPair};
use crate::utils_is_compiled::is_compiled_keyframes_call_expression;
use crate::utils_traverse_expression_traverse_binary_expression::traverse_binary_expression;
use crate::utils_traverse_expression_traverse_call_expression::traverse_call_expression;
use crate::utils_traverse_expression_traverse_function::traverse_function;
use crate::utils_traverse_expression_traverse_identifier::traverse_identifier;
use crate::utils_traverse_expression_traverse_member_expression::traverse_member_expression;
use crate::utils_traverse_expression_traverse_unary_expression::traverse_unary_expression;

fn skip_wrappers<'a>(expression: &'a Expr) -> &'a Expr {
    match expression {
        Expr::TsAs(ts_as) => skip_wrappers(&ts_as.expr),
        Expr::TsTypeAssertion(assertion) => skip_wrappers(&assertion.expr),
        Expr::TsConstAssertion(assertion) => skip_wrappers(&assertion.expr),
        Expr::TsNonNull(non_null) => skip_wrappers(&non_null.expr),
        Expr::Paren(paren) => skip_wrappers(&paren.expr),
        _ => expression,
    }
}

fn is_literal_value(expr: &Expr, meta: &Metadata) -> bool {
    match expr {
        Expr::Lit(Lit::Str(_)) | Expr::Lit(Lit::Num(_)) | Expr::Object(_) | Expr::TaggedTpl(_) => {
            true
        }
        Expr::Call(_) => {
            let state = meta.state();
            let is_keyframes = is_compiled_keyframes_call_expression(expr, &state);
            drop(state);
            is_keyframes
        }
        _ => false,
    }
}

fn binding_is_mutated(meta: &Metadata, name: &str) -> bool {
    if let Some(scope) = meta.own_scope() {
        if scope
            .borrow()
            .get(name)
            .map(|binding| !binding.constant)
            .unwrap_or(false)
        {
            return true;
        }
    }

    meta.parent_scope()
        .borrow()
        .get(name)
        .map(|binding| !binding.constant)
        .unwrap_or(false)
}

fn references_mutated_identifiers(expr: &Expr, meta: &Metadata) -> bool {
    struct MutatedIdentifierVisitor {
        meta: Metadata,
        mutated: bool,
    }

    impl Visit for MutatedIdentifierVisitor {
        noop_visit_type!();

        fn visit_ident(&mut self, ident: &swc_core::ecma::ast::Ident) {
            if self.mutated {
                return;
            }

            if binding_is_mutated(&self.meta, ident.sym.as_ref()) {
                self.mutated = true;
                return;
            }
        }
    }

    let mut visitor = MutatedIdentifierVisitor {
        meta: meta.clone(),
        mutated: false,
    };

    expr.visit_with(&mut visitor);

    visitor.mutated
}

fn make_string_literal(value: Cow<'_, str>, span: Span) -> Expr {
    Expr::Lit(Lit::Str(Str {
        span,
        value: value.into_owned().into(),
        raw: None,
    }))
}

fn make_numeric_literal(value: f64, span: Span) -> Expr {
    Expr::Lit(Lit::Num(Number {
        span,
        value,
        raw: None,
    }))
}

fn evaluate_binary_expression(bin: &BinExpr) -> Option<Expr> {
    use swc_core::ecma::ast::BinaryOp::*;

    let left = bin.left.as_ref();
    let right = bin.right.as_ref();

    let span = bin.span;

    match bin.op {
        Add => match (left, right) {
            (Expr::Lit(Lit::Str(a)), Expr::Lit(Lit::Str(b))) => {
                let mut combined = a.value.to_string();
                combined.push_str(b.value.as_ref());
                Some(make_string_literal(Cow::Owned(combined), span))
            }
            (Expr::Lit(Lit::Str(a)), Expr::Lit(Lit::Num(b))) => {
                let mut combined = a.value.to_string();
                combined.push_str(&b.value.to_string());
                Some(make_string_literal(Cow::Owned(combined), span))
            }
            (Expr::Lit(Lit::Num(a)), Expr::Lit(Lit::Str(b))) => {
                let mut combined = a.value.to_string();
                combined.push_str(b.value.as_ref());
                Some(make_string_literal(Cow::Owned(combined), span))
            }
            (Expr::Lit(Lit::Num(a)), Expr::Lit(Lit::Num(b))) => {
                Some(make_numeric_literal(a.value + b.value, span))
            }
            _ => None,
        },
        Sub | Mul | Div | Mod | Exp => {
            if let (Expr::Lit(Lit::Num(a)), Expr::Lit(Lit::Num(b))) = (left, right) {
                let result = match bin.op {
                    Sub => a.value - b.value,
                    Mul => a.value * b.value,
                    Div => a.value / b.value,
                    Mod => a.value % b.value,
                    Exp => a.value.powf(b.value),
                    _ => unreachable!("operator filtered in outer match"),
                };

                Some(make_numeric_literal(result, span))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn try_static_evaluate(expr: &Expr, meta: &Metadata) -> Option<Expr> {
    if references_mutated_identifiers(expr, meta) {
        return None;
    }

    if let Expr::Bin(bin) = expr {
        if let Some(evaluated) = evaluate_binary_expression(bin) {
            return Some(evaluated);
        }
    }

    let ctx = ExprCtx {
        unresolved_ctxt: SyntaxContext::empty(),
        is_unresolved_ref_safe: false,
        in_strict: false,
        remaining_depth: 8,
    };

    if !matches!(expr, Expr::Lit(Lit::Str(_))) {
        if let Value::Known(value) = expr.as_pure_number(ctx) {
            let allow_nan = matches!(expr, Expr::Lit(Lit::Num(_)) | Expr::Bin(_));

            if value.is_finite() || allow_nan {
                return Some(make_numeric_literal(value, expr.span()));
            }
        }
    }

    match expr.as_pure_string(ctx) {
        Value::Known(value) => Some(make_string_literal(value, expr.span())),
        Value::Unknown => None,
    }
}

/// Mirrors the Babel `evaluateExpression` helper by recursively resolving and
/// evaluating expressions into literal forms where possible while preserving the
/// metadata threaded through each traversal.
pub fn evaluate_expression(expression: &Expr, meta: Metadata) -> ResultPair {
    let mut updated_meta = meta;
    let target_expression = skip_wrappers(expression);
    let mut evaluated_value: Option<Expr> = None;

    match target_expression {
        Expr::Ident(ident) => {
            let pair = traverse_identifier(ident, updated_meta.clone(), evaluate_expression);
            evaluated_value = Some(pair.value);
            updated_meta = pair.meta;
        }
        Expr::Member(member) => {
            let pair =
                traverse_member_expression(member, updated_meta.clone(), evaluate_expression);
            evaluated_value = Some(pair.value);
            updated_meta = pair.meta;
        }
        Expr::Fn(_) | Expr::Arrow(_) => {
            let pair =
                traverse_function(target_expression, updated_meta.clone(), evaluate_expression);
            evaluated_value = Some(pair.value);
            updated_meta = pair.meta;
        }
        Expr::Call(call) => {
            let pair = traverse_call_expression(call, updated_meta.clone(), evaluate_expression);
            evaluated_value = Some(pair.value);
            updated_meta = pair.meta;
        }
        Expr::Bin(bin) => {
            let pair = traverse_binary_expression(bin, updated_meta.clone(), evaluate_expression);
            evaluated_value = Some(pair.value);
            updated_meta = pair.meta;
        }
        Expr::Unary(unary) => {
            let pair = traverse_unary_expression(unary, updated_meta.clone(), evaluate_expression);
            evaluated_value = Some(pair.value);
            updated_meta = pair.meta;
        }
        _ => {}
    }

    if let Some(value) = evaluated_value {
        if is_literal_value(&value, &updated_meta) {
            return create_result_pair(value, updated_meta);
        }

        if let Some(evaluated) = try_static_evaluate(&value, &updated_meta) {
            return create_result_pair(evaluated, updated_meta);
        }

        return create_result_pair(target_expression.clone(), updated_meta);
    }

    if let Some(evaluated) = try_static_evaluate(target_expression, &updated_meta) {
        return create_result_pair(evaluated, updated_meta);
    }

    create_result_pair(target_expression.clone(), updated_meta)
}

#[cfg(test)]
mod tests {
    use super::evaluate_expression;
    use crate::types::{CompiledImports, Metadata, PluginOptions, TransformFile, TransformState};
    use crate::utils_types::{BindingPath, BindingSource, PartialBindingWithMeta};
    use std::cell::RefCell;
    use std::rc::Rc;
    use swc_core::common::sync::Lrc;
    use swc_core::common::{FileName, SourceMap, SyntaxContext, DUMMY_SP};
    use swc_core::ecma::ast::{CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, Number, Str};
    use swc_ecma_parser::lexer::Lexer;
    use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax};

    fn parse_expression(code: &str) -> Expr {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom("expr.tsx".into()).into(), code.into());
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax {
                tsx: true,
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        *parser.parse_expr().expect("parse expression")
    }

    fn create_metadata() -> Metadata {
        let cm: Lrc<SourceMap> = Default::default();
        let file = TransformFile::new(cm, Vec::new());
        let state = Rc::new(RefCell::new(TransformState::new(
            file,
            PluginOptions::default(),
        )));

        Metadata::new(state)
    }

    fn string_literal(value: &str) -> Expr {
        Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: value.into(),
            raw: None,
        }))
    }

    fn numeric_literal(value: f64) -> Expr {
        Expr::Lit(Lit::Num(Number {
            span: DUMMY_SP,
            value,
            raw: None,
        }))
    }

    #[test]
    fn resolves_identifier_bindings() {
        let meta = create_metadata();
        let binding_meta = meta.clone();

        let binding = PartialBindingWithMeta::new(
            Some(string_literal("blue")),
            Some(BindingPath::new(Some(DUMMY_SP))),
            true,
            binding_meta,
            BindingSource::Module,
        );

        meta.insert_parent_binding("color", binding);

        let ident = Expr::Ident(Ident::new("color".into(), DUMMY_SP, SyntaxContext::empty()));
        let pair = evaluate_expression(&ident, meta);

        match pair.value {
            Expr::Lit(Lit::Str(Str { value, .. })) => assert_eq!(value.as_ref(), "blue"),
            other => panic!("expected string literal, found {:?}", other),
        }
    }

    #[test]
    fn statically_evaluates_binary_expressions() {
        let expr = parse_expression("1 + 2");
        let meta = create_metadata();

        let pair = evaluate_expression(&expr, meta);

        match pair.value {
            Expr::Lit(Lit::Num(Number { value, .. })) => assert_eq!(value, 3.0),
            other => panic!("expected numeric literal, found {:?}", other),
        }
    }

    #[test]
    fn returns_identifier_when_binding_not_constant() {
        let meta = create_metadata();
        let binding = PartialBindingWithMeta::new(
            Some(string_literal("value")),
            Some(BindingPath::new(Some(DUMMY_SP))),
            false,
            meta.clone(),
            BindingSource::Module,
        );

        meta.insert_parent_binding("color", binding);

        let ident = Expr::Ident(Ident::new("color".into(), DUMMY_SP, SyntaxContext::empty()));
        let pair = evaluate_expression(&ident, meta.clone());

        match pair.value {
            Expr::Ident(result) => assert_eq!(result.sym.as_ref(), "color"),
            other => panic!("expected identifier, found {:?}", other),
        }
    }

    #[test]
    fn statically_evaluates_string_concatenation() {
        let expr = parse_expression("'ab' + 'cd'");
        let meta = create_metadata();

        let pair = evaluate_expression(&expr, meta);

        match pair.value {
            Expr::Lit(Lit::Str(Str { value, .. })) => assert_eq!(value.as_ref(), "abcd"),
            other => panic!("expected string literal, found {:?}", other),
        }
    }

    #[test]
    fn preserves_keyframes_call_expressions() {
        let meta = create_metadata();
        {
            let mut state = meta.state_mut();
            state.compiled_imports = Some(CompiledImports {
                keyframes: vec!["keyframes".into()],
                ..CompiledImports::default()
            });
        }

        let call = Expr::Call(CallExpr {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(
                "keyframes".into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            )))),
            args: vec![],
            type_args: None,
        });

        let pair = evaluate_expression(&call, meta);

        match pair.value {
            Expr::Call(returned) => match returned.callee {
                Callee::Expr(expr) => match expr.as_ref() {
                    Expr::Ident(ident) => assert_eq!(ident.sym.as_ref(), "keyframes"),
                    other => panic!("expected identifier callee, found {:?}", other),
                },
                _ => panic!("expected callee expression"),
            },
            other => panic!("expected call expression, found {:?}", other),
        }
    }

    #[test]
    fn strips_type_assertions_before_evaluating() {
        let meta = create_metadata();
        let binding_meta = meta.clone();

        let binding = PartialBindingWithMeta::new(
            Some(string_literal("blue")),
            Some(BindingPath::new(Some(DUMMY_SP))),
            true,
            binding_meta,
            BindingSource::Module,
        );

        meta.insert_parent_binding("color", binding);

        let expr = parse_expression("(color as string)");
        let pair = evaluate_expression(&expr, meta);

        match pair.value {
            Expr::Lit(Lit::Str(Str { value, .. })) => assert_eq!(value.as_ref(), "blue"),
            other => panic!("expected string literal, found {:?}", other),
        }
    }

    #[test]
    fn falls_back_to_original_expression_when_not_evaluable() {
        let expr = Expr::Call(CallExpr {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(
                "dynamic".into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            )))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(string_literal("value")),
            }],
            type_args: None,
        });

        let meta = create_metadata();
        let pair = evaluate_expression(&expr, meta);

        match pair.value {
            Expr::Call(call) => match call.callee {
                Callee::Expr(callee) => match callee.as_ref() {
                    Expr::Ident(ident) => assert_eq!(ident.sym.as_ref(), "dynamic"),
                    other => panic!("expected identifier callee, found {:?}", other),
                },
                _ => panic!("expected call callee"),
            },
            other => panic!("expected call expression, found {:?}", other),
        }
    }
}
