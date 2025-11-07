//! Minimal CSS normalisation utilities used by the SWC plugin.
//!
//! The Babel implementation relies on a dedicated PostCSS pipeline. The Rust
//! port mirrors the public surface of that pipeline so hashes remain stable
//! once the full feature set is brought across. As additional behaviour is
//! required the helpers in this module should be extended to stay in lockstep
//! with `packages/css`.

use std::borrow::Cow;

mod color;
mod ordered_values;
mod reduce_initial;
mod sort;

pub use color::{is_color, minify_color};
pub use ordered_values::normalise_ordered_value;
pub use reduce_initial::from_initial;
pub use sort::{sort_atomic_style_sheet, SortConfig};

/// Normalises timing function shorthands such as `300ms` to `0.3s`.
///
/// The implementation intentionally mirrors the behaviour of the PostCSS setup
/// used in the Babel plugin. We stick to floating point maths to match the
/// JavaScript implementation byte-for-byte and therefore hash compatible
/// outputs.
#[allow(dead_code)]
pub fn normalise_timing_function(value: &str) -> Cow<'_, str> {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut index = 0;
    let mut last_emitted = 0;
    let mut changed = false;
    let mut output = String::new();

    while index < len {
        if !is_number_start(bytes, index) {
            index += 1;
            continue;
        }

        let start = index;
        let (next, has_digits) = parse_number(bytes, index);

        if !has_digits {
            index += 1;
            continue;
        }

        index = next;

        if index < len && bytes[index] == b'm' && index + 1 < len && bytes[index + 1] == b's' {
            if let Ok(number) = value[start..index].parse::<f64>() {
                output.push_str(&value[last_emitted..start]);
                output.push_str(&format!("{:.3}", number / 1000.0));
                output.push('s');
                changed = true;
                index += 2;
                last_emitted = index;
                continue;
            }
        } else if index < len && bytes[index] == b's' {
            output.push_str(&value[last_emitted..=index]);
            index += 1;
            last_emitted = index;
            continue;
        }

        // Could not handle this token, advance a single character and try again.
        index = start + 1;
    }

    if !changed {
        return Cow::Borrowed(value);
    }

    if last_emitted < len {
        output.push_str(&value[last_emitted..]);
    }

    Cow::Owned(output)
}

/// Normalises the CSS `currentColor` keyword casing.
///
/// The JavaScript implementation performs this normalisation through the
/// custom `normalize-current-color` PostCSS plugin when `optimizeCss` is
/// enabled. The Rust port exposes the same helper so callers can decide when to
/// apply it based on configuration.
#[allow(dead_code)]
pub fn normalise_current_color(value: &str) -> Cow<'_, str> {
    if value.eq_ignore_ascii_case("currentcolor") || value.eq_ignore_ascii_case("current-color") {
        Cow::Borrowed("currentColor")
    } else {
        Cow::Borrowed(value)
    }
}

#[allow(dead_code)]
pub fn normalise_zero_unit(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut split_index = None;

    for (index, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() || ch == '+' || ch == '-' || ch == '.' {
            continue;
        }
        split_index = Some(index);
        break;
    }

    let Some(index) = split_index else {
        if trimmed
            .parse::<f64>()
            .map(|number| number == 0.0)
            .unwrap_or(false)
        {
            return Some(String::from("0"));
        }
        return None;
    };

    let (number_part, unit_part) = trimmed.split_at(index);
    if unit_part.is_empty() {
        return None;
    }

    if !unit_part
        .chars()
        .all(|ch| ch.is_ascii_alphabetic() || ch == '%' || ch == '-')
    {
        return None;
    }

    if number_part
        .parse::<f64>()
        .map(|number| number == 0.0)
        .unwrap_or(false)
    {
        Some(String::from("0"))
    } else {
        None
    }
}

pub fn reduce_initial_value(property: &str, value: &str) -> Option<String> {
    if !value.eq_ignore_ascii_case("initial") {
        return None;
    }

    from_initial(property).map(|replacement| replacement.to_string())
}

fn is_number_start(bytes: &[u8], index: usize) -> bool {
    match bytes[index] {
        b'+' | b'-' => {
            let next = index + 1;
            next < bytes.len() && (bytes[next].is_ascii_digit() || bytes[next] == b'.')
        }
        b'.' => {
            let next = index + 1;
            next < bytes.len() && bytes[next].is_ascii_digit()
        }
        b'0'..=b'9' => true,
        _ => false,
    }
}

fn parse_number(bytes: &[u8], mut index: usize) -> (usize, bool) {
    let len = bytes.len();
    let mut has_digits = false;

    if index < len && (bytes[index] == b'+' || bytes[index] == b'-') {
        index += 1;
    }

    while index < len && bytes[index].is_ascii_digit() {
        has_digits = true;
        index += 1;
    }

    if index < len && bytes[index] == b'.' {
        index += 1;

        while index < len && bytes[index].is_ascii_digit() {
            has_digits = true;
            index += 1;
        }
    }

    (index, has_digits)
}

/// Normalises the `content` CSS property to ensure strings are consistently
/// quoted.
///
/// The behaviour is adapted from the Babel/PostCSS toolchain so that hashes
/// derived from the resulting CSS remain identical across the Rust and
/// JavaScript implementations.
#[allow(dead_code)]
pub fn normalise_content_value(value: &str) -> Cow<'_, str> {
    if value.is_empty() {
        return Cow::Borrowed("\"\"");
    }

    if value.contains('"') || value.contains('\'') {
        return Cow::Borrowed(value);
    }

    let trimmed = value.trim_start();

    if starts_with_function_like(trimmed) {
        return Cow::Borrowed(value);
    }

    if trimmed.contains("-quote") {
        return Cow::Borrowed(value);
    }

    if matches_keyword(trimmed) {
        return Cow::Borrowed(value);
    }

    Cow::Owned(format!("\"{value}\""))
}

fn starts_with_function_like(value: &str) -> bool {
    let mut chars = value.chars();
    let mut seen = false;

    while let Some(ch) = chars.next() {
        if ch.is_ascii_alphabetic() || ch == '-' {
            seen = true;
            continue;
        }

        if ch == '(' {
            return seen;
        }

        return false;
    }

    false
}

fn matches_keyword(value: &str) -> bool {
    const KEYWORDS: [&str; 6] = ["inherit", "initial", "none", "normal", "revert", "unset"];

    for keyword in KEYWORDS {
        if value.starts_with(keyword) {
            let tail = value.chars().nth(keyword.len());
            if tail.is_none() || tail.unwrap().is_whitespace() {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        minify_color, normalise_current_color, normalise_timing_function, normalise_zero_unit,
        reduce_initial_value,
    };

    #[test]
    fn converts_milliseconds_to_seconds() {
        assert_eq!(normalise_timing_function("300ms"), "0.300s");
        assert_eq!(normalise_timing_function("16ms"), "0.016s");
    }

    #[test]
    fn leaves_seconds_alone() {
        assert_eq!(normalise_timing_function("0.5s"), "0.5s");
    }

    #[test]
    fn normalises_composite_values() {
        assert_eq!(
            normalise_timing_function("ease-in 150ms ease-out 75ms"),
            "ease-in 0.150s ease-out 0.075s"
        );
    }

    #[test]
    fn normalises_comma_separated_values() {
        assert_eq!(normalise_timing_function("150ms, 75ms"), "0.150s, 0.075s");
    }

    #[test]
    fn handles_negative_durations() {
        assert_eq!(normalise_timing_function("-200ms"), "-0.200s");
    }

    #[test]
    fn normalises_current_color_keyword() {
        assert_eq!(normalise_current_color("currentcolor"), "currentColor");
        assert_eq!(normalise_current_color("CURRENT-COLOR"), "currentColor");
    }

    #[test]
    fn leaves_other_values_untouched() {
        assert_eq!(normalise_current_color("inherit"), "inherit");
        assert_eq!(normalise_current_color("var(--token)"), "var(--token)");
    }

    #[test]
    fn normalises_zero_units() {
        assert_eq!(normalise_zero_unit("0px"), Some("0".to_string()));
        assert_eq!(normalise_zero_unit("0.0em"), Some("0".to_string()));
        assert_eq!(normalise_zero_unit(" 0rem "), Some("0".to_string()));
        assert_eq!(normalise_zero_unit("1px"), None);
    }

    #[test]
    fn reduces_initial_values() {
        assert_eq!(
            reduce_initial_value("margin-left", "initial"),
            Some("0".to_string())
        );
        assert_eq!(reduce_initial_value("color", "initial"), None);
        assert_eq!(reduce_initial_value("margin", "inherit"), None);
    }

    #[test]
    fn minifies_simple_colors() {
        assert_eq!(minify_color("rebeccapurple"), Some("#639".to_string()));
        assert_eq!(minify_color("rgb(255, 0, 0)"), Some("red".to_string()));
        assert_eq!(minify_color("var(--token)"), None);
    }
}
