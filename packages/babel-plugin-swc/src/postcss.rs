//! Minimal CSS normalisation utilities used by the SWC plugin.
//!
//! The Babel implementation relies on a dedicated PostCSS pipeline. The Rust
//! port mirrors the public surface of that pipeline so hashes remain stable
//! once the full feature set is brought across. As additional behaviour is
//! required the helpers in this module should be extended to stay in lockstep
//! with `packages/css`.

use std::borrow::Cow;

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
    use super::normalise_timing_function;

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
}
