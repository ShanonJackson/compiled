//! Shared helpers used across the SWC port of the Compiled plugin.
//!
//! Only a very small subset of the original utility layer is currently
//! required. New helpers can be ported from the Babel implementation on an
//! as-needed basis as the transformation logic grows.

pub mod cache;
pub mod hash;
pub mod kebab_case;
pub mod wtf8;

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{Expr, ExprStmt, ModuleItem, Stmt};

/// Wraps an expression in a statement so it can be injected at the module
/// level.
#[allow(dead_code)]
pub fn into_module_stmt(expr: Expr) -> ModuleItem {
    ModuleItem::Stmt(Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(expr),
    }))
}
