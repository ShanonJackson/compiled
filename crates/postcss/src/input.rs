use crate::ast::Position;
use crate::error::CssSyntaxError;

/// Wraps a CSS source string with metadata (file path, BOM handling) and
/// provides efficient offset → line/column lookups via a cached line-break index.
///
/// This is a 1:1 port of PostCSS 8.4.31 `input.js`.
#[derive(Debug, Clone)]
pub struct Input {
    /// The CSS source with BOM stripped.
    pub css: String,
    /// Whether the original source had a BOM.
    pub has_bom: bool,
    /// File path (if provided).
    pub file: Option<String>,
    /// Generated id (if no file path).
    pub id: Option<String>,
    /// Cached line start offsets for binary-search based offset→line/col lookup.
    /// Built lazily on first call to `from_offset`.
    line_starts: Option<Vec<usize>>,
}

impl Input {
    /// Create a new Input, stripping a leading BOM if present.
    pub fn new(css: &str, file: Option<&str>) -> Self {
        let (stripped, has_bom) = if css.starts_with('\u{FEFF}') || css.starts_with('\u{FFFE}') {
            (&css[3..], true) // BOM is 3 bytes in UTF-8
        } else {
            (css, false)
        };

        Input {
            css: stripped.to_string(),
            has_bom,
            file: file.map(|s| s.to_string()),
            id: if file.is_none() {
                Some("<input css>".to_string())
            } else {
                None
            },
            line_starts: None,
        }
    }

    /// The `from` identifier — either the file path or the generated id.
    pub fn from_path(&self) -> &str {
        if let Some(ref f) = self.file {
            f
        } else if let Some(ref id) = self.id {
            id
        } else {
            "<input css>"
        }
    }

    /// Convert a byte offset into a 1-indexed line and column.
    ///
    /// Uses binary search on cached line-break positions — exact port of
    /// PostCSS's `fromOffset()`.
    pub fn from_offset(&mut self, offset: usize) -> Position {
        // Build line_starts cache on first call.
        if self.line_starts.is_none() {
            let mut starts = Vec::new();
            let mut prev = 0;
            for (i, ch) in self.css.char_indices() {
                if starts.is_empty() {
                    starts.push(0);
                }
                if ch == '\n' {
                    // Next line starts at the byte after \n.
                    let next = i + 1;
                    starts.push(next);
                    prev = next;
                }
            }
            if starts.is_empty() {
                starts.push(0);
            }
            let _ = prev;
            self.line_starts = Some(starts);
        }

        let line_starts = self.line_starts.as_ref().unwrap();
        let last_line_start = *line_starts.last().unwrap();

        let line_idx = if offset >= last_line_start {
            line_starts.len() - 1
        } else {
            // Binary search: find the largest line_start <= offset.
            let mut min = 0usize;
            let mut max = if line_starts.len() >= 2 {
                line_starts.len() - 2
            } else {
                0
            };
            while min < max {
                let mid = min + ((max - min) >> 1);
                if offset < line_starts[mid] {
                    max = mid.saturating_sub(1);
                } else if offset >= line_starts[mid + 1] {
                    min = mid + 1;
                } else {
                    min = mid;
                    break;
                }
            }
            min
        };

        Position {
            line: (line_idx + 1) as u32,
            column: (offset - line_starts[line_idx] + 1) as u32,
            offset,
        }
    }

    /// Create a CssSyntaxError pointing at the given offset.
    pub fn error(&mut self, message: &str, offset: usize) -> CssSyntaxError {
        let pos = self.from_offset(offset);
        CssSyntaxError::new(
            message,
            Some(pos.line),
            Some(pos.column),
            Some(&self.css),
            self.file.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_offset_single_line() {
        let mut input = Input::new("color: red", None);
        let pos = input.from_offset(0);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);

        let pos = input.from_offset(7);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 8);
    }

    #[test]
    fn test_from_offset_multi_line() {
        let mut input = Input::new("a {\n  color: red;\n}", None);
        // 'a' is at offset 0 → line 1, col 1
        assert_eq!(input.from_offset(0), Position { line: 1, column: 1, offset: 0 });
        // '{' is at offset 2 → line 1, col 3
        assert_eq!(input.from_offset(2), Position { line: 1, column: 3, offset: 2 });
        // 'c' of color is at offset 6 → line 2, col 3 (offset 4 is \n, 5 is first space on line 2)
        // line 2 starts at offset 4
        assert_eq!(input.from_offset(6), Position { line: 2, column: 3, offset: 6 });
        // '}' is at offset 18 → line 3, col 1
        assert_eq!(input.from_offset(18), Position { line: 3, column: 1, offset: 18 });
    }

    #[test]
    fn test_bom_stripping() {
        let css_with_bom = "\u{FEFF}a { color: red }";
        let input = Input::new(css_with_bom, None);
        assert!(input.has_bom);
        assert_eq!(input.css, "a { color: red }");
    }

    #[test]
    fn test_no_bom() {
        let input = Input::new("a { color: red }", None);
        assert!(!input.has_bom);
    }
}
