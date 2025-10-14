pub mod css;
pub mod eval;
pub mod hash;

use crate::css::{
    add_unit_if_needed, atomicize_literal, atomicize_rules, normalize_selector, AtRuleInput,
    CssArtifacts, CssOptions, CssRuleInput,
};
use crate::hash::hash;
use once_cell::sync::Lazy;
use oxc_resolver::{ResolveOptions, Resolver};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use swc_core::common::{FileName, DUMMY_SP};
use swc_core::ecma::ast::*;
use swc_core::ecma::atoms::JsWord;
use swc_core::ecma::codegen::{text_writer::JsWriter, Emitter};
use swc_core::ecma::parser::{EsConfig, EsVersion, Parser, StringInput, Syntax, TsConfig};
use swc_core::ecma::utils::{private_ident, quote_ident};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use swc_core::plugin::metadata::TransformPluginMetadataContext;
use swc_core::plugin::plugin_transform;
use swc_core::plugin::proxies::TransformPluginProgramMetadata;

static LATEST_ARTIFACTS: Lazy<Mutex<StyleArtifacts>> =
    Lazy::new(|| Mutex::new(StyleArtifacts::default()));

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleArtifacts {
    #[serde(default)]
    pub style_rules: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
}

pub fn take_latest_artifacts() -> StyleArtifacts {
    let mut guard = LATEST_ARTIFACTS.lock().expect("artifacts lock poisoned");
    std::mem::take(&mut *guard)
}

#[derive(Debug, Clone, Default)]
struct NameTracker {
    used: HashSet<String>,
}

impl NameTracker {
    fn from_module(module: &Module) -> Self {
        let mut tracker = NameTracker::default();
        tracker.collect_module(module);
        tracker
    }

    fn collect_module(&mut self, module: &Module) {
        for item in &module.body {
            match item {
                ModuleItem::ModuleDecl(decl) => self.collect_module_decl(decl),
                ModuleItem::Stmt(stmt) => self.collect_stmt(stmt),
            }
        }
    }

    fn collect_module_decl(&mut self, decl: &ModuleDecl) {
        match decl {
            ModuleDecl::Import(import) => {
                for specifier in &import.specifiers {
                    match specifier {
                        ImportSpecifier::Named(named) => self.mark_ident(&named.local),
                        ImportSpecifier::Default(default_spec) => {
                            self.mark_ident(&default_spec.local)
                        }
                        ImportSpecifier::Namespace(namespace) => self.mark_ident(&namespace.local),
                    }
                }
            }
            ModuleDecl::ExportDecl(export_decl) => {
                self.collect_decl(&export_decl.decl);
            }
            ModuleDecl::ExportDefaultDecl(default_decl) => match &default_decl.decl {
                DefaultDecl::Class(class_expr) => {
                    if let Some(ident) = &class_expr.ident {
                        self.mark_ident(ident);
                    }
                }
                DefaultDecl::Fn(fn_expr) => {
                    if let Some(ident) = &fn_expr.ident {
                        self.mark_ident(ident);
                    }
                }
                DefaultDecl::TsInterfaceDecl(decl) => {
                    self.mark_ident(&decl.id);
                }
            },
            ModuleDecl::ExportNamed(_)
            | ModuleDecl::ExportDefaultExpr(_)
            | ModuleDecl::ExportAll(_) => {}
            ModuleDecl::TsImportEquals(import_equals) => {
                self.mark_ident(&import_equals.id);
            }
            ModuleDecl::TsExportAssignment(_) => {}
            ModuleDecl::TsNamespaceExport(export) => {
                self.mark_ident(&export.id);
            }
        }
    }

    fn collect_stmt(&mut self, stmt: &Stmt) {
        if let Stmt::Decl(decl) = stmt {
            self.collect_decl(decl);
        }
    }

    fn collect_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Var(var_decl) => {
                for declarator in &var_decl.decls {
                    self.collect_pat(&declarator.name);
                }
            }
            Decl::Fn(fn_decl) => self.mark_ident(&fn_decl.ident),
            Decl::Class(class_decl) => self.mark_ident(&class_decl.ident),
            Decl::TsInterface(interface_decl) => self.mark_ident(&interface_decl.id),
            Decl::TsTypeAlias(alias_decl) => self.mark_ident(&alias_decl.id),
            Decl::TsEnum(enum_decl) => self.mark_ident(&enum_decl.id),
            Decl::TsModule(module_decl) => match &module_decl.id {
                TsModuleName::Ident(ident) => self.mark_ident(ident),
                TsModuleName::Str(_) => {}
            },
            Decl::Using(using_decl) => self.collect_pat(&using_decl.id),
        }
    }

    fn collect_pat(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(binding) => self.mark_ident(&binding.id),
            Pat::Array(array) => {
                for elem in &array.elems {
                    if let Some(pat) = elem {
                        self.collect_pat(pat);
                    }
                }
            }
            Pat::Object(object) => {
                for prop in &object.props {
                    match prop {
                        ObjectPatProp::KeyValue(key_value) => {
                            self.collect_pat(&key_value.value);
                        }
                        ObjectPatProp::Assign(assign) => {
                            self.mark_ident(&assign.key);
                        }
                        ObjectPatProp::Rest(rest) => {
                            self.collect_pat(&rest.arg);
                        }
                    }
                }
            }
            Pat::Assign(assign) => self.collect_pat(&assign.left),
            Pat::Rest(rest) => self.collect_pat(&rest.arg),
            Pat::Expr(_) => {}
            Pat::Invalid(_) => {}
        }
    }

    fn mark_ident(&mut self, ident: &Ident) {
        self.used.insert(ident.sym.to_string());
    }

    fn fresh_ident(&mut self, base: &str) -> Ident {
        if !self.used.contains(base) {
            self.used.insert(base.to_string());
            return Ident::new(base.into(), DUMMY_SP);
        }

        if base == "_" {
            let mut index = 1usize;
            loop {
                let candidate = format!("_{}", index);
                if self.used.insert(candidate.clone()) {
                    return Ident::new(candidate.into(), DUMMY_SP);
                }
                index += 1;
            }
        }

        let mut index = 0usize;
        loop {
            let candidate = if index == 0 {
                format!("_{}", base)
            } else {
                format!("_{}{}", base, index + 1)
            };
            if self.used.insert(candidate.clone()) {
                return Ident::new(candidate.into(), DUMMY_SP);
            }
            index += 1;
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ExtractStylesToDirectoryOptions {
    source: String,
    dest: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PluginOptions {
    extract: bool,
    import_sources: Vec<String>,
    class_hash_prefix: Option<String>,
    process_xcss: bool,
    class_name_compression_map: BTreeMap<String, String>,
    import_react: Option<bool>,
    add_component_name: Option<bool>,
    nonce: Option<String>,
    cache: Option<Value>,
    optimize_css: Option<bool>,
    extensions: Vec<String>,
    parser_babel_plugins: Vec<Value>,
    increase_specificity: Option<bool>,
    sort_at_rules: Option<bool>,
    flatten_multiple_selectors: Option<bool>,
    style_sheet_path: Option<String>,
    compiled_require_exclude: Option<bool>,
    extract_styles_to_directory: Option<ExtractStylesToDirectoryOptions>,
    sort_shorthand: Option<bool>,
    on_included_files: Option<Value>,
}

impl Default for PluginOptions {
    fn default() -> Self {
        Self {
            extract: true,
            import_sources: vec!["@compiled/react".into()],
            class_hash_prefix: None,
            process_xcss: true,
            class_name_compression_map: BTreeMap::new(),
            import_react: None,
            add_component_name: None,
            nonce: None,
            cache: None,
            optimize_css: None,
            extensions: Vec::new(),
            parser_babel_plugins: Vec::new(),
            increase_specificity: None,
            sort_at_rules: None,
            flatten_multiple_selectors: None,
            style_sheet_path: None,
            compiled_require_exclude: None,
            extract_styles_to_directory: None,
            sort_shorthand: None,
            on_included_files: None,
        }
    }
}

fn syntax_for_filename(name: &str) -> Syntax {
    if name.ends_with(".ts") || name.ends_with(".tsx") || name.ends_with(".cts") {
        Syntax::Typescript(TsConfig {
            tsx: name.ends_with(".tsx"),
            decorators: true,
            dynamic_import: true,
            import_assertions: true,
            ..Default::default()
        })
    } else {
        Syntax::Es(EsConfig {
            jsx: name.ends_with(".jsx") || name.ends_with(".tsx"),
            decorators: true,
            export_default_from: true,
            import_assertions: true,
            dynamic_import: true,
            top_level_await: true,
            ..Default::default()
        })
    }
}

fn emit_expression(expr: &Expr) -> String {
    use std::sync::Arc;

    let cm: Arc<swc_core::common::SourceMap> = Default::default();
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: swc_core::ecma::codegen::Config {
                minify: false,
                target: Some(EsVersion::Es2022),
                ascii_only: false,
                omit_last_semi: false,
                inline_script: false,
            },
            comments: None,
            cm,
            wr: writer,
        };
        emitter.emit_expr(expr).expect("failed to emit expression");
    }
    String::from_utf8(buf).expect("expression emitted as utf8")
}

fn program_to_source(program: &Program) -> Result<String, std::io::Error> {
    use std::sync::Arc;

    let cm: Arc<swc_core::common::SourceMap> = Default::default();
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: swc_core::ecma::codegen::Config {
                minify: false,
                target: Some(EsVersion::Es2022),
                ascii_only: false,
                omit_last_semi: false,
                inline_script: false,
            },
            comments: None,
            cm,
            wr: writer,
        };
        emitter.emit_program(program)?;
    }
    Ok(String::from_utf8(buf).expect("emitted source to be utf8"))
}

#[derive(Debug, Clone, PartialEq)]
enum StaticValue {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Object(BTreeMap<String, StaticValue>),
    Array(Vec<StaticValue>),
}

impl StaticValue {
    fn as_str(&self) -> Option<&str> {
        if let StaticValue::Str(value) = self {
            Some(value.as_str())
        } else {
            None
        }
    }

    fn as_num(&self) -> Option<f64> {
        if let StaticValue::Num(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    fn as_object(&self) -> Option<&BTreeMap<String, StaticValue>> {
        if let StaticValue::Object(map) = self {
            Some(map)
        } else {
            None
        }
    }

    fn as_array(&self) -> Option<&[StaticValue]> {
        if let StaticValue::Array(values) = self {
            Some(values.as_slice())
        } else {
            None
        }
    }

    fn to_js_string(&self) -> Option<String> {
        match self {
            StaticValue::Str(value) => Some(value.clone()),
            StaticValue::Num(value) => Some(value.to_string()),
            StaticValue::Bool(value) => Some(value.to_string()),
            StaticValue::Null => Some("null".to_string()),
            _ => None,
        }
    }

    fn to_property_key(&self) -> Option<String> {
        match self {
            StaticValue::Str(value) => Some(value.clone()),
            StaticValue::Num(value) => Some(value.to_string()),
            StaticValue::Bool(value) => Some(value.to_string()),
            StaticValue::Null => Some("null".to_string()),
            _ => None,
        }
    }
}

fn to_id(ident: &Ident) -> (JsWord, swc_core::common::SyntaxContext) {
    (ident.sym.clone(), ident.span.ctxt())
}

fn collect_static_bindings(
    module: &Module,
    evaluator: Option<&ModuleEvaluator>,
    module_path: Option<&Path>,
) -> HashMap<(JsWord, swc_core::common::SyntaxContext), StaticValue> {
    let mut visiting = HashSet::new();
    collect_module_statics_from_ast(module, module_path, evaluator, &mut visiting).bindings
}

fn evaluate_static(
    expr: &Expr,
    bindings: &HashMap<(JsWord, swc_core::common::SyntaxContext), StaticValue>,
) -> Option<StaticValue> {
    match expr {
        Expr::Lit(Lit::Str(str)) => Some(StaticValue::Str(str.value.to_string())),
        Expr::Lit(Lit::Num(num)) => Some(StaticValue::Num(num.value)),
        Expr::Lit(Lit::Bool(boolean)) => Some(StaticValue::Bool(boolean.value)),
        Expr::Lit(Lit::Null(_)) => Some(StaticValue::Null),
        Expr::Tpl(template) => {
            let mut result = String::new();
            for (index, quasi) in template.quasis.iter().enumerate() {
                result.push_str(
                    quasi
                        .cooked
                        .as_ref()
                        .or_else(|| quasi.raw.as_ref())
                        .map(|atom| atom.to_string())
                        .unwrap_or_default()
                        .as_str(),
                );
                if let Some(expr) = template.exprs.get(index) {
                    let value = evaluate_static(expr, bindings)?;
                    result.push_str(value.as_str()?);
                }
            }
            Some(StaticValue::Str(result))
        }
        Expr::Paren(paren) => evaluate_static(&paren.expr, bindings),
        Expr::TsAs(ts_as) => evaluate_static(&ts_as.expr, bindings),
        Expr::TsTypeAssertion(assert) => evaluate_static(&assert.expr, bindings),
        Expr::TsConstAssertion(assert) => evaluate_static(&assert.expr, bindings),
        Expr::TsNonNull(non_null) => evaluate_static(&non_null.expr, bindings),
        Expr::Member(member) => {
            let object = match &member.obj {
                ExprOrSuper::Expr(expr) => evaluate_static(expr, bindings)?,
                ExprOrSuper::Super(_) => return None,
            };

            let key = match &member.prop {
                MemberProp::Ident(ident) => ident.sym.to_string(),
                MemberProp::Computed(computed) => {
                    let evaluated = evaluate_static(&computed.expr, bindings)?;
                    evaluated.to_property_key()?
                }
                MemberProp::PrivateName(_) => return None,
            };

            match &object {
                StaticValue::Object(map) => map.get(&key).cloned(),
                StaticValue::Array(values) => {
                    let index = key.parse::<usize>().ok()?;
                    values.get(index).cloned()
                }
                _ => None,
            }
        }
        Expr::Bin(bin) => {
            let left = evaluate_static(&bin.left, bindings)?;
            let right = evaluate_static(&bin.right, bindings)?;
            match bin.op {
                BinaryOp::Add => {
                    if let (Some(lhs), Some(rhs)) = (left.as_num(), right.as_num()) {
                        Some(StaticValue::Num(lhs + rhs))
                    } else {
                        let left_str = left.to_js_string()?;
                        let right_str = right.to_js_string()?;
                        Some(StaticValue::Str(format!("{}{}", left_str, right_str)))
                    }
                }
                _ => None,
            }
        }
        Expr::Object(obj) => {
            let mut map = BTreeMap::new();
            for prop in &obj.props {
                match prop {
                    PropOrSpread::Prop(prop) => match &**prop {
                        Prop::KeyValue(KeyValueProp { key, value }) => {
                            let name = match key {
                                PropName::Ident(ident) => ident.sym.to_string(),
                                PropName::Str(str) => str.value.to_string(),
                                PropName::Num(num) => num.value.to_string(),
                                PropName::Computed(computed) => {
                                    let evaluated = evaluate_static(&computed.expr, bindings)?;
                                    evaluated.to_property_key()?
                                }
                                _ => return None,
                            };
                            let value = evaluate_static(value, bindings)?;
                            map.insert(name, value);
                        }
                        _ => return None,
                    },
                    PropOrSpread::Spread(SpreadElement { expr, .. }) => {
                        let value = evaluate_static(expr, bindings)?;
                        if let StaticValue::Object(other) = value {
                            for (key, value) in other {
                                map.insert(key, value);
                            }
                        } else {
                            return None;
                        }
                    }
                }
            }
            Some(StaticValue::Object(map))
        }
        Expr::Array(array) => {
            let mut values = Vec::new();
            for elem in &array.elems {
                if let Some(elem) = elem {
                    let value = evaluate_static(&elem.expr, bindings)?;
                    values.push(value);
                }
            }
            Some(StaticValue::Array(values))
        }
        Expr::Ident(ident) => bindings.get(&to_id(ident)).cloned(),
        _ => None,
    }
}

fn record_var_decl(
    var: &VarDecl,
    bindings: &mut HashMap<(JsWord, swc_core::common::SyntaxContext), StaticValue>,
) -> Vec<(Ident, StaticValue)> {
    let mut recorded = Vec::new();
    if var.kind != VarDeclKind::Const {
        return recorded;
    }
    for decl in &var.decls {
        if let Pat::Ident(BindingIdent { id, .. }) = &decl.name {
            if let Some(init) = &decl.init {
                if let Some(value) = evaluate_static(init, bindings) {
                    bindings.insert(to_id(id), value.clone());
                    recorded.push((id.clone(), value));
                }
            }
        }
    }
    recorded
}

const DEFAULT_RESOLVE_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"];

#[derive(Clone, Default)]
struct ModuleStaticResult {
    bindings: HashMap<(JsWord, swc_core::common::SyntaxContext), StaticValue>,
    exports: HashMap<String, StaticValue>,
}

struct ModuleEvaluator {
    resolver: Resolver,
    cache: RefCell<HashMap<PathBuf, ModuleStaticResult>>,
    included_files: RefCell<BTreeSet<PathBuf>>,
}

impl ModuleEvaluator {
    fn new(cwd: &Path, extensions: &[String]) -> Self {
        let mut options = ResolveOptions::default();
        if extensions.is_empty() {
            options.extensions = DEFAULT_RESOLVE_EXTENSIONS
                .iter()
                .map(|ext| ext.to_string())
                .collect();
        } else {
            options.extensions = extensions
                .iter()
                .map(|ext| {
                    if ext.starts_with('.') {
                        ext.clone()
                    } else {
                        format!(".{ext}")
                    }
                })
                .collect();
        }
        let resolver = Resolver::new(cwd.to_path_buf(), options);
        Self {
            resolver,
            cache: RefCell::new(HashMap::new()),
            included_files: RefCell::new(BTreeSet::new()),
        }
    }

    fn resolve(&self, from: &Path, request: &str) -> Option<PathBuf> {
        self.resolver
            .resolve(from, request)
            .ok()
            .map(|result| result.full_path)
    }

    fn statics_for(&self, path: &Path) -> Option<ModuleStaticResult> {
        let mut visiting = HashSet::new();
        self.statics_for_inner(path, &mut visiting)
    }

    fn statics_for_inner(
        &self,
        path: &Path,
        visiting: &mut HashSet<PathBuf>,
    ) -> Option<ModuleStaticResult> {
        self.included_files.borrow_mut().insert(path.to_path_buf());
        if let Some(cached) = self.cache.borrow().get(path).cloned() {
            return Some(cached);
        }
        if !visiting.insert(path.to_path_buf()) {
            return None;
        }
        let source = fs::read_to_string(path).ok()?;
        let module = parse_module_from_source(&source, path)?;
        let result = collect_module_statics_from_ast(&module, Some(path), Some(self), visiting);
        visiting.remove(path);
        self.cache
            .borrow_mut()
            .insert(path.to_path_buf(), result.clone());
        Some(result)
    }

    fn included_files(&self) -> Vec<PathBuf> {
        self.included_files.borrow().iter().cloned().collect()
    }
}

fn parse_module_from_source(source: &str, path: &Path) -> Option<Module> {
    use std::sync::Arc;

    let filename = path.to_string_lossy().to_string();
    let cm: Arc<swc_core::common::SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(filename.clone()), source.into());
    let syntax = syntax_for_filename(&filename);
    let lexer = swc_core::ecma::parser::lexer::Lexer::new(
        syntax,
        EsVersion::Es2022,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    parser.parse_module().ok()
}

fn collect_module_statics_from_ast(
    module: &Module,
    module_path: Option<&Path>,
    evaluator: Option<&ModuleEvaluator>,
    visiting: &mut HashSet<PathBuf>,
) -> ModuleStaticResult {
    let mut result = ModuleStaticResult::default();

    if let (Some(evaluator), Some(module_path)) = (evaluator, module_path) {
        for item in &module.body {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item {
                if let Some(resolved) = evaluator.resolve(module_path, import.src.value.as_ref()) {
                    if let Some(imported) = evaluator.statics_for_inner(&resolved, visiting) {
                        for specifier in &import.specifiers {
                            match specifier {
                                ImportSpecifier::Named(named) => {
                                    let export_name = match &named.imported {
                                        Some(ModuleExportName::Ident(ident)) => {
                                            ident.sym.to_string()
                                        }
                                        Some(ModuleExportName::Str(str)) => str.value.to_string(),
                                        Some(ModuleExportName::Num(_)) => continue,
                                        None => named.local.sym.to_string(),
                                    };
                                    if let Some(value) = imported.exports.get(&export_name) {
                                        result.bindings.insert(to_id(&named.local), value.clone());
                                    }
                                }
                                ImportSpecifier::Default(default_spec) => {
                                    if let Some(value) = imported.exports.get("default") {
                                        result
                                            .bindings
                                            .insert(to_id(&default_spec.local), value.clone());
                                    }
                                }
                                ImportSpecifier::Namespace(_) => {}
                            }
                        }
                    }
                }
            }
        }
    }

    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => {
                record_var_decl(var, &mut result.bindings);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) => {
                match &export_decl.decl {
                    Decl::Var(var) => {
                        for (ident, value) in record_var_decl(var, &mut result.bindings) {
                            result.exports.insert(ident.sym.to_string(), value);
                        }
                    }
                    _ => {}
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(named)) => {
                if let Some(src) = &named.src {
                    if let (Some(evaluator), Some(module_path)) = (evaluator, module_path) {
                        if let Some(resolved) = evaluator.resolve(module_path, src.value.as_ref()) {
                            if let Some(exported) = evaluator.statics_for_inner(&resolved, visiting)
                            {
                                for specifier in &named.specifiers {
                                    if let ExportSpecifier::Named(named_spec) = specifier {
                                        let orig_name = match &named_spec.orig {
                                            ModuleExportName::Ident(ident) => ident.sym.to_string(),
                                            ModuleExportName::Str(str) => str.value.to_string(),
                                            ModuleExportName::Num(_) => continue,
                                        };
                                        let export_name = match &named_spec.exported {
                                            Some(ModuleExportName::Ident(ident)) => {
                                                ident.sym.to_string()
                                            }
                                            Some(ModuleExportName::Str(str)) => {
                                                str.value.to_string()
                                            }
                                            Some(ModuleExportName::Num(_)) => continue,
                                            None => orig_name.clone(),
                                        };
                                        if let Some(value) = exported.exports.get(&orig_name) {
                                            result.exports.insert(export_name, value.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    for specifier in &named.specifiers {
                        if let ExportSpecifier::Named(named_spec) = specifier {
                            let export_name = match &named_spec.exported {
                                Some(ModuleExportName::Ident(ident)) => ident.sym.to_string(),
                                Some(ModuleExportName::Str(str)) => str.value.to_string(),
                                Some(ModuleExportName::Num(_)) => continue,
                                None => match &named_spec.orig {
                                    ModuleExportName::Ident(ident) => ident.sym.to_string(),
                                    ModuleExportName::Str(str) => str.value.to_string(),
                                    ModuleExportName::Num(_) => continue,
                                },
                            };
                            match &named_spec.orig {
                                ModuleExportName::Ident(ident) => {
                                    if let Some(value) = result.bindings.get(&to_id(ident)).cloned()
                                    {
                                        result.exports.insert(export_name, value);
                                    }
                                }
                                ModuleExportName::Str(str) => {
                                    if let Some(value) =
                                        result.exports.get(str.value.as_ref()).cloned()
                                    {
                                        result.exports.insert(export_name, value);
                                    }
                                }
                                ModuleExportName::Num(_) => {}
                            }
                        }
                    }
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(export_all)) => {
                if let (Some(evaluator), Some(module_path)) = (evaluator, module_path) {
                    if let Some(resolved) =
                        evaluator.resolve(module_path, export_all.src.value.as_ref())
                    {
                        if let Some(exported) = evaluator.statics_for_inner(&resolved, visiting) {
                            for (name, value) in exported.exports {
                                if name != "default" {
                                    result.exports.entry(name).or_insert(value);
                                }
                            }
                        }
                    }
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(default_expr)) => {
                if let Some(value) = evaluate_static(&default_expr.expr, &result.bindings) {
                    result.exports.insert("default".into(), value);
                }
            }
            _ => {}
        }
    }

    result
}

fn kebab_case(input: &str) -> String {
    let mut result = String::new();
    for (index, ch) in input.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                result.push('-');
            }
            for lower in ch.to_lowercase() {
                result.push(lower);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn encode_uri_component(input: &str) -> String {
    fn is_allowed(ch: char) -> bool {
        matches!(ch,
            'A'..='Z'
                | 'a'..='z'
                | '0'..='9'
                | '-' | '_' | '.' | '~'
                | '*' | '\'' | '(' | ')'
        )
    }

    let mut encoded = String::new();
    for ch in input.chars() {
        if is_allowed(ch) {
            encoded.push(ch);
        } else {
            let mut buf = [0u8; 4];
            let encoded_bytes = ch.encode_utf8(&mut buf).as_bytes();
            for byte in encoded_bytes {
                encoded.push('%');
                encoded.push_str(&format!("{:02X}", byte));
            }
        }
    }

    encoded
}

fn append_stylesheet_requires(module: &mut Module, path: &str, rules: &[String]) {
    if rules.is_empty() {
        return;
    }

    for rule in rules {
        let request = format!("{}?style={}", path, encode_uri_component(rule));
        let call = Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(quote_ident!("require")))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Lit(Lit::Str(Str::from(request)))),
            }],
            type_args: None,
        });
        module.body.insert(
            0,
            ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(call),
            })),
        );
    }
}

fn prepend_module_import(module: &mut Module, import_src: &str) {
    let already_present = module.body.iter().any(|item| {
        if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item {
            if import.specifiers.is_empty() {
                return import.src.value.as_ref() == import_src;
            }
        }
        false
    });

    if already_present {
        return;
    }

    module.body.insert(
        0,
        ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
            span: DUMMY_SP,
            specifiers: Vec::new(),
            src: Box::new(Str::from(import_src)),
            type_only: false,
            with: None,
        })),
    );
}

fn sort_and_join_rules(rules: &[String], sort_at_rules: bool, sort_shorthand: bool) -> String {
    if sort_at_rules || sort_shorthand {
        let mut deduped: BTreeSet<String> = BTreeSet::new();
        for rule in rules {
            deduped.insert(rule.clone());
        }
        deduped.into_iter().collect::<Vec<_>>().join("\n")
    } else {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for rule in rules {
            if seen.insert(rule.clone()) {
                ordered.push(rule.clone());
            }
        }
        ordered.join("\n")
    }
}

fn write_stylesheet_to_directory(
    module: &mut Module,
    options: &ExtractStylesToDirectoryOptions,
    rules: &[String],
    sort_at_rules: bool,
    sort_shorthand: bool,
    file_path: &Path,
    cwd: &Path,
) -> Result<(), String> {
    if rules.is_empty() {
        return Ok(());
    }

    let file_stem = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("Source filename '{}' was not defined", file_path.display()))?;

    let source_root = {
        let source = Path::new(&options.source);
        if source.is_absolute() {
            source.to_path_buf()
        } else {
            cwd.join(source)
        }
    };

    let absolute_file = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        cwd.join(file_path)
    };

    let relative_path = absolute_file.strip_prefix(&source_root).map_err(|_| {
        format!(
            "Source directory '{}' was not found relative to source file ('{}')",
            options.source,
            absolute_file.display()
        )
    })?;

    let relative_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));

    let dest_root = {
        let dest = Path::new(&options.dest);
        if dest.is_absolute() {
            dest.to_path_buf()
        } else {
            cwd.join(dest)
        }
    };

    let output_dir = dest_root.join(relative_dir);
    fs::create_dir_all(&output_dir).map_err(|err| {
        format!(
            "Failed to create directory '{}': {}",
            output_dir.display(),
            err
        )
    })?;

    let css_filename = format!("{}.compiled.css", file_stem);
    let css_path = output_dir.join(&css_filename);
    let stylesheet = sort_and_join_rules(rules, sort_at_rules, sort_shorthand);
    fs::write(&css_path, stylesheet).map_err(|err| {
        format!(
            "Failed to write stylesheet '{}': {}",
            css_path.display(),
            err
        )
    })?;

    prepend_module_import(module, &format!("./{}", css_filename));

    Ok(())
}

fn build_css_from_object(value: &BTreeMap<String, StaticValue>) -> Option<String> {
    let mut declarations = Vec::new();
    for (key, value) in value {
        let css_value = match value {
            StaticValue::Str(str) => str.clone(),
            StaticValue::Num(num) => {
                if num.fract() == 0.0 {
                    format!("{}", *num as i64)
                } else {
                    num.to_string()
                }
            }
            StaticValue::Bool(boolean) => boolean.to_string(),
            _ => return None,
        };
        declarations.push(format!("{}:{}", kebab_case(key), css_value));
    }
    Some(format!("{};", declarations.join(";")))
}

fn normalize_content_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "\"\"".to_string();
    }

    if trimmed.contains('"') || trimmed.contains('\'') {
        return trimmed.to_string();
    }

    if trimmed.contains("-quote") {
        return trimmed.to_string();
    }

    match trimmed {
        "inherit" | "initial" | "none" | "normal" | "revert" | "unset" => {
            return trimmed.to_string()
        }
        _ => {}
    }

    let mut prefix = String::new();
    let mut chars = trimmed.chars();
    while let Some(ch) = chars.next() {
        if ch.is_ascii_alphabetic() || ch == '-' {
            prefix.push(ch);
            continue;
        }

        if ch == '(' {
            if !prefix.is_empty() {
                return trimmed.to_string();
            }
            break;
        }

        if ch.is_ascii_whitespace() {
            break;
        }

        prefix.clear();
        break;
    }

    if !prefix.is_empty() {
        if trimmed[prefix.len()..].starts_with('(') {
            return trimmed.to_string();
        }
    }

    format!("\"{}\"", trimmed)
}

fn static_value_to_css_value(property: &str, value: &StaticValue) -> Option<(String, bool)> {
    match value {
        StaticValue::Str(str) => {
            let mut trimmed = str.trim().to_string();
            if trimmed.is_empty() {
                if property == "content" {
                    return Some(("\"\"".to_string(), false));
                }
                return None;
            }
            let mut important = false;
            if let Some(stripped) = trimmed.strip_suffix("!important") {
                important = true;
                trimmed = stripped.trim_end().to_string();
            }
            let normalized = if property == "content" {
                normalize_content_value(&trimmed)
            } else {
                trimmed
            };
            if normalized.is_empty() {
                return None;
            }
            Some((normalized, important))
        }
        StaticValue::Num(num) => {
            let raw = if num.fract() == 0.0 {
                format!("{}", *num as i64)
            } else {
                num.to_string()
            };
            let value = add_unit_if_needed(property, &raw);
            Some((value, false))
        }
        StaticValue::Bool(boolean) => Some((boolean.to_string(), false)),
        StaticValue::Null => None,
        _ => None,
    }
}

fn extend_selectors(current: &[String], raw: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let segments: Vec<&str> = raw
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();

    let parents = if current.is_empty() {
        vec![normalize_selector(None)]
    } else {
        current.to_vec()
    };

    if segments.is_empty() {
        return parents;
    }

    for parent in &parents {
        for segment in &segments {
            let normalized = normalize_selector(Some(segment));
            let combined = normalized.replace('&', parent);
            if seen.insert(combined.clone()) {
                result.push(combined);
            }
        }
    }

    result
}

fn parse_at_rule_key(key: &str) -> AtRuleInput {
    let trimmed = key.trim_start_matches('@').trim();
    let mut name_end = None;
    for (index, ch) in trimmed.char_indices() {
        if ch.is_whitespace() {
            name_end = Some(index);
            break;
        }
    }
    match name_end {
        Some(index) => AtRuleInput {
            name: trimmed[..index].to_string(),
            params: trimmed[index..].trim().to_string(),
        },
        None => AtRuleInput {
            name: trimmed.to_string(),
            params: String::new(),
        },
    }
}

const ATOMIC_AT_RULES: &[&str] = &[
    "container",
    "-moz-document",
    "else",
    "layer",
    "media",
    "starting-style",
    "supports",
    "when",
];

const NON_ATOMIC_AT_RULES: &[&str] = &[
    "color-profile",
    "counter-style",
    "font-face",
    "font-palette-values",
    "keyframes",
    "page",
    "property",
];

fn push_css_value(
    key: &str,
    value: &StaticValue,
    selectors: &[String],
    at_rules: &[AtRuleInput],
    out: &mut Vec<CssRuleInput>,
    flatten_selectors: bool,
) -> bool {
    let property = if key.starts_with("--") {
        key.to_string()
    } else {
        kebab_case(key)
    };
    let (value, important) = match static_value_to_css_value(&property, value) {
        Some(result) => result,
        None => return false,
    };

    if selectors.is_empty() {
        out.push(CssRuleInput {
            selectors: vec![normalize_selector(None)],
            at_rules: at_rules.to_vec(),
            property,
            value,
            important,
        });
        return true;
    }

    if !flatten_selectors && selectors.len() > 1 {
        out.push(CssRuleInput {
            selectors: selectors.to_vec(),
            at_rules: at_rules.to_vec(),
            property,
            value,
            important,
        });
        return true;
    }

    for selector in selectors {
        out.push(CssRuleInput {
            selectors: vec![selector.clone()],
            at_rules: at_rules.to_vec(),
            property: property.clone(),
            value: value.clone(),
            important,
        });
    }

    true
}

fn flatten_css_object(
    map: &BTreeMap<String, StaticValue>,
    selectors: &[String],
    at_rules: &[AtRuleInput],
    out: &mut Vec<CssRuleInput>,
    raw_rules: &mut Vec<String>,
    flatten_selectors: bool,
) -> bool {
    for (key, value) in map {
        if key == "selectors" {
            let nested = match value.as_object() {
                Some(obj) => obj,
                None => return false,
            };
            for (selector_key, selector_value) in nested {
                let next_selectors = extend_selectors(selectors, selector_key);
                let nested_obj = match selector_value.as_object() {
                    Some(obj) => obj,
                    None => return false,
                };
                if !flatten_css_object(
                    nested_obj,
                    &next_selectors,
                    at_rules,
                    out,
                    raw_rules,
                    flatten_selectors,
                ) {
                    return false;
                }
            }
            continue;
        }

        if key.starts_with('@') {
            let descriptor = parse_at_rule_key(key);
            let normalized_name = descriptor.name.to_ascii_lowercase();
            if NON_ATOMIC_AT_RULES.contains(&normalized_name.as_str()) {
                let mut next_at_rules = at_rules.to_vec();
                next_at_rules.push(descriptor);
                match value {
                    StaticValue::Object(obj) => {
                        let Some(body) = build_css_from_object(obj) else {
                            return false;
                        };
                        let css = wrap_at_rules(
                            body.trim().trim_end_matches(';').to_string(),
                            &next_at_rules,
                        );
                        raw_rules.push(css);
                    }
                    StaticValue::Array(items) => {
                        for item in items {
                            let StaticValue::Object(obj) = item else {
                                return false;
                            };
                            let Some(body) = build_css_from_object(obj) else {
                                return false;
                            };
                            let css = wrap_at_rules(
                                body.trim().trim_end_matches(';').to_string(),
                                &next_at_rules,
                            );
                            raw_rules.push(css);
                        }
                    }
                    StaticValue::Str(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let css = wrap_at_rules(trimmed.to_string(), &next_at_rules);
                            raw_rules.push(css);
                        }
                    }
                    _ => return false,
                }
                continue;
            }

            if !ATOMIC_AT_RULES.contains(&normalized_name.as_str()) {
                return false;
            }
            let mut next_at_rules = at_rules.to_vec();
            next_at_rules.push(descriptor);
            match value {
                StaticValue::Object(obj) => {
                    if !flatten_css_object(
                        obj,
                        selectors,
                        &next_at_rules,
                        out,
                        raw_rules,
                        flatten_selectors,
                    ) {
                        return false;
                    }
                }
                StaticValue::Array(items) => {
                    for item in items {
                        if let StaticValue::Object(obj) = item {
                            if !flatten_css_object(
                                obj,
                                selectors,
                                &next_at_rules,
                                out,
                                raw_rules,
                                flatten_selectors,
                            ) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }
                _ => return false,
            }
            continue;
        }

        if let Some(nested) = value.as_object() {
            let next_selectors = extend_selectors(selectors, key);
            if !flatten_css_object(
                nested,
                &next_selectors,
                at_rules,
                out,
                raw_rules,
                flatten_selectors,
            ) {
                return false;
            }
            continue;
        }

        if let Some(array) = value.as_array() {
            for item in array {
                if !push_css_value(key, item, selectors, at_rules, out, flatten_selectors) {
                    return false;
                }
            }
            continue;
        }

        if !push_css_value(key, value, selectors, at_rules, out, flatten_selectors) {
            return false;
        }
    }

    true
}

fn css_artifacts_from_static_object(
    map: &BTreeMap<String, StaticValue>,
    options: &CssOptions,
) -> Option<CssArtifacts> {
    let mut inputs = Vec::new();
    let base_selectors = vec![normalize_selector(None)];
    let mut raw_rules = Vec::new();
    if !flatten_css_object(
        map,
        &base_selectors,
        &[],
        &mut inputs,
        &mut raw_rules,
        options.flatten_multiple_selectors,
    ) {
        return None;
    }
    let mut artifacts = atomicize_rules(&inputs, options);
    artifacts.raw_rules.extend(raw_rules);
    Some(artifacts)
}

fn css_artifacts_from_static_value(
    value: &StaticValue,
    options: &CssOptions,
) -> Option<CssArtifacts> {
    match value {
        StaticValue::Object(map) => css_artifacts_from_static_object(map, options),
        StaticValue::Str(text) => Some(atomicize_literal(text, options)),
        StaticValue::Array(items) => {
            let mut combined = CssArtifacts::default();
            for item in items {
                match css_artifacts_from_static_value(item, options) {
                    Some(artifacts) => combined.merge(artifacts),
                    None => return None,
                }
            }
            Some(combined)
        }
        StaticValue::Null => Some(CssArtifacts::default()),
        _ => None,
    }
}

fn binding_ident_from_pat(pat: &Pat) -> Option<Ident> {
    match pat {
        Pat::Ident(ident) => Some(ident.id.clone()),
        Pat::Assign(assign) => binding_ident_from_pat(&assign.left),
        _ => None,
    }
}

struct ClassNamesBodyVisitor<'a, 'b> {
    parent: &'a mut TransformVisitor<'b>,
    css_idents: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    style_idents: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    failed: bool,
    sheets: Vec<String>,
}

impl<'a, 'b> ClassNamesBodyVisitor<'a, 'b> {
    fn new(
        parent: &'a mut TransformVisitor<'b>,
        css_idents: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
        style_idents: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    ) -> Self {
        Self {
            parent,
            css_idents,
            style_idents,
            failed: false,
            sheets: Vec::new(),
        }
    }
}

impl<'a, 'b> VisitMut for ClassNamesBodyVisitor<'a, 'b> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if self.failed {
            return;
        }

        match expr {
            Expr::Call(call) => {
                if let Callee::Expr(callee_expr) = &mut call.callee {
                    if let Expr::Ident(ident) = &**callee_expr {
                        if self.css_idents.contains(&to_id(ident)) {
                            let values = match self.parent.evaluate_call_arguments(call) {
                                Some(values) => values,
                                None => {
                                    self.failed = true;
                                    return;
                                }
                            };
                            let mut combined = CssArtifacts::default();
                            for value in &values {
                                let artifacts = match css_artifacts_from_static_value(
                                    value,
                                    &self.parent.css_options(),
                                ) {
                                    Some(artifacts) => artifacts,
                                    None => {
                                        self.failed = true;
                                        return;
                                    }
                                };
                                combined.merge(artifacts);
                            }
                            let mut class_names = Vec::new();
                            for rule in &combined.rules {
                                self.parent.register_rule(rule.css.clone());
                                class_names.push(rule.class_name.clone());
                            }
                            for css in &combined.raw_rules {
                                self.parent.register_rule(css.clone());
                            }
                            if !self.parent.options.extract {
                                for rule in &combined.rules {
                                    self.sheets.push(rule.css.clone());
                                }
                                for css in &combined.raw_rules {
                                    self.sheets.push(css.clone());
                                }
                            }
                            self.parent.needs_runtime_ax = true;
                            let joined = class_names.join(" ");
                            let array = Expr::Array(ArrayLit {
                                span: DUMMY_SP,
                                elems: if joined.is_empty() {
                                    Vec::new()
                                } else {
                                    vec![Some(ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Lit(Lit::Str(Str::from(joined)))),
                                    })]
                                },
                            });
                            *expr = Expr::Call(CallExpr {
                                span: call.span,
                                callee: Callee::Expr(Box::new(Expr::Ident(
                                    self.parent.runtime_class_ident(),
                                ))),
                                args: vec![ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(array),
                                }],
                                type_args: None,
                            });
                            return;
                        }
                    }
                }
                call.visit_mut_children_with(self);
            }
            Expr::TaggedTpl(tagged) => {
                if let Expr::Ident(ident) = &*tagged.tag {
                    if self.css_idents.contains(&to_id(ident)) {
                        let css = match self.parent.evaluate_template(tagged) {
                            Some(css) => css,
                            None => {
                                self.failed = true;
                                return;
                            }
                        };
                        let artifacts = atomicize_literal(&css, &self.parent.css_options());
                        let mut class_names = Vec::new();
                        for rule in &artifacts.rules {
                            self.parent.register_rule(rule.css.clone());
                            class_names.push(rule.class_name.clone());
                        }
                        for css in &artifacts.raw_rules {
                            self.parent.register_rule(css.clone());
                        }
                        if !self.parent.options.extract {
                            for rule in &artifacts.rules {
                                self.sheets.push(rule.css.clone());
                            }
                            for css in &artifacts.raw_rules {
                                self.sheets.push(css.clone());
                            }
                        }
                        self.parent.needs_runtime_ax = true;
                        let joined = class_names.join(" ");
                        let array = Expr::Array(ArrayLit {
                            span: DUMMY_SP,
                            elems: if joined.is_empty() {
                                Vec::new()
                            } else {
                                vec![Some(ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(Expr::Lit(Lit::Str(Str::from(joined)))),
                                })]
                            },
                        });
                        *expr = Expr::Call(CallExpr {
                            span: tagged.span,
                            callee: Callee::Expr(Box::new(Expr::Ident(
                                self.parent.runtime_class_ident(),
                            ))),
                            args: vec![ExprOrSpread {
                                spread: None,
                                expr: Box::new(array),
                            }],
                            type_args: None,
                        });
                        return;
                    }
                }
                tagged.visit_mut_children_with(self);
            }
            Expr::Ident(ident) => {
                if self.style_idents.contains(&to_id(ident)) {
                    *expr = Expr::Ident(Ident::new("undefined".into(), DUMMY_SP));
                    return;
                }
            }
            _ => expr.visit_mut_children_with(self),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CompiledImportKind {
    Css,
    Keyframes,
    Styled,
    CssMap,
    ClassNames,
}

struct TransformVisitor<'a> {
    options: &'a PluginOptions,
    bindings: HashMap<(JsWord, swc_core::common::SyntaxContext), StaticValue>,
    css_imports: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    keyframes_imports: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    styled_imports: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    css_map_imports: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    compiled_import_kinds: HashMap<(JsWord, swc_core::common::SyntaxContext), CompiledImportKind>,
    retain_imports: HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    collected_rules: Vec<String>,
    seen_rules: HashSet<String>,
    needs_runtime_ax: bool,
    needs_runtime_cc: bool,
    needs_runtime_cs: bool,
    needs_jsx_runtime: bool,
    needs_jsxs_runtime: bool,
    needs_react_namespace: bool,
    needs_forward_ref: bool,
    has_forward_ref_binding: bool,
    styled_display_names: Vec<(Ident, String)>,
    hoisted_sheets: BTreeMap<String, Ident>,
    hoisted_sheet_order: Vec<String>,
    name_tracker: NameTracker,
    runtime_class_ident: Option<Ident>,
    runtime_ix_ident: Option<Ident>,
    runtime_cc_ident: Option<Ident>,
    runtime_cs_ident: Option<Ident>,
    jsx_ident: Option<Ident>,
    jsxs_ident: Option<Ident>,
    react_namespace_ident: Option<Ident>,
    forward_ref_ident: Option<Ident>,
    has_react_namespace_binding: bool,
}

impl<'a> TransformVisitor<'a> {
    fn new(
        options: &'a PluginOptions,
        bindings: HashMap<(JsWord, swc_core::common::SyntaxContext), StaticValue>,
        name_tracker: NameTracker,
    ) -> Self {
        Self {
            options,
            bindings,
            css_imports: HashSet::new(),
            keyframes_imports: HashSet::new(),
            styled_imports: HashSet::new(),
            css_map_imports: HashSet::new(),
            compiled_import_kinds: HashMap::new(),
            retain_imports: HashSet::new(),
            collected_rules: Vec::new(),
            seen_rules: HashSet::new(),
            needs_runtime_ax: false,
            needs_runtime_cc: false,
            needs_runtime_cs: false,
            needs_jsx_runtime: false,
            needs_jsxs_runtime: false,
            needs_react_namespace: false,
            needs_forward_ref: false,
            has_forward_ref_binding: false,
            styled_display_names: Vec::new(),
            hoisted_sheets: BTreeMap::new(),
            hoisted_sheet_order: Vec::new(),
            name_tracker,
            runtime_class_ident: None,
            runtime_ix_ident: None,
            runtime_cc_ident: None,
            runtime_cs_ident: None,
            jsx_ident: None,
            jsxs_ident: None,
            react_namespace_ident: None,
            forward_ref_ident: None,
            has_react_namespace_binding: false,
        }
    }

    fn register_rule(&mut self, css: String) {
        if self.seen_rules.insert(css.clone()) {
            self.collected_rules.push(css);
        }
    }

    fn hoist_sheet_ident(&mut self, css: &str) -> Ident {
        if let Some(ident) = self.hoisted_sheets.get(css) {
            return ident.clone();
        }

        let ident = self.name_tracker.fresh_ident("_");
        self.hoisted_sheets.insert(css.to_string(), ident.clone());
        self.hoisted_sheet_order.push(css.to_string());
        ident
    }

    fn css_options(&self) -> CssOptions {
        CssOptions {
            class_hash_prefix: self.options.class_hash_prefix.clone(),
            class_name_compression_map: self.options.class_name_compression_map.clone(),
            increase_specificity: self.options.increase_specificity.unwrap_or(false),
            sort_at_rules: self.options.sort_at_rules.unwrap_or(true),
            sort_shorthand: self.options.sort_shorthand.unwrap_or(true),
            flatten_multiple_selectors: self.options.flatten_multiple_selectors.unwrap_or(true),
        }
    }

    fn should_import_react_namespace(&self) -> bool {
        self.options.import_react.unwrap_or(true)
    }

    fn should_emit_component_class_name(&self) -> bool {
        if !self.options.add_component_name.unwrap_or(false) {
            return false;
        }
        match std::env::var("NODE_ENV") {
            Ok(value) => value != "production",
            Err(_) => true,
        }
    }

    fn is_development_env(&self) -> bool {
        let node_env = std::env::var("NODE_ENV").ok();
        let babel_env = std::env::var("BABEL_ENV").ok();
        if node_env.is_none() && babel_env.is_none() {
            return true;
        }
        matches!(babel_env.as_deref(), Some("development") | Some("test"))
            || matches!(node_env.as_deref(), Some("development") | Some("test"))
    }

    fn runtime_class_helper(&self) -> &'static str {
        if self.options.class_name_compression_map.is_empty() {
            "ax"
        } else {
            "ac"
        }
    }

    fn runtime_class_ident(&mut self) -> Ident {
        if let Some(ident) = &self.runtime_class_ident {
            return ident.clone();
        }
        let helper = self.runtime_class_helper();
        let ident = self.name_tracker.fresh_ident(helper);
        self.runtime_class_ident = Some(ident.clone());
        ident
    }

    fn runtime_ix_ident(&mut self) -> Ident {
        if let Some(ident) = &self.runtime_ix_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("ix");
        self.runtime_ix_ident = Some(ident.clone());
        ident
    }

    fn runtime_cc_ident(&mut self) -> Ident {
        if let Some(ident) = &self.runtime_cc_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("CC");
        self.runtime_cc_ident = Some(ident.clone());
        ident
    }

    fn runtime_cs_ident(&mut self) -> Ident {
        if let Some(ident) = &self.runtime_cs_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("CS");
        self.runtime_cs_ident = Some(ident.clone());
        ident
    }

    fn jsx_ident(&mut self) -> Ident {
        if let Some(ident) = &self.jsx_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("jsx");
        self.jsx_ident = Some(ident.clone());
        ident
    }

    fn jsxs_ident(&mut self) -> Ident {
        if let Some(ident) = &self.jsxs_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("jsxs");
        self.jsxs_ident = Some(ident.clone());
        ident
    }

    fn react_namespace_ident(&mut self) -> Ident {
        if let Some(ident) = &self.react_namespace_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("React");
        self.react_namespace_ident = Some(ident.clone());
        ident
    }

    fn forward_ref_ident(&mut self) -> Ident {
        if let Some(ident) = &self.forward_ref_ident {
            return ident.clone();
        }
        let ident = self.name_tracker.fresh_ident("forwardRef");
        self.forward_ref_ident = Some(ident.clone());
        ident
    }

    fn import_named_spec(local: &Ident, export: &str) -> ImportSpecifier {
        ImportSpecifier::Named(ImportNamedSpecifier {
            span: DUMMY_SP,
            local: local.clone(),
            imported: if local.sym.as_ref() == export {
                None
            } else {
                Some(ModuleExportName::Ident(Ident::new(export.into(), DUMMY_SP)))
            },
            is_type_only: false,
        })
    }

    fn is_css_ident(&self, expr: &Expr) -> bool {
        if let Expr::Ident(ident) = expr {
            self.css_imports.contains(&to_id(ident))
        } else {
            false
        }
    }

    fn is_keyframes_ident(&self, expr: &Expr) -> bool {
        if let Expr::Ident(ident) = expr {
            self.keyframes_imports.contains(&to_id(ident))
        } else {
            false
        }
    }

    fn evaluate_template(&self, template: &TaggedTpl) -> Option<String> {
        let mut result = String::new();
        for (index, quasi) in template.quasis.iter().enumerate() {
            result.push_str(
                quasi
                    .cooked
                    .as_ref()
                    .or_else(|| quasi.raw.as_ref())
                    .map(|atom| atom.to_string())
                    .unwrap_or_default()
                    .as_str(),
            );
            if let Some(expr) = template.exprs.get(index) {
                let value = evaluate_static(expr, &self.bindings)?;
                result.push_str(value.as_str()?);
            }
        }
        Some(result)
    }

    fn evaluate_call_argument(&self, call: &CallExpr) -> Option<StaticValue> {
        let first = call.args.first()?;
        evaluate_static(&first.expr, &self.bindings)
    }

    fn evaluate_call_arguments(&self, call: &CallExpr) -> Option<Vec<StaticValue>> {
        let mut values = Vec::with_capacity(call.args.len());
        for arg in &call.args {
            if arg.spread.is_some() {
                return None;
            }
            let value = evaluate_static(&arg.expr, &self.bindings)?;
            values.push(value);
        }
        Some(values)
    }

    fn collect_class_names_bindings(
        &self,
        pat: &Pat,
        css_idents: &mut HashSet<(JsWord, swc_core::common::SyntaxContext)>,
        style_idents: &mut HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    ) -> bool {
        match pat {
            Pat::Object(object) => {
                for prop in &object.props {
                    match prop {
                        ObjectPatProp::KeyValue(kv) => {
                            let name = match &kv.key {
                                PropName::Ident(ident) => ident.sym.as_ref().to_string(),
                                PropName::Str(str) => str.value.as_ref().to_string(),
                                _ => return false,
                            };
                            let binding = match binding_ident_from_pat(&kv.value) {
                                Some(ident) => ident,
                                None => return false,
                            };
                            let id = to_id(&binding);
                            match name.as_str() {
                                "css" => {
                                    css_idents.insert(id);
                                }
                                "style" => {
                                    style_idents.insert(id);
                                }
                                _ => {}
                            }
                        }
                        ObjectPatProp::Assign(assign) => {
                            let name = assign.key.sym.as_ref();
                            let id = to_id(&assign.key);
                            match name {
                                "css" => {
                                    css_idents.insert(id);
                                }
                                "style" => {
                                    style_idents.insert(id);
                                }
                                _ => {}
                            }
                        }
                        ObjectPatProp::Rest(_) => {}
                    }
                }
                true
            }
            Pat::Assign(assign) => {
                self.collect_class_names_bindings(&assign.left, css_idents, style_idents)
            }
            _ => false,
        }
    }

    fn extract_class_names_bindings(
        &self,
        params: &[Pat],
    ) -> Option<(
        HashSet<(JsWord, swc_core::common::SyntaxContext)>,
        HashSet<(JsWord, swc_core::common::SyntaxContext)>,
    )> {
        let first = params.first()?;
        let mut css_idents = HashSet::new();
        let mut style_idents = HashSet::new();
        if !self.collect_class_names_bindings(first, &mut css_idents, &mut style_idents) {
            return None;
        }
        if css_idents.is_empty() {
            return None;
        }
        Some((css_idents, style_idents))
    }

    fn handle_class_names_element(&mut self, element: &JSXElement) -> Option<Expr> {
        let ident = match &element.opening.name {
            JSXElementName::Ident(ident) => ident,
            _ => return None,
        };
        let id = to_id(ident);
        if self.compiled_import_kinds.get(&id) != Some(&CompiledImportKind::ClassNames) {
            return None;
        }

        let function_expr = element.children.iter().find_map(|child| {
            if let JSXChild::JSXExprContainer(container) = child {
                match &container.expr {
                    JSXExpr::Expr(expr) => Some(expr.as_ref().clone()),
                    _ => None,
                }
            } else {
                None
            }
        });

        let expr = match function_expr {
            Some(expr) => expr,
            None => {
                self.retain_imports.insert(id);
                return None;
            }
        };

        let (params, body_expr) = match expr {
            Expr::Arrow(arrow) => {
                let params = arrow.params.clone();
                let body = match &arrow.body {
                    BlockStmtOrExpr::Expr(expr) => (*expr.clone()),
                    BlockStmtOrExpr::BlockStmt(block) => Expr::Call(CallExpr {
                        span: block.span,
                        callee: Callee::Expr(Box::new(Expr::Arrow(ArrowExpr {
                            span: DUMMY_SP,
                            params: vec![],
                            body: BlockStmtOrExpr::BlockStmt(block.clone()),
                            is_async: arrow.is_async,
                            is_generator: arrow.is_generator,
                            type_params: None,
                            return_type: None,
                        }))),
                        args: vec![],
                        type_args: None,
                    }),
                };
                (params, body)
            }
            Expr::Fn(fn_expr) => {
                let params: Vec<Pat> = fn_expr
                    .function
                    .params
                    .iter()
                    .map(|param| param.pat.clone())
                    .collect();
                let body = Expr::Call(CallExpr {
                    span: fn_expr.function.span,
                    callee: Callee::Expr(Box::new(Expr::Fn(fn_expr.clone()))),
                    args: vec![],
                    type_args: None,
                });
                (params, body)
            }
            _ => {
                self.retain_imports.insert(id);
                return None;
            }
        };

        let (css_idents, style_idents) = match self.extract_class_names_bindings(&params) {
            Some(result) => result,
            None => {
                self.retain_imports.insert(id);
                return None;
            }
        };

        let mut rewritten = body_expr;
        {
            let mut visitor = ClassNamesBodyVisitor::new(self, css_idents, style_idents);
            rewritten.visit_mut_with(&mut visitor);
            if visitor.failed {
                self.retain_imports.insert(id);
                return None;
            }
            if !self.options.extract && !visitor.sheets.is_empty() {
                let key_expr = element
                    .opening
                    .attrs
                    .iter()
                    .find_map(|attr| match attr {
                        JSXAttrOrSpread::JSXAttr(attr)
                            if matches!(attr.name, JSXAttrName::Ident(ref ident) if ident.sym.as_ref() == "key") =>
                        {
                            attr.value.as_ref().and_then(|value| match value {
                                JSXAttrValue::JSXExprContainer(container) => match &container.expr {
                                    JSXExpr::Expr(expr) => Some((**expr).clone()),
                                    _ => None,
                                },
                                JSXAttrValue::Lit(Lit::Str(str)) => {
                                    Some(Expr::Lit(Lit::Str(str.clone())))
                                }
                                _ => None,
                            })
                        }
                        _ => None,
                    });
                rewritten = self.build_runtime_component(rewritten, visitor.sheets, key_expr);
            }
        }

        Some(rewritten)
    }

    fn handle_css_template(&mut self, template: &TaggedTpl) -> Option<Expr> {
        let css = self.evaluate_template(template)?;
        let artifacts = atomicize_literal(&css, &self.css_options());
        for rule in artifacts.rules {
            self.register_rule(rule.css);
        }
        for css in artifacts.raw_rules {
            self.register_rule(css);
        }
        Some(Expr::Lit(Lit::Null(Null { span: DUMMY_SP })))
    }

    fn handle_css_call(&mut self, call: &CallExpr) -> Option<Expr> {
        let values = self.evaluate_call_arguments(call)?;
        let mut combined = CssArtifacts::default();
        for value in &values {
            let artifacts = css_artifacts_from_static_value(value, &self.css_options())?;
            combined.merge(artifacts);
        }
        for rule in combined.rules {
            self.register_rule(rule.css);
        }
        for css in combined.raw_rules {
            self.register_rule(css);
        }
        Some(Expr::Lit(Lit::Null(Null { span: DUMMY_SP })))
    }

    fn handle_keyframes_template(&mut self, template: &TaggedTpl) -> Option<Expr> {
        let css = self.evaluate_template(template)?;
        self.emit_keyframes_rule(&css, &Expr::TaggedTpl(template.clone()))
    }

    fn handle_keyframes_call(&mut self, call: &CallExpr) -> Option<Expr> {
        let value = self.evaluate_call_argument(call)?;
        let css = value.as_object().and_then(build_css_from_object)?;
        self.emit_keyframes_rule(&css, &Expr::Call(call.clone()))
    }

    fn emit_keyframes_rule(&mut self, css: &str, expression: &Expr) -> Option<Expr> {
        let normalized = css.replace(' ', "");
        let name = format!("k{}", hash(&emit_expression(expression), 0));
        let rule = format!("@keyframes {}{{{}}}", name, normalized);
        self.register_rule(rule);
        Some(Expr::Lit(Lit::Null(Null { span: DUMMY_SP })))
    }

    fn preserve_import_for_ident(&mut self, ident: &Ident) {
        self.retain_imports.insert(to_id(ident));
    }

    fn preserve_import_for_expr(&mut self, expr: &Expr) {
        if let Expr::Ident(ident) = expr {
            self.preserve_import_for_ident(ident);
        }
    }

    fn preserve_styled_usage_in_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(call) => {
                if let Callee::Expr(callee_expr) = &call.callee {
                    match &**callee_expr {
                        Expr::Member(member) => {
                            if let Expr::Ident(obj) = &*member.obj {
                                if self.styled_imports.contains(&to_id(obj)) {
                                    self.preserve_import_for_ident(obj);
                                }
                            }
                        }
                        Expr::Ident(ident) => {
                            if self.styled_imports.contains(&to_id(ident)) {
                                self.preserve_import_for_ident(ident);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Expr::Ident(ident) => {
                if self.styled_imports.contains(&to_id(ident)) {
                    self.preserve_import_for_ident(ident);
                }
            }
            Expr::Member(member) => {
                if let Expr::Ident(obj) = &*member.obj {
                    if self.styled_imports.contains(&to_id(obj)) {
                        self.preserve_import_for_ident(obj);
                    }
                }
            }
            _ => {}
        }
    }

    fn inject_imports(&mut self, module: &mut Module) {
        let mut insertion_index = 0usize;
        for (index, item) in module.body.iter().enumerate() {
            match item {
                ModuleItem::ModuleDecl(ModuleDecl::Import(_)) => insertion_index = index + 1,
                _ => break,
            }
        }

        let mut imports = Vec::new();
        if self.needs_runtime_ax {
            let helper_export = self.runtime_class_helper();
            let helper_ident = self.runtime_class_ident();
            let ix_ident = self.runtime_ix_ident();
            let cc_ident = if self.needs_runtime_cc {
                Some(self.runtime_cc_ident())
            } else {
                None
            };
            let cs_ident = if self.needs_runtime_cs {
                Some(self.runtime_cs_ident())
            } else {
                None
            };
            let runtime_source = "@compiled/react/runtime";

            let runtime_import_index =
                module
                    .body
                    .iter()
                    .enumerate()
                    .find_map(|(idx, item)| match item {
                        ModuleItem::ModuleDecl(ModuleDecl::Import(import))
                            if import.src.value.as_ref() == runtime_source =>
                        {
                            Some(idx)
                        }
                        _ => None,
                    });

            if let Some(idx) = runtime_import_index {
                if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = module
                    .body
                    .get_mut(idx)
                    .expect("runtime import index valid")
                {
                    let mut has_helper = false;
                    let mut has_ix = false;
                    let mut has_cc = false;
                    let mut has_cs = false;

                    for spec in &import.specifiers {
                        if let ImportSpecifier::Named(named) = spec {
                            let local = named.local.sym.as_ref();
                            if local == helper_ident.sym.as_ref() {
                                has_helper = true;
                            }
                            if local == ix_ident.sym.as_ref() {
                                has_ix = true;
                            }
                            if let Some(cc_ident) = cc_ident.as_ref() {
                                if local == cc_ident.sym.as_ref() {
                                    has_cc = true;
                                }
                            }
                            if let Some(cs_ident) = cs_ident.as_ref() {
                                if local == cs_ident.sym.as_ref() {
                                    has_cs = true;
                                }
                            }
                        }
                    }

                    if !has_helper {
                        import
                            .specifiers
                            .push(Self::import_named_spec(&helper_ident, helper_export));
                    }

                    if !has_ix {
                        import
                            .specifiers
                            .push(Self::import_named_spec(&ix_ident, "ix"));
                    }

                    if let Some(cc_ident) = cc_ident.as_ref() {
                        if !has_cc {
                            import
                                .specifiers
                                .push(Self::import_named_spec(cc_ident, "CC"));
                        }
                    }

                    if let Some(cs_ident) = cs_ident.as_ref() {
                        if !has_cs {
                            import
                                .specifiers
                                .push(Self::import_named_spec(cs_ident, "CS"));
                        }
                    }
                }
            } else {
                let mut specifiers = Vec::new();
                specifiers.push(Self::import_named_spec(&helper_ident, helper_export));
                specifiers.push(Self::import_named_spec(&ix_ident, "ix"));
                if let Some(cc_ident) = cc_ident.as_ref() {
                    specifiers.push(Self::import_named_spec(cc_ident, "CC"));
                }
                if let Some(cs_ident) = cs_ident.as_ref() {
                    specifiers.push(Self::import_named_spec(cs_ident, "CS"));
                }

                imports.push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                    span: DUMMY_SP,
                    specifiers,
                    src: Box::new(Str::from(runtime_source)),
                    type_only: false,
                    with: None,
                })));
            }
        }
        if self.needs_jsx_runtime || self.needs_jsxs_runtime {
            let jsx_runtime_source = "react/jsx-runtime";
            let jsx_ident = if self.needs_jsx_runtime {
                Some(self.jsx_ident())
            } else {
                None
            };
            let jsxs_ident = if self.needs_jsxs_runtime {
                Some(self.jsxs_ident())
            } else {
                None
            };
            let jsx_import_index =
                module
                    .body
                    .iter()
                    .enumerate()
                    .find_map(|(idx, item)| match item {
                        ModuleItem::ModuleDecl(ModuleDecl::Import(import))
                            if import.src.value.as_ref() == jsx_runtime_source =>
                        {
                            Some(idx)
                        }
                        _ => None,
                    });

            if let Some(idx) = jsx_import_index {
                if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = module
                    .body
                    .get_mut(idx)
                    .expect("jsx runtime import index valid")
                {
                    let mut has_jsx = false;
                    let mut has_jsxs = false;
                    for spec in &import.specifiers {
                        if let ImportSpecifier::Named(named) = spec {
                            if let Some(ident) = jsx_ident.as_ref() {
                                if named.local.sym.as_ref() == ident.sym.as_ref() {
                                    has_jsx = true;
                                }
                            }
                            if let Some(ident) = jsxs_ident.as_ref() {
                                if named.local.sym.as_ref() == ident.sym.as_ref() {
                                    has_jsxs = true;
                                }
                            }
                        }
                    }
                    if let Some(ident) = jsx_ident.as_ref() {
                        if !has_jsx {
                            import
                                .specifiers
                                .push(Self::import_named_spec(ident, "jsx"));
                        }
                    }
                    if let Some(ident) = jsxs_ident.as_ref() {
                        if !has_jsxs {
                            import
                                .specifiers
                                .push(Self::import_named_spec(ident, "jsxs"));
                        }
                    }
                }
            } else {
                let mut specifiers = Vec::new();
                if let Some(ident) = jsx_ident.as_ref() {
                    specifiers.push(Self::import_named_spec(ident, "jsx"));
                }
                if let Some(ident) = jsxs_ident.as_ref() {
                    specifiers.push(Self::import_named_spec(ident, "jsxs"));
                }
                imports.push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                    span: DUMMY_SP,
                    specifiers,
                    src: Box::new(Str::from(jsx_runtime_source)),
                    type_only: false,
                    with: None,
                })));
            }
        }
        if self.needs_react_namespace && self.should_import_react_namespace() {
            if !self.has_react_namespace_binding {
                let ident = self.react_namespace_ident();
                imports.push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                    span: DUMMY_SP,
                    specifiers: vec![ImportSpecifier::Namespace(ImportStarAsSpecifier {
                        span: DUMMY_SP,
                        local: ident.clone(),
                    })],
                    src: Box::new(Str::from("react")),
                    type_only: false,
                    with: None,
                })));
                self.has_react_namespace_binding = true;
            }
        }
        if self.needs_forward_ref && !self.has_forward_ref_binding {
            let ident = self.forward_ref_ident();
            imports.push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                span: DUMMY_SP,
                specifiers: vec![Self::import_named_spec(&ident, "forwardRef")],
                src: Box::new(Str::from("react")),
                type_only: false,
                with: None,
            })));
            self.has_forward_ref_binding = true;
        }

        module
            .body
            .splice(insertion_index..insertion_index, imports);
    }

    fn append_display_names(&mut self, module: &mut Module) {
        for (ident, display_name) in self.styled_display_names.drain(..) {
            let condition = Expr::Bin(BinExpr {
                span: DUMMY_SP,
                op: BinaryOp::StrictEqEq,
                left: Box::new(Expr::Member(MemberExpr {
                    span: DUMMY_SP,
                    obj: Box::new(Expr::Member(MemberExpr {
                        span: DUMMY_SP,
                        obj: Box::new(Expr::Ident(quote_ident!("process"))),
                        prop: MemberProp::Ident(quote_ident!("env")),
                    })),
                    prop: MemberProp::Ident(quote_ident!("NODE_ENV")),
                })),
                right: Box::new(Expr::Lit(Lit::Str(Str::from("production")))),
            });
            let test = Expr::Unary(UnaryExpr {
                span: DUMMY_SP,
                op: UnaryOp::Bang,
                arg: Box::new(condition),
            });
            let stmt = Stmt::If(IfStmt {
                span: DUMMY_SP,
                test: Box::new(test),
                cons: Box::new(Stmt::Block(BlockStmt {
                    span: DUMMY_SP,
                    stmts: vec![Stmt::Expr(ExprStmt {
                        span: DUMMY_SP,
                        expr: Box::new(Expr::Assign(AssignExpr {
                            span: DUMMY_SP,
                            op: AssignOp::Assign,
                            left: PatOrExpr::Expr(Box::new(Expr::Member(MemberExpr {
                                span: DUMMY_SP,
                                obj: Box::new(Expr::Ident(ident.clone())),
                                prop: MemberProp::Ident(quote_ident!("displayName")),
                            }))),
                            right: Box::new(Expr::Lit(Lit::Str(Str::from(display_name)))),
                        })),
                    })],
                })),
                alt: None,
            });
            module.body.push(ModuleItem::Stmt(stmt));
        }
    }
}

impl<'a> VisitMut for TransformVisitor<'a> {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.css_imports.clear();
        self.keyframes_imports.clear();
        self.styled_imports.clear();
        self.css_map_imports.clear();
        self.compiled_import_kinds.clear();
        self.retain_imports.clear();
        for item in &module.body {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item {
                let source = import.src.value.to_string();
                if source == "react" {
                    for specifier in &import.specifiers {
                        match specifier {
                            ImportSpecifier::Named(named) => {
                                let imported_name = named
                                    .imported
                                    .as_ref()
                                    .map(|name| match name {
                                        ModuleExportName::Ident(ident) => ident.sym.as_ref(),
                                        ModuleExportName::Str(str) => str.value.as_ref(),
                                        ModuleExportName::Num(_) => "",
                                    })
                                    .unwrap_or_else(|| named.local.sym.as_ref());
                                if imported_name == "forwardRef" {
                                    self.has_forward_ref_binding = true;
                                    if self.forward_ref_ident.is_none() {
                                        self.forward_ref_ident = Some(named.local.clone());
                                    }
                                }
                            }
                            ImportSpecifier::Namespace(namespace) => {
                                self.has_react_namespace_binding = true;
                                if self.react_namespace_ident.is_none() {
                                    self.react_namespace_ident = Some(namespace.local.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if source == "@compiled/react/runtime" {
                    for specifier in &import.specifiers {
                        if let ImportSpecifier::Named(named) = specifier {
                            let imported_name = named
                                .imported
                                .as_ref()
                                .map(|name| match name {
                                    ModuleExportName::Ident(ident) => ident.sym.as_ref(),
                                    ModuleExportName::Str(str) => str.value.as_ref(),
                                    ModuleExportName::Num(_) => "",
                                })
                                .unwrap_or_else(|| named.local.sym.as_ref());

                            match imported_name {
                                "ax" | "ac" => {
                                    if imported_name == self.runtime_class_helper()
                                        && self.runtime_class_ident.is_none()
                                    {
                                        self.runtime_class_ident = Some(named.local.clone());
                                    }
                                }
                                "ix" => {
                                    if self.runtime_ix_ident.is_none() {
                                        self.runtime_ix_ident = Some(named.local.clone());
                                    }
                                }
                                "CC" => {
                                    if self.runtime_cc_ident.is_none() {
                                        self.runtime_cc_ident = Some(named.local.clone());
                                    }
                                }
                                "CS" => {
                                    if self.runtime_cs_ident.is_none() {
                                        self.runtime_cs_ident = Some(named.local.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                if source == "react/jsx-runtime" {
                    for specifier in &import.specifiers {
                        if let ImportSpecifier::Named(named) = specifier {
                            let imported_name = named
                                .imported
                                .as_ref()
                                .map(|name| match name {
                                    ModuleExportName::Ident(ident) => ident.sym.as_ref(),
                                    ModuleExportName::Str(str) => str.value.as_ref(),
                                    ModuleExportName::Num(_) => "",
                                })
                                .unwrap_or_else(|| named.local.sym.as_ref());

                            match imported_name {
                                "jsx" => {
                                    if self.jsx_ident.is_none() {
                                        self.jsx_ident = Some(named.local.clone());
                                    }
                                }
                                "jsxs" => {
                                    if self.jsxs_ident.is_none() {
                                        self.jsxs_ident = Some(named.local.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                if self.options.import_sources.contains(&source) {
                    for specifier in &import.specifiers {
                        match specifier {
                            ImportSpecifier::Named(named) => {
                                let imported_name = named
                                    .imported
                                    .as_ref()
                                    .map(|name| match name {
                                        ModuleExportName::Ident(ident) => ident.sym.as_ref(),
                                        ModuleExportName::Str(str) => str.value.as_ref(),
                                        ModuleExportName::Num(_) => "",
                                    })
                                    .unwrap_or_else(|| named.local.sym.as_ref());
                                let id = to_id(&named.local);
                                match imported_name {
                                    "css" => {
                                        self.css_imports.insert(id.clone());
                                        self.compiled_import_kinds
                                            .insert(id, CompiledImportKind::Css);
                                    }
                                    "keyframes" => {
                                        self.keyframes_imports.insert(id.clone());
                                        self.compiled_import_kinds
                                            .insert(id, CompiledImportKind::Keyframes);
                                    }
                                    "styled" => {
                                        self.styled_imports.insert(id.clone());
                                        self.compiled_import_kinds
                                            .insert(id, CompiledImportKind::Styled);
                                    }
                                    "cssMap" => {
                                        self.css_map_imports.insert(id.clone());
                                        self.compiled_import_kinds
                                            .insert(id, CompiledImportKind::CssMap);
                                    }
                                    "ClassNames" => {
                                        self.compiled_import_kinds
                                            .insert(id, CompiledImportKind::ClassNames);
                                    }
                                    _ => {}
                                }
                            }
                            ImportSpecifier::Default(spec) => {
                                if source == "@compiled/react" {
                                    let id = to_id(&spec.local);
                                    self.styled_imports.insert(id.clone());
                                    self.compiled_import_kinds
                                        .insert(id, CompiledImportKind::Styled);
                                }
                            }
                            ImportSpecifier::Namespace(_) => {}
                        }
                    }
                }
            }
        }

        module.visit_mut_children_with(self);

        let mut new_body = Vec::with_capacity(module.body.len());
        for item in module.body.drain(..) {
            match item {
                ModuleItem::ModuleDecl(ModuleDecl::Import(mut import)) => {
                    let source = import.src.value.to_string();
                    if self.options.import_sources.contains(&source) {
                        import.specifiers.retain(|specifier| match specifier {
                            ImportSpecifier::Named(named) => {
                                let id = to_id(&named.local);
                                if self.compiled_import_kinds.contains_key(&id) {
                                    self.retain_imports.contains(&id)
                                } else {
                                    true
                                }
                            }
                            ImportSpecifier::Default(spec) => {
                                let id = to_id(&spec.local);
                                if self.compiled_import_kinds.contains_key(&id) {
                                    self.retain_imports.contains(&id)
                                } else {
                                    true
                                }
                            }
                            ImportSpecifier::Namespace(_) => true,
                        });
                        if import.specifiers.is_empty()
                            && !import.type_only
                            && import.with.is_none()
                        {
                            continue;
                        }
                    }
                    new_body.push(ModuleItem::ModuleDecl(ModuleDecl::Import(import)));
                }
                other => new_body.push(other),
            }
        }
        module.body = new_body;
        if !self.options.extract && !self.hoisted_sheet_order.is_empty() {
            let mut insertion_index = 0usize;
            for (index, item) in module.body.iter().enumerate() {
                if matches!(item, ModuleItem::ModuleDecl(ModuleDecl::Import(_))) {
                    insertion_index = index + 1;
                } else {
                    break;
                }
            }

            let mut declarations = Vec::new();
            for css in &self.hoisted_sheet_order {
                if let Some(ident) = self.hoisted_sheets.get(css) {
                    declarations.push(ModuleItem::Stmt(Stmt::Decl(Decl::Var(VarDecl {
                        span: DUMMY_SP,
                        kind: VarDeclKind::Const,
                        decls: vec![VarDeclarator {
                            span: DUMMY_SP,
                            name: Pat::Ident(BindingIdent::from(ident.clone())),
                            init: Some(Box::new(Expr::Lit(Lit::Str(Str::from(css.clone()))))),
                            definite: false,
                        }],
                    }))));
                }
            }

            module
                .body
                .splice(insertion_index..insertion_index, declarations);
        }
        self.inject_imports(module);
        self.append_display_names(module);
        self.hoisted_sheet_order.clear();
        self.hoisted_sheets.clear();
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::JSXElement(element) = expr {
            if let Some(mut replacement) = self.handle_class_names_element(element) {
                replacement.visit_mut_with(self);
                *expr = replacement;
                return;
            }

            let css_info = self.process_css_prop(element);
            element.visit_mut_with(self);

            if let Some((sheets, key)) = css_info {
                if !self.options.extract && !sheets.is_empty() {
                    let inner = element.clone();
                    let wrapper = self.build_runtime_component(
                        Expr::JSXElement(Box::new(inner)),
                        sheets,
                        key,
                    );
                    *expr = wrapper;
                }
            }
            return;
        }
        expr.visit_mut_children_with(self);

        if let Expr::TaggedTpl(template) = expr.clone() {
            if self.is_css_ident(&template.tag) {
                if let Some(replacement) = self.handle_css_template(&template) {
                    *expr = replacement;
                } else {
                    self.preserve_import_for_expr(&template.tag);
                }
                return;
            }
            if self.is_keyframes_ident(&template.tag) {
                if let Some(replacement) = self.handle_keyframes_template(&template) {
                    *expr = replacement;
                } else {
                    self.preserve_import_for_expr(&template.tag);
                }
                return;
            }
        }

        if let Expr::Call(call) = expr.clone() {
            if let Callee::Expr(callee_expr) = &call.callee {
                if self.is_css_ident(callee_expr) {
                    if let Some(replacement) = self.handle_css_call(&call) {
                        *expr = replacement;
                    } else {
                        self.preserve_import_for_expr(callee_expr);
                    }
                    return;
                }
                if self.is_keyframes_ident(callee_expr) {
                    if let Some(replacement) = self.handle_keyframes_call(&call) {
                        *expr = replacement;
                    } else {
                        self.preserve_import_for_expr(callee_expr);
                    }
                    return;
                }
                if let Expr::Ident(ident) = &**callee_expr {
                    if self.css_map_imports.contains(&to_id(ident)) {
                        if let Some(value) = self.evaluate_call_argument(&call) {
                            if let StaticValue::Object(object) = value {
                                let mut props = Vec::new();
                                for (key, value) in &object {
                                    let variant_object = match value.as_object() {
                                        Some(inner) => inner,
                                        None => {
                                            self.retain_imports.insert(to_id(ident));
                                            return;
                                        }
                                    };
                                    let artifacts = match css_artifacts_from_static_object(
                                        variant_object,
                                        &self.css_options(),
                                    ) {
                                        Some(artifacts) => artifacts,
                                        None => {
                                            self.retain_imports.insert(to_id(ident));
                                            return;
                                        }
                                    };
                                    let mut class_names = Vec::new();
                                    for rule in &artifacts.rules {
                                        self.register_rule(rule.css.clone());
                                        class_names.push(rule.class_name.clone());
                                    }
                                    for css in &artifacts.raw_rules {
                                        self.register_rule(css.clone());
                                    }
                                    drop(artifacts);
                                    let joined = class_names.join(" ");
                                    props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(
                                        KeyValueProp {
                                            key: PropName::Ident(Ident::new(
                                                key.clone().into(),
                                                DUMMY_SP,
                                            )),
                                            value: Box::new(Expr::Lit(Lit::Str(Str::from(joined)))),
                                        },
                                    ))));
                                }
                                *expr = Expr::Object(ObjectLit {
                                    span: DUMMY_SP,
                                    props,
                                });
                                return;
                            }
                        }
                        self.retain_imports.insert(to_id(ident));
                    }
                }
            }
        }
    }

    fn visit_mut_var_declarator(&mut self, declarator: &mut VarDeclarator) {
        declarator.visit_mut_children_with(self);
        let init_expr = match &mut declarator.init {
            Some(init) => &mut **init,
            None => return,
        };
        let tagged = match init_expr {
            Expr::TaggedTpl(tagged) => tagged,
            other => {
                self.preserve_styled_usage_in_expr(other);
                return;
            }
        };
        let mut styled_source_ident: Option<Ident> = None;
        let default_component_expr = match &mut tagged.tag {
            Expr::Member(member) => {
                let styled_ident = match &*member.obj {
                    Expr::Ident(ident) => ident.clone(),
                    _ => return,
                };
                if !self.styled_imports.contains(&to_id(&styled_ident)) {
                    return;
                }
                styled_source_ident = Some(styled_ident.clone());
                match &member.prop {
                    MemberProp::Ident(ident) => {
                        Some(Expr::Lit(Lit::Str(Str::from(ident.sym.to_string()))))
                    }
                    MemberProp::Str(str) => Some(Expr::Lit(Lit::Str(str.clone()))),
                    MemberProp::Computed(comp) => {
                        if let Expr::Lit(Lit::Str(str)) = &*comp.expr {
                            Some(Expr::Lit(Lit::Str(str.clone())))
                        } else {
                            self.preserve_import_for_ident(&styled_ident);
                            None
                        }
                    }
                    _ => {
                        self.preserve_import_for_ident(&styled_ident);
                        None
                    }
                }
            }
            Expr::Call(call) => {
                let styled_ident = match &call.callee {
                    Callee::Expr(expr) => match &**expr {
                        Expr::Ident(ident) => ident.clone(),
                        _ => return,
                    },
                    _ => return,
                };
                if !self.styled_imports.contains(&to_id(&styled_ident)) {
                    return;
                }
                styled_source_ident = Some(styled_ident.clone());
                let first = match call.args.get(0) {
                    Some(arg) => arg,
                    None => {
                        self.preserve_import_for_ident(&styled_ident);
                        return;
                    }
                };
                if first.spread.is_some() {
                    self.preserve_import_for_ident(&styled_ident);
                    return;
                }
                Some((*first.expr).clone())
            }
            _ => return,
        };
        let default_component_expr = match default_component_expr {
            Some(expr) => expr,
            None => {
                if let Some(ident) = &styled_source_ident {
                    self.preserve_import_for_ident(ident);
                }
                return;
            }
        };
        let css = match self.evaluate_template(tagged) {
            Some(css) => css,
            None => {
                if let Some(ident) = &styled_source_ident {
                    self.preserve_import_for_ident(ident);
                }
                return;
            }
        };
        let component_name = match &declarator.name {
            Pat::Ident(binding) => Some(binding.id.sym.to_string()),
            _ => None,
        };
        let artifacts = atomicize_literal(&css, &self.css_options());
        for rule in &artifacts.rules {
            self.register_rule(rule.css.clone());
        }
        for css in &artifacts.raw_rules {
            self.register_rule(css.clone());
        }
        let mut runtime_sheets = Vec::new();
        for rule in &artifacts.rules {
            runtime_sheets.push(rule.css.clone());
        }
        for css in &artifacts.raw_rules {
            runtime_sheets.push(css.clone());
        }
        self.needs_react_namespace = true;
        self.needs_jsx_runtime = true;
        self.needs_runtime_ax = true;
        self.needs_forward_ref = true;

        let mut class_strings: Vec<ExprOrSpread> = Vec::new();
        if let Some(name) = component_name.as_ref() {
            if self.should_emit_component_class_name() {
                class_strings.push(ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Lit(Lit::Str(Str::from(format!("c_{}", name))))),
                });
            }
        }
        class_strings.extend(artifacts.rules.iter().map(|rule| ExprOrSpread {
            spread: None,
            expr: Box::new(Expr::Lit(Lit::Str(Str::from(rule.class_name.clone())))),
        }));

        let props_ident = self.name_tracker.fresh_ident("__cmplp");
        let style_ident = self.name_tracker.fresh_ident("__cmpls");
        let ref_ident = self.name_tracker.fresh_ident("__cmplr");
        let component_ident = self.name_tracker.fresh_ident("C");

        let mut object_props = Vec::new();
        object_props.push(ObjectPatProp::KeyValue(KeyValuePatProp {
            key: PropName::Ident(quote_ident!("as")),
            value: Box::new(Pat::Assign(AssignPat {
                span: DUMMY_SP,
                left: Box::new(Pat::Ident(BindingIdent::from(component_ident.clone()))),
                right: Box::new(default_component_expr),
                type_ann: None,
            })),
        }));
        object_props.push(ObjectPatProp::KeyValue(KeyValuePatProp {
            key: PropName::Ident(quote_ident!("style")),
            value: Box::new(Pat::Ident(BindingIdent::from(style_ident.clone()))),
        }));
        object_props.push(ObjectPatProp::Rest(RestPat {
            span: DUMMY_SP,
            dot3_token: DUMMY_SP,
            arg: Box::new(Pat::Ident(BindingIdent::from(props_ident.clone()))),
            type_ann: None,
        }));

        let params = vec![
            Pat::Object(ObjectPat {
                span: DUMMY_SP,
                props: object_props,
                optional: false,
                type_ann: None,
            }),
            Pat::Ident(BindingIdent::from(ref_ident.clone())),
        ];

        let class_array = Expr::Array(ArrayLit {
            span: DUMMY_SP,
            elems: class_strings
                .into_iter()
                .chain(std::iter::once(ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Member(MemberExpr {
                        span: DUMMY_SP,
                        obj: Box::new(Expr::Ident(props_ident.clone())),
                        prop: MemberProp::Ident(quote_ident!("className")),
                    })),
                }))
                .map(Some)
                .collect(),
        });

        let jsx_call = Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(self.jsx_ident()))),
            args: vec![
                ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Ident(component_ident.clone())),
                },
                ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Object(ObjectLit {
                        span: DUMMY_SP,
                        props: vec![
                            PropOrSpread::Spread(SpreadElement {
                                dot3_token: DUMMY_SP,
                                expr: Box::new(Expr::Ident(props_ident.clone())),
                                span: DUMMY_SP,
                            }),
                            PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                                key: PropName::Ident(quote_ident!("style")),
                                value: Box::new(Expr::Ident(style_ident.clone())),
                            }))),
                            PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                                key: PropName::Ident(quote_ident!("ref")),
                                value: Box::new(Expr::Ident(ref_ident.clone())),
                            }))),
                            PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                                key: PropName::Ident(quote_ident!("className")),
                                value: Box::new(Expr::Call(CallExpr {
                                    span: DUMMY_SP,
                                    callee: Callee::Expr(Box::new(Expr::Ident(
                                        self.runtime_class_ident(),
                                    ))),
                                    args: vec![ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(class_array),
                                    }],
                                    type_args: None,
                                })),
                            }))),
                        ],
                    })),
                },
            ],
            type_args: None,
        });

        let render_expr = if self.options.extract {
            jsx_call
        } else {
            self.build_runtime_component(jsx_call, runtime_sheets, None)
        };

        let guard = Stmt::If(IfStmt {
            span: DUMMY_SP,
            test: Box::new(Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: Box::new(Expr::Ident(props_ident.clone())),
                prop: MemberProp::Ident(quote_ident!("innerRef")),
            })),
            cons: Box::new(Stmt::Block(BlockStmt {
                span: DUMMY_SP,
                stmts: vec![Stmt::Throw(ThrowStmt {
                    span: DUMMY_SP,
                    arg: Box::new(Expr::New(NewExpr {
                        span: DUMMY_SP,
                        callee: Box::new(Expr::Ident(quote_ident!("Error"))),
                        args: Some(vec![ExprOrSpread {
                            spread: None,
                            expr: Box::new(Expr::Lit(Lit::Str(Str::from(
                                "Please use 'ref' instead of 'innerRef'.",
                            )))),
                        }]),
                        type_args: None,
                    })),
                })],
            })),
            alt: None,
        });

        let arrow = Expr::Arrow(ArrowExpr {
            span: DUMMY_SP,
            params,
            body: BlockStmtOrExpr::BlockStmt(BlockStmt {
                span: DUMMY_SP,
                stmts: {
                    let mut stmts = Vec::new();
                    if self.is_development_env() {
                        stmts.push(guard);
                    }
                    stmts.push(Stmt::Return(ReturnStmt {
                        span: DUMMY_SP,
                        arg: Some(Box::new(render_expr)),
                    }));
                    stmts
                },
            }),
            is_async: false,
            is_generator: false,
            type_params: None,
            return_type: None,
        });

        *init = Box::new(Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(self.forward_ref_ident()))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(arrow),
            }],
            type_args: None,
        }));

        if let Pat::Ident(BindingIdent { id, .. }) = &declarator.name {
            self.styled_display_names
                .push((id.clone(), id.sym.to_string()));
        }
    }

    fn handle_xcss_attributes(&mut self, element: &mut JSXOpeningElement) {
        if !self.options.process_xcss {
            return;
        }

        for attr in &mut element.attrs {
            let JSXAttrOrSpread::JSXAttr(attr) = attr else {
                continue;
            };
            let JSXAttrName::Ident(name) = &attr.name else {
                continue;
            };
            if !name.sym.as_ref().to_ascii_lowercase().ends_with("xcss") {
                continue;
            }

            let Some(JSXAttrValue::JSXExprContainer(container)) = &mut attr.value else {
                continue;
            };

            let JSXExpr::Expr(expr) = &mut container.expr else {
                continue;
            };

            if !matches!(**expr, Expr::Object(_)) {
                continue;
            }

            let evaluated = evaluate_static(expr, &self.bindings)
                .and_then(|value| match value {
                    StaticValue::Object(map) => Some(map),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Object given to the xcss prop must be static"));

            let artifacts = css_artifacts_from_static_object(&evaluated, &self.css_options())
                .unwrap_or_else(|| panic!("Object given to the xcss prop must be static"));

            let mut class_names = Vec::new();
            for rule in &artifacts.rules {
                self.register_rule(rule.css.clone());
                class_names.push(rule.class_name.clone());
            }
            for css in &artifacts.raw_rules {
                self.register_rule(css.clone());
            }

            if class_names.is_empty() {
                **expr = Expr::Ident(quote_ident!("undefined"));
            } else {
                let joined = class_names.join(" ");
                **expr = Expr::Lit(Lit::Str(Str::from(joined)));
            }
        }
    }

    fn process_css_prop(
        &mut self,
        element: &mut JSXElement,
    ) -> Option<(Vec<String>, Option<Expr>)> {
        let mut css_index = None;
        let mut class_index = None;
        let mut key_expr: Option<Expr> = None;

        for (index, attr) in element.opening.attrs.iter().enumerate() {
            let JSXAttrOrSpread::JSXAttr(attr) = attr else {
                continue;
            };

            let JSXAttrName::Ident(name) = &attr.name else {
                continue;
            };

            match name.sym.as_ref() {
                "css" => css_index = Some(index),
                "className" => class_index = Some(index),
                "key" => {
                    if let Some(value) = &attr.value {
                        key_expr = match value {
                            JSXAttrValue::JSXExprContainer(container) => match &container.expr {
                                JSXExpr::Expr(expr) => Some((**expr).clone()),
                                _ => None,
                            },
                            JSXAttrValue::Lit(Lit::Str(str)) => {
                                Some(Expr::Lit(Lit::Str(str.clone())))
                            }
                            _ => None,
                        };
                    }
                }
                _ => {}
            }
        }

        let css_index = css_index?;
        let css_attr = match &element.opening.attrs[css_index] {
            JSXAttrOrSpread::JSXAttr(attr) => attr,
            _ => return None,
        };

        let css_value = match &css_attr.value {
            Some(JSXAttrValue::JSXExprContainer(container)) => match &container.expr {
                JSXExpr::Expr(expr) => evaluate_static(expr, &self.bindings),
                _ => None,
            },
            Some(JSXAttrValue::Lit(Lit::Str(str))) => Some(StaticValue::Str(str.value.to_string())),
            _ => None,
        };

        let css_map = css_value.and_then(|value| value.as_object().cloned())?;
        let artifacts = css_artifacts_from_static_object(&css_map, &self.css_options())?;

        let mut runtime_sheets = Vec::new();
        for rule in &artifacts.rules {
            self.register_rule(rule.css.clone());
            runtime_sheets.push(rule.css.clone());
        }
        for css in &artifacts.raw_rules {
            self.register_rule(css.clone());
            runtime_sheets.push(css.clone());
        }

        self.needs_runtime_ax = true;
        self.needs_jsx_runtime = true;
        self.needs_react_namespace = true;

        let mut class_entries: Vec<ExprOrSpread> = artifacts
            .rules
            .iter()
            .map(|rule| ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Lit(Lit::Str(Str::from(rule.class_name.clone())))),
            })
            .collect();

        if let Some(index) = class_index {
            if let JSXAttrOrSpread::JSXAttr(class_attr) = &element.opening.attrs[index] {
                if let Some(value) = &class_attr.value {
                    let expr = match value {
                        JSXAttrValue::JSXExprContainer(container) => match &container.expr {
                            JSXExpr::Expr(expr) => Some((**expr).clone()),
                            _ => None,
                        },
                        JSXAttrValue::Lit(lit) => match lit {
                            Lit::Str(str) => Some(Expr::Lit(Lit::Str(str.clone()))),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(expr) = expr {
                        class_entries.push(ExprOrSpread {
                            spread: None,
                            expr: Box::new(expr),
                        });
                    }
                }
            }
        }

        let class_attr = JSXAttrOrSpread::JSXAttr(JSXAttr {
            span: DUMMY_SP,
            name: JSXAttrName::Ident(quote_ident!("className")),
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                span: DUMMY_SP,
                expr: JSXExpr::Expr(Box::new(Expr::Call(CallExpr {
                    span: DUMMY_SP,
                    callee: Callee::Expr(Box::new(Expr::Ident(self.runtime_class_ident()))),
                    args: vec![ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Array(ArrayLit {
                            span: DUMMY_SP,
                            elems: class_entries.into_iter().map(Some).collect(),
                        })),
                    }],
                    type_args: None,
                }))),
            })),
        });

        let mut new_attrs = Vec::new();
        for (index, attr) in element.opening.attrs.iter().enumerate() {
            if index == css_index {
                continue;
            }
            if Some(index) == class_index {
                continue;
            }
            new_attrs.push(attr.clone());
        }
        new_attrs.push(class_attr);
        element.opening.attrs = new_attrs;

        Some((runtime_sheets, key_expr))
    }

    fn build_runtime_component(
        &mut self,
        child: Expr,
        sheets: Vec<String>,
        key: Option<Expr>,
    ) -> Expr {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut sheet_exprs = Vec::new();
        for css in sheets {
            if seen.insert(css.clone()) {
                let ident = self.hoist_sheet_ident(&css);
                sheet_exprs.push(ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Ident(ident)),
                });
            }
        }

        let sheets_array = Expr::Array(ArrayLit {
            span: DUMMY_SP,
            elems: sheet_exprs.into_iter().map(Some).collect(),
        });

        let mut cs_props = Vec::new();
        if let Some(nonce) = &self.options.nonce {
            cs_props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                key: PropName::Ident(quote_ident!("nonce")),
                value: Box::new(Expr::Ident(Ident::new(nonce.clone().into(), DUMMY_SP))),
            }))));
        }
        cs_props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
            key: PropName::Ident(quote_ident!("children")),
            value: Box::new(sheets_array),
        }))));

        let cs_call = Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(self.jsx_ident()))),
            args: vec![
                ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Ident(self.runtime_cs_ident())),
                },
                ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Object(ObjectLit {
                        span: DUMMY_SP,
                        props: cs_props,
                    })),
                },
            ],
            type_args: None,
        });

        let children_array = Expr::Array(ArrayLit {
            span: DUMMY_SP,
            elems: vec![
                Some(ExprOrSpread {
                    spread: None,
                    expr: Box::new(cs_call),
                }),
                Some(ExprOrSpread {
                    spread: None,
                    expr: Box::new(child),
                }),
            ],
        });

        let mut cc_props = Vec::new();
        cc_props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
            key: PropName::Ident(quote_ident!("children")),
            value: Box::new(children_array),
        }))));

        let mut args = vec![
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Ident(self.runtime_cc_ident())),
            },
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Object(ObjectLit {
                    span: DUMMY_SP,
                    props: cc_props,
                })),
            },
        ];

        if let Some(key_expr) = key {
            args.push(ExprOrSpread {
                spread: None,
                expr: Box::new(key_expr),
            });
        }

        self.needs_runtime_cc = true;
        self.needs_runtime_cs = true;
        self.needs_jsx_runtime = true;
        self.needs_jsxs_runtime = true;

        Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(self.jsxs_ident()))),
            args,
            type_args: None,
        })
    }

    fn visit_mut_jsx_opening_element(&mut self, element: &mut JSXOpeningElement) {
        element.visit_mut_children_with(self);
        self.handle_xcss_attributes(element);
    }
}

fn parse_transformed_source(code: &str, filename: &str) -> Program {
    use std::sync::Arc;

    let cm: Arc<swc_core::common::SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(filename.into()), code.into());

    let syntax_for_file = |name: &str| {
        if name.ends_with(".ts") || name.ends_with(".tsx") || name.ends_with(".cts") {
            Syntax::Typescript(TsConfig {
                tsx: name.ends_with(".tsx"),
                decorators: true,
                dynamic_import: true,
                import_assertions: true,
                ..Default::default()
            })
        } else {
            Syntax::Es(EsConfig {
                jsx: name.ends_with(".jsx") || name.ends_with(".tsx"),
                decorators: true,
                export_default_from: true,
                import_assertions: true,
                dynamic_import: true,
                top_level_await: true,
                ..Default::default()
            })
        }
    };

    let make_parser = |syntax: Syntax| {
        let lexer = swc_core::ecma::parser::lexer::Lexer::new(
            syntax,
            EsVersion::Es2022,
            StringInput::from(&*fm),
            None,
        );
        Parser::new_from(lexer)
    };

    let mut parser = make_parser(syntax_for_file(filename));
    match parser.parse_module() {
        Ok(module) => Program::Module(module),
        Err(module_err) => {
            let mut parser = make_parser(syntax_for_file(filename));
            match parser.parse_script() {
                Ok(script) => Program::Script(script),
                Err(script_err) => panic!(
                    "failed to parse emitted output as module ({module_err}), and as script ({script_err})"
                ),
            }
        }
    }
}

fn transform_program(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let config_value = metadata
        .get_transform_plugin_config()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or(Value::Object(serde_json::Map::new()));

    let mut options: PluginOptions = serde_json::from_value(config_value).unwrap_or_default();
    if options.import_sources.is_empty() {
        options.import_sources.push("@compiled/react".into());
    }

    let context: Option<TransformPluginMetadataContext> = metadata.get_context();
    let filename = context
        .as_ref()
        .and_then(|ctx| ctx.filename.clone())
        .unwrap_or_else(|| "unknown.js".to_string());

    let mut program = program;
    if let Program::Module(mut module) = program {
        let file_path = PathBuf::from(&filename);
        let file_dir = file_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let project_root = std::env::current_dir().unwrap_or_else(|_| file_dir.clone());
        let evaluator = ModuleEvaluator::new(&file_dir, &options.extensions);
        let bindings =
            collect_static_bindings(&module, Some(&evaluator), Some(file_path.as_path()));
        let name_tracker = NameTracker::from_module(&module);
        let mut visitor = TransformVisitor::new(&options, bindings, name_tracker);
        module.visit_mut_with(&mut visitor);

        let collected_rules = visitor.collected_rules.clone();
        if options.compiled_require_exclude.unwrap_or(false) {
            // Skip any runtime require hooks when exclusion is requested.
        } else if let Some(path) = options.style_sheet_path.as_ref() {
            append_stylesheet_requires(&mut module, path, &collected_rules);
        }

        if let Some(ref extract_opts) = options.extract_styles_to_directory {
            let sort_at_rules = options.sort_at_rules.unwrap_or(true);
            let sort_shorthand = options.sort_shorthand.unwrap_or(true);
            if let Err(message) = write_stylesheet_to_directory(
                &mut module,
                extract_opts,
                &collected_rules,
                sort_at_rules,
                sort_shorthand,
                &file_path,
                &project_root,
            ) {
                panic!("{message}");
            }
        }

        let included_files: Vec<String> = evaluator
            .included_files()
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();

        if let Ok(mut guard) = LATEST_ARTIFACTS.lock() {
            guard.style_rules = collected_rules.clone();
            guard.metadata = json!({
                "styleRules": guard.style_rules,
                "includedFiles": included_files,
            });
        }
        program = Program::Module(module);
    }

    parse_transformed_source(
        &program_to_source(&program).expect("failed to emit program"),
        &filename,
    )
}

#[plugin_transform]
pub fn transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    transform_program(program, metadata)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        collect_static_bindings, program_to_source, take_latest_artifacts, transform_program,
        ExtractStylesToDirectoryOptions, ModuleEvaluator, PluginOptions, StyleArtifacts,
        TransformVisitor,
    };
    use serde_json::Value;
    use swc_core::common::{FileName, SourceMap};
    use swc_core::ecma::ast::Program;
    use swc_core::ecma::parser::EsVersion;
    use swc_core::ecma::parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};
    use swc_core::ecma::visit::VisitMutWith;
    use swc_core::plugin::metadata::TransformPluginMetadataContext;
    use swc_core::plugin::proxies::TransformPluginProgramMetadata;

    fn parse(code: &str) -> Program {
        use std::sync::Arc;

        let cm: Arc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom("test.tsx".into()), code.into());
        let lexer = Lexer::new(
            Syntax::Typescript(TsConfig {
                tsx: true,
                decorators: true,
                ..Default::default()
            }),
            EsVersion::Es2022,
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        Program::Module(parser.parse_module().expect("module"))
    }

    fn transform_source(input: &str) -> (String, StyleArtifacts) {
        let program = parse(input);
        let metadata = TransformPluginProgramMetadata::default();
        let transformed = transform_program(program, metadata);
        let emitted = program_to_source(&transformed).expect("emit program");
        let artifacts = take_latest_artifacts();
        (emitted, artifacts)
    }

    fn transform_source_with_options(
        input: &str,
        options: PluginOptions,
    ) -> (String, StyleArtifacts) {
        let program = parse(input);
        let mut metadata = TransformPluginProgramMetadata::default();
        metadata.transform_plugin_config =
            Some(serde_json::to_string(&options).expect("serialize options"));
        let transformed = transform_program(program, metadata);
        let emitted = program_to_source(&transformed).expect("emit program");
        let artifacts = take_latest_artifacts();
        (emitted, artifacts)
    }

    #[test]
    fn css_literal_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst className = css`color: red;`;\n",
        );
        assert!(emitted.contains("const className = null"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn css_alias_import_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { css as CompiledCss } from '@compiled/react';\nconst className = CompiledCss`color: red;`;\n",
        );
        assert!(emitted.contains("const className = null"));
        assert!(!emitted.contains("CompiledCss"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn css_content_property_matches_babel_blank() {
        let (_, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ content: '' });\n",
        );
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule == "._1sb2b3bt{content:\"\"}"));
    }

    #[test]
    fn css_content_property_adds_quotes() {
        let (_, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ content: 'hello' });\n",
        );
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule == "._1sb21e8g{content:\"hello\"}"));
    }

    #[test]
    fn css_content_property_preserves_existing_quotes() {
        let (_, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ content: \"'hello'\" });\n",
        );
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule == "._1sb25hbz{content:'hello'}"));
    }

    #[test]
    fn css_map_nested_selectors_align_with_babel() {
        let source = r#"import { cssMap } from '@compiled/react';

const styles = cssMap({
  success: {
    color: '#0b0',
    '&:hover': {
      color: '#060',
    },
    '@media': {
      'screen and (min-width: 500px)': {
        fontSize: '10vw',
      },
    },
    selectors: {
      span: {
        color: 'lightgreen',
        '&:hover': {
          color: '#090',
        },
      },
    },
  },
  danger: {
    color: 'red',
    '&:hover': {
      color: 'darkred',
    },
    '@media': {
      'screen and (min-width: 500px)': {
        fontSize: '20vw',
      },
    },
    selectors: {
      span: {
        color: 'orange',
        '&:hover': {
          color: 'pink',
        },
      },
    },
  },
});

const Element = (variant) => <div css={styles[variant]} />;"#;
        let (_, artifacts) = transform_source(source);
        for expected in [
            "._syazjafr{color:#0b0}",
            "._30l3aebp:hover{color:#060}",
            "@media screen and (min-width:500px){._1takoyl8{font-size:10vw}}",
            "._1tjq1v9d span{color:lightgreen}",
            "._yzbcy77s span:hover{color:#090}",
            "._syaz5scu{color:red}",
            "._30l3qaj3:hover{color:darkred}",
            "@media screen and (min-width:500px){._1taki9ra{font-size:20vw}}",
            "._1tjqruxl span{color:orange}",
            "._yzbc32ev span:hover{color:pink}",
        ] {
            assert!(
                artifacts.style_rules.iter().any(|rule| rule == expected),
                "missing expected rule {expected}"
            );
        }
    }

    #[test]
    fn css_literal_resolves_imported_values() {
        let temp_root = std::env::temp_dir().join(format!(
            "compiled_swc_plugin_test_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_root).expect("create temp root");
        let dep_path = temp_root.join("tokens.ts");
        fs::write(&dep_path, "export const brand = 'green';\n").expect("write dep");
        let entry_path = temp_root.join("entry.tsx");
        let source = r#"import { css } from '@compiled/react';
import { brand } from './tokens';
const className = css`color: ${brand};`;
"#;
        fs::write(&entry_path, source).expect("write entry");

        let program = parse(source);
        let mut module = match program {
            Program::Module(module) => module,
            _ => panic!("expected module"),
        };
        let evaluator = ModuleEvaluator::new(&temp_root, &Vec::new());
        let bindings =
            collect_static_bindings(&module, Some(&evaluator), Some(entry_path.as_path()));
        let name_tracker = NameTracker::from_module(&module);
        let mut visitor = TransformVisitor::new(&PluginOptions::default(), bindings, name_tracker);
        module.visit_mut_with(&mut visitor);
        assert!(visitor
            .collected_rules
            .iter()
            .any(|rule| rule.contains("color:green")));
        let included = evaluator.included_files();
        assert!(included
            .iter()
            .any(|path| path.to_string_lossy().contains("tokens.ts")));
        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn css_literal_resolves_with_custom_extension() {
        let temp_root = std::env::temp_dir().join(format!(
            "compiled_swc_plugin_custom_ext_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_root).expect("create temp root");

        let dep_path = temp_root.join("tokens.custom");
        fs::write(&dep_path, "export const brand = 'green';\n").expect("write dep");

        let entry_path = temp_root.join("entry.tsx");
        let source = r#"import { css } from '@compiled/react';
import { brand } from './tokens';
const className = css({ color: brand });
"#;
        fs::write(&entry_path, source).expect("write entry");

        let program = parse(source);
        let mut metadata = TransformPluginProgramMetadata::default();
        let mut options = PluginOptions::default();
        options.extensions = vec![".custom".into()];
        metadata.transform_plugin_config =
            Some(serde_json::to_string(&options).expect("serialize options"));
        metadata.context = Some(TransformPluginMetadataContext {
            filename: Some(entry_path.to_string_lossy().to_string()),
            cwd: Some(temp_root.to_string_lossy().to_string()),
            ..Default::default()
        });

        let transformed = transform_program(program, metadata);
        let emitted = program_to_source(&transformed).expect("emit program");
        assert!(emitted.contains("const className = null"));

        let artifacts = take_latest_artifacts();
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:green")));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn css_object_resolves_computed_selector_and_value() {
        let source = r#"import { css } from '@compiled/react';
const palette = { success: 'green' };
const media = { breakpoints: { sm: '@media screen and (min-width: 30rem)' } };
const className = css({
  [media.breakpoints.sm]: {
    color: palette.success,
  },
});
"#;

        let (emitted, artifacts) = transform_source(source);
        assert!(emitted.contains("const className = null"));
        assert!(artifacts.style_rules.iter().any(|rule| {
            let has_media = rule.contains("@media screen and (min-width:30rem)")
                || rule.contains("@media screen and (min-width: 30rem)");
            has_media && rule.contains("color:green")
        }));
    }

    #[test]
    fn styled_component_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { styled } from '@compiled/react';\nconst Styled = styled.div`color: red;`;\n",
        );
        assert!(emitted.contains("import { ax, ix } from \"@compiled/react/runtime\";"));
        assert!(emitted.contains("forwardRef"));
        assert!(emitted.contains("Styled.displayName"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn styled_call_component_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { styled } from '@compiled/react';\nconst Button = 'button';\nconst Styled = styled(Button)`color: red;`;\n",
        );
        assert!(emitted.contains("import { ax, ix } from \"@compiled/react/runtime\";"));
        assert!(emitted.contains("forwardRef"));
        assert!(emitted.contains("Styled.displayName"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn styled_adds_component_class_when_enabled() {
        let prev = std::env::var("NODE_ENV").ok();
        std::env::remove_var("NODE_ENV");
        let mut options = PluginOptions::default();
        options.add_component_name = Some(true);
        let (emitted, _) = transform_source_with_options(
            "import { styled } from '@compiled/react';\nconst MyDiv = styled.div`color: red;`;\n",
            options,
        );
        if let Some(prev) = prev {
            std::env::set_var("NODE_ENV", prev);
        } else {
            std::env::remove_var("NODE_ENV");
        }
        assert!(emitted.contains("\"c_MyDiv\""));
    }

    #[test]
    fn styled_runtime_wraps_with_cc() {
        let mut options = PluginOptions::default();
        options.extract = false;
        let (emitted, artifacts) = transform_source_with_options(
            "import { styled } from '@compiled/react';\nconst Styled = styled.div({ color: 'red' });\n",
            options,
        );
        assert!(emitted.contains("jsxs(CC,{"));
        assert!(emitted.contains("jsx(CS,{"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn styled_reuses_forward_ref_alias() {
        let (emitted, _) = transform_source(
            "import { forwardRef as useForwardRef } from 'react';\nimport { styled } from '@compiled/react';\nconst Styled = styled.div`color: red;`;\n",
        );
        assert!(emitted.contains("useForwardRef("));
        assert!(!emitted.contains("import { forwardRef"));
    }

    #[test]
    fn styled_skips_inner_ref_guard_in_production() {
        let prev = std::env::var("NODE_ENV").ok();
        std::env::set_var("NODE_ENV", "production");
        let (emitted, _) = transform_source(
            "import { styled } from '@compiled/react';\nconst Styled = styled.div`color: red;`;\n",
        );
        if let Some(prev) = prev {
            std::env::set_var("NODE_ENV", prev);
        } else {
            std::env::remove_var("NODE_ENV");
        }
        assert!(!emitted.contains("innerRef"));
    }

    #[test]
    fn keyframes_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { keyframes } from '@compiled/react';\nconst fadeIn = keyframes`from { opacity: 0; } to { opacity: 1; }`;\n",
        );
        assert!(emitted.contains("const fadeIn = null"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("@keyframes")));
    }

    #[test]
    fn keyframes_alias_import_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { keyframes as animation } from '@compiled/react';\nconst fadeIn = animation`from { opacity: 0; } to { opacity: 1; }`;\n",
        );
        assert!(emitted.contains("const fadeIn = null"));
        assert!(!emitted.contains("animation`"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("@keyframes")));
    }

    #[test]
    fn css_map_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { cssMap } from '@compiled/react';\nconst map = cssMap({ primary: { backgroundColor: 'red' } });\n",
        );
        assert!(emitted.contains("primary"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("background-color:red")));
    }

    #[test]
    fn css_map_alias_import_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { cssMap as compiledMap } from '@compiled/react';\nconst map = compiledMap({ primary: { backgroundColor: 'red' } });\n",
        );
        assert!(emitted.contains("primary"));
        assert!(!emitted.contains("compiledMap"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("background-color:red")));
    }

    #[test]
    fn css_prop_in_jsx_transforms() {
        let (emitted, artifacts) = transform_source(
            "import '@compiled/react';\nconst styles = { color: 'red' };\nconst Component = () => <div css={styles} />;\n",
        );
        assert!(emitted.contains("className"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn css_prop_nested_rules_match_babel_hashes() {
        let (_, artifacts) = transform_source(
            "import '@compiled/react';\nconst Component = () => (\n  <div css={{\n    color: 'red',\n    '&:hover': { color: 'blue' },\n    '@media': {\n      'screen and (min-width: 500px)': {\n        color: 'green',\n      },\n    },\n    content: ''\n  }} />\n);\n",
        );

        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule == "._syaz5scu{color:red}"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule == "._1sb2b3bt{content:\"\"}"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule == "._30l313q2:hover{color:blue}"));
        assert!(artifacts.style_rules.iter().any(|rule| {
            rule.contains("@media") && rule.contains("_f8e2bf54") && rule.contains("color:green")
        }));
    }

    #[test]
    fn css_prop_runtime_wraps_with_cc() {
        let mut options = PluginOptions::default();
        options.extract = false;
        let (emitted, artifacts) = transform_source_with_options(
            "import '@compiled/react';\nconst Component = () => <div css={{ color: 'red' }} />;\n",
            options,
        );
        assert!(emitted.contains("import { ax, ix, CC, CS } from \"@compiled/react/runtime\";"));
        assert!(emitted.contains("import { jsx, jsxs } from \"react/jsx-runtime\";"));
        assert!(emitted.contains("jsxs(CC,{"));
        assert!(emitted.contains("jsx(CS,{"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn css_prop_runtime_honors_nonce() {
        let mut options = PluginOptions::default();
        options.extract = false;
        options.nonce = Some("__webpack_nonce__".into());
        let (emitted, _) = transform_source_with_options(
            "import '@compiled/react';\nconst Component = () => <div css={{ color: 'red' }} />;\n",
            options,
        );
        assert!(emitted.contains("nonce:__webpack_nonce__"));
    }

    #[test]
    fn class_names_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { ClassNames } from '@compiled/react';\nconst Component = () => (\n  <ClassNames>{({ css }) => <div className={css({ color: 'red' })} />}</ClassNames>\n);\n",
        );
        assert!(!emitted.contains("ClassNames"));
        assert!(emitted.contains("ax(["));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn runtime_helpers_avoid_name_collisions() {
        let mut options = PluginOptions::default();
        options.extract = false;
        let (emitted, _) = transform_source_with_options(
            "import { css } from '@compiled/react';\nconst ax = 'one';\nconst ix = 'two';\nconst jsx = 'three';\nconst jsxs = 'four';\nconst Component = () => <div css={{ color: 'red' }} />;\n",
            options,
        );
        assert!(emitted.contains("ax as _ax"));
        assert!(emitted.contains("ix as _ix"));
        assert!(emitted.contains("jsx as _jsx"));
        assert!(emitted.contains("jsxs as _jsxs"));
        assert!(emitted.contains("_ax(["));
    }

    #[test]
    fn hoisted_sheet_identifiers_avoid_collisions() {
        let mut options = PluginOptions::default();
        options.extract = false;
        let (emitted, _) = transform_source_with_options(
            "import { styled } from '@compiled/react';\nconst _ = 'keep';\nconst Styled = styled.div({ color: 'red' });\n",
            options,
        );
        assert!(emitted.contains("const _ = 'keep'") || emitted.contains("const _ = \"keep\""));
        assert!(emitted.contains("const _1 = \"") || emitted.contains("const _1 = '"));
    }

    #[test]
    fn styled_alias_import_transforms() {
        let (emitted, artifacts) = transform_source(
            "import { styled as compileStyled } from '@compiled/react';\nconst Styled = compileStyled.div`color: red;`;\n",
        );
        assert!(emitted.contains("forwardRef"));
        assert!(!emitted.contains("compileStyled"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn class_names_runtime_wraps_with_cc() {
        let mut options = PluginOptions::default();
        options.extract = false;
        let (emitted, artifacts) = transform_source_with_options(
            "import { ClassNames } from '@compiled/react';\nconst Component = () => (\n  <ClassNames>{({ css }) => <div className={css({ color: 'red' })} />}</ClassNames>\n);\n",
            options,
        );
        assert!(emitted.contains("jsxs(CC,{"));
        assert!(emitted.contains("jsx(CS,{"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn css_call_with_selectors() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ selectors: { '&:hover': { color: 'red' } } });\n",
        );
        assert!(emitted.contains("const styles = null"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains(":hover{color:red}")));
    }

    #[test]
    fn css_call_with_at_rule() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ '@media screen and (min-width: 500px)': { color: 'red' } });\n",
        );
        assert!(emitted.contains("const styles = null"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("@media")));
    }

    #[test]
    fn css_increase_specificity_appends_selector() {
        let mut options = PluginOptions::default();
        options.increase_specificity = Some(true);
        let (_, artifacts) = transform_source_with_options(
            "import { css } from '@compiled/react';\nconst styles = css({ color: 'red' });\n",
            options,
        );
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains(":not(#")));
    }

    #[test]
    fn css_flattens_multiple_selectors_when_disabled() {
        let mut options = PluginOptions::default();
        options.flatten_multiple_selectors = Some(false);
        let (_, artifacts) = transform_source_with_options(
            "import { css } from '@compiled/react';\nconst styles = css({ selectors: { 'div, span': { color: 'red' } } });\n",
            options,
        );
        let combined = artifacts
            .style_rules
            .iter()
            .find(|rule| rule.contains("div") && rule.contains("span"))
            .expect("expected combined selector rule");
        assert!(combined.contains(","));
    }

    #[test]
    fn css_media_query_with_many_rules() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ '@media screen and (min-width: 640px)': { color: 'red', backgroundColor: 'blue', padding: 12, marginTop: 4, borderColor: 'black', borderRadius: 2 } });\n",
        );
        assert!(emitted.contains("const styles = null"));
        let media_rules: Vec<&String> = artifacts
            .style_rules
            .iter()
            .filter(|rule| rule.contains("@media"))
            .collect();
        assert_eq!(media_rules.len(), 6);
        assert!(media_rules.iter().any(|rule| rule.contains("color:red")));
        assert!(media_rules
            .iter()
            .any(|rule| rule.contains("background-color:blue")));
        assert!(media_rules.iter().any(|rule| rule.contains("padding:12px")));
        assert!(media_rules
            .iter()
            .any(|rule| rule.contains("margin-top:4px")));
        assert!(media_rules
            .iter()
            .any(|rule| rule.contains("border-color:black")));
        assert!(media_rules
            .iter()
            .any(|rule| rule.contains("border-radius:2px")));
    }

    #[test]
    fn css_emits_property_rule() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ '@property --radius': { syntax: '\"<length>\"', inherits: false, initialValue: '0px' } });\n",
        );
        assert!(emitted.contains("const styles = null"));
        assert_eq!(artifacts.style_rules.len(), 1);
        assert!(artifacts.style_rules.iter().any(|rule| rule
            .contains("@property --radius{syntax:\"<length>\";inherits:false;initial-value:0px}")));
    }

    #[test]
    fn css_call_with_multiple_arguments() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css({ color: 'red' }, { backgroundColor: 'blue' });\n",
        );
        assert!(emitted.contains("const styles = null"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("background-color:blue")));
    }

    #[test]
    fn css_call_with_array_argument() {
        let (emitted, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst styles = css([{ color: 'red' }, { backgroundColor: 'blue' }]);\n",
        );
        assert!(emitted.contains("const styles = null"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("background-color:blue")));
    }

    #[test]
    fn xcss_inline_object_transforms() {
        let (emitted, artifacts) = transform_source(
            "const Component = () => <div xcss={{ color: 'red', backgroundColor: 'blue' }} />;\n",
        );
        assert!(emitted.contains("_syaz5scu _bfhk13q2"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("background-color:blue")));
    }

    #[test]
    #[should_panic(expected = "Object given to the xcss prop must be static")]
    fn xcss_inline_object_must_be_static() {
        let _ = transform_source(
            "const render = (value: string) => <div xcss={{ color: value }} />;\n",
        );
    }

    #[test]
    fn xcss_css_map_reference_preserved() {
        let (emitted, artifacts) = transform_source(
            "import { cssMap } from '@compiled/react';\nconst styles = cssMap({ primary: { color: 'red' } });\nconst Component = () => <div xcss={styles.primary} />;\n",
        );
        assert!(emitted.contains("xcss={styles.primary}"));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn class_name_compression_map_switches_to_ac() {
        let mut options = PluginOptions::default();
        options
            .class_name_compression_map
            .insert("1wyb1fwx".into(), "a".into());
        let (emitted, artifacts) = transform_source_with_options(
            "import { ClassNames } from '@compiled/react';\nconst Component = () => (\n  <ClassNames>{({ css }) => <div className={css({ fontSize: 12 })} />}</ClassNames>\n);\n",
            options,
        );
        assert!(emitted.contains("ac(["));
        assert!(emitted.contains("import { ac, ix } from \"@compiled/react/runtime\";"));
        assert!(artifacts.style_rules.iter().any(|rule| rule.contains(".a")));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("font-size:12px")));
    }

    #[test]
    fn style_sheet_path_inserts_requires() {
        let mut options = PluginOptions::default();
        options.style_sheet_path = Some("./__compiled.css".into());
        let (emitted, artifacts) = transform_source_with_options(
            "import { css } from '@compiled/react';\nconst styles = css({ color: 'red' });\n",
            options,
        );
        assert!(emitted.contains("require(\"./__compiled.css?style="));
        assert!(artifacts
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn metadata_contains_style_rules() {
        let (_, artifacts) = transform_source(
            "import { css } from '@compiled/react';\nconst className = css`color: red;`;\n",
        );
        let Some(Value::Array(rules)) = artifacts.metadata.get("styleRules") else {
            panic!("expected styleRules array in metadata");
        };
        assert!(!rules.is_empty());
    }

    #[test]
    fn extract_styles_to_directory_writes_file() {
        let temp_root = std::env::temp_dir().join(format!(
            "compiled_swc_extract_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let source_dir = temp_root.join("src");
        fs::create_dir_all(&source_dir).expect("create source dir");
        let filename = source_dir.join("component.tsx");
        let dest_dir = temp_root.join("out");

        let mut options = PluginOptions::default();
        options.extract_styles_to_directory = Some(ExtractStylesToDirectoryOptions {
            source: source_dir.to_string_lossy().to_string(),
            dest: dest_dir.to_string_lossy().to_string(),
        });

        let mut metadata = TransformPluginProgramMetadata::default();
        metadata.transform_plugin_config =
            Some(serde_json::to_string(&options).expect("serialize options"));
        metadata.context = Some(TransformPluginMetadataContext {
            filename: Some(filename.to_string_lossy().to_string()),
            cwd: Some(temp_root.to_string_lossy().to_string()),
            ..Default::default()
        });

        let program = parse(
            "import { css } from '@compiled/react';\nconst styles = css({ color: 'red' });\n",
        );
        let transformed = transform_program(program, metadata);
        let emitted = program_to_source(&transformed).expect("emit program");
        assert!(emitted.contains("import \"./component.compiled.css\""));

        let css_path = dest_dir.join("component.compiled.css");
        let stylesheet = fs::read_to_string(&css_path).expect("read stylesheet");
        assert!(stylesheet.contains("color:red"));

        let _ = fs::remove_dir_all(&temp_root);
    }
}
