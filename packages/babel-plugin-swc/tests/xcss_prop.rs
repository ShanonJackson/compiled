use std::panic;

use compiled_babel_plugin_swc::options::PluginConfig;
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use compiled_babel_plugin_swc::xcss_prop;
use swc_core::common::sync::Lrc;
use swc_core::common::SourceMap;
use swc_core::ecma::ast::Program;
use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};

#[path = "utils.rs"]
mod utils;
use utils::{panic_message, parse_module_with_metadata, run_transform};

#[test]
fn inline_object_must_be_static() {
    let source = "<Component xcss={{ color: value }} />;";
    let result = panic::catch_unwind(|| run_transform(source));

    assert!(result.is_err(), "expected xcss transform to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains(xcss_prop::STATIC_OBJECT_ERROR),
        "unexpected panic message: {message}"
    );
}

#[test]
fn respects_process_xcss_flag() {
    let source = "<Component xcss={{ color: 'red' }} />;";
    let parsed = parse_module_with_metadata(source);

    let metadata = TransformMetadata {
        filename: Some(std::path::PathBuf::from("inline.tsx")),
        source_file_name: None,
        root_dir: None,
        caller: None,
        source_map: Some(parsed.source_map.clone()),
        comments: Some(parsed.comments.clone()),
    };

    let mut config = PluginConfig::default();
    config.process_xcss = false;

    let mut transform = Transform::new(config, metadata);
    let result = transform.apply(Program::Module(parsed.module.clone()));

    let emitted = emit_program(&result.program);
    assert!(
        emitted.contains("<Component xcss={{"),
        "expected xcss transform to be skipped when disabled"
    );
}

fn emit_program(program: &Program) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: CodegenConfig::default().with_target(swc_core::ecma::ast::EsVersion::Es2022),
            comments: None,
            cm,
            wr: writer,
        };
        emitter.emit_program(program).expect("emit program");
    }

    String::from_utf8(buf).expect("utf8 output")
}
