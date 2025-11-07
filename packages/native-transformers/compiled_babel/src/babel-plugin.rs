use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut};

use crate::types::{PluginOptions, TransformMetadata};

/// Primary SWC transform that will eventually mirror `@compiled/babel-plugin`.
pub struct CompiledBabelTransform {
    options: PluginOptions,
    metadata: TransformMetadata,
}

impl CompiledBabelTransform {
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

impl VisitMut for CompiledBabelTransform {
    noop_visit_mut_type!();

    fn visit_mut_program(&mut self, _: &mut Program) {
        // The real implementation will mirror the Babel visitors. For now, we simply retain the
        // original program without mutations so the scaffold can be used in tests while we port
        // features incrementally.
    }
}
