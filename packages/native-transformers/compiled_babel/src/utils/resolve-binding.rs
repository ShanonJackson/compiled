use crate::types::Metadata;
use crate::utils_types::{EvaluateExpression, PartialBindingWithMeta};

/// Resolves a binding from the current metadata scopes, mirroring the initial
/// behaviour of the Babel helper by preferring bindings declared in the
/// metadata's own scope before falling back to the parent scope.
pub fn resolve_binding(
    reference_name: &str,
    meta: Metadata,
    evaluate_expression: EvaluateExpression,
) -> Option<PartialBindingWithMeta> {
    let _ = evaluate_expression;

    if let Some(scope) = meta.own_scope() {
        if let Some(binding) = scope.borrow().get(reference_name) {
            return Some(binding.clone());
        }
    }

    if let Some(binding) = meta.parent_scope().borrow().get(reference_name) {
        return Some(binding.clone());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::resolve_binding;
    use crate::types::{Metadata, PluginOptions, TransformFile, TransformState};
    use crate::utils_create_result_pair::create_result_pair;
    use crate::utils_types::{
        BindingPath, BindingSource, EvaluateExpression, PartialBindingWithMeta,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use swc_core::common::sync::Lrc;
    use swc_core::common::{FileName, SourceMap, DUMMY_SP};
    use swc_core::ecma::ast::{Expr, Lit, Str};
    use swc_ecma_parser::lexer::Lexer;
    use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax};

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

    fn identity_evaluate(
        expr: &Expr,
        meta: Metadata,
    ) -> crate::utils_create_result_pair::ResultPair {
        create_result_pair(expr.clone(), meta)
    }

    fn parse_expression(code: &str) -> Expr {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom("expr.tsx".into()).into(), code.into());
        let lexer = Lexer::new(
            Syntax::Es(EsSyntax {
                jsx: true,
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        *parser.parse_expr().expect("parse expression")
    }

    #[test]
    fn resolves_binding_from_parent_scope() {
        let meta = create_metadata();
        let binding_expr = string_literal("blue");
        let binding = PartialBindingWithMeta::new(
            Some(binding_expr.clone()),
            Some(BindingPath::new(Some(DUMMY_SP))),
            true,
            meta.clone(),
            BindingSource::Module,
        );

        meta.insert_parent_binding("color", binding.clone());

        let result = resolve_binding(
            "color",
            meta.clone(),
            identity_evaluate as EvaluateExpression,
        )
        .expect("binding");

        assert!(result.constant);
        assert_eq!(result.node, Some(binding_expr));
        assert_eq!(result.source, BindingSource::Module);
    }

    #[test]
    fn prefers_binding_from_own_scope() {
        let meta = create_metadata();
        let parent_binding = PartialBindingWithMeta::new(
            Some(string_literal("parent")),
            None,
            true,
            meta.clone(),
            BindingSource::Module,
        );
        meta.insert_parent_binding("value", parent_binding);

        let own_scope = meta.allocate_own_scope();
        let scoped_meta = meta.with_own_scope(Some(own_scope.clone()));
        let own_binding = PartialBindingWithMeta::new(
            Some(string_literal("own")),
            None,
            true,
            scoped_meta.clone(),
            BindingSource::Module,
        );
        own_scope
            .borrow_mut()
            .insert("value".into(), own_binding.clone());

        let result = resolve_binding(
            "value",
            scoped_meta,
            identity_evaluate as EvaluateExpression,
        )
        .expect("binding");

        assert_eq!(result.node, Some(string_literal("own")));
    }

    #[test]
    fn returns_none_when_binding_missing() {
        let meta = create_metadata();

        let result = resolve_binding("missing", meta, identity_evaluate as EvaluateExpression);

        assert!(result.is_none());
    }

    #[test]
    fn preserves_binding_metadata_on_clone() {
        let meta = create_metadata();
        let expr = parse_expression("({ primary: 'blue' })");
        let binding = PartialBindingWithMeta::new(
            Some(expr.clone()),
            None,
            true,
            meta.clone(),
            BindingSource::Module,
        );
        meta.insert_parent_binding("theme", binding.clone());

        let result = resolve_binding(
            "theme",
            meta.clone(),
            identity_evaluate as EvaluateExpression,
        )
        .expect("binding");

        assert!(result.constant);
        assert_eq!(result.node, Some(expr));
        assert_eq!(
            result.meta.state().file().filename,
            meta.state().file().filename
        );
    }
}
