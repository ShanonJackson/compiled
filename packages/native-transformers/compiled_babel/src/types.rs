use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swc_core::common::comments::Comment;
use swc_core::common::sync::Lrc;
use swc_core::common::{SourceMap, Span};
use swc_core::ecma::ast::{Expr, Ident, Program};

use crate::constants::DEFAULT_IMPORT_SOURCES;
use crate::utils_cache::{Cache, CacheOptions};
use crate::utils_types::PartialBindingWithMeta;

fn normalized_join(root: &Path, segment: &str) -> PathBuf {
    root.join(segment).components().collect()
}
/// Mirror of the union used by the Babel plugin for controlling cache behaviour.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CacheBehavior {
    /// Equivalent to setting the option to a boolean.
    Enabled(bool),
    /// Matches the `'file-pass'` literal supported by the Babel plugin.
    FilePass(String),
}

impl CacheBehavior {
    pub fn is_enabled(&self) -> bool {
        match self {
            CacheBehavior::Enabled(value) => *value,
            CacheBehavior::FilePass(_) => true,
        }
    }

    pub fn is_file_pass(&self) -> bool {
        matches!(self, CacheBehavior::FilePass(_))
    }
}

impl Default for CacheBehavior {
    fn default() -> Self {
        CacheBehavior::Enabled(false)
    }
}

/// Represents a resolver configuration that can be either an inline object or a module string.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ResolverOption {
    Module(String),
    Inline(Value),
}

impl ResolverOption {
    pub fn as_module(&self) -> Option<&str> {
        match self {
            ResolverOption::Module(value) => Some(value.as_str()),
            ResolverOption::Inline(_) => None,
        }
    }
}

/// Normalized resolver stored on the transform state.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedResolver {
    Inline(Value),
    Module(String),
}

impl ResolvedResolver {
    pub fn from_option(option: &ResolverOption, root: &Path) -> Self {
        match option {
            ResolverOption::Module(specifier) => {
                if specifier.starts_with('.') {
                    let joined = normalized_join(root, specifier);
                    ResolvedResolver::Module(joined.to_string_lossy().into_owned())
                } else {
                    ResolvedResolver::Module(specifier.clone())
                }
            }
            ResolverOption::Inline(value) => ResolvedResolver::Inline(value.clone()),
        }
    }

    pub fn as_module(&self) -> Option<&str> {
        match self {
            ResolvedResolver::Module(value) => Some(value.as_str()),
            ResolvedResolver::Inline(_) => None,
        }
    }

    pub fn as_inline(&self) -> Option<&Value> {
        match self {
            ResolvedResolver::Inline(value) => Some(value),
            ResolvedResolver::Module(_) => None,
        }
    }
}

/// Rust representation of the Babel plugin options.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginOptions {
    pub cache: Option<CacheBehavior>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<usize>,
    pub import_react: Option<bool>,
    pub nonce: Option<String>,
    pub import_sources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_included_files: Option<Value>,
    pub optimize_css: Option<bool>,
    pub resolver: Option<ResolverOption>,
    pub extensions: Option<Vec<String>>,
    pub parser_babel_plugins: Option<Vec<Value>>,
    pub add_component_name: Option<bool>,
    pub class_name_compression_map: Option<BTreeMap<String, String>>,
    pub process_xcss: Option<bool>,
    pub increase_specificity: Option<bool>,
    pub sort_at_rules: Option<bool>,
    pub class_hash_prefix: Option<String>,
    pub flatten_multiple_selectors: Option<bool>,
    pub extract: Option<bool>,
}

impl Default for PluginOptions {
    fn default() -> Self {
        Self {
            cache: None,
            max_size: None,
            import_react: None,
            nonce: None,
            import_sources: None,
            on_included_files: None,
            optimize_css: None,
            resolver: None,
            extensions: None,
            parser_babel_plugins: None,
            add_component_name: None,
            class_name_compression_map: None,
            process_xcss: None,
            increase_specificity: None,
            sort_at_rules: None,
            class_hash_prefix: None,
            flatten_multiple_selectors: None,
            extract: None,
        }
    }
}

/// Metadata returned from the transform.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformMetadata {
    pub included_files: Vec<String>,
    pub style_rules: Vec<String>,
}

/// Result of a transform run containing the mutated program and collected metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformOutput {
    pub program: Program,
    pub metadata: TransformMetadata,
}

impl TransformOutput {
    pub fn empty(program: Program) -> Self {
        Self {
            program,
            metadata: TransformMetadata::default(),
        }
    }
}

/// Represents the file-level information tracked during a transform.
#[derive(Clone)]
pub struct TransformFile {
    pub source_map: Lrc<SourceMap>,
    pub comments: Vec<Comment>,
    pub filename: Option<String>,
    pub cwd: PathBuf,
    pub root: PathBuf,
    pub loc: Option<TransformFileLocation>,
}

/// Location metadata exposed by Babel's `BabelFile.loc` helper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformFileLocation {
    pub filename: String,
}

/// Options used to construct `TransformFile` instances.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransformFileOptions {
    pub filename: Option<String>,
    pub cwd: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub loc_filename: Option<String>,
}

impl TransformFile {
    pub fn new(source_map: Lrc<SourceMap>, comments: Vec<Comment>) -> Self {
        Self::with_options(source_map, comments, TransformFileOptions::default())
    }

    pub fn with_options(
        source_map: Lrc<SourceMap>,
        comments: Vec<Comment>,
        options: TransformFileOptions,
    ) -> Self {
        let TransformFileOptions {
            filename,
            cwd,
            root,
            loc_filename,
        } = options;

        let cwd_path = cwd
            .or_else(|| env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let root_path = root.unwrap_or_else(|| cwd_path.clone());
        let loc = loc_filename
            .or_else(|| filename.as_ref().cloned())
            .map(|filename| TransformFileLocation { filename });

        Self {
            source_map,
            comments,
            filename,
            cwd: cwd_path,
            root: root_path,
            loc,
        }
    }
}

impl Default for TransformFile {
    fn default() -> Self {
        Self::new(Lrc::new(SourceMap::default()), Vec::new())
    }
}

impl fmt::Debug for TransformFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransformFile")
            .field("filename", &self.filename)
            .field("cwd", &self.cwd)
            .field("root", &self.root)
            .field("comments", &self.comments)
            .finish()
    }
}

/// Shared pragma flags toggled during a transform.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PragmaFlags {
    pub jsx: bool,
    pub jsx_import_source: bool,
    pub classic_jsx_pragma_is_compiled: bool,
    pub classic_jsx_pragma_local_name: Option<String>,
}

/// Tracks discovered compiled imports for the current file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompiledImports {
    pub class_names: Vec<String>,
    pub css: Vec<String>,
    pub keyframes: Vec<String>,
    pub styled: Vec<String>,
    pub css_map: Vec<String>,
}

/// Tracks compiled runtime imports that have already been inserted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportedCompiledImports {
    pub css: Option<String>,
}

/// Represents a cleanup action scheduled for visitor exit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CleanupAction {
    Replace,
    Remove,
}

/// Placeholder for the Babel `NodePath` cleanup entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathCleanup {
    pub action: CleanupAction,
    pub span: Span,
}

/// Tracks spans that have already been transformed to avoid duplicate work.
#[derive(Clone, Debug, Default)]
pub struct TransformCache {
    spans: IndexSet<Span>,
}

impl TransformCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has(&self, span: Span) -> bool {
        self.spans.contains(&span)
    }

    pub fn set(&mut self, span: Span) {
        self.spans.insert(span);
    }

    pub fn clear(&mut self) {
        self.spans.clear();
    }

    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

/// Core transform state shared across visitors.
#[derive(Debug)]
pub struct TransformState {
    pub compiled_imports: Option<CompiledImports>,
    pub uses_xcss: bool,
    pub imported_compiled_imports: ImportedCompiledImports,
    pub import_sources: Vec<String>,
    pub pragma: PragmaFlags,
    pub paths_to_cleanup: Vec<PathCleanup>,
    pub opts: PluginOptions,
    pub file: TransformFile,
    pub included_files: Vec<String>,
    pub sheets: IndexMap<String, Ident>,
    pub style_rules: IndexSet<String>,
    pub sheet_identifier_counter: usize,
    pub cache: Cache<Value>,
    pub css_map: IndexMap<String, Vec<String>>,
    pub ignore_member_expressions: IndexSet<String>,
    pub resolver: Option<ResolvedResolver>,
    pub transform_cache: TransformCache,
    pub filename: Option<String>,
    pub cwd: PathBuf,
    pub root: PathBuf,
}

impl TransformState {
    pub fn new(file: TransformFile, opts: PluginOptions) -> Self {
        let filename = file.filename.clone();
        let cwd = file.cwd.clone();
        let root = file.root.clone();
        let import_sources = Self::resolve_import_sources(&file, &opts);
        let resolver = opts
            .resolver
            .as_ref()
            .map(|resolver_option| ResolvedResolver::from_option(resolver_option, &root));

        let cache_enabled = opts
            .cache
            .as_ref()
            .map(CacheBehavior::is_enabled)
            .unwrap_or(false);
        let max_size = opts.max_size;
        let mut cache = Cache::new();
        cache.initialize(CacheOptions {
            cache: Some(cache_enabled),
            max_size,
        });

        Self {
            compiled_imports: None,
            uses_xcss: false,
            imported_compiled_imports: ImportedCompiledImports::default(),
            import_sources,
            pragma: PragmaFlags::default(),
            paths_to_cleanup: Vec::new(),
            opts,
            file,
            included_files: Vec::new(),
            sheets: IndexMap::new(),
            style_rules: IndexSet::new(),
            sheet_identifier_counter: 0,
            cache,
            css_map: IndexMap::new(),
            ignore_member_expressions: IndexSet::new(),
            resolver,
            transform_cache: TransformCache::default(),
            filename,
            cwd,
            root,
        }
    }

    pub fn file(&self) -> &TransformFile {
        &self.file
    }

    pub fn file_mut(&mut self) -> &mut TransformFile {
        &mut self.file
    }

    fn resolve_import_sources(file: &TransformFile, opts: &PluginOptions) -> Vec<String> {
        let mut sources: Vec<String> = DEFAULT_IMPORT_SOURCES
            .iter()
            .map(|source| source.to_string())
            .collect();

        if let Some(additional) = &opts.import_sources {
            for origin in additional {
                if origin.starts_with('.') {
                    let joined = normalized_join(&file.root, origin);
                    sources.push(joined.to_string_lossy().into_owned());
                } else {
                    sources.push(origin.clone());
                }
            }
        }

        sources
    }
}

/// Shared pointer to the transform state, allowing metadata clones to mutate it.
pub type SharedTransformState = Rc<RefCell<TransformState>>;

/// Contextual metadata threaded through helper utilities during traversal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataContext {
    Root,
    Keyframes { keyframe: String },
    Fragment,
}

/// Shared scope map that mirrors Babel's `NodePath` binding storage.
pub type SharedScope = Rc<RefCell<IndexMap<String, PartialBindingWithMeta>>>;

fn new_scope() -> SharedScope {
    Rc::new(RefCell::new(IndexMap::new()))
}

/// Metadata wrapper that mirrors the Babel helpers.
#[derive(Clone, Debug)]
pub struct Metadata {
    pub state: SharedTransformState,
    pub context: MetadataContext,
    pub parent_span: Option<Span>,
    pub own_span: Option<Span>,
    pub parent_scope: SharedScope,
    pub own_scope: Option<SharedScope>,
    pub parent_expr: Option<Box<Expr>>,
}

impl Metadata {
    pub fn new(state: SharedTransformState) -> Self {
        Self {
            state,
            context: MetadataContext::Root,
            parent_span: None,
            own_span: None,
            parent_scope: new_scope(),
            own_scope: None,
            parent_expr: None,
        }
    }

    pub fn with_context(&self, context: MetadataContext) -> Self {
        Self {
            context,
            ..self.clone()
        }
    }

    pub fn with_parent_span(&self, parent_span: Option<Span>) -> Self {
        Self {
            parent_span,
            ..self.clone()
        }
    }

    pub fn with_own_span(&self, own_span: Option<Span>) -> Self {
        Self {
            own_span,
            ..self.clone()
        }
    }

    pub fn state(&self) -> Ref<'_, TransformState> {
        self.state.borrow()
    }

    pub fn state_mut(&self) -> RefMut<'_, TransformState> {
        self.state.borrow_mut()
    }

    pub fn with_parent_scope(&self, parent_scope: SharedScope) -> Self {
        Self {
            parent_scope,
            ..self.clone()
        }
    }

    pub fn with_own_scope(&self, own_scope: Option<SharedScope>) -> Self {
        Self {
            own_scope,
            ..self.clone()
        }
    }

    pub fn with_parent_expr(&self, parent_expr: Option<&Expr>) -> Self {
        Self {
            parent_expr: parent_expr.map(|expr| Box::new(expr.clone())),
            ..self.clone()
        }
    }

    pub fn parent_expr(&self) -> Option<&Expr> {
        self.parent_expr.as_deref()
    }

    pub fn parent_scope(&self) -> SharedScope {
        self.parent_scope.clone()
    }

    pub fn own_scope(&self) -> Option<SharedScope> {
        self.own_scope.clone()
    }

    pub fn insert_parent_binding(&self, name: impl Into<String>, binding: PartialBindingWithMeta) {
        self.parent_scope.borrow_mut().insert(name.into(), binding);
    }

    pub fn insert_own_binding(&self, name: impl Into<String>, binding: PartialBindingWithMeta) {
        if let Some(scope) = &self.own_scope {
            scope.borrow_mut().insert(name.into(), binding);
        }
    }

    pub fn allocate_own_scope(&self) -> SharedScope {
        new_scope()
    }
}

/// Tag information used when building styled components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagType {
    InBuiltComponent,
    UserDefinedComponent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub tag_type: TagType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use swc_core::common::sync::Lrc;
    use swc_core::common::{BytePos, SourceMap};

    #[test]
    fn merges_default_import_sources_with_relative_entries() {
        let cm: Lrc<SourceMap> = Default::default();
        let cwd = env::current_dir().expect("current dir");
        let root = cwd.join("compiled-tests");

        let file = TransformFile::with_options(
            cm,
            Vec::new(),
            TransformFileOptions {
                cwd: Some(cwd.clone()),
                root: Some(root.clone()),
                filename: Some(root.join("file.tsx").to_string_lossy().into_owned()),
                ..TransformFileOptions::default()
            },
        );

        let options = PluginOptions {
            import_sources: Some(vec!["./relative/module".into(), "@scope/package".into()]),
            ..PluginOptions::default()
        };

        let state = TransformState::new(file, options);

        let mut expected = DEFAULT_IMPORT_SOURCES
            .iter()
            .map(|source| source.to_string())
            .collect::<Vec<_>>();
        expected.push(root.join("relative/module").to_string_lossy().into_owned());
        expected.push("@scope/package".into());

        assert_eq!(state.import_sources, expected);
    }

    #[test]
    fn normalizes_relative_resolver_modules_against_root() {
        let cm: Lrc<SourceMap> = Default::default();
        let cwd = env::current_dir().expect("current dir");
        let root = cwd.join("resolver-root");

        let file = TransformFile::with_options(
            cm,
            Vec::new(),
            TransformFileOptions {
                cwd: Some(cwd.clone()),
                root: Some(root.clone()),
                ..TransformFileOptions::default()
            },
        );

        let options = PluginOptions {
            resolver: Some(ResolverOption::Module("./custom/resolver.js".into())),
            ..PluginOptions::default()
        };

        let state = TransformState::new(file, options);
        let resolver = state.resolver.expect("resolver should be initialized");
        let expected = root
            .join("custom/resolver.js")
            .to_string_lossy()
            .into_owned();

        assert_eq!(resolver.as_module(), Some(expected.as_str()));
    }

    #[test]
    fn initializes_cache_based_on_behavior_flag() {
        let cm: Lrc<SourceMap> = Default::default();
        let file = TransformFile::new(cm, Vec::new());

        let options = PluginOptions {
            cache: Some(CacheBehavior::Enabled(true)),
            ..PluginOptions::default()
        };

        let mut state = TransformState::new(file, options);

        let inserted = state
            .cache
            .load(Some("namespace"), "cache-key", || Value::from("first"));
        assert_eq!(inserted, Value::from("first"));

        let cached = state
            .cache
            .load(Some("namespace"), "cache-key", || Value::from("second"));
        assert_eq!(cached, Value::from("first"));
    }

    #[test]
    fn transform_cache_tracks_spans() {
        let mut cache = TransformCache::default();
        let span = Span::new(BytePos(1), BytePos(5));

        assert!(!cache.has(span));
        cache.set(span);
        assert!(cache.has(span));
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
