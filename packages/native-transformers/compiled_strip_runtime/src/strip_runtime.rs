use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut};

use crate::types::{PluginOptions, TransformMetadata};

/// Native SWC transform scaffolding for `@compiled/babel-plugin-strip-runtime`.
pub struct StripRuntimeTransform {
    options: PluginOptions,
    metadata: TransformMetadata,
}

impl StripRuntimeTransform {
    pub fn new(options: PluginOptions) -> Self {
        Self {
            options,
            metadata: TransformMetadata::default(),
        }
    }

    pub fn into_metadata(self) -> TransformMetadata {
        self.metadata
    }

    pub fn options(&self) -> &PluginOptions {
        &self.options
    }
}

impl VisitMut for StripRuntimeTransform {
    noop_visit_mut_type!();

    fn visit_program(&mut self, _: &mut Program) {
        // The full visitor will be ported from Babel. For now, no mutations are performed.
    }
}
