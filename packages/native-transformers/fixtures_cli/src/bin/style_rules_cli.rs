use std::io::{self, Read};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use swc_core::common::{sync::Lrc, FileName, SourceMap};
use swc_core::ecma::ast::{EsVersion, Program};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

use compiled_babel::{
  transform_with_file as compiled_transform, CacheBehavior, PluginOptions as CompiledOptions,
  TransformFile, TransformFileOptions,
};
use compiled_strip_runtime::{transform as strip_transform, TransformConfig as StripConfig};

#[derive(Deserialize)]
struct Request {
  filename: String,
  source: String,
  extract: Option<bool>,
}

#[derive(Serialize)]
struct Response {
  #[serde(rename = "styleRules")]
  style_rules: Vec<String>,
}

fn parse_program(cm: &Lrc<SourceMap>, filename: &str, src: &str) -> Result<Program, String> {
  let fm = cm.new_source_file(
    FileName::Real(PathBuf::from(filename)).into(),
    src.to_string(),
  );
  let lexer = Lexer::new(
    Syntax::Typescript(TsSyntax {
      tsx: true,
      ..Default::default()
    }),
    EsVersion::Es2022,
    StringInput::from(&*fm),
    None,
  );
  let mut parser = Parser::new_from(lexer);
  parser.parse_program().map_err(|err| format!("{:?}", err))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let request: Request = serde_json::from_str(&input)?;

  let cm: Lrc<SourceMap> = Default::default();
  let program = parse_program(&cm, &request.filename, &request.source)?;

  let compiled_opts = CompiledOptions {
    cache: Some(CacheBehavior::Enabled(false)),
    import_react: Some(true),
    optimize_css: Some(true),
    extract: Some(request.extract.unwrap_or(true)),
    ..CompiledOptions::default()
  };

  let transform_file = TransformFile::with_options(
    cm.clone(),
    Vec::new(),
    TransformFileOptions {
      filename: Some(request.filename.clone()),
      ..Default::default()
    },
  );

  let compiled_output = compiled_transform(program, transform_file, compiled_opts);

  let strip_cfg = StripConfig {
    filename: Some(request.filename.clone()),
    options: compiled_strip_runtime::PluginOptions {
      compiled_require_exclude: Some(true),
      ..Default::default()
    },
    ..Default::default()
  };
  let strip_output = strip_transform(compiled_output.program, strip_cfg);

  let mut style_rules = if !strip_output.metadata.style_rules.is_empty() {
    strip_output.metadata.style_rules
  } else {
    compiled_output.metadata.style_rules
  };
  style_rules.retain(|rule| rule.contains('{'));
  style_rules.sort();
  style_rules.dedup();

  let response = Response { style_rules };
  println!("{}", serde_json::to_string(&response)?);
  Ok(())
}
