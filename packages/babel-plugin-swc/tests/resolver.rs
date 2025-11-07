use std::path::PathBuf;

use compiled_babel_plugin_swc::options::{PluginConfig, ResolverConfig};
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use serde_json::json;
use swc_core::ecma::ast::Program;

#[path = "utils.rs"]
mod utils;

use utils::parse_module_from_path;

#[test]
fn resolves_imports_via_alias_config() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("tests/fixtures/resolver");
    let entry_path = fixture_dir.join("entry.tsx");
    let parsed = parse_module_from_path(&entry_path);

    let alias_target = fixture_dir.join("mixins/simple.js");
    let mut resolver = ResolverConfig::default();
    resolver.alias = Some(json!({
        "test": alias_target.to_string_lossy(),
    }));

    let mut config = PluginConfig::default();
    config.resolver = Some(resolver);
    config.extract = true;

    let metadata = TransformMetadata {
        filename: Some(entry_path.clone()),
        source_file_name: None,
        root_dir: Some(fixture_dir.clone()),
        caller: None,
        source_map: Some(parsed.source_map.clone()),
        comments: Some(parsed.comments.clone()),
    };

    let mut transform = Transform::new(config, metadata);

    let result = transform.apply(Program::Module(parsed.module.clone()));

    assert!(
        result
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:red")),
        "expected resolved style rules to include red"
    );
    assert!(
        result
            .included_files
            .iter()
            .any(|path| path.ends_with("mixins/simple.js")),
        "resolved module should be recorded as an included file"
    );
}

#[test]
fn resolves_package_using_condition_names() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("tests/fixtures/resolver");
    let entry_path = fixture_dir.join("conditional-entry.tsx");
    let parsed = parse_module_from_path(&entry_path);

    let modules_dir = fixture_dir.join("modules");

    let metadata = TransformMetadata {
        filename: Some(entry_path.clone()),
        source_file_name: None,
        root_dir: Some(fixture_dir.clone()),
        caller: None,
        source_map: Some(parsed.source_map.clone()),
        comments: Some(parsed.comments.clone()),
    };

    let mut resolver = ResolverConfig::default();
    resolver.modules = vec![modules_dir.to_string_lossy().into_owned()];
    resolver.condition_names = vec!["compiled".to_string()];

    let mut config = PluginConfig::default();
    config.resolver = Some(resolver);
    config.extract = true;

    let mut transform = Transform::new(config, metadata.clone());
    let compiled_result = transform.apply(Program::Module(parsed.module.clone()));

    assert!(
        compiled_result
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:crimson")),
        "expected compiled condition to resolve crimson variant"
    );
    assert!(
        compiled_result
            .included_files
            .iter()
            .any(|path| path.ends_with("compiled.js")),
        "resolved compiled file should be tracked"
    );

    let mut no_condition_resolver = ResolverConfig::default();
    no_condition_resolver.modules = vec![modules_dir.to_string_lossy().into_owned()];

    let mut fallback_config = PluginConfig::default();
    fallback_config.resolver = Some(no_condition_resolver);
    fallback_config.extract = true;

    let mut fallback_transform = Transform::new(fallback_config, metadata);
    let fallback_result = fallback_transform.apply(Program::Module(parsed.module));

    assert!(
        fallback_result
            .style_rules
            .iter()
            .any(|rule| rule.contains("color:black")),
        "default resolution should fall back to browser export"
    );
}
