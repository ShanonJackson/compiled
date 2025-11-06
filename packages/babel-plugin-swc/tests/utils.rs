use std::any::Any;
use std::fs;
use std::path::{Path, PathBuf};

use compiled_babel_plugin_swc::options::PluginConfig;
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::ast::{EsVersion, Module, Program};
use swc_core::ecma::parser::{parse_file_as_module, EsSyntax, Syntax};

#[derive(Clone)]
pub struct ParsedModule {
    pub module: Module,
    pub source_map: Lrc<SourceMap>,
    pub comments: Lrc<SingleThreadedComments>,
}

pub fn parse_module_with_metadata(source: &str) -> ParsedModule {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("inline.tsx".into()).into(),
        source.to_owned(),
    );
    let syntax = Syntax::Es(EsSyntax {
        jsx: true,
        ..Default::default()
    });
    let mut errors = Vec::new();
    let comments = SingleThreadedComments::default();
    let module = parse_file_as_module(&fm, syntax, EsVersion::Es2022, Some(&comments), &mut errors)
        .expect("parse inline module");

    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    ParsedModule {
        module,
        source_map: cm,
        comments: Lrc::new(comments),
    }
}

#[allow(dead_code)]
pub fn parse_module_from_path(path: &Path) -> ParsedModule {
    let source = fs::read_to_string(path).expect("read module from path");
    parse_module_from_source(path, &source)
}

pub fn parse_module_from_source(path: &Path, source: &str) -> ParsedModule {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), source.to_owned());
    let syntax = Syntax::Es(EsSyntax {
        jsx: true,
        ..Default::default()
    });
    let mut errors = Vec::new();
    let comments = SingleThreadedComments::default();
    let module = parse_file_as_module(&fm, syntax, EsVersion::Es2022, Some(&comments), &mut errors)
        .expect("parse module from path");

    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    ParsedModule {
        module,
        source_map: cm,
        comments: Lrc::new(comments),
    }
}

#[allow(dead_code)]
pub fn parse_module(source: &str) -> Module {
    parse_module_with_metadata(source).module
}

#[allow(dead_code)]
pub fn run_transform(source: &str) {
    let parsed = parse_module_with_metadata(source);
    let mut config = PluginConfig::default();
    config.extract = false;

    let metadata = TransformMetadata {
        filename: Some(PathBuf::from("inline.tsx")),
        root_dir: None,
        caller: None,
        source_map: Some(parsed.source_map.clone()),
        comments: Some(parsed.comments.clone()),
    };

    let mut transform = Transform::new(config, metadata);
    transform.apply(Program::Module(parsed.module));
}

#[allow(dead_code)]
pub fn panic_message(error: Box<dyn Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<&'static str>() {
        message.to_string()
    } else if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}
