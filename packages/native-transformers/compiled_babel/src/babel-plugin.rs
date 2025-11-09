use std::cell::RefCell;
use std::rc::Rc;

use std::path::{Path, PathBuf};

use swc_core::common::Spanned;
use swc_core::ecma::ast::{
    Expr, ImportDecl, ImportNamedSpecifier, ImportSpecifier, Module, ModuleDecl,
    ModuleExportName, ModuleItem, Program,
};
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use crate::css_prop::visit_css_prop;
use crate::types::{
    CompiledImports, Metadata, PluginOptions, SharedTransformState, TransformFile,
    TransformMetadata, TransformState,
};
use crate::utils_append_runtime_imports::append_runtime_imports;

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

        let should_process_css_prop = {
            let state = self.state.borrow();
            state
                .compiled_imports
                .as_ref()
                .map(has_active_compiled_imports)
                .unwrap_or(false)
        };

        if should_process_css_prop {
            let metadata = Metadata::new(self.state());
            let mut visitor = CssPropVisitor::new(metadata);
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
