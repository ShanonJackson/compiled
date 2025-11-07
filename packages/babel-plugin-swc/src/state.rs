use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use oxc_resolver::{ResolveContext, Resolver};
use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::sync::Lrc;
use swc_core::common::SyntaxContext;
use swc_core::common::{FileName, SourceMap, DUMMY_SP};
use swc_core::ecma::ast::{
    ArrayLit, BinaryOp, CallExpr, Callee, CondExpr, EsVersion, Expr, ExprOrSpread, ExprStmt, Ident,
    ImportDecl, Lit, MemberExpr, MemberProp, Module, ModuleDecl, ModuleItem, ObjectLit, Prop,
    PropOrSpread, Stmt, Str, Tpl, UnaryOp,
};
use swc_core::ecma::parser::{parse_file_as_module, EsSyntax, Syntax, TsSyntax};

use crate::{
    css::property::{trim_number, CssObject, CssValue},
    options::{CacheMode, PluginConfig},
    postcss::{sort_atomic_style_sheet, SortConfig},
    resolver::create_resolver,
    types::{TransformMetadata, TransformResult},
    utils::{
        cache::{Cache, CacheOptions},
        encode::to_uri_component,
        wtf8::wtf8_to_string,
    },
};

#[derive(Debug, Clone)]
pub enum ImportKind {
    Default,
    Named(String),
}

#[derive(Debug, Clone)]
pub struct ImportBinding {
    pub source: String,
    pub kind: ImportKind,
}

#[derive(Debug, Clone, Default)]
struct ModuleAnalysis {
    locals: HashMap<String, CssValue>,
    exports: HashMap<String, CssValue>,
}

impl ModuleAnalysis {
    fn exported_value(&self, name: &str) -> Option<CssValue> {
        self.exports.get(name).cloned()
    }
}

#[derive(Debug, Clone)]
struct AggregatedAtRule {
    display_stack: Vec<String>,
    body: String,
    index: Option<usize>,
}

#[derive(Debug, Clone)]
enum ClassRuleEntry {
    Standalone(String),
    Aggregated(Vec<String>),
}

fn wrap_rule_stack(stack: &[String], body: &str) -> String {
    let mut rule = body.to_string();
    for at_rule in stack.iter().rev() {
        rule = format!("{at_rule}{{{rule}}}");
    }

    rule
}

#[allow(dead_code)]
pub struct TransformState {
    pub config: PluginConfig,
    resolver: Resolver,
    pub metadata: TransformMetadata,
    included_files: Vec<PathBuf>,
    style_rules: Vec<String>,
    seen_style_rules: HashSet<String>,
    aggregated_at_rules: HashMap<Vec<String>, AggregatedAtRule>,
    seen_atomic_selectors: HashSet<String>,
    css_idents: HashSet<String>,
    styled_idents: HashSet<String>,
    keyframes_idents: HashSet<String>,
    class_names_idents: HashSet<String>,
    css_map_idents: HashSet<String>,
    runtime_class_library_used: bool,
    runtime_components_used: bool,
    uses_xcss: bool,
    styled_display_names: HashSet<String>,
    class_rules: HashMap<String, ClassRuleEntry>,
    css_map_sheets: HashMap<String, Vec<String>>,
    pending_css_map_calls: HashMap<String, CallExpr>,
    import_bindings: HashMap<String, ImportBinding>,
    resolved_imports: HashMap<String, CssValue>,
    local_bindings: HashMap<String, CssValue>,
    file_cache: Cache<String>,
    module_cache: Cache<Module>,
    module_analysis_cache: Cache<ModuleAnalysis>,
}

#[allow(dead_code)]
impl TransformState {
    pub fn new(config: PluginConfig, metadata: TransformMetadata) -> Self {
        let cache_enabled = matches!(config.cache, CacheMode::FilePass);
        let cache_options = CacheOptions {
            cache: cache_enabled,
            ..CacheOptions::default()
        };

        let mut file_cache = Cache::new();
        let mut module_cache = Cache::new();
        let mut module_analysis_cache = Cache::new();

        file_cache.initialize(cache_options.clone());
        module_cache.initialize(cache_options.clone());
        module_analysis_cache.initialize(cache_options);

        let resolver = create_resolver(config.resolver.as_ref(), metadata.root_dir.as_deref());

        Self {
            config,
            resolver,
            metadata,
            included_files: Vec::new(),
            style_rules: Vec::new(),
            seen_style_rules: HashSet::new(),
            aggregated_at_rules: HashMap::new(),
            seen_atomic_selectors: HashSet::new(),
            css_idents: HashSet::new(),
            styled_idents: HashSet::new(),
            keyframes_idents: HashSet::new(),
            class_names_idents: HashSet::new(),
            css_map_idents: HashSet::new(),
            runtime_class_library_used: false,
            runtime_components_used: false,
            uses_xcss: false,
            styled_display_names: HashSet::new(),
            class_rules: HashMap::new(),
            css_map_sheets: HashMap::new(),
            pending_css_map_calls: HashMap::new(),
            import_bindings: HashMap::new(),
            resolved_imports: HashMap::new(),
            local_bindings: HashMap::new(),
            file_cache,
            module_cache,
            module_analysis_cache,
        }
    }

    pub fn begin_pass(&mut self) {
        self.class_rules.clear();
        self.css_map_sheets.clear();
        self.aggregated_at_rules.clear();
        self.seen_atomic_selectors.clear();
        self.runtime_class_library_used = false;
        self.runtime_components_used = false;
        self.uses_xcss = false;
        self.pending_css_map_calls.clear();
        self.styled_display_names.clear();
    }

    pub fn resolver(&self) -> &Resolver {
        &self.resolver
    }

    pub fn source_map(&self) -> Option<&SourceMap> {
        self.metadata.source_map.as_deref()
    }

    pub fn comments(&self) -> Option<&SingleThreadedComments> {
        self.metadata.comments.as_deref()
    }

    pub fn record_included_file(&mut self, file: impl Into<PathBuf>) {
        self.included_files.push(file.into());
    }

    pub fn push_style_rule(&mut self, rule: impl Into<String>) {
        if !self.config.extract {
            return;
        }

        let rule = rule.into();
        if self.seen_style_rules.insert(rule.clone()) {
            self.style_rules.push(rule);
        }
    }

    pub fn record_atomic_style(
        &mut self,
        at_rule_stack: Vec<String>,
        at_rule_hash_key: String,
        selector_rule: &str,
    ) -> String {
        let signature = format!("{at_rule_hash_key}|{selector_rule}");
        if !self.seen_atomic_selectors.insert(signature) {
            return self.current_wrapped_rule(&at_rule_stack, selector_rule);
        }

        if at_rule_stack.is_empty() {
            let rule = selector_rule.to_string();
            if self.config.extract {
                if self.seen_style_rules.insert(rule.clone()) {
                    self.style_rules.push(rule.clone());
                }
            }
            return rule;
        }

        let entry = self
            .aggregated_at_rules
            .entry(at_rule_stack.clone())
            .or_insert_with(|| AggregatedAtRule {
                display_stack: at_rule_stack.clone(),
                body: String::new(),
                index: None,
            });

        entry.body.push_str(selector_rule);

        let wrapped = wrap_rule_stack(&entry.display_stack, &entry.body);

        if self.config.extract {
            match entry.index {
                Some(index) => {
                    self.style_rules[index] = wrapped.clone();
                }
                None => {
                    entry.index = Some(self.style_rules.len());
                    self.style_rules.push(wrapped.clone());
                }
            }
        }

        wrapped
    }

    fn current_wrapped_rule(&self, at_rule_stack: &[String], selector_rule: &str) -> String {
        if at_rule_stack.is_empty() {
            return selector_rule.to_string();
        }

        if let Some(entry) = self.aggregated_at_rules.get(at_rule_stack) {
            wrap_rule_stack(&entry.display_stack, &entry.body)
        } else {
            wrap_rule_stack(at_rule_stack, selector_rule)
        }
    }

    pub fn register_css_ident(&mut self, ident: impl Into<String>) {
        self.css_idents.insert(ident.into());
    }

    pub fn is_css_ident(&self, ident: &str) -> bool {
        self.css_idents.contains(ident)
    }

    pub fn register_styled_ident(&mut self, ident: impl Into<String>) {
        self.styled_idents.insert(ident.into());
    }

    pub fn is_styled_ident(&self, ident: &str) -> bool {
        self.styled_idents.contains(ident)
    }

    pub fn register_keyframes_ident(&mut self, ident: impl Into<String>) {
        self.keyframes_idents.insert(ident.into());
    }

    pub fn is_keyframes_ident(&self, ident: &str) -> bool {
        self.keyframes_idents.contains(ident)
    }

    pub fn register_class_names_ident(&mut self, ident: impl Into<String>) {
        self.class_names_idents.insert(ident.into());
    }

    pub fn is_class_names_ident(&self, ident: &str) -> bool {
        self.class_names_idents.contains(ident)
    }

    pub fn register_css_map_ident(&mut self, ident: impl Into<String>) {
        self.css_map_idents.insert(ident.into());
    }

    pub fn is_css_map_ident(&self, ident: &str) -> bool {
        self.css_map_idents.contains(ident)
    }

    pub fn record_class_rule(&mut self, class_name: &str, at_rule_stack: &[String], rule: &str) {
        let entry = if at_rule_stack.is_empty() {
            ClassRuleEntry::Standalone(rule.to_string())
        } else {
            ClassRuleEntry::Aggregated(at_rule_stack.to_vec())
        };

        self.class_rules.insert(class_name.to_string(), entry);
    }

    pub fn mark_runtime_class_library_used(&mut self) {
        self.runtime_class_library_used = true;
    }

    pub fn runtime_class_library_used(&self) -> bool {
        self.runtime_class_library_used
    }

    pub fn mark_runtime_components_used(&mut self) {
        self.runtime_components_used = true;
    }

    pub fn runtime_components_used(&self) -> bool {
        self.runtime_components_used
    }

    pub fn mark_uses_xcss(&mut self) {
        self.uses_xcss = true;
    }

    pub fn uses_xcss(&self) -> bool {
        self.uses_xcss
    }

    pub fn record_styled_display_name(&mut self, name: impl Into<String>) {
        self.styled_display_names.insert(name.into());
    }

    pub fn take_styled_display_names(&mut self) -> HashSet<String> {
        std::mem::take(&mut self.styled_display_names)
    }

    pub fn runtime_class_library_ident(&self) -> &str {
        if self
            .config
            .class_name_compression_map
            .as_ref()
            .map(|value| !value.is_null())
            .unwrap_or(false)
        {
            "ac"
        } else {
            "ax"
        }
    }

    pub fn lookup_class_rule(&self, class_name: &str) -> Option<String> {
        match self.class_rules.get(class_name)? {
            ClassRuleEntry::Standalone(rule) => Some(rule.clone()),
            ClassRuleEntry::Aggregated(stack) => self
                .aggregated_at_rules
                .get(stack)
                .map(|entry| wrap_rule_stack(&entry.display_stack, &entry.body)),
        }
    }

    pub fn record_css_map_sheets(&mut self, binding: impl Into<String>, sheets: Vec<String>) {
        self.css_map_sheets.insert(binding.into(), sheets);
    }

    pub fn css_map_sheets(&self) -> &HashMap<String, Vec<String>> {
        &self.css_map_sheets
    }

    pub fn record_pending_css_map_call(&mut self, binding: impl Into<String>, call: CallExpr) {
        self.pending_css_map_calls.insert(binding.into(), call);
    }

    pub fn pending_css_map_call(&self, binding: &str) -> Option<&CallExpr> {
        self.pending_css_map_calls.get(binding)
    }

    pub fn remove_pending_css_map_call(&mut self, binding: &str) {
        self.pending_css_map_calls.remove(binding);
    }

    pub fn register_import_binding(&mut self, local: impl Into<String>, binding: ImportBinding) {
        self.import_bindings.insert(local.into(), binding);
    }

    pub fn register_local_binding(&mut self, name: impl Into<String>, value: CssValue) {
        self.local_bindings.insert(name.into(), value);
    }

    pub fn lookup_local_binding(&self, name: &str) -> Option<&CssValue> {
        self.local_bindings.get(name)
    }

    pub fn resolve_identifier_value(&mut self, name: &str) -> Option<CssValue> {
        if let Some(value) = self.local_bindings.get(name) {
            return Some(value.clone());
        }

        if let Some(value) = self.resolved_imports.get(name) {
            return Some(value.clone());
        }

        let binding = self.import_bindings.get(name)?.clone();
        let module_path = self.resolve_module_path(&binding.source).ok()?;
        let export_name = match binding.kind {
            ImportKind::Default => "default".to_string(),
            ImportKind::Named(name) => name,
        };

        let exports = self.load_module_analysis(&module_path).ok()?;
        let value = exports.exported_value(&export_name)?;
        self.resolved_imports
            .insert(name.to_string(), value.clone());
        Some(value)
    }

    pub fn finalize(&mut self, mut program: swc_core::ecma::ast::Program) -> TransformResult {
        if self.config.extract {
            if let swc_core::ecma::ast::Program::Module(module) = &mut program {
                self.insert_style_sheet_requires(module);
                self.extract_styles_to_directory(module);
            }
        }

        TransformResult {
            program,
            style_rules: std::mem::take(&mut self.style_rules),
            included_files: std::mem::take(&mut self.included_files),
        }
    }

    fn insert_style_sheet_requires(&self, module: &mut Module) {
        if self.config.compiled_require_exclude {
            return;
        }

        let Some(style_sheet_path) = &self.config.style_sheet_path else {
            return;
        };

        if self.style_rules.is_empty() {
            return;
        }

        let mut original_body = std::mem::take(&mut module.body);
        let mut new_body = Vec::with_capacity(self.style_rules.len() + original_body.len());

        for rule in &self.style_rules {
            let encoded = to_uri_component(rule);
            let specifier = format!("{style_sheet_path}?style={encoded}");

            let require_call = Expr::Call(CallExpr {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                args: vec![ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Lit(Lit::Str(Str {
                        span: DUMMY_SP,
                        value: specifier.into(),
                        raw: None,
                    }))),
                }],
                callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(
                    "require".into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                )))),
                type_args: None,
            });

            new_body.push(ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(require_call),
            })));
        }

        new_body.append(&mut original_body);
        module.body = new_body;
    }

    fn extract_styles_to_directory(&self, module: &mut Module) {
        let Some(config) = &self.config.extract_styles_to_directory else {
            return;
        };

        if self.style_rules.is_empty() {
            return;
        }

        let filename = self
            .metadata
            .filename
            .as_ref()
            .expect("Source filename was not defined");
        let source_file_name = self
            .metadata
            .source_file_name
            .as_ref()
            .or_else(|| self.metadata.filename.as_ref())
            .expect("Source filename was not defined");
        let cwd = self
            .metadata
            .root_dir
            .as_ref()
            .expect("extractStylesToDirectory requires root_dir metadata");

        let css_filename = filename
            .file_stem()
            .map(|stem| format!("{}.compiled.css", stem.to_string_lossy()))
            .expect("Unable to determine CSS filename for extraction");

        let source_str = source_file_name.to_string_lossy();
        let source = &config.source;
        let Some(index) = source_str.find(source) else {
            panic!(
                "{}: Source directory '{}' was not found relative to source file ('{}')",
                filename.display(),
                source,
                source_str,
            );
        };

        let relative_path = &source_str[index + source.len()..];
        let mut css_path = cwd.join(&config.dest);

        if let Some(parent) = Path::new(relative_path).parent() {
            if !parent.as_os_str().is_empty() && parent != Path::new(".") {
                css_path = css_path.join(parent);
            }
        }

        css_path = css_path.join(&css_filename);

        if let Some(dir) = css_path.parent() {
            fs::create_dir_all(dir).unwrap_or_else(|err| {
                panic!("failed to create stylesheet directory {:?}: {err}", dir)
            });
        }

        let sort_config = SortConfig {
            sort_at_rules_enabled: self.config.sort_at_rules,
            sort_shorthand_enabled: self.config.sort_shorthand,
        };
        let stylesheet = sort_atomic_style_sheet(&self.style_rules, sort_config);

        fs::write(&css_path, stylesheet).unwrap_or_else(|err| {
            panic!("failed to write extracted stylesheet {:?}: {err}", css_path)
        });

        let import_src = format!("./{css_filename}");
        let import = ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
            span: DUMMY_SP,
            specifiers: Vec::new(),
            src: Box::new(Str {
                span: DUMMY_SP,
                value: import_src.into(),
                raw: None,
            }),
            type_only: false,
            with: None,
            phase: Default::default(),
        }));

        module.body.insert(0, import);
    }

    fn resolve_module_path(&mut self, request: &str) -> Result<PathBuf> {
        let filename = self
            .metadata
            .filename
            .as_ref()
            .ok_or_else(|| anyhow!("Cannot resolve module without filename"))?;

        let directory = filename
            .parent()
            .ok_or_else(|| anyhow!("Input file has no parent directory"))?;

        let mut context = ResolveContext::default();
        let resolution = self
            .resolver
            .resolve_with_context(directory, request, &mut context)
            .map_err(|err| anyhow!(format!("{:?}", err)))?;

        for dependency in &context.file_dependencies {
            self.record_included_file(dependency.clone());
        }

        Ok(resolution.into_path_buf())
    }

    fn load_module_analysis(&mut self, path: &Path) -> Result<ModuleAnalysis> {
        let key = path.to_string_lossy().into_owned();

        if let Some(existing) = self
            .module_analysis_cache
            .get(Some("module-analysis"), &key)
        {
            return Ok(existing);
        }

        let module = self.load_module(path)?;
        let analysis = self.analyse_module(&module);
        self.module_analysis_cache
            .insert(Some("module-analysis"), &key, analysis.clone());

        Ok(analysis)
    }

    fn load_module(&mut self, path: &Path) -> Result<Module> {
        let path_buf = path.to_path_buf();
        let key = path.to_string_lossy().into_owned();
        self.record_included_file(path_buf.clone());

        let source = if let Some(existing) = self.file_cache.get(Some("read-file"), &key) {
            existing
        } else {
            let contents = fs::read_to_string(&path_buf)
                .map_err(|err| anyhow!("failed to read module {:?}: {err}", path_buf))?;
            self.file_cache
                .insert(Some("read-file"), &key, contents.clone());
            contents
        };

        if let Some(parsed) = self.module_cache.get(Some("parse-module"), &key) {
            return Ok(parsed);
        }

        let module = self.parse_module(&path_buf, &source)?;
        self.module_cache
            .insert(Some("parse-module"), &key, module.clone());

        Ok(module)
    }

    fn parse_module(&self, path: &Path, source: &str) -> Result<Module> {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), source.to_owned());
        let syntax = self.syntax_for_file(path);

        let mut errors = Vec::new();
        let module = parse_file_as_module(&fm, syntax, EsVersion::Es2022, None, &mut errors)
            .map_err(|err| anyhow!(format!("{:?}", err)))?;

        if let Some(error) = errors.into_iter().next() {
            return Err(anyhow!(format!("{:?}", error)));
        }

        Ok(module)
    }

    fn syntax_for_file(&self, path: &Path) -> Syntax {
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let plugins = &self.config.parser_babel_plugins;
        let has_ts_plugin = plugins.iter().any(|plugin| plugin == "typescript");
        let has_jsx_plugin = plugins.iter().any(|plugin| plugin == "jsx");
        let has_decorators = plugins.iter().any(|plugin| plugin == "decorators");

        let is_ts = matches!(ext, "ts" | "tsx") || has_ts_plugin;
        let is_jsx = matches!(ext, "tsx" | "jsx") || has_jsx_plugin;

        if is_ts {
            Syntax::Typescript(TsSyntax {
                tsx: is_jsx,
                decorators: has_decorators,
                ..Default::default()
            })
        } else {
            Syntax::Es(EsSyntax {
                jsx: is_jsx,
                decorators: has_decorators,
                ..Default::default()
            })
        }
    }

    fn analyse_module(&self, module: &Module) -> ModuleAnalysis {
        let mut analysis = ModuleAnalysis::default();

        for item in &module.body {
            match item {
                swc_core::ecma::ast::ModuleItem::Stmt(stmt) => {
                    if let swc_core::ecma::ast::Stmt::Decl(decl) = stmt {
                        if let swc_core::ecma::ast::Decl::Var(var) = decl {
                            self.collect_var_decl(var, false, &mut analysis);
                        }
                    }
                }
                swc_core::ecma::ast::ModuleItem::ModuleDecl(decl) => match decl {
                    swc_core::ecma::ast::ModuleDecl::ExportDecl(export_decl) => {
                        if let swc_core::ecma::ast::Decl::Var(var) = &export_decl.decl {
                            self.collect_var_decl(var, true, &mut analysis);
                        }
                    }
                    swc_core::ecma::ast::ModuleDecl::ExportNamed(named) => {
                        if named.src.is_none() {
                            for spec in &named.specifiers {
                                if let swc_core::ecma::ast::ExportSpecifier::Named(named_spec) =
                                    spec
                                {
                                    let local_name = match &named_spec.orig {
                                        swc_core::ecma::ast::ModuleExportName::Ident(ident) => {
                                            ident.sym.to_string()
                                        }
                                        swc_core::ecma::ast::ModuleExportName::Str(str) => {
                                            wtf8_to_string(&str.value)
                                        }
                                    };

                                    if let Some(value) = analysis.locals.get(&local_name).cloned() {
                                        let export_name = named_spec
                                            .exported
                                            .as_ref()
                                            .map(|name| match name {
                                                swc_core::ecma::ast::ModuleExportName::Ident(
                                                    ident,
                                                ) => ident.sym.to_string(),
                                                swc_core::ecma::ast::ModuleExportName::Str(str) => {
                                                    wtf8_to_string(&str.value)
                                                }
                                            })
                                            .unwrap_or_else(|| local_name.clone());

                                        analysis.exports.insert(export_name, value);
                                    }
                                }
                            }
                        }
                    }
                    swc_core::ecma::ast::ModuleDecl::ExportDefaultExpr(default_expr) => {
                        if let Some(value) =
                            self.evaluate_static_expr(&default_expr.expr, &analysis)
                        {
                            analysis.exports.insert("default".to_string(), value);
                        }
                    }
                    _ => {}
                },
            }
        }

        analysis
    }

    fn collect_var_decl(
        &self,
        var: &swc_core::ecma::ast::VarDecl,
        exported: bool,
        analysis: &mut ModuleAnalysis,
    ) {
        if var.kind != swc_core::ecma::ast::VarDeclKind::Const {
            return;
        }

        for decl in &var.decls {
            if let swc_core::ecma::ast::Pat::Ident(ident) = &decl.name {
                if let Some(init) = &decl.init {
                    if let Some(value) = self.evaluate_static_expr(init, analysis) {
                        let name = ident.id.sym.to_string();
                        analysis.locals.insert(name.clone(), value.clone());
                        if exported {
                            analysis.exports.insert(name, value);
                        }
                    }
                }
            }
        }
    }

    fn evaluate_static_expr(
        &self,
        expr: &swc_core::ecma::ast::Expr,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        match expr {
            Expr::Paren(paren) => self.evaluate_static_expr(&paren.expr, analysis),
            Expr::Lit(Lit::Str(str)) => Some(CssValue::String(wtf8_to_string(&str.value))),
            Expr::Lit(Lit::Num(num)) => Some(CssValue::Number(num.value)),
            Expr::Lit(Lit::Bool(b)) => Some(CssValue::Bool(b.value)),
            Expr::Lit(Lit::Null(_)) => Some(CssValue::Null),
            Expr::Tpl(tpl) => self.evaluate_static_template_literal(tpl, analysis),
            Expr::TsAs(ts_as) => self.evaluate_static_expr(&ts_as.expr, analysis),
            Expr::TsConstAssertion(assert) => self.evaluate_static_expr(&assert.expr, analysis),
            Expr::TsNonNull(non_null) => self.evaluate_static_expr(&non_null.expr, analysis),
            Expr::TsTypeAssertion(assert) => self.evaluate_static_expr(&assert.expr, analysis),
            Expr::Object(object) => self.evaluate_object_literal(object, analysis),
            Expr::Array(array) => self.evaluate_array_literal(array, analysis),
            Expr::Member(member) => self.evaluate_member_expr(member, analysis),
            Expr::Ident(ident) => analysis.locals.get(ident.sym.as_ref()).cloned(),
            Expr::Call(call) => self.evaluate_static_call_expr(call, analysis),
            Expr::Cond(cond) => self.evaluate_static_cond_expr(cond, analysis),
            Expr::Unary(unary) => {
                let value = self.evaluate_static_expr(&unary.arg, analysis)?;
                evaluate_unary_expression(unary.op, value)
            }
            Expr::Bin(bin) => {
                let left = self.evaluate_static_expr(&bin.left, analysis)?;
                let right = self.evaluate_static_expr(&bin.right, analysis)?;
                evaluate_binary_expression(bin.op, left, right)
            }
            _ => None,
        }
    }

    fn evaluate_object_literal(
        &self,
        object: &ObjectLit,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        let mut map: CssObject = CssObject::new();

        for prop in &object.props {
            match prop {
                PropOrSpread::Prop(prop) => match &**prop {
                    Prop::KeyValue(kv) => {
                        let key = self.object_key_to_string(&kv.key, analysis)?;
                        let value = self.evaluate_static_expr(&kv.value, analysis)?;
                        map.insert(key, value);
                    }
                    Prop::Shorthand(ident) => {
                        let key = ident.sym.to_string();
                        let expr = Expr::Ident(ident.clone());
                        let value = self.evaluate_static_expr(&expr, analysis)?;
                        map.insert(key, value);
                    }
                    Prop::Assign(assign) => {
                        let key = assign.key.sym.to_string();
                        let value = self.evaluate_static_expr(&assign.value, analysis)?;
                        map.insert(key, value);
                    }
                    Prop::Getter(_) | Prop::Setter(_) | Prop::Method(_) => return None,
                },
                PropOrSpread::Spread(spread) => {
                    let value = self.evaluate_static_expr(&spread.expr, analysis)?;
                    if let Some(entries) = value.into_object() {
                        for (key, value) in entries {
                            map.insert(key, value);
                        }
                    } else {
                        return None;
                    }
                }
            }
        }

        Some(CssValue::Object(map))
    }

    fn evaluate_array_literal(
        &self,
        array: &ArrayLit,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        let mut values = Vec::new();

        for elem in &array.elems {
            match elem {
                Some(ExprOrSpread { spread: None, expr }) => {
                    values.push(self.evaluate_static_expr(expr, analysis)?);
                }
                Some(ExprOrSpread {
                    spread: Some(_),
                    expr,
                }) => {
                    let spread_value = self.evaluate_static_expr(expr, analysis)?;
                    if let CssValue::Array(items) = spread_value {
                        values.extend(items);
                    } else {
                        return None;
                    }
                }
                None => values.push(CssValue::Null),
            }
        }

        Some(CssValue::Array(values))
    }

    fn evaluate_member_expr(
        &self,
        member: &MemberExpr,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        let object_value = self.evaluate_static_expr(&member.obj, analysis)?;
        let property_name = self.member_prop_to_string(&member.prop, analysis)?;

        match object_value {
            CssValue::Object(map) => map.get(&property_name).cloned(),
            _ => None,
        }
    }

    fn evaluate_static_call_expr(
        &self,
        call: &swc_core::ecma::ast::CallExpr,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        if let swc_core::ecma::ast::Callee::Expr(callee) = &call.callee {
            if let Expr::Member(member) = &**callee {
                if is_string_concat(member) {
                    return self.evaluate_static_concat_call(&member.obj, &call.args, analysis);
                }
            }
        }

        None
    }

    fn evaluate_static_cond_expr(
        &self,
        cond: &CondExpr,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        let test = self.evaluate_static_expr(&cond.test, analysis)?;

        if test.is_truthy() {
            self.evaluate_static_expr(&cond.cons, analysis)
        } else {
            self.evaluate_static_expr(&cond.alt, analysis)
        }
    }

    fn evaluate_static_concat_call(
        &self,
        object: &Expr,
        args: &[ExprOrSpread],
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        let mut result = self
            .evaluate_static_expr(object, analysis)?
            .into_css_string()?;

        for arg in args {
            if arg.spread.is_some() {
                return None;
            }

            let value = self.evaluate_static_expr(&arg.expr, analysis)?;
            result.push_str(&value.into_css_string()?);
        }

        Some(CssValue::String(result))
    }

    fn evaluate_static_template_literal(
        &self,
        tpl: &Tpl,
        analysis: &ModuleAnalysis,
    ) -> Option<CssValue> {
        let mut result = String::new();

        for (index, quasi) in tpl.quasis.iter().enumerate() {
            result.push_str(quasi.raw.as_ref());

            if let Some(expr) = tpl.exprs.get(index) {
                let value = self.evaluate_static_expr(expr, analysis)?;
                let segment = value.into_template_segment(&self.config)?;
                result.push_str(&segment);
            }
        }

        Some(CssValue::String(result))
    }

    fn object_key_to_string(
        &self,
        key: &swc_core::ecma::ast::PropName,
        analysis: &ModuleAnalysis,
    ) -> Option<String> {
        match key {
            swc_core::ecma::ast::PropName::Ident(ident) => Some(ident.sym.to_string()),
            swc_core::ecma::ast::PropName::Str(str) => Some(wtf8_to_string(&str.value)),
            swc_core::ecma::ast::PropName::Num(num) => Some(num.value.to_string()),
            swc_core::ecma::ast::PropName::Computed(computed) => {
                let expr = computed.expr.as_ref();
                match self.evaluate_static_expr(expr, analysis)? {
                    CssValue::String(text) => Some(text),
                    CssValue::Number(num) => Some(trim_number(num)),
                    _ => None,
                }
            }
            swc_core::ecma::ast::PropName::BigInt(_) => None,
        }
    }

    fn member_prop_to_string(
        &self,
        prop: &MemberProp,
        analysis: &ModuleAnalysis,
    ) -> Option<String> {
        match prop {
            MemberProp::Ident(ident) => Some(ident.sym.to_string()),
            MemberProp::PrivateName(_) => None,
            MemberProp::Computed(expr) => {
                let value = self.evaluate_static_expr(&expr.expr, analysis)?;
                match value {
                    CssValue::String(text) => Some(text),
                    CssValue::Number(num) => Some(trim_number(num)),
                    _ => None,
                }
            }
        }
    }
}

fn is_string_concat(member: &MemberExpr) -> bool {
    matches!(member.prop, MemberProp::Ident(ref ident) if ident.sym.as_ref() == "concat")
}

fn evaluate_unary_expression(op: UnaryOp, value: CssValue) -> Option<CssValue> {
    match op {
        UnaryOp::Plus => value.into_number().map(CssValue::Number),
        UnaryOp::Minus => value.into_number().map(|num| CssValue::Number(-num)),
        UnaryOp::Bang => value.into_bool().map(|b| CssValue::Bool(!b)),
        _ => None,
    }
}

fn evaluate_binary_expression(op: BinaryOp, left: CssValue, right: CssValue) -> Option<CssValue> {
    match op {
        BinaryOp::Add => {
            let left_string = left.clone().into_css_string();
            let right_string = right.clone().into_css_string();

            if let (Some(lhs), Some(rhs)) = (left_string, right_string) {
                return Some(CssValue::String(format!("{lhs}{rhs}")));
            }

            let left_number = left.into_number()?;
            let right_number = right.into_number()?;
            Some(CssValue::Number(left_number + right_number))
        }
        BinaryOp::LogicalAnd => {
            if left.is_truthy() {
                Some(right)
            } else {
                Some(left)
            }
        }
        BinaryOp::LogicalOr => {
            if left.is_truthy() {
                Some(left)
            } else {
                Some(right)
            }
        }
        BinaryOp::NullishCoalescing => {
            if !left.is_nullish() {
                Some(left)
            } else {
                Some(right)
            }
        }
        BinaryOp::Sub => {
            let left_number = left.into_number()?;
            let right_number = right.into_number()?;
            Some(CssValue::Number(left_number - right_number))
        }
        BinaryOp::Mul => {
            let left_number = left.into_number()?;
            let right_number = right.into_number()?;
            Some(CssValue::Number(left_number * right_number))
        }
        BinaryOp::Div => {
            let left_number = left.into_number()?;
            let right_number = right.into_number()?;
            Some(CssValue::Number(left_number / right_number))
        }
        BinaryOp::Mod => {
            let left_number = left.into_number()?;
            let right_number = right.into_number()?;
            Some(CssValue::Number(left_number % right_number))
        }
        _ => None,
    }
}
