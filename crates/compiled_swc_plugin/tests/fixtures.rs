use std::fs;

mod support;

use support::{
  EnvGuard, canonicalize_output, emit_program, fixtures_dir, load_fixture_config, parse_program,
  run_transform,
};

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
    if let Ok(filter) = std::env::var("FIXTURE_FILTER") {
      if fixture_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name != filter)
        .unwrap_or(true)
      {
        continue;
      }
    }
    let input_path = fixture_path.join("in.jsx");
    let expected_path = fixture_path.join("out.js");
    if !input_path.exists() || !expected_path.exists() {
      panic!(
        "fixture {:?} is missing required files (in.jsx/out.js)",
        fixture_path
      );
    }
    let input = fs::read_to_string(&input_path).expect("failed to read fixture input");
    let expected_source =
      fs::read_to_string(&expected_path).expect("failed to read fixture output");
    let expected = emit_program(&parse_program(&expected_path, &expected_source));
    let (config_json, node_env, babel_env) = load_fixture_config(&fixture_path);
    let _guard = EnvGuard::new(node_env.as_deref(), babel_env.as_deref());
    let actual = canonicalize_output(&run_transform(&input_path, &input, &config_json));
    if let Ok(filter) = std::env::var("FIXTURE_DEBUG") {
      if fixture_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| name == filter)
        .unwrap_or(false)
      {
        println!("expected:\n{}", normalize(&expected));
        println!("actual:\n{}", normalize(&actual));
      }
    }
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
