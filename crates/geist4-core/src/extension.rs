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
    /// Whether nodes of this kind hold inline text: a text child is
    /// parsed as inline content like a paragraph's, and the kind's
    /// children render as inline content.
    pub inline_text: bool,
    /// The attribute names the kind accepts, in order.
    pub attrs: Vec<String>,
}

/// The rendered HTML of one of an extension node's children, together
/// with the child's own node when that child is itself an extension
/// node, so a hook that arranges its children (as a table groups its
/// rows into `thead` and `tbody`) can read the structure it is
/// arranging. Children that are plain inline nodes carry no node.
pub struct RenderedChild<'a> {
    /// The child's node, when the child is an extension node.
    pub node: Option<&'a ExtNode>,
    /// The child's rendered HTML.
    pub html: String,
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
    /// rendered HTML of the node's children along with their nodes;
    /// the hook appends its output to `out`. Children of a kind that
    /// declared [`inline_text`](KindDecl::inline_text) render as
    /// inline content, others as blocks. Unrendered kinds produce no
    /// output and are reported by `check`.
    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        let _ = (node, children, out);
    }

    /// Runs where a new block could start, after the built-in block
    /// starts have all declined, once per line per extension in
    /// registration order. The hook opens one of its blocks with
    /// [`open_extension`](BlockStartCtx::open_extension), converts an
    /// open paragraph into one with
    /// [`convert_paragraph`](BlockStartCtx::convert_paragraph), or
    /// declines by doing nothing. Opening consumes the line.
    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        let _ = ctx;
    }

    /// Runs for each line while an extension block of this extension
    /// is open, to decide whether the line continues it. Returning
    /// false closes the block; the line is then free to start other
    /// blocks. The hook may read the block's state; it should not
    /// depend on mutations here surviving, because a line that both
    /// continues the block and starts a new one closes it first.
    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_>) -> bool {
        let _ = ctx;
        false
    }

    /// Runs when an extension block closes, before the tree is
    /// returned. The hook builds the block's children under its node
    /// from the state it kept while the block was open.
    fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        let _ = (state, ctx);
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

/// What the place a new block would open in is.
///
/// The block-start hook sees this as the deepest open block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartContainer {
    /// A container — the document, a block quote, a list item — that
    /// new blocks open inside.
    Container,
    /// An open paragraph. The hook can turn it into one of its blocks
    /// with [`convert_paragraph`](BlockStartCtx::convert_paragraph),
    /// keeping the paragraph's last line as the new block's first
    /// line.
    Paragraph,
    /// An extension block of the extension at this index is the
    /// deepest open block.
    Extension(usize),
}

/// The context of the block-start hook.
///
/// The hook sees the current line as it stands after the open
/// containers' prefixes were consumed, and what it would open in.
/// The methods are implemented next to the block parser, which owns
/// the state they drive.
pub struct BlockStartCtx<'p, 'a, 'l, 'ext> {
    pub(crate) inner: crate::parser::StartInner<'p, 'a, 'l, 'ext>,
}

/// The context of the block-continuation hook.
pub struct BlockContinueCtx<'c, 's> {
    /// The line's content from its first non-space byte, without the
    /// line ending.
    pub(crate) line: &'c [u8],
    /// The open block's state, taken out of the frame for the hook.
    pub(crate) state: Option<&'s mut dyn std::any::Any>,
}

/// The context of the block-close hook.
pub struct CloseCtx<'ctx, 'doc> {
    /// The document's arena; the hook builds its nodes through it.
    pub arena: &'ctx mut crate::arena::Arena<'doc>,
    /// The extension block's node, to build children under.
    pub node: crate::arena::NodeId,
    /// The byte offset where the block's first line began.
    pub start_offset: usize,
    /// The byte offset just past the block's last line.
    pub end_offset: usize,
}

impl<'p, 'a, 'l, 'ext> BlockStartCtx<'p, 'a, 'l, 'ext> {
    /// The index of the extension whose hook is running.
    pub fn extension(&self) -> usize {
        self.inner.extension_index()
    }

    /// The current line's content from its first non-space byte,
    /// without the line ending.
    pub fn line(&self) -> &'l [u8] {
        self.inner.line()
    }

    /// Whether the line is blank: only spaces and tabs remain.
    pub fn line_blank(&self) -> bool {
        self.inner.line_blank()
    }

    /// Whether the line's first non-space byte sits four or more
    /// virtual columns past the container prefixes.
    pub fn indented(&self) -> bool {
        self.inner.indented()
    }

    /// What the new block would open in.
    pub fn container(&self) -> StartContainer {
        self.inner.container()
    }

    /// The open paragraph's accumulated text, if the deepest open
    /// block is a paragraph. The text stays in the frame; the hook
    /// that wants it for its own block's first line passes the byte
    /// offset where that line begins to
    /// [`convert_paragraph`](Self::convert_paragraph).
    pub fn paragraph_text(&self) -> Option<&str> {
        self.inner.paragraph_text()
    }

    /// Marks the open paragraph as having declined this extension's
    /// block: the hook will not be called with it again. A delimiter
    /// row that matches no header row uses this, so a later line
    /// cannot turn a paragraph that failed once into a block after
    /// all.
    pub fn visit_paragraph(&mut self) {
        self.inner.visit_paragraph();
    }

    /// Converts the open paragraph into an extension block of the
    /// running extension: the paragraph's node becomes a node of
    /// `kind`, the text before byte `pre_len` becomes a paragraph of
    /// its own in front of it (with `\|` unescaped to `|` and the
    /// whole prefix trimmed, as the reference implementations prepare
    /// a table's pre-header text), and the state box travels with the
    /// new frame. Consumes the rest of the line.
    pub fn convert_paragraph(
        &mut self,
        kind: String,
        pre_len: usize,
        state: Box<dyn std::any::Any + Send>,
    ) {
        self.inner.convert_paragraph(kind, pre_len, state);
    }

    /// Opens an extension block of the running extension inside the
    /// container the hook was called with, interrupting an open
    /// paragraph if one is open. Consumes the rest of the line.
    pub fn open_extension(
        &mut self,
        kind: String,
        state: Box<dyn std::any::Any + Send>,
    ) -> crate::arena::NodeId {
        self.inner.open_extension(kind, state)
    }
}

impl<'c, 's> BlockContinueCtx<'c, 's> {
    /// The line's content from its first non-space byte, without the
    /// line ending.
    pub fn line(&self) -> &'c [u8] {
        self.line
    }

    /// The open block's state, downcast to the extension's own type.
    pub fn state<T: std::any::Any>(&mut self) -> Option<&mut T> {
        let state = self.state.as_deref_mut()?;
        state.downcast_mut::<T>()
    }
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
