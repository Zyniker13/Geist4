//! The document tree: nodes in one arena, referred to by index.

use crate::node::Node;
use crate::span::Span;

/// A handle to a node in the [`Arena`].
///
/// Nodes are stored in one `Vec` and refer to each other by index, so there
/// is no reference counting and no per-node allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

impl NodeId {
    /// The numeric index of this node.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

/// One stored node: its kind, its source span, and its children.
#[derive(Debug)]
pub struct NodeEntry<'a> {
    /// What kind of node this is.
    pub kind: Node<'a>,
    /// The source span the node was parsed from.
    pub span: Span,
    /// The child nodes, in document order.
    pub children: Vec<NodeId>,
}

/// An arena of nodes.
///
/// The document is an arena tree: nodes live in one `Vec` and refer to each
/// other by index, so there is no reference counting and no per-node
/// allocation. Every node records its source span as byte offsets into the
/// input.
#[derive(Debug, Default)]
pub struct Arena<'a> {
    nodes: Vec<NodeEntry<'a>>,
}

impl<'a> Arena<'a> {
    /// Creates an empty arena.
    #[must_use]
    pub fn new() -> Self {
        Arena { nodes: Vec::new() }
    }

    /// Allocates a node with no children and returns its id.
    pub fn alloc(&mut self, kind: Node<'a>, span: Span) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(NodeEntry {
            kind,
            span,
            children: Vec::new(),
        });
        id
    }

    /// The stored entry for `id`.
    #[must_use]
    pub fn entry(&self, id: NodeId) -> &NodeEntry<'a> {
        &self.nodes[id.0]
    }

    /// The mutable entry for `id`.
    pub fn entry_mut(&mut self, id: NodeId) -> &mut NodeEntry<'a> {
        &mut self.nodes[id.0]
    }

    /// The kind of node `id` holds.
    #[must_use]
    pub fn kind(&self, id: NodeId) -> &Node<'a> {
        &self.nodes[id.0].kind
    }

    /// The source span of node `id`.
    #[must_use]
    pub fn span(&self, id: NodeId) -> Span {
        self.nodes[id.0].span
    }

    /// The children of node `id`, in document order.
    #[must_use]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.0].children
    }

    /// The last child of node `id`, if any.
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].children.last().copied()
    }

    /// Appends `child` as the last child of `parent`.
    pub fn push_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent.0].children.push(child);
    }

    /// Removes and returns the last child of `parent`, if any.
    pub fn pop_child(&mut self, parent: NodeId) -> Option<NodeId> {
        self.nodes[parent.0].children.pop()
    }

    /// Replaces the kind of node `id`, keeping span and children.
    pub fn set_kind(&mut self, id: NodeId, kind: Node<'a>) {
        self.nodes[id.0].kind = kind;
    }

    /// Sets the source span of node `id`.
    pub fn set_span(&mut self, id: NodeId, span: Span) {
        self.nodes[id.0].span = span;
    }

    /// Extends the span of node `id` to also cover `other`.
    pub fn extend_span(&mut self, id: NodeId, other: Span) {
        let span = self.nodes[id.0].span.join(other);
        self.nodes[id.0].span = span;
    }

    /// How many nodes the arena holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the arena holds no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
