use std::panic::catch_unwind;
use std::path::PathBuf;

use compiled_babel_plugin_swc::options::PluginConfig;
use compiled_babel_plugin_swc::transform::Transform;
use compiled_babel_plugin_swc::types::TransformMetadata;
use swc_core::ecma::ast::Program;
#[path = "utils.rs"]
mod utils;
use utils::{panic_message, parse_module, run_transform};

#[test]
fn css_map_tagged_template_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap`color: red;`;
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected cssMap tagged template to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("cssMap function cannot be used as a tagged template expression."),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_misused_outside_declarator_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const create = () => cssMap({ root: { color: 'red' } });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected cssMap misuse to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("CSS Map must be declared at the top-most scope of the module."),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_duplicate_selector_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap({
            success: {
                '&:hover': { color: 'red' },
                selectors: {
                    '&:hover': { color: 'blue' },
                },
            },
        });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected duplicate selectors to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("Cannot declare a selector more than once in CSS Map."),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_spread_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const base = { root: { color: 'red' } };
        const styles = cssMap({
            ...base,
        });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected spread element to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("Spread element is not supported in CSS Map."),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_object_method_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap({
            danger() {}
        });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected object method to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("Object method is not supported in CSS Map."),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_plain_selector_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap({
            success: {
                selectors: {
                    ':hover': { color: 'red' },
                },
            },
        });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected plain selector to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains(
            "This selector is applied to the parent element, and so you need to specify the ampersand symbol (&) directly before it."
        ),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_duplicate_selectors_block_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap({
            success: {
                selectors: {
                    '&:hover': { color: 'red' },
                },
                selectors: {
                    '&:active': { color: 'blue' },
                },
            },
        });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(
        result.is_err(),
        "expected duplicate selectors block to panic"
    );
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains(
            "Duplicate `selectors` key found in cssMap; expected either zero `selectors` keys or one."
        ),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_selectors_value_type_panics() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap({
            success: {
                selectors: 'color: red',
            },
        });
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(result.is_err(), "expected invalid selectors value to panic");
    let message = panic_message(result.err().unwrap());
    assert!(
        message.contains("Value of `selectors` key must be an object."),
        "unexpected panic message: {message}"
    );
}

#[test]
fn css_map_records_variant_sheets() {
    let source = r#"
        import { cssMap } from '@compiled/react';
        const styles = cssMap({
            primary: { color: 'red' },
            secondary: { color: 'blue' },
        });
    "#;

    let module = parse_module(source);
    let mut config = PluginConfig::default();
    config.extract = false;

    let metadata = TransformMetadata {
        filename: Some(PathBuf::from("inline.tsx")),
        source_file_name: None,
        root_dir: None,
        caller: None,
        source_map: None,
        comments: None,
    };

    let mut transform = Transform::new(config, metadata);
    transform.apply(Program::Module(module));

    let sheets = transform
        .state()
        .css_map_sheets()
        .get("styles")
        .expect("cssMap sheets recorded");

    assert_eq!(sheets.len(), 2, "expected two variant sheets");
    assert!(
        sheets.iter().any(|rule| rule.contains("color:red")),
        "expected red rule in cssMap sheets"
    );
    assert!(
        sheets.iter().any(|rule| rule.contains("color:blue")),
        "expected blue rule in cssMap sheets"
    );
}
