//! Port of `src/class-names` from the Babel plugin.
//!
//! The Babel implementation performs a full transformation of the
//! `<ClassNames>` component. The SWC port is still in progress; however, we can
//! already mirror the validation behaviour that ensures the component receives
//! a render-prop function as its child. The original plugin throws a
//! `buildCodeFrameError` with a descriptive message when the structure is
//! incorrect. To keep behaviour identical we surface the same panic message so
//! fixture tests can rely on Babel parity even before the full lowering is in
//! place.

use swc_core::ecma::ast::{Expr, JSXElement, JSXElementChild, JSXExpr};

const CLASS_NAMES_CHILDREN_ERROR: &str =
    "ClassNames children should be a function\nE.g: <ClassNames>{props => <div />}</ClassNames>";

/// Ensures that the `<ClassNames>` element receives a function child.
///
/// Babel throws a runtime error with the message above. We replicate the same
/// string (including the newline) so consumers observe identical behaviour when
/// misusing the component. Returning a `Result` would force each caller to map
/// the message into a panic, so the helper panics directly to keep the call
/// site minimal and avoid drifting from the JS implementation.
pub fn assert_function_children(node: &JSXElement) {
    if has_function_child(node) {
        return;
    }

    panic!("{CLASS_NAMES_CHILDREN_ERROR}");
}

fn has_function_child(node: &JSXElement) -> bool {
    node.children.iter().any(|child| match child {
        JSXElementChild::JSXExprContainer(container) => match &container.expr {
            JSXExpr::Expr(expr) => matches!(expr.as_ref(), Expr::Fn(_) | Expr::Arrow(_)),
            _ => false,
        },
        // Ignore whitespace or other JSX nodes – only expression containers can
        // carry the render prop we are interested in.
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::common::{SyntaxContext, DUMMY_SP};
    use swc_core::ecma::ast::{
        ArrowExpr, BindingIdent, BlockStmtOrExpr, Expr, Ident, JSXElementName, JSXExpr,
        JSXExprContainer, JSXOpeningElement, Pat,
    };

    fn class_names_element_with_child(child: JSXElementChild) -> JSXElement {
        JSXElement {
            span: DUMMY_SP,
            opening: JSXOpeningElement {
                name: JSXElementName::Ident(Ident::new(
                    "ClassNames".into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                )),
                attrs: Default::default(),
                self_closing: false,
                type_args: None,
                span: DUMMY_SP,
            },
            closing: None,
            children: vec![child],
        }
    }

    #[test]
    fn detects_function_child() {
        let arrow = JSXElementChild::JSXExprContainer(JSXExprContainer {
            span: DUMMY_SP,
            expr: JSXExpr::Expr(Box::new(Expr::Arrow(ArrowExpr {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                params: vec![Pat::Ident(BindingIdent {
                    id: Ident::new("props".into(), DUMMY_SP, SyntaxContext::empty()),
                    type_ann: None,
                })],
                body: Box::new(BlockStmtOrExpr::Expr(Box::new(Expr::Ident(Ident::new(
                    "props".into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                ))))),
                is_async: false,
                is_generator: false,
                type_params: None,
                return_type: None,
            }))),
        });

        let element = class_names_element_with_child(arrow);
        assert!(has_function_child(&element));
    }

    #[test]
    fn rejects_missing_child() {
        let child = JSXElementChild::JSXExprContainer(JSXExprContainer {
            span: DUMMY_SP,
            expr: JSXExpr::Expr(Box::new(Expr::Ident(Ident::new(
                "value".into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            )))),
        });

        let element = class_names_element_with_child(child);
        assert!(!has_function_child(&element));
    }
}
