//! Entry points that mirror the structure of `babel-plugin.ts` in the
//! TypeScript implementation.

use swc_core::ecma::ast::Program;

use crate::{
    options::PluginConfig,
    transform::Transform,
    types::{TransformMetadata, TransformResult},
};

pub struct CompiledPlugin {
    transform: Transform,
}

impl CompiledPlugin {
    pub fn new(config: PluginConfig, metadata: TransformMetadata) -> Self {
        Self {
            transform: Transform::new(config, metadata),
        }
    }

    pub fn transform(mut self, program: Program) -> TransformResult {
        self.transform.apply(program)
    }
}

pub fn transform(
    program: Program,
    config: PluginConfig,
    metadata: TransformMetadata,
) -> TransformResult {
    let mut transform = Transform::new(config, metadata);

    transform.apply(program)
}
