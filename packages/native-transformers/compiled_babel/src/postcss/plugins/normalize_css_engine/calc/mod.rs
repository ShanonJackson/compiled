use crate::postcss::value_parser as vp;
use postcss as pc;

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Value {
        num: f64,
        unit: Option<String>,
    },
    Op {
        op: char,
        left: Box<Node>,
        right: Box<Node>,
    },
    Literal(String), // opaque literal like var(--x) or identifier, preserved for stringifier
}

#[derive(Clone)]
struct Lexer<'a> {
    s: &'a str,
    i: usize,
}
impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.i += ch.len_utf8();
        Some(ch)
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }
    fn take_while<F: Fn(char) -> bool>(&mut self, f: F) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if f(c) {
                out.push(c);
                self.bump();
            } else {
                break;
            }
        }
        out
    }
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }
}

// Parser roughly matching postcss-calc grammar
fn parse_calc_expression(input: &str) -> Option<Node> {
    let mut lx = Lexer::new(input);
    fn parse_number_unit(lx: &mut Lexer) -> Option<Node> {
        lx.skip_ws();
        let mut sign = 1.0;
        if let Some(c) = lx.peek() {
            if c == '+' {
                lx.bump();
            } else if c == '-' {
                lx.bump();
                sign = -1.0;
            }
        }
        lx.skip_ws();
        let num_str = lx.take_while(|c| c.is_ascii_digit() || c == '.');
        if num_str.is_empty() {
            return None;
        }
        let mut num: f64 = num_str.parse().ok()?;
        num *= sign;
        let unit = lx.take_while(|c| c.is_ascii_alphabetic() || c == '%');
        let unit_opt = if unit.is_empty() { None } else { Some(unit) };
        Some(Node::Value {
            num,
            unit: unit_opt,
        })
    }
    fn parse_primary(lx: &mut Lexer) -> Option<Node> {
        lx.skip_ws();
        match lx.peek()? {
            '(' => {
                lx.bump();
                let node = parse_add_sub(lx)?;
                lx.skip_ws();
                if lx.peek()? != ')' {
                    return None;
                }
                lx.bump();
                Some(node)
            }
            c if c.is_ascii_digit() || c == '.' || c == '+' || c == '-' => parse_number_unit(lx),
            _ => {
                // identifier or function -> capture literal including possible function body
                let ident =
                    lx.take_while(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
                let mut s = ident;
                if lx.peek() == Some('(') {
                    let mut depth = 0i32;
                    while let Some(ch) = lx.bump() {
                        s.push(ch);
                        if ch == '(' {
                            depth += 1;
                        } else if ch == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                }
                Some(Node::Literal(s))
            }
        }
    }
    fn parse_mul_div(lx: &mut Lexer) -> Option<Node> {
        let mut node = parse_primary(lx)?;
        loop {
            lx.skip_ws();
            let op = match lx.peek() {
                Some('*') => '*',
                Some('/') => '/',
                _ => break,
            };
            lx.bump();
            let rhs = parse_primary(lx)?;
            node = Node::Op {
                op,
                left: Box::new(node),
                right: Box::new(rhs),
            };
        }
        Some(node)
    }
    fn parse_add_sub(lx: &mut Lexer) -> Option<Node> {
        let mut node = parse_mul_div(lx)?;
        loop {
            lx.skip_ws();
            let op = match lx.peek() {
                Some('+') => '+',
                Some('-') => '-',
                _ => break,
            };
            lx.bump();
            let rhs = parse_mul_div(lx)?;
            node = Node::Op {
                op,
                left: Box::new(node),
                right: Box::new(rhs),
            };
        }
        Some(node)
    }
    let node = parse_add_sub(&mut lx)?;
    lx.skip_ws();
    if !lx.rest().is_empty() {
        return None;
    }
    Some(node)
}

fn same_unit(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum UnitKind {
    Length,
    Angle,
    Time,
    Frequency,
    Resolution,
    Percent,
    Number,
    Unknown,
}

fn unit_kind(unit: &str) -> UnitKind {
    match unit.to_ascii_lowercase().as_str() {
        // Length
        "px" | "cm" | "mm" | "q" | "in" | "pt" | "pc" => UnitKind::Length,
        // Font-relative/viewport lengths are not converted by postcss-calc reducer
        "em" | "rem" | "ex" | "ch" | "vw" | "vh" | "vmin" | "vmax" => UnitKind::Length,
        // Angle
        "deg" | "grad" | "rad" | "turn" => UnitKind::Angle,
        // Time
        "s" | "ms" => UnitKind::Time,
        // Frequency
        "hz" | "khz" => UnitKind::Frequency,
        // Resolution
        "dpi" | "dpcm" | "dppx" => UnitKind::Resolution,
        // Percent
        "%" => UnitKind::Percent,
        _ => UnitKind::Unknown,
    }
}

fn convert_unit(value: f64, source: &str, target: &str, precision: usize) -> Option<f64> {
    let s = source.to_ascii_lowercase();
    let t = target.to_ascii_lowercase();
    if s == t {
        return Some(value);
    }
    // Conversions map based on postcss-calc convertUnit.js
    fn round(v: f64, p: usize) -> f64 {
        let f = 10f64.powi(p as i32);
        (v * f).round() / f
    }
    let factor = match (t.as_str(), s.as_str()) {
        // Length
        ("px", "px") => Some(1.0),
        ("px", "cm") => Some(96.0 / 2.54),
        ("px", "mm") => Some(96.0 / 25.4),
        ("px", "q") => Some(96.0 / 101.6),
        ("px", "in") => Some(96.0),
        ("px", "pt") => Some(96.0 / 72.0),
        ("px", "pc") => Some(96.0 / 6.0),
        ("cm", "px") => Some(2.54 / 96.0),
        ("cm", "cm") => Some(1.0),
        ("cm", "mm") => Some(0.1),
        ("cm", "q") => Some(0.025),
        ("cm", "in") => Some(2.54),
        ("cm", "pt") => Some(2.54 / 72.0),
        ("cm", "pc") => Some(2.54 / 6.0),
        ("mm", "px") => Some(25.4 / 96.0),
        ("mm", "cm") => Some(10.0),
        ("mm", "mm") => Some(1.0),
        ("mm", "q") => Some(0.25),
        ("mm", "in") => Some(25.4),
        ("mm", "pt") => Some(25.4 / 72.0),
        ("mm", "pc") => Some(25.4 / 6.0),
        ("q", "px") => Some(101.6 / 96.0),
        ("q", "cm") => Some(40.0),
        ("q", "mm") => Some(4.0),
        ("q", "q") => Some(1.0),
        ("q", "in") => Some(101.6),
        ("q", "pt") => Some(101.6 / 72.0),
        ("q", "pc") => Some(101.6 / 6.0),
        ("in", "px") => Some(1.0 / 96.0),
        ("in", "cm") => Some(1.0 / 2.54),
        ("in", "mm") => Some(1.0 / 25.4),
        ("in", "q") => Some(1.0 / 101.6),
        ("in", "in") => Some(1.0),
        ("in", "pt") => Some(1.0 / 72.0),
        ("in", "pc") => Some(1.0 / 6.0),
        ("pt", "px") => Some(0.75),
        ("pt", "cm") => Some(72.0 / 2.54),
        ("pt", "mm") => Some(72.0 / 25.4),
        ("pt", "q") => Some(72.0 / 101.6),
        ("pt", "in") => Some(72.0),
        ("pt", "pt") => Some(1.0),
        ("pt", "pc") => Some(12.0),
        ("pc", "px") => Some(0.0625),
        ("pc", "cm") => Some(6.0 / 2.54),
        ("pc", "mm") => Some(6.0 / 25.4),
        ("pc", "q") => Some(6.0 / 101.6),
        ("pc", "in") => Some(6.0),
        ("pc", "pt") => Some(6.0 / 72.0),
        ("pc", "pc") => Some(1.0),
        // Angle
        ("deg", "deg") => Some(1.0),
        ("deg", "grad") => Some(0.9),
        ("deg", "rad") => Some(180.0 / std::f64::consts::PI),
        ("deg", "turn") => Some(360.0),
        ("grad", "deg") => Some(400.0 / 360.0),
        ("grad", "grad") => Some(1.0),
        ("grad", "rad") => Some(200.0 / std::f64::consts::PI),
        ("grad", "turn") => Some(400.0),
        ("rad", "deg") => Some(std::f64::consts::PI / 180.0),
        ("rad", "grad") => Some(std::f64::consts::PI / 200.0),
        ("rad", "rad") => Some(1.0),
        ("rad", "turn") => Some(std::f64::consts::PI * 2.0),
        ("turn", "deg") => Some(1.0 / 360.0),
        ("turn", "grad") => Some(0.0025),
        ("turn", "rad") => Some(0.5 / std::f64::consts::PI),
        ("turn", "turn") => Some(1.0),
        // Time
        ("s", "s") => Some(1.0),
        ("s", "ms") => Some(0.001),
        ("ms", "s") => Some(1000.0),
        ("ms", "ms") => Some(1.0),
        // Frequency
        ("hz", "hz") => Some(1.0),
        ("hz", "khz") => Some(1000.0),
        ("khz", "hz") => Some(0.001),
        ("khz", "khz") => Some(1.0),
        // Resolution
        ("dpi", "dpi") => Some(1.0),
        ("dpi", "dpcm") => Some(1.0 / 2.54),
        ("dpi", "dppx") => Some(1.0 / 96.0),
        ("dpcm", "dpi") => Some(2.54),
        ("dpcm", "dpcm") => Some(1.0),
        ("dpcm", "dppx") => Some(2.54 / 96.0),
        ("dppx", "dpi") => Some(96.0),
        ("dppx", "dpcm") => Some(96.0 / 2.54),
        ("dppx", "dppx") => Some(1.0),
        _ => None,
    }?;
    Some(round(value * factor, precision))
}

#[derive(Clone, Copy)]
struct Options {
    precision: usize,
    preserve: bool,
    warn_when_cannot_resolve: bool,
    media_queries: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            precision: 5,
            preserve: false,
            warn_when_cannot_resolve: false,
            media_queries: false,
        }
    }
}

#[derive(Clone, Debug)]
struct CollectItem {
    sign: char,
    node: Node,
}

fn flip_sign(op: char) -> char {
    if op == '+' {
        '-'
    } else {
        '+'
    }
}

fn is_value_node(node: &Node) -> bool {
    matches!(node, Node::Value { .. })
}

fn is_zero_value(node: &Node) -> bool {
    matches!(node, Node::Value { num, .. } if *num == 0.0)
}

fn convert_value_to_target(
    value: f64,
    from: &Option<String>,
    target: &Option<String>,
    precision: usize,
) -> Option<f64> {
    if same_unit(from, target) {
        return Some(value);
    }
    match (from, target) {
        (Some(f), Some(t)) => convert_unit(value, f, t, precision),
        _ => None,
    }
}

fn push_value_item(
    mut sign: char,
    mut num: f64,
    unit: Option<String>,
    collected: &mut Vec<CollectItem>,
    precision: usize,
) {
    if num < 0.0 {
        num = -num;
        sign = flip_sign(sign);
    }

    let mut handled = false;
    if num != 0.0 {
        for item in collected.iter_mut() {
            if let Node::Value {
                num: ref mut existing_num,
                unit: ref existing_unit,
            } = item.node
            {
                if let Some(converted) =
                    convert_value_to_target(num, &unit, existing_unit, precision)
                {
                    if item.sign == sign {
                        *existing_num += converted;
                    } else {
                        *existing_num -= converted;
                        if *existing_num < 0.0 {
                            *existing_num = -*existing_num;
                            item.sign = flip_sign(item.sign);
                        }
                    }
                    handled = true;
                    break;
                }
            }
        }
    }

    if !handled {
        collected.push(CollectItem {
            sign,
            node: Node::Value { num, unit },
        });
    }
}

fn collect_add_sub_items(
    sign: char,
    node: Node,
    collected: &mut Vec<CollectItem>,
    precision: usize,
) {
    match node {
        Node::Value { num, unit } => push_value_item(sign, num, unit, collected, precision),
        Node::Op { op, left, right } if op == '+' || op == '-' => {
            collect_add_sub_items(sign, *left, collected, precision);
            let right_sign = if sign == '-' { flip_sign(op) } else { op };
            collect_add_sub_items(right_sign, *right, collected, precision);
        }
        Node::Op { op, left, right } if op == '*' || op == '/' => {
            let reduced = reduce_node(Node::Op { op, left, right }, precision);
            // Avoid infinite recursion when the multiplicative expression
            // cannot be reduced to an additive expression. In that case,
            // carry the math expression forward as a single collectible,
            // matching postcss-calc behaviour.
            if let Node::Op { op: reduced_op, .. } = &reduced {
                if *reduced_op == '*' || *reduced_op == '/' {
                    collected.push(CollectItem {
                        sign,
                        node: reduced,
                    });
                    return;
                }
            }
            collect_add_sub_items(sign, reduced, collected, precision);
        }
        other => collected.push(CollectItem { sign, node: other }),
    }
}

fn reduce_add_sub_expression(left: Node, right: Node, op: char, precision: usize) -> Node {
    let mut collected = Vec::new();
    collect_add_sub_items('+', left, &mut collected, precision);
    let right_sign = if op == '+' { '+' } else { '-' };
    collect_add_sub_items(right_sign, right, &mut collected, precision);

    if collected.is_empty() {
        return Node::Value {
            num: 0.0,
            unit: None,
        };
    }

    let mut without_zero: Vec<CollectItem> = collected
        .iter()
        .cloned()
        .filter(|item| !is_zero_value(&item.node))
        .collect();

    if without_zero.is_empty()
        || (without_zero[0].sign == '-' && !is_value_node(&without_zero[0].node))
    {
        if let Some(zero_item) = collected.into_iter().find(|item| is_zero_value(&item.node)) {
            without_zero.insert(0, zero_item);
        }
    }

    if without_zero.is_empty() {
        return Node::Value {
            num: 0.0,
            unit: None,
        };
    }

    if without_zero[0].sign == '-' {
        if let Node::Value { ref mut num, .. } = without_zero[0].node {
            *num = -*num;
            without_zero[0].sign = '+';
        }
    }

    let mut iter = without_zero.into_iter();
    let mut root = iter.next().unwrap().node;
    for item in iter {
        root = Node::Op {
            op: item.sign,
            left: Box::new(root),
            right: Box::new(item.node),
        };
    }

    root
}

fn apply_number_multiplication(node: Node, multiplier: f64, precision: usize) -> Node {
    match node {
        Node::Value { num, unit } => Node::Value {
            num: num * multiplier,
            unit,
        },
        Node::Op { op, left, right } if op == '+' || op == '-' => Node::Op {
            op,
            left: Box::new(apply_number_multiplication(*left, multiplier, precision)),
            right: Box::new(apply_number_multiplication(*right, multiplier, precision)),
        },
        other => Node::Op {
            op: '*',
            left: Box::new(other),
            right: Box::new(Node::Value {
                num: multiplier,
                unit: None,
            }),
        },
    }
}

fn apply_number_division(node: Node, divisor: f64, precision: usize) -> Node {
    if divisor == 0.0 {
        return Node::Op {
            op: '/',
            left: Box::new(node),
            right: Box::new(Node::Value {
                num: divisor,
                unit: None,
            }),
        };
    }
    match node {
        Node::Value { num, unit } => Node::Value {
            num: num / divisor,
            unit,
        },
        Node::Op { op, left, right } if op == '+' || op == '-' => Node::Op {
            op,
            left: Box::new(apply_number_division(*left, divisor, precision)),
            right: Box::new(apply_number_division(*right, divisor, precision)),
        },
        other => Node::Op {
            op: '/',
            left: Box::new(other),
            right: Box::new(Node::Value {
                num: divisor,
                unit: None,
            }),
        },
    }
}

fn reduce_multiplication_expression(left: Node, right: Node, precision: usize) -> Node {
    let left_reduced = reduce_node(left, precision);
    let right_reduced = reduce_node(right, precision);

    if let Node::Value { num, unit: None } = right_reduced {
        return apply_number_multiplication(left_reduced, num, precision);
    }

    if let Node::Value { num, unit: None } = left_reduced {
        return apply_number_multiplication(right_reduced, num, precision);
    }

    Node::Op {
        op: '*',
        left: Box::new(left_reduced),
        right: Box::new(right_reduced),
    }
}

fn reduce_division_expression(left: Node, right: Node, precision: usize) -> Node {
    let left_reduced = reduce_node(left, precision);
    let right_reduced = reduce_node(right, precision);

    if let Node::Value { num, unit: None } = right_reduced {
        return apply_number_division(left_reduced, num, precision);
    }

    Node::Op {
        op: '/',
        left: Box::new(left_reduced),
        right: Box::new(right_reduced),
    }
}

fn reduce_node(node: Node, precision: usize) -> Node {
    match node {
        Node::Value { .. } | Node::Literal(_) => node,
        Node::Op { op, left, right } => match op {
            '+' | '-' => reduce_add_sub_expression(*left, *right, op, precision),
            '*' => reduce_multiplication_expression(*left, *right, precision),
            '/' => reduce_division_expression(*left, *right, precision),
            _ => Node::Op { op, left, right },
        },
    }
}

fn fmt_number(n: f64, precision: usize) -> String {
    let factor = 10f64.powi(precision as i32);
    let rounded = (n * factor).round() / factor;
    let mut s = ryu_js::Buffer::new().format_finite(rounded).to_string();
    if s == "-0" {
        s = "0".to_string();
    }
    if s.starts_with("0.") {
        s.replacen("0.", ".", 1)
    } else if s.starts_with("-0.") {
        s.replacen("-0.", "-.", 1)
    } else {
        s
    }
}

fn op_prec(op: char) -> i32 {
    match op {
        '*' | '/' => 0,
        '+' | '-' => 1,
        _ => 1,
    }
}

fn needs_paren(parent_op: char, child: &Node) -> bool {
    if let Node::Op { op: c, .. } = child {
        op_prec(parent_op) < op_prec(*c)
    } else {
        false
    }
}

fn stringify_ast(node: &Node, precision: usize) -> String {
    match node {
        Node::Value { num, unit } => {
            let mut s = fmt_number(*num, precision);
            if let Some(u) = unit {
                s.push_str(u);
            }
            s
        }
        Node::Literal(s) => s.clone(),
        Node::Op { op, left, right } => {
            let ls = if needs_paren(*op, left) {
                format!("({})", stringify_ast(left, precision))
            } else {
                stringify_ast(left, precision)
            };
            let rs = if needs_paren(*op, right) {
                format!("({})", stringify_ast(right, precision))
            } else {
                stringify_ast(right, precision)
            };
            let sep = match *op {
                '+' | '-' => format!(" {} ", op),
                _ => op.to_string(),
            };
            format!("{}{}{}", ls, sep, rs)
        }
    }
}

pub fn plugin() -> pc::BuiltPlugin {
    let opt = Options::default();
    pc::plugin("postcss-calc")
        .once_exit(move |css, _| {
            let process_decl = |decl: postcss::ast::nodes::Declaration| {
                let value = decl.value();
                if value.is_empty() {
                    return;
                }
                let mut parsed = vp::parse(&value);
                let mut changed = false;
                vp::walk(
                    &mut parsed.nodes[..],
                    &mut |n| {
                        if let vp::Node::Function {
                            value: name, nodes, ..
                        } = n
                        {
                            let name_l = name.to_ascii_lowercase();
                            let is_calc = name_l == "calc"
                                || name_l == "-webkit-calc"
                                || name_l == "-moz-calc";
                            if is_calc {
                                let inner = vp::stringify(nodes);
                                if let Some(ast) = parse_calc_expression(&inner) {
                                    let reduced = reduce_node(ast, opt.precision);
                                    if std::env::var("COMPILED_CLI_TRACE").is_ok() {
                                        eprintln!(
                                            "[calc] normalize input='{}' reduced={:?}",
                                            inner, reduced
                                        );
                                    }
                                    match reduced {
                                        Node::Value { num, unit } => {
                                            let mut v = fmt_number(num, opt.precision);
                                            if let Some(u) = unit {
                                                v.push_str(&u);
                                            }
                                            *n = vp::Node::Word { value: v };
                                        }
                                        other => {
                                            let expr = stringify_ast(&other, opt.precision);
                                            let wrapped = format!("{}({})", name, expr);
                                            *n = vp::Node::Word { value: wrapped };
                                        }
                                    }
                                    changed = true;
                                }
                            }
                        }
                        true
                    },
                    false,
                );
                if changed {
                    decl.set_value(vp::stringify(&parsed.nodes));
                }
            };
            match css {
                pc::ast::nodes::RootLike::Root(root) => {
                    root.walk_decls(|node, _| {
                        if let Some(decl) = postcss::ast::nodes::as_declaration(&node) {
                            process_decl(decl);
                        }
                        true
                    });
                }
                pc::ast::nodes::RootLike::Document(doc) => {
                    doc.walk_decls(|node, _| {
                        if let Some(decl) = postcss::ast::nodes::as_declaration(&node) {
                            process_decl(decl);
                        }
                        true
                    });
                }
            }
            Ok(())
        })
        .build()
}
