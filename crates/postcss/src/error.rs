use std::fmt;

/// A CSS syntax error with source location information.
#[derive(Debug, Clone)]
pub struct CssSyntaxError {
    /// The error message.
    pub message: String,
    /// Reason (the message without "filename:line:col: " prefix).
    pub reason: String,
    /// The source line number (1-indexed).
    pub line: Option<u32>,
    /// The source column number (1-indexed).
    pub column: Option<u32>,
    /// The byte offset in the source.
    pub offset: Option<usize>,
    /// The source file path or id.
    pub file: Option<String>,
    /// The source CSS string.
    pub source: Option<String>,
    /// The plugin that raised the error (if any).
    pub plugin: Option<String>,
}

impl CssSyntaxError {
    /// Create a new CssSyntaxError.
    pub fn new(
        message: &str,
        line: Option<u32>,
        column: Option<u32>,
        source: Option<&str>,
        file: Option<&str>,
    ) -> Self {
        let reason = message.to_string();
        let full_message = if let (Some(f), Some(l), Some(c)) = (file, line, column) {
            format!("{f}:{l}:{c}: {message}")
        } else if let (Some(l), Some(c)) = (line, column) {
            format!("<css input>:{l}:{c}: {message}")
        } else {
            message.to_string()
        };

        CssSyntaxError {
            message: full_message,
            reason,
            line,
            column,
            offset: None,
            file: file.map(|s| s.to_string()),
            source: source.map(|s| s.to_string()),
            plugin: None,
        }
    }

    /// Create an "Unclosed" error at the given offset.
    pub fn unclosed(what: &str, offset: usize) -> Self {
        let mut err = CssSyntaxError::new(&format!("Unclosed {what}"), None, None, None, None);
        err.offset = Some(offset);
        err
    }
}

impl fmt::Display for CssSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CssSyntaxError {}
