use postcss as pc;
use crate::postcss::value_parser as vp;

fn parse_string_ast(input: &str) -> (Vec<(String, &'static str)>, bool) {
    // Returns list of (value, kind) and quotes flag
    // kinds: space, esc_squote, esc_dquote, squote, dquote, newline, string
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut nodes: Vec<(String, &'static str)> = Vec::new();
    let mut quotes = false;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        match ch {
            ' ' | '\t' | '\r' | '\x0C' => {
                let start = i;
                let mut j = i;
                while j < bytes.len() {
                    let c = bytes[j] as char;
                    if c == ' ' || c == '\n' || c == '\t' || c == '\r' || c == '\x0C' { j += 1; } else { break; }
                }
                nodes.push((input[start..j].to_string(), "space")); i = j; continue;
            }
            '\'' => { nodes.push(("'".to_string(), "squote")); quotes = true; i += 1; continue; }
            '"' => { nodes.push(('"'.to_string(), "dquote")); quotes = true; i += 1; continue; }
            '\\' => {
                let next = if i+1 < bytes.len() { bytes[i+1] as char } else { '\0' };
                if next == '\'' { nodes.push(("\\'".to_string(), "esc_squote")); quotes = true; i += 2; continue; }
                if next == '"' { nodes.push(("\\\"".to_string(), "esc_dquote")); quotes = true; i += 2; continue; }
                if next == '\n' { nodes.push(("\\\n".to_string(), "newline")); i += 2; continue; }
            }
            _ => {}
        }
        // word until WORD_END
        let start = i;
        let mut j = i+1;
        while j < bytes.len() {
            let c = bytes[j] as char;
            if c == ' ' || c == '\n' || c == '\t' || c == '\r' || c == '\x0C' || c == '\\' || c == '\'' || c == '"' { break; }
            j += 1;
        }
        nodes.push((input[start..j].to_string(), "string"));
        i = j;
    }
    (nodes, quotes)
}

fn stringify_ast(nodes: &[(String, &'static str)]) -> String {
    let mut out = String::new();
    for (v, kind) in nodes.iter() {
        if *kind == "newline" { continue; }
        out.push_str(v);
    }
    out
}

fn change_wrapping_quotes(node_quote: &mut char, nodes: &mut Vec<(String,&'static str)>) {
    // Flip quotes if escaping is reduced
    let mut has_squote = 0; let mut has_dquote = 0; let mut esc_s = 0; let mut esc_d = 0;
    for (_, k) in nodes.iter() {
        match *k { "squote" => has_squote += 1, "dquote" => has_dquote += 1, "esc_squote" => esc_s += 1, "esc_dquote" => esc_d += 1, _ => {} }
    }
    if has_squote == 0 && has_dquote == 0 {
        if *node_quote == '\'' && esc_s > 0 && esc_d == 0 { *node_quote = '"'; }
        else if *node_quote == '"' && esc_d > 0 && esc_s == 0 { *node_quote = '\''; }
    }
    // Replace escaped inner quotes to actual quotes when parent quote differs
    let parent = *node_quote;
    let mut replaced: Vec<(String,&'static str)> = Vec::with_capacity(nodes.len());
    for (v, k) in nodes.drain(..) {
        if k == "esc_dquote" && parent == '\'' { replaced.push(('"'.to_string(), "dquote")); }
        else if k == "esc_squote" && parent == '"' { replaced.push(("'".to_string(), "squote")); }
        else { replaced.push((v, k)); }
    }
    *nodes = replaced;
}

fn normalize_value(value: &str, preferred_quote: char) -> String {
    let mut parsed = vp::parse(value);
    vp::walk(&mut parsed.nodes[..], &mut |n| {
        match n {
            vp::Node::String { value: s, quote, .. } => {
                let (mut nodes, quotes) = parse_string_ast(s);
                let mut q = *quote;
                if q == '\0' { q = preferred_quote; }
                if quotes { change_wrapping_quotes(&mut q, &mut nodes); }
                else { q = preferred_quote; }
                *quote = q;
                *s = stringify_ast(&nodes);
            }
            _ => {}
        }
        true
    }, false);
    vp::stringify(&parsed.nodes)
}

pub fn plugin() -> pc::BuiltPlugin {
    pc::plugin("postcss-normalize-string")
        .prepare(|_result| {
            let preferred_quote = '"';
            let cache = std::sync::Mutex::new(std::collections::HashMap::<String,String>::new());
            pc::PreparedCallbacks::default().once_exit(move |css, _| {
                css.walk(|node| {
                    match node {
                        pc::NodeLike::Rule(r) => { let sel = r.selector(); let key = format!("S|{}", sel); if let Some(v)=cache.lock().unwrap().get(&key).cloned(){ r.set_selector(v); } else { let nv = normalize_value(&sel, preferred_quote); cache.lock().unwrap().insert(key.clone(), nv.clone()); r.set_selector(nv); } }
                        pc::NodeLike::Decl(d) => { let v = d.value(); let key = format!("V|{}", v); if let Some(n)=cache.lock().unwrap().get(&key).cloned(){ d.set_value(n); } else { let nv = normalize_value(&v, preferred_quote); cache.lock().unwrap().insert(key.clone(), nv.clone()); d.set_value(nv); } }
                        pc::NodeLike::AtRule(a) => { let p = a.params(); let key = format!("P|{}", p); if let Some(n)=cache.lock().unwrap().get(&key).cloned(){ a.set_params(n); } else { let nv = normalize_value(&p, preferred_quote); cache.lock().unwrap().insert(key.clone(), nv.clone()); a.set_params(nv); } }
                        _ => {}
                    }
                    true
                });
                Ok(())
            })
        })
        .build()
}

