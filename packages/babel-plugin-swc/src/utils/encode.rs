use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

// The encodeURIComponent whitelist excludes the following characters from
// percent-encoding: alphabetic, decimal digits, - _ . ! ~ * ' ( )
const ENCODE_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'!')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'\'');

/// Percent encodes a CSS rule so it can be appended to a query parameter.
///
/// Mirrors the behaviour of the JavaScript `encodeURIComponent` helper used in
/// the Babel strip-runtime plugin while ensuring exclamation marks are escaped
/// to avoid confusing Webpack's loader syntax.
#[allow(dead_code)]
pub fn to_uri_component(input: &str) -> String {
    let encoded = utf8_percent_encode(input, ENCODE_COMPONENT).to_string();
    encoded.replace('!', "%21")
}
