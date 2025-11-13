use postcss as pc;
use crate::postcss::value_parser as vp;
use regex::Regex;

fn merge_range_bounds(left: &str, right: &str) -> Option<String> {
    let lchars: Vec<char> = left.chars().collect();
    let rchars: Vec<char> = right.chars().collect();
    if lchars.len() != rchars.len() { return None; }
    let mut question = 0usize;
    let mut group = String::from("u+");
    for i in 0..lchars.len() {
        let lc = lchars[i];
        let rc = rchars[i];
        if lc == rc && question == 0 { group.push(lc); }
        else if lc == '0' && rc == 'f' { question += 1; group.push('?'); }
        else { return None; }
    }
    if question < 6 { Some(group) } else { None }
}

fn normalize_single_range(range: &str) -> String {
    // input like u+abcd or u+00-ff or already with wildcards
    let r = range.to_lowercase();
    let mut parts = r[2..].splitn(2, '-');
    let a = parts.next().unwrap_or("");
    if let Some(b) = parts.next() {
        if let Some(merged) = merge_range_bounds(a, b) { return merged; }
    }
    r
}

pub fn plugin() -> pc::BuiltPlugin {
    pc::plugin("postcss-normalize-unicode")
        .prepare(|_result| {
            // Resolve browserslist to determine legacy bug: lower-case u prefix bug in old IE/Edge.
            // We default to modern (no bug) when not configured.
            let is_legacy = false;
            let re = Regex::new(r"(?i)u\+[0-9a-f?]+(?:-[0-9a-f?]+)?").unwrap();
            pc::PreparedCallbacks::default().once_exit(move |css, _| {
                css.walk_decls(|decl, _| {
                    if decl.prop().eq_ignore_ascii_case("unicode-range") {
                        let value = decl.value();
                        if value.is_empty() { return true; }
                        let newv = re.replace_all(&value, |caps: &regex::Captures| {
                            let mut out = normalize_single_range(&caps[0]);
                            if is_legacy { out = Regex::new(r"^u(?=\+)").unwrap().replace(&out, "U").to_string(); }
                            out
                        }).to_string();
                        if newv != value { decl.set_value(newv); }
                    }
                    true
                });
                Ok(())
            })
        })
        .build()
}

