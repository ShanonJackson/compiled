use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use indexmap::IndexSet;
use oxc_resolver::{ResolveOptions, Resolver};
use serde_json::Value;
use swc_core::common::comments::{Comment, SingleThreadedComments};
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::ast::{EsVersion, Expr, Program, Prop, PropName, PropOrSpread};
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax, TsSyntax};

use crate::constants::DEFAULT_CODE_EXTENSIONS;
use crate::types::{CachedModule, Metadata, TransformFile, TransformFileOptions, TransformState};
use crate::utils_create_result_pair::{create_result_pair, ResultPair};
use crate::utils_module_scope;
use crate::utils_traversers::{
    get_default_export, get_named_export, set_imported_compiled_imports,
};
use crate::utils_traversers_types::TraverserResult;
use crate::utils_types::{
    BindingPathKind, EvaluateExpression, ImportBindingKind, PartialBindingWithMeta,
};

fn ensure_module_resolver(state: &mut TransformState) {
    if state.module_resolver.is_some() {
        return;
    }

    let mut options = ResolveOptions::default();
    let extensions = state.opts.extensions.clone().unwrap_or_else(|| {
        DEFAULT_CODE_EXTENSIONS
            .iter()
            .map(|ext| ext.to_string())
            .collect()
    });
    options.extensions = extensions;

    state.module_resolver = Some(Resolver::new(options));
}

fn resolve_request(state: &mut TransformState, filename: &str, request: &str) -> Option<String> {
    let resolver = state.module_resolver.as_ref()?;
    let base = Path::new(filename)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(PathBuf::new);

    resolver
        .resolve(&base, request)
        .ok()
        .map(|resolution| resolution.into_path_buf().to_string_lossy().into_owned())
}

fn parse_program(path: &str, code: &str) -> Option<(Program, Lrc<SourceMap>, Vec<Comment>)> {
    let source_map: Lrc<SourceMap> = Default::default();
    let file_name: FileName = FileName::Real(PathBuf::from(path).into());
    let file = source_map.new_source_file(file_name.into(), code.into());
    let comments = SingleThreadedComments::default();

    let syntax = if path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with(".mts")
        || path.ends_with(".cts")
    {
        Syntax::Typescript(TsSyntax {
            tsx: path.ends_with("x"),
            ..Default::default()
        })
    } else {
        Syntax::Es(EsSyntax {
            jsx: path.ends_with("x"),
            ..Default::default()
        })
    };

    let lexer = Lexer::new(
        syntax,
        EsVersion::Es2022,
        StringInput::from(&*file),
        Some(&comments),
    );

    let mut parser = Parser::new_from(lexer);
    let program = parser.parse_program().ok()?;

    if !parser.take_errors().is_empty() {
        return None;
    }

    let mut collected: Vec<Comment> = Vec::new();
    let (leading, trailing) = comments.take_all();

    {
        let mut leading = leading.borrow_mut();
        for (_, mut list) in leading.drain() {
            collected.append(&mut list);
        }
    }

    {
        let mut trailing = trailing.borrow_mut();
        for (_, mut list) in trailing.drain() {
            collected.append(&mut list);
        }
    }

    Some((program, source_map, collected))
}

fn load_or_parse_module(meta: &Metadata, source: &str) -> Option<CachedModule> {
    let (module_path, code, options, resolver_clone, cwd, root) = {
        let mut state = meta.state_mut();
        let filename = state.filename.clone()?;

        ensure_module_resolver(&mut state);
        let resolved = resolve_request(&mut state, &filename, source)?;

        if !state
            .opts
            .extensions
            .clone()
            .unwrap_or_else(|| {
                DEFAULT_CODE_EXTENSIONS
                    .iter()
                    .map(|ext| ext.to_string())
                    .collect()
            })
            .iter()
            .any(|ext| resolved.ends_with(ext))
        {
            return None;
        }

        if !state.included_files.contains(&resolved) {
            state.included_files.push(resolved.clone());
        }

        if let Some(cached) = state.module_cache.get(&resolved) {
            return Some(cached.clone());
        }

        let resolver_clone = state
            .module_resolver
            .as_ref()
            .map(|resolver| resolver.clone_with_options(resolver.options().clone()));
        let options = state.opts.clone();
        let cwd = state.cwd.clone();
        let root = state.root.clone();
        let code_value = state.cache.load(Some("read-file"), &resolved, || {
            Value::String(fs::read_to_string(&resolved).expect("module should read"))
        });
        let code = code_value
            .as_str()
            .map(|value| value.to_string())
            .unwrap_or_else(|| fs::read_to_string(&resolved).expect("module should read"));

        (resolved, code, options, resolver_clone, cwd, root)
    };

    let (program, source_map, comments) = parse_program(&module_path, &code)?;

    let transform_file = TransformFile::with_options(
        source_map.clone(),
        comments,
        TransformFileOptions {
            filename: Some(module_path.clone()),
            cwd: Some(cwd.clone()),
            root: Some(root.clone()),
            loc_filename: None,
        },
    );

    let shared_state = Rc::new(RefCell::new(TransformState::new(transform_file, options)));

    {
        let mut module_state = shared_state.borrow_mut();
        module_state.module_resolver = resolver_clone;
    }

    if let Program::Module(module) = &program {
        utils_module_scope::populate_module_scope(&shared_state, module);
    }

    {
        let mut module_state = shared_state.borrow_mut();
        set_imported_compiled_imports(&program, &mut module_state);
    }

    let cached = CachedModule {
        program: program.clone(),
        state: shared_state.clone(),
    };

    {
        let mut state = meta.state_mut();
        state.module_cache.insert(module_path, cached.clone());
    }

    Some(cached)
}

fn get_scoped_binding(reference_name: &str, meta: &Metadata) -> Option<PartialBindingWithMeta> {
    if let Some(scope) = meta.own_scope() {
        if let Some(binding) = scope.borrow().get(reference_name) {
            return Some(binding.clone());
        }
    }

    meta.parent_scope().borrow().get(reference_name).cloned()
}

fn extract_property_value(
    expr: &Expr,
    meta: Metadata,
    segment: &str,
    visited: &mut IndexSet<String>,
    evaluate_expression: EvaluateExpression,
) -> Option<ResultPair> {
    match expr {
        Expr::Object(object) => object.props.iter().find_map(|prop| match prop {
            PropOrSpread::Prop(prop) => match prop.as_ref() {
                Prop::KeyValue(kv) => {
                    let key_matches = match &kv.key {
                        PropName::Ident(ident) => ident.sym.as_ref() == segment,
                        PropName::Str(value) => value.value.as_ref() == segment,
                        PropName::Num(value) => value.value.to_string() == segment,
                        PropName::BigInt(value) => value.value.to_string() == segment,
                        PropName::Computed(_) => false,
                    };

                    if key_matches {
                        Some(create_result_pair(*kv.value.clone(), meta.clone()))
                    } else {
                        None
                    }
                }
                Prop::Assign(assign) => {
                    if assign.key.sym.as_ref() == segment {
                        Some(create_result_pair(*assign.value.clone(), meta.clone()))
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }),
        Expr::Ident(ident) => {
            if !visited.insert(ident.sym.to_string()) {
                return None;
            }

            let binding = resolve_binding(ident.sym.as_ref(), meta.clone(), evaluate_expression)?;

            if let Some(node) = &binding.node {
                extract_property_value(
                    node,
                    binding.meta.clone(),
                    segment,
                    visited,
                    evaluate_expression,
                )
            } else {
                Some(create_result_pair(expr.clone(), binding.meta.clone()))
            }
        }
        Expr::Paren(paren) => {
            extract_property_value(&paren.expr, meta, segment, visited, evaluate_expression)
        }
        Expr::Member(_member) => {
            let pair = evaluate_expression(expr, meta.clone());
            if pair.value == *expr {
                None
            } else {
                extract_property_value(
                    &pair.value,
                    pair.meta,
                    segment,
                    visited,
                    evaluate_expression,
                )
            }
        }
        _ => {
            let pair = evaluate_expression(expr, meta.clone());
            if pair.value == *expr {
                None
            } else {
                extract_property_value(
                    &pair.value,
                    pair.meta,
                    segment,
                    visited,
                    evaluate_expression,
                )
            }
        }
    }
}

fn resolve_variable_binding(
    binding: PartialBindingWithMeta,
    path: &[String],
    default: &Option<Expr>,
    evaluate_expression: EvaluateExpression,
) -> Option<PartialBindingWithMeta> {
    let Some(base) = binding.node.as_ref() else {
        return Some(binding);
    };

    let mut pair = evaluate_expression(base, binding.meta.clone());
    let mut visited = IndexSet::new();

    for (index, segment) in path.iter().enumerate() {
        match extract_property_value(
            &pair.value,
            pair.meta.clone(),
            segment,
            &mut visited,
            evaluate_expression,
        ) {
            Some(next_pair) => {
                pair = next_pair;
            }
            None => {
                if index + 1 == path.len() {
                    if let Some(default_expr) = default {
                        pair = evaluate_expression(default_expr, binding.meta.clone());
                        break;
                    }
                }

                return None;
            }
        }
    }

    let mut resolved = binding.clone();
    resolved.node = Some(pair.value);
    resolved.meta = pair.meta;
    Some(resolved)
}

fn resolve_import_binding(
    binding: PartialBindingWithMeta,
    source: &str,
    kind: &ImportBindingKind,
    meta: Metadata,
) -> Option<PartialBindingWithMeta> {
    if source.starts_with("@compiled/") {
        return None;
    }

    let cached = load_or_parse_module(&meta, source)?;

    match kind {
        ImportBindingKind::Namespace => Some(binding),
        ImportBindingKind::Default => {
            let result = get_default_export(&cached.program)?;
            Some(build_import_binding(binding, result, cached.state))
        }
        ImportBindingKind::Named(name) => {
            let result = get_named_export(&cached.program, name)?;
            Some(build_import_binding(binding, result, cached.state))
        }
    }
}

fn build_import_binding(
    mut binding: PartialBindingWithMeta,
    traversed: TraverserResult<Expr>,
    state: Rc<RefCell<TransformState>>,
) -> PartialBindingWithMeta {
    let metadata = Metadata::new(state).with_parent_span(Some(traversed.span));
    binding.node = Some(traversed.node);
    binding.meta = metadata;
    binding
}

pub fn resolve_binding(
    reference_name: &str,
    meta: Metadata,
    evaluate_expression: EvaluateExpression,
) -> Option<PartialBindingWithMeta> {
    let binding = get_scoped_binding(reference_name, &meta)?;
    let path_kind = binding.path.clone().map(|path| path.kind);

    match path_kind {
        Some(BindingPathKind::Variable { path, default }) => {
            resolve_variable_binding(binding, &path, &default, evaluate_expression)
        }
        Some(BindingPathKind::Import { source, kind }) => {
            resolve_import_binding(binding, &source, &kind, meta)
        }
        _ => Some(binding),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_binding;
    use crate::types::{
        Metadata, PluginOptions, TransformFile, TransformFileOptions, TransformState,
    };
    use crate::utils_create_result_pair::create_result_pair;
    use crate::utils_types::{
        BindingPath, BindingSource, EvaluateExpression, ImportBindingKind, PartialBindingWithMeta,
    };
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;
    use swc_core::common::sync::Lrc;
    use swc_core::common::{FileName, SourceMap, DUMMY_SP};
    use swc_core::ecma::ast::{Expr, Lit, Str};
    use swc_ecma_parser::lexer::Lexer;
    use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax};
    use tempfile::tempdir;

    fn create_metadata() -> Metadata {
        let cm: Lrc<SourceMap> = Default::default();
        let file = TransformFile::new(cm, Vec::new());
        let state = Rc::new(RefCell::new(TransformState::new(
            file,
            PluginOptions::default(),
        )));

        Metadata::new(state)
    }

    fn string_literal(value: &str) -> Expr {
        Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: value.into(),
            raw: None,
        }))
    }

    fn identity_evaluate(
        expr: &Expr,
        meta: Metadata,
    ) -> crate::utils_create_result_pair::ResultPair {
        create_result_pair(expr.clone(), meta)
    }

    fn parse_expression(code: &str) -> Expr {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom("expr.tsx".into()).into(), code.into());
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
        *parser.parse_expr().expect("parse expression")
    }

    #[test]
    fn resolves_binding_from_parent_scope() {
        let meta = create_metadata();
        let binding_expr = string_literal("blue");
        let binding = PartialBindingWithMeta::new(
            Some(binding_expr.clone()),
            Some(BindingPath::new(Some(DUMMY_SP))),
            true,
            meta.clone(),
            BindingSource::Module,
        );

        meta.insert_parent_binding("color", binding.clone());

        let result = resolve_binding(
            "color",
            meta.clone(),
            identity_evaluate as EvaluateExpression,
        )
        .expect("binding");

        assert!(result.constant);
        assert_eq!(result.node, Some(binding_expr));
        assert_eq!(result.source, BindingSource::Module);
    }

    #[test]
    fn prefers_binding_from_own_scope() {
        let meta = create_metadata();
        let parent_binding = PartialBindingWithMeta::new(
            Some(string_literal("parent")),
            None,
            true,
            meta.clone(),
            BindingSource::Module,
        );
        meta.insert_parent_binding("value", parent_binding);

        let own_scope = meta.allocate_own_scope();
        let scoped_meta = meta.with_own_scope(Some(own_scope.clone()));
        let own_binding = PartialBindingWithMeta::new(
            Some(string_literal("own")),
            None,
            true,
            scoped_meta.clone(),
            BindingSource::Module,
        );
        own_scope
            .borrow_mut()
            .insert("value".into(), own_binding.clone());

        let result = resolve_binding(
            "value",
            scoped_meta,
            identity_evaluate as EvaluateExpression,
        )
        .expect("binding");

        assert_eq!(result.node, Some(string_literal("own")));
    }

    #[test]
    fn resolves_destructured_binding() {
        let meta = create_metadata();
        let object_expr = parse_expression("({ color: 'red' })");
        let binding = PartialBindingWithMeta::new(
            Some(object_expr),
            Some(BindingPath::variable(
                Some(DUMMY_SP),
                vec!["color".into()],
                None,
            )),
            true,
            meta.clone(),
            BindingSource::Module,
        );
        meta.insert_parent_binding("color", binding);

        let result = resolve_binding("color", meta, identity_evaluate as EvaluateExpression)
            .expect("binding");

        let Expr::Lit(Lit::Str(str_lit)) = result.node.expect("resolved literal") else {
            panic!("expected string literal");
        };

        assert_eq!(str_lit.value, "red");
    }

    #[test]
    fn resolves_named_import_binding() {
        let dir = tempdir().expect("temp directory");
        let entry_path = dir.path().join("entry.tsx");
        fs::write(&entry_path, "").expect("write entry");

        let module_path = dir.path().join("colors.ts");
        fs::write(&module_path, "export const blue = 'blue';").expect("write module");

        let cm: Lrc<SourceMap> = Default::default();
        let file = TransformFile::with_options(
            cm,
            Vec::new(),
            TransformFileOptions {
                filename: Some(entry_path.to_string_lossy().into_owned()),
                cwd: Some(dir.path().to_path_buf()),
                root: Some(dir.path().to_path_buf()),
                loc_filename: None,
            },
        );

        let state = Rc::new(RefCell::new(TransformState::new(
            file,
            PluginOptions::default(),
        )));
        let meta = Metadata::new(state.clone());

        let binding = PartialBindingWithMeta::new(
            None,
            Some(BindingPath::import(
                Some(DUMMY_SP),
                "./colors".into(),
                ImportBindingKind::Named("blue".into()),
            )),
            true,
            meta.clone(),
            BindingSource::Import,
        );
        meta.insert_parent_binding("blue", binding);

        let result = resolve_binding("blue", meta, identity_evaluate as EvaluateExpression)
            .expect("binding");

        let Expr::Lit(Lit::Str(str_lit)) = result.node.expect("resolved literal") else {
            panic!("expected string literal");
        };

        assert_eq!(str_lit.value, "blue");
    }
}
