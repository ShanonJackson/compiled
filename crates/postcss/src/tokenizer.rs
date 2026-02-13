//! CSS tokenizer — zero-copy, byte-level port of PostCSS 8.4.31 `tokenize.js`.
//!
//! Tokens hold `&str` slices into the original input, avoiding allocation.
//! Hand-rolled byte scanners replace JS regexes for maximum throughput.

/// Token type, matching PostCSS 8.4.31 token kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Space,
    Word,
    String,
    Brackets,
    Comment,
    AtWord,
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `[`
    OpenSquare,
    /// `]`
    CloseSquare,
    /// `{`
    OpenCurly,
    /// `}`
    CloseCurly,
    /// `;`
    Semicolon,
    /// `:`
    Colon,
}

/// A token produced by the tokenizer.
///
/// `value` is a zero-copy `&str` slice into the original CSS input.
#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenType,
    pub value: &'a str,
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset of the last character (inclusive), if applicable.
    pub end: Option<usize>,
}

// ── Character class lookup tables (replacing JS regexes) ───────────────────

/// Characters that terminate an `@word` token.
/// JS: `RE_AT_END = /[\t\n\f\r "#'()/;[\\\]{}]/g`
#[inline(always)]
fn is_at_end(b: u8) -> bool {
    matches!(
        b,
        b'\t' | b'\n'
            | b'\x0C'
            | b'\r'
            | b' '
            | b'"'
            | b'#'
            | b'\''
            | b'('
            | b')'
            | b'/'
            | b';'
            | b'['
            | b'\\'
            | b']'
            | b'{'
            | b'}'
    )
}

/// Characters that terminate a `word` token.
/// JS: `RE_WORD_END = /[\t\n\f\r !"#'():;@[\\\]{}]|\/(?=\*)/g`
/// The `\/(?=\*)` lookahead is handled separately.
#[inline(always)]
fn is_word_end(b: u8) -> bool {
    matches!(
        b,
        b'\t' | b'\n'
            | b'\x0C'
            | b'\r'
            | b' '
            | b'!'
            | b'"'
            | b'#'
            | b'\''
            | b'('
            | b')'
            | b':'
            | b';'
            | b'@'
            | b'['
            | b'\\'
            | b']'
            | b'{'
            | b'}'
    )
}

/// Characters that make a bracket token "bad" (must be preceded by any char).
/// JS: `RE_BAD_BRACKET = /.[\r\n"'(/\\]/`
#[inline(always)]
fn is_bad_bracket_char(b: u8) -> bool {
    matches!(b, b'\r' | b'\n' | b'"' | b'\'' | b'(' | b'/' | b'\\')
}

/// Hex digit test for escape sequences.
#[inline(always)]
fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

/// Whitespace characters that form space tokens.
#[inline(always)]
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\t' | b'\r' | b'\x0C')
}

// ── Tokenizer ──────────────────────────────────────────────────────────────

pub struct Tokenizer<'a> {
    css: &'a str,
    bytes: &'a [u8],
    len: usize,
    pos: usize,
    /// Buffer of tokens used for the `url()` previous-word check.
    buffer: Vec<Token<'a>>,
    /// Tokens pushed back via `back()`.
    returned: Vec<Token<'a>>,
    /// Whether to ignore unclosed errors (lenient mode).
    ignore_errors: bool,
}

impl<'a> Tokenizer<'a> {
    /// Create a new tokenizer for the given CSS string.
    pub fn new(css: &'a str, ignore_errors: bool) -> Self {
        Tokenizer {
            css,
            bytes: css.as_bytes(),
            len: css.len(),
            pos: 0,
            buffer: Vec::new(),
            returned: Vec::new(),
            ignore_errors,
        }
    }

    /// Returns the current byte position.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Returns true if there are no more tokens.
    pub fn end_of_file(&self) -> bool {
        self.returned.is_empty() && self.pos >= self.len
    }

    /// Push a token back so it will be returned by the next `next_token()` call.
    pub fn back(&mut self, token: Token<'a>) {
        self.returned.push(token);
    }

    /// Produce the next token, or `None` if at EOF.
    ///
    /// This is a faithful 1:1 port of PostCSS 8.4.31 `tokenize.js`'s
    /// `nextToken()` function, preserving every edge case.
    pub fn next_token(&mut self) -> Option<Token<'a>> {
        self.next_token_opts(false)
    }

    /// Produce the next token with optional `ignoreUnclosed` flag.
    pub fn next_token_opts(&mut self, ignore_unclosed: bool) -> Option<Token<'a>> {
        if let Some(t) = self.returned.pop() {
            return Some(t);
        }
        if self.pos >= self.len {
            return None;
        }

        let code = self.bytes[self.pos];

        match code {
            // ── Whitespace ─────────────────────────────────────────
            b' ' | b'\n' | b'\t' | b'\r' | b'\x0C' => {
                let start = self.pos;
                let mut next = self.pos + 1;
                while next < self.len && is_space(self.bytes[next]) {
                    next += 1;
                }
                let token = Token {
                    kind: TokenType::Space,
                    value: &self.css[start..next],
                    start,
                    end: None,
                };
                // JS: `pos = next - 1` then `pos++` at the end → pos = next.
                self.pos = next;
                Some(token)
            }

            // ── Single-char punctuation ────────────────────────────
            b'[' => self.single_char(TokenType::OpenSquare),
            b']' => self.single_char(TokenType::CloseSquare),
            b'{' => self.single_char(TokenType::OpenCurly),
            b'}' => self.single_char(TokenType::CloseCurly),
            b':' => self.single_char(TokenType::Colon),
            b';' => self.single_char(TokenType::Semicolon),
            b')' => self.single_char(TokenType::CloseParen),

            // ── Open paren (with url() bracket handling) ───────────
            b'(' => self.open_paren(ignore_unclosed),

            // ── Strings ────────────────────────────────────────────
            b'\'' | b'"' => self.string_token(code, ignore_unclosed),

            // ── At-word ────────────────────────────────────────────
            b'@' => self.at_word(),

            // ── Backslash escape ───────────────────────────────────
            b'\\' => self.backslash(),

            // ── Default: comment or word ───────────────────────────
            _ => {
                if code == b'/' && self.pos + 1 < self.len && self.bytes[self.pos + 1] == b'*' {
                    self.comment(ignore_unclosed)
                } else {
                    self.word()
                }
            }
        }
    }

    // ── Helper methods ─────────────────────────────────────────────────────

    /// Emit a single-character token and advance position.
    fn single_char(&mut self, kind: TokenType) -> Option<Token<'a>> {
        let start = self.pos;
        let token = Token {
            kind,
            value: &self.css[start..start + 1],
            start,
            end: Some(start),
        };
        self.pos = start + 1;
        Some(token)
    }

    /// Handle `(` — either a plain paren or a `url(...)` brackets token.
    fn open_paren(&mut self, ignore_unclosed: bool) -> Option<Token<'a>> {
        let start = self.pos;

        // Check if previous word token was "url".
        let prev_word = if let Some(last) = self.buffer.last() {
            last.value
        } else {
            ""
        };

        let n = if self.pos + 1 < self.len {
            self.bytes[self.pos + 1]
        } else {
            0
        };

        if prev_word == "url"
            && n != b'\''
            && n != b'"'
            && n != b' '
            && n != b'\n'
            && n != b'\t'
            && n != b'\x0C'
            && n != b'\r'
        {
            // url(...) without quotes — scan to matching `)`, handling escapes.
            self.buffer.pop(); // consume the "url" from the buffer
            let mut next = self.pos;
            loop {
                let found = memchr::memchr(b')', &self.bytes[next + 1..]);
                match found {
                    None => {
                        if self.ignore_errors || ignore_unclosed {
                            next = self.pos;
                            break;
                        } else {
                            panic!("Unclosed bracket at offset {}", self.pos);
                        }
                    }
                    Some(rel) => {
                        next = next + 1 + rel;
                        // Check for backslash escapes before the `)`.
                        let mut escape_pos = next;
                        let mut escaped = false;
                        while escape_pos > 0 && self.bytes[escape_pos - 1] == b'\\' {
                            escape_pos -= 1;
                            escaped = !escaped;
                        }
                        if !escaped {
                            break;
                        }
                        // The `)` was escaped, keep scanning.
                    }
                }
            }

            let token = Token {
                kind: TokenType::Brackets,
                value: &self.css[start..next + 1],
                start,
                end: Some(next),
            };
            self.pos = next + 1;
            Some(token)
        } else {
            // Check for simple brackets: `(...)` with no bad chars inside.
            let close = memchr::memchr(b')', &self.bytes[self.pos + 1..]);
            match close {
                Some(rel) => {
                    let next = self.pos + 1 + rel;
                    let content = &self.bytes[self.pos..next + 1];
                    // RE_BAD_BRACKET: /.[\r\n"'(/\\]/ — any char followed by a bad char.
                    let bad = content.len() >= 2
                        && content[1..]
                            .iter()
                            .any(|&b| is_bad_bracket_char(b));
                    if bad {
                        let token = Token {
                            kind: TokenType::OpenParen,
                            value: "(",
                            start,
                            end: Some(start),
                        };
                        self.pos = start + 1;
                        Some(token)
                    } else {
                        let token = Token {
                            kind: TokenType::Brackets,
                            value: &self.css[start..next + 1],
                            start,
                            end: Some(next),
                        };
                        self.pos = next + 1;
                        Some(token)
                    }
                }
                None => {
                    // No closing paren found — emit as plain `(`.
                    let token = Token {
                        kind: TokenType::OpenParen,
                        value: "(",
                        start,
                        end: Some(start),
                    };
                    self.pos = start + 1;
                    Some(token)
                }
            }
        }
    }

    /// Parse a quoted string (`'...'` or `"..."`).
    fn string_token(&mut self, quote: u8, ignore_unclosed: bool) -> Option<Token<'a>> {
        let start = self.pos;
        let quote_char = quote;
        let mut next = self.pos;

        loop {
            let found = memchr::memchr(quote_char, &self.bytes[next + 1..]);
            match found {
                None => {
                    if self.ignore_errors || ignore_unclosed {
                        next = self.pos + 1;
                        break;
                    } else {
                        panic!("Unclosed string at offset {}", self.pos);
                    }
                }
                Some(rel) => {
                    next = next + 1 + rel;
                    // Check for escapes.
                    let mut escape_pos = next;
                    let mut escaped = false;
                    while escape_pos > 0 && self.bytes[escape_pos - 1] == b'\\' {
                        escape_pos -= 1;
                        escaped = !escaped;
                    }
                    if !escaped {
                        break;
                    }
                }
            }
        }

        let token = Token {
            kind: TokenType::String,
            value: &self.css[start..next + 1],
            start,
            end: Some(next),
        };
        self.pos = next + 1;
        Some(token)
    }

    /// Parse an `@word` token (e.g., `@media`, `@keyframes`).
    fn at_word(&mut self) -> Option<Token<'a>> {
        let start = self.pos;
        // Scan forward from pos+1 until we hit a terminator or EOF.
        let mut next = self.pos + 1;
        while next < self.len && !is_at_end(self.bytes[next]) {
            next += 1;
        }
        // `next` is now at the terminator or EOF. The last char of the token
        // is at `next - 1`.
        let end = next - 1;

        let token = Token {
            kind: TokenType::AtWord,
            value: &self.css[start..next],
            start,
            end: Some(end),
        };
        self.pos = next;
        Some(token)
    }

    /// Parse a backslash escape sequence.
    fn backslash(&mut self) -> Option<Token<'a>> {
        let start = self.pos;
        let mut next = self.pos;
        let mut escape = true;

        // Count consecutive backslashes.
        while next + 1 < self.len && self.bytes[next + 1] == b'\\' {
            next += 1;
            escape = !escape;
        }

        if next + 1 < self.len {
            let code = self.bytes[next + 1];
            if escape
                && code != b'/'
                && !is_space(code)
            {
                next += 1;
                // Check for hex escape.
                if is_hex(self.bytes[next]) {
                    while next + 1 < self.len && is_hex(self.bytes[next + 1]) {
                        next += 1;
                    }
                    if next + 1 < self.len && self.bytes[next + 1] == b' ' {
                        next += 1;
                    }
                }
            }
        }

        let token = Token {
            kind: TokenType::Word,
            value: &self.css[start..next + 1],
            start,
            end: Some(next),
        };
        self.pos = next + 1;
        Some(token)
    }

    /// Parse a `/* ... */` comment.
    fn comment(&mut self, ignore_unclosed: bool) -> Option<Token<'a>> {
        let start = self.pos;
        // Search for `*/` starting from pos+2.
        let search_start = self.pos + 2;
        let found = if search_start < self.len {
            find_comment_end(&self.bytes[search_start..])
        } else {
            None
        };

        let next = match found {
            Some(rel) => search_start + rel + 1, // points to the `/` of `*/`
            None => {
                if self.ignore_errors || ignore_unclosed {
                    self.len - 1
                } else {
                    panic!("Unclosed comment at offset {}", self.pos);
                }
            }
        };

        let token = Token {
            kind: TokenType::Comment,
            value: &self.css[start..next + 1],
            start,
            end: Some(next),
        };
        self.pos = next + 1;
        Some(token)
    }

    /// Parse a word token (identifiers, numbers, etc.).
    fn word(&mut self) -> Option<Token<'a>> {
        let start = self.pos;
        let mut next = self.pos + 1;

        while next < self.len {
            let b = self.bytes[next];
            if is_word_end(b) {
                break;
            }
            // Handle the `\/(?=\*)` lookahead: `/` followed by `*` is a word end.
            if b == b'/' && next + 1 < self.len && self.bytes[next + 1] == b'*' {
                break;
            }
            next += 1;
        }

        let end = next - 1;
        let token = Token {
            kind: TokenType::Word,
            value: &self.css[start..next],
            start,
            end: Some(end),
        };
        // Push to buffer (for url() previous-word check).
        self.buffer.push(token.clone());
        self.pos = next;
        Some(token)
    }
}

/// Find `*/` in a byte slice, returning the offset of `*` (so `*` is at
/// `result`, `/` is at `result + 1`).
fn find_comment_end(bytes: &[u8]) -> Option<usize> {
    let mut pos = 0;
    while pos + 1 < bytes.len() {
        if let Some(rel) = memchr::memchr(b'*', &bytes[pos..]) {
            let star_pos = pos + rel;
            if star_pos + 1 < bytes.len() && bytes[star_pos + 1] == b'/' {
                return Some(star_pos);
            }
            pos = star_pos + 1;
        } else {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(css: &str) -> Vec<Token> {
        let mut t = Tokenizer::new(css, false);
        let mut tokens = Vec::new();
        while let Some(tok) = t.next_token() {
            tokens.push(tok);
        }
        tokens
    }

    #[test]
    fn test_simple_rule() {
        let tokens = tokenize("a { }");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenType::Word);
        assert_eq!(tokens[0].value, "a");
        assert_eq!(tokens[1].kind, TokenType::Space);
        assert_eq!(tokens[2].kind, TokenType::OpenCurly);
        assert_eq!(tokens[3].kind, TokenType::Space);
        assert_eq!(tokens[4].kind, TokenType::CloseCurly);
    }

    #[test]
    fn test_declaration() {
        let tokens = tokenize("color: red;");
        assert_eq!(tokens[0].kind, TokenType::Word);
        assert_eq!(tokens[0].value, "color");
        assert_eq!(tokens[1].kind, TokenType::Colon);
        assert_eq!(tokens[2].kind, TokenType::Space);
        assert_eq!(tokens[3].kind, TokenType::Word);
        assert_eq!(tokens[3].value, "red");
        assert_eq!(tokens[4].kind, TokenType::Semicolon);
    }

    #[test]
    fn test_comment() {
        let tokens = tokenize("/* hello */");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenType::Comment);
        assert_eq!(tokens[0].value, "/* hello */");
    }

    #[test]
    fn test_at_word() {
        let tokens = tokenize("@media screen");
        assert_eq!(tokens[0].kind, TokenType::AtWord);
        assert_eq!(tokens[0].value, "@media");
        assert_eq!(tokens[1].kind, TokenType::Space);
        assert_eq!(tokens[2].kind, TokenType::Word);
        assert_eq!(tokens[2].value, "screen");
    }

    #[test]
    fn test_string_single_quote() {
        let tokens = tokenize("'hello world'");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenType::String);
        assert_eq!(tokens[0].value, "'hello world'");
    }

    #[test]
    fn test_string_double_quote() {
        let tokens = tokenize("\"hello world\"");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenType::String);
        assert_eq!(tokens[0].value, "\"hello world\"");
    }

    #[test]
    fn test_escaped_string() {
        let tokens = tokenize("'it\\'s ok'");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenType::String);
        assert_eq!(tokens[0].value, "'it\\'s ok'");
    }

    #[test]
    fn test_brackets() {
        // Simple brackets with no bad chars inside
        let tokens = tokenize("calc(1 + 2)");
        // "calc" is a word, "(1 + 2)" is brackets
        assert_eq!(tokens[0].kind, TokenType::Word);
        assert_eq!(tokens[0].value, "calc");
        assert_eq!(tokens[1].kind, TokenType::Brackets);
        assert_eq!(tokens[1].value, "(1 + 2)");
    }

    #[test]
    fn test_back() {
        let mut t = Tokenizer::new("a b", false);
        let tok = t.next_token().unwrap();
        assert_eq!(tok.value, "a");
        t.back(tok.clone());
        let tok2 = t.next_token().unwrap();
        assert_eq!(tok2.value, "a");
    }

    #[test]
    fn test_backslash_escape() {
        let tokens = tokenize("\\41");
        assert_eq!(tokens[0].kind, TokenType::Word);
        assert_eq!(tokens[0].value, "\\41");
    }

    #[test]
    fn test_hex_escape_with_space() {
        let tokens = tokenize("\\41 B");
        assert_eq!(tokens[0].kind, TokenType::Word);
        // Hex escape \41 followed by space (consumed as part of escape), then B
        assert_eq!(tokens[0].value, "\\41 ");
        assert_eq!(tokens[1].kind, TokenType::Word);
        assert_eq!(tokens[1].value, "B");
    }

    #[test]
    fn test_multiple_spaces() {
        let tokens = tokenize("  \t\n  ");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenType::Space);
        assert_eq!(tokens[0].value, "  \t\n  ");
    }

    #[test]
    fn test_url_brackets() {
        // url() without quotes — should be a Brackets token
        let tokens = tokenize("url(foo.png)");
        assert_eq!(tokens[0].kind, TokenType::Word);
        assert_eq!(tokens[0].value, "url");
        assert_eq!(tokens[1].kind, TokenType::Brackets);
        assert_eq!(tokens[1].value, "(foo.png)");
    }
}
