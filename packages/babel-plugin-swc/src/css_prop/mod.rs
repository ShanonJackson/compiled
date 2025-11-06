//! Port of `src/css-prop` helpers from the Babel plugin.
//!
//! The Babel implementation provides a small utility that inspects nearby
//! comments to determine whether the css prop transform should be disabled for a
//! given node. The SWC port mirrors this behaviour so that the comment
//! directives remain fully compatible.

use swc_core::common::Span;
use swc_core::ecma::ast::{JSXAttr, JSXElement};

use crate::constants::{
    COMPILED_DIRECTIVE_DISABLE_LINE, COMPILED_DIRECTIVE_DISABLE_NEXT_LINE,
    COMPILED_DIRECTIVE_TRANSFORM_CSS_PROP,
};
use crate::state::TransformState;
use crate::utils::comments::{get_node_comments, NodeComments};

/// Returns `true` when the css prop transform should be skipped for the given
/// JSX opening element and attribute.
///
/// This mirrors `visitCssPropPath`'s early exit behaviour where the visitor
/// looks at both the JSX element and the css prop itself for directive
/// comments.
pub fn is_css_prop_disabled(state: &TransformState, element: &JSXElement, attr: &JSXAttr) -> bool {
    let next_line_directive = format!(
        "{} {}",
        COMPILED_DIRECTIVE_DISABLE_NEXT_LINE, COMPILED_DIRECTIVE_TRANSFORM_CSS_PROP
    );
    let line_directive = format!(
        "{} {}",
        COMPILED_DIRECTIVE_DISABLE_LINE, COMPILED_DIRECTIVE_TRANSFORM_CSS_PROP
    );

    span_has_directive(state, element.span, &next_line_directive, &line_directive)
        || span_has_directive(
            state,
            element.opening.span,
            &next_line_directive,
            &line_directive,
        )
        || span_has_directive(state, attr.span, &next_line_directive, &line_directive)
}

fn span_has_directive(
    state: &TransformState,
    span: Span,
    next_line_directive: &str,
    line_directive: &str,
) -> bool {
    let NodeComments { before, current } = get_node_comments(state, span);

    before
        .iter()
        .any(|comment| comment_matches(comment, next_line_directive))
        || current
            .iter()
            .any(|comment| comment_matches(comment, line_directive))
}

fn comment_matches(comment: &str, directive: &str) -> bool {
    comment.trim().starts_with(directive)
}
