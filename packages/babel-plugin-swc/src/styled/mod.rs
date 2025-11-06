//! Port of `src/styled` from the Babel plugin.
//!
//! The Babel implementation throws a `buildCodeFrameError` when a styled
//! template literal contains a logical expression that would emit an invalid
//! CSS declaration without a value. The SWC port mirrors this behaviour so
//! misuse is surfaced with the same panic message.

use swc_core::ecma::ast::{
    ArrowExpr, BinaryOp, BlockStmtOrExpr, CallExpr, Callee, Expr, MemberExpr, MemberProp, TaggedTpl,
};

use crate::state::TransformState;

#[derive(Clone)]
pub enum TagType {
    InBuiltComponent,
    UserDefinedComponent,
}

#[derive(Clone)]
pub struct Tag {
    pub name: String,
    pub tag_type: TagType,
}

#[derive(Clone)]
pub enum StyledCss {
    TaggedTemplate(TaggedTpl),
    ObjectArgs(Vec<Expr>),
}

#[derive(Clone)]
pub struct StyledData {
    pub tag: Tag,
    pub css: StyledCss,
}

const INVALID_LOGICAL_EXPRESSION_ERROR: &str = "A logical expression contains an invalid CSS declaration.\n      Compiled doesn't support CSS properties that are defined with a conditional rule that doesn't specify a default value.\n      Eg. font-weight: ${(props) => (props.isPrimary && props.isMaybe) && 'bold'}; is invalid.\n      Use ${(props) => props.isPrimary && props.isMaybe && ({ 'font-weight': 'bold' })}; instead";

/// Asserts that a styled tagged template literal does not contain the logical
/// expression pattern that Babel treats as invalid.
///
/// The panic string matches the original plugin so tests relying on Babel
/// parity can assert against the same text.
pub fn assert_valid_tagged_template(node: &TaggedTpl) {
    if has_invalid_logical_expression(node) {
        panic!("{INVALID_LOGICAL_EXPRESSION_ERROR}");
    }
}

pub fn extract_styled_data(expr: &Expr, state: &TransformState) -> Option<StyledData> {
    match expr {
        Expr::TaggedTpl(tagged) => extract_from_tagged_template(tagged, state),
        Expr::Call(call) => extract_from_call_expression(call, state),
        _ => None,
    }
}

fn extract_from_tagged_template(tagged: &TaggedTpl, state: &TransformState) -> Option<StyledData> {
    let tag = extract_tag_from_callee(&tagged.tag, state)?;
    Some(StyledData {
        tag,
        css: StyledCss::TaggedTemplate(tagged.clone()),
    })
}

fn extract_from_call_expression(call: &CallExpr, state: &TransformState) -> Option<StyledData> {
    let callee = match &call.callee {
        Callee::Expr(expr) => expr,
        Callee::Super(_) | Callee::Import(_) => return None,
    };

    let tag = extract_tag_from_expr(callee, state)?;
    let mut args = Vec::new();

    for arg in &call.args {
        if arg.spread.is_some() {
            return None;
        }

        args.push((*arg.expr).clone());
    }

    if args.is_empty() {
        return None;
    }

    Some(StyledData {
        tag,
        css: StyledCss::ObjectArgs(args),
    })
}

fn extract_tag_from_callee(expr: &Expr, state: &TransformState) -> Option<Tag> {
    match expr {
        Expr::Member(member) => extract_inbuilt_tag(member, state),
        Expr::Call(call) => extract_user_component_tag(call, state),
        _ => None,
    }
}

fn extract_tag_from_expr(expr: &Expr, state: &TransformState) -> Option<Tag> {
    extract_tag_from_callee(expr, state)
}

fn extract_inbuilt_tag(member: &MemberExpr, state: &TransformState) -> Option<Tag> {
    let Expr::Ident(object_ident) = &*member.obj else {
        return None;
    };

    if !state.is_styled_ident(object_ident.sym.as_ref()) {
        return None;
    }

    let MemberProp::Ident(prop_ident) = &member.prop else {
        return None;
    };

    Some(Tag {
        name: prop_ident.sym.to_string(),
        tag_type: TagType::InBuiltComponent,
    })
}

fn extract_user_component_tag(call: &CallExpr, state: &TransformState) -> Option<Tag> {
    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Ident(styled_ident) = &**callee else {
        return None;
    };

    if !state.is_styled_ident(styled_ident.sym.as_ref()) {
        return None;
    }

    let first_arg = call.args.get(0)?;
    if first_arg.spread.is_some() {
        return None;
    }

    let Expr::Ident(component_ident) = &*first_arg.expr else {
        return None;
    };

    Some(Tag {
        name: component_ident.sym.to_string(),
        tag_type: TagType::UserDefinedComponent,
    })
}

fn has_invalid_logical_expression(node: &TaggedTpl) -> bool {
    if !contains_logical_arrow(&node.tpl.exprs) {
        return false;
    }

    node.tpl
        .quasis
        .iter()
        .map(|quasi| quasi.raw.as_ref())
        .any(|raw| contains_empty_declaration(raw))
}

fn contains_logical_arrow(exprs: &[Box<Expr>]) -> bool {
    exprs.iter().any(|expr| match &**expr {
        Expr::Arrow(arrow) => arrow_has_logical_body(arrow),
        _ => false,
    })
}

fn arrow_has_logical_body(arrow: &ArrowExpr) -> bool {
    match &*arrow.body {
        BlockStmtOrExpr::Expr(expr) => match &**expr {
            Expr::Bin(bin) => matches!(
                bin.op,
                BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
            ),
            _ => false,
        },
        _ => false,
    }
}

fn contains_empty_declaration(raw: &str) -> bool {
    for declaration in raw.split(';') {
        if let Some((_, value)) = declaration.split_once(':') {
            if value.trim().is_empty() {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::common::{SyntaxContext, DUMMY_SP};
    use swc_core::ecma::ast::{
        ArrowExpr, BlockStmtOrExpr, Expr, Ident, TaggedTpl, Tpl, TplElement,
    };

    fn tagged_tpl(raw: &[&str], logical: bool) -> TaggedTpl {
        let exprs = if logical {
            vec![Box::new(Expr::Arrow(ArrowExpr {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                params: Vec::new(),
                body: Box::new(BlockStmtOrExpr::Expr(Box::new(Expr::Bin(
                    swc_core::ecma::ast::BinExpr {
                        span: DUMMY_SP,
                        op: BinaryOp::LogicalAnd,
                        left: Box::new(Expr::Ident(Ident::new(
                            "a".into(),
                            DUMMY_SP,
                            SyntaxContext::empty(),
                        ))),
                        right: Box::new(Expr::Ident(Ident::new(
                            "b".into(),
                            DUMMY_SP,
                            SyntaxContext::empty(),
                        ))),
                    },
                )))),
                is_async: false,
                is_generator: false,
                type_params: None,
                return_type: None,
            }))]
        } else {
            Vec::new()
        };

        let quasis = raw
            .iter()
            .map(|text| TplElement {
                span: DUMMY_SP,
                tail: false,
                cooked: None,
                raw: text.to_string().into(),
            })
            .collect();

        TaggedTpl {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            tag: Box::new(Expr::Ident(Ident::new(
                "styled".into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            ))),
            type_params: None,
            tpl: Box::new(Tpl {
                span: DUMMY_SP,
                exprs,
                quasis,
            }),
        }
    }

    #[test]
    fn ignores_when_no_logical_arrow() {
        let node = tagged_tpl(&["color: red"], false);
        assert!(!has_invalid_logical_expression(&node));
    }

    #[test]
    fn detects_empty_declaration() {
        let node = tagged_tpl(&["font-weight: "], true);
        assert!(has_invalid_logical_expression(&node));
    }
}
