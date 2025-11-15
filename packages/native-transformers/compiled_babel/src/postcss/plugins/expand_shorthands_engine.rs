#[cfg(feature = "postcss_engine")]
pub fn plugin() -> postcss::BuiltPlugin {
    use postcss::ast::nodes::{as_declaration, as_rule};
    use postcss::ast::NodeAccess;
    use crate::postcss::plugins::expand_shorthands::index::expand_shorthand_pairs;

    postcss::plugin("expand-shorthands")
        .rule(|rule, _| {
            // Collect target declaration nodes first to avoid iterator invalidation
            let mut targets: Vec<postcss::ast::NodeRef> = Vec::new();
            for child in rule.nodes() {
                if let Some(decl) = as_declaration(&child) {
                    let prop = decl.prop();
                    // Only attempt expansion for known shorthands
                    if let Some(pairs) = expand_shorthand_pairs(&prop.to_lowercase(), &decl.value()) {
                        // Skip if expansion yields empty set (remove)
                        targets.push(child.clone());
                    }
                }
            }

            for child in targets {
                if let Some(decl) = as_declaration(&child) {
                    let prop = decl.prop();
                    let value = decl.value();
                    let important = decl.important();
                    if let Some(pairs) = expand_shorthand_pairs(&prop.to_lowercase(), &value) {
                        // Determine original index, then remove shorthand and insert longhands at same spot
                        let idx_opt = rule.child_index(&child);
                        rule.remove_child(child.clone());
                        if let Some(idx) = idx_opt {
                            let mut insert_at = idx;
                            for (name, val) in pairs.into_iter() {
                                let mut raws = postcss::ast::RawData::default();
                                raws.set_text("between", ":");
                                let new_decl = postcss::ast::nodes::declaration_with_raws(name, val, important, raws);
                                // Use low-level Node::insert on the underlying node
                                postcss::ast::Node::insert(&rule.to_node(), insert_at, new_decl);
                                insert_at += 1;
                            }
                        } else {
                            for (name, val) in pairs.into_iter() {
                                let mut raws = postcss::ast::RawData::default();
                                raws.set_text("between", ":");
                                let new_decl = postcss::ast::nodes::declaration_with_raws(name, val, important, raws);
                                rule.append(new_decl);
                            }
                        }
                    }
                }
            }

            Ok(())
        })
        .build()
}
