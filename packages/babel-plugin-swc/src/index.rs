//! Equivalent exports to `src/index.ts` from the Babel plugin.

pub use crate::babel_plugin::{transform, CompiledPlugin};
pub use crate::options::PluginConfig as PluginOptions;
pub use crate::types::{Metadata, Tag, TransformMetadata, TransformResult};
