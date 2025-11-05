use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use compiled_babel_plugin_swc::options::PluginConfig;
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::ast::{EsVersion, Module, Program};
use swc_core::ecma::codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};
use swc_core::ecma::parser::{parse_file_as_module, EsSyntax, Syntax};

#[test]
fn fixtures_match_expected_outputs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests/fixtures");

    let overwrite = should_overwrite();

    for entry in fs::read_dir(&fixtures_dir).expect("fixtures directory to exist") {
        let entry = entry.expect("fixture entry");
        if !entry.file_type().expect("entry type").is_dir() {
            continue;
        }

        let fixture_dir = entry.path();
        let input_path = fixture_dir.join("in.jsx");
        if !input_path.exists() {
            continue;
        }

        let source = fs::read_to_string(&input_path).expect("read fixture input");
        let module = parse_fixture_module(&input_path, &source);

        let non_extract_output = transform_fixture(&fixture_dir, module.clone(), false);
        let expected_non_extract = if overwrite {
            String::new()
        } else {
            read_expected(&fixture_dir.join("out.js"))
        };
        if !overwrite {
            assert_eq!(
                normalise_line_endings(&non_extract_output.code),
                normalise_line_endings(&expected_non_extract),
                "non-extract output mismatch for {:?}",
                fixture_dir.file_name().unwrap()
            );
        }

        let extract_output = transform_fixture(&fixture_dir, module, true);
        let expected_style_rules = if overwrite {
            Vec::new()
        } else {
            read_expected_json(&fixture_dir.join("swc-style-rules.json"))
        };
        if !overwrite {
            assert_eq!(
                extract_output.style_rules,
                expected_style_rules,
                "style rules mismatch for {:?}",
                fixture_dir.file_name().unwrap()
            );
        }

        if overwrite {
            fs::write(
                fixture_dir.join("actual.js"),
                format_code_for_snapshot(&non_extract_output.code),
            )
            .expect("write actual.js");
            fs::write(
                fixture_dir.join("out.js"),
                format_code_for_snapshot(&non_extract_output.code),
            )
            .expect("write out.js");
            fs::write(
                fixture_dir.join("swc-style-rules.json"),
                serde_json::to_string_pretty(&extract_output.style_rules)
                    .expect("serialize style rules"),
            )
            .expect("write swc-style-rules.json");
        }
    }
}

struct FixtureOutput {
    code: String,
    style_rules: Vec<String>,
}

fn transform_fixture(dir: &Path, module: Module, extract: bool) -> FixtureOutput {
    let filename = dir.join("in.jsx");
    let mut config = PluginConfig::default();
    config.extract = extract;

    let metadata = TransformMetadata {
        filename: Some(filename),
        root_dir: Some(dir.to_path_buf()),
        caller: None,
    };

    let mut transform = Transform::new(config, metadata);
    let result = transform.apply(Program::Module(module));

    FixtureOutput {
        code: emit_program(&result.program),
        style_rules: result.style_rules,
    }
}

fn parse_fixture_module(path: &Path, source: &str) -> Module {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), source.to_owned());
    let syntax = Syntax::Es(EsSyntax {
        jsx: true,
        ..Default::default()
    });
    let mut errors = Vec::new();
    let module = parse_file_as_module(&fm, syntax, EsVersion::Es2022, None, &mut errors)
        .expect("parse fixture module");

    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    module
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

fn read_expected(path: &Path) -> String {
    fs::read_to_string(path).expect("read expected output")
}

fn read_expected_json(path: &Path) -> Vec<String> {
    let file = fs::read_to_string(path).expect("read expected json");
    serde_json::from_str(&file).expect("deserialize expected json")
}

fn normalise_line_endings(value: &str) -> String {
    value.replace('\r', "")
}

fn format_code_for_snapshot(code: &str) -> String {
    let mut formatted = code.trim().to_string();
    formatted.push('\n');
    formatted
}

fn should_overwrite() -> bool {
    if env::var_os("SWC_FIXTURES_OVERWRITE").is_some() {
        return true;
    }

    env::args().any(|arg| arg == "--overwrite")
}
