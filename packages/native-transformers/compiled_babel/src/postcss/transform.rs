use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexSet;
use swc_core::common::{input::StringInput, FileName, SourceMap};
use swc_core::css::ast::Stylesheet;
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};
use swc_core::css::parser::{parse_string_input, parser::ParserConfig};

#[cfg(feature = "postcss_engine")]
use super::postcss_pipeline::transform_css_via_postcss;

use super::plugins::discard_comments::collect_preserved_comments;
use super::plugins::{
    atomicify_rules::atomicify_rules, discard_duplicates::discard_duplicates,
    discard_empty_rules::discard_empty_rules, expand_shorthands::index::expand_shorthands,
    extract_stylesheets::extract_stylesheets,
    flatten_multiple_selectors::flatten_multiple_selectors,
    increase_specificity::increase_specificity, nested::nested, normalize_css::normalize_css,
    normalize_whitespace::normalize_whitespace, parent_orphaned_pseudos::parent_orphaned_pseudos,
    sort_atomic_style_sheet::sort_atomic_style_sheet,
};

/// Options equivalent to `packages/css/src/transform.ts`.
#[derive(Debug, Clone, Default)]
pub struct TransformCssOptions {
    pub optimize_css: Option<bool>,
    pub class_name_compression_map: Option<HashMap<String, String>>,
    pub increase_specificity: Option<bool>,
    pub sort_at_rules: Option<bool>,
    pub sort_shorthand: Option<bool>,
    pub class_hash_prefix: Option<String>,
    pub flatten_multiple_selectors: Option<bool>,
    pub declaration_placeholder: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransformCssResult {
    pub sheets: Vec<String>,
    pub class_names: Vec<String>,
}

#[derive(Debug)]
pub struct CssTransformError {
    message: String,
}

impl CssTransformError {
    pub(crate) fn from_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn parser_error(css: &str, message: impl Into<String>) -> Self {
        let mut formatted = String::new();
        formatted.push_str(
            "An unhandled exception was raised when parsing your CSS, this is probably a bug!\n",
        );
        formatted.push_str(
            "Raise an issue here: https://github.com/atlassian-labs/compiled/issues/new?assignees=&labels=&template=bug_report.md&title=CSS%20Parsing%20Exception:\n\n",
        );
        formatted.push_str("Input CSS: {\n");
        formatted.push_str(css);
        formatted.push_str("\n}\n\n");
        formatted.push_str("Exception: ");
        formatted.push_str(&message.into());

        Self { message: formatted }
    }
}

impl std::fmt::Display for CssTransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CssTransformError {}

/// Execution context shared across plugins.
#[derive(Debug)]
pub struct TransformContext<'a> {
    pub options: &'a TransformCssOptions,
    pub sheets: Vec<String>,
    preserved_comments: Vec<String>,
    class_names: IndexSet<String>,
}

impl<'a> TransformContext<'a> {
    pub fn new(options: &'a TransformCssOptions) -> Self {
        Self {
            options,
            sheets: Vec::new(),
            preserved_comments: Vec::new(),
            class_names: IndexSet::new(),
        }
    }

    pub fn push_class_name(&mut self, class_name: impl Into<String>) {
        self.class_names.insert(class_name.into());
    }

    pub fn push_sheet(&mut self, sheet: impl Into<String>) {
        self.sheets.push(sheet.into());
    }

    pub fn set_preserved_comments(&mut self, comments: Vec<String>) {
        self.preserved_comments = comments;
    }

    pub fn take_preserved_comments(&mut self) -> Vec<String> {
        std::mem::take(&mut self.preserved_comments)
    }

    pub fn finish(self) -> TransformCssResult {
        TransformCssResult {
            sheets: self.sheets,
            class_names: self.class_names.into_iter().collect(),
        }
    }
}

/// Trait implemented by every plugin translation.
pub trait Plugin {
    fn name(&self) -> &'static str;
    fn run(&self, stylesheet: &mut Stylesheet, ctx: &mut TransformContext<'_>);
}

/// Parse CSS source into an AST using swc's CSS parser.
fn parse_stylesheet(css: &str) -> Result<Stylesheet, CssTransformError> {
    let cm: Arc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom("inline.css".into()).into(), css.into());
    let mut errors = vec![];
    match parse_string_input::<Stylesheet>(
        StringInput::from(&*fm),
        None,
        ParserConfig::default(),
        &mut errors,
    ) {
        Ok(stylesheet) => {
            if let Some(error) = errors.into_iter().next() {
                let message = format!("{error:?}");
                Err(CssTransformError::parser_error(css, message))
            } else {
                Ok(stylesheet)
            }
        }
        Err(err) => Err(CssTransformError::parser_error(css, format!("{err:?}"))),
    }
}

/// Serialize a stylesheet back to CSS text.
fn serialize_stylesheet(stylesheet: &Stylesheet) -> Result<String, CssTransformError> {
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });
        generator.emit(stylesheet).map_err(|err| {
            CssTransformError::from_message(format!("failed to serialize stylesheet: {err}"))
        })?;
    }
    Ok(output)
}

/// Execute the CSS pipeline.
pub(crate) fn transform_css_via_swc_pipeline(
    css: &str,
    mut options: TransformCssOptions,
) -> Result<TransformCssResult, CssTransformError> {
    let preserved_comments = collect_preserved_comments(css, options.optimize_css);
    let mut stylesheet = match parse_stylesheet(css) {
        Ok(sheet) => sheet,
        Err(original_err) => {
            const PLACEHOLDER: &str = "__compiled_declaration_wrapper__";
            let wrapped = format!(".{PLACEHOLDER} {{{}}}", css);
            match parse_stylesheet(&wrapped) {
                Ok(sheet) => {
                    options.declaration_placeholder = Some(format!(".{PLACEHOLDER}"));
                    sheet
                }
                Err(_) => return Err(original_err),
            }
        }
    };
    let mut ctx = TransformContext::new(&options);
    ctx.set_preserved_comments(preserved_comments);

    let flatten_multiple_selectors_option = options.flatten_multiple_selectors.unwrap_or(true);

    let mut pipeline: Vec<Box<dyn Plugin>> = Vec::new();
    pipeline.push(Box::new(discard_duplicates()));
    pipeline.push(Box::new(discard_empty_rules()));
    pipeline.push(Box::new(parent_orphaned_pseudos()));
    pipeline.push(Box::new(nested()));

    for plugin in normalize_css(&options) {
        pipeline.push(plugin);
    }
    // COMPAT: Run minimal color minification before hashing so value-based
    // class name hashes match Babel (which normalizes colors pre-atomicify).
    pipeline.push(Box::new(super::plugins::colormin_lite::colormin_lite()));
    pipeline.push(Box::new(expand_shorthands()));
    pipeline.push(Box::new(atomicify_rules()));

    if flatten_multiple_selectors_option {
        pipeline.push(Box::new(flatten_multiple_selectors()));
        pipeline.push(Box::new(discard_duplicates()));
    }

    if options.increase_specificity.unwrap_or(false) {
        pipeline.push(Box::new(increase_specificity()));
    }

    let sort_at_rules_option = options.sort_at_rules;
    let sort_shorthand_option = options.sort_shorthand;
    pipeline.push(Box::new(sort_atomic_style_sheet(
        sort_at_rules_option,
        sort_shorthand_option,
    )));

    // Autoprefixer will be wired in when its port lands.
    pipeline.push(Box::new(normalize_whitespace()));
    pipeline.push(Box::new(extract_stylesheets()));

    for plugin in pipeline {
        plugin.run(&mut stylesheet, &mut ctx);
    }

    let serialized_stylesheet = serialize_stylesheet(&stylesheet)?;
    let preserved_comments = ctx.take_preserved_comments();

    let comment_block = preserved_comments.concat();

    if ctx.sheets.is_empty() {
        let mut sheet = serialized_stylesheet;
        if !comment_block.is_empty() {
            sheet = format!("{}{}", comment_block, sheet);
        }
        ctx.push_sheet(sheet);
    } else {
        if !comment_block.is_empty() {
            if let Some(first) = ctx.sheets.first_mut() {
                first.insert_str(0, &comment_block);
            }
        }

        if ctx.sheets.iter().any(|sheet| sheet.is_empty()) {
            for sheet in &mut ctx.sheets {
                if sheet.is_empty() {
                    *sheet = serialized_stylesheet.clone();
                }
            }
        }
    }

    Ok(ctx.finish())
}

/// Execute the CSS pipeline.
pub fn transform_css(
    css: &str,
    options: TransformCssOptions,
) -> Result<TransformCssResult, CssTransformError> {
    // Default to the PostCSS engine-backed pipeline when available.
    #[cfg(feature = "postcss_engine")]
    {
        if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] via-postcss begin"); }
        let r = transform_css_via_postcss(css, options);
        if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] via-postcss end"); }
        return r;
    }
    transform_css_via_swc_pipeline(css, options)
}

/// Legacy Babel plugin name used in error reporting.
#[allow(dead_code)]
const FALLBACK_PLUGIN_NAME: &str = "@compiled/postcss";
