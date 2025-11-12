use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use compiled_babel::{
    transform_with_file as compiled_transform, CacheBehavior, PluginOptions as BabelPluginOptions,
    TransformFile, TransformFileOptions,
};
use compiled_strip_runtime::{
    transform as strip_transform, PluginOptions as StripPluginOptions, TransformConfig,
};
use serde_json::{from_str, to_string_pretty};
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::ast::EsVersion;
use swc_core::ecma::ast::Program;
use swc_ecma_codegen::{text_writer::JsWriter, Config as CodegenConfig, Emitter};
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures")
}

fn read_style_rules(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }

    let parsed = from_str::<Vec<String>>(&contents)?;
    Ok(parsed)
}

fn parse_fixture_program(code: &str, filename: &Path) -> (Program, Lrc<SourceMap>) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom(filename.display().to_string()).into(),
        code.into(),
    );

    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: true,
            decorators: true,
            dts: false,
            ..Default::default()
        }),
        EsVersion::Es2022,
        StringInput::from(&*fm),
        None,
    );

    let mut parser = Parser::new_from(lexer);
    let program = parser
        .parse_program()
        .unwrap_or_else(|error| panic!("failed to parse {}: {error:?}", filename.display()));

    if let Some(err) = parser.take_errors().into_iter().next() {
        panic!("failed to parse {}: {err:?}", filename.display());
    }

    (program, cm)
}

fn print_program(cm: &Lrc<SourceMap>, program: &Program) -> String {
    let mut buf = Vec::new();
    {
        let mut emitter = Emitter {
            cfg: CodegenConfig::default(),
            comments: None,
            cm: cm.clone(),
            wr: JsWriter::new(cm.clone(), "\n", &mut buf, None),
        };

        match program {
            Program::Module(module) => emitter.emit_module(module).unwrap(),
            Program::Script(script) => emitter.emit_script(script).unwrap(),
        }
    }

    let mut code = String::from_utf8(buf).expect("emitted code should be valid UTF-8");
    if !code.ends_with('\n') {
        code.push('\n');
    }

    code
}

fn run_fixture(root: &Path, name: &str) -> Result<(), Box<dyn Error>> {
    let fixture_dir = root.join(name);
    let input_path = fixture_dir.join("in.jsx");
    let input_code = fs::read_to_string(&input_path)?;

    // Ensure Babel baselines exist for manual inspection.
    let _ = fs::metadata(fixture_dir.join("babel-out.jsx"))?;

    // Canonical output is now SWC in out.jsx
    let mut expected_out = fs::read_to_string(fixture_dir.join("out.jsx"))?;
    let babel_style_rules = read_style_rules(&fixture_dir.join("babel-style-rules.json"))?;
    let mut stored_swc_style_rules = read_style_rules(&fixture_dir.join("swc-style-rules.json"))?;

    let update_fixtures = std::env::var("UPDATE_FIXTURES").is_ok();

    let (program, cm) = parse_fixture_program(&input_code, &input_path);

    let transform_file = TransformFile::with_options(
        cm.clone(),
        Vec::new(),
        TransformFileOptions {
            filename: Some(input_path.display().to_string()),
            ..TransformFileOptions::default()
        },
    );

    let mut babel_options = BabelPluginOptions::default();
    babel_options.cache = Some(CacheBehavior::Enabled(false));
    babel_options.optimize_css = Some(true);
    babel_options.import_react = Some(true);
    babel_options.extract = Some(true);

    let compiled_output = compiled_transform(program, transform_file, babel_options);
    let compiled_program = compiled_output.program.clone();
    let compiled_code = print_program(&cm, &compiled_program);

    let strip_output = strip_transform(
        compiled_output.program,
        TransformConfig {
            filename: Some(input_path.display().to_string()),
            options: StripPluginOptions {
                compiled_require_exclude: Some(true),
                ..Default::default()
            },
            ..Default::default()
        },
    );

    let generated_code = print_program(&cm, &strip_output.program);

    if update_fixtures {
        eprintln!(
            "updating fixture {name}:\ncompiled metadata: {:?}\ncompiled output:\n{compiled_code}\nstrip output:\n{generated_code}",
            compiled_output.metadata.style_rules
        );

        let style_rules_path = fixture_dir.join("swc-style-rules.json");
        let style_rules_json = to_string_pretty(&strip_output.metadata.style_rules)?;
        fs::write(style_rules_path, format!("{}\n", style_rules_json))?;
        stored_swc_style_rules = strip_output.metadata.style_rules.clone();

        let out_path = fixture_dir.join("out.jsx");
        fs::write(&out_path, &generated_code)?;
        expected_out = generated_code.clone();
    }

    assert_eq!(
        strip_output.metadata.style_rules, babel_style_rules,
        "strip-runtime metadata diverged from Babel baseline for fixture {name}"
    );
    assert_eq!(
        strip_output.metadata.style_rules, stored_swc_style_rules,
        "stored swc-style-rules.json is outdated for fixture {name}"
    );

    let mut expected_code = expected_out;
    if !expected_code.ends_with('\n') {
        expected_code.push('\n');
    }

    assert_eq!(
        generated_code, expected_code,
        "actual.js snapshot is outdated for fixture {name}"
    );

    Ok(())
}

#[test]
fn native_fixtures_match_baselines() -> Result<(), Box<dyn Error>> {
    let root = fixtures_root();
    let mut fixtures: Vec<String> = fs::read_dir(&root)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') {
                    None
                } else {
                    Some(name.into_owned())
                }
            } else {
                None
            }
        })
        .collect();

    fixtures.sort();

    for fixture in fixtures {
        run_fixture(&root, &fixture)?;
    }

    Ok(())
}
