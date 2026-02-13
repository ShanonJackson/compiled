//! Plugin trait — port of PostCSS 8.4.31 plugin system.
//!
//! Plugins implement lifecycle hooks that are called at specific points during
//! AST processing. The execution model matches JS PostCSS exactly.

use crate::ast::{NodeId, Stylesheet};

/// A PostCSS plugin with lifecycle hooks.
///
/// All hooks have default no-op implementations so plugins only need to
/// implement the hooks they care about.
pub trait Plugin {
    /// The plugin name (for error messages and debugging).
    fn name(&self) -> &str;

    /// Called once before visitor walking begins (per root).
    fn once(&mut self, _root: NodeId, _ss: &mut Stylesheet) {}

    /// Called once after all visitor walking completes (per root).
    fn once_exit(&mut self, _root: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each Declaration node during walking.
    fn declaration(&mut self, _decl: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each Declaration node during exit phase.
    fn declaration_exit(&mut self, _decl: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each Rule node during walking.
    fn rule(&mut self, _rule: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each Rule node during exit phase.
    fn rule_exit(&mut self, _rule: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each AtRule node during walking.
    fn at_rule(&mut self, _at_rule: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each AtRule node during exit phase.
    fn at_rule_exit(&mut self, _at_rule: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each Comment node during walking.
    fn comment(&mut self, _comment: NodeId, _ss: &mut Stylesheet) {}

    /// Called for each Comment node during exit phase.
    fn comment_exit(&mut self, _comment: NodeId, _ss: &mut Stylesheet) {}

    /// Whether this plugin has any visitor hooks (non-Once hooks).
    /// Override to return true if the plugin uses Declaration/Rule/AtRule/Comment hooks.
    fn has_visitors(&self) -> bool {
        false
    }
}
