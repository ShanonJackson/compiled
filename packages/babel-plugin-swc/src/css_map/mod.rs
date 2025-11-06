//! Port of `src/css-map` from the Babel plugin.

pub mod errors;
pub mod process_selectors;

use swc_core::ecma::ast::{ArrayLit, Expr, ExprOrSpread, ObjectLit, Prop, PropOrSpread};

use self::errors::ErrorMessages;

pub const EXTENDED_SELECTORS_KEY: &str = "selectors";

/// Ensures that the cssMap argument is a static object expression just like the
/// Babel implementation expects.
pub fn validate_root_object(expr: &Expr) -> Result<&ObjectLit, ErrorMessages> {
    match expr {
        Expr::Object(object) => Ok(object),
        _ => Err(ErrorMessages::ArgumentType),
    }
}

/// Validates each variant declared in the cssMap call. The Babel plugin throws
/// descriptive errors when encountering spreads, methods, or dynamic variant
/// objects. We mirror that behaviour here so fixture expectations stay in sync.
pub fn validate_root_variants(object: &ObjectLit) -> Result<(), ErrorMessages> {
    for prop in &object.props {
        match prop {
            PropOrSpread::Prop(prop) => match &**prop {
                Prop::KeyValue(kv) => {
                    if !matches!(&*kv.value, Expr::Object(_)) {
                        return Err(ErrorMessages::StaticVariantObject);
                    }

                    if let Expr::Object(nested) = &*kv.value {
                        validate_nested_object(nested)?;
                    }
                }
                Prop::Shorthand(_) | Prop::Assign(_) => {
                    return Err(ErrorMessages::StaticVariantObject);
                }
                Prop::Getter(_) | Prop::Setter(_) | Prop::Method(_) => {
                    return Err(ErrorMessages::NoObjectMethod);
                }
            },
            PropOrSpread::Spread(_) => return Err(ErrorMessages::NoSpreadElement),
        }
    }

    Ok(())
}

fn validate_nested_object(object: &ObjectLit) -> Result<(), ErrorMessages> {
    for prop in &object.props {
        match prop {
            PropOrSpread::Prop(prop) => match &**prop {
                Prop::KeyValue(kv) => validate_nested_value(&kv.value)?,
                Prop::Shorthand(_) => {}
                Prop::Assign(_) => return Err(ErrorMessages::StaticVariantObject),
                Prop::Getter(_) | Prop::Setter(_) | Prop::Method(_) => {
                    return Err(ErrorMessages::NoObjectMethod);
                }
            },
            PropOrSpread::Spread(_) => return Err(ErrorMessages::NoSpreadElement),
        }
    }

    Ok(())
}

fn validate_nested_value(expr: &Expr) -> Result<(), ErrorMessages> {
    match expr {
        Expr::Object(object) => validate_nested_object(object),
        Expr::Array(array) => validate_array(array),
        _ => Ok(()),
    }
}

fn validate_array(array: &ArrayLit) -> Result<(), ErrorMessages> {
    for elem in &array.elems {
        if let Some(ExprOrSpread {
            spread: Some(_), ..
        }) = elem
        {
            return Err(ErrorMessages::StaticVariantObject);
        }

        if let Some(ExprOrSpread { spread: None, expr }) = elem {
            validate_nested_value(expr)?;
        }
    }

    Ok(())
}
