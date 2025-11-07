use super::ast::*;
use super::stringify::stringify_nodes;

#[derive(Clone, Debug, Default)]
pub struct ParsedValue {
    nodes: Vec<Node>,
}

impl ParsedValue {
    pub fn new(nodes: Vec<Node>) -> Self {
        Self { nodes }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn to_string(&self) -> String {
        stringify_nodes(&self.nodes)
    }

    pub fn walk<F>(&self, mut callback: F)
    where
        F: FnMut(&Node) -> bool,
    {
        walk_nodes(&self.nodes, &mut callback);
    }
}

fn walk_nodes<F>(nodes: &[Node], callback: &mut F)
where
    F: FnMut(&Node) -> bool,
{
    for node in nodes {
        let should_descend = callback(node);
        if should_descend {
            if let NodeData::Function(func) = &*node.borrow() {
                walk_nodes(&func.nodes, callback);
            }
        }
    }
}

const OPEN_PAREN: u8 = b'(';
const CLOSE_PAREN: u8 = b')';
const SINGLE_QUOTE: u8 = b'\'';
const DOUBLE_QUOTE: u8 = b'"';
const BACKSLASH: u8 = b'\\';
const SLASH: u8 = b'/';
const COMMA: u8 = b',';
const COLON: u8 = b':';
const STAR: u8 = b'*';
const U_LOWER: u8 = b'u';
const U_UPPER: u8 = b'U';
const PLUS: u8 = b'+';

pub fn parse_value(input: &str) -> ParsedValue {
    Parser::new(input).parse()
}

struct Context {
    nodes: Vec<Node>,
    function: Option<Node>,
    after: String,
}

struct Parser<'a> {
    value: &'a str,
    pos: usize,
    max: usize,
    stack: Vec<Context>,
    before: String,
    name: String,
}

impl<'a> Parser<'a> {
    fn new(value: &'a str) -> Self {
        Self {
            value,
            pos: 0,
            max: value.len(),
            stack: vec![Context {
                nodes: Vec::new(),
                function: None,
                after: String::new(),
            }],
            before: String::new(),
            name: String::new(),
        }
    }

    fn parse(mut self) -> ParsedValue {
        while self.pos < self.max {
            let code = self.char_code_at(self.pos);
            if code <= 32 {
                self.consume_whitespace();
            } else if code == SINGLE_QUOTE || code == DOUBLE_QUOTE {
                self.consume_string(code);
            } else if code == SLASH && self.peek_char(self.pos + 1) == Some(STAR) {
                self.consume_comment();
            } else if self.is_calc_operator(code) {
                self.consume_calc_operator(code);
            } else if code == SLASH || code == COMMA || code == COLON {
                self.consume_divider(code);
            } else if code == OPEN_PAREN {
                self.consume_open_paren();
            } else if code == CLOSE_PAREN && self.stack.len() > 1 {
                self.consume_close_paren();
            } else {
                self.consume_word();
            }
        }

        for context in &mut self.stack[1..] {
            context.after.clear();
            if let Some(function) = &context.function {
                if let NodeData::Function(func) = &mut *function.borrow_mut() {
                    func.unclosed = true;
                    func.source_end_index = self.value.len();
                }
            }
        }

        let nodes = self
            .stack
            .into_iter()
            .next()
            .map(|ctx| ctx.nodes)
            .unwrap_or_default();

        ParsedValue::new(nodes)
    }

    fn current_context(&mut self) -> &mut Context {
        self.stack
            .last_mut()
            .expect("parser stack should always contain a context")
    }

    fn parent_function_name(&self) -> Option<String> {
        self.stack
            .last()
            .and_then(|ctx| ctx.function.as_ref())
            .and_then(|node| match &*node.borrow() {
                NodeData::Function(func) => Some(func.value.clone()),
                _ => None,
            })
    }

    fn char_code_at(&self, index: usize) -> u8 {
        self.value.as_bytes()[index]
    }

    fn peek_char(&self, index: usize) -> Option<u8> {
        if index < self.max {
            Some(self.value.as_bytes()[index])
        } else {
            None
        }
    }

    fn consume_whitespace(&mut self) {
        let mut next = self.pos;
        let mut code = self.char_code_at(next);
        while code <= 32 && next < self.max {
            next += 1;
            if next >= self.max {
                break;
            }
            code = self.char_code_at(next);
        }

        let token = &self.value[self.pos..next];
        let next_code = if next < self.max {
            self.char_code_at(next)
        } else {
            0
        };

        let stack_len = self.stack.len();

        if next_code == CLOSE_PAREN && stack_len > 1 {
            self.current_context().after = token.to_string();
        } else if self.extend_previous_div(token) {
        } else if next_code == COMMA
            || next_code == COLON
            || (next_code == SLASH && !self.is_comment_start(next) && !self.is_calc_function())
        {
            self.before = token.to_string();
        } else {
            let node = new_node(NodeData::Space(SpaceNode {
                value: token.to_string(),
                source_index: self.pos,
                source_end_index: next,
            }));
            self.current_context().nodes.push(node);
        }

        self.pos = next;
    }

    fn extend_previous_div(&mut self, token: &str) -> bool {
        if let Some(node) = self.current_context().nodes.last() {
            if let NodeData::Div(div) = &mut *node.borrow_mut() {
                div.after.push_str(token);
                div.source_end_index += token.len();
                return true;
            }
        }
        false
    }

    fn consume_string(&mut self, quote: u8) {
        let mut next = self.pos;
        let mut unclosed = false;
        let quote_char = quote as char;
        loop {
            let mut escape = false;
            next = match self.value[next + 1..].find(quote_char) {
                Some(offset) => next + 1 + offset,
                None => {
                    unclosed = true;
                    self.max - 1
                }
            };

            let mut escape_pos = next;
            while escape_pos > self.pos && self.value.as_bytes()[escape_pos - 1] == BACKSLASH {
                escape = !escape;
                escape_pos -= 1;
            }

            if !escape {
                break;
            }
        }

        let value_start = self.pos + 1;
        let value_end = if unclosed { next + 1 } else { next };
        let node = new_node(NodeData::String(StringNode {
            value: self.value[value_start..value_end].to_string(),
            quote: Some(quote_char),
            unclosed,
            source_index: self.pos,
            source_end_index: if unclosed { next + 1 } else { next + 1 },
        }));
        self.current_context().nodes.push(node);
        self.pos = next + 1;
    }

    fn consume_comment(&mut self) {
        let start = self.pos;
        if let Some(end) = self.value[self.pos + 2..].find("*/") {
            let absolute_end = self.pos + 2 + end;
            let node = new_node(NodeData::Comment(CommentNode {
                value: self.value[self.pos + 2..absolute_end].to_string(),
                unclosed: false,
                source_index: start,
                source_end_index: absolute_end + 2,
            }));
            self.current_context().nodes.push(node);
            self.pos = absolute_end + 2;
        } else {
            let node = new_node(NodeData::Comment(CommentNode {
                value: self.value[self.pos + 2..].to_string(),
                unclosed: true,
                source_index: start,
                source_end_index: self.max,
            }));
            self.current_context().nodes.push(node);
            self.pos = self.max;
        }
    }

    fn consume_calc_operator(&mut self, code: u8) {
        let ch = code as char;
        let node = new_node(NodeData::Word(WordNode {
            value: ch.to_string(),
            source_index: self.pos,
            source_end_index: self.pos + 1,
        }));
        self.current_context().nodes.push(node);
        self.pos += 1;
    }

    fn consume_divider(&mut self, code: u8) {
        let value = (code as char).to_string();
        let before = std::mem::take(&mut self.before);
        let node = new_node(NodeData::Div(DivNode {
            value,
            before,
            after: String::new(),
            source_index: self.pos,
            source_end_index: self.pos + 1,
        }));
        self.current_context().nodes.push(node);
        self.pos += 1;
    }

    fn consume_open_paren(&mut self) {
        let mut next = self.pos;
        while next + 1 < self.max && self.char_code_at(next + 1) <= 32 {
            next += 1;
        }
        let before = if next > self.pos {
            self.value[self.pos + 1..=next].to_string()
        } else {
            String::new()
        };

        let function_start = self.pos - self.name.len();
        let name = std::mem::take(&mut self.name);
        self.pos = next + 1;
        let next_code = self.peek_char(self.pos);

        if name.eq_ignore_ascii_case("url")
            && next_code.is_some()
            && next_code != Some(SINGLE_QUOTE)
            && next_code != Some(DOUBLE_QUOTE)
        {
            self.consume_url_function(function_start, name, before);
        } else {
            let function = new_node(NodeData::Function(FunctionNode {
                value: name,
                before,
                after: String::new(),
                nodes: Vec::new(),
                unclosed: false,
                source_index: function_start,
                source_end_index: self.pos,
            }));
            self.current_context().nodes.push(function.clone());
            self.stack.push(Context {
                nodes: Vec::new(),
                function: Some(function),
                after: String::new(),
            });
        }
    }

    fn consume_url_function(&mut self, start: usize, name: String, before: String) {
        let mut next = self.pos - 1;
        let mut unclosed = false;
        loop {
            let mut escape = false;
            if let Some(pos) = self.value[next + 1..].find(')') {
                next = next + 1 + pos;
            } else {
                next = self.max - 1;
                unclosed = true;
            }

            let mut escape_pos = next;
            while escape_pos > self.pos && self.value.as_bytes()[escape_pos - 1] == BACKSLASH {
                escape = !escape;
                escape_pos -= 1;
            }

            if !escape {
                break;
            }
        }

        let mut whitespace_pos = next;
        while whitespace_pos > self.pos && self.value.as_bytes()[whitespace_pos - 1] <= 32 {
            whitespace_pos -= 1;
        }

        let mut nodes = Vec::new();
        if self.pos < whitespace_pos {
            nodes.push(new_node(NodeData::Word(WordNode {
                value: self.value[self.pos..whitespace_pos].to_string(),
                source_index: self.pos,
                source_end_index: whitespace_pos,
            })));
        }

        let after = if whitespace_pos < next {
            self.value[whitespace_pos..next].to_string()
        } else {
            String::new()
        };

        let function = new_node(NodeData::Function(FunctionNode {
            value: name,
            before,
            after,
            nodes,
            unclosed,
            source_index: start,
            source_end_index: if unclosed { next + 1 } else { next + 1 },
        }));
        self.current_context().nodes.push(function);
        self.pos = next + 1;
    }

    fn consume_close_paren(&mut self) {
        self.pos += 1;
        let mut context = self.stack.pop().expect("expected function context");
        if let Some(function_node) = context.function.take() {
            if let NodeData::Function(func) = &mut *function_node.borrow_mut() {
                func.nodes = context.nodes;
                func.after = context.after;
                func.source_end_index = self.pos;
            }
            self.current_context().nodes.push(function_node);
            self.current_context().nodes.pop();
        }
    }

    fn consume_word(&mut self) {
        let start = self.pos;
        let mut next = self.pos;
        while next < self.max {
            let code = self.char_code_at(next);
            if code == BACKSLASH {
                next += 2;
                continue;
            }
            if code <= 32
                || code == SINGLE_QUOTE
                || code == DOUBLE_QUOTE
                || code == COMMA
                || code == COLON
                || code == SLASH
                || code == OPEN_PAREN
                || (code == STAR && self.is_calc_function())
                || (code == CLOSE_PAREN && self.stack.len() > 1)
            {
                break;
            }
            next += 1;
        }

        let token = &self.value[start..next];
        let next_code = self.peek_char(next);
        if next_code == Some(OPEN_PAREN) {
            self.name = token.to_string();
        } else if token.len() > 2
            && (token.as_bytes()[0] == U_LOWER || token.as_bytes()[0] == U_UPPER)
            && token.as_bytes()[1] == PLUS
            && token[2..]
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() || ch == '-' || ch == '?')
        {
            let node = new_node(NodeData::UnicodeRange(UnicodeRangeNode {
                value: token.to_string(),
                source_index: start,
                source_end_index: next,
            }));
            self.current_context().nodes.push(node);
        } else {
            let node = new_node(NodeData::Word(WordNode {
                value: token.to_string(),
                source_index: start,
                source_end_index: next,
            }));
            self.current_context().nodes.push(node);
        }
        self.pos = next;
    }

    fn is_comment_start(&self, index: usize) -> bool {
        self.peek_char(index + 1) == Some(STAR)
    }

    fn is_calc_function(&self) -> bool {
        matches!(self.parent_function_name().as_deref(), Some("calc"))
    }

    fn is_calc_operator(&self, code: u8) -> bool {
        (code == SLASH || code == STAR) && self.is_calc_function()
    }
}
