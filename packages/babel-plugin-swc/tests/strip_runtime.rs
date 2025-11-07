use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

use compiled_babel_plugin_swc::options::{ExtractStylesToDirectory, PluginConfig};
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::ast::{EsVersion, Program};
use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};
use swc_core::ecma::parser::{parse_file_as_module, EsSyntax, Syntax};

#[path = "utils.rs"]
mod utils;
use utils::panic_message;

fn transform_source(source: &str, config: PluginConfig) -> (String, Vec<String>) {
    let metadata = TransformMetadata {
        filename: Some(PathBuf::from("/virtual/app.tsx")),
        source_file_name: None,
        root_dir: Some(PathBuf::from("/virtual")),
        caller: None,
        source_map: None,
        comments: None,
    };

    transform_with_metadata(source, config, metadata)
}

fn transform_with_metadata(
    source: &str,
    config: PluginConfig,
    mut metadata: TransformMetadata,
) -> (String, Vec<String>) {
    let cm: Lrc<SourceMap> = Default::default();
    let filename = metadata
        .filename
        .clone()
        .expect("metadata.filename must be provided");
    let fm = cm.new_source_file(FileName::Real(filename.clone()).into(), source.to_owned());
    let syntax = Syntax::Es(EsSyntax {
        jsx: true,
        ..Default::default()
    });
    let mut errors = Vec::new();
    let comments = SingleThreadedComments::default();
    let module = parse_file_as_module(&fm, syntax, EsVersion::Es2022, Some(&comments), &mut errors)
        .expect("parse module");
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    metadata.source_map = Some(cm.clone());
    metadata.comments = Some(Lrc::new(comments));

    let mut transform = Transform::new(config, metadata);
    let result = transform.apply(Program::Module(module));

    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: CodegenConfig::default().with_target(EsVersion::Es2022),
            comments: None,
            cm,
            wr: writer,
        };
        emitter.emit_program(&result.program).expect("emit program");
    }

    let code = String::from_utf8(buf).expect("utf8 output");
    (code, result.style_rules)
}

#[test]
fn inserts_style_sheet_requires_when_configured() {
    let source = r#"
        import '@compiled/react';
        export const Component = () => <div css={{ color: 'red' }}>hello</div>;
    "#;

    let mut config = PluginConfig::default();
    config.extract = true;
    config.style_sheet_path = Some("./style-loader.js".to_string());

    let (code, style_rules) = transform_source(source, config);

    assert!(code.contains("require(\"./style-loader.js?style=._syaz5scu%7Bcolor%3Ared%7D\");"));
    assert!(code.contains("import '@compiled/react';"));
    assert_eq!(style_rules, vec!["._syaz5scu{color:red}".to_string()]);
}

#[test]
fn skips_style_sheet_requires_when_excluded() {
    let source = r#"
        import '@compiled/react';
        export const Component = () => <div css={{ color: 'red' }}>hello</div>;
    "#;

    let mut config = PluginConfig::default();
    config.extract = true;
    config.style_sheet_path = Some("./style-loader.js".to_string());
    config.compiled_require_exclude = true;

    let (code, _) = transform_source(source, config);

    assert!(!code.contains("style-loader.js?style"));
}

#[test]
fn extracts_styles_to_directory_and_inserts_import() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("create src directory");

    let source = r#"
        import '@compiled/react';

        export const Component = () => (
          <div css={{ fontSize: 12, color: 'blue' }}>
            hello world
          </div>
        );
    "#;

    let mut config = PluginConfig::default();
    config.extract = true;
    config.extract_styles_to_directory = Some(ExtractStylesToDirectory {
        source: "src/".to_string(),
        dest: "dist/".to_string(),
    });

    let metadata = TransformMetadata {
        filename: Some(src_dir.join("app.tsx")),
        source_file_name: Some(PathBuf::from("src/app.tsx")),
        root_dir: Some(dir.path().to_path_buf()),
        caller: None,
        source_map: None,
        comments: None,
    };

    let (code, style_rules) = transform_with_metadata(source, config, metadata);

    assert!(
        code.contains("import \"./app.compiled.css\";")
            || code.contains("import './app.compiled.css';"),
        "expected compiled stylesheet import to be present in output:\n{code}"
    );

    let css_path = dir.path().join("dist").join("app.compiled.css");
    let contents = fs::read_to_string(&css_path).expect("css file written");
    assert_eq!(
        contents,
        "._1wyb1fwx{font-size:12px}\n._syaz13q2{color:blue}"
    );
    assert_eq!(
        style_rules,
        vec![
            "._1wyb1fwx{font-size:12px}".to_string(),
            "._syaz13q2{color:blue}".to_string(),
        ]
    );
}

#[test]
fn errors_when_source_directory_is_missing() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("create src directory");

    let source = r#"
        import '@compiled/react';
        export const Component = () => <div css={{ color: 'red' }}>hello</div>;
    "#;

    let mut config = PluginConfig::default();
    config.extract = true;
    config.extract_styles_to_directory = Some(ExtractStylesToDirectory {
        source: "not-existing-src/".to_string(),
        dest: "dist/".to_string(),
    });

    let metadata = TransformMetadata {
        filename: Some(src_dir.join("app.tsx")),
        source_file_name: Some(PathBuf::from("src/app.tsx")),
        root_dir: Some(dir.path().to_path_buf()),
        caller: None,
        source_map: None,
        comments: None,
    };

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        transform_with_metadata(source, config, metadata)
    }));
    assert!(result.is_err(), "expected transform to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("Source directory 'not-existing-src/' was not found"),
        "unexpected panic message: {message}"
    );
}
