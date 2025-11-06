//! Port of `src/xcss-prop` helpers from the Babel plugin.
//!
//! These utilities intentionally mirror the structure of the TypeScript
//! implementation so the surrounding SWC transform can remain a mechanical
//! translation.

use std::collections::HashSet;

use swc_core::ecma::ast::{
    Expr, JSXAttr, JSXAttrName, MemberExpr, MemberProp, OptChainBase, OptChainExpr,
};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::state::TransformState;

/// Panic string emitted when an inline xcss object cannot be evaluated to a
/// static value.
pub const STATIC_OBJECT_ERROR: &str = "Object given to the xcss prop must be static";

/// Panic string emitted when the inline xcss transform encounters an unexpected
/// number of class names.
pub const UNEXPECTED_CLASSNAME_COUNT_ERROR: &str =
    "Unexpected count of class names please raise an issue on Github";

/// Returns `true` when the provided JSX attribute corresponds to an xcss prop.
pub fn is_xcss_attribute(attr: &JSXAttr) -> bool {
    match &attr.name {
        JSXAttrName::Ident(ident) => ident.sym.as_ref().to_lowercase().ends_with("xcss"),
        JSXAttrName::JSXNamespacedName(_) => false,
    }
}

/// Collects identifiers referenced through member expressions within the
/// supplied expression.
pub fn collect_member_expression_identifiers(expr: &Expr) -> Vec<String> {
    let mut identifiers = Vec::new();
    let mut visitor = MemberCollector {
        identifiers: &mut identifiers,
    };
    expr.visit_with(&mut visitor);
    identifiers
}

/// Collects cssMap-backed sheet strings for each identifier. Identifiers are
/// only processed once, mirroring the behaviour of the Babel implementation.
pub fn collect_pass_styles(state: &TransformState, identifiers: &[String]) -> Vec<String> {
    let mut styles = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();

    for name in identifiers {
        if !seen.insert(name.as_str()) {
            continue;
        }

        if let Some(entries) = state.css_map_sheets().get(name) {
            styles.extend(entries.clone());
        }
    }

    styles
}

struct MemberCollector<'a> {
    identifiers: &'a mut Vec<String>,
}

impl<'a> Visit for MemberCollector<'a> {
    fn visit_member_expr(&mut self, member: &MemberExpr) {
        if let Expr::Ident(ident) = &*member.obj {
            self.identifiers.push(ident.sym.to_string());
        } else {
            member.obj.visit_with(self);
        }

        if let MemberProp::Computed(prop) = &member.prop {
            prop.expr.visit_with(self);
        }
    }

    fn visit_opt_chain_expr(&mut self, opt: &OptChainExpr) {
        match &*opt.base {
            OptChainBase::Member(member) => member.visit_with(self),
            OptChainBase::Call(call) => {
                call.callee.visit_with(self);
                call.args.visit_with(self);
            }
        }
    }
}
