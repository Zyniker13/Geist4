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
    /// The child's core kind name, when the child is a core node
    /// (see [`crate::node::Node::kind_name`]). Extension children
    /// carry `None` here and their kind in `node` instead, so a hook
    /// can tell what kind of block each child is without matching on
    /// rendered HTML.
    pub core_kind: Option<&'static str>,
    /// The child's rendered HTML.
    pub html: String,
}

/// A core node an extension claimed during parsing, handed to
/// [`Extension::render_override`](crate::extension::Extension::render_override()).
///
/// Claiming happens through [`crate::arena::Arena::set_extension`] (with attributes
/// set alongside through [`crate::arena::Arena::set_attr`]); the node then renders
/// through the claiming extension instead of its built-in renderer,
/// which is how task list items turn a list item into a checkbox.
pub struct CoreNode<'a> {
    /// The node kind's name: `"item"`, `"heading"`, `"paragraph"` and
    /// so on, matching the crate's node names.
    pub kind: &'a str,
    /// The attributes the parse phase set on the node, in set order.
    pub attrs: &'a [(String, AttrValue)],
}

/// The children of a claimed core node, handed to
/// [`Extension::render_override`](crate::extension::Extension::render_override()).
///
/// The hook writes to the output it is given and pulls the node's
/// children in, in order, with [`render_next`](Self::render_next),
/// which renders each child the way the built-in renderer would,
/// including its conditional newlines, against the output as it
/// stands. Children the hook never pulls produce no output.
pub struct OverrideChildren<'c, 'doc> {
    pub(crate) renderer: &'c mut crate::basic_html::Renderer<'doc>,
    pub(crate) children: &'c [crate::arena::NodeId],
    pub(crate) tight: bool,
    pub(crate) next: usize,
}

impl OverrideChildren<'_, '_> {
    /// How many children have not been rendered yet.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.children.len() - self.next
    }

    /// Renders the next child into `out` with the built-in renderer's
    /// conventions. Returns false when no child remains.
    pub fn render_next(&mut self, out: &mut String) -> bool {
        let Some(&child) = self.children.get(self.next) else {
            return false;
        };
        self.next += 1;
        self.renderer.render_block_into(child, self.tight, out);
        true
    }
}

/// An extension: hooks for block starts and continuation, inline
/// triggers, post-processing passes, and renderers for its node kinds.
///
/// Every hook has a default implementation, so an extension enables
/// only what it needs. Extensions run in registration order.
pub trait Extension: Send + Sync {
    /// The extension's name, unique among the registered extensions.
    fn name(&self) -> &str;

    /// The front matter delimiter this extension claims, if any. When
    /// an extension declares one, the core checks the document's very
    /// beginning before any line is processed: a matching prologue
    /// becomes a `front_matter` node holding the raw text, and the
    /// rest of the document parses as if the prologue were not there.
    /// The first registered extension whose delimiter matches wins.
    fn front_matter_delimiter(&self) -> Option<&str> {
        None
    }

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

    /// Runs when a `[`…`]` group matched no link: neither an inline
    /// `(` destination, nor a reference definition, nor an opener
    /// match. The hook may claim the whole group as one inline node of
    /// its own with [`claim`](InlineBracketCtx::claim). Check
    /// [`content_starts_with`](InlineBracketCtx::content_starts_with)
    /// before reading
    /// [`content`](InlineBracketCtx::content) — content exists only
    /// when the group is plain text and inline HTML, and building it
    /// per close bracket would make bracket-heavy input quadratic.
    /// An unclaimed group stays literal text, exactly as without the
    /// hook.
    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        let _ = ctx;
    }

    /// Whether this extension's blocks hold other blocks: while one of
    /// them is open, the parser opens paragraphs, lists, quotes, code
    /// and other blocks inside it, the way it opens blocks inside a
    /// block quote or list item. Extension blocks without this are
    /// line accumulators: their lines are kept in the hook's state and
    /// [`close`](Extension::close) builds the children.
    fn contains_blocks(&self) -> bool {
        false
    }

    /// A pure look at a line, answering whether this extension would
    /// start a block on it. The parser consults it when a lazy
    /// paragraph continuation is considered, so a line that would open
    /// one of this extension's blocks never lazily continues the
    /// paragraph — the same role the built-in block starts play there.
    /// It must not change any state.
    fn block_start_looks(&self, line: &[u8]) -> bool {
        let _ = line;
        false
    }

    /// Runs once at the start of each HTML render, before the first
    /// node is rendered. Hooks reset their per-render state here, so
    /// extensions that count or wrap output start clean even though
    /// the extension object itself is shared across renders.
    fn begin_render(&self) {}

    /// Runs once at the end of each HTML render, after the last node.
    /// The hook appends its output to `out` — a wrapper an extension
    /// opened during the render closes here.
    fn document_suffix(&self, out: &mut String) {
        let _ = out;
    }

    /// Runs once after the document tree is complete, before the
    /// document is returned. Hooks may rewrite kinds, set attributes,
    /// or splice nodes; this is where heading identifiers, task list
    /// items, footnote collection and the disallowed raw HTML filter
    /// work.
    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_>) {
        let _ = ctx;
    }

    /// Whether the HTML renderer must run raw HTML literals past this
    /// extension's [`html_filter`](Extension::html_filter) hook.
    /// Default false, so documents without a filtering extension pay
    /// nothing.
    fn filters_html(&self) -> bool {
        false
    }

    /// Decides whether a raw HTML literal that starts at `<` may pass
    /// through the output as is. Returning false has the renderer
    /// replace that one `<` with `&lt;`; every other `<` in the same
    /// literal is checked on its own, exactly like the reference's
    /// html-filter hook. The default allows everything.
    fn html_filter(&self, literal: &[u8]) -> bool {
        let _ = literal;
        true
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

    /// Renders a core node this extension claimed during parsing with
    /// [`crate::arena::Arena::set_extension`]. Runs instead of the node's built-in
    /// renderer: the hook writes the node's whole output — opening,
    /// children, closing — to `out`, pulling the children in with
    /// [`OverrideChildren::render_next`]. The children arrive the way
    /// the built-in renderer would have rendered them, so a hook that
    /// changes only the wrapping keeps the built-in child output.
    fn render_override(
        &self,
        node: &CoreNode<'_>,
        children: &mut OverrideChildren<'_, '_>,
        out: &mut String,
    ) {
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
    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> bool {
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

/// The context an inline bracket hook runs with.
///
/// The inline parser hands the hook the plain-text content of a
/// `[`…`]` group that matched no link, plus the input positions of the
/// brackets. Claiming replaces the group — the `[`, the content nodes
/// and the `]` — with one inline node of the hook's kind, the way the
/// emphasis machinery replaces its run pairs.
pub struct InlineBracketCtx<'p, 'defs, 'a, 'ext> {
    pub(crate) inner: crate::inline::BracketInner<'p, 'defs, 'a, 'ext>,
}

/// The context of the block-continuation hook.
pub struct BlockContinueCtx<'c, 's, 'l> {
    /// The line's content from its first non-space byte, without the
    /// line ending.
    pub(crate) line: &'c [u8],
    /// The open block's state, taken out of the frame for the hook.
    pub(crate) state: Option<&'s mut dyn std::any::Any>,
    /// The line cursor, positioned where the container prefixes left
    /// it; a hook that consumes a prefix off the line advances it.
    pub(crate) cursor: &'c mut crate::parser::Cursor<'l>,
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

    /// The arena, for hooks that build their block's children.
    pub fn arena(&mut self) -> &mut crate::arena::Arena<'a> {
        self.inner.arena_mut()
    }

    /// The deepest open frame's node when it is an extension block.
    /// Used with [`container`](Self::container)'s
    /// [`StartContainer::Extension`] to append to an open extension
    /// block.
    pub fn open_node(&self) -> Option<crate::arena::NodeId> {
        self.inner.open_node()
    }

    /// The open extension block's state, when the deepest open block
    /// is an extension block, downcast to the extension's own type.
    pub fn state<T: std::any::Any>(&mut self) -> Option<&mut T> {
        self.inner.frame_state()?.downcast_mut::<T>()
    }

    /// Consumes the rest of the line as content of the open block
    /// without opening a new one, for hooks that append to an open
    /// extension block directly.
    pub fn absorb_line(&mut self) {
        self.inner.absorb_line();
    }

    /// Consumes `bytes` more bytes of the line at the current content
    /// position without opening a block. The rest of the line becomes
    /// content of the open container as usual — typically a paragraph
    /// — and no further block-start scanning happens on the line. A
    /// hook that peels a prefix off a line the container itself keeps
    /// uses this, as the task list marker does.
    pub fn consume(&mut self, bytes: usize) {
        self.inner.consume(bytes);
    }

    /// The full current line, from its first byte — container prefixes
    /// and leading whitespace included — without its line ending. The
    /// hook's [`line`](Self::line) starts at the first non-space byte
    /// of the content; a hook that must match the line the way the
    /// reference implementation's scanners see it — from the very
    /// start, list markers and quote marks included — scans this.
    pub fn raw_line(&self) -> &'l [u8] {
        self.inner.raw_line()
    }

    /// The deepest open block's node when it is a plain container (the
    /// document, a block quote, a list or a list item), so a hook can
    /// inspect it and mark it with the arena.
    pub fn container_node(&self) -> Option<crate::arena::NodeId> {
        self.inner.container_node()
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
    /// new frame. Consumes the rest of the line. Returns the new
    /// block's node.
    pub fn convert_paragraph(
        &mut self,
        kind: String,
        pre_len: usize,
        state: Box<dyn std::any::Any + Send>,
    ) -> crate::arena::NodeId {
        self.inner.convert_paragraph(kind, pre_len, state)
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

    /// Opens a block like
    /// [`open_extension`](Self::open_extension), but leaves the rest of
    /// the line — everything after the marker the hook matched — for
    /// the parser to add to the new block as its first paragraph. Call
    /// [`seek_content`](Self::seek_content) to place the cursor first.
    pub fn open_extension_keep_rest(
        &mut self,
        kind: String,
        state: Box<dyn std::any::Any + Send>,
    ) -> crate::arena::NodeId {
        self.inner.open_extension_keep_rest(kind, state)
    }

    /// Moves the cursor to the line's first non-space byte and then
    /// `n` bytes further — the content after the marker bytes the hook
    /// matched. The hook opened its block with
    /// [`open_extension_keep_rest`](Self::open_extension_keep_rest).
    pub fn seek_content(&mut self, n: usize) {
        self.inner.cursor.skip_to_content();
        self.inner.cursor.advance_bytes(n);
    }
}

impl<'c, 's, 'l> BlockContinueCtx<'c, 's, 'l> {
    /// The line's content from its first non-space byte, without the
    /// line ending.
    pub fn line(&self) -> &'c [u8] {
        self.line
    }

    /// Whether nothing but the container prefixes remains on the line:
    /// the line is blank after the prefixes the walk already consumed.
    /// A continuation hook that accepts this must consume the whole
    /// line.
    pub fn pure_blank(&self) -> bool {
        self.cursor.rest_len() == 0
    }

    /// The columns between the cursor and the line's first non-space
    /// byte — the leading indentation the hook may consume from.
    #[must_use]
    pub fn indent(&self) -> usize {
        let (pos, col) = self.cursor.find_nonspace();
        let _ = pos;
        col - self.cursor.col()
    }

    /// The open block's state, downcast to the extension's own type.
    pub fn state<T: std::any::Any>(&mut self) -> Option<&mut T> {
        let state = self.state.as_deref_mut()?;
        state.downcast_mut::<T>()
    }

    /// Consumes `n` more columns off the line, advancing the cursor
    /// the way a container prefix does — through tabs, in columns. The
    /// parser opens whatever follows the consumed prefix inside the
    /// block.
    pub fn consume_cols(&mut self, n: usize) {
        self.cursor.advance_cols(n);
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
