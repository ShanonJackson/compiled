use std::io::{self, Read};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use swc_core::common::comments::SingleThreadedComments;
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
  #[serde(rename = "compiledConfig")]
  compiled_config: Option<String>,
  extract: Option<bool>,
}

#[derive(Serialize)]
struct Response {
  #[serde(rename = "styleRules")]
  style_rules: Vec<String>,
}

fn resolver_compat_from_config(raw: &Option<String>) -> Option<compiled_babel::ResolverCompatOptions> {
  let text = raw.as_ref()?;
  let value: serde_json::Value = serde_json::from_str(text).ok()?;
  let include_sources = value
    .get("resolverCompat")
    .and_then(|rc| rc.get("includeSourcesFor"))
    .and_then(|arr| {
      arr.as_array().map(|items| {
        items
          .iter()
          .filter_map(|item| item.as_str().map(|s| s.to_string()))
          .collect::<Vec<String>>()
      })
    })
    .filter(|list| !list.is_empty());

  include_sources.map(|include_sources_for| compiled_babel::ResolverCompatOptions {
    include_sources_for: Some(include_sources_for),
  })
}

fn parse_program(
  cm: &Lrc<SourceMap>,
  filename: &str,
  src: &str,
) -> Result<(Program, Vec<swc_core::common::comments::Comment>), String> {
  let fm = cm.new_source_file(FileName::Real(PathBuf::from(filename)).into(), src.to_string());
  let comments = SingleThreadedComments::default();
  let lexer = Lexer::new(
    Syntax::Typescript(TsSyntax {
      tsx: true,
      ..Default::default()
    }),
    EsVersion::Es2022,
    StringInput::from(&*fm),
    Some(&comments),
  );
  let mut parser = Parser::new_from(lexer);
  let program = parser.parse_program().map_err(|err| format!("{:?}", err))?;

  let (leading, trailing) = comments.take_all();
  let mut gathered: Vec<_> = Vec::new();
  for map in [&leading, &trailing] {
    for list in map.borrow().values() {
      gathered.extend(list.clone());
    }
  }

  Ok((program, gathered))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let request: Request = serde_json::from_str(&input)?;

  let cm: Lrc<SourceMap> = Default::default();
  let (program, mut gathered_comments) =
    parse_program(&cm, &request.filename, &request.source)?;

  let compiled_opts = CompiledOptions {
    cache: Some(CacheBehavior::Enabled(false)),
    import_react: Some(true),
    optimize_css: Some(true),
    resolver_compat: resolver_compat_from_config(&request.compiled_config),
    extract: Some(request.extract.unwrap_or(true)),
    ..CompiledOptions::default()
  };

  let transform_file = TransformFile::with_options(
    cm.clone(),
    std::mem::take(&mut gathered_comments),
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
