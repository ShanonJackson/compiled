use swc_core::ecma::ast::{Expr, ObjectLit};

use crate::types::Metadata;
use crate::utils_create_result_pair::{create_result_pair, ResultPair};
use crate::utils_traversers_object::get_object_property_value;

pub fn evaluate_object_path(
  expression: &ObjectLit,
  meta: Metadata,
  property_name: &str,
) -> ResultPair {
  let debug = std::env::var("STACK_DEBUG_BINDING").is_ok()
    || std::env::var("STACK_DEBUG_SHARED")
      .map(|value| value == property_name)
      .unwrap_or(false)
    || matches!(property_name, "columnMinWidth" | "sharedStyles");
  if let Some(result) = get_object_property_value(expression, property_name) {
    if debug {
      eprintln!(
        "[evaluate_object_path] hit prop='{}' span={:?} expr_kind={}",
        property_name,
        result.span,
        match &result.node {
          Expr::Lit(_) => "Lit",
          Expr::Object(_) => "Object",
          Expr::Ident(_) => "Ident",
          Expr::Member(_) => "Member",
          Expr::Call(_) => "Call",
          _ => "Other",
        }
      );
    }
    return create_result_pair(result.node, meta);
  }

  if debug {
    eprintln!(
      "[evaluate_object_path] miss prop='{}', returning object",
      property_name
    );
  }
  create_result_pair(Expr::Object(expression.clone()), meta)
}
