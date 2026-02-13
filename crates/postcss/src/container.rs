//! Container operations — tree walking and mutation with iterator safety.
//!
//! Port of PostCSS 8.4.31 `container.js`.
//!
//! The critical invariant is that `each()` / `walk()` handle mutations
//! (insert, remove) during iteration by adjusting active iterator indices,
//! exactly matching JS PostCSS behavior.

use smallvec::SmallVec;

use crate::ast::*;
use crate::raws::Raws;

/// Result of a walk/each callback: continue or stop.
pub enum WalkAction {
    Continue,
    Stop,
}

impl Stylesheet {
    // ── Iteration ──────────────────────────────────────────────────────────

    /// Iterate direct children of a container node, calling `callback` for each.
    ///
    /// Handles mutations during iteration: if nodes are inserted before the
    /// current index, the iterator adjusts forward; if removed, backward.
    ///
    /// Returns `true` if iteration was stopped early (callback returned Stop).
    pub fn each<F>(&mut self, node_id: NodeId, mut callback: F) -> bool
    where
        F: FnMut(NodeId, usize, &mut Stylesheet) -> WalkAction,
    {
        let iter_id = self.next_iterator_id;
        self.next_iterator_id += 1;
        self.iterators.insert(iter_id, 0);

        loop {
            let idx = *self.iterators.get(&iter_id).unwrap();
            let children_len = self
                .node(node_id)
                .children()
                .map_or(0, |c| c.len());

            if idx >= children_len {
                break;
            }

            let child_id = self.node(node_id).children().unwrap()[idx];

            match callback(child_id, idx, self) {
                WalkAction::Stop => {
                    self.iterators.remove(&iter_id);
                    return true;
                }
                WalkAction::Continue => {}
            }

            // Advance iterator.
            if let Some(i) = self.iterators.get_mut(&iter_id) {
                *i += 1;
            }
        }

        self.iterators.remove(&iter_id);
        false
    }

    /// Walk the tree depth-first, calling `callback` for every node.
    pub fn walk<F>(&mut self, node_id: NodeId, callback: &mut F) -> bool
    where
        F: FnMut(NodeId, &mut Stylesheet) -> WalkAction,
    {
        // We need to manually manage the traversal to avoid borrow issues.
        let children: Vec<NodeId> = self
            .node(node_id)
            .children()
            .map(|c| c.to_vec())
            .unwrap_or_default();

        for child_id in children {
            match callback(child_id, self) {
                WalkAction::Stop => return true,
                WalkAction::Continue => {}
            }
            // Recurse into children if it's a container.
            if self.node(child_id).is_container() {
                if self.walk(child_id, callback) {
                    return true;
                }
            }
        }
        false
    }

    /// Walk all Rule nodes.
    pub fn walk_rules<F>(&mut self, node_id: NodeId, callback: &mut F) -> bool
    where
        F: FnMut(NodeId, &mut Stylesheet) -> WalkAction,
    {
        self.walk(node_id, &mut |id, ss| {
            if ss.node(id).node_type() == "rule" {
                callback(id, ss)
            } else {
                WalkAction::Continue
            }
        })
    }

    /// Walk all Declaration nodes.
    pub fn walk_decls<F>(&mut self, node_id: NodeId, callback: &mut F) -> bool
    where
        F: FnMut(NodeId, &mut Stylesheet) -> WalkAction,
    {
        self.walk(node_id, &mut |id, ss| {
            if ss.node(id).node_type() == "decl" {
                callback(id, ss)
            } else {
                WalkAction::Continue
            }
        })
    }

    /// Walk all AtRule nodes.
    pub fn walk_at_rules<F>(&mut self, node_id: NodeId, callback: &mut F) -> bool
    where
        F: FnMut(NodeId, &mut Stylesheet) -> WalkAction,
    {
        self.walk(node_id, &mut |id, ss| {
            if ss.node(id).node_type() == "atrule" {
                callback(id, ss)
            } else {
                WalkAction::Continue
            }
        })
    }

    /// Walk all Comment nodes.
    pub fn walk_comments<F>(&mut self, node_id: NodeId, callback: &mut F) -> bool
    where
        F: FnMut(NodeId, &mut Stylesheet) -> WalkAction,
    {
        self.walk(node_id, &mut |id, ss| {
            if ss.node(id).node_type() == "comment" {
                callback(id, ss)
            } else {
                WalkAction::Continue
            }
        })
    }

    // ── Mutation ───────────────────────────────────────────────────────────

    /// Append a child node to a container.
    pub fn append(&mut self, parent_id: NodeId, child_id: NodeId) {
        self.node_mut(child_id).set_parent(Some(parent_id));
        let parent = self.node_mut(parent_id);
        if let Some(children) = parent.children_mut() {
            children.push(child_id);
        }
    }

    /// Prepend a child node to a container.
    pub fn prepend(&mut self, parent_id: NodeId, child_id: NodeId) {
        self.node_mut(child_id).set_parent(Some(parent_id));
        let parent = self.node_mut(parent_id);
        if let Some(children) = parent.children_mut() {
            children.insert(0, child_id);
            // Adjust active iterators: shift all indices forward by 1.
        }
        // Adjust active iterators on the parent.
        for (_, idx) in self.iterators.iter_mut() {
            *idx += 1;
        }
    }

    /// Insert a child node before an existing child in a container.
    pub fn insert_before(&mut self, parent_id: NodeId, existing_id: NodeId, new_id: NodeId) {
        self.node_mut(new_id).set_parent(Some(parent_id));

        let index = self.index_of(parent_id, existing_id);

        if let Some(children) = self.node_mut(parent_id).children_mut() {
            children.insert(index, new_id);
        }

        // Adjust active iterators: indices at or after the insertion point shift.
        for (_, idx) in self.iterators.iter_mut() {
            if *idx >= index {
                *idx += 1;
            }
        }
    }

    /// Insert a child node after an existing child in a container.
    pub fn insert_after(&mut self, parent_id: NodeId, existing_id: NodeId, new_id: NodeId) {
        self.node_mut(new_id).set_parent(Some(parent_id));

        let index = self.index_of(parent_id, existing_id);

        if let Some(children) = self.node_mut(parent_id).children_mut() {
            if index + 1 >= children.len() {
                children.push(new_id);
            } else {
                children.insert(index + 1, new_id);
            }
        }

        // Adjust active iterators: indices after the insertion point shift.
        for (_, idx) in self.iterators.iter_mut() {
            if *idx > index {
                *idx += 1;
            }
        }
    }

    /// Remove a child node from its parent container.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        let index = self.index_of(parent_id, child_id);

        if let Some(children) = self.node_mut(parent_id).children_mut() {
            children.remove(index);
        }

        // Clear parent reference.
        self.node_mut(child_id).set_parent(None);

        // Adjust active iterators: indices at or after removal shift back.
        for (_, idx) in self.iterators.iter_mut() {
            if *idx >= index && *idx > 0 {
                *idx -= 1;
            }
        }
    }

    /// Remove a node from its parent (convenience method).
    pub fn remove_node(&mut self, node_id: NodeId) {
        if let Some(parent_id) = self.node(node_id).parent() {
            self.remove_child(parent_id, node_id);
        }
    }

    /// Remove all children from a container.
    pub fn remove_all(&mut self, parent_id: NodeId) {
        let children: Vec<NodeId> = self
            .node(parent_id)
            .children()
            .map(|c| c.to_vec())
            .unwrap_or_default();

        for child_id in children {
            self.node_mut(child_id).set_parent(None);
        }

        if let Some(c) = self.node_mut(parent_id).children_mut() {
            c.clear();
        }
    }

    /// Replace a node with one or more replacement nodes.
    pub fn replace_with(&mut self, node_id: NodeId, replacements: Vec<NodeId>) {
        if let Some(parent_id) = self.node(node_id).parent() {
            let index = self.index_of(parent_id, node_id);

            // Remove the original.
            if let Some(children) = self.node_mut(parent_id).children_mut() {
                children.remove(index);
            }
            self.node_mut(node_id).set_parent(None);

            // Insert replacements at the same position.
            for (offset, rep_id) in replacements.iter().enumerate() {
                self.node_mut(*rep_id).set_parent(Some(parent_id));
                if let Some(children) = self.node_mut(parent_id).children_mut() {
                    children.insert(index + offset, *rep_id);
                }
            }

            // Adjust iterators: the net change is (replacements.len() - 1).
            let count = replacements.len();
            for (_, idx) in self.iterators.iter_mut() {
                if *idx >= index {
                    if count > 1 {
                        *idx += count - 1;
                    } else if count == 0 && *idx > 0 {
                        *idx -= 1;
                    }
                }
            }
        }
    }

    /// Find the index of a child within a parent's children.
    pub fn index_of(&self, parent_id: NodeId, child_id: NodeId) -> usize {
        self.node(parent_id)
            .children()
            .and_then(|children| children.iter().position(|&id| id == child_id))
            .expect("child not found in parent")
    }

    // ── Cloning ────────────────────────────────────────────────────────────

    /// Deep-clone a node and all its descendants into the arena.
    /// The cloned nodes have no parent set (caller must attach them).
    pub fn clone_node(&mut self, node_id: NodeId) -> NodeId {
        let node = self.node(node_id).clone();
        let children_to_clone: Vec<NodeId> = node.children().map(|c| c.to_vec()).unwrap_or_default();

        // Create the clone without children first.
        let mut cloned = node;
        // Clear children — we'll add cloned children.
        if let Some(c) = cloned.children_mut() {
            c.clear();
        }
        cloned.set_parent(None);

        let cloned_id = self.add_node(cloned);

        // Recursively clone children.
        for child_id in children_to_clone {
            let cloned_child_id = self.clone_node(child_id);
            self.node_mut(cloned_child_id).set_parent(Some(cloned_id));
            if let Some(children) = self.node_mut(cloned_id).children_mut() {
                children.push(cloned_child_id);
            }
        }

        cloned_id
    }

    // ── Node creation helpers ──────────────────────────────────────────────

    /// Create a new Rule node (not yet attached to any parent).
    pub fn create_rule(&mut self, selector: &str) -> NodeId {
        self.add_node(CssNode::Rule(RuleData {
            selector: selector.to_string(),
            children: SmallVec::new(),
            parent: None,
            raws: Raws::default(),
            source: None,
            is_clean: false,
        }))
    }

    /// Create a new Declaration node.
    pub fn create_decl(&mut self, prop: &str, value: &str) -> NodeId {
        self.add_node(CssNode::Declaration(DeclData {
            prop: prop.to_string(),
            value: value.to_string(),
            important: false,
            parent: None,
            raws: Raws::default(),
            source: None,
            is_clean: false,
        }))
    }

    /// Create a new AtRule node.
    pub fn create_at_rule(&mut self, name: &str, params: &str) -> NodeId {
        self.add_node(CssNode::AtRule(AtRuleData {
            name: name.to_string(),
            params: params.to_string(),
            children: Some(SmallVec::new()),
            parent: None,
            raws: Raws::default(),
            source: None,
            is_clean: false,
        }))
    }

    /// Create a new Comment node.
    pub fn create_comment(&mut self, text: &str) -> NodeId {
        self.add_node(CssNode::Comment(CommentData {
            text: text.to_string(),
            parent: None,
            raws: Raws::default(),
            source: None,
            is_clean: false,
        }))
    }
}
