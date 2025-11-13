use postcss as pc;
use crate::postcss::value_parser as vp;

#[derive(Clone, Debug, PartialEq)]
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Value { num: f64, unit: Option<String> },
    Op { op: char, left: Box<Node>, right: Box<Node> },
    Literal(String), // opaque literal like var(--x) or identifier, preserved for stringifier
}

#[derive(Clone)]
struct Lexer<'a> { s: &'a str, i: usize }
impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self { Self { s, i: 0 } }
    fn peek(&self) -> Option<char> { self.s[self.i..].chars().next() }
    fn bump(&mut self) -> Option<char> { let ch = self.peek()?; self.i += ch.len_utf8(); Some(ch) }
    fn skip_ws(&mut self) { while let Some(c) = self.peek() { if c.is_whitespace() { self.bump(); } else { break; } } }
    fn take_while<F: Fn(char)->bool>(&mut self, f: F) -> String { let mut out=String::new(); while let Some(c)=self.peek(){ if f(c){ out.push(c); self.bump(); } else { break; } } out }
    fn rest(&self) -> &'a str { &self.s[self.i..] }
}

// Parser roughly matching postcss-calc grammar
fn parse_calc_expression(input: &str) -> Option<Node> {
    let mut lx = Lexer::new(input);
    fn parse_number_unit(lx: &mut Lexer) -> Option<Node> {
        lx.skip_ws();
        let mut sign = 1.0;
        if let Some(c) = lx.peek() { if c == '+' { lx.bump(); } else if c == '-' { lx.bump(); sign = -1.0; } }
        lx.skip_ws();
        let num_str = lx.take_while(|c| c.is_ascii_digit() || c == '.');
        if num_str.is_empty() { return None; }
        let mut num: f64 = num_str.parse().ok()?; num *= sign;
        let unit = lx.take_while(|c| c.is_ascii_alphabetic() || c == '%');
        let unit_opt = if unit.is_empty() { None } else { Some(unit) };
        Some(Node::Value { num, unit: unit_opt })
    }
    fn parse_primary(lx: &mut Lexer) -> Option<Node> {
        lx.skip_ws();
        match lx.peek()? {
            '(' => { lx.bump(); let node = parse_add_sub(lx)?; lx.skip_ws(); if lx.peek()? != ')' { return None; } lx.bump(); Some(node) }
            c if c.is_ascii_digit() || c == '.' || c == '+' || c == '-' => parse_number_unit(lx),
            _ => {
                // identifier or function -> capture literal including possible function body
                let ident = lx.take_while(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
                let mut s = ident;
                if lx.peek() == Some('(') {
                    let mut depth = 0i32;
                    while let Some(ch) = lx.bump() {
                        s.push(ch);
                        if ch == '(' { depth += 1; }
                        else if ch == ')' { depth -= 1; if depth == 0 { break; } }
                    }
                }
                Some(Node::Literal(s))
            }
        }
    }
    fn parse_mul_div(lx: &mut Lexer) -> Option<Node> { let mut node = parse_primary(lx)?; loop { lx.skip_ws(); let op = match lx.peek(){ Some('*')=>'*', Some('/')=>'/', _=> break }; lx.bump(); let rhs = parse_primary(lx)?; node = Node::Op { op, left: Box::new(node), right: Box::new(rhs) }; } Some(node) }
    fn parse_add_sub(lx: &mut Lexer) -> Option<Node> { let mut node = parse_mul_div(lx)?; loop { lx.skip_ws(); let op = match lx.peek(){ Some('+')=>'+', Some('-')=>'-', _=> break }; lx.bump(); let rhs = parse_mul_div(lx)?; node = Node::Op { op, left: Box::new(node), right: Box::new(rhs) }; } Some(node) }
    let node = parse_add_sub(&mut lx)?; lx.skip_ws(); if !lx.rest().is_empty() { return None; } Some(node)
}

fn same_unit(a: &Option<String>, b: &Option<String>) -> bool { match (a,b){(None,None)=>true,(Some(x),Some(y))=>x==y,_=>false} }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum UnitKind { Length, Angle, Time, Frequency, Resolution, Percent, Number, Unknown }

fn unit_kind(unit: &str) -> UnitKind {
    match unit.to_ascii_lowercase().as_str() {
        // Length
        "px"|"cm"|"mm"|"q"|"in"|"pt"|"pc" => UnitKind::Length,
        // Font-relative/viewport lengths are not converted by postcss-calc reducer
        "em"|"rem"|"ex"|"ch"|"vw"|"vh"|"vmin"|"vmax" => UnitKind::Length,
        // Angle
        "deg"|"grad"|"rad"|"turn" => UnitKind::Angle,
        // Time
        "s"|"ms" => UnitKind::Time,
        // Frequency
        "hz"|"khz" => UnitKind::Frequency,
        // Resolution
        "dpi"|"dpcm"|"dppx" => UnitKind::Resolution,
        // Percent
        "%" => UnitKind::Percent,
        _ => UnitKind::Unknown,
    }
}

fn convert_unit(value: f64, source: &str, target: &str, precision: usize) -> Option<f64> {
    let s = source.to_ascii_lowercase();
    let t = target.to_ascii_lowercase();
    if s == t { return Some(value); }
    // Conversions map based on postcss-calc convertUnit.js
    fn round(v: f64, p: usize) -> f64 { let f = 10f64.powi(p as i32); (v * f).round() / f }
    let factor = match (t.as_str(), s.as_str()) {
        // Length
        ("px","px")=>Some(1.0),("px","cm")=>Some(96.0/2.54),("px","mm")=>Some(96.0/25.4),("px","q")=>Some(96.0/101.6),("px","in")=>Some(96.0),("px","pt")=>Some(96.0/72.0),("px","pc")=>Some(96.0/6.0),
        ("cm","px")=>Some(2.54/96.0),("cm","cm")=>Some(1.0),("cm","mm")=>Some(0.1),("cm","q")=>Some(0.025),("cm","in")=>Some(2.54),("cm","pt")=>Some(2.54/72.0),("cm","pc")=>Some(2.54/6.0),
        ("mm","px")=>Some(25.4/96.0),("mm","cm")=>Some(10.0),("mm","mm")=>Some(1.0),("mm","q")=>Some(0.25),("mm","in")=>Some(25.4),("mm","pt")=>Some(25.4/72.0),("mm","pc")=>Some(25.4/6.0),
        ("q","px")=>Some(101.6/96.0),("q","cm")=>Some(40.0),("q","mm")=>Some(4.0),("q","q")=>Some(1.0),("q","in")=>Some(101.6),("q","pt")=>Some(101.6/72.0),("q","pc")=>Some(101.6/6.0),
        ("in","px")=>Some(1.0/96.0),("in","cm")=>Some(1.0/2.54),("in","mm")=>Some(1.0/25.4),("in","q")=>Some(1.0/101.6),("in","in")=>Some(1.0),("in","pt")=>Some(1.0/72.0),("in","pc")=>Some(1.0/6.0),
        ("pt","px")=>Some(0.75),("pt","cm")=>Some(72.0/2.54),("pt","mm")=>Some(72.0/25.4),("pt","q")=>Some(72.0/101.6),("pt","in")=>Some(72.0),("pt","pt")=>Some(1.0),("pt","pc")=>Some(12.0),
        ("pc","px")=>Some(0.0625),("pc","cm")=>Some(6.0/2.54),("pc","mm")=>Some(6.0/25.4),("pc","q")=>Some(6.0/101.6),("pc","in")=>Some(6.0),("pc","pt")=>Some(6.0/72.0),("pc","pc")=>Some(1.0),
        // Angle
        ("deg","deg")=>Some(1.0),("deg","grad")=>Some(0.9),("deg","rad")=>Some(180.0/std::f64::consts::PI),("deg","turn")=>Some(360.0),
        ("grad","deg")=>Some(400.0/360.0),("grad","grad")=>Some(1.0),("grad","rad")=>Some(200.0/std::f64::consts::PI),("grad","turn")=>Some(400.0),
        ("rad","deg")=>Some(std::f64::consts::PI/180.0),("rad","grad")=>Some(std::f64::consts::PI/200.0),("rad","rad")=>Some(1.0),("rad","turn")=>Some(std::f64::consts::PI*2.0),
        ("turn","deg")=>Some(1.0/360.0),("turn","grad")=>Some(0.0025),("turn","rad")=>Some(0.5/std::f64::consts::PI),("turn","turn")=>Some(1.0),
        // Time
        ("s","s")=>Some(1.0),("s","ms")=>Some(0.001), ("ms","s")=>Some(1000.0),("ms","ms")=>Some(1.0),
        // Frequency
        ("hz","hz")=>Some(1.0),("hz","khz")=>Some(1000.0),("khz","hz")=>Some(0.001),("khz","khz")=>Some(1.0),
        // Resolution
        ("dpi","dpi")=>Some(1.0),("dpi","dpcm")=>Some(1.0/2.54),("dpi","dppx")=>Some(1.0/96.0),
        ("dpcm","dpi")=>Some(2.54),("dpcm","dpcm")=>Some(1.0),("dpcm","dppx")=>Some(2.54/96.0),
        ("dppx","dpi")=>Some(96.0),("dppx","dpcm")=>Some(96.0/2.54),("dppx","dppx")=>Some(1.0),
        _ => None
    }?;
    Some(round(value * factor, precision))
}

#[derive(Clone, Copy)]
struct Options { precision: usize, preserve: bool, warn_when_cannot_resolve: bool, media_queries: bool }
impl Default for Options { fn default() -> Self { Self { precision: 5, preserve: false, warn_when_cannot_resolve: false, media_queries: false } } }

fn reduce_with_precision(node: &Node, precision: usize) -> Option<Node> {
    match node {
        Node::Value { .. } => Some(node.clone()),
        Node::Literal(_) => None,
        Node::Op { op, left, right } => {
            // Evaluate with precedence already encoded in AST
            let l = reduce_with_precision(left, precision)?;
            let r = reduce_with_precision(right, precision)?;
            match (op, l, r) {
                ('+', Node::Value { num: ln, unit: lu }, Node::Value { num: rn, unit: ru }) => {
                    if same_unit(&lu, &ru) { Some(Node::Value { num: ln + rn, unit: lu.or(ru) }) }
                    else if let (Some(ref lustr), Some(ref rustr)) = (&lu, &ru) {
                        if unit_kind(lustr) == unit_kind(rustr) {
                            if let Some(conv) = convert_unit(rn, rustr, lustr, precision) {
                                Some(Node::Value { num: ln + conv, unit: lu.clone() })
                            } else { None }
                        } else { None }
                    } else { None }
                }
                ('-', Node::Value { num: ln, unit: lu }, Node::Value { num: rn, unit: ru }) => {
                    if same_unit(&lu, &ru) { Some(Node::Value { num: ln - rn, unit: lu.or(ru) }) }
                    else if let (Some(ref lustr), Some(ref rustr)) = (&lu, &ru) {
                        if unit_kind(lustr) == unit_kind(rustr) {
                            if let Some(conv) = convert_unit(rn, rustr, lustr, precision) {
                                Some(Node::Value { num: ln - conv, unit: lu.clone() })
                            } else { None }
                        } else { None }
                    } else { None }
                }
                // multiplication
                ('*', Node::Value { num: ln, unit: lu }, Node::Value { num: rn, unit: ru }) => {
                    match (lu, ru) {
                        (Some(u), None) => Some(Node::Value { num: ln * rn, unit: Some(u) }),
                        (None, Some(u)) => Some(Node::Value { num: ln * rn, unit: Some(u) }),
                        (None, None) => Some(Node::Value { num: ln * rn, unit: None }),
                        _ => None,
                    }
                }
                // division
                ('/', Node::Value { num: ln, unit: lu }, Node::Value { num: rn, unit: ru }) => {
                    if rn == 0.0 { return None; }
                    match (lu, ru) {
                        (Some(u), None) => Some(Node::Value { num: ln / rn, unit: Some(u) }),
                        (None, Some(_)) => None,
                        (None, None) => Some(Node::Value { num: ln / rn, unit: None }),
                        (Some(_), Some(_)) => None,
                    }
                }
                // Distribute scalar over addition/subtraction: (a +/- b) * c, c * (a +/- b)
                ('*', Node::Op { op: lop, left: ll, right: lr }, Node::Value { num: rn, unit: ru }) if *lop == '+' || *lop == '-' => {
                    // (ll op lr) * rn => (ll*rn) op (lr*rn)
                    let left_mul = reduce_with_precision(&Node::Op { op: '*', left: ll.clone(), right: Box::new(Node::Value { num: rn, unit: ru.clone() }) }, precision)?;
                    let right_mul = reduce_with_precision(&Node::Op { op: '*', left: lr.clone(), right: Box::new(Node::Value { num: rn, unit: ru.clone() }) }, precision)?;
                    Some(Node::Op { op: *lop, left: Box::new(left_mul), right: Box::new(right_mul) })
                }
                ('*', Node::Value { num: ln, unit: lu }, Node::Op { op: rop, left: rl, right: rr }) if *rop == '+' || *rop == '-' => {
                    let left_mul = reduce_with_precision(&Node::Op { op: '*', left: Box::new(Node::Value { num: ln, unit: lu.clone() }), right: rl.clone() }, precision)?;
                    let right_mul = reduce_with_precision(&Node::Op { op: '*', left: Box::new(Node::Value { num: ln, unit: lu.clone() }), right: rr.clone() }, precision)?;
                    Some(Node::Op { op: *rop, left: Box::new(left_mul), right: Box::new(right_mul) })
                }
                // Distribute division over addition/subtraction when dividing by scalar: (a +/- b) / n
                ('/', Node::Op { op: lop, left: ll, right: lr }, Node::Value { num: rn, unit: ru }) if (*lop == '+' || *lop == '-') && ru.is_none() => {
                    if rn == 0.0 { return None; }
                    let left_div = reduce_with_precision(&Node::Op { op: '/', left: ll.clone(), right: Box::new(Node::Value { num: rn, unit: None }) }, precision)?;
                    let right_div = reduce_with_precision(&Node::Op { op: '/', left: lr.clone(), right: Box::new(Node::Value { num: rn, unit: None }) }, precision)?;
                    Some(Node::Op { op: *lop, left: Box::new(left_div), right: Box::new(right_div) })
                }
                // Multiplication by zero yields zero (unitless) when safe
                ('*', Node::Value { num: ln, unit: _ }, Node::Value { num: rn, unit: _ }) if ln == 0.0 || rn == 0.0 => {
                    Some(Node::Value { num: 0.0, unit: None })
                }
                _ => None,
            }
        }
    }
}

fn fmt_number(n: f64, precision: usize) -> String {
    let factor = 10f64.powi(precision as i32);
    let rounded = (n * factor).round() / factor;
    let mut s = ryu_js::Buffer::new().format_finite(rounded).to_string();
    if s == "-0" { s = "0".to_string(); }
    if s.starts_with("0.") { s.replacen("0.", ".", 1) } else if s.starts_with("-0.") { s.replacen("-0.", "-.", 1) } else { s }
}

fn try_reduce_calc(contents: &str, opt: &Options) -> Option<String> {
    let ast = parse_calc_expression(contents)?;
    if let Some(reduced) = reduce_with_precision(&ast, opt.precision) {
        if let Node::Value { num, unit } = reduced {
            let mut v = fmt_number(num, opt.precision);
            if let Some(u) = unit { v.push_str(&u); }
            return Some(v);
        }
    }
    None
}

fn op_prec(op: char) -> i32 { match op { '*'|'/' => 0, '+'|'-' => 1, _ => 1 } }

fn needs_paren(parent_op: char, child: &Node) -> bool {
    if let Node::Op { op: c, .. } = child { op_prec(parent_op) < op_prec(*c) } else { false }
}

fn stringify_ast(node: &Node, precision: usize) -> String {
    match node {
        Node::Value { num, unit } => {
            let mut s = fmt_number(*num, precision);
            if let Some(u) = unit { s.push_str(u); }
            s
        }
        Node::Literal(s) => s.clone(),
        Node::Op { op, left, right } => {
            let ls = if needs_paren(*op, left) { format!("({})", stringify_ast(left, precision)) } else { stringify_ast(left, precision) };
            let rs = if needs_paren(*op, right) { format!("({})", stringify_ast(right, precision)) } else { stringify_ast(right, precision) };
            let sep = match *op { '+'|'-' => format!(" {} ", op), _ => op.to_string() };
            format!("{}{}{}", ls, sep, rs)
        }
    }
}

pub fn plugin() -> pc::BuiltPlugin {
    pc::plugin("postcss-calc")
        .prepare(|_result| {
            let opt = Options::default();
            pc::PreparedCallbacks::default().once_exit(move |css, _| {
                css.walk_decls(|decl, _| {
                    let value = decl.value(); if value.is_empty() { return true; }
                    let mut parsed = vp::parse(&value);
                    let mut changed = false;
                    vp::walk(&mut parsed.nodes[..], &mut |n| {
                        if let vp::Node::Function { value: name, nodes, .. } = n {
                            let name_l = name.to_ascii_lowercase();
                            let is_calc = name_l == "calc" || name_l == "-webkit-calc" || name_l == "-moz-calc";
                            if is_calc {
                                let inner = vp::stringify(nodes);
                                if let Some(new_val) = try_reduce_calc(&inner, &opt) {
                                    *n = vp::Node::Word { value: new_val };
                                    changed = true;
                                } else {
                                    // Canonicalize expression spacing and parentheses and rewrap
                                    if let Some(ast) = parse_calc_expression(&inner) {
                                        let expr = stringify_ast(&ast, opt.precision);
                                        let wrapped = format!("{}({})", name, expr);
                                        *n = vp::Node::Word { value: wrapped };
                                        changed = true;
                                    }
                                }
                            }
                        }
                        true
                    }, false);
                    if changed { decl.set_value(vp::stringify(&parsed.nodes)); }
                    true
                });
                Ok(())
            })
        })
        .build()
}
