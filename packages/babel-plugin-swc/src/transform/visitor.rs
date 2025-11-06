use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;

use swc_core::common::{SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ArrayLit, ArrowExpr, AssignPat, BinaryOp, BindingIdent, BlockStmt, BlockStmtOrExpr, CallExpr,
    Callee, CondExpr, Decl, Expr, ExprOrSpread, Ident, IfStmt, ImportDecl, ImportSpecifier,
    JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXClosingElement, JSXElement,
    JSXElementChild, JSXElementName, JSXExpr, JSXExprContainer, JSXOpeningElement, KeyValuePatProp,
    KeyValueProp, Lit, MemberExpr, MemberProp, Module, ModuleDecl, ModuleExportName, ModuleItem,
    NewExpr, ObjectLit, ObjectPat, ObjectPatProp, Pat, Prop, PropName, PropOrSpread, RestPat,
    ReturnStmt, SpreadElement, Stmt, Str, TaggedTpl, ThrowStmt, Tpl, VarDecl, VarDeclKind,
    VarDeclarator,
};
use swc_core::ecma::visit::{noop_fold_type, Fold, FoldWith};

use crate::{
    class_names,
    constants::{
        DEFAULT_IMPORT_SOURCES, PROPS_IDENTIFIER_NAME, REF_IDENTIFIER_NAME, STYLE_IDENTIFIER_NAME,
    },
    css::{
        property::{
            array_to_css_string, collapse_whitespace, normalise_at_rule_key,
            normalise_property_pair, serialize_css_object, trim_number, CssObject, CssValue,
        },
        shorthand::{expand_shorthand_property, shorthand_bucket},
    },
    css_map::{
        errors::{create_error_message, ErrorMessages},
        process_selectors::merge_extended_selectors_into_properties,
        validate_root_object, validate_root_variants,
    },
    css_prop, keyframes,
    state::{ImportBinding, ImportKind, TransformState},
    styled::{self, TagType},
    utils::{
        build_display_name::build_display_name_stmt,
        hash::murmur2_hash,
        kebab_case::kebab_case,
        wtf8::{wtf8_to_cow, wtf8_to_string},
    },
    xcss_prop::{
        collect_member_expression_identifiers, collect_pass_styles, is_xcss_attribute,
        STATIC_OBJECT_ERROR, UNEXPECTED_CLASSNAME_COUNT_ERROR,
    },
};

fn css_value_contains_object(value: &CssValue) -> bool {
    match value {
        CssValue::Object(_) => true,
        CssValue::Array(values) => values.iter().any(|item| css_value_contains_object(item)),
        _ => false,
    }
}

pub struct RootFolder<'a> {
    state: &'a mut TransformState,
    css_map_var_depth: usize,
}

impl<'a> RootFolder<'a> {
    pub fn new(state: &'a mut TransformState) -> Self {
        Self {
            state,
            css_map_var_depth: 0,
        }
    }

    fn is_class_names_element(&self, element: &JSXElement) -> bool {
        let swc_core::ecma::ast::JSXElement { opening, .. } = element;
        let swc_core::ecma::ast::JSXElementName::Ident(ident) = &opening.name else {
            return false;
        };

        self.state.is_class_names_ident(ident.sym.as_ref())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct AtRuleEntry {
    display: String,
    hash: String,
}

#[derive(Clone, Default)]
struct CssContext {
    at_rules: Vec<AtRuleEntry>,
    selectors: Vec<String>,
    expand_shorthands: bool,
}

impl CssContext {
    fn root() -> Self {
        Self {
            at_rules: Vec::new(),
            selectors: vec!["&".to_string()],
            expand_shorthands: false,
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
            expand_shorthands: self.expand_shorthands,
        }
    }

    fn with_at_rule(&self, at_rule: &str) -> Self {
        let mut at_rules = self.at_rules.clone();
        at_rules.push(AtRuleEntry {
            display: normalise_at_rule_key(at_rule),
            hash: hash_at_rule_key(at_rule),
        });

        Self {
            at_rules,
            selectors: self.selectors.clone(),
            expand_shorthands: self.expand_shorthands,
        }
    }

    fn enable_shorthand_expansion(mut self) -> Self {
        self.expand_shorthands = true;
        self
    }

    fn wrap_rule(&self, mut rule: String) -> String {
        for at_rule in self.at_rules.iter().rev() {
            rule = format!("{}{{{rule}}}", at_rule.display);
        }

        rule
    }

    fn selectors_for_hash(&self) -> String {
        self.selectors.join("")
    }

    fn at_rules_for_hash(&self) -> String {
        let mut result = String::new();
        for entry in &self.at_rules {
            result.push_str(&entry.hash);
        }

        result
    }

    fn at_rule_stack(&self) -> Vec<String> {
        self.at_rules
            .iter()
            .map(|entry| entry.display.clone())
            .collect()
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

        let mut module = module.fold_children_with(self);
        self.insert_styled_display_names(&mut module);
        module
    }

    fn fold_jsx_element(&mut self, element: JSXElement) -> JSXElement {
        if self.is_class_names_element(&element) {
            return self.transform_class_names_element(element);
        }

        let mut element = element.fold_children_with(self);
        element = self.transform_css_prop(element);
        element = self.transform_xcss_prop(element);

        element
    }

    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Call(call) => self.fold_call_expr(call),
            Expr::TaggedTpl(tagged) => self.fold_tagged_tpl(tagged),
            other => other.fold_children_with(self),
        }
    }

    fn fold_var_declarator(&mut self, declarator: VarDeclarator) -> VarDeclarator {
        let mut declarator = declarator;

        declarator.init = declarator
            .init
            .map(|expr| self.fold_var_init(expr, &declarator.name));

        declarator
    }
}

impl<'a> RootFolder<'a> {
    fn transform_css_prop(&mut self, mut element: JSXElement) -> JSXElement {
        let Some((css_attr, css_index)) = take_jsx_attribute(&mut element.opening, "css") else {
            return element;
        };

        if css_prop::is_css_prop_disabled(self.state, &element, &css_attr) {
            element
                .opening
                .attrs
                .insert(css_index, JSXAttrOrSpread::JSXAttr(css_attr));
            return element;
        }

        let Some(value) = css_attr.value.clone() else {
            element
                .opening
                .attrs
                .insert(css_index, JSXAttrOrSpread::JSXAttr(css_attr));
            return element;
        };

        let Some(expr) = jsx_attr_value_to_expr(value) else {
            element
                .opening
                .attrs
                .insert(css_index, JSXAttrOrSpread::JSXAttr(css_attr));
            return element;
        };

        let Some(css_value) = self.resolve_css_value(&expr) else {
            element
                .opening
                .attrs
                .insert(css_index, JSXAttrOrSpread::JSXAttr(css_attr));
            return element;
        };

        let has_inline_css = css_value_contains_object(&css_value);
        let context = CssContext::root().enable_shorthand_expansion();
        let Some(mut class_names) = self.process_css_root_value(css_value, &context) else {
            return element;
        };

        if class_names.is_empty() {
            return element;
        }

        class_names = dedupe_preserving_order(class_names);

        let key_attr = take_jsx_attribute(&mut element.opening, "key").map(|(attr, _)| attr);
        let existing_class_attr =
            take_jsx_attribute(&mut element.opening, "className").map(|(attr, _)| attr);

        self.apply_class_name(&mut element.opening, existing_class_attr, &class_names);

        if self.state.config.extract {
            return element;
        }

        if has_inline_css {
            let mut sheets = Vec::new();
            for class_name in &class_names {
                for token in class_name.split_whitespace() {
                    if let Some(rule) = self.state.lookup_class_rule(token) {
                        if !sheets.iter().any(|existing: &String| existing == &rule) {
                            sheets.push(rule);
                        }
                    }
                }
            }

            self.build_runtime_wrapper(element, key_attr, sheets)
        } else {
            if let Some(attr) = key_attr {
                element.opening.attrs.push(JSXAttrOrSpread::JSXAttr(attr));
            }

            element
        }
    }

    fn transform_class_names_element(&mut self, element: JSXElement) -> JSXElement {
        class_names::assert_function_children(&element);

        let Some(info) = extract_class_names_render_prop(&element) else {
            return element.fold_children_with(self);
        };

        let mut folder = ClassNamesFolder::new(self, info.css_idents);
        let mut body_expr = info.body;
        body_expr = body_expr.fold_with(&mut folder);
        let collected_class_names = folder.finish();
        body_expr = self.fold_expr(body_expr);

        let mut body_expr = body_expr;
        loop {
            match body_expr {
                Expr::Paren(paren) => {
                    body_expr = *paren.expr;
                    continue;
                }
                other => {
                    body_expr = other;
                    break;
                }
            }
        }

        let Expr::JSXElement(inner_element) = body_expr else {
            return element.fold_children_with(self);
        };

        let mut sheets = Vec::new();
        let mut seen = HashSet::new();
        for class_group in &collected_class_names {
            for class_name in class_group.split(' ').filter(|value| !value.is_empty()) {
                if let Some(rule) = self.state.lookup_class_rule(class_name) {
                    if seen.insert(rule.clone()) {
                        sheets.push(rule);
                    }
                }
            }
        }

        if self.state.config.extract {
            *inner_element
        } else {
            self.build_runtime_wrapper(*inner_element, None, sheets)
        }
    }

    fn transform_xcss_prop(&mut self, mut element: JSXElement) -> JSXElement {
        if !self.state.config.process_xcss {
            return element;
        }

        let attr_index = element
            .opening
            .attrs
            .iter()
            .enumerate()
            .find(|(_, attr)| matches!(attr, JSXAttrOrSpread::JSXAttr(attr) if is_xcss_attribute(attr)))
            .map(|(index, _)| index);

        let Some(index) = attr_index else {
            return element;
        };

        let JSXAttrOrSpread::JSXAttr(attr) = &mut element.opening.attrs[index] else {
            return element;
        };

        let Some(value) = attr.value.as_mut() else {
            return element;
        };

        let JSXAttrValue::JSXExprContainer(container) = value else {
            return element;
        };

        let JSXExpr::Expr(expr_box) = &mut container.expr else {
            return element;
        };

        let expr_clone = (*expr_box).clone();
        let mut sheets: Vec<String> = Vec::new();

        if matches!(expr_box.as_ref(), Expr::Object(_)) {
            let css_value = self
                .resolve_css_value(&expr_clone)
                .unwrap_or_else(|| panic!("{STATIC_OBJECT_ERROR}"));
            let CssValue::Object(map) = css_value else {
                panic!("{STATIC_OBJECT_ERROR}");
            };

            let context = CssContext::root().enable_shorthand_expansion();
            let mut class_names = self
                .process_css_root_value(CssValue::Object(map), &context)
                .unwrap_or_else(|| panic!("{STATIC_OBJECT_ERROR}"));
            class_names = dedupe_preserving_order(class_names);

            match class_names.as_slice() {
                [] => {
                    **expr_box = Expr::Ident(Ident::new(
                        "undefined".into(),
                        DUMMY_SP,
                        SyntaxContext::empty(),
                    ));
                }
                [single] => {
                    **expr_box = Expr::Lit(Lit::Str(Str {
                        span: DUMMY_SP,
                        value: single.clone().into(),
                        raw: None,
                    }));
                }
                _ => panic!("{UNEXPECTED_CLASSNAME_COUNT_ERROR}"),
            }

            for class_name in class_names {
                if let Some(rule) = self.state.lookup_class_rule(&class_name) {
                    if !sheets.iter().any(|existing| existing == &rule) {
                        sheets.push(rule);
                    }
                }
            }
        } else {
            let identifiers = collect_member_expression_identifiers(&expr_clone);

            for name in &identifiers {
                if self.state.css_map_sheets().get(name).is_none() {
                    if let Some(call) = self.state.pending_css_map_call(name).cloned() {
                        let _ = self.process_css_map_declarator(name, call);
                    }
                }
            }

            sheets = collect_pass_styles(self.state, &identifiers);
            if sheets.is_empty() {
                return element;
            }
        }

        self.state.mark_uses_xcss();
        sheets = dedupe_preserving_order(sheets);

        if self.state.config.extract {
            return element;
        }

        let key_attr = take_jsx_attribute(&mut element.opening, "key").map(|(attr, _)| attr);

        self.build_runtime_wrapper(element, key_attr, sheets)
    }

    fn apply_class_name(
        &mut self,
        opening: &mut JSXOpeningElement,
        existing: Option<JSXAttr>,
        class_names: &[String],
    ) {
        let mut expressions: Vec<Expr> = class_names
            .iter()
            .map(|name| {
                Expr::Lit(Lit::Str(Str {
                    span: DUMMY_SP,
                    value: name.clone().into(),
                    raw: None,
                }))
            })
            .collect();

        if let Some(attr) = existing {
            if let Some(value) = attr.value {
                if let Some(expr) = jsx_attr_value_to_expr(value) {
                    expressions.push(expr);
                }
            }
        }

        if expressions.is_empty() {
            return;
        }

        self.state.mark_runtime_class_library_used();

        let runtime_ident = jsx_ident(self.state.runtime_class_library_ident());
        let args = vec![ExprOrSpread {
            spread: None,
            expr: Box::new(Expr::Array(ArrayLit {
                span: DUMMY_SP,
                elems: expressions
                    .into_iter()
                    .map(|expr| {
                        Some(ExprOrSpread {
                            spread: None,
                            expr: Box::new(expr),
                        })
                    })
                    .collect(),
            })),
        }];

        let call = Expr::Call(swc_core::ecma::ast::CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(runtime_ident.clone()))),
            args,
            type_args: None,
            ctxt: SyntaxContext::empty(),
        });

        let attr = JSXAttr {
            span: DUMMY_SP,
            name: JSXAttrName::Ident(jsx_ident("className").into()),
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                span: DUMMY_SP,
                expr: JSXExpr::Expr(Box::new(call)),
            })),
        };

        opening.attrs.push(JSXAttrOrSpread::JSXAttr(attr));
    }

    fn build_runtime_wrapper(
        &mut self,
        element: JSXElement,
        key_attr: Option<JSXAttr>,
        sheets: Vec<String>,
    ) -> JSXElement {
        let cs_element = build_cs_element(&self.state, sheets);
        let mut cc_attrs = Vec::new();

        if let Some(key_attr) = key_attr {
            cc_attrs.push(JSXAttrOrSpread::JSXAttr(key_attr));
        }

        self.state.mark_runtime_components_used();

        JSXElement {
            span: DUMMY_SP,
            opening: JSXOpeningElement {
                name: JSXElementName::Ident(jsx_ident("CC")),
                attrs: cc_attrs,
                self_closing: false,
                type_args: None,
                span: DUMMY_SP,
            },
            closing: Some(JSXClosingElement {
                span: DUMMY_SP,
                name: JSXElementName::Ident(jsx_ident("CC")),
            }),
            children: vec![
                JSXElementChild::JSXElement(Box::new(cs_element)),
                JSXElementChild::JSXElement(Box::new(element)),
            ],
        }
    }

    fn insert_styled_display_names(&mut self, module: &mut Module) {
        let mut pending = self.state.take_styled_display_names();
        if pending.is_empty() {
            return;
        }

        let mut new_body = Vec::with_capacity(module.body.len() + pending.len());

        for item in module.body.drain(..) {
            let names = match &item {
                ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => collect_binding_names(var),
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
                    Decl::Var(var) => collect_binding_names(var),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };

            new_body.push(item);

            for name in names {
                if pending.remove(&name) {
                    new_body.push(ModuleItem::Stmt(build_display_name_stmt(&name, None)));
                }
            }
        }

        for name in pending {
            new_body.push(ModuleItem::Stmt(build_display_name_stmt(&name, None)));
        }

        module.body = new_body;
    }

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
                    if let Expr::Call(call) = &**init {
                        if let Callee::Expr(callee) = &call.callee {
                            if let Expr::Ident(callee_ident) = &**callee {
                                if self.state.is_css_map_ident(callee_ident.sym.as_ref()) {
                                    self.state.record_pending_css_map_call(
                                        ident.id.sym.to_string(),
                                        call.clone(),
                                    );
                                }
                            }
                        }
                    }
                }
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
                let segment = value.into_template_segment(&self.state.config)?;
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

        if self.is_css_map_callee(&call.callee) && self.css_map_var_depth == 0 {
            css_map_panic(ErrorMessages::DefineMap);
        }

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

    fn fold_tagged_tpl(&mut self, tagged: swc_core::ecma::ast::TaggedTpl) -> Expr {
        let tagged = tagged.fold_children_with(self);

        if self.is_styled_tagged_template(&tagged) {
            styled::assert_valid_tagged_template(&tagged);
        }

        if let Expr::Ident(ident) = &*tagged.tag {
            if self.state.is_css_map_ident(ident.sym.as_ref()) {
                css_map_panic(ErrorMessages::NoTaggedTemplate);
            }
            if self.state.is_keyframes_ident(ident.sym.as_ref()) {
                if let Some(expr) = self.process_keyframes_template(&tagged) {
                    return expr;
                }
            }
        }

        Expr::TaggedTpl(tagged)
    }

    fn is_css_map_callee(&self, callee: &Callee) -> bool {
        match callee {
            Callee::Expr(expr) => match &**expr {
                Expr::Ident(ident) => self.state.is_css_map_ident(ident.sym.as_ref()),
                _ => false,
            },
            Callee::Super(_) | Callee::Import(_) => false,
        }
    }

    fn is_styled_tagged_template(&self, tagged: &TaggedTpl) -> bool {
        match &*tagged.tag {
            Expr::Member(member) => {
                if let Expr::Ident(object) = &*member.obj {
                    if self.state.is_styled_ident(object.sym.as_ref()) {
                        return matches!(member.prop, MemberProp::Ident(_));
                    }
                }
                false
            }
            Expr::Call(call) => match &call.callee {
                Callee::Expr(callee) => match &**callee {
                    Expr::Ident(ident) => self.state.is_styled_ident(ident.sym.as_ref()),
                    _ => false,
                },
                Callee::Super(_) | Callee::Import(_) => false,
            },
            _ => false,
        }
    }

    fn fold_var_init(&mut self, expr: Box<Expr>, name: &Pat) -> Box<Expr> {
        let is_css_map_call =
            matches!(&*expr, Expr::Call(call) if self.is_css_map_callee(&call.callee));

        if is_css_map_call {
            self.css_map_var_depth += 1;
        }

        let expr = *expr.fold_with(self);

        if is_css_map_call {
            self.css_map_var_depth -= 1;
        }

        let expr = expr;

        if let Some(identifier) = extract_binding_name(name) {
            if let Some(transformed) = self.process_styled_declarator(&identifier, &expr) {
                return Box::new(transformed);
            }

            if let Expr::Call(call) = &expr {
                if let Some(transformed) =
                    self.process_css_map_declarator(&identifier, call.clone())
                {
                    return Box::new(transformed);
                }
            }

            if let Some(value) = self.evaluate_expr(&expr) {
                self.state.register_local_binding(identifier, value);
            }
        }

        Box::new(expr)
    }

    fn process_styled_declarator(&mut self, binding: &str, expr: &Expr) -> Option<Expr> {
        let data = styled::extract_styled_data(expr, &self.state)?;
        self.lower_styled_component(binding, data)
    }

    fn lower_styled_component(&mut self, binding: &str, data: styled::StyledData) -> Option<Expr> {
        match data.css {
            styled::StyledCss::ObjectArgs(args) => {
                self.lower_styled_object(binding, data.tag, args)
            }
            styled::StyledCss::TaggedTemplate(_) => None,
        }
    }

    fn lower_styled_object(
        &mut self,
        binding: &str,
        tag: styled::Tag,
        args: Vec<Expr>,
    ) -> Option<Expr> {
        let mut class_names = Vec::new();
        let context = CssContext::root().enable_shorthand_expansion();

        for arg in &args {
            let value = self.resolve_css_value(arg)?;
            let entries = self.process_css_root_value(value, &context)?;
            class_names.extend(entries);
        }

        if class_names.is_empty() {
            return None;
        }

        let mut class_names = dedupe_preserving_order(class_names);
        let dev_env = is_development_env();

        if self.state.config.add_component_name && dev_env {
            class_names.insert(0, format!("c_{}", binding));
        }

        let mut sheets = Vec::new();
        for class_name in &class_names {
            if let Some(rule) = self.state.lookup_class_rule(class_name) {
                if !sheets.iter().any(|existing: &String| existing == &rule) {
                    sheets.push(rule);
                }
            }
        }

        let mut opening = JSXOpeningElement {
            name: JSXElementName::Ident(jsx_ident("C")),
            attrs: Vec::new(),
            self_closing: true,
            type_args: None,
            span: DUMMY_SP,
        };

        opening
            .attrs
            .push(JSXAttrOrSpread::SpreadElement(SpreadElement {
                dot3_token: DUMMY_SP,
                expr: Box::new(Expr::Ident(Ident::new(
                    PROPS_IDENTIFIER_NAME.into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                ))),
            }));

        opening.attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
            span: DUMMY_SP,
            name: JSXAttrName::Ident(jsx_ident("style").into()),
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                span: DUMMY_SP,
                expr: JSXExpr::Expr(Box::new(Expr::Ident(Ident::new(
                    STYLE_IDENTIFIER_NAME.into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                )))),
            })),
        }));

        opening.attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
            span: DUMMY_SP,
            name: JSXAttrName::Ident(jsx_ident("ref").into()),
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                span: DUMMY_SP,
                expr: JSXExpr::Expr(Box::new(Expr::Ident(Ident::new(
                    REF_IDENTIFIER_NAME.into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                )))),
            })),
        }));

        let class_member = Expr::Member(MemberExpr {
            span: DUMMY_SP,
            obj: Box::new(Expr::Ident(Ident::new(
                PROPS_IDENTIFIER_NAME.into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            ))),
            prop: MemberProp::Ident(
                Ident::new("className".into(), DUMMY_SP, SyntaxContext::empty()).into(),
            ),
        });

        let existing_class_attr = JSXAttr {
            span: DUMMY_SP,
            name: JSXAttrName::Ident(jsx_ident("className").into()),
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                span: DUMMY_SP,
                expr: JSXExpr::Expr(Box::new(class_member)),
            })),
        };

        self.apply_class_name(&mut opening, Some(existing_class_attr), &class_names);

        let inner_element = JSXElement {
            span: DUMMY_SP,
            opening,
            closing: None,
            children: Vec::new(),
        };

        let return_element = if self.state.config.extract {
            inner_element
        } else {
            self.build_runtime_wrapper(inner_element, None, sheets)
        };

        let mut stmts = Vec::new();

        if dev_env {
            let condition = Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: Box::new(Expr::Ident(Ident::new(
                    PROPS_IDENTIFIER_NAME.into(),
                    DUMMY_SP,
                    SyntaxContext::empty(),
                ))),
                prop: MemberProp::Ident(
                    Ident::new("innerRef".into(), DUMMY_SP, SyntaxContext::empty()).into(),
                ),
            });

            let throw_stmt = Stmt::Throw(ThrowStmt {
                span: DUMMY_SP,
                arg: Box::new(Expr::New(NewExpr {
                    span: DUMMY_SP,
                    callee: Box::new(Expr::Ident(Ident::new(
                        "Error".into(),
                        DUMMY_SP,
                        SyntaxContext::empty(),
                    ))),
                    args: Some(vec![ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Lit(Lit::Str(Str {
                            span: DUMMY_SP,
                            value: "Please use 'ref' instead of 'innerRef'.".into(),
                            raw: None,
                        }))),
                    }]),
                    type_args: None,
                    ctxt: SyntaxContext::empty(),
                })),
            });

            stmts.push(Stmt::If(IfStmt {
                span: DUMMY_SP,
                test: Box::new(condition),
                cons: Box::new(Stmt::Block(BlockStmt {
                    span: DUMMY_SP,
                    stmts: vec![throw_stmt],
                    ctxt: SyntaxContext::empty(),
                })),
                alt: None,
            }));
        }

        stmts.push(Stmt::Return(ReturnStmt {
            span: DUMMY_SP,
            arg: Some(Box::new(Expr::JSXElement(Box::new(return_element)))),
        }));

        let component_ident = Ident::new("C".into(), DUMMY_SP, SyntaxContext::empty());

        let default_tag_expr = match tag.tag_type {
            TagType::InBuiltComponent => Expr::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                value: tag.name.clone().into(),
                raw: None,
            })),
            TagType::UserDefinedComponent => Expr::Ident(Ident::new(
                tag.name.clone().into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            )),
        };

        let first_param = Pat::Object(ObjectPat {
            span: DUMMY_SP,
            props: vec![
                ObjectPatProp::KeyValue(KeyValuePatProp {
                    key: PropName::Ident(
                        Ident::new("as".into(), DUMMY_SP, SyntaxContext::empty()).into(),
                    ),
                    value: Box::new(Pat::Assign(AssignPat {
                        span: DUMMY_SP,
                        left: Box::new(Pat::Ident(BindingIdent {
                            id: component_ident.clone(),
                            type_ann: None,
                        })),
                        right: Box::new(default_tag_expr),
                    })),
                }),
                ObjectPatProp::KeyValue(KeyValuePatProp {
                    key: PropName::Ident(
                        Ident::new("style".into(), DUMMY_SP, SyntaxContext::empty()).into(),
                    ),
                    value: Box::new(Pat::Ident(BindingIdent {
                        id: Ident::new(
                            STYLE_IDENTIFIER_NAME.into(),
                            DUMMY_SP,
                            SyntaxContext::empty(),
                        ),
                        type_ann: None,
                    })),
                }),
                ObjectPatProp::Rest(RestPat {
                    dot3_token: DUMMY_SP,
                    span: DUMMY_SP,
                    arg: Box::new(Pat::Ident(BindingIdent {
                        id: Ident::new(
                            PROPS_IDENTIFIER_NAME.into(),
                            DUMMY_SP,
                            SyntaxContext::empty(),
                        ),
                        type_ann: None,
                    })),
                    type_ann: None,
                }),
            ],
            optional: false,
            type_ann: None,
        });

        let second_param = Pat::Ident(BindingIdent {
            id: Ident::new(REF_IDENTIFIER_NAME.into(), DUMMY_SP, SyntaxContext::empty()),
            type_ann: None,
        });

        let arrow = Expr::Arrow(ArrowExpr {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            params: vec![first_param, second_param],
            body: Box::new(BlockStmtOrExpr::BlockStmt(BlockStmt {
                span: DUMMY_SP,
                stmts,
                ctxt: SyntaxContext::empty(),
            })),
            is_async: false,
            is_generator: false,
            type_params: None,
            return_type: None,
        });

        self.state.record_styled_display_name(binding.to_string());

        Some(Expr::Call(swc_core::ecma::ast::CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(
                "forwardRef".into(),
                DUMMY_SP,
                SyntaxContext::empty(),
            )))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(arrow),
            }],
            type_args: None,
            ctxt: SyntaxContext::empty(),
        }))
    }

    fn process_css_map_declarator(
        &mut self,
        binding: &str,
        call: swc_core::ecma::ast::CallExpr,
    ) -> Option<Expr> {
        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Ident(ident) = &**callee {
                if !self.state.is_css_map_ident(ident.sym.as_ref()) {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }

        if call.args.len() != 1 {
            css_map_panic(ErrorMessages::NumberOfArgument);
        }

        let arg = call.args.first().expect("cssMap expects one argument");
        if arg.spread.is_some() {
            css_map_panic(ErrorMessages::ArgumentType);
        }

        let object_literal =
            validate_root_object(&arg.expr).unwrap_or_else(|code| css_map_panic(code));
        validate_root_variants(object_literal).unwrap_or_else(|code| css_map_panic(code));

        let value = self
            .resolve_css_value(&arg.expr)
            .unwrap_or_else(|| css_map_panic(ErrorMessages::StaticVariantObject));
        let map = value
            .into_object()
            .unwrap_or_else(|| css_map_panic(ErrorMessages::ArgumentType));
        let context = CssContext::root().enable_shorthand_expansion();

        let mut props: Vec<PropOrSpread> = Vec::new();
        let mut binding_map: CssObject = CssObject::new();
        let mut total_sheets: Vec<String> = Vec::new();

        for (key, value) in map.iter() {
            let merged = match value.clone() {
                CssValue::Object(object) => merge_extended_selectors_into_properties(&object)
                    .unwrap_or_else(|code| css_map_panic(code)),
                _ => css_map_panic(ErrorMessages::StaticVariantObject),
            };

            let class_names = self
                .process_css_root_value(CssValue::Object(merged.clone()), &context)
                .unwrap_or_else(|| css_map_panic(ErrorMessages::StaticVariantObject));
            let class_value = match class_names.as_slice() {
                [] => String::new(),
                [single] => single.clone(),
                _ => class_names.join(" "),
            };
            for class_name in &class_names {
                if let Some(rule) = self.state.lookup_class_rule(class_name) {
                    if !total_sheets.iter().any(|existing| existing == &rule) {
                        total_sheets.push(rule);
                    }
                }
            }
            let prop_name = prop_name_from_key(key);
            let expr = Expr::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                value: class_value.clone().into(),
                raw: None,
            }));

            binding_map.insert(key.clone(), CssValue::String(class_value));

            props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                key: prop_name,
                value: Box::new(expr),
            }))));
        }

        self.state.record_css_map_sheets(binding, total_sheets);

        if !binding_map.is_empty() {
            self.state
                .register_local_binding(binding.to_string(), CssValue::Object(binding_map));
        }

        self.state.remove_pending_css_map_call(binding);

        Some(Expr::Object(ObjectLit {
            span: DUMMY_SP,
            props,
        }))
    }

    fn process_keyframes_call(&mut self, call: &swc_core::ecma::ast::CallExpr) -> Option<Expr> {
        let arg = call.args.first()?;
        let value = self.evaluate_expr(&arg.expr)?;
        let map = match value {
            CssValue::Object(map) => map,
            _ => return None,
        };

        let (name, rule) = keyframes::build_keyframes_rule(call, &map, &self.state.config)?;
        self.state.push_style_rule(rule);

        Some(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: name.into(),
            raw: None,
        })))
    }

    fn process_keyframes_template(
        &mut self,
        tagged: &swc_core::ecma::ast::TaggedTpl,
    ) -> Option<Expr> {
        let value = self.evaluate_template_literal(&tagged.tpl)?;
        let CssValue::String(body) = value else {
            return None;
        };

        let (name, rule) =
            keyframes::build_keyframes_rule_from_template(tagged, &body, &self.state.config)?;
        self.state.push_style_rule(rule);

        Some(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: name.into(),
            raw: None,
        })))
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

        let mut entries: Vec<_> = map
            .iter()
            .enumerate()
            .map(|(index, (key, value))| {
                let kebab = if key.starts_with("--") {
                    None
                } else {
                    Some(kebab_case(key))
                };
                let bucket = kebab.as_deref().and_then(|name| shorthand_bucket(name));
                (index, key, value, bucket)
            })
            .collect();

        entries.sort_by(|(index_a, _, _, bucket_a), (index_b, _, _, bucket_b)| {
            match (bucket_a, bucket_b) {
                (Some(a), Some(b)) => a.cmp(b).then_with(|| index_a.cmp(index_b)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => index_a.cmp(index_b),
            }
        });

        for (_, key, value, _) in entries {
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
                        if let Some(body) = serialize_css_object(&map, &self.state.config) {
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
        let Some((property_name, value)) = normalise_property_pair(key, value, &self.state.config)
        else {
            return Vec::new();
        };

        if context.expand_shorthands {
            if let Some(expanded) = expand_shorthand_property(&property_name, &value) {
                if expanded.is_empty() {
                    return Vec::new();
                }

                let mut class_names = Vec::new();
                for (name, value) in expanded {
                    class_names.push(self.emit_atomic_pair(&name, &value, context));
                }

                return class_names;
            }
        }

        vec![self.emit_atomic_pair(&property_name, &value, context)]
    }

    fn emit_atomic_pair(
        &mut self,
        property_name: &str,
        value: &str,
        context: &CssContext,
    ) -> String {
        let class_name = self.build_atomic_class(property_name, value, context);
        let selector_rule = build_selector_rule(context, &class_name, property_name, value);
        let at_rule_stack = context.at_rule_stack();
        let at_rule_hash_key = context.at_rules_for_hash();
        let rule =
            self.state
                .record_atomic_style(at_rule_stack.clone(), at_rule_hash_key, &selector_rule);
        self.state
            .record_class_rule(&class_name, &at_rule_stack, &rule);
        class_name
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

fn extract_binding_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) => Some(ident.id.sym.to_string()),
        _ => None,
    }
}

fn collect_binding_names(var: &VarDecl) -> Vec<String> {
    var.decls
        .iter()
        .filter_map(|decl| extract_binding_name(&decl.name))
        .collect()
}

fn is_development_env() -> bool {
    let babel_env = std::env::var("BABEL_ENV").ok();
    let node_env = std::env::var("NODE_ENV").ok();
    let default_env = babel_env.is_none() && node_env.is_none();

    let is_dev_like = |value: Option<&String>| match value {
        Some(name) => matches!(name.as_str(), "development" | "test"),
        None => false,
    };

    default_env || is_dev_like(babel_env.as_ref()) || is_dev_like(node_env.as_ref())
}

fn take_jsx_attribute(opening: &mut JSXOpeningElement, name: &str) -> Option<(JSXAttr, usize)> {
    let index = opening.attrs.iter().position(|attr| match attr {
        JSXAttrOrSpread::JSXAttr(attr) => match &attr.name {
            JSXAttrName::Ident(ident) => ident.sym.as_ref() == name,
            _ => false,
        },
        JSXAttrOrSpread::SpreadElement(_) => false,
    })?;

    if !matches!(opening.attrs[index], JSXAttrOrSpread::JSXAttr(_)) {
        return None;
    }

    match opening.attrs.remove(index) {
        JSXAttrOrSpread::JSXAttr(attr) => Some((attr, index)),
        JSXAttrOrSpread::SpreadElement(_) => None,
    }
}

fn jsx_attr_value_to_expr(value: JSXAttrValue) -> Option<Expr> {
    match value {
        JSXAttrValue::Lit(lit) => Some(Expr::Lit(lit)),
        JSXAttrValue::JSXExprContainer(container) => match container.expr {
            JSXExpr::Expr(expr) => Some(*expr),
            JSXExpr::JSXEmptyExpr(_) => None,
        },
        JSXAttrValue::JSXElement(element) => Some(Expr::JSXElement(element)),
        JSXAttrValue::JSXFragment(fragment) => Some(Expr::JSXFragment(fragment)),
    }
}

fn jsx_ident(name: &str) -> Ident {
    Ident::new(name.into(), DUMMY_SP, SyntaxContext::empty())
}

fn build_cs_element(state: &TransformState, sheets: Vec<String>) -> JSXElement {
    let array_expr = Expr::Array(ArrayLit {
        span: DUMMY_SP,
        elems: sheets
            .into_iter()
            .map(|sheet| {
                Some(ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Lit(Lit::Str(Str {
                        span: DUMMY_SP,
                        value: sheet.into(),
                        raw: None,
                    }))),
                })
            })
            .collect(),
    });

    let mut attrs = Vec::new();

    if let Some(nonce) = &state.config.nonce {
        let ident = jsx_ident(nonce);
        attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
            span: DUMMY_SP,
            name: JSXAttrName::Ident(jsx_ident("nonce").into()),
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                span: DUMMY_SP,
                expr: JSXExpr::Expr(Box::new(Expr::Ident(ident))),
            })),
        }));
    }

    JSXElement {
        span: DUMMY_SP,
        opening: JSXOpeningElement {
            name: JSXElementName::Ident(jsx_ident("CS")),
            attrs,
            self_closing: false,
            type_args: None,
            span: DUMMY_SP,
        },
        closing: Some(JSXClosingElement {
            span: DUMMY_SP,
            name: JSXElementName::Ident(jsx_ident("CS")),
        }),
        children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
            span: DUMMY_SP,
            expr: JSXExpr::Expr(Box::new(array_expr)),
        })],
    }
}

struct ClassNamesRenderPropInfo {
    body: Expr,
    css_idents: Vec<String>,
}

fn extract_class_names_render_prop(element: &JSXElement) -> Option<ClassNamesRenderPropInfo> {
    let child = element.children.iter().find_map(|child| match child {
        JSXElementChild::JSXExprContainer(container) => Some(container),
        _ => None,
    })?;

    let expr = match &child.expr {
        JSXExpr::Expr(expr) => expr,
        _ => return None,
    };

    match &**expr {
        expr @ Expr::Arrow(_) => {
            let Expr::Arrow(arrow) = expr else {
                unreachable!()
            };
            let css_idents = collect_class_names_css_idents(&arrow.params);
            let BlockStmtOrExpr::Expr(body) = &*arrow.body else {
                return None;
            };

            Some(ClassNamesRenderPropInfo {
                body: *body.clone(),
                css_idents,
            })
        }
        other => {
            panic!("unexpected classnames expr: {:?}", other);
        }
    }
}

fn collect_class_names_css_idents(params: &[Pat]) -> Vec<String> {
    let mut result = Vec::new();

    for pat in params {
        collect_css_idents_from_pat(pat, &mut result);
    }

    result
}

fn collect_css_idents_from_pat(pat: &Pat, output: &mut Vec<String>) {
    match pat {
        Pat::Ident(ident) => {
            if ident.id.sym.as_ref() == "css" {
                output.push(ident.id.sym.to_string());
            }
        }
        Pat::Assign(assign) => {
            collect_css_idents_from_pat(&assign.left, output);
        }
        Pat::Object(object) => {
            for prop in &object.props {
                match prop {
                    ObjectPatProp::KeyValue(kv) => {
                        if let PropName::Ident(ident) = &kv.key {
                            if ident.sym.as_ref() == "css" {
                                collect_css_idents_from_pat(&kv.value, output);
                            }
                        }
                        if let PropName::Str(str) = &kv.key {
                            if str.value.as_ref() == "css" {
                                collect_css_idents_from_pat(&kv.value, output);
                            }
                        }
                    }
                    ObjectPatProp::Assign(assign) => {
                        if assign.key.sym.as_ref() == "css" {
                            output.push(assign.key.sym.to_string());
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        collect_css_idents_from_pat(&rest.arg, output);
                    }
                }
            }
        }
        _ => {}
    }
}

struct ClassNamesFolder<'a, 'b> {
    parent: &'a mut RootFolder<'b>,
    css_idents: HashSet<String>,
    collected_class_names: Vec<String>,
}

impl<'a, 'b> ClassNamesFolder<'a, 'b> {
    fn new(parent: &'a mut RootFolder<'b>, css_idents: Vec<String>) -> Self {
        Self {
            parent,
            css_idents: css_idents.into_iter().collect(),
            collected_class_names: Vec::new(),
        }
    }

    fn finish(self) -> Vec<String> {
        self.collected_class_names
    }
}

impl<'a, 'b> Fold for ClassNamesFolder<'a, 'b> {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        let expr = expr.fold_children_with(self);

        if let Expr::Call(call) = expr {
            let call = call;
            if let Callee::Expr(callee) = &call.callee {
                if let Expr::Ident(ident) = &**callee {
                    if self.css_idents.contains(ident.sym.as_ref()) {
                        if let Some(result) = self.parent.process_css_call(&call) {
                            if let Expr::Lit(Lit::Str(str)) = &result {
                                self.collected_class_names.push(str.value.to_string());
                            }

                            let runtime_ident = Ident::new(
                                self.parent.state.runtime_class_library_ident().into(),
                                DUMMY_SP,
                                SyntaxContext::empty(),
                            );
                            self.parent.state.mark_runtime_class_library_used();

                            let array_expr = Expr::Array(ArrayLit {
                                span: DUMMY_SP,
                                elems: vec![Some(ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(result),
                                })],
                            });

                            return Expr::Call(CallExpr {
                                span: DUMMY_SP,
                                callee: Callee::Expr(Box::new(Expr::Ident(runtime_ident))),
                                args: vec![ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(array_expr),
                                }],
                                type_args: None,
                                ctxt: SyntaxContext::empty(),
                            });
                        }
                    }
                }
            }

            Expr::Call(call)
        } else {
            expr
        }
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
            let mut collapsed = collapse_whitespace(part);
            if collapsed.is_empty() {
                return "&".to_string();
            }

            if collapsed.starts_with(':') {
                collapsed = format!("&{collapsed}");
            }

            if collapsed.contains('&') {
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

fn prop_name_from_key(key: &str) -> PropName {
    if is_valid_identifier(key) {
        PropName::Ident(Ident::new(key.into(), DUMMY_SP, SyntaxContext::empty()).into())
    } else {
        PropName::Str(Str {
            span: DUMMY_SP,
            value: key.into(),
            raw: None,
        })
    }
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();

    match chars.next() {
        Some(ch) if ch == '_' || ch == '$' || ch.is_ascii_alphabetic() => {}
        _ => return false,
    }

    for ch in chars {
        if !(ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()) {
            return false;
        }
    }

    true
}

fn hash_at_rule_key(key: &str) -> String {
    normalise_at_rule_key(key)
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

fn css_map_panic(code: ErrorMessages) -> ! {
    panic!("{}", create_error_message(code));
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
            source_map: None,
            comments: None,
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
