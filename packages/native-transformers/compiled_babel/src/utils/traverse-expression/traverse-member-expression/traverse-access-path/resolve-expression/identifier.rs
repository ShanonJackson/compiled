use swc_core::ecma::ast::{Expr, Ident};

use crate::types::Metadata;
use crate::utils_create_result_pair::{create_result_pair, ResultPair};
use crate::utils_resolve_binding::resolve_binding;
use crate::utils_types::EvaluateExpression;

fn describe_expr(expr: &Expr) -> &'static str {
  use swc_core::ecma::ast::Expr::*;
  match expr {
    Ident(_) => "Ident",
    Member(_) => "Member",
    Object(_) => "Object",
    Array(_) => "Array",
    Lit(_) => "Lit",
    Tpl(_) => "Tpl",
    Call(_) => "Call",
    Fn(_) | Arrow(_) => "Function",
    _ => "Other",
  }
}

pub fn evaluate_identifier(
  expression: &Ident,
  meta: Metadata,
  evaluate_expression: EvaluateExpression,
) -> ResultPair {
  let debug = std::env::var("STACK_DEBUG_BINDING").is_ok()
    || std::env::var("STACK_DEBUG_SHARED")
      .map(|value| value == expression.sym.as_ref())
      .unwrap_or(false)
    || matches!(expression.sym.as_ref(), "sharedStyles" | "columnMinWidth");

  if debug {
    eprintln!(
      "[evaluate_identifier] ref='{}' file='{:?}'",
      expression.sym.as_ref(),
      meta.state().file().filename
    );
  }
  if let Some(binding) = resolve_binding(expression.sym.as_ref(), meta.clone(), evaluate_expression)
  {
    if debug {
      eprintln!(
        "[evaluate_identifier] binding ref='{}' node_present={} constant={} path={:?}",
        expression.sym.as_ref(),
        binding.node.is_some(),
        binding.constant,
        binding.path.as_ref().map(|p| &p.kind)
      );
    }
    if binding.constant {
      if let Some(node) = binding.node.as_ref() {
        let result = (evaluate_expression)(node, binding.meta.clone());
        if debug {
          eprintln!(
            "[evaluate_identifier] ref='{}' evaluated -> {}",
            expression.sym.as_ref(),
            describe_expr(&result.value)
          );
        }
        return create_result_pair(result.value, result.meta);
      }
    }
  }

  create_result_pair(Expr::Ident(expression.clone()), meta)
}
