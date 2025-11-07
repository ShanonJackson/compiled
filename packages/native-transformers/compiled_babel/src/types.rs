use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use swc_core::ecma::ast::Program;

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

/// Rust representation of the Babel plugin options.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginOptions {
    pub cache: Option<CacheBehavior>,
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
}

impl Default for PluginOptions {
    fn default() -> Self {
        Self {
            cache: None,
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
        }
    }
}

/// Representation of a compiled style rule emitted by the transformer.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StyleRule {
    pub class_name: String,
    pub selector: String,
    pub css_text: String,
}

/// Metadata returned from the transform.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformMetadata {
    pub included_files: Vec<String>,
    pub style_rules: Vec<StyleRule>,
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
