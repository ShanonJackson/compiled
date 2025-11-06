use swc_core::common::{SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    AssignExpr, AssignOp, AssignTarget, BinExpr, BinaryOp, Expr, Ident, IfStmt, Lit, MemberExpr,
    MemberProp, SimpleAssignTarget, Stmt, Str,
};

/// Mirrors `buildDisplayName` from the Babel implementation by emitting an
/// `if (process.env.NODE_ENV !== 'production') { Identifier.displayName = 'Name' }`
/// statement. The caller is responsible for appending the statement in the
/// appropriate location.
pub fn build_display_name_stmt(identifier: &str, display_name: Option<&str>) -> Stmt {
    let display = display_name.unwrap_or(identifier);

    let process_ident = Expr::Ident(Ident::new(
        "process".into(),
        DUMMY_SP,
        SyntaxContext::empty(),
    ));
    let env_ident = Ident::new("env".into(), DUMMY_SP, SyntaxContext::empty());
    let node_env_ident = Ident::new("NODE_ENV".into(), DUMMY_SP, SyntaxContext::empty());

    let env_member = Expr::Member(MemberExpr {
        span: DUMMY_SP,
        obj: Box::new(process_ident),
        prop: MemberProp::Ident(env_ident.into()),
    });
    let node_env_member = Expr::Member(MemberExpr {
        span: DUMMY_SP,
        obj: Box::new(env_member),
        prop: MemberProp::Ident(node_env_ident.into()),
    });

    let condition = Expr::Bin(BinExpr {
        span: DUMMY_SP,
        op: BinaryOp::NotEqEq,
        left: Box::new(node_env_member),
        right: Box::new(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: "production".into(),
            raw: None,
        }))),
    });

    let assignment = Expr::Assign(AssignExpr {
        span: DUMMY_SP,
        op: AssignOp::Assign,
        left: AssignTarget::Simple(SimpleAssignTarget::Member(MemberExpr {
            span: DUMMY_SP,
            obj: Box::new(Expr::Ident(Ident::new(
                identifier.into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            ))),
            prop: MemberProp::Ident(
                Ident::new("displayName".into(), DUMMY_SP, SyntaxContext::empty()).into(),
            ),
        })),
        right: Box::new(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: display.into(),
            raw: None,
        }))),
    });

    Stmt::If(IfStmt {
        span: DUMMY_SP,
        test: Box::new(condition),
        cons: Box::new(Stmt::Block(swc_core::ecma::ast::BlockStmt {
            span: DUMMY_SP,
            stmts: vec![Stmt::Expr(swc_core::ecma::ast::ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(assignment),
            })],
            ctxt: SyntaxContext::empty(),
        })),
        alt: None,
    })
}
