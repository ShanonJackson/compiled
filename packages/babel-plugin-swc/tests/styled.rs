use std::panic::catch_unwind;

#[path = "utils.rs"]
mod utils;
use utils::{panic_message, run_transform};

#[test]
fn styled_logical_expression_without_value_panics() {
    let source = r#"
        import { styled } from '@compiled/react';

        const Component = styled.div`
          font-weight: ${ (props) => (props.isPrimary && props.isMaybe) && 'bold' };
        `;
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(
        result.is_err(),
        "expected styled template with invalid logical expression to panic"
    );
    let message = panic_message(result.err().unwrap());
    assert_eq!(
        message,
        "A logical expression contains an invalid CSS declaration.\n      Compiled doesn't support CSS properties that are defined with a conditional rule that doesn't specify a default value.\n      Eg. font-weight: ${(props) => (props.isPrimary && props.isMaybe) && 'bold'}; is invalid.\n      Use ${(props) => props.isPrimary && props.isMaybe && ({ 'font-weight': 'bold' })}; instead",
        "unexpected panic message: {message}"
    );
}
