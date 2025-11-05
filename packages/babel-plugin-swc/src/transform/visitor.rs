use std::borrow::Cow;
use std::collections::HashSet;

use swc_core::common::{sync::Lrc, SourceMap, DUMMY_SP};
use swc_core::ecma::ast::{
    ArrayLit, BinaryOp, Callee, CondExpr, Decl, EsVersion, Expr, ExprOrSpread, ExprStmt,
    ImportDecl, ImportSpecifier, Lit, MemberExpr, MemberProp, Module, ModuleDecl, ModuleExportName,
    ModuleItem, ObjectLit, Pat, Prop, PropOrSpread, Stmt, Str, Tpl, VarDecl, VarDeclKind,
};
use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};
use swc_core::ecma::visit::{noop_fold_type, Fold, FoldWith};

use crate::{
    constants::DEFAULT_IMPORT_SOURCES,
    css::property::{
        array_to_css_string, collapse_whitespace, normalise_at_rule_key, normalise_property_pair,
        serialize_css_object, trim_number, CssObject, CssValue,
    },
    state::{ImportBinding, ImportKind, TransformState},
    utils::{
        hash::murmur2_hash,
        wtf8::{wtf8_to_cow, wtf8_to_string},
    },
};

pub struct RootFolder<'a> {
    state: &'a mut TransformState,
}

impl<'a> RootFolder<'a> {
    pub fn new(state: &'a mut TransformState) -> Self {
        Self { state }
    }
}

#[derive(Clone, Default)]
struct CssContext {
    at_rules: Vec<String>,
    selectors: Vec<String>,
}

impl CssContext {
    fn root() -> Self {
        Self {
            at_rules: Vec::new(),
            selectors: vec!["&".to_string()],
        }
    }

    fn with_selector(&self, selector: &str) -> Self {
        let mut combined = Vec::new();
        let parts = normalise_selector_parts(selector);

        for parent in &self.selectors {
            for part in &parts {
                let replaced = part.replace('&', parent);
                combined.push(replaced);
            }
        }

        Self {
            at_rules: self.at_rules.clone(),
            selectors: dedupe_preserving_order(combined),
        }
    }

    fn with_at_rule(&self, at_rule: &str) -> Self {
        let mut at_rules = self.at_rules.clone();
        at_rules.push(normalise_at_rule_key(at_rule));

        Self {
            at_rules,
            selectors: self.selectors.clone(),
        }
    }

    fn wrap_rule(&self, mut rule: String) -> String {
        for at_rule in self.at_rules.iter().rev() {
            rule = format!("{at_rule}{{{rule}}}");
        }

        rule
    }

    fn selectors_for_hash(&self) -> String {
        self.selectors.join("")
    }

    fn at_rules_for_hash(&self) -> String {
        self.at_rules.join("")
    }
}

impl<'a> Fold for RootFolder<'a> {
    noop_fold_type!();

    fn fold_module(&mut self, module: Module) -> Module {
        self.collect_imports(&module);
        self.collect_local_bindings(&module);

        if self.state.config.extract {
            // When extract mode is enabled we currently return the module
            // untouched. A future revision will inline the strip-runtime
            // semantics directly in this visitor.
        }

        module.fold_children_with(self)
    }

    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Call(call) => self.fold_call_expr(call),
            other => other.fold_children_with(self),
        }
    }
}

impl<'a> RootFolder<'a> {
    fn collect_imports(&mut self, module: &Module) {
        for item in &module.body {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = item {
                let import_source = wtf8_to_string(&import_decl.src.value);
                self.register_import_bindings(import_decl, &import_source);

                if DEFAULT_IMPORT_SOURCES
                    .iter()
                    .any(|source| import_source.as_str() == *source)
                {
                    self.register_css_imports(import_decl);
                }
            }
        }
    }

    fn register_css_imports(&mut self, import_decl: &ImportDecl) {
        for specifier in &import_decl.specifiers {
            match specifier {
                ImportSpecifier::Named(named) => {
                    let imported: Cow<'_, str> = match &named.imported {
                        Some(ModuleExportName::Ident(ident)) => Cow::Borrowed(ident.sym.as_ref()),
                        Some(ModuleExportName::Str(str)) => wtf8_to_cow(&str.value),
                        None => Cow::Borrowed(named.local.sym.as_ref()),
                    };

                    match imported.as_ref() {
                        "css" => self.state.register_css_ident(named.local.sym.to_string()),
                        "styled" => self
                            .state
                            .register_styled_ident(named.local.sym.to_string()),
                        "keyframes" => self
                            .state
                            .register_keyframes_ident(named.local.sym.to_string()),
                        "ClassNames" => self
                            .state
                            .register_class_names_ident(named.local.sym.to_string()),
                        "cssMap" => self
                            .state
                            .register_css_map_ident(named.local.sym.to_string()),
                        _ => {}
                    }
                }
                ImportSpecifier::Default(default_spec) => {
                    // Styled components can be imported as the default export in user code.
                    self.state
                        .register_styled_ident(default_spec.local.sym.to_string());
                }
                ImportSpecifier::Namespace(_) => {}
            }
        }
    }

    fn register_import_bindings(&mut self, import_decl: &ImportDecl, source: &str) {
        for specifier in &import_decl.specifiers {
            match specifier {
                ImportSpecifier::Named(named) => {
                    let imported = match &named.imported {
                        Some(ModuleExportName::Ident(ident)) => ident.sym.to_string(),
                        Some(ModuleExportName::Str(str)) => wtf8_to_string(&str.value),
                        None => named.local.sym.to_string(),
                    };

                    self.state.register_import_binding(
                        named.local.sym.to_string(),
                        ImportBinding {
                            source: source.to_string(),
                            kind: ImportKind::Named(imported),
                        },
                    );
                }
                ImportSpecifier::Default(default_spec) => {
                    self.state.register_import_binding(
                        default_spec.local.sym.to_string(),
                        ImportBinding {
                            source: source.to_string(),
                            kind: ImportKind::Default,
                        },
                    );
                }
                ImportSpecifier::Namespace(_) => {}
            }
        }
    }

    fn collect_local_bindings(&mut self, module: &Module) {
        for item in &module.body {
            match item {
                ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => self.record_var_decl(var),
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) => {
                    if let Decl::Var(var) = &export_decl.decl {
                        self.record_var_decl(var);
                    }
                }
                _ => {}
            }
        }
    }

    fn record_var_decl(&mut self, var: &VarDecl) {
        if var.kind != VarDeclKind::Const {
            return;
        }

        for decl in &var.decls {
            if let Pat::Ident(ident) = &decl.name {
                if let Some(init) = &decl.init {
                    if let Some(value) = self.evaluate_expr(init) {
                        self.state
                            .register_local_binding(ident.id.sym.to_string(), value);
                    }
                }
            }
        }
    }

    fn evaluate_expr(&mut self, expr: &Expr) -> Option<CssValue> {
        match expr {
            Expr::Paren(paren) => self.evaluate_expr(&paren.expr),
            Expr::Lit(Lit::Str(str)) => Some(CssValue::String(wtf8_to_string(&str.value))),
            Expr::Lit(Lit::Num(num)) => Some(CssValue::Number(num.value)),
            Expr::Lit(Lit::Bool(bool)) => Some(CssValue::Bool(bool.value)),
            Expr::Lit(Lit::Null(_)) => Some(CssValue::Null),
            Expr::Tpl(tpl) => self.evaluate_template_literal(tpl),
            Expr::TsAs(ts_as) => self.evaluate_expr(&ts_as.expr),
            Expr::TsConstAssertion(assert) => self.evaluate_expr(&assert.expr),
            Expr::TsNonNull(non_null) => self.evaluate_expr(&non_null.expr),
            Expr::TsTypeAssertion(assert) => self.evaluate_expr(&assert.expr),
            Expr::Object(object) => self.evaluate_object_literal(object),
            Expr::Array(array) => self.evaluate_array_literal(array),
            Expr::Member(member) => self.evaluate_member_expr(member),
            Expr::Call(call) => self.evaluate_call_expr(call),
            Expr::Cond(cond) => self.evaluate_conditional_expr(cond),
            Expr::Ident(ident) => self
                .state
                .lookup_local_binding(ident.sym.as_ref())
                .cloned()
                .or_else(|| self.state.resolve_identifier_value(ident.sym.as_ref())),
            Expr::Unary(unary) => {
                let value = self.evaluate_expr(&unary.arg)?;
                evaluate_unary_expression(unary.op, value)
            }
            Expr::Bin(bin) => {
                let left = self.evaluate_expr(&bin.left)?;
                let right = self.evaluate_expr(&bin.right)?;
                evaluate_binary_expression(bin.op, left, right)
            }
            _ => None,
        }
    }

    fn evaluate_object_literal(&mut self, object: &ObjectLit) -> Option<CssValue> {
        let mut map: CssObject = CssObject::new();

        for prop in &object.props {
            match prop {
                PropOrSpread::Prop(prop) => match &**prop {
                    Prop::KeyValue(kv) => {
                        let key = self.object_key_to_string(&kv.key)?;
                        let value = self.evaluate_expr(&kv.value)?;
                        map.insert(key, value);
                    }
                    Prop::Shorthand(ident) => {
                        let key = ident.sym.to_string();
                        let expr = Expr::Ident(ident.clone());
                        let value = self.evaluate_expr(&expr)?;
                        map.insert(key, value);
                    }
                    Prop::Assign(assign) => {
                        let key = assign.key.sym.to_string();
                        let value = self.evaluate_expr(&assign.value)?;
                        map.insert(key, value);
                    }
                    Prop::Getter(_) | Prop::Setter(_) | Prop::Method(_) => return None,
                },
                PropOrSpread::Spread(spread) => {
                    let value = self.evaluate_expr(&spread.expr)?;
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

    fn evaluate_array_literal(&mut self, array: &ArrayLit) -> Option<CssValue> {
        let mut values = Vec::new();

        for elem in &array.elems {
            match elem {
                Some(ExprOrSpread { spread: None, expr }) => {
                    values.push(self.evaluate_expr(expr)?);
                }
                Some(ExprOrSpread {
                    spread: Some(_),
                    expr,
                }) => {
                    let spread_value = self.evaluate_expr(expr)?;
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

    fn evaluate_member_expr(&mut self, member: &MemberExpr) -> Option<CssValue> {
        let object_value = self.evaluate_expr(&member.obj)?;
        let property_name = self.member_prop_to_string(&member.prop)?;

        match object_value {
            CssValue::Object(map) => map.get(&property_name).cloned(),
            _ => None,
        }
    }

    fn evaluate_call_expr(&mut self, call: &swc_core::ecma::ast::CallExpr) -> Option<CssValue> {
        if let swc_core::ecma::ast::Callee::Expr(callee) = &call.callee {
            if let Expr::Member(member) = &**callee {
                if is_string_concat(member) {
                    return self.evaluate_concat_call(&member.obj, &call.args);
                }
            }
        }

        None
    }

    fn evaluate_conditional_expr(&mut self, cond: &CondExpr) -> Option<CssValue> {
        let test = self.evaluate_expr(&cond.test)?;

        if test.is_truthy() {
            self.evaluate_expr(&cond.cons)
        } else {
            self.evaluate_expr(&cond.alt)
        }
    }

    fn evaluate_concat_call(&mut self, object: &Expr, args: &[ExprOrSpread]) -> Option<CssValue> {
        let mut result = self.evaluate_expr(object)?.into_css_string()?;

        for arg in args {
            if arg.spread.is_some() {
                return None;
            }

            let value = self.evaluate_expr(&arg.expr)?;
            result.push_str(&value.into_css_string()?);
        }

        Some(CssValue::String(result))
    }

    fn evaluate_template_literal(&mut self, tpl: &Tpl) -> Option<CssValue> {
        let mut result = String::new();

        for (index, quasi) in tpl.quasis.iter().enumerate() {
            result.push_str(quasi.raw.as_ref());

            if let Some(expr) = tpl.exprs.get(index) {
                let value = self.evaluate_expr(expr)?;
                let segment = value.into_template_segment()?;
                result.push_str(&segment);
            }
        }

        Some(CssValue::String(result))
    }

    fn object_key_to_string(&mut self, key: &swc_core::ecma::ast::PropName) -> Option<String> {
        match key {
            swc_core::ecma::ast::PropName::Ident(ident) => Some(ident.sym.to_string()),
            swc_core::ecma::ast::PropName::Str(str) => Some(wtf8_to_string(&str.value)),
            swc_core::ecma::ast::PropName::Num(num) => Some(num.value.to_string()),
            swc_core::ecma::ast::PropName::Computed(computed) => {
                let value = self.evaluate_expr(&computed.expr)?;
                match value {
                    CssValue::String(text) => Some(text),
                    CssValue::Number(num) => Some(trim_number(num)),
                    _ => None,
                }
            }
            swc_core::ecma::ast::PropName::BigInt(_) => None,
        }
    }

    fn member_prop_to_string(&mut self, prop: &MemberProp) -> Option<String> {
        match prop {
            MemberProp::Ident(ident) => Some(ident.sym.to_string()),
            MemberProp::PrivateName(_) => None,
            MemberProp::Computed(expr) => {
                let value = self.evaluate_expr(&expr.expr)?;
                match value {
                    CssValue::String(text) => Some(text),
                    CssValue::Number(num) => Some(trim_number(num)),
                    _ => None,
                }
            }
        }
    }

    fn fold_call_expr(&mut self, call: swc_core::ecma::ast::CallExpr) -> Expr {
        let call = call.fold_children_with(self);

        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Ident(ident) = &**callee {
                let name = ident.sym.to_string();

                if self.state.is_keyframes_ident(&name) {
                    if let Some(expr) = self.process_keyframes_call(&call) {
                        return expr;
                    }
                }

                if self.state.is_css_ident(&name) {
                    if let Some(expr) = self.process_css_call(&call) {
                        return expr;
                    }
                }
            }
        }

        Expr::Call(call)
    }

    fn process_keyframes_call(&mut self, call: &swc_core::ecma::ast::CallExpr) -> Option<Expr> {
        let arg = call.args.first()?;
        let value = self.evaluate_expr(&arg.expr)?;
        let map = match value {
            CssValue::Object(map) => map,
            _ => return None,
        };

        let body = serialize_css_object(&map)?;
        let name = self.generate_keyframes_name(call);
        let rule = format!("@keyframes {name}{{{body}}}");
        self.state.push_style_rule(rule);

        Some(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: name.into(),
            raw: None,
        })))
    }

    fn generate_keyframes_name(&self, call: &swc_core::ecma::ast::CallExpr) -> String {
        let expr = Expr::Call(call.clone());
        let code = emit_expression(&expr);
        let hash = murmur2_hash(&code, 0);
        format!("k{hash}")
    }

    fn process_css_call(&mut self, call: &swc_core::ecma::ast::CallExpr) -> Option<Expr> {
        if call.args.is_empty() {
            return None;
        }

        let context = CssContext::root();
        let mut class_names = Vec::new();

        for arg in &call.args {
            if arg.spread.is_some() {
                return None;
            }

            let value = self.resolve_css_value(&arg.expr)?;
            let entries = self.process_css_root_value(value, &context)?;
            class_names.extend(entries);
        }

        if class_names.is_empty() {
            return None;
        }

        let joined = class_names.join(" ");

        Some(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: joined.into(),
            raw: None,
        })))
    }

    fn process_css_root_value(
        &mut self,
        value: CssValue,
        context: &CssContext,
    ) -> Option<Vec<String>> {
        match value {
            CssValue::Object(map) => Some(self.process_css_object_map(&map, context)),
            CssValue::Array(values) => {
                let mut class_names = Vec::new();

                for value in values {
                    class_names.extend(self.process_css_root_value(value, context)?);
                }

                Some(class_names)
            }
            CssValue::String(text) => {
                if text.trim().is_empty() {
                    Some(Vec::new())
                } else {
                    Some(vec![text])
                }
            }
            CssValue::Number(num) => Some(vec![trim_number(num)]),
            CssValue::Bool(_) | CssValue::Null => Some(Vec::new()),
        }
    }

    fn process_css_entry(
        &mut self,
        key: &str,
        value: CssValue,
        context: &CssContext,
    ) -> Vec<String> {
        if key.starts_with('@') {
            return self.process_at_rule(key, value, context);
        }

        match value {
            CssValue::Object(map) => {
                if key == "selectors" {
                    self.process_selectors_object(&map, context)
                } else {
                    let nested_context = context.with_selector(key);
                    self.process_css_object_map(&map, &nested_context)
                }
            }
            CssValue::Array(values) => self.process_css_array(key, &values, context),
            other => self.process_atomic_property(key, other, context),
        }
    }

    fn process_css_object_map(&mut self, map: &CssObject, context: &CssContext) -> Vec<String> {
        let mut class_names = Vec::new();

        for (key, value) in map.iter() {
            class_names.extend(self.process_css_entry(key, value.clone(), context));
        }

        class_names
    }

    fn process_selectors_object(&mut self, map: &CssObject, context: &CssContext) -> Vec<String> {
        let mut class_names = Vec::new();

        for (selector, value) in map.iter() {
            let nested_context = context.with_selector(selector);
            class_names.extend(match value.clone() {
                CssValue::Object(object) => self.process_css_object_map(&object, &nested_context),
                CssValue::Array(values) => {
                    self.process_css_array(selector, &values, &nested_context)
                }
                other => self.process_atomic_property(selector, other, &nested_context),
            });
        }

        class_names
    }

    fn process_css_array(
        &mut self,
        key: &str,
        values: &[CssValue],
        context: &CssContext,
    ) -> Vec<String> {
        let mut class_names = Vec::new();
        let mut pending_values: Vec<CssValue> = Vec::new();

        fn flush_pending(
            state: &mut RootFolder<'_>,
            class_names: &mut Vec<String>,
            pending_values: &mut Vec<CssValue>,
            key: &str,
            context: &CssContext,
        ) {
            if pending_values.is_empty() {
                return;
            }

            if let Some(joined) = array_to_css_string(pending_values) {
                class_names.extend(state.process_atomic_property(
                    key,
                    CssValue::String(joined),
                    context,
                ));
            }

            pending_values.clear();
        }

        for value in values {
            match value {
                CssValue::Object(_) => {
                    flush_pending(self, &mut class_names, &mut pending_values, key, context);
                    class_names.extend(self.process_css_entry(key, value.clone(), context));
                }
                CssValue::Array(nested) => {
                    flush_pending(self, &mut class_names, &mut pending_values, key, context);
                    class_names.extend(self.process_css_array(key, nested, context));
                }
                CssValue::Null => {}
                other => pending_values.push(other.clone()),
            }
        }

        flush_pending(self, &mut class_names, &mut pending_values, key, context);

        class_names
    }

    fn process_at_rule(&mut self, key: &str, value: CssValue, context: &CssContext) -> Vec<String> {
        match value {
            CssValue::Array(items) => {
                let mut class_names = Vec::new();
                for item in items {
                    class_names.extend(self.process_at_rule(key, item, context));
                }
                class_names
            }
            CssValue::Object(map) => {
                let at_rule_name = extract_at_rule_name(key);
                match classify_at_rule(at_rule_name) {
                    AtRuleKind::Atomic => {
                        let nested_context = context.with_at_rule(key);
                        self.process_css_object_map(&map, &nested_context)
                    }
                    AtRuleKind::Ignored => {
                        if let Some(body) = serialize_css_object(&map) {
                            let mut rule = format!("{}{{{body}}}", normalise_at_rule_key(key));
                            rule = context.wrap_rule(rule);
                            self.state.push_style_rule(rule);
                        }
                        Vec::new()
                    }
                    AtRuleKind::Forbidden | AtRuleKind::Unknown => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn process_atomic_property(
        &mut self,
        key: &str,
        value: CssValue,
        context: &CssContext,
    ) -> Vec<String> {
        let Some((property_name, value)) = normalise_property_pair(key, value) else {
            return Vec::new();
        };

        let class_name = self.build_atomic_class(&property_name, &value, context);
        let selector_rule = build_selector_rule(context, &class_name, &property_name, &value);
        let rule = context.wrap_rule(selector_rule);
        self.state.push_style_rule(rule);

        vec![class_name]
    }

    fn resolve_css_value(&mut self, expr: &Expr) -> Option<CssValue> {
        self.evaluate_expr(expr)
    }

    fn build_atomic_class(&self, prop: &str, value: &str, context: &CssContext) -> String {
        let prefix = self.state.config.class_hash_prefix.as_deref().unwrap_or("");
        let at_rules_key = if context.at_rules.is_empty() {
            "undefined".to_string()
        } else {
            context.at_rules_for_hash()
        };
        let selectors_key = if context.selectors.is_empty() {
            String::new()
        } else {
            context.selectors_for_hash()
        };
        let group_input = format!("{prefix}{at_rules_key}{selectors_key}{prop}");
        let group_hash = truncate_hash(&murmur2_hash(&group_input, 0));
        let value_hash = truncate_hash(&murmur2_hash(value, 0));
        format!("_{}{}", group_hash, value_hash)
    }
}

fn build_selector_rule(
    context: &CssContext,
    class_name: &str,
    property: &str,
    value: &str,
) -> String {
    let selectors: Vec<String> = context
        .selectors
        .iter()
        .map(|selector| selector.replace('&', &format!(".{class_name}")))
        .collect();
    let selector = selectors.join(",");
    format!("{selector}{{{property}:{value}}}")
}

fn normalise_selector_parts(selector: &str) -> Vec<String> {
    selector
        .split(',')
        .map(|part| {
            let collapsed = collapse_whitespace(part);
            if collapsed.is_empty() {
                "&".to_string()
            } else if collapsed.contains('&') {
                collapsed
            } else {
                format!("& {collapsed}")
            }
        })
        .collect()
}

fn dedupe_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for value in values {
        if seen.insert(value.clone()) {
            result.push(value);
        }
    }

    result
}

fn extract_at_rule_name(key: &str) -> &str {
    let trimmed = key.trim_start_matches('@');
    trimmed.split_whitespace().next().unwrap_or("")
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AtRuleKind {
    Atomic,
    Ignored,
    Forbidden,
    Unknown,
}

fn classify_at_rule(name: &str) -> AtRuleKind {
    match name {
        "container" | "-moz-document" | "else" | "layer" | "media" | "starting-style"
        | "supports" | "when" => AtRuleKind::Atomic,
        "charset" | "import" | "namespace" => AtRuleKind::Forbidden,
        "color-profile"
        | "counter-style"
        | "font-face"
        | "font-palette-values"
        | "keyframes"
        | "page"
        | "property" => AtRuleKind::Ignored,
        _ => AtRuleKind::Unknown,
    }
}

fn is_string_concat(member: &MemberExpr) -> bool {
    matches!(member.prop, MemberProp::Ident(ref ident) if ident.sym.as_ref() == "concat")
}

fn evaluate_unary_expression(
    op: swc_core::ecma::ast::UnaryOp,
    value: CssValue,
) -> Option<CssValue> {
    match op {
        swc_core::ecma::ast::UnaryOp::Plus => value.into_number().map(CssValue::Number),
        swc_core::ecma::ast::UnaryOp::Minus => {
            value.into_number().map(|num| CssValue::Number(-num))
        }
        swc_core::ecma::ast::UnaryOp::Bang => value.into_bool().map(|b| CssValue::Bool(!b)),
        _ => None,
    }
}

fn evaluate_binary_expression(op: BinaryOp, left: CssValue, right: CssValue) -> Option<CssValue> {
    match op {
        BinaryOp::Add => {
            if matches!(left, CssValue::String(_)) || matches!(right, CssValue::String(_)) {
                let left_string = left.clone().into_css_string()?;
                let right_string = right.clone().into_css_string()?;
                return Some(CssValue::String(format!("{left_string}{right_string}")));
            }

            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
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
        BinaryOp::Gt => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Bool(left_number > right_number))
        }
        BinaryOp::Lt => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Bool(left_number < right_number))
        }
        BinaryOp::GtEq => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Bool(left_number >= right_number))
        }
        BinaryOp::LtEq => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Bool(left_number <= right_number))
        }
        BinaryOp::EqEqEq | BinaryOp::EqEq => Some(CssValue::Bool(left == right)),
        BinaryOp::NotEqEq | BinaryOp::NotEq => Some(CssValue::Bool(left != right)),
        BinaryOp::Sub => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Number(left_number - right_number))
        }
        BinaryOp::Mul => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Number(left_number * right_number))
        }
        BinaryOp::Div => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Number(left_number / right_number))
        }
        BinaryOp::Mod => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Number(left_number % right_number))
        }
        BinaryOp::Exp => {
            let left_number = left.to_number()?;
            let right_number = right.to_number()?;
            Some(CssValue::Number(left_number.powf(right_number)))
        }
        BinaryOp::BitAnd => {
            let left_number = left.to_number()? as i64;
            let right_number = right.to_number()? as i64;
            Some(CssValue::Number((left_number & right_number) as f64))
        }
        BinaryOp::BitOr => {
            let left_number = left.to_number()? as i64;
            let right_number = right.to_number()? as i64;
            Some(CssValue::Number((left_number | right_number) as f64))
        }
        BinaryOp::BitXor => {
            let left_number = left.to_number()? as i64;
            let right_number = right.to_number()? as i64;
            Some(CssValue::Number((left_number ^ right_number) as f64))
        }
        BinaryOp::LShift => {
            let left_number = left.to_number()? as i64;
            let right_number = right.to_number()? as u32;
            Some(CssValue::Number((left_number << right_number) as f64))
        }
        BinaryOp::RShift => {
            let left_number = left.to_number()? as i64;
            let right_number = right.to_number()? as u32;
            Some(CssValue::Number((left_number >> right_number) as f64))
        }
        BinaryOp::ZeroFillRShift => {
            let left_number = left.to_number()? as u64;
            let right_number = right.to_number()? as u32;
            Some(CssValue::Number((left_number >> right_number) as f64))
        }
        _ => None,
    }
}

fn truncate_hash(value: &str) -> String {
    value.chars().take(4).collect()
}

fn emit_expression(expr: &Expr) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buf = Vec::new();

    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: CodegenConfig::default().with_target(EsVersion::Es2022),
            comments: None,
            cm,
            wr: writer,
        };

        let module = Module {
            span: DUMMY_SP,
            body: vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(expr.clone()),
            }))],
            shebang: None,
        };

        emitter
            .emit_module(&module)
            .expect("emit expression module");
    }

    let code = String::from_utf8(buf).expect("utf8 expression");
    let trimmed = code.trim();
    let without_semicolon = trimmed.strip_suffix(';').unwrap_or(trimmed);
    without_semicolon.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{evaluate_binary_expression, BinaryOp, RootFolder};
    use crate::{
        options::PluginConfig, state::TransformState, transform::Transform,
        types::TransformMetadata,
    };
    use std::fs;
    use std::path::Path;
    use swc_core::common::sync::Lrc;
    use swc_core::common::{FileName, SourceMap};
    use swc_core::ecma::ast::{EsVersion, Program};
    use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};
    use swc_core::ecma::parser::{parse_file_as_module, Syntax, TsSyntax};
    use swc_core::ecma::visit::Fold;

    use crate::css::property::CssValue;

    #[test]
    fn evaluates_numeric_arithmetic() {
        assert_eq!(
            evaluate_binary_expression(BinaryOp::Add, CssValue::Number(2.0), CssValue::Number(3.0),),
            Some(CssValue::Number(5.0))
        );

        assert_eq!(
            evaluate_binary_expression(BinaryOp::Sub, CssValue::Number(5.0), CssValue::Number(3.0),),
            Some(CssValue::Number(2.0))
        );

        assert_eq!(
            evaluate_binary_expression(BinaryOp::Mul, CssValue::Number(4.0), CssValue::Number(2.5),),
            Some(CssValue::Number(10.0))
        );

        assert_eq!(
            evaluate_binary_expression(BinaryOp::Div, CssValue::Number(9.0), CssValue::Number(3.0),),
            Some(CssValue::Number(3.0))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::Mod,
                CssValue::Number(10.0),
                CssValue::Number(4.0),
            ),
            Some(CssValue::Number(2.0))
        );

        assert_eq!(
            evaluate_binary_expression(BinaryOp::Exp, CssValue::Number(2.0), CssValue::Number(3.0),),
            Some(CssValue::Number(8.0))
        );
    }

    #[test]
    fn evaluates_comparisons_and_equality() {
        assert_eq!(
            evaluate_binary_expression(BinaryOp::Gt, CssValue::Number(5.0), CssValue::Number(3.0),),
            Some(CssValue::Bool(true))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::LtEq,
                CssValue::Number(5.0),
                CssValue::Number(5.0),
            ),
            Some(CssValue::Bool(true))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::EqEqEq,
                CssValue::String("abc".into()),
                CssValue::String("abc".into()),
            ),
            Some(CssValue::Bool(true))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::NotEqEq,
                CssValue::Bool(true),
                CssValue::Bool(false),
            ),
            Some(CssValue::Bool(true))
        );
    }

    #[test]
    fn evaluates_bitwise_operations() {
        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::BitAnd,
                CssValue::Number(6.0),
                CssValue::Number(3.0),
            ),
            Some(CssValue::Number(2.0))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::BitOr,
                CssValue::Number(4.0),
                CssValue::Number(1.0),
            ),
            Some(CssValue::Number(5.0))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::BitXor,
                CssValue::Number(5.0),
                CssValue::Number(3.0),
            ),
            Some(CssValue::Number(6.0))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::LShift,
                CssValue::Number(2.0),
                CssValue::Number(3.0),
            ),
            Some(CssValue::Number(16.0))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::RShift,
                CssValue::Number(16.0),
                CssValue::Number(2.0),
            ),
            Some(CssValue::Number(4.0))
        );

        assert_eq!(
            evaluate_binary_expression(
                BinaryOp::ZeroFillRShift,
                CssValue::Number(16.0),
                CssValue::Number(1.0),
            ),
            Some(CssValue::Number(8.0))
        );
    }

    fn parse_module(path: &Path, source: &str) -> swc_core::ecma::ast::Module {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), source.to_owned());
        let mut errors = Vec::new();

        parse_file_as_module(
            &fm,
            Syntax::Typescript(TsSyntax {
                tsx: true,
                ..Default::default()
            }),
            EsVersion::Es2022,
            None,
            &mut errors,
        )
        .expect("failed to parse module")
    }

    fn emit_program(program: &Program) -> String {
        let cm: Lrc<SourceMap> = Default::default();
        let mut buf = Vec::new();

        {
            let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
            let mut emitter = Emitter {
                cfg: CodegenConfig::default().with_target(EsVersion::Es2022),
                comments: None,
                cm,
                wr: writer,
            };

            emitter.emit_program(program).expect("emit program");
        }

        String::from_utf8(buf).expect("utf8 program")
    }

    #[test]
    fn resolves_imported_values_in_css_objects() {
        let dir = tempfile::tempdir().unwrap();
        let tokens_path = dir.path().join("tokens.ts");
        fs::write(
            &tokens_path,
            "export const tokens = { palette: { primary: 'red' } } as const;",
        )
        .unwrap();

        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';
            import { tokens } from './tokens';

            export const className = css({ color: tokens.palette.primary });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);

        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert_eq!(result.style_rules.len(), 1);
        assert!(result.style_rules[0].contains("color:red"));
    }

    #[test]
    fn resolves_imported_concat_values() {
        let dir = tempfile::tempdir().unwrap();
        let tokens_path = dir.path().join("tokens.ts");
        fs::write(
            &tokens_path,
            "export const tokens = { spacing: '8px '.concat('var(--gap)') } as const;",
        )
        .unwrap();

        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';
            import { tokens } from './tokens';

            export const className = css({ margin: tokens.spacing });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);
        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert_eq!(result.style_rules.len(), 1);
        assert!(result.style_rules[0].contains("margin:8px var(--gap)"));
    }

    #[test]
    fn evaluates_template_literals_with_static_bindings() {
        let dir = tempfile::tempdir().unwrap();
        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';

            const base = 'blue';
            export const className = css({ color: `${base}` });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);
        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert_eq!(result.style_rules.len(), 1);
        assert!(result.style_rules[0].contains("color:blue"));
    }

    #[test]
    fn evaluates_string_concat_calls() {
        let dir = tempfile::tempdir().unwrap();
        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';

            export const className = css({ margin: "8px ".concat('16px') });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);
        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert_eq!(result.style_rules.len(), 1);
        assert!(result.style_rules[0].contains("margin:8px 16px"));
    }

    #[test]
    fn evaluates_unary_number_expressions() {
        let dir = tempfile::tempdir().unwrap();
        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';

            export const className = css({ marginTop: -16 });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);
        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert_eq!(result.style_rules.len(), 1);
        assert!(result.style_rules[0].contains("margin-top:-16px"));
    }

    #[test]
    fn evaluates_logical_operations() {
        let dir = tempfile::tempdir().unwrap();
        let tokens_path = dir.path().join("tokens.ts");
        fs::write(
            &tokens_path,
            "export const tokens = { maybe: 'green', fallback: 'blue', nullish: null } as const;",
        )
        .unwrap();

        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';
            import { tokens } from './tokens';

            export const className = css({
              color: tokens.maybe || tokens.fallback,
              backgroundColor: tokens.nullish ?? 'white',
              borderColor: tokens.maybe && 'black',
            });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);
        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert!(result
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:green")));
        assert!(result
            .style_rules
            .iter()
            .any(|rule| rule.contains("background-color:white")));
        assert!(result
            .style_rules
            .iter()
            .any(|rule| rule.contains("border-color:black")));
    }

    #[test]
    fn evaluates_conditional_expressions() {
        let dir = tempfile::tempdir().unwrap();
        let tokens_path = dir.path().join("tokens.ts");
        fs::write(
            &tokens_path,
            "export const primary = true; export const fallback = 'blue'; ",
        )
        .unwrap();

        let entry_path = dir.path().join("entry.tsx");
        let entry_source = r#"
            import { css } from '@compiled/react';
            import { primary, fallback } from './tokens';

            export const className = css({
              color: primary ? 'red' : fallback,
            });
        "#;
        fs::write(&entry_path, entry_source).unwrap();

        let module = parse_module(&entry_path, entry_source);
        let mut config = PluginConfig::default();
        config.extract = true;

        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut state = TransformState::new(config, metadata);
        let mut folder = RootFolder::new(&mut state);
        let transformed = folder.fold_module(module);
        let result = state.finalize(Program::Module(transformed));

        assert!(result
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")));
    }

    #[test]
    fn handles_selector_arrays_in_css_objects() {
        let dir = tempfile::tempdir().unwrap();
        let entry_path = dir.path().join("entry.tsx");
        let source = r#"
            import { css } from '@compiled/react';

            export const className = css({
              selectors: {
                '&:hover,&:focus': [
                  { color: 'red' },
                  { backgroundColor: 'white' },
                  { textDecoration: 'underline' },
                ],
              },
            });
        "#;

        fs::write(&entry_path, source).unwrap();

        let module = parse_module(&entry_path, source);
        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut non_extract_config = PluginConfig::default();
        non_extract_config.extract = false;
        let mut transform = Transform::new(non_extract_config, metadata.clone());
        let non_extract = transform.apply(Program::Module(module.clone()));
        let non_extract_code = emit_program(&non_extract.program);
        assert_eq!(
            non_extract_code.trim(),
            "import { css } from '@compiled/react';\nexport const className = \"_zicb5scu _vcge1x77 _1ks98stv\";"
        );

        let mut extract_config = PluginConfig::default();
        extract_config.extract = true;
        let mut extract_transform = Transform::new(extract_config, metadata);
        let extract = extract_transform.apply(Program::Module(module));

        assert_eq!(
            extract.style_rules,
            vec![
                String::from("._zicb5scu:hover:hover,._zicb5scu:hover:focus,._zicb5scu:focus:hover,._zicb5scu:focus:focus{color:red}"),
                String::from("._vcge1x77:hover:hover,._vcge1x77:hover:focus,._vcge1x77:focus:hover,._vcge1x77:focus:focus{background-color:white}"),
                String::from("._1ks98stv:hover:hover,._1ks98stv:hover:focus,._1ks98stv:focus:hover,._1ks98stv:focus:focus{text-decoration:underline}"),
            ]
        );
    }

    #[test]
    fn handles_multiple_css_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let entry_path = dir.path().join("entry.tsx");
        let source = r#"
            import { css } from '@compiled/react';

            export const className = css({ color: 'red' }, { fontSize: 20 });
        "#;

        fs::write(&entry_path, source).unwrap();

        let module = parse_module(&entry_path, source);
        let metadata = TransformMetadata {
            filename: Some(entry_path.clone()),
            root_dir: Some(dir.path().to_path_buf()),
            caller: None,
        };

        let mut non_extract_config = PluginConfig::default();
        non_extract_config.extract = false;
        let mut transform = Transform::new(non_extract_config, metadata.clone());
        let non_extract = transform.apply(Program::Module(module.clone()));
        let non_extract_code = emit_program(&non_extract.program);

        assert_eq!(
            non_extract_code.trim(),
            "import { css } from '@compiled/react';\nexport const className = \"_syaz5scu _1wybgktf\";"
        );

        let mut extract_config = PluginConfig::default();
        extract_config.extract = true;
        let mut extract_transform = Transform::new(extract_config, metadata);
        let extract = extract_transform.apply(Program::Module(module));

        assert_eq!(
            extract.style_rules,
            vec![
                String::from("._syaz5scu{color:red}"),
                String::from("._1wybgktf{font-size:20px}"),
            ]
        );
    }
}
