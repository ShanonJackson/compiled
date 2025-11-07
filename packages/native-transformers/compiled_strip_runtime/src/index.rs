use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::VisitMutWith;

use crate::strip_runtime::StripRuntimeTransform;
use crate::types::{PluginOptions, TransformMetadata, TransformOutput};

/// Entry point mirroring the Babel strip-runtime plugin API.
pub fn transform(program: Program, options: PluginOptions) -> TransformOutput {
    let mut transform = StripRuntimeTransform::new(options);
    let mut program = program;
    program.visit_mut_with(&mut transform);

    let metadata: TransformMetadata = transform.into_metadata();

    TransformOutput { program, metadata }
}

pub use crate::types::{
    ExtractStylesToDirectory, PluginOptions, TransformMetadata, TransformOutput,
};
