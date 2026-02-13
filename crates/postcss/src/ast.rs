use std::collections::HashMap;

use smallvec::SmallVec;

use crate::raws::Raws;

/// Index into the `Stylesheet` node arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Source position (1-indexed line/column, 0-indexed offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub offset: usize,
}

/// Source location span for a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub start: Position,
    pub end: Option<Position>,
    /// Index of the `Input` that produced this node (for multi-file support).
    pub input_id: usize,
}

// ── Node data structs ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RootData {
    pub children: SmallVec<[NodeId; 8]>,
    pub raws: Raws,
    pub source: Option<Source>,
    /// Cache for detected raw formatting values (mirrors JS `root.rawCache`).
    pub raw_cache: HashMap<String, String>,
    pub is_clean: bool,
}

#[derive(Debug, Clone)]
pub struct RuleData {
    pub selector: String,
    pub children: SmallVec<[NodeId; 8]>,
    pub parent: Option<NodeId>,
    pub raws: Raws,
    pub source: Option<Source>,
    /// Per-iteration dirty flag (mirrors JS `isClean` symbol).
    pub is_clean: bool,
}

#[derive(Debug, Clone)]
pub struct AtRuleData {
    /// The at-rule name without the leading `@`.
    pub name: String,
    /// The parameter string (e.g. `"(min-width: 480px)"`).
    pub params: String,
    /// Child nodes — `None` for statement at-rules (e.g. `@import`).
    pub children: Option<SmallVec<[NodeId; 8]>>,
    pub parent: Option<NodeId>,
    pub raws: Raws,
    pub source: Option<Source>,
    pub is_clean: bool,
}

#[derive(Debug, Clone)]
pub struct DeclData {
    pub prop: String,
    pub value: String,
    pub important: bool,
    pub parent: Option<NodeId>,
    pub raws: Raws,
    pub source: Option<Source>,
    pub is_clean: bool,
}

#[derive(Debug, Clone)]
pub struct CommentData {
    pub text: String,
    pub parent: Option<NodeId>,
    pub raws: Raws,
    pub source: Option<Source>,
    pub is_clean: bool,
}

// ── CssNode enum ───────────────────────────────────────────────────────────

/// A node in the PostCSS AST.
#[derive(Debug, Clone)]
pub enum CssNode {
    Root(RootData),
    Rule(RuleData),
    AtRule(AtRuleData),
    Declaration(DeclData),
    Comment(CommentData),
}

impl CssNode {
    /// Returns the node type string, matching PostCSS JS conventions.
    pub fn node_type(&self) -> &'static str {
        match self {
            CssNode::Root(_) => "root",
            CssNode::Rule(_) => "rule",
            CssNode::AtRule(_) => "atrule",
            CssNode::Declaration(_) => "decl",
            CssNode::Comment(_) => "comment",
        }
    }

    /// Returns a reference to the node's raws.
    pub fn raws(&self) -> &Raws {
        match self {
            CssNode::Root(d) => &d.raws,
            CssNode::Rule(d) => &d.raws,
            CssNode::AtRule(d) => &d.raws,
            CssNode::Declaration(d) => &d.raws,
            CssNode::Comment(d) => &d.raws,
        }
    }

    /// Returns a mutable reference to the node's raws.
    pub fn raws_mut(&mut self) -> &mut Raws {
        match self {
            CssNode::Root(d) => &mut d.raws,
            CssNode::Rule(d) => &mut d.raws,
            CssNode::AtRule(d) => &mut d.raws,
            CssNode::Declaration(d) => &mut d.raws,
            CssNode::Comment(d) => &mut d.raws,
        }
    }

    /// Returns the parent NodeId, if any.
    pub fn parent(&self) -> Option<NodeId> {
        match self {
            CssNode::Root(_) => None,
            CssNode::Rule(d) => d.parent,
            CssNode::AtRule(d) => d.parent,
            CssNode::Declaration(d) => d.parent,
            CssNode::Comment(d) => d.parent,
        }
    }

    /// Sets the parent NodeId.
    pub fn set_parent(&mut self, parent: Option<NodeId>) {
        match self {
            CssNode::Root(_) => {} // Root has no parent
            CssNode::Rule(d) => d.parent = parent,
            CssNode::AtRule(d) => d.parent = parent,
            CssNode::Declaration(d) => d.parent = parent,
            CssNode::Comment(d) => d.parent = parent,
        }
    }

    /// Returns the children slice if this is a container node.
    pub fn children(&self) -> Option<&[NodeId]> {
        match self {
            CssNode::Root(d) => Some(&d.children),
            CssNode::Rule(d) => Some(&d.children),
            CssNode::AtRule(d) => d.children.as_deref(),
            CssNode::Declaration(_) | CssNode::Comment(_) => None,
        }
    }

    /// Returns a mutable reference to children if this is a container node.
    pub fn children_mut(&mut self) -> Option<&mut SmallVec<[NodeId; 8]>> {
        match self {
            CssNode::Root(d) => Some(&mut d.children),
            CssNode::Rule(d) => Some(&mut d.children),
            CssNode::AtRule(d) => d.children.as_mut(),
            CssNode::Declaration(_) | CssNode::Comment(_) => None,
        }
    }

    /// Ensures this node has a children vec (for AtRule that starts without one).
    pub fn ensure_children(&mut self) {
        if let CssNode::AtRule(d) = self {
            if d.children.is_none() {
                d.children = Some(SmallVec::new());
            }
        }
    }

    /// Returns the source location if set.
    pub fn source(&self) -> Option<&Source> {
        match self {
            CssNode::Root(d) => d.source.as_ref(),
            CssNode::Rule(d) => d.source.as_ref(),
            CssNode::AtRule(d) => d.source.as_ref(),
            CssNode::Declaration(d) => d.source.as_ref(),
            CssNode::Comment(d) => d.source.as_ref(),
        }
    }

    /// Returns a mutable reference to the source location.
    pub fn source_mut(&mut self) -> &mut Option<Source> {
        match self {
            CssNode::Root(d) => &mut d.source,
            CssNode::Rule(d) => &mut d.source,
            CssNode::AtRule(d) => &mut d.source,
            CssNode::Declaration(d) => &mut d.source,
            CssNode::Comment(d) => &mut d.source,
        }
    }

    /// Get/set the is_clean flag (Root is always queried separately via raw_cache).
    pub fn is_clean(&self) -> bool {
        match self {
            CssNode::Root(d) => d.is_clean,
            CssNode::Rule(d) => d.is_clean,
            CssNode::AtRule(d) => d.is_clean,
            CssNode::Declaration(d) => d.is_clean,
            CssNode::Comment(d) => d.is_clean,
        }
    }

    pub fn set_clean(&mut self, clean: bool) {
        match self {
            CssNode::Root(d) => d.is_clean = clean,
            CssNode::Rule(d) => d.is_clean = clean,
            CssNode::AtRule(d) => d.is_clean = clean,
            CssNode::Declaration(d) => d.is_clean = clean,
            CssNode::Comment(d) => d.is_clean = clean,
        }
    }

    /// Returns true if this node is a container (can have children).
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            CssNode::Root(_) | CssNode::Rule(_) | CssNode::AtRule(_)
        )
    }
}

// ── Stylesheet arena ───────────────────────────────────────────────────────

/// The arena-based stylesheet. All nodes live in a flat `Vec` and reference
/// each other by `NodeId`. This avoids Rc/RefCell overhead and enables
/// cache-friendly traversal.
#[derive(Debug)]
pub struct Stylesheet {
    /// The node arena.
    pub nodes: Vec<CssNode>,
    /// The root node id.
    pub root: NodeId,
    /// Active iterator states (iterator_id → current_index).
    /// Used by `each()` to safely handle mutations during iteration.
    pub(crate) iterators: HashMap<usize, usize>,
    /// Next iterator id (monotonically increasing).
    pub(crate) next_iterator_id: usize,
}

impl Stylesheet {
    /// Create a new Stylesheet with an empty Root node.
    pub fn new() -> Self {
        let root = CssNode::Root(RootData {
            children: SmallVec::new(),
            raws: Raws::default(),
            source: None,
            raw_cache: HashMap::new(),
            is_clean: false,
        });
        Stylesheet {
            nodes: vec![root],
            root: NodeId(0),
            iterators: HashMap::new(),
            next_iterator_id: 0,
        }
    }

    /// Add a node to the arena, returning its NodeId.
    pub fn add_node(&mut self, node: CssNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// Get a reference to a node by id.
    pub fn node(&self, id: NodeId) -> &CssNode {
        &self.nodes[id.0]
    }

    /// Get a mutable reference to a node by id.
    pub fn node_mut(&mut self, id: NodeId) -> &mut CssNode {
        &mut self.nodes[id.0]
    }

    /// Get the parent NodeId of a node.
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent()
    }

    /// Get the children of a container node.
    pub fn children_of(&self, id: NodeId) -> Option<&[NodeId]> {
        self.node(id).children()
    }

    /// Get the first child of a container node.
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.children_of(id)
            .and_then(|children| children.first().copied())
    }

    /// Get the last child of a container node.
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.children_of(id)
            .and_then(|children| children.last().copied())
    }

    /// Find the root NodeId by walking up from any node.
    pub fn root_of(&self, id: NodeId) -> NodeId {
        let mut current = id;
        while let Some(parent) = self.parent_of(current) {
            current = parent;
        }
        current
    }

    /// Count the depth of a node (number of ancestors before root).
    pub fn depth(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut current = id;
        while let Some(parent) = self.parent_of(current) {
            if self.node(parent).node_type() != "root" {
                depth += 1;
            }
            current = parent;
        }
        depth
    }

    /// Clear the raw cache on the root node.
    pub fn clear_raw_cache(&mut self) {
        if let CssNode::Root(d) = &mut self.nodes[self.root.0] {
            d.raw_cache.clear();
        }
    }

    /// Get a cached raw value from the root node.
    pub fn get_raw_cache(&self, key: &str) -> Option<&String> {
        if let CssNode::Root(d) = &self.nodes[self.root.0] {
            d.raw_cache.get(key)
        } else {
            None
        }
    }

    /// Set a cached raw value on the root node.
    pub fn set_raw_cache(&mut self, key: String, value: String) {
        if let CssNode::Root(d) = &mut self.nodes[self.root.0] {
            d.raw_cache.insert(key, value);
        }
    }

    /// Mark a node and all its ancestors as dirty (not clean).
    pub fn mark_dirty(&mut self, id: NodeId) {
        let mut current = id;
        self.node_mut(current).set_clean(false);
        while let Some(parent) = self.parent_of(current) {
            self.node_mut(parent).set_clean(false);
            current = parent;
        }
    }
}

impl Default for Stylesheet {
    fn default() -> Self {
        Self::new()
    }
}
