fn is_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\t' | '\r' | '\x0C')
}

pub fn minify_value_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_single = false;
    let mut in_double = false;
    let mut escape_next = false;
    let mut paren_depth = 0usize;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if escape_next {
            out.push(ch);
            escape_next = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escape_next = true;
            out.push(ch);
            i += 1;
            continue;
        }
        if in_single {
            out.push(ch);
            if ch == '\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            out.push(ch);
            if ch == '"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match ch {
            '\'' => {
                in_single = true;
                out.push(ch);
            }
            '"' => {
                in_double = true;
                out.push(ch);
            }
            '(' => {
                paren_depth += 1;
                out.push(ch);
            }
            ')' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
                out.push(ch);
            }
            c if is_whitespace(c) => {
                let mut j = i + 1;
                while j < chars.len() && is_whitespace(chars[j]) {
                    j += 1;
                }
                let prev = out.chars().rev().find(|c| !is_whitespace(*c));
                let next = chars.get(j).copied();
                let is_punct = |ch: Option<char>| {
                    matches!(
                        ch,
                        Some(':') | Some(',') | Some('/') | Some('=') | Some(')') | Some('(') | Some('!')
                    )
                };
                if is_punct(prev) || is_punct(next) {
                    // remove space
                } else {
                    out.push(' ');
                }
                i = j;
                continue;
            }
            ':' | ',' | '=' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(ch);
                let mut j = i + 1;
                while j < chars.len() && is_whitespace(chars[j]) {
                    j += 1;
                }
                i = j;
                continue;
            }
            _ => out.push(ch),
        }
        i += 1;
    }
    out.trim().to_string()
}
