//! Types that mirror the shape of the Babel plugin.
//!
//! These definitions intentionally stay close to the TypeScript source so that
//! porting individual modules becomes a mechanical exercise. Many of the
//! fields are currently unused but are kept to ensure we don't accidentally
//! drift from the semantics of the original implementation while the SWC port
//! is completed.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::sync::Lrc;
use swc_core::common::SourceMap;
use swc_core::ecma::ast::Ident;

use crate::options::PluginConfig;

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginOptions {
    #[serde(flatten)]
    pub config: PluginConfig,
}

/// Metadata that callers can feed into the transformer. When running inside a
/// bundler we expect this to be populated with information about the file being
/// transformed alongside any contextual flags.
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct TransformMetadata {
    pub filename: Option<PathBuf>,
    pub source_file_name: Option<PathBuf>,
    pub root_dir: Option<PathBuf>,
    pub caller: Option<String>,
    pub source_map: Option<Lrc<SourceMap>>,
    pub comments: Option<Lrc<SingleThreadedComments>>,
}

impl std::fmt::Debug for TransformMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransformMetadata")
            .field("filename", &self.filename)
            .field("source_file_name", &self.source_file_name)
            .field("root_dir", &self.root_dir)
            .field("caller", &self.caller)
            .field(
                "source_map",
                &self.source_map.as_ref().map(|_| "<SourceMap>"),
            )
            .field("comments", &self.comments.as_ref().map(|_| "<Comments>"))
            .finish()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct CompiledImports {
    pub class_names: Option<Vec<String>>,
    pub css: Option<Vec<String>>,
    pub keyframes: Option<Vec<String>>,
    pub styled: Option<Vec<String>>,
    pub css_map: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct Pragma {
    pub jsx: bool,
    pub jsx_import_source: bool,
    pub classic_jsx_pragma_is_compiled: bool,
    pub classic_jsx_pragma_local_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct State {
    pub compiled_imports: Option<CompiledImports>,
    pub uses_xcss: bool,
    pub imported_compiled_imports: HashMap<String, String>,
    pub import_sources: Vec<String>,
    pub pragma: Pragma,
    pub paths_to_cleanup: Vec<PathCleanup>,
    pub opts: PluginConfig,
    pub sheets: HashMap<String, Ident>,
    pub css_map: HashMap<String, Vec<String>>,
    pub ignore_member_expressions: HashMap<String, bool>,
    pub resolver: Option<String>,
    pub included_files: Vec<PathBuf>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PathCleanup {
    pub action: PathCleanupAction,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PathCleanupAction {
    Replace,
    Remove,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum MetadataContext {
    Root,
    Keyframes { keyframe: String },
    Fragment,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Metadata {
    pub context: MetadataContext,
    pub state: State,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct Tag {
    pub name: String,
    pub r#type: TagType,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub enum TagType {
    #[default]
    InBuiltComponent,
    UserDefinedComponent,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct TransformResult {
    pub program: swc_core::ecma::ast::Program,
    pub style_rules: Vec<String>,
    pub included_files: Vec<PathBuf>,
}
