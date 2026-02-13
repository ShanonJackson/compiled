/// A raw value preserves both the original (raw) text and the normalized value.
/// When stringifying, if `value` matches the node's current property, the `raw`
/// string is emitted instead — this preserves original formatting (comments,
/// extra whitespace, etc.) in a roundtrip.
#[derive(Debug, Clone, PartialEq)]
pub struct RawValue {
    /// The original source text including comments and whitespace.
    pub raw: String,
    /// The normalized value with comments/trailing whitespace stripped.
    pub value: String,
}

/// Format-preservation metadata for a CSS AST node.
///
/// Every field is optional — only set if the parser encountered the
/// corresponding formatting detail. The stringifier checks these first
/// and falls back to `DEFAULT_RAW` constants when they are absent.
#[derive(Debug, Clone, Default)]
pub struct Raws {
    // ── Common fields ──────────────────────────────────────────────
    /// Whitespace before the node (between previous sibling's end and this node's start).
    pub before: Option<String>,
    /// Whitespace inside a container after the last child (before `}`).
    pub after: Option<String>,
    /// Whitespace between the node's "key" part and its "value" part.
    ///   - Declaration: between property name and `:`
    ///   - Rule: between selector and `{`
    ///   - AtRule (no block): between params and `;`
    pub between: Option<String>,
    /// Whether the last declaration in a block has a trailing `;`.
    pub semicolon: Option<bool>,
    /// A standalone `;` that directly follows a rule's `}` (rare but valid CSS).
    pub own_semicolon: Option<String>,

    // ── Declaration-specific ───────────────────────────────────────
    /// The raw `!important` string if it differs from the canonical `" !important"`.
    pub important: Option<String>,
    /// Raw value preserving comments / extra whitespace in the declaration value.
    pub value: Option<RawValue>,

    // ── Comment-specific ───────────────────────────────────────────
    /// Whitespace immediately after `/*`.
    pub left: Option<String>,
    /// Whitespace immediately before `*/`.
    pub right: Option<String>,

    // ── AtRule-specific ────────────────────────────────────────────
    /// Whitespace between `@name` and the parameters.
    pub after_name: Option<String>,
    /// Raw params preserving comments / extra whitespace.
    pub params: Option<RawValue>,

    // ── Rule-specific ──────────────────────────────────────────────
    /// Raw selector preserving comments / extra whitespace.
    pub selector: Option<RawValue>,
}
