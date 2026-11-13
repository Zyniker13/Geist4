//! The document tree: nodes in one arena, referred to by index.

use crate::extension::AttrValue;
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
    /// Per-node extension attributes, indexed by node id. Entries stay
    /// empty (and unallocated) until a hook sets an attribute.
    attrs: Vec<Vec<(String, crate::extension::AttrValue)>>,
    /// Per-node owning extension, indexed by node id: the extension's
    /// position in the parse options' extension list, set when the parse
    /// phase claims a built-in node for an extension.
    ext_owner: Vec<Option<usize>>,
}

impl<'a> Arena<'a> {
    /// Creates an empty arena.
    #[must_use]
    pub fn new() -> Self {
        Arena {
            nodes: Vec::new(),
            attrs: Vec::new(),
            ext_owner: Vec::new(),
        }
    }

    /// Reserves room for `additional` more nodes, so bulk allocation does
    /// not repeatedly move the whole node vector.
    pub fn reserve(&mut self, additional: usize) {
        self.nodes.reserve(additional);
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
        let kids = &mut self.nodes[parent.0].children;
        // Most containers hold a handful of children; one reserve keeps
        // the 1 -> 2 -> 4 regrowth from reallocating per append.
        if kids.capacity() == 0 {
            kids.reserve(4);
        }
        kids.push(child);
    }

    /// Removes and returns the last child of `parent`, if any.
    pub fn pop_child(&mut self, parent: NodeId) -> Option<NodeId> {
        self.nodes[parent.0].children.pop()
    }

    /// Sets an attribute on a node, replacing a same-named attribute and
    /// keeping the first set order otherwise. Attributes carry extension
    /// state on a node the parse phase claimed; the renderer hands them
    /// to the claiming extension's render override.
    pub fn set_attr(&mut self, id: NodeId, name: impl Into<String>, value: AttrValue) {
        let slot = self.attr_slot(id);
        let name = name.into();
        if let Some(existing) = slot.iter_mut().find(|(n, _)| *n == name) {
            existing.1 = value;
        } else {
            slot.push((name, value));
        }
    }

    /// The named attribute's value on a node, if a hook set one.
    #[must_use]
    pub fn attr(&self, id: NodeId, name: &str) -> Option<&AttrValue> {
        self.attrs
            .get(id.0)
            .and_then(|attrs| attrs.iter().find(|(n, _)| n == name))
            .map(|(_, value)| value)
    }

    /// The attributes set on a node, in set order; empty when none were.
    #[must_use]
    pub fn attrs(&self, id: NodeId) -> &[(String, AttrValue)] {
        self.attrs.get(id.0).map_or(&[], Vec::as_slice)
    }

    /// Marks the node as claimed by the extension at `index` in the
    /// parse options' extension list. A claimed node renders through
    /// that extension's [`Extension::render_override`](crate::extension::Extension::render_override()) hook instead of
    /// its built-in renderer.
    pub fn set_extension(&mut self, id: NodeId, index: usize) {
        let slot = self.owner_slot(id);
        *slot = Some(index);
    }

    /// The index of the extension that claimed the node during parsing,
    /// if any.
    #[must_use]
    pub fn extension_of(&self, id: NodeId) -> Option<usize> {
        self.ext_owner.get(id.0).copied().flatten()
    }

    /// The attribute slot for `id`, grown to fit.
    fn attr_slot(&mut self, id: NodeId) -> &mut Vec<(String, AttrValue)> {
        if self.attrs.len() <= id.0 {
            self.attrs.resize_with(id.0 + 1, Vec::new);
        }
        &mut self.attrs[id.0]
    }

    /// The owner slot for `id`, grown to fit.
    fn owner_slot(&mut self, id: NodeId) -> &mut Option<usize> {
        if self.ext_owner.len() <= id.0 {
            self.ext_owner.resize_with(id.0 + 1, || None);
        }
        &mut self.ext_owner[id.0]
    }

    /// Inserts `child` into `parent`'s children at `index`, shifting the
    /// rest right. Post-processing passes splice nodes this way: one
    /// text node becomes text, link and text again.
    pub fn insert_child(&mut self, parent: NodeId, index: usize, child: NodeId) {
        let kids = &mut self.nodes[parent.0].children;
        if kids.capacity() == 0 {
            kids.reserve(4);
        }
        kids.insert(index.min(kids.len()), child);
    }

    /// Removes and returns the child of `parent` at `index`, if there
    /// is one. Post-processing passes drop the swallowed halves of
    /// merged text runs this way.
    pub fn remove_child(&mut self, parent: NodeId, index: usize) -> Option<NodeId> {
        let kids = &mut self.nodes[parent.0].children;
        if index < kids.len() {
            Some(kids.remove(index))
        } else {
            None
        }
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
