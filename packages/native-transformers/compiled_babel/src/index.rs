use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::VisitMutWith;

use crate::babel_plugin::CompiledBabelTransform;
#[allow(unused_imports)]
pub use crate::types::{
    CacheBehavior, PluginOptions, ResolverOption, StyleRule, TransformMetadata, TransformOutput,
};

/// Entry point mirroring `packages/babel-plugin/src/index.ts`.
pub fn transform(program: Program, options: PluginOptions) -> TransformOutput {
    let mut transform = CompiledBabelTransform::new(options);
    let mut program = program;
    program.visit_mut_with(&mut transform);

    let metadata: TransformMetadata = transform.into_metadata();

    TransformOutput { program, metadata }
}
