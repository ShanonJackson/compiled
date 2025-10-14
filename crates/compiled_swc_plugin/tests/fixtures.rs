use std::fs;
use std::path::{Path, PathBuf};

use compiled_swc_plugin::{take_latest_artifacts, transform};
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::ast::Program;
use swc_core::ecma::codegen::{text_writer::JsWriter, Emitter};
use swc_core::ecma::parser::{
    lexer::Lexer, EsConfig, EsVersion, Parser, StringInput, Syntax, TsConfig,
};
use swc_core::plugin::metadata::TransformPluginMetadataContext;
use swc_core::plugin::proxies::TransformPluginProgramMetadata;

fn syntax_for_filename(path: &Path) -> Syntax {
    let name = path.to_string_lossy();
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

fn parse_program(path: &Path, source: &str) -> Program {
    use std::sync::Arc;

    let cm: Arc<SourceMap> = Default::default();
    let filename = FileName::Real(path.to_path_buf());
    let fm = cm.new_source_file(filename, source.into());
    let lexer = Lexer::new(
        syntax_for_filename(path),
        EsVersion::Es2022,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    Program::Module(parser.parse_module().expect("failed to parse module"))
}

fn emit_program(program: &Program) -> String {
    use std::sync::Arc;

    let cm: Arc<SourceMap> = Default::default();
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
        emitter
            .emit_program(program)
            .expect("failed to emit program");
    }
    String::from_utf8(buf).expect("emitted JS should be utf8")
}

fn run_transform(input_path: &Path, source: &str) -> String {
    let program = parse_program(input_path, source);
    let mut metadata = TransformPluginProgramMetadata::default();
    metadata.context = Some(TransformPluginMetadataContext {
        filename: Some(input_path.to_string_lossy().to_string()),
        ..Default::default()
    });
    metadata.transform_plugin_config = Some("{\"extract\":false}".to_string());
    let transformed = transform(program, metadata);
    // drain artifacts so subsequent fixtures start from a clean state
    let _ = take_latest_artifacts();
    emit_program(&transformed)
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[test]
fn fixture_outputs_match() {
    let dir = fixtures_dir();
    let entries = fs::read_dir(&dir).expect("fixtures directory should exist");
    for entry in entries {
        let entry = entry.expect("failed to read fixture entry");
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let fixture_path = entry.path();
        let input_path = fixture_path.join("in.jsx");
        let expected_path = fixture_path.join("out.js");
        if !input_path.exists() || !expected_path.exists() {
            panic!(
                "fixture {:?} is missing required files (in.jsx/out.js)",
                fixture_path
            );
        }
        let input = fs::read_to_string(&input_path).expect("failed to read fixture input");
        let expected = fs::read_to_string(&expected_path).expect("failed to read fixture output");
        let actual = run_transform(&input_path, &input);
        assert_eq!(
            normalize(&expected),
            normalize(&actual),
            "fixture {:?} did not match",
            fixture_path.file_name().unwrap()
        );
    }
}

fn normalize(output: &str) -> String {
    output.replace("\r\n", "\n").trim().to_string()
}
