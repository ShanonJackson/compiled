use swc_core::ecma::ast::{Expr, ObjectLit};

use crate::types::Metadata;
use crate::utils_create_result_pair::{create_result_pair, ResultPair};
use crate::utils_traversers_object::get_object_property_value;

pub fn evaluate_object_path(
  expression: &ObjectLit,
  meta: Metadata,
  property_name: &str,
) -> ResultPair {
  if let Some(result) = get_object_property_value(expression, property_name) {
    return create_result_pair(result.node, meta);
  }

  create_result_pair(Expr::Object(expression.clone()), meta)
}
