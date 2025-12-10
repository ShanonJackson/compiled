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
  extract_stylesheets::extract_stylesheets, flatten_multiple_selectors::flatten_multiple_selectors,
  increase_specificity::increase_specificity, nested::nested, normalize_css::normalize_css,
  normalize_whitespace::normalize_whitespace, parent_orphaned_pseudos::parent_orphaned_pseudos,
  sort_atomic_style_sheet::sort_atomic_style_sheet,
};
use std::env;

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
    let raw = sheet.into();
    let normalized =
      crate::postcss::plugins::extract_stylesheets::normalize_block_value_spacing(&raw);
    self.sheets.push(normalized);
  }

  pub fn set_preserved_comments(&mut self, comments: Vec<String>) {
    self.preserved_comments = comments;
  }

  pub fn take_preserved_comments(&mut self) -> Vec<String> {
    std::mem::take(&mut self.preserved_comments)
  }

  fn first_class_from_sheet(sheet: &str) -> Option<String> {
    // Find first '.' and read until '{', whitespace, or comma.
    let dot = sheet.find('.')?;
    let rest = &sheet[dot + 1..];
    let end = rest
      .find(|c: char| c == '{' || c == ' ' || c == ',')
      .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() {
      None
    } else {
      Some(name.to_string())
    }
  }

  fn reorder_class_names_by_sheets(mut class_names: Vec<String>, sheets: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let mut ordered_keys: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for sheet in sheets {
      if let Some(class_name) = Self::first_class_from_sheet(sheet) {
        if seen.insert(class_name.clone()) {
          ordered_keys.push(class_name);
        }
      }
    }

    if ordered_keys.is_empty() {
      return class_names;
    }

    let mut included: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::with_capacity(class_names.len());

    // Add in the order classes appear in sheets
    for key in ordered_keys {
      if let Some(pos) = class_names.iter().position(|c| c == &key) {
        let name = class_names.remove(pos);
        included.insert(name.clone());
        result.push(name);
      }
    }

    // Append any remaining classes preserving original encounter order
    for name in class_names.into_iter() {
      if !included.contains(&name) {
        result.push(name);
      }
    }

    result
  }

  pub fn finish(self) -> TransformCssResult {
    let sheets: Vec<String> = self
      .sheets
      .into_iter()
      .map(|mut sheet| {
        sheet = sheet.replace(" *", "*");
        sheet = sheet.replace("* ", "*");
        sheet = sheet.replace("*-", "* -");
        sheet = sheet.replace("*+", "* +");
        sheet
      })
      .collect();
    let encountered: Vec<String> = self.class_names.into_iter().collect();
    let class_names = Self::reorder_class_names_by_sheets(encountered, &sheets);
    TransformCssResult {
      sheets,
      class_names,
    }
  }
}

/// Trait implemented by every plugin translation.
pub trait Plugin {
  fn name(&self) -> &'static str;
  fn run(&self, stylesheet: &mut Stylesheet, ctx: &mut TransformContext<'_>);
}

#[derive(Debug, Clone, Copy)]
struct NoopPlugin {
  name: &'static str,
}

impl Plugin for NoopPlugin {
  fn name(&self) -> &'static str {
    self.name
  }

  fn run(&self, _stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {}
}

#[derive(Debug, Clone, Default)]
struct PluginGate {
  target: Option<String>,
}

impl PluginGate {
  fn new() -> Self {
    let target = env::var("POSTCSS_PLUGIN_UNDER_TEST")
      .ok()
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty());
    Self { target }
  }

  fn is_active(&self) -> bool {
    self.target.is_some()
  }

  fn matches(&self, name: &str) -> bool {
    self
      .target
      .as_ref()
      .map(|target| target == name)
      .unwrap_or(false)
  }

  fn force_enable(&self, name: &str, fallback: bool) -> bool {
    self.matches(name) || fallback
  }

  fn gate(&self, name: &'static str, plugin: Box<dyn Plugin>) -> Box<dyn Plugin> {
    if !self.is_active() || self.matches(name) {
      plugin
    } else {
      Box::new(NoopPlugin { name })
    }
  }
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

  let gate = PluginGate::new();
  let flatten_multiple_selectors_option = gate.force_enable(
    "flatten-multiple-selectors",
    options.flatten_multiple_selectors.unwrap_or(true),
  );

  let mut pipeline: Vec<Box<dyn Plugin>> = Vec::new();
  {
    let plugin = Box::new(discard_duplicates());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(discard_empty_rules());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(parent_orphaned_pseudos());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(nested());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }

  for plugin in normalize_css(&options) {
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  // COMPAT: Run minimal color minification before hashing so value-based
  // class name hashes match Babel (which normalizes colors pre-atomicify).
  {
    let plugin = Box::new(super::plugins::colormin_lite::colormin_lite());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(expand_shorthands());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  if options.optimize_css.unwrap_or(true) {
    // COMPAT: Re-run reduce-initial after shorthands are expanded so properties
    // like text-decoration-color are normalized with the final values.
    let plugin = Box::new(super::plugins::reduce_initial::reduce_initial());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(atomicify_rules());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }

  if flatten_multiple_selectors_option {
    {
      let plugin = Box::new(flatten_multiple_selectors());
      let name = plugin.name();
      pipeline.push(gate.gate(name, plugin));
    }
    {
      let plugin = Box::new(discard_duplicates());
      let name = plugin.name();
      pipeline.push(gate.gate(name, plugin));
    }
  }

  if gate.force_enable("increase-specificity", options.increase_specificity.unwrap_or(false)) {
    let plugin = Box::new(increase_specificity());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }

  let sort_at_rules_option = options.sort_at_rules;
  let sort_shorthand_option = options.sort_shorthand;
  {
    let plugin = Box::new(sort_atomic_style_sheet(
      sort_at_rules_option,
      sort_shorthand_option,
    ));
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }

  // Autoprefixer-equivalent vendor prefixing must run after
  // sort-atomic-style-sheet and before whitespace/extract to match Babel.
  // Full Autoprefixer port (wired to browserslist and caniuse data)
  if gate.force_enable(
    "autoprefixer",
    env::var("AUTOPREFIXER")
      .map(|v| v != "off")
      .unwrap_or(true),
  ) {
    let plugin = Box::new(super::plugins::vendor_autoprefixer::vendor_autoprefixer());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(normalize_whitespace());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }
  {
    let plugin = Box::new(extract_stylesheets());
    let name = plugin.name();
    pipeline.push(gate.gate(name, plugin));
  }

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
  // Skip empty inputs (only whitespace/semicolons), mirroring Babel which would
  // produce no sheets/class names for an empty declaration block.
  let trimmed = css.trim();
  if trimmed.is_empty() || trimmed.chars().all(|c| c == ';') {
    return Ok(TransformCssResult {
      sheets: Vec::new(),
      class_names: Vec::new(),
    });
  }

  // Handle brace-wrapped declaration blocks (e.g. "{color:red;}") by stripping
  // the braces and reusing the SWC pipeline to mirror Babel's wrap-bare-decls.
  // Allow bare declarations by letting the PostCSS pipeline run with ignore_errors;
  // the wrap-bare-decls plugin will lift them into an empty-selector rule as Babel does.
  // No pre-wrapping here—feed the raw CSS through.

  if std::env::var("STACK_DEBUG").is_ok() {
    eprintln!("[transform_css] postcss path css=\"{}\"", css);
  }

  // Default to the PostCSS engine-backed pipeline when available.
  #[cfg(feature = "postcss_engine")]
  {
    // Reset plugin timing before run so we get per-call numbers.
    let _ = postcss::metrics::take_plugin_ns();
    let t0 = std::time::Instant::now();
    if std::env::var("COMPILED_CLI_TRACE").is_ok() {
      eprintln!("[postcss] via-postcss begin");
    }
    let r = transform_css_via_postcss(css, options);
    if std::env::var("COMPILED_CLI_TRACE").is_ok() {
      eprintln!("[postcss] via-postcss end");
    }
    super::metrics::record_postcss_ns(t0.elapsed().as_nanos() as u64);
    let plugin_ns = postcss::metrics::take_plugin_ns();
    super::metrics::record_postcss_plugin_ns(plugin_ns);
    return r;
  }
  #[cfg(not(feature = "postcss_engine"))]
  {
    return transform_css_via_swc_pipeline(css, options);
  }
}

/// Legacy Babel plugin name used in error reporting.
#[allow(dead_code)]
const FALLBACK_PLUGIN_NAME: &str = "@compiled/postcss";
