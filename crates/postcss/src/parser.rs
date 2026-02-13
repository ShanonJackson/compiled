//! CSS parser — 1:1 port of PostCSS 8.4.31 `parser.js`.
//!
//! Converts a token stream into an arena-based AST (`Stylesheet`).
//! The critical `other()` heuristic for distinguishing rules from declarations
//! is ported verbatim, including custom property edge cases.

use smallvec::SmallVec;

use crate::ast::*;
use crate::input::Input;
use crate::raws::{RawValue, Raws};
use crate::tokenizer::{Token, TokenType, Tokenizer};

/// Characters considered "safe" neighbors for comments in value reconstruction.
/// When a comment sits between two safe neighbors, it is kept in the clean value.
/// JS: `const SAFE_COMMENT_NEIGHBOR = { empty: true, space: true }`
fn is_safe_comment_neighbor(kind: Option<TokenType>) -> bool {
    match kind {
        None => true,            // 'empty' in JS
        Some(TokenType::Space) => true,
        _ => false,
    }
}

/// Parse CSS source into a Stylesheet.
pub fn parse(input: &Input, stylesheet: &mut Stylesheet) {
    let mut parser = Parser::new(input, stylesheet);
    parser.parse();
}

struct Parser<'a> {
    input: &'a Input,
    tokenizer: Tokenizer<'a>,
    ss: &'a mut Stylesheet,
    /// The currently active container node.
    current: NodeId,
    /// Accumulated whitespace that will become the next node's `raws.before`.
    spaces: String,
    /// Whether we just saw a semicolon (for `raws.semicolon` on containers).
    semicolon: bool,
    /// Tracks if we are inside a custom property (--var) declaration.
    custom_property: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a Input, ss: &'a mut Stylesheet) -> Self {
        // Set source on root node.
        let root = ss.root;
        ss.node_mut(root).source_mut().replace(Source {
            start: Position {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: None,
            input_id: 0,
        });

        Parser {
            input,
            tokenizer: Tokenizer::new(&input.css, false),
            ss,
            current: root,
            spaces: String::new(),
            semicolon: false,
            custom_property: false,
        }
    }

    /// Main parse loop — iterate tokens and dispatch to handlers.
    fn parse(&mut self) {
        loop {
            let token = match self.tokenizer.next_token() {
                Some(t) => t,
                None => break,
            };

            match token.kind {
                TokenType::Space => {
                    self.spaces.push_str(token.value);
                }
                TokenType::Semicolon => {
                    self.free_semicolon(&token);
                }
                TokenType::CloseCurly => {
                    self.end(&token);
                }
                TokenType::Comment => {
                    self.comment(&token);
                }
                TokenType::AtWord => {
                    self.atrule(token);
                }
                TokenType::OpenCurly => {
                    self.empty_rule(token);
                }
                _ => {
                    self.other(token);
                }
            }
        }

        // End-of-file: handle remaining spaces.
        self.end_file();
    }

    // ── Node initialization ────────────────────────────────────────────────

    /// Initialize a new node: push it into the current container and set
    /// `raws.before` from accumulated spaces.
    fn init(&mut self, node_id: NodeId, _offset: usize) {
        // Set raws.before from accumulated spaces.
        {
            let node = self.ss.node_mut(node_id);
            node.raws_mut().before = Some(std::mem::take(&mut self.spaces));
            node.set_parent(Some(self.current));
        }

        // Add to current container's children.
        let current = self.ss.node_mut(self.current);
        if let Some(children) = current.children_mut() {
            children.push(node_id);
        }

        // Reset semicolon flag (unless it's a comment).
        if self.ss.node(node_id).node_type() != "comment" {
            self.semicolon = false;
        }
    }

    // ── Block closing ──────────────────────────────────────────────────────

    /// Handle `}` — close the current block.
    fn end(&mut self, token: &Token) {
        let current = self.current;

        // Set raws.semicolon if the block had a trailing semicolon.
        if self.ss.node(current).children().map_or(false, |c| !c.is_empty()) {
            self.ss.node_mut(current).raws_mut().semicolon = Some(self.semicolon);
        }
        self.semicolon = false;

        // Append accumulated spaces to raws.after.
        let spaces = std::mem::take(&mut self.spaces);
        {
            let raws = self.ss.node_mut(current).raws_mut();
            let after = raws.after.get_or_insert_with(String::new);
            after.push_str(&spaces);
        }

        // Set source end position.
        if let Some(src) = self.ss.node_mut(current).source_mut() {
            src.end = Some(Position {
                line: 0,
                column: 0,
                offset: token.start,
            });
        }

        // Pop back to parent.
        if let Some(parent) = self.ss.node(current).parent() {
            self.current = parent;
        }
        // else: unexpected close at root — in JS PostCSS this would error,
        // but we silently ignore (the JS version also has error handling here).
    }

    /// Handle end-of-file.
    fn end_file(&mut self) {
        // If we're not back at root, there are unclosed blocks.
        // JS PostCSS handles this with implicit closing.

        // Set raws.after on current node with remaining spaces.
        // Always set raws.after on the root so the stringifier doesn't
        // fall back to DEFAULT_RAW.after ("\n") for empty input.
        let spaces = std::mem::take(&mut self.spaces);
        {
            let raws = self.ss.node_mut(self.current).raws_mut();
            let after = raws.after.get_or_insert_with(String::new);
            after.push_str(&spaces);
        }

        // Set raws.semicolon if applicable.
        if self.semicolon {
            self.ss.node_mut(self.current).raws_mut().semicolon = Some(true);
        }
    }

    // ── Comment ────────────────────────────────────────────────────────────

    /// Parse a comment token into a Comment AST node.
    fn comment(&mut self, token: &Token) {
        let text_with_delims = token.value;
        // Strip /* and */
        let inner = &text_with_delims[2..text_with_delims.len() - 2];

        let (text, left, right) = if inner.chars().all(|c| c.is_whitespace()) {
            // Comment is only whitespace.
            (String::new(), inner.to_string(), String::new())
        } else {
            // Extract leading/trailing whitespace and the content.
            let trimmed_start = inner.len() - inner.trim_start().len();
            let trimmed_end = inner.len() - inner.trim_end().len();
            let left = &inner[..trimmed_start];
            let right = &inner[inner.len() - trimmed_end..];
            let text = &inner[trimmed_start..inner.len() - trimmed_end];
            (text.to_string(), left.to_string(), right.to_string())
        };

        let mut raws = Raws::default();
        raws.left = Some(left);
        raws.right = Some(right);

        let node_id = self.ss.add_node(CssNode::Comment(CommentData {
            text,
            parent: None,
            raws,
            source: Some(Source {
                start: Position {
                    line: 0,
                    column: 0,
                    offset: token.start,
                },
                end: token.end.map(|e| Position {
                    line: 0,
                    column: 0,
                    offset: e,
                }),
                input_id: 0,
            }),
            is_clean: false,
        }));

        self.init(node_id, token.start);
    }

    // ── AtRule ──────────────────────────────────────────────────────────────

    /// Parse an `@word` token into an AtRule AST node.
    fn atrule(&mut self, token: Token<'a>) {
        let name = token.value[1..].to_string(); // strip @
        let start_offset = token.start;

        let node_id = self.ss.add_node(CssNode::AtRule(AtRuleData {
            name,
            params: String::new(),
            children: None,
            parent: None,
            raws: Raws::default(),
            source: Some(Source {
                start: Position {
                    line: 0,
                    column: 0,
                    offset: start_offset,
                },
                end: None,
                input_id: 0,
            }),
            is_clean: false,
        }));

        self.init(node_id, start_offset);

        // Collect params tokens.
        let mut params: Vec<Token<'a>> = Vec::new();
        let mut brackets: Vec<u8> = Vec::new();
        let mut open = false;
        let mut last_token_end: Option<usize> = None;

        loop {
            let tok = match self.tokenizer.next_token() {
                Some(t) => t,
                None => break,
            };

            match tok.kind {
                TokenType::OpenParen | TokenType::OpenSquare => {
                    brackets.push(if tok.kind == TokenType::OpenParen {
                        b')'
                    } else {
                        b']'
                    });
                    params.push(tok);
                }
                TokenType::OpenCurly if !brackets.is_empty() => {
                    brackets.push(b'}');
                    params.push(tok);
                }
                TokenType::CloseParen | TokenType::CloseSquare | TokenType::CloseCurly
                    if !brackets.is_empty()
                        && tok.value.as_bytes()[0] == *brackets.last().unwrap() =>
                {
                    brackets.pop();
                    params.push(tok);
                }
                TokenType::Semicolon if brackets.is_empty() => {
                    // Semicolon-terminated at-rule (no block).
                    last_token_end = Some(tok.start);
                    self.semicolon = true;
                    break;
                }
                TokenType::OpenCurly if brackets.is_empty() => {
                    // Block at-rule.
                    open = true;
                    break;
                }
                TokenType::CloseCurly if brackets.is_empty() => {
                    // Unmatched } — end the at-rule and process the }.
                    // Extract end position from last param.
                    if let Some(last) = params.last() {
                        last_token_end = last.end.or(Some(last.start));
                    }
                    self.end(&tok);
                    break;
                }
                _ => {
                    params.push(tok);
                }
            }
        }

        // Extract raws.between (trailing spaces/comments from params).
        let between = self.spaces_and_comments_from_end(&mut params);
        self.ss.node_mut(node_id).raws_mut().between = Some(between);

        if !params.is_empty() {
            // raws.afterName = leading spaces/comments.
            let after_name = self.spaces_and_comments_from_start(&mut params);
            self.ss.node_mut(node_id).raws_mut().after_name = Some(after_name);

            // Set params using raw reconstruction.
            self.set_raw(node_id, "params", &params);

            // Set source end from last param.
            if let Some(last) = params.last() {
                let end_offset = last.end.unwrap_or(last.start);
                if let CssNode::AtRule(d) = self.ss.node_mut(node_id) {
                    if let Some(ref mut src) = d.source {
                        src.end = Some(Position {
                            line: 0,
                            column: 0,
                            offset: end_offset,
                        });
                    }
                }
            }

            // If not opening a block, move trailing between to spaces.
            if !open {
                let raws = self.ss.node_mut(node_id).raws_mut();
                if let Some(ref between) = raws.between {
                    self.spaces = between.clone();
                    raws.between = Some(String::new());
                }
            }
        } else {
            self.ss.node_mut(node_id).raws_mut().after_name = Some(String::new());
            if let CssNode::AtRule(d) = self.ss.node_mut(node_id) {
                d.params = String::new();
            }
        }

        if let Some(end) = last_token_end {
            if let CssNode::AtRule(d) = self.ss.node_mut(node_id) {
                if let Some(ref mut src) = d.source {
                    src.end = Some(Position {
                        line: 0,
                        column: 0,
                        offset: end,
                    });
                }
            }
        }

        if open {
            // Initialize children array and set as current.
            self.ss.node_mut(node_id).ensure_children();
            self.current = node_id;
        }
    }

    // ── Rule ───────────────────────────────────────────────────────────────

    /// Create a Rule node from accumulated tokens (called when `{` is found
    /// at bracket-depth 0 in `other()`).
    fn rule(&mut self, tokens: &[Token]) {
        // Extract raws.between (trailing spaces/comments before `{`).
        let mut tokens_vec: Vec<Token> = tokens.to_vec();

        // Pop the `{` token.
        tokens_vec.pop();

        let between = self.spaces_and_comments_from_end(&mut tokens_vec);

        let start_offset = tokens_vec.first().map(|t| t.start).unwrap_or(0);

        let node_id = self.ss.add_node(CssNode::Rule(RuleData {
            selector: String::new(),
            children: SmallVec::new(),
            parent: None,
            raws: Raws::default(),
            source: Some(Source {
                start: Position {
                    line: 0,
                    column: 0,
                    offset: start_offset,
                },
                end: None,
                input_id: 0,
            }),
            is_clean: false,
        }));

        self.init(node_id, start_offset);

        self.ss.node_mut(node_id).raws_mut().between = Some(between);

        // Set selector using raw reconstruction.
        self.set_raw(node_id, "selector", &tokens_vec);

        self.current = node_id;
    }

    /// Handle empty rule (`{ }` without selector tokens).
    fn empty_rule(&mut self, token: Token<'a>) {
        let node_id = self.ss.add_node(CssNode::Rule(RuleData {
            selector: String::new(),
            children: SmallVec::new(),
            parent: None,
            raws: Raws {
                between: Some(String::new()),
                ..Raws::default()
            },
            source: Some(Source {
                start: Position {
                    line: 0,
                    column: 0,
                    offset: token.start,
                },
                end: None,
                input_id: 0,
            }),
            is_clean: false,
        }));

        self.init(node_id, token.start);
        self.current = node_id;
    }

    // ── Declaration ────────────────────────────────────────────────────────

    /// Parse a declaration from accumulated tokens.
    fn decl(&mut self, tokens: &[Token], custom_property: bool) {
        let mut tokens_vec: Vec<Token> = tokens.to_vec();

        let start_offset = tokens_vec.first().map(|t| t.start).unwrap_or(0);

        // Find end position from last token (before popping semicolon).
        let last_token = tokens_vec.last().cloned();

        // Create node and call init FIRST (matching JS PostCSS order).
        // init() resets self.semicolon, so we must detect semicolon AFTER.
        let node_id = self.ss.add_node(CssNode::Declaration(DeclData {
            prop: String::new(),
            value: String::new(),
            important: false,
            parent: None,
            raws: Raws::default(),
            source: Some(Source {
                start: Position {
                    line: 0,
                    column: 0,
                    offset: start_offset,
                },
                end: None,
                input_id: 0,
            }),
            is_clean: false,
        }));

        self.init(node_id, start_offset);

        // Check for trailing semicolon AFTER init (init resets self.semicolon).
        // This matches JS PostCSS order: init() → semicolon detection.
        let has_semicolon = tokens_vec
            .last()
            .map_or(false, |t| t.kind == TokenType::Semicolon);
        if has_semicolon {
            self.semicolon = true;
            tokens_vec.pop();
        }

        // Set source end position.
        {
            let end_offset = tokens_vec
                .last()
                .and_then(|t| t.end.or(Some(t.start)))
                .or_else(|| last_token.as_ref().and_then(|t| t.end.or(Some(t.start))))
                .unwrap_or(start_offset);
            if let Some(src) = self.ss.node_mut(node_id).source_mut() {
                src.end = Some(Position {
                    line: 0,
                    column: 0,
                    offset: end_offset,
                });
            }
        }

        // Skip leading non-word tokens, appending them to raws.before.
        while !tokens_vec.is_empty() && tokens_vec[0].kind != TokenType::Word {
            let t = tokens_vec.remove(0);
            let raws = self.ss.node_mut(node_id).raws_mut();
            let before = raws.before.get_or_insert_with(String::new);
            before.push_str(t.value);
        }

        if tokens_vec.is_empty() {
            return; // should not happen with valid CSS
        }

        // Extract property name: consume tokens until `:`, space, or comment.
        let mut prop = String::new();
        while !tokens_vec.is_empty() {
            let kind = tokens_vec[0].kind;
            if kind == TokenType::Colon || kind == TokenType::Space || kind == TokenType::Comment {
                break;
            }
            prop.push_str(tokens_vec.remove(0).value);
        }

        // Extract raws.between (tokens between prop and `:`, inclusive of `:`).
        let mut between = String::new();
        while !tokens_vec.is_empty() {
            let t = tokens_vec.remove(0);
            if t.kind == TokenType::Colon {
                between.push_str(t.value);
                break;
            }
            // If it's a word token with word chars, that's an error in JS —
            // we just include it in between for robustness.
            between.push_str(t.value);
        }

        // Handle vendor prefix hacks: `_prop` or `*prop` → move prefix to raws.before.
        if !prop.is_empty() && (prop.starts_with('_') || prop.starts_with('*')) {
            let prefix = &prop[..1];
            let raws = self.ss.node_mut(node_id).raws_mut();
            let before = raws.before.get_or_insert_with(String::new);
            before.push_str(prefix);
            prop = prop[1..].to_string();
        }

        // Set the prop.
        if let CssNode::Declaration(d) = self.ss.node_mut(node_id) {
            d.prop = prop;
        }

        // Collect leading spaces/comments from value tokens.
        let mut first_spaces: Vec<Token> = Vec::new();
        while !tokens_vec.is_empty() {
            let kind = tokens_vec[0].kind;
            if kind != TokenType::Space && kind != TokenType::Comment {
                break;
            }
            first_spaces.push(tokens_vec.remove(0));
        }

        // Check for !important (backward scan from end).
        let mut important = false;
        let mut important_raw: Option<String> = None;

        // Walk backward from end, skipping trailing space/comment tokens.
        let mut i = tokens_vec.len();
        while i > 0 {
            i -= 1;
            let val_lower = tokens_vec[i].value.to_lowercase();

            if val_lower == "!important" {
                important = true;
                // Collect raw string from position i to end.
                let mut raw_str = String::new();
                // Also collect trailing spaces that were after !important.
                let remaining: String = tokens_vec[i..].iter().map(|t| t.value).collect();
                raw_str.push_str(&remaining);
                tokens_vec.truncate(i);
                // Collect trailing spaces from truncated tokens.
                let trailing = self.spaces_from_end_str(&mut tokens_vec);
                let full = format!("{}{}", trailing, raw_str);
                if full != " !important" {
                    important_raw = Some(full);
                }
                break;
            } else if val_lower == "important" {
                // Check for separated `!` + `important`.
                let mut found_bang = false;
                let mut j = i;
                let mut raw_parts = String::new();
                while j > 0 {
                    j -= 1;
                    let prev_val = tokens_vec[j].value.trim();
                    if prev_val == "!" {
                        found_bang = true;
                        // Collect raw from j to end.
                        raw_parts =
                            tokens_vec[j..=i].iter().map(|t| t.value).collect();
                        tokens_vec.truncate(j);
                        break;
                    }
                    if tokens_vec[j].kind != TokenType::Space {
                        break;
                    }
                }
                if found_bang {
                    important = true;
                    if raw_parts != " !important" {
                        important_raw = Some(raw_parts);
                    }
                }
                break;
            }

            if tokens_vec[i].kind != TokenType::Space && tokens_vec[i].kind != TokenType::Comment {
                break;
            }
        }

        if important {
            if let CssNode::Declaration(d) = self.ss.node_mut(node_id) {
                d.important = true;
            }
            if let Some(raw) = important_raw {
                self.ss.node_mut(node_id).raws_mut().important = Some(raw);
            }
        }

        // Check if remaining tokens have any non-space/non-comment content.
        let has_word = tokens_vec
            .iter()
            .any(|t| t.kind != TokenType::Space && t.kind != TokenType::Comment);
        if has_word {
            // Merge first_spaces into between.
            let leading: String = first_spaces.iter().map(|t| t.value).collect();
            between.push_str(&leading);
            first_spaces.clear();
        }

        self.ss.node_mut(node_id).raws_mut().between = Some(between);

        // Set value using raw reconstruction.
        let mut all_value_tokens: Vec<Token> = first_spaces;
        all_value_tokens.extend(tokens_vec);
        self.set_raw_value(node_id, &all_value_tokens, custom_property);
    }

    // ── The critical other() heuristic ─────────────────────────────────────

    /// Dispatches accumulated tokens to either `rule()` or `decl()`.
    ///
    /// This is the most critical parsing decision in PostCSS — determining
    /// whether a sequence of tokens forms a CSS rule (selector + block) or
    /// a CSS declaration (property: value).
    fn other(&mut self, start: Token<'a>) {
        let mut end = false;
        let mut colon = false;
        let mut bracket: Option<Token<'a>> = None;
        let mut brackets: Vec<u8> = Vec::new();
        let custom_property = start.value.starts_with("--");
        let mut tokens: Vec<Token<'a>> = Vec::new();
        let mut token = start;

        loop {
            tokens.push(token.clone());

            match token.kind {
                TokenType::OpenParen | TokenType::OpenSquare => {
                    if bracket.is_none() {
                        bracket = Some(token.clone());
                    }
                    brackets.push(if token.kind == TokenType::OpenParen {
                        b')'
                    } else {
                        b']'
                    });
                }
                TokenType::OpenCurly if custom_property && colon => {
                    // Custom properties can contain `{` in values.
                    if bracket.is_none() {
                        bracket = Some(token.clone());
                    }
                    brackets.push(b'}');
                }
                _ if !brackets.is_empty() => {
                    let expected = *brackets.last().unwrap();
                    if token.value.as_bytes().first() == Some(&expected) {
                        brackets.pop();
                        if brackets.is_empty() {
                            bracket = None;
                        }
                    }
                }
                TokenType::Semicolon if brackets.is_empty() => {
                    if colon {
                        self.decl(&tokens, custom_property);
                        return;
                    } else {
                        break;
                    }
                }
                TokenType::OpenCurly if brackets.is_empty() => {
                    self.rule(&tokens);
                    return;
                }
                TokenType::CloseCurly if brackets.is_empty() => {
                    self.tokenizer.back(tokens.pop().unwrap());
                    end = true;
                    break;
                }
                TokenType::Colon if brackets.is_empty() => {
                    colon = true;
                }
                _ => {}
            }

            match self.tokenizer.next_token() {
                Some(t) => token = t,
                None => {
                    end = true;
                    break;
                }
            }
        }

        if end && colon {
            if !custom_property {
                // Pop trailing space/comment tokens.
                while !tokens.is_empty() {
                    let last_kind = tokens.last().unwrap().kind;
                    if last_kind != TokenType::Space && last_kind != TokenType::Comment {
                        break;
                    }
                    self.tokenizer.back(tokens.pop().unwrap());
                }
            }
            self.decl(&tokens, custom_property);
        }
        // else: unknownWord — we silently skip for robustness.
    }

    // ── Standalone semicolon ───────────────────────────────────────────────

    /// Handle a standalone `;` token. In JS PostCSS, this sets
    /// `raws.ownSemicolon` on the previous rule if it directly follows `}`.
    fn free_semicolon(&mut self, token: &Token) {
        self.spaces.push_str(token.value);

        // Check if the previous child of current container is a rule —
        // if so, attach accumulated spaces (which include the `;`) as ownSemicolon.
        let current = self.current;
        if let Some(children) = self.ss.node(current).children() {
            if let Some(&last_child_id) = children.last() {
                let last_child = self.ss.node(last_child_id);
                if last_child.node_type() == "rule"
                    && last_child.raws().own_semicolon.is_none()
                {
                    let raws = self.ss.node_mut(last_child_id).raws_mut();
                    raws.own_semicolon = Some(std::mem::take(&mut self.spaces));
                }
            }
        }
    }

    // ── Raw value reconstruction ───────────────────────────────────────────

    /// Reconstruct a value from tokens, detecting whether the result is "clean"
    /// (no comments or trailing whitespace that differ from the raw).
    ///
    /// If dirty, stores both `raw` and `value` in `raws[prop]`.
    /// Corresponds to the parser's `raw()` method in JS (not the stringifier's).
    fn set_raw(&mut self, node_id: NodeId, prop: &str, tokens: &[Token]) {
        let mut value = String::new();
        let mut clean = true;

        for (i, token) in tokens.iter().enumerate() {
            match token.kind {
                TokenType::Space if i == tokens.len() - 1 => {
                    clean = false;
                }
                TokenType::Comment => {
                    let prev = if i > 0 {
                        Some(tokens[i - 1].kind)
                    } else {
                        None
                    };
                    let next = if i + 1 < tokens.len() {
                        Some(tokens[i + 1].kind)
                    } else {
                        None
                    };

                    if !is_safe_comment_neighbor(prev) && !is_safe_comment_neighbor(next) {
                        // Check if value ends with `,` — if so, mark dirty.
                        if value.ends_with(',') {
                            clean = false;
                        } else {
                            value.push_str(token.value);
                        }
                    } else {
                        clean = false;
                    }
                }
                _ => {
                    value.push_str(token.value);
                }
            }
        }

        if !clean {
            let raw: String = tokens.iter().map(|t| t.value).collect();
            match prop {
                "selector" => {
                    if let CssNode::Rule(d) = self.ss.node_mut(node_id) {
                        d.selector = value.clone();
                        d.raws.selector = Some(RawValue { raw, value });
                    }
                }
                "params" => {
                    if let CssNode::AtRule(d) = self.ss.node_mut(node_id) {
                        d.params = value.clone();
                        d.raws.params = Some(RawValue { raw, value });
                    }
                }
                _ => {}
            }
        } else {
            match prop {
                "selector" => {
                    if let CssNode::Rule(d) = self.ss.node_mut(node_id) {
                        d.selector = value;
                    }
                }
                "params" => {
                    if let CssNode::AtRule(d) = self.ss.node_mut(node_id) {
                        d.params = value;
                    }
                }
                _ => {}
            }
        }
    }

    /// Set the declaration value from tokens, with raw preservation.
    fn set_raw_value(&mut self, node_id: NodeId, tokens: &[Token], custom_property: bool) {
        let mut value = String::new();
        let mut clean = true;

        for (i, token) in tokens.iter().enumerate() {
            match token.kind {
                TokenType::Space if i == tokens.len() - 1 && !custom_property => {
                    clean = false;
                }
                TokenType::Comment => {
                    let prev = if i > 0 {
                        Some(tokens[i - 1].kind)
                    } else {
                        None
                    };
                    let next = if i + 1 < tokens.len() {
                        Some(tokens[i + 1].kind)
                    } else {
                        None
                    };

                    if !is_safe_comment_neighbor(prev) && !is_safe_comment_neighbor(next) {
                        if value.ends_with(',') {
                            clean = false;
                        } else {
                            value.push_str(token.value);
                        }
                    } else {
                        clean = false;
                    }
                }
                _ => {
                    value.push_str(token.value);
                }
            }
        }

        if let CssNode::Declaration(d) = self.ss.node_mut(node_id) {
            d.value = value.clone();
            if !clean {
                let raw: String = tokens.iter().map(|t| t.value).collect();
                d.raws.value = Some(RawValue { raw, value });
            }
        }
    }

    // ── Token utility methods ──────────────────────────────────────────────

    /// Pop trailing space and comment tokens from the end of a token array,
    /// returning them as a concatenated string.
    fn spaces_and_comments_from_end(&self, tokens: &mut Vec<Token>) -> String {
        let mut result = String::new();
        while let Some(last) = tokens.last() {
            if last.kind == TokenType::Space || last.kind == TokenType::Comment {
                result.insert_str(0, last.value);
                tokens.pop();
            } else {
                break;
            }
        }
        result
    }

    /// Shift leading space and comment tokens from the start of a token array,
    /// returning them as a concatenated string.
    fn spaces_and_comments_from_start(&self, tokens: &mut Vec<Token>) -> String {
        let mut result = String::new();
        while !tokens.is_empty() {
            let kind = tokens[0].kind;
            if kind == TokenType::Space || kind == TokenType::Comment {
                result.push_str(tokens.remove(0).value);
            } else {
                break;
            }
        }
        result
    }

    /// Pop trailing space tokens from a token array and return as string.
    fn spaces_from_end_str(&self, tokens: &mut Vec<Token>) -> String {
        let mut result = String::new();
        while let Some(last) = tokens.last() {
            if last.kind == TokenType::Space {
                result.insert_str(0, last.value);
                tokens.pop();
            } else {
                break;
            }
        }
        result
    }
}
