use swc_core::common::Span;
use swc_core::ecma::ast::{BinaryOp, Expr};

use crate::types::Metadata;
use crate::utils_create_result_pair::ResultPair;

/// Represents CSS that should always be emitted without additional guards.
#[derive(Clone, Debug, PartialEq)]
pub struct UnconditionalCssItem {
    pub css: String,
}

/// Represents CSS gated behind a conditional expression.
#[derive(Clone, Debug, PartialEq)]
pub struct ConditionalCssItem {
    pub test: Expr,
    pub consequent: Box<CssItem>,
    pub alternate: Box<CssItem>,
}

/// Logical operators supported when composing conditional CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalOperator {
    Or,
    Nullish,
    And,
}

impl LogicalOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogicalOperator::Or => "||",
            LogicalOperator::Nullish => "??",
            LogicalOperator::And => "&&",
        }
    }

    pub fn to_binary_op(&self) -> BinaryOp {
        match self {
            LogicalOperator::Or => BinaryOp::LogicalOr,
            LogicalOperator::Nullish => BinaryOp::NullishCoalescing,
            LogicalOperator::And => BinaryOp::LogicalAnd,
        }
    }
}

/// Represents CSS that is composed with a logical expression.
#[derive(Clone, Debug, PartialEq)]
pub struct LogicalCssItem {
    pub expression: Expr,
    pub operator: LogicalOperator,
    pub css: String,
}

/// Represents CSS that has already been extracted into a sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetCssItem {
    pub css: String,
}

/// Represents CSS generated from a cssMap entry.
#[derive(Clone, Debug, PartialEq)]
pub struct CssMapItem {
    pub name: String,
    pub expression: Expr,
    pub css: String,
}

/// Enum that captures the various CSS item variants handled by the transformer.
#[derive(Clone, Debug, PartialEq)]
pub enum CssItem {
    Unconditional(UnconditionalCssItem),
    Conditional(ConditionalCssItem),
    Logical(LogicalCssItem),
    Sheet(SheetCssItem),
    Map(CssMapItem),
}

impl CssItem {
    pub fn unconditional(css: impl Into<String>) -> Self {
        CssItem::Unconditional(UnconditionalCssItem { css: css.into() })
    }

    pub fn sheet(css: impl Into<String>) -> Self {
        CssItem::Sheet(SheetCssItem { css: css.into() })
    }
}

/// Represents a CSS variable interpolation that should be emitted alongside CSS.
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub name: String,
    pub expression: Expr,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

/// Bundled CSS output from builders.
#[derive(Clone, Debug, PartialEq)]
pub struct CssOutput {
    pub css: Vec<CssItem>,
    pub variables: Vec<Variable>,
}

impl CssOutput {
    pub fn new() -> Self {
        Self {
            css: Vec::new(),
            variables: Vec::new(),
        }
    }
}

/// Describes whether a resolved binding originated from a local module or an import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingSource {
    Import,
    Module,
}

/// Minimal representation of a binding path captured during traversal.
///
/// Babel exposes a `NodePath`, but in the SWC port we track the originating
/// span so downstream helpers can continue to reason about the source location
/// without depending on Babel internals.
#[derive(Clone, Debug, PartialEq)]
pub struct BindingPath {
    pub span: Option<Span>,
}

impl BindingPath {
    pub fn new(span: Option<Span>) -> Self {
        Self { span }
    }
}

/// Binding information paired with metadata for downstream evaluation helpers.
#[derive(Clone, Debug)]
pub struct PartialBindingWithMeta {
    pub node: Option<Expr>,
    pub path: Option<BindingPath>,
    pub constant: bool,
    pub meta: Metadata,
    pub source: BindingSource,
}

impl PartialBindingWithMeta {
    pub fn new(
        node: Option<Expr>,
        path: Option<BindingPath>,
        constant: bool,
        meta: Metadata,
        source: BindingSource,
    ) -> Self {
        Self {
            node,
            path,
            constant,
            meta,
            source,
        }
    }
}

/// Function pointer type used by traversal helpers when recursively evaluating expressions.
pub type EvaluateExpression = fn(&Expr, Metadata) -> ResultPair;

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::common::DUMMY_SP;
    use swc_core::ecma::ast::{Expr, Lit, Str};

    #[test]
    fn creates_css_item_variants() {
        let unconditional = CssItem::unconditional("color: red;");
        match unconditional {
            CssItem::Unconditional(item) => assert_eq!(item.css, "color: red;"),
            _ => panic!("expected unconditional css item"),
        }

        let sheet = CssItem::sheet(".a { color: blue; }");
        match sheet {
            CssItem::Sheet(item) => assert_eq!(item.css, ".a { color: blue; }"),
            _ => panic!("expected sheet css item"),
        }
    }

    #[test]
    fn stores_binding_metadata() {
        let cm: swc_core::common::sync::Lrc<swc_core::common::SourceMap> = Default::default();
        let file = crate::types::TransformFile::new(cm.clone(), Vec::new());
        let state = std::rc::Rc::new(std::cell::RefCell::new(crate::types::TransformState::new(
            file,
            crate::types::PluginOptions::default(),
        )));
        let meta = Metadata::new(state);
        let expr = Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: "test".into(),
            raw: None,
        }));

        let binding = PartialBindingWithMeta::new(
            Some(expr.clone()),
            Some(BindingPath::new(Some(DUMMY_SP))),
            true,
            meta.clone(),
            BindingSource::Module,
        );

        assert!(binding.constant);
        assert_eq!(binding.node, Some(expr));
        assert_eq!(binding.meta.state().included_files.len(), 0);
        assert_eq!(binding.source, BindingSource::Module);
    }
}
