use std::panic::catch_unwind;

#[path = "utils.rs"]
mod utils;
use utils::{panic_message, run_transform};

#[test]
fn class_names_children_must_be_function() {
    let source = r#"
        import { ClassNames } from '@compiled/react';
        export const Component = () => (
          <ClassNames>
            <div />
          </ClassNames>
        );
    "#;

    let result = catch_unwind(|| run_transform(source));
    assert!(
        result.is_err(),
        "expected ClassNames without render prop to panic"
    );
    let message = panic_message(result.err().unwrap());
    assert_eq!(
        message,
        "ClassNames children should be a function\nE.g: <ClassNames>{props => <div />}</ClassNames>",
        "unexpected panic message: {message}"
    );
}
