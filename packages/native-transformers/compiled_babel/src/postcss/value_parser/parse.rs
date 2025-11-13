use super::{Node, ParsedValue};

pub fn parse(input: &str) -> ParsedValue {
    // Minimal port of postcss-value-parser parse.js sufficient for cssnano plugins.
    let mut tokens: Vec<Node> = Vec::new();
    let mut value = input.to_string();
    let mut pos: usize = 0;
    let max = value.len();
    let bytes: Vec<u8> = value.as_bytes().to_vec();
    let mut stack: Vec<Vec<Node>> = vec![Vec::new()];
    let mut balanced: i32 = 0;

    while pos < max {
        let code = bytes[pos];
        // whitespace (<= 32)
        if code <= 32 {
            let start = pos;
            let mut next = pos;
            while next < max && bytes[next] <= 32 { next += 1; }
            let token = String::from_utf8(bytes[start..next].to_vec()).unwrap_or_default();
            stack.last_mut().unwrap().push(Node::Space { value: token });
            pos = next;
            continue;
        }

        // comments /* ... */
        if code == b'/' && pos + 1 < max && bytes[pos + 1] == b'*' {
            let start = pos + 2;
            let mut end = start;
            let mut unclosed = false;
            while end + 1 < max && !(bytes[end] == b'*' && bytes[end + 1] == b'/') {
                end += 1;
            }
            if end + 1 >= max { unclosed = true; }
            let val = if unclosed {
                String::from_utf8(bytes[start..max].to_vec()).unwrap_or_default()
            } else {
                String::from_utf8(bytes[start..end].to_vec()).unwrap_or_default()
            };
            stack.last_mut().unwrap().push(Node::Comment { value: val, unclosed });
            pos = if unclosed { max } else { end + 2 };
            continue;
        }

        // strings '...' or "..."
        if code == b'\'' || code == b'"' {
            let quote = code as char;
            let mut next = pos;
            let mut unclosed = false;
            loop {
                next = match value[next + 1..].find(quote) { Some(o) => next + 1 + o, None => { unclosed = true; max - 1 } };
                // check escapes
                let mut escape = false; let mut escape_pos = next;
                while escape_pos > 0 && bytes[escape_pos - 1] == b'\\' { escape = !escape; escape_pos -= 1; }
                if !escape { break; }
                if next + 1 >= max { unclosed = true; break; }
            }
            let val = if pos + 1 <= next { String::from_utf8(bytes[pos + 1..next].to_vec()).unwrap_or_default() } else { String::new() };
            stack.last_mut().unwrap().push(Node::String { value: val, quote, unclosed });
            pos = if unclosed { max } else { next + 1 };
            continue;
        }

        // functions name(...)
        if is_ident_start(code) {
            // read word or function
            let start = pos;
            let mut end = pos;
            while end < max && is_ident_continue(bytes[end]) { end += 1; }
            let name = String::from_utf8(bytes[start..end].to_vec()).unwrap_or_default();
            if end < max && bytes[end] == b'(' {
                // parse function
                let mut fn_nodes: Vec<Node> = Vec::new();
                balanced += 1;
                // consume '('
                end += 1;
                let inner_start = end;
                // naive: find matching ')', non-nested handling by recursion on commas and simple strings/comments
                let mut depth = 1usize; let mut i = end;
                while i < max && depth > 0 {
                    let c = bytes[i];
                    if c == b'(' { depth += 1; }
                    else if c == b')' { depth -= 1; if depth == 0 { break; } }
                    i += 1;
                }
                let inner_end = i;
                let inner = if inner_start <= inner_end { String::from_utf8(bytes[inner_start..inner_end].to_vec()).unwrap_or_default() } else { String::new() };
                let parsed_inner = parse(&inner); // recurse
                fn_nodes = parsed_inner.nodes;
                stack.last_mut().unwrap().push(Node::Function { value: name, nodes: fn_nodes, before: String::new(), after: String::new(), unclosed: inner_end >= max });
                pos = if inner_end < max { inner_end + 1 } else { max };
                balanced -= 1;
                continue;
            } else {
                stack.last_mut().unwrap().push(Node::Word { value: name });
                pos = end;
                continue;
            }
        }

        // div punctuation: comma, colon, slash
        if code == b',' || code == b':' || code == b'/' {
            let val = (code as char).to_string();
            stack.last_mut().unwrap().push(Node::Div { value: val, before: String::new(), after: String::new() });
            pos += 1;
            continue;
        }

        // default: read as word until whitespace or punctuation
        let start = pos;
        let mut end = pos;
        while end < max {
            let c = bytes[end];
            if c <= 32 || c == b',' || c == b':' || c == b'/' || c == b'(' || c == b')' || c == b'\'' || c == b'"' { break; }
            end += 1;
        }
        let word = String::from_utf8(bytes[start..end].to_vec()).unwrap_or_default();
        stack.last_mut().unwrap().push(Node::Word { value: word });
        pos = end;
    }

    tokens = stack.pop().unwrap_or_default();
    ParsedValue { nodes: tokens }
}

fn is_ident_start(c: u8) -> bool {
    (c >= b'a' && c <= b'z') || (c >= b'A' && c <= b'Z') || c == b'_' || c == b'-'
}

fn is_ident_continue(c: u8) -> bool {
    is_ident_start(c) || (c >= b'0' && c <= b'9')
}

