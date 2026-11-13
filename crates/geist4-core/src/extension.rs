//! The extension API.
//!
//! An extension adds parsing or rendering behavior through hooks that
//! the parser and renderer call at fixed points. The built-in
//! extensions in the `geist4-ext` crate use exactly this public API,
//! and the loadable extension format (see `docs/extensions/format.md`)
//! compiles down to it, so the surface stays honest and small.
//!
//! With no extensions registered, every hook site sees an empty
//! registry and behaves exactly like plain CommonMark; nothing here
//! can change how the specification's examples parse.
//!
//! Hooks are pure: no I/O, no clock, no randomness, no global state.
//! The same source and options always produce the same tree, which is
//! what makes a declarative definition and a sandboxed WebAssembly
//! extension interchangeable.

use std::sync::Arc;

/// A value of an extension node's attribute.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    /// A string value (URLs, identifiers, raw text).
    Str(String),
    /// An integer value (levels, counts, flags as numbers).
    Int(i64),
    /// A boolean value.
    Bool(bool),
}

/// An extension node: the kind name, its attributes, and children.
///
/// Kinds are data rather than Rust variants so that any binding — and
/// the declarative tier of the loadable format — can construct and
/// inspect them uniformly. An extension declares the kinds it adds
/// when it is registered.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtNode {
    /// The kind name, as the extension declared it.
    pub kind: String,
    /// The attributes in declaration order.
    pub attrs: Vec<(String, AttrValue)>,
}

/// The declaration of one node kind an extension adds.
#[derive(Debug, Clone, PartialEq)]
pub struct KindDecl {
    /// The kind name, unique among the registered extensions.
    pub kind: String,
    /// Whether nodes of this kind have children.
    pub container: bool,
    /// The attribute names the kind accepts, in order.
    pub attrs: Vec<String>,
}

/// An extension: hooks for block starts and continuation, inline
/// triggers, post-processing passes, and renderers for its node kinds.
///
/// Every hook has a default implementation, so an extension enables
/// only what it needs. Extensions run in registration order.
pub trait Extension: Send + Sync {
    /// The extension's name, unique among the registered extensions.
    fn name(&self) -> &str;

    /// The node kinds this extension adds, declared up front.
    fn kinds(&self) -> Vec<KindDecl> {
        Vec::new()
    }

    /// The bytes at which the inline parser hands control to this
    /// extension's [`inline_trigger`](Extension::inline_trigger) hook.
    /// Built-in trigger bytes (`\`, `` ` ``, `&`, `<`, `*`, `_`, `[`,
    /// `!`, `]`, line breaks) never reach the hook.
    fn inline_triggers(&self) -> &[u8] {
        &[]
    }

    /// Runs when the inline parser reaches one of this extension's
    /// trigger bytes. The hook may scan a delimiter run — which the
    /// emphasis machinery then pairs with a matching run — or emit
    /// nodes directly. The cursor must end past the bytes the hook
    /// consumed; a hook that leaves it at the trigger byte makes that
    /// byte literal text.
    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let _ = ctx;
    }

    /// Runs once after the document tree is complete, before the
    /// document is returned. Hooks may rewrite kinds, set attributes,
    /// or splice nodes; this is where heading identifiers, task list
    /// items, footnote collection and the disallowed raw HTML filter
    /// work.
    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_>) {
        let _ = ctx;
    }

    /// Renders one of this extension's nodes. `children` is the
    /// already-rendered HTML of the node's children; the hook appends
    /// its output to `out`. Unrendered kinds produce no output and are
    /// reported by `check`.
    fn render(&self, node: &ExtNode, children: &str, out: &mut String) {
        let _ = (node, children, out);
    }
}

/// A delimiter run scanned by an inline trigger hook.
///
/// The run's characters were consumed from the subject and pushed as a
/// text node; the emphasis machinery later pairs runs whose openers
/// and closers it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// How many delimiter characters the run holds.
    pub length: usize,
    /// Whether the run may open (left-flanking, as for emphasis).
    pub can_open: bool,
    /// Whether the run may close (right-flanking, as for emphasis).
    pub can_close: bool,
}

/// The context an extension's inline trigger hook runs with.
///
/// The parser hands the hook the cursor at the trigger byte. Scanning
/// a run consumes its characters and creates their text node;
/// [`push_delim`](InlineTriggerCtx::push_delim) then puts the run on
/// the delimiter stack tagged with a node kind, so a later matching
/// run wraps the content between them in a node of that kind.
///
/// The methods are implemented next to the inline parser, which owns
/// the state they drive.
pub struct InlineTriggerCtx<'p, 'defs, 'a, 'ext> {
    pub(crate) inner: crate::inline::TriggerInner<'p, 'defs, 'a, 'ext>,
}

/// The context of a post-processing pass over the document tree.
pub struct TreeCtx<'ctx, 'doc, 'opts> {
    /// The document's arena; hooks rewrite nodes through it.
    pub arena: &'ctx mut crate::arena::Arena<'doc>,
    /// The tree root.
    pub root: crate::arena::NodeId,
    /// The options the parse runs with; hooks may read other
    /// extensions' declarations, but never their private state.
    pub options: &'opts Options,
}

/// The parse and render options: the registered extensions, in order.
///
/// Registration order is run order; extensions that must see another's
/// output register after it.
#[derive(Default)]
pub struct Options {
    extensions: Vec<Arc<dyn Extension>>,
}

impl Options {
    /// Empty options: plain CommonMark, no extensions.
    pub fn new() -> Self {
        Options::default()
    }

    /// Registers an extension, keeping the existing registration
    /// order.
    pub fn extension(&mut self, ext: Arc<dyn Extension>) -> &mut Self {
        self.extensions.push(ext);
        self
    }

    /// The registered extensions, in run order.
    pub fn extensions(&self) -> &[Arc<dyn Extension>] {
        &self.extensions
    }

    /// Whether no extension is registered; hook sites short-circuit on
    /// this.
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }

    /// Looks up a registered extension by name.
    pub fn by_name(&self, name: &str) -> Option<&Arc<dyn Extension>> {
        self.extensions.iter().find(|e| e.name() == name)
    }
}

impl Clone for Options {
    fn clone(&self) -> Self {
        Options {
            extensions: self.extensions.clone(),
        }
    }
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field(
                "extensions",
                &self
                    .extensions
                    .iter()
                    .map(|e| e.name().to_string())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}
