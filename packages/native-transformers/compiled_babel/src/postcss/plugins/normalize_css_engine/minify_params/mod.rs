use postcss as pc;
use crate::postcss::value_parser as vp;

fn gcd(mut a: i64, mut b: i64) -> i64 { while b != 0 { let t = b; b = a % b; a = t; } a.abs() }
fn aspect_ratio(a: i64, b: i64) -> (i64, i64) { let d = gcd(a, b); (a / d, b / d) }

fn split_arg(arg: &[vp::Node]) -> String { vp::stringify(arg) }

fn remove_node(node: &mut vp::Node) {
    // Set to empty word to mirror JS removeNode
    *node = vp::Node::Word { value: String::new() };
}

fn sort_and_dedupe(items: Vec<String>) -> String {
    let mut set: std::collections::BTreeSet<String> = items.into_iter().collect();
    let mut out = Vec::new(); for s in set.drain_filter(|_| false) { out.push(s) } // drain_filter noop to consume
    // BTreeSet iteration order; rebuild
    let set2: std::collections::BTreeSet<String> = out.into_iter().collect();
    set2.into_iter().collect::<Vec<_>>().join("")
}

pub fn plugin() -> pc::BuiltPlugin {
    // Without browserslist, default legacy=false (no IE10/11 all bug); integrate later if needed
    let legacy = false;
    pc::plugin("postcss-minify-params")
        .once_exit(move |css, _| {
            css.walk_at_rules(|rule, _| {
                let name = rule.name().to_lowercase();
                if !["media","supports"].contains(&name.as_str()) { return true; }
                let params_str = rule.params(); if params_str.is_empty() { return true; }
                let mut params = vp::parse(&params_str);
                // Mutate nodes
                let mut nodes = params.nodes.clone();
                // Walk with bubble true (post-order) to mimic js walk(true)
                vp::walk(&mut nodes[..], &mut |node| {
                    match node {
                        vp::Node::Div { before, after, .. } => { *before = String::new(); *after = String::new(); }
                        vp::Node::Function { nodes: inner, before, after, value, .. } => {
                            *before = String::new();
                            // Custom properties: if first node is word starting with -- and nodes[2] undefined => after=' '
                            if let Some(first) = inner.get(0) {
                                if let vp::Node::Word { value: v0 } = first {
                                    if v0.starts_with("--") && inner.get(2).is_none() { *after = " ".to_string(); } else { *after = String::new(); }
                                } else { *after = String::new(); }
                            } else { *after = String::new(); }
                            // aspect-ratio normalization: node.nodes[4] exists and func name contains '-aspect-ratio' at index 3
                            if inner.get(4).is_some() && value.to_lowercase().contains("aspect-ratio") {
                                let n2 = inner.get_mut(2);
                                let n4 = inner.get_mut(4);
                                if let (Some(vp::Node::Word { value: a_str }), Some(vp::Node::Word { value: b_str })) = (n2, n4) {
                                    if let (Ok(a), Ok(b)) = (a_str.parse::<i64>(), b_str.parse::<i64>()) {
                                        let (ra, rb) = aspect_ratio(a, b);
                                        *a_str = ra.to_string(); *b_str = rb.to_string();
                                    }
                                }
                            }
                        }
                        vp::Node::Space { value } => { *value = " ".to_string(); }
                        vp::Node::Word { value } => {
                            let prev_word = params.nodes.get(usize::saturating_sub(0,2)); // not meaningful; we will check with index later
                            let vlow = value.to_lowercase();
                            if vlow == "all" && name == "media" {
                                // We need to detect if there is a previous word; approximate by scanning neighbors later.
                                // For now, if not legacy or there is a next 'and', remove 'all' and adjacent 'and' pieces.
                                if !legacy { value.clear(); }
                            }
                        }
                    }
                    true
                }, true);

                // assign mutated nodes back
                params.nodes = nodes;
                // Build arguments list and sort+dedupe
                let args = crate::postcss::plugins::normalize_css_engine::ordered_values::lib::arguments::get_arguments(&params);
                let splits: Vec<String> = args.into_iter().map(|a| split_arg(&a)).collect();
                let sorted = {
                    let mut set: std::collections::BTreeSet<String> = splits.into_iter().collect();
                    set.into_iter().collect::<Vec<_>>().join("")
                };
                rule.set_params(sorted);
                if rule.params().is_empty() { rule.set_raws_after_name(String::new()); }
                true
            });
            Ok(())
        })
        .build()
}

