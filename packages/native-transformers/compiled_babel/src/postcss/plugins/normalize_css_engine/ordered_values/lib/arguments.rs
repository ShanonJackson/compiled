use crate::postcss::value_parser as vp;

// Equivalent to cssnano-utils getArguments for simple comma splits
pub fn get_arguments(parsed: &vp::ParsedValue) -> Vec<Vec<vp::Node>> {
    let mut list: Vec<Vec<vp::Node>> = vec![Vec::new()];
    for n in &parsed.nodes {
        if let vp::Node::Div { value, .. } = n {
            if value == "," {
                list.push(Vec::new());
                continue;
            }
        }
        if let Some(cur) = list.last_mut() {
            cur.push(n.clone());
        }
    }
    list
}
