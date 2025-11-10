use swc_core::ecma::ast::Expr;

use crate::types::Metadata;
use crate::utils_create_result_pair::{create_result_pair, ResultPair};

pub fn evaluate_namespace_import_path(
    expression: &Expr,
    meta: Metadata,
    _path_name: &str,
) -> ResultPair {
    create_result_pair(expression.clone(), meta)
}
