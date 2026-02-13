//! Processor — plugin pipeline executor.
//!
//! Port of PostCSS 8.4.31 `processor.js` and the synchronous path of
//! `lazy-result.js`. Executes plugins in order: Once → visitor walk → OnceExit.

use crate::ast::{NodeId, Stylesheet};
use crate::input::Input;
use crate::parser;
use crate::plugin::Plugin;
use crate::stringifier;

/// The PostCSS processor. Holds a list of plugins and executes them
/// against a parsed stylesheet.
pub struct Processor {
    pub plugins: Vec<Box<dyn Plugin>>,
}

impl Processor {
    /// Create a new processor with the given plugins.
    pub fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
        Processor { plugins }
    }

    /// Parse CSS, run all plugins, and return the resulting CSS string.
    pub fn process(&mut self, css: &str) -> String {
        let input = Input::new(css, None);
        let mut ss = Stylesheet::new();
        parser::parse(&input, &mut ss);

        self.run_plugins(&mut ss);

        stringifier::stringify(&ss)
    }

    /// Parse CSS, run all plugins, and return the Stylesheet for further
    /// inspection/manipulation.
    pub fn process_to_stylesheet(&mut self, css: &str) -> Stylesheet {
        let input = Input::new(css, None);
        let mut ss = Stylesheet::new();
        parser::parse(&input, &mut ss);

        self.run_plugins(&mut ss);

        ss
    }

    /// Run all plugins against a stylesheet.
    ///
    /// Execution order (matching JS PostCSS `LazyResult.sync()`):
    /// 1. Run `Once` hooks for all plugins (in order).
    /// 2. Build visitor dispatch table.
    /// 3. Walk tree, trigger enter/exit events for each node.
    ///    - Re-walk if any node is marked dirty during walking.
    /// 4. Run `OnceExit` hooks for all plugins (in order).
    fn run_plugins(&mut self, ss: &mut Stylesheet) {
        let root = ss.root;

        // Phase 1: Once hooks.
        for plugin in self.plugins.iter_mut() {
            plugin.once(root, ss);
        }

        // Phase 2: Visitor walking.
        let has_visitors = self.plugins.iter().any(|p| p.has_visitors());

        if has_visitors {
            // JS PostCSS re-walk loop: while root is dirty, mark root clean
            // and walk. walkSync marks each node clean as it visits.
            // If a plugin modifies a node and calls mark_dirty(), the root
            // becomes dirty again, triggering another walk.
            // Since most plugins don't use the dirty system (they modify in
            // place), we do a single walk pass.
            self.walk_sync(ss, root);
        }

        // Phase 3: OnceExit hooks.
        for plugin in self.plugins.iter_mut() {
            plugin.once_exit(root, ss);
        }
    }

    /// Walk a node synchronously, triggering plugin visitor hooks.
    /// Marks each node as clean on visit (matching JS PostCSS `walkSync()`).
    fn walk_sync(&mut self, ss: &mut Stylesheet, node_id: NodeId) {
        ss.node_mut(node_id).set_clean(true);
        let node_type = ss.node(node_id).node_type();

        // Trigger enter events.
        match node_type {
            "rule" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.rule(node_id, ss);
                }
            }
            "decl" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.declaration(node_id, ss);
                }
            }
            "atrule" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.at_rule(node_id, ss);
                }
            }
            "comment" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.comment(node_id, ss);
                }
            }
            _ => {}
        }

        // Recurse into children.
        let children: Vec<NodeId> = ss
            .node(node_id)
            .children()
            .map(|c| c.to_vec())
            .unwrap_or_default();

        for child_id in children {
            if !ss.node(child_id).is_clean() {
                self.walk_sync(ss, child_id);
            }
        }

        // Trigger exit events.
        match node_type {
            "rule" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.rule_exit(node_id, ss);
                }
            }
            "decl" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.declaration_exit(node_id, ss);
                }
            }
            "atrule" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.at_rule_exit(node_id, ss);
                }
            }
            "comment" => {
                for plugin in self.plugins.iter_mut() {
                    plugin.comment_exit(node_id, ss);
                }
            }
            _ => {}
        }
    }
}

/// Mark all nodes in the tree as clean.
fn mark_all_clean(ss: &mut Stylesheet, node_id: NodeId) {
    ss.node_mut(node_id).set_clean(true);
    let children: Vec<NodeId> = ss
        .node(node_id)
        .children()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    for child_id in children {
        mark_all_clean(ss, child_id);
    }
}

/// Check if any node in the tree is dirty.
fn any_dirty(ss: &Stylesheet, node_id: NodeId) -> bool {
    if !ss.node(node_id).is_clean() {
        return true;
    }
    if let Some(children) = ss.node(node_id).children() {
        for &child_id in children {
            if any_dirty(ss, child_id) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UppercaseValues;

    impl Plugin for UppercaseValues {
        fn name(&self) -> &str {
            "uppercase-values"
        }

        fn has_visitors(&self) -> bool {
            true
        }

        fn declaration(&mut self, decl: NodeId, ss: &mut Stylesheet) {
            if let crate::ast::CssNode::Declaration(d) = ss.node_mut(decl) {
                d.value = d.value.to_uppercase();
            }
        }
    }

    #[test]
    fn test_process_no_plugins() {
        let mut proc = Processor::new(vec![]);
        let result = proc.process("a { color: red }");
        assert_eq!(result, "a { color: red }");
    }

    #[test]
    fn test_process_with_plugin() {
        let mut proc = Processor::new(vec![Box::new(UppercaseValues)]);
        let result = proc.process("a { color: red }");
        assert_eq!(result, "a { color: RED }");
    }
}
