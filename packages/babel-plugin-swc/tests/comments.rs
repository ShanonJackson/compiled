use std::path::PathBuf;

use compiled_babel_plugin_swc::options::PluginConfig;
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use swc_core::ecma::ast::{EsVersion, Program};

#[path = "utils.rs"]
mod utils;
use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::sync::Lrc;
use swc_core::common::SourceMap;
use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};
use utils::parse_module_with_metadata;

#[test]
fn preserves_leading_comment_for_runtime_imports() {
    let source = r#"// license comment
import { css } from '@compiled/react';
export const Component = () => <div css={{ color: 'red' }} />;
"#;

    let parsed = parse_module_with_metadata(source);
    let mut config = PluginConfig::default();
    config.extract = false;

    let metadata = TransformMetadata {
        filename: Some(PathBuf::from("inline.tsx")),
        source_file_name: None,
        root_dir: None,
        caller: None,
        source_map: Some(parsed.source_map.clone()),
        comments: Some(parsed.comments.clone()),
    };

    let mut transform = Transform::new(config, metadata);
    let result = transform.apply(Program::Module(parsed.module));
    let comments_ref = transform
        .state()
        .metadata
        .comments
        .as_ref()
        .expect("comments to exist")
        .clone();
    let code = emit_program_with_comments(&result.program, comments_ref);

    assert!(
        code.starts_with("// license comment\n"),
        "leading comment was not preserved: {code}"
    );
    assert!(
        code.contains("@compiled/react/runtime"),
        "runtime import should be injected"
    );
}

#[test]
fn preserves_leading_comment_for_stylesheet_requires() {
    let source = r#"// header comment
import { css } from '@compiled/react';
export const styles = css({ color: 'red' });
"#;

    let parsed = parse_module_with_metadata(source);
    let mut config = PluginConfig::default();
    config.extract = true;
    config.style_sheet_path = Some("style.css".into());

    let metadata = TransformMetadata {
        filename: Some(PathBuf::from("inline.tsx")),
        source_file_name: None,
        root_dir: None,
        caller: None,
        source_map: Some(parsed.source_map.clone()),
        comments: Some(parsed.comments.clone()),
    };

    let mut transform = Transform::new(config, metadata);
    let result = transform.apply(Program::Module(parsed.module));
    let comments_ref = transform
        .state()
        .metadata
        .comments
        .as_ref()
        .expect("comments to exist")
        .clone();
    let code = emit_program_with_comments(&result.program, comments_ref);

    assert!(
        code.starts_with("// header comment\n"),
        "leading comment was not preserved: {code}"
    );
    assert!(
        code.contains("require(\"style.css?style="),
        "stylesheet require should be emitted"
    );
}

fn emit_program_with_comments(program: &Program, comments: Lrc<SingleThreadedComments>) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buf = Vec::new();

    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: CodegenConfig::default().with_target(EsVersion::Es2022),
            comments: Some(&comments),
            cm,
            wr: writer,
        };

        emitter.emit_program(program).expect("emit program");
    }

    String::from_utf8(buf).expect("utf8 program")
}
