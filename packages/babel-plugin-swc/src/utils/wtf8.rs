use std::borrow::Cow;

use swc_core::ecma::atoms::Atom;

pub fn wtf8_to_string(value: &Atom) -> String {
    value.as_str().to_owned()
}

pub fn wtf8_to_cow<'a>(value: &'a Atom) -> Cow<'a, str> {
    Cow::Borrowed(value.as_str())
}
