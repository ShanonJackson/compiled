use std::cell::RefCell;
use std::rc::Rc;

use std::path::{Path, PathBuf};

use swc_core::common::{Span, Spanned};
use swc_core::ecma::ast::{
    BlockStmt, Decl, DefaultDecl, EmptyStmt, Expr, Ident, ImportDecl, ImportNamedSpecifier,
    ImportSpecifier, Module, ModuleDecl, ModuleExportName, ModuleItem, Pat, Program, Stmt, VarDecl,
    VarDeclarator,
};
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use crate::class_names::visit_class_names;
use crate::css_map::{visit_css_map_path, CssMapUsage};
use crate::css_prop::visit_css_prop;
use crate::styled::{visit_styled, StyledVisitResult};
use crate::types::{
    CompiledImports, Metadata, PluginOptions, SharedTransformState, TransformFile,
    TransformMetadata, TransformState,
};
use crate::utils_append_runtime_imports::append_runtime_imports;
use crate::xcss_prop::visit_xcss_prop;

/// Primary SWC transform that will eventually mirror `@compiled/babel-plugin`.
pub struct CompiledBabelTransform {
    state: SharedTransformState,
    metadata: TransformMetadata,
}

impl CompiledBabelTransform {
    pub fn new(options: PluginOptions) -> Self {
        let state = Rc::new(RefCell::new(TransformState::new(
            TransformFile::default(),
            options,
        )));

        Self {
            state,
            metadata: TransformMetadata::default(),
        }
    }

    pub fn into_metadata(self) -> TransformMetadata {
        let mut metadata = self.metadata;
        let state = self.state.borrow();

        if metadata.included_files.is_empty() {
            metadata.included_files = state.included_files.clone();
        }

        metadata
    }

    pub fn state(&self) -> SharedTransformState {
        Rc::clone(&self.state)
    }

    pub fn metadata_mut(&mut self) -> &mut TransformMetadata {
        &mut self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::CompiledBabelTransform;
    use crate::types::{PluginOptions, TransformFile, TransformFileOptions};
    use swc_core::common::sync::Lrc;
    use swc_core::common::{FileName, SourceMap};
    use swc_core::ecma::ast::{
        BlockStmtOrExpr, Decl, Expr, ImportSpecifier, JSXElementName, ModuleDecl, ModuleItem,
        Program, Stmt,
    };
    use swc_core::ecma::visit::VisitMutWith;
    use swc_ecma_parser::lexer::Lexer;
    use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax};

    fn parse_program(code: &str) -> (Program, Lrc<SourceMap>) {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(Lrc::new(FileName::Custom("test.tsx".into())), code.into());
        let lexer = Lexer::new(
            Syntax::Es(EsSyntax {
                jsx: true,
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        let program = parser.parse_program().expect("failed to parse program");
        assert!(parser.take_errors().is_empty());

        (program, cm)
    }

    #[test]
    fn records_compiled_imports_and_removes_matched_specifiers() {
        let (mut program, _) =
            parse_program("import { styled, ClassNames } from '@compiled/react';");

        let mut transform = CompiledBabelTransform::new(PluginOptions::default());
        program.visit_mut_with(&mut transform);

        let Program::Module(module) = &program else {
            panic!("expected module program");
        };
        assert_eq!(module.body.len(), 1);
        let ModuleItem::ModuleDecl(ModuleDecl::Import(runtime_import)) = &module.body[0] else {
            panic!("expected runtime import");
        };
        assert_eq!(runtime_import.src.value.as_ref(), "@compiled/react/runtime");

        let state = transform.state();
        let state_ref = state.borrow();
        let imports = state_ref
            .compiled_imports
            .as_ref()
            .expect("compiled imports should be tracked");
        assert_eq!(imports.styled, vec!["styled".to_string()]);
        assert_eq!(imports.class_names, vec!["ClassNames".to_string()]);
        assert!(imports.css.is_empty());
        assert!(imports.keyframes.is_empty());
        assert!(imports.css_map.is_empty());
    }

    #[test]
    fn retains_unmatched_specifiers() {
        let (mut program, _) = parse_program("import { something } from '@compiled/react';");

        let mut transform = CompiledBabelTransform::new(PluginOptions::default());
        program.visit_mut_with(&mut transform);

        let Program::Module(module) = &program else {
            panic!("expected module program");
        };

        assert_eq!(module.body.len(), 1);
        let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = &module.body[0] else {
            panic!("expected retained import");
        };
        assert_eq!(import.specifiers.len(), 1);

        let state = transform.state();
        let state_ref = state.borrow();
        let imports = state_ref
            .compiled_imports
            .as_ref()
            .expect("compiled imports should be initialised");
        assert!(imports.styled.is_empty());
        assert!(imports.class_names.is_empty());
        assert!(imports.css.is_empty());
        assert!(imports.keyframes.is_empty());
        assert!(imports.css_map.is_empty());
    }

    #[test]
    fn transforms_css_prop_into_compiled_component() {
        let source = r#"
            import { css } from '@compiled/react';

            const Component = () => <div css={{ color: 'red' }} />;
        "#;

        let (mut program, cm) = parse_program(source);

        let mut transform = CompiledBabelTransform::new(PluginOptions::default());
        {
            let file = TransformFile::with_options(
                cm.clone(),
                Vec::new(),
                TransformFileOptions {
                    filename: Some("test.tsx".into()),
                    ..TransformFileOptions::default()
                },
            );
            let mut state = transform.state.borrow_mut();
            state.filename = file.filename.clone();
            state.cwd = file.cwd.clone();
            state.root = file.root.clone();
            state.file = file;
        }
        program.visit_mut_with(&mut transform);

        let Program::Module(module) = &program else {
            panic!("expected module program");
        };

        assert_eq!(module.body.len(), 2);

        let ModuleItem::ModuleDecl(ModuleDecl::Import(runtime_import)) = &module.body[0] else {
            panic!("expected runtime import");
        };
        assert_eq!(runtime_import.src.value.as_ref(), "@compiled/react/runtime");
        let specifiers: Vec<String> = runtime_import
            .specifiers
            .iter()
            .map(|specifier| match specifier {
                ImportSpecifier::Named(named) => named.local.sym.to_string(),
                ImportSpecifier::Default(default) => default.local.sym.to_string(),
                ImportSpecifier::Namespace(namespace) => namespace.local.sym.to_string(),
            })
            .collect();
        assert_eq!(specifiers, vec!["ax", "ix", "CC", "CS"]);

        let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) = &module.body[1] else {
            panic!("expected variable declaration");
        };
        assert_eq!(var_decl.decls.len(), 1);
        let declarator = &var_decl.decls[0];
        let Some(init) = &declarator.init else {
            panic!("expected initializer");
        };

        let Expr::Arrow(arrow) = &**init else {
            panic!("expected arrow expression");
        };
        let BlockStmtOrExpr::Expr(body_expr) = arrow.body.as_ref() else {
            panic!("expected expression body");
        };
        let Expr::JSXElement(element) = &**body_expr else {
            panic!("expected jsx element");
        };

        let JSXElementName::Ident(ident) = &element.opening.name else {
            panic!("expected CC identifier");
        };
        assert_eq!(ident.sym.as_ref(), "CC");
    }

    #[test]
    fn transforms_styled_usage_and_inserts_display_name() {
        let source = r#"
            import { styled } from '@compiled/react';

            const Component = styled.div({ color: 'red' });
        "#;

        let (mut program, cm) = parse_program(source);

        let mut transform = CompiledBabelTransform::new(PluginOptions {
            add_component_name: Some(true),
            ..PluginOptions::default()
        });

        {
            let file = TransformFile::with_options(
                cm.clone(),
                Vec::new(),
                TransformFileOptions {
                    filename: Some("test.tsx".into()),
                    ..TransformFileOptions::default()
                },
            );
            let mut state = transform.state.borrow_mut();
            state.filename = file.filename.clone();
            state.cwd = file.cwd.clone();
            state.root = file.root.clone();
            state.file = file;
        }

        program.visit_mut_with(&mut transform);

        let Program::Module(module) = &program else {
            panic!("expected module program");
        };

        assert_eq!(module.body.len(), 3);

        let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) = &module.body[1] else {
            panic!("expected styled variable declaration");
        };
        assert_eq!(var_decl.decls.len(), 1);

        let ModuleItem::Stmt(Stmt::If(_)) = &module.body[2] else {
            panic!("expected display name assignment");
        };
    }
}

fn imported_name(specifier: &ImportNamedSpecifier) -> &str {
    match &specifier.imported {
        Some(ModuleExportName::Ident(ident)) => ident.sym.as_ref(),
        Some(ModuleExportName::Str(value)) => value.value.as_ref(),
        None => specifier.local.sym.as_ref(),
    }
}

fn normalized_join(base: &Path, segment: &str) -> PathBuf {
    base.join(segment).components().collect()
}

fn is_compiled_module(user_module: &str, state: &TransformState) -> bool {
    if state
        .import_sources
        .iter()
        .any(|origin| origin == user_module)
    {
        return true;
    }

    if !user_module.starts_with('.') {
        return false;
    }

    let Some(filename) = &state.filename else {
        return false;
    };

    let file_path = Path::new(filename);
    let base_dir = file_path.parent().unwrap_or_else(|| Path::new(""));
    let resolved = normalized_join(base_dir, user_module);

    state
        .import_sources
        .iter()
        .any(|origin| normalized_join(Path::new(""), origin) == resolved)
}

fn record_compiled_import(imports: &mut CompiledImports, name: &str, local: &str) -> bool {
    match name {
        "styled" => {
            imports.styled.push(local.to_string());
            true
        }
        "ClassNames" => {
            imports.class_names.push(local.to_string());
            true
        }
        "css" => {
            imports.css.push(local.to_string());
            true
        }
        "keyframes" => {
            imports.keyframes.push(local.to_string());
            true
        }
        "cssMap" => {
            imports.css_map.push(local.to_string());
            true
        }
        _ => false,
    }
}

fn has_active_compiled_imports(imports: &CompiledImports) -> bool {
    !(imports.class_names.is_empty()
        && imports.css.is_empty()
        && imports.keyframes.is_empty()
        && imports.styled.is_empty()
        && imports.css_map.is_empty())
}

impl VisitMut for CompiledBabelTransform {
    noop_visit_mut_type!();

    fn visit_mut_program(&mut self, program: &mut Program) {
        match program {
            Program::Module(module) => {
                self.visit_mut_module(module);

                let mut state = self.state.borrow_mut();
                let has_imports = state
                    .compiled_imports
                    .as_ref()
                    .map(has_active_compiled_imports)
                    .unwrap_or(false);
                if has_imports || state.uses_xcss {
                    append_runtime_imports(program, &mut state);
                }
            }
            Program::Script(script) => script.visit_mut_children_with(self),
        }
    }
}

impl CompiledBabelTransform {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let mut index = 0;

        while index < module.body.len() {
            let keep = match &mut module.body[index] {
                ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                    self.visit_mut_import_decl(import)
                }
                item => {
                    item.visit_mut_with(self);
                    true
                }
            };

            if keep {
                index += 1;
            } else {
                module.body.remove(index);
            }
        }

        let (
            has_css_import,
            has_styled_import,
            has_class_names_import,
            has_css_map_import,
            process_xcss,
        ) = {
            let state = self.state.borrow();
            let imports = state.compiled_imports.clone();
            (
                imports
                    .as_ref()
                    .map(|imports| !imports.css.is_empty())
                    .unwrap_or(false),
                imports
                    .as_ref()
                    .map(|imports| !imports.styled.is_empty())
                    .unwrap_or(false),
                imports
                    .as_ref()
                    .map(|imports| !imports.class_names.is_empty())
                    .unwrap_or(false),
                imports
                    .as_ref()
                    .map(|imports| !imports.css_map.is_empty())
                    .unwrap_or(false),
                state.opts.process_xcss.unwrap_or(false),
            )
        };

        if has_styled_import {
            let metadata = Metadata::new(self.state());
            let mut visitor = StyledVisitor::new(metadata);
            module.visit_mut_with(&mut visitor);
            visitor.insert_display_names(module);
        }

        if has_css_import {
            let metadata = Metadata::new(self.state());
            let mut visitor = CssPropVisitor::new(metadata);
            module.visit_mut_with(&mut visitor);
        }

        if has_css_map_import {
            let metadata = Metadata::new(self.state());
            let mut visitor = CssMapVisitor::new(metadata);
            module.visit_mut_with(&mut visitor);
        }

        if has_class_names_import {
            let metadata = Metadata::new(self.state());
            let mut visitor = ClassNamesVisitor::new(metadata);
            module.visit_mut_with(&mut visitor);
        }

        if process_xcss {
            let metadata = Metadata::new(self.state());
            let mut visitor = XcssVisitor::new(metadata);
            module.visit_mut_with(&mut visitor);
        }
    }

    fn visit_mut_import_decl(&mut self, import: &mut ImportDecl) -> bool {
        let module_name = import.src.value.to_string();

        let mut state = self.state.borrow_mut();
        if !is_compiled_module(&module_name, &state) {
            return true;
        }

        let mut remaining = Vec::with_capacity(import.specifiers.len());
        let mut css_alias: Option<String> = None;

        {
            let compiled_imports = state
                .compiled_imports
                .get_or_insert_with(CompiledImports::default);

            for specifier in import.specifiers.drain(..) {
                match specifier {
                    ImportSpecifier::Named(named) => {
                        let imported = imported_name(&named).to_string();
                        let local = named.local.sym.to_string();

                        if record_compiled_import(compiled_imports, &imported, &local) {
                            if imported == "css" && css_alias.is_none() {
                                css_alias = Some(local);
                            }
                            continue;
                        }

                        remaining.push(ImportSpecifier::Named(named));
                    }
                    other => {
                        remaining.push(other);
                    }
                }
            }
        }

        import.specifiers = remaining;

        if let Some(alias) = css_alias {
            state.imported_compiled_imports.css = Some(alias);
        }

        !import.specifiers.is_empty()
    }
}

struct CssPropVisitor {
    meta: Metadata,
}

impl CssPropVisitor {
    fn new(meta: Metadata) -> Self {
        Self { meta }
    }
}

impl VisitMut for CssPropVisitor {
    noop_visit_mut_type!();

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        if matches!(expr, Expr::JSXElement(_)) {
            let meta = self
                .meta
                .with_parent_expr(Some(expr))
                .with_own_span(Some(expr.span()));
            visit_css_prop(expr, &meta);
        }
    }
}

struct StyledVisitor {
    meta: Metadata,
    display_names: Vec<DisplayNameInsertion>,
}

impl StyledVisitor {
    fn new(meta: Metadata) -> Self {
        Self {
            meta,
            display_names: Vec::new(),
        }
    }

    fn process_expr(&mut self, expr: &mut Expr, variable_name: Option<&str>) -> StyledVisitResult {
        expr.visit_mut_children_with(self);

        let meta = self
            .meta
            .with_parent_expr(Some(expr))
            .with_own_span(Some(expr.span()));
        visit_styled(expr, &meta, variable_name)
    }

    fn insert_display_names(&mut self, module: &mut Module) {
        if self.display_names.is_empty() {
            return;
        }

        let mut insertions = std::mem::take(&mut self.display_names);
        insert_display_names_in_items(&mut module.body, &mut insertions);
    }
}

impl VisitMut for StyledVisitor {
    noop_visit_mut_type!();

    fn visit_mut_var_declarator(&mut self, declarator: &mut VarDeclarator) {
        declarator.name.visit_mut_with(self);

        let Some(init) = declarator.init.as_mut() else {
            return;
        };

        let variable_name = match &declarator.name {
            Pat::Ident(binding) => Some(binding.id.sym.as_ref()),
            _ => None,
        };

        let result = self.process_expr(init, variable_name);

        if let Some(display_name) = result.display_name {
            self.display_names.push(DisplayNameInsertion {
                declarator_span: declarator.span,
                stmt: display_name,
            });
        }
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        self.process_expr(expr, None);
    }
}

struct ClassNamesVisitor {
    meta: Metadata,
}

impl ClassNamesVisitor {
    fn new(meta: Metadata) -> Self {
        Self { meta }
    }
}

impl VisitMut for ClassNamesVisitor {
    noop_visit_mut_type!();

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        if matches!(expr, Expr::JSXElement(_)) {
            let meta = self
                .meta
                .with_parent_expr(Some(expr))
                .with_own_span(Some(expr.span()));
            visit_class_names(expr, &meta);
        }
    }
}

struct CssMapVisitor {
    meta: Metadata,
}

impl CssMapVisitor {
    fn new(meta: Metadata) -> Self {
        Self { meta }
    }

    fn is_css_map_ident(&self, ident: &Ident) -> bool {
        self.meta
            .state()
            .compiled_imports
            .as_ref()
            .map(|imports| {
                imports
                    .css_map
                    .iter()
                    .any(|name| name == ident.sym.as_ref())
            })
            .unwrap_or(false)
    }
}

impl VisitMut for CssMapVisitor {
    noop_visit_mut_type!();

    fn visit_mut_var_declarator(&mut self, declarator: &mut VarDeclarator) {
        declarator.name.visit_mut_with(self);

        let Some(init) = declarator.init.as_mut() else {
            return;
        };

        let Some(binding_ident) = (match &declarator.name {
            Pat::Ident(binding) => Some(binding.id.clone()),
            _ => None,
        }) else {
            init.visit_mut_with(self);
            return;
        };

        match init.as_mut() {
            Expr::Call(call) => {
                let Some(callee_ident) = (match &call.callee {
                    swc_core::ecma::ast::Callee::Expr(expr) => match expr.as_ref() {
                        Expr::Ident(ident) => Some(ident.clone()),
                        _ => None,
                    },
                    _ => None,
                }) else {
                    call.visit_mut_children_with(self);
                    return;
                };

                if !self.is_css_map_ident(&callee_ident) {
                    call.visit_mut_children_with(self);
                    return;
                }

                let call_expr = call.clone();
                let init_expr = Expr::Call(call_expr.clone());
                let init_span = call_expr.span;

                let meta = self
                    .meta
                    .with_parent_expr(Some(&init_expr))
                    .with_parent_span(Some(declarator.span))
                    .with_own_span(Some(init_span));

                let object =
                    visit_css_map_path(CssMapUsage::Call(&call_expr), Some(&binding_ident), &meta);

                *init = Expr::Object(object).into();
            }
            Expr::TaggedTpl(tagged) => {
                if let Expr::Ident(tag_ident) = &*tagged.tag {
                    if self.is_css_map_ident(tag_ident) {
                        let tagged_tpl = tagged.clone();
                        let init_expr = Expr::TaggedTpl(tagged_tpl.clone());
                        let init_span = tagged_tpl.span;
                        let meta = self
                            .meta
                            .with_parent_expr(Some(&init_expr))
                            .with_parent_span(Some(declarator.span))
                            .with_own_span(Some(init_span));

                        let object = visit_css_map_path(
                            CssMapUsage::TaggedTemplate(&tagged_tpl),
                            Some(&binding_ident),
                            &meta,
                        );

                        *init = Expr::Object(object).into();
                        return;
                    }
                }

                tagged.visit_mut_children_with(self);
            }
            _ => {
                init.visit_mut_with(self);
            }
        }
    }
}

struct XcssVisitor {
    meta: Metadata,
}

impl XcssVisitor {
    fn new(meta: Metadata) -> Self {
        Self { meta }
    }
}

impl VisitMut for XcssVisitor {
    noop_visit_mut_type!();

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        if matches!(expr, Expr::JSXElement(_)) {
            let meta = self
                .meta
                .with_parent_expr(Some(expr))
                .with_own_span(Some(expr.span()));
            visit_xcss_prop(expr, &meta);
        }
    }
}

#[derive(Clone)]
struct DisplayNameInsertion {
    declarator_span: Span,
    stmt: Stmt,
}

fn insert_display_names_in_items(
    items: &mut Vec<ModuleItem>,
    insertions: &mut Vec<DisplayNameInsertion>,
) {
    let mut index = 0;

    while index < items.len() {
        let extras = match &mut items[index] {
            ModuleItem::Stmt(stmt) => insert_display_names_in_stmt(stmt, insertions)
                .into_iter()
                .map(ModuleItem::Stmt)
                .collect::<Vec<_>>(),
            ModuleItem::ModuleDecl(decl) => insert_display_names_in_module_decl(decl, insertions),
        };

        if !extras.is_empty() {
            let count = extras.len();
            items.splice(index + 1..index + 1, extras);
            index += 1 + count;
        } else {
            index += 1;
        }
    }
}

fn insert_display_names_in_module_decl(
    decl: &mut ModuleDecl,
    insertions: &mut Vec<DisplayNameInsertion>,
) -> Vec<ModuleItem> {
    match decl {
        ModuleDecl::ExportDecl(export_decl) => match &mut export_decl.decl {
            Decl::Var(var_decl) => collect_display_names_from_var_decl(var_decl, insertions)
                .into_iter()
                .map(ModuleItem::Stmt)
                .collect(),
            _ => Vec::new(),
        },
        ModuleDecl::ExportDefaultDecl(default_decl) => {
            if let DefaultDecl::Fn(function) = &mut default_decl.decl {
                if let Some(body) = function.function.body.as_mut() {
                    insert_display_names_in_stmts(&mut body.stmts, insertions);
                }
            }

            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn insert_display_names_in_stmts(
    stmts: &mut Vec<Stmt>,
    insertions: &mut Vec<DisplayNameInsertion>,
) {
    let mut index = 0;

    while index < stmts.len() {
        let extras = insert_display_names_in_stmt(&mut stmts[index], insertions);

        if !extras.is_empty() {
            let count = extras.len();
            stmts.splice(index + 1..index + 1, extras);
            index += 1 + count;
        } else {
            index += 1;
        }
    }
}

fn insert_display_names_in_stmt(
    stmt: &mut Stmt,
    insertions: &mut Vec<DisplayNameInsertion>,
) -> Vec<Stmt> {
    match stmt {
        Stmt::Decl(Decl::Var(var_decl)) => {
            collect_display_names_from_var_decl(var_decl, insertions)
        }
        Stmt::Block(block) => {
            insert_display_names_in_stmts(&mut block.stmts, insertions);
            Vec::new()
        }
        Stmt::If(if_stmt) => {
            let cons_stmts = ensure_block(&mut if_stmt.cons);
            insert_display_names_in_stmts(cons_stmts, insertions);

            if let Some(alt) = if_stmt.alt.as_mut() {
                let alt_stmts = ensure_block(alt);
                insert_display_names_in_stmts(alt_stmts, insertions);
            }

            Vec::new()
        }
        Stmt::While(while_stmt) => {
            let body = ensure_block(&mut while_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        Stmt::DoWhile(do_while_stmt) => {
            let body = ensure_block(&mut do_while_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        Stmt::For(for_stmt) => {
            let body = ensure_block(&mut for_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        Stmt::ForIn(for_in_stmt) => {
            let body = ensure_block(&mut for_in_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        Stmt::ForOf(for_of_stmt) => {
            let body = ensure_block(&mut for_of_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        Stmt::Switch(switch_stmt) => {
            for case in &mut switch_stmt.cases {
                insert_display_names_in_stmts(&mut case.cons, insertions);
            }
            Vec::new()
        }
        Stmt::Try(try_stmt) => {
            insert_display_names_in_stmts(&mut try_stmt.block.stmts, insertions);

            if let Some(handler) = try_stmt.handler.as_mut() {
                insert_display_names_in_stmts(&mut handler.body.stmts, insertions);
            }

            if let Some(finalizer) = try_stmt.finalizer.as_mut() {
                insert_display_names_in_stmts(&mut finalizer.stmts, insertions);
            }

            Vec::new()
        }
        Stmt::Labeled(labeled_stmt) => {
            let body = ensure_block(&mut labeled_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        Stmt::With(with_stmt) => {
            let body = ensure_block(&mut with_stmt.body);
            insert_display_names_in_stmts(body, insertions);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn collect_display_names_from_var_decl(
    var_decl: &VarDecl,
    insertions: &mut Vec<DisplayNameInsertion>,
) -> Vec<Stmt> {
    let mut extras = Vec::new();

    for declarator in &var_decl.decls {
        if let Some(index) = insertions
            .iter()
            .position(|insertion| insertion.declarator_span == declarator.span)
        {
            let insertion = insertions.remove(index);
            extras.push(insertion.stmt);
        }
    }

    extras
}

fn ensure_block(stmt: &mut Stmt) -> &mut Vec<Stmt> {
    match stmt {
        Stmt::Block(block) => &mut block.stmts,
        _ => {
            let span = stmt.span();
            let original = std::mem::replace(stmt, Stmt::Empty(EmptyStmt { span }));
            *stmt = Stmt::Block(BlockStmt {
                span,
                ctxt: Default::default(),
                stmts: vec![original],
            });

            match stmt {
                Stmt::Block(block) => &mut block.stmts,
                _ => unreachable!(),
            }
        }
    }
}
