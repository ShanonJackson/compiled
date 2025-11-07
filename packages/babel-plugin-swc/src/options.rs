use serde::Deserialize;

/// Configuration accepted by the SWC variant of the Compiled plugin.
///
/// The structure intentionally mirrors the options supported by the
/// TypeScript implementation so that existing configuration files can be
/// reused without any translation layer. Additional options may be surfaced
/// over time as the port matures.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    #[serde(default)]
    pub cache: CacheMode,
    #[serde(default = "default_true")]
    pub import_react: bool,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub import_sources: Vec<String>,
    #[serde(default)]
    pub optimize_css: bool,
    #[serde(default)]
    pub resolver: Option<ResolverConfig>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub parser_babel_plugins: Vec<String>,
    #[serde(default)]
    pub add_component_name: bool,
    #[serde(default)]
    pub class_name_compression_map: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub process_xcss: bool,
    #[serde(default)]
    pub increase_specificity: bool,
    #[serde(default = "default_true")]
    pub sort_at_rules: bool,
    #[serde(default)]
    pub class_hash_prefix: Option<String>,
    #[serde(default = "default_true")]
    pub flatten_multiple_selectors: bool,
    #[serde(default)]
    pub extract: bool,
    #[serde(default)]
    pub style_sheet_path: Option<String>,
    #[serde(default)]
    pub compiled_require_exclude: bool,
    #[serde(default)]
    pub extract_styles_to_directory: Option<ExtractStylesToDirectory>,
    #[serde(default = "default_true")]
    pub sort_shorthand: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            cache: CacheMode::Disabled,
            import_react: default_true(),
            nonce: None,
            import_sources: Vec::new(),
            optimize_css: false,
            resolver: None,
            extensions: Vec::new(),
            parser_babel_plugins: Vec::new(),
            add_component_name: false,
            class_name_compression_map: None,
            process_xcss: default_true(),
            increase_specificity: false,
            sort_at_rules: default_true(),
            class_hash_prefix: None,
            flatten_multiple_selectors: default_true(),
            extract: false,
            style_sheet_path: None,
            compiled_require_exclude: false,
            extract_styles_to_directory: None,
            sort_shorthand: default_true(),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheMode {
    #[serde(rename = "file-pass")]
    FilePass,
    #[serde(other)]
    Disabled,
}

impl Default for CacheMode {
    fn default() -> Self {
        CacheMode::Disabled
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverConfig {
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub alias: Option<serde_json::Value>,
    #[serde(default)]
    pub alias_fields: Vec<Vec<String>>,
    #[serde(default)]
    pub condition_names: Vec<String>,
    #[serde(default)]
    pub extension_alias: Option<serde_json::Value>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub exports_fields: Vec<Vec<String>>,
    #[serde(default)]
    pub imports_fields: Vec<Vec<String>>,
    #[serde(default)]
    pub fallback: Option<serde_json::Value>,
    #[serde(default)]
    pub fully_specified: Option<bool>,
    #[serde(default)]
    pub main_fields: Vec<String>,
    #[serde(default)]
    pub main_files: Vec<String>,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub prefer_relative: Option<bool>,
    #[serde(default)]
    pub prefer_absolute: Option<bool>,
    #[serde(default)]
    pub restrictions: Vec<String>,
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub symlinks: Option<bool>,
    #[serde(default)]
    pub builtin_modules: Option<bool>,
    #[serde(default)]
    pub tsconfig: Option<serde_json::Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractStylesToDirectory {
    pub source: String,
    pub dest: String,
}

fn default_true() -> bool {
    true
}
