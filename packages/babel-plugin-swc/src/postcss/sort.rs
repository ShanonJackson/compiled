//! Helpers that mirror the stylesheet sorting performed by the JavaScript
//! toolchain.
//!
//! The Babel implementation relies on `@compiled/css` which pipes the
//! collected atomic rules through a set of PostCSS transforms. The Rust port
//! progressively re-implements these routines so that the generated output
//! remains byte-for-byte compatible once the entire pipeline is brought
//! across. The current implementation focuses on providing the same public API
//! so callers can begin wiring the extraction flow while additional behaviour
//! is filled in.

/// Configuration toggles that influence how style sheets are ordered.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SortConfig {
    pub sort_at_rules_enabled: bool,
    pub sort_shorthand_enabled: bool,
}

/// Sorts an atomic style sheet into a deterministic order.
///
/// The JavaScript implementation first performs a lexical sort on the list of
/// atomic rules before handing the result to a PostCSS pipeline. We mirror the
/// same initial ordering step here while a native variant of the full
/// normalisation logic is ported. The returned string joins each rule with a
/// newline so the format matches the Babel output.
#[allow(dead_code)]
pub fn sort_atomic_style_sheet(rules: &[String], _config: SortConfig) -> String {
    let mut sorted = rules.to_vec();
    sorted.sort();
    sorted.join("\n")
}
