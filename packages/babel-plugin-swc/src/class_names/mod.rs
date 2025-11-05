//! Port of `src/class-names` from the Babel plugin.
//!
//! The full visitor logic is yet to be implemented. The function signature is
//! provided so that future patches can focus purely on porting the behaviour
//! without touching call-sites.

use swc_core::ecma::ast::JSXElement;
use swc_core::ecma::visit::Fold;

use crate::types::Metadata;

#[allow(dead_code)]
pub fn visit_class_names_path<T>(_node: &mut JSXElement, _meta: &mut Metadata, _fold: &mut T)
where
    T: Fold,
{
    // TODO: port logic from packages/babel-plugin/src/class-names
}
