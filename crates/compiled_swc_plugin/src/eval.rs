use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveOptions, Resolver};
use serde::{Deserialize, Serialize};
use swc_core::ecma::ast::{Expr, Lit};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvaluationError {
    #[error("unsupported expression kind")]
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvaluatorOptions {
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug)]
pub struct StaticEvaluator {
    resolver: Resolver,
    _options: EvaluatorOptions,
}

impl StaticEvaluator {
    pub fn new(cwd: &Path, options: EvaluatorOptions) -> Self {
        let mut resolve_options = ResolveOptions::default();
        if !options.extensions.is_empty() {
            resolve_options.extensions = options.extensions.clone();
        }
        let resolver = Resolver::new(cwd.to_path_buf(), resolve_options);
        Self {
            resolver,
            _options: options,
        }
    }

    pub fn resolve(&self, from: &Path, request: &str) -> Option<PathBuf> {
        self.resolver
            .resolve(from, request)
            .ok()
            .map(|res| res.full_path)
    }

    pub fn evaluate(&self, expr: &Expr) -> Result<Option<EvaluatedValue>, EvaluationError> {
        match expr {
            Expr::Lit(lit) => Ok(Some(match lit {
                Lit::Str(str_lit) => EvaluatedValue::String(str_lit.value.to_string()),
                Lit::Num(num_lit) => EvaluatedValue::Number(num_lit.value),
                Lit::Bool(bool_lit) => EvaluatedValue::Bool(bool_lit.value),
                Lit::Null(_) => EvaluatedValue::Null,
                _ => return Err(EvaluationError::Unsupported),
            })),
            Expr::Tpl(tpl) if tpl.exprs.is_empty() => {
                let cooked = tpl
                    .quasis
                    .iter()
                    .map(|q| {
                        q.cooked
                            .as_ref()
                            .or_else(|| q.raw.as_ref())
                            .map(|atom| atom.to_string())
                            .unwrap_or_default()
                    })
                    .collect::<String>();
                Ok(Some(EvaluatedValue::String(cooked)))
            }
            Expr::Ident(_) => Ok(None),
            _ => Err(EvaluationError::Unsupported),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvaluatedValue {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use swc_common::DUMMY_SP;
    use swc_ecma_ast::{Expr, Lit, Str};

    use super::{EvaluatedValue, StaticEvaluator};

    #[test]
    fn evaluates_string_literal() {
        let evaluator = StaticEvaluator::new(PathBuf::from(".").as_path(), Default::default());
        let expr = Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: "hello".into(),
            raw: None,
        }));
        assert_eq!(
            evaluator.evaluate(&expr).unwrap(),
            Some(EvaluatedValue::String("hello".into()))
        );
    }
}
