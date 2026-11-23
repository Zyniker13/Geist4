//! The block-structure parser: phase 1 of the two-phase CommonMark
//! algorithm.
//!
//! Lines are processed one at a time against a stack of open blocks, in the
//! shape the specification's appendix describes: match the open containers
//! against the line's prefix, close what no longer matches (except lazy
//! paragraph continuations, which stay open), open any new blocks the line
//! starts, then add the remaining text to the open leaf block. The mechanics
//! follow the reference implementation closely: per-container offsets are
//! relative to the parent's content column, list-item padding follows the
//! reference algorithm exactly, and per-node last-line-blank flags decide
//! tight versus loose lists when a list closes.

mod cursor;
mod leaf;
pub mod textmap;

use crate::arena::{Arena, NodeId};
use crate::node::{
    CodeBlockData, HeadingData, HtmlBlockData, ItemData, LinkDef, ListData, Node, SpannedDef,
};
use crate::span::Span;
pub(crate) use cursor::Cursor;
use std::borrow::Cow;
use std::collections::HashMap;
use textmap::LineAnchor;
pub use textmap::TextMap;

/// The mutable state of one open block while parsing.
enum OpenKind {
    /// The document root.
    Document,
    /// A `>` block quote.
    BlockQuote,
    /// A list holding the marker type of its items.
    List(ListData),
    /// A list item; `content_offset` is how many columns the item's content
    /// starts past its parent container's content column.
    Item { content_offset: usize },
    /// A paragraph accumulating text.
    Paragraph { text: String },
    /// A paragraph that became a setext heading; the node is finished and
    /// the frame only carries its span.
    Heading,
    /// A fenced or indented code block accumulating lines. For a fenced
    /// block the opening line's remainder (the info string) stays at the
    /// front of the literal until the block closes.
    CodeBlock {
        fenced: bool,
        fence_char: u8,
        fence_len: usize,
        fence_offset: usize,
        literal: String,
    },
    /// A raw HTML block of one of the seven kinds.
    HtmlBlock { kind: u8, literal: String },
    /// An extension block: which registered extension owns it and its
    /// opaque state, taken out for hook calls and put back after.
    Extension {
        index: usize,
        state: Option<Box<dyn std::any::Any + Send>>,
    },
}

/// One entry in the stack of open blocks.
struct Frame {
    id: NodeId,
    kind: OpenKind,
    /// The line number the block started on, counting from one.
    start_line: usize,
    /// The source offset where the block began.
    start_offset: usize,
    /// The source offset just past the block's last line so far.
    end_offset: usize,
}

/// A collected link reference definition with its normalized label.
/// The result of matching the open containers against one line.
enum WalkResult {
    /// Every open container matched.
    Matched,
    /// The frame at `depth` failed to match; frames at `depth..` must close.
    FailedAt(usize),
    /// A closing code fence consumed the line and closed its block; the rest
    /// of the line's processing is skipped.
    FenceClosed,
    /// An extension block closed with the line consumed whole — a closing
    /// fence; frames at `depth..` must close and the line's processing is
    /// skipped.
    ClosedAt(usize),
}

/// What the setext-underline check decided to do with an open paragraph.
enum SetextOutcome {
    /// The paragraph became a heading.
    Converted(NodeId),
    /// The paragraph was nothing but link definitions; it stays open with
    /// empty text and the line continues it.
    Dissolved,
    /// The line is not a setext underline.
    NotSetext,
}

/// A parsed document: the arena tree, the root node, and the link reference
/// definitions the document defines.
pub struct Document<'a> {
    /// The nodes of the document, stored in one arena.
    pub arena: Arena<'a>,
    /// The document root node.
    pub root: NodeId,
    /// The document's link reference definitions, keyed by their normalized
    /// label (lowercased, whitespace runs collapsed to one space).
    pub link_defs: HashMap<String, LinkDef<'a>>,
    /// Every link reference definition the document defines, in document
    /// order and including duplicates; see [`SpannedDef`].
    pub defs: Vec<SpannedDef<'a>>,
    /// The problems the parse noticed in its source: broken references
    /// and similar issues the tree absorbed without changing the
    /// output. Empty unless something was found; see
    /// [`crate::problems::Problem`].
    pub problems: Vec<crate::problems::Problem>,
}

/// Parses the block structure of `src` into a [`Document`].
///
/// The block phase stores each leaf block's text; a post-pass then parses
/// every paragraph's and heading's inline content, so reference definitions
/// are visible to every leaf block in the document, as in the reference
/// parser.
#[must_use]
pub fn parse_document(src: &str) -> Document<'_> {
    parse_with(src, &crate::extension::Options::new())
}

/// Parses `src` with the given options: the registered extensions add
/// hooks to the parse, and their post-processing passes run once the
/// tree is complete.
pub fn parse_with<'a>(src: &'a str, options: &crate::extension::Options) -> Document<'a> {
    let extensions: Option<&[std::sync::Arc<dyn crate::extension::Extension>]> =
        if options.is_empty() {
            None
        } else {
            Some(options.extensions())
        };
    // A parse begins: every extension gets the signal before the first
    // line, so per-parse state — a sandboxed component's instance, say —
    // is set up exactly once, in registration order.
    if let Some(extensions) = extensions {
        for ext in extensions {
            ext.begin_parse();
        }
    }
    let (mut arena, root, link_defs, mut problems, defs) =
        Parser::new(src).extensions(extensions).run();
    if !options.is_empty() {
        for ext in options.extensions() {
            ext.post_process(&mut crate::extension::TreeCtx {
                arena: &mut arena,
                root,
                options,
                problems: &mut problems,
            });
        }
    }
    Document {
        arena,
        root,
        link_defs,
        problems,
        defs,
    }
}

/// Parses a source *fragment* — a substring of a larger document — for
/// incremental reparsing: the block and inline phases run exactly as
/// [`parse_with`] runs them over a standalone source, except that the
/// extensions' `begin_parse` and `post_process` hooks do not fire
/// (those run once per assembled document, driven by the caller, over
/// the finished tree). Spans stay in fragment coordinates: the caller
/// shifts them into document coordinates.
///
/// The fragment must start at a line start; the caller guarantees it.
/// `src` is any `&str` slice — the fragment parse borrows it the same
/// way a full parse borrows its source, so the returned tree's
/// `Cow::Borrowed` payloads point into `src`.
#[must_use]
pub fn parse_fragment<'a>(src: &'a str, options: &crate::extension::Options) -> Document<'a> {
    let extensions: Option<&[std::sync::Arc<dyn crate::extension::Extension>]> =
        if options.is_empty() {
            None
        } else {
            Some(options.extensions())
        };
    // The front-matter claim reads only position zero; a fragment never
    // begins a document, so it must not fire. The cheapest correct way
    // to keep that guarantee is to hand the parser an options slice
    // that has no front-matter delimiter: every built-in's delimiter is
    // opt-in through the extension, so run the fragment through the
    // plain options when the caller's options are empty, and otherwise
    // trust the region choice — a fragment that happens to sit at a
    // front-matter delimiter has a `---`/`+++` first line, which parses
    // as a thematic break or a paragraph in a full parse; the
    // incremental driver never selects such a region as a reparse
    // boundary (its checkpoint rejects regions whose first line could
    // be a prologue).
    let (arena, root, link_defs, problems, defs) = Parser::new(src).extensions(extensions).run();
    Document {
        arena,
        root,
        link_defs,
        problems,
        defs,
    }
}

/// Splits a front matter prologue off the very beginning of `src`,
/// returning the byte offset one past its last byte. Mirrors the
/// reference algorithm exactly: one leading byte order mark may
/// precede the opener; the opener is the delimiter followed by a line
/// ending; the closer is the first later occurrence of a line ending,
/// the delimiter, and then a line ending or the end of input —
/// searched as three patterns in the reference's priority order, so a
/// carriage-return closer anywhere in the body wins over an earlier
/// line-feed closer; the closer's own line ending and one following
/// blank line belong to the prologue. An empty body does not close
/// (the closer search needs a preceding line ending inside the body).
fn split_off_front_matter(src: &str, delimiter: &str) -> Option<usize> {
    let mut at = 0usize;
    if src.starts_with('\u{feff}') {
        at = '\u{feff}'.len_utf8();
    }
    if !src[at..].starts_with(delimiter) {
        return None;
    }
    at += delimiter.len();
    if src[at..].starts_with('\n') {
        at += 1;
    } else if src[at..].starts_with("\r\n") {
        at += 2;
    } else {
        return None;
    }
    let body = &src[at..];
    let closer = format!("\n{delimiter}");
    let hit = body
        .find(&format!("{closer}\r\n"))
        .or_else(|| body.find(&format!("{closer}\n")))
        .or_else(|| body.find(&closer))?;
    at += hit + 1 + delimiter.len();
    if at == src.len() {
        return Some(at);
    }
    if src[at..].starts_with('\n') {
        at += 1;
    } else if src[at..].starts_with("\r\n") {
        at += 2;
    } else {
        return None;
    }
    if src[at..].starts_with('\n') {
        at += 1;
    } else if src[at..].starts_with("\r\n") {
        at += 2;
    }
    Some(at)
}

/// Prepares a converted paragraph's prefix as paragraph text: `\|`
/// unescapes to `|` and the whole prefix is trimmed, as the reference
/// implementations prepare a table's pre-header text.
fn unescape_pipes_trim(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'|') {
            out.push('|');
            i += 2;
        } else {
            let ch = raw[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out.trim().to_owned()
}

/// Parses the block structure of `src`.
struct Parser<'a, 'ext> {
    src: &'a str,
    arena: Arena<'a>,
    open: Vec<Frame>,
    line_number: usize,
    link_defs: HashMap<String, LinkDef<'a>>,
    /// Every definition scanned, in document order with duplicates;
    /// `link_defs` keeps only the first of each label.
    defs: Vec<SpannedDef<'a>>,
    /// Whether each node's last processed line was blank, indexed by node.
    last_line_blank: Vec<bool>,
    /// Whether the text being added is a lazy paragraph continuation.
    adding_lazy: bool,
    /// Whether the line being processed is a plain slice of the source.
    /// A line containing NUL bytes is rebuilt with U+FFFD replacements
    /// and is not, so its content must not be borrowed from the source.
    line_is_src: bool,
    /// Per-leaf-block maps from text-buffer offsets back to source
    /// offsets, keyed by the paragraph or heading node. The inline phase
    /// consumes them to place inline nodes' spans in the source.
    /// Per-leaf-block buffer maps, indexed by the arena slot the block
    /// allocated. The slot index is dense, so a side table beats hashing
    /// on the per-line record path; only leaf blocks hold a map.
    text_maps: Vec<Option<TextMap>>,
    /// The registered extensions, or `None` on the plain path; passed
    /// through to the inline phase's hook sites.
    extensions: Option<&'ext [std::sync::Arc<dyn crate::extension::Extension>]>,
    /// Paragraphs that declined an extension block start; the hook is
    /// not called with them again.
    visited_paragraphs: Vec<NodeId>,
    /// The problems the parse noticed, drained from the inline phase
    /// and handed to the document.
    problems: Vec<crate::problems::Problem>,
    /// Whether a block-start hook consumed the current line, which
    /// makes it not blank however empty the rest looks.
    line_consumed: bool,
    /// Extension blocks opened so far on the current line. A hook that
    /// opens a block without advancing the cursor would be offered the
    /// same text again and again; the cap ends that loop and keeps any
    /// hostile extension from spinning the parse.
    extension_opens: usize,
}

/// How many extension blocks one line may open. Legitimate chains are
/// short (`: - item` opens two); anything past the cap cannot advance
/// the line and is an extension fighting itself.
const EXTENSION_OPENS_PER_LINE: usize = 16;

/// The state behind a block-start hook. Its fields stay private to
/// the parser; hooks reach the state through the context's methods.
pub(crate) struct StartInner<'p, 'a, 'l, 'ext> {
    parser: &'p mut Parser<'a, 'ext>,
    pub(crate) cursor: &'p mut Cursor<'l>,
    line_offset: usize,
    line_stop: usize,
    line: &'l [u8],
    line_blank: bool,
    indented: bool,
    indent: usize,
    extension: usize,
    fired: bool,
    fired_node: Option<NodeId>,
}

impl<'p, 'a, 'l, 'ext> StartInner<'p, 'a, 'l, 'ext> {
    /// The index of the extension whose hook is running.
    pub(crate) fn extension_index(&self) -> usize {
        self.extension
    }

    /// The current line's content from its first non-space byte,
    /// without the line ending.
    pub(crate) fn line(&self) -> &'l [u8] {
        self.line
    }

    /// Whether the line is blank: only spaces and tabs remain.
    pub(crate) fn line_blank(&self) -> bool {
        self.line_blank
    }

    /// Whether the line's first non-space byte sits four or more
    /// virtual columns past the container prefixes.
    pub(crate) fn indented(&self) -> bool {
        self.indented
    }

    /// How many virtual columns the line's first non-space byte sits
    /// past the container prefixes.
    pub(crate) fn indent(&self) -> usize {
        self.indent
    }

    /// The arena, for hooks that build their block's children.
    pub(crate) fn arena_mut(&mut self) -> &mut crate::arena::Arena<'a> {
        &mut self.parser.arena
    }

    /// The deepest open frame's node when it is an extension block.
    pub(crate) fn open_node(&self) -> Option<NodeId> {
        let frame = self.parser.open.last().unwrap();
        match &frame.kind {
            OpenKind::Extension { .. } => Some(frame.id),
            _ => None,
        }
    }

    /// The open extension block's state, when the deepest frame is one.
    pub(crate) fn frame_state(&mut self) -> Option<&mut dyn std::any::Any> {
        match &mut self.parser.open.last_mut().unwrap().kind {
            OpenKind::Extension { state, .. } => state.as_deref_mut().map(|s| s as _),
            _ => None,
        }
    }

    /// Consumes the rest of the line as content of the open block
    /// without opening a new one, for hooks that append to the open
    /// block directly.
    pub(crate) fn absorb_line(&mut self) {
        self.parser.line_consumed = true;
        self.cursor.bump_all();
        self.fired = true;
    }

    /// Consumes `bytes` more bytes of the line at the current content
    /// position without marking the line consumed or stopping later
    /// hooks: the remaining line still opens a paragraph in the open
    /// container as usual, with the reference's exact walk-break
    /// behavior for hooks that peel a prefix off a kept line.
    pub(crate) fn consume(&mut self, bytes: usize) {
        self.cursor.bump_n(bytes);
    }

    /// The full current line, from its first byte, without its line
    /// ending. The cursor holds the line slice for its whole life, so
    /// container prefixes and leading whitespace are still in it.
    pub(crate) fn raw_line(&self) -> &'l [u8] {
        self.cursor.line()
    }

    /// The deepest open frame's node when it is a plain container.
    pub(crate) fn container_node(&self) -> Option<NodeId> {
        let frame = self.parser.open.last().unwrap();
        match frame.kind {
            OpenKind::Paragraph { .. }
            | OpenKind::Extension { .. }
            | OpenKind::Heading
            | OpenKind::CodeBlock { .. }
            | OpenKind::HtmlBlock { .. } => None,
            _ => Some(frame.id),
        }
    }

    /// What the new block would open in.
    pub(crate) fn container(&self) -> crate::extension::StartContainer {
        match &self.parser.open.last().unwrap().kind {
            OpenKind::Paragraph { .. } => crate::extension::StartContainer::Paragraph,
            OpenKind::Extension { index, .. } => {
                crate::extension::StartContainer::Extension(*index)
            }
            _ => crate::extension::StartContainer::Container,
        }
    }

    /// The open paragraph's accumulated text, if any.
    pub(crate) fn paragraph_text(&self) -> Option<&str> {
        match &self.parser.open.last().unwrap().kind {
            OpenKind::Paragraph { text } => Some(text),
            _ => None,
        }
    }

    /// Marks the open paragraph as having declined this hook.
    pub(crate) fn visit_paragraph(&mut self) {
        let id = self.parser.open.last().unwrap().id;
        if matches!(
            self.parser.open.last().unwrap().kind,
            OpenKind::Paragraph { .. }
        ) {
            self.parser.visited_paragraphs.push(id);
        }
    }

    /// Converts the open paragraph into an extension block of the
    /// running extension. The text before `pre_len` becomes a
    /// paragraph in front of the new block, with `\|` unescaped
    /// and the prefix trimmed.
    pub(crate) fn convert_paragraph(
        &mut self,
        kind: String,
        pre_len: usize,
        state: Box<dyn std::any::Any + Send>,
    ) -> NodeId {
        let frame = self.parser.open.pop().expect("a paragraph frame is open");
        let OpenKind::Paragraph { text } = frame.kind else {
            unreachable!("convert_paragraph runs on a paragraph frame");
        };
        self.parser.arena.set_kind(
            frame.id,
            Node::Extension(crate::extension::ExtNode {
                kind,
                attrs: Vec::new(),
            }),
        );
        if pre_len > 0 && pre_len <= text.len() {
            let pre = unescape_pipes_trim(&text[..pre_len]);
            if !pre.is_empty() {
                let parent = self.parser.open.last().unwrap().id;
                let pre_id = self.parser.arena.alloc(
                    Node::Paragraph,
                    Span::new(frame.start_offset, frame.start_offset + pre_len),
                );
                let text_id = self
                    .parser
                    .arena
                    .alloc(Node::Text(Cow::Owned(pre)), Span::empty(frame.start_offset));
                self.parser.arena.push_child(pre_id, text_id);
                let pre_index = pre_id.index();
                if self.parser.last_line_blank.len() <= pre_index {
                    self.parser.last_line_blank.resize(pre_index + 1, false);
                }
                // The former paragraph node was the parent's last
                // child; put the new paragraph in front of it.
                let last = self.parser.arena.pop_child(parent);
                debug_assert_eq!(last, Some(frame.id));
                self.parser.arena.push_child(parent, pre_id);
                self.parser.arena.push_child(parent, frame.id);
            }
        }
        let index = frame.id.index();
        if self.parser.last_line_blank.len() <= index {
            self.parser.last_line_blank.resize(index + 1, false);
        }
        self.parser.last_line_blank[index] = false;
        self.parser.open.push(Frame {
            id: frame.id,
            kind: OpenKind::Extension {
                index: self.extension,
                state: Some(state),
            },
            start_line: self.parser.line_number,
            start_offset: frame.start_offset,
            end_offset: self.line_stop,
        });
        self.parser.line_consumed = true;
        self.cursor.bump_all();
        self.fired = true;
        self.fired_node = Some(frame.id);
        frame.id
    }

    /// Opens an extension block of the running extension inside
    /// the container the hook was called with.
    pub(crate) fn open_extension(
        &mut self,
        kind: String,
        state: Box<dyn std::any::Any + Send>,
    ) -> NodeId {
        self.parser.interrupt_paragraph();
        self.parser.close_until_can_contain();
        let parent = self.parser.open.last().unwrap().id;
        let id = self.parser.arena.alloc(
            Node::Extension(crate::extension::ExtNode {
                kind,
                attrs: Vec::new(),
            }),
            Span::empty(self.line_offset),
        );
        self.parser.arena.push_child(parent, id);
        self.parser.last_line_blank.push(false);
        self.parser.open.push(Frame {
            id,
            kind: OpenKind::Extension {
                index: self.extension,
                state: Some(state),
            },
            start_line: self.parser.line_number,
            start_offset: self.line_offset,
            end_offset: self.line_stop,
        });
        self.parser.line_consumed = true;
        self.cursor.bump_all();
        self.fired = true;
        self.fired_node = Some(id);
        id
    }

    /// Opens an extension block like
    /// [`open_extension`](Self::open_extension), but leaves the line's
    /// remaining bytes at the cursor for the parser to add to the new
    /// block — the way a list item's marker is consumed and the item's
    /// first line stays. The hook calls
    /// [`seek_content`](Self::seek_content) to move the cursor past
    /// its marker first.
    pub(crate) fn open_extension_keep_rest(
        &mut self,
        kind: String,
        state: Box<dyn std::any::Any + Send>,
    ) -> NodeId {
        self.parser.interrupt_paragraph();
        self.parser.close_until_can_contain();
        let parent = self.parser.open.last().unwrap().id;
        let id = self.parser.arena.alloc(
            Node::Extension(crate::extension::ExtNode {
                kind,
                attrs: Vec::new(),
            }),
            Span::empty(self.line_offset),
        );
        self.parser.arena.push_child(parent, id);
        self.parser.last_line_blank.push(false);
        self.parser.open.push(Frame {
            id,
            kind: OpenKind::Extension {
                index: self.extension,
                state: Some(state),
            },
            start_line: self.parser.line_number,
            start_offset: self.line_offset,
            end_offset: self.line_stop,
        });
        self.fired = true;
        self.fired_node = Some(id);
        id
    }
}

impl<'a, 'ext> Parser<'a, 'ext> {
    fn new(src: &'a str) -> Self {
        let mut arena = Arena::new();
        // Bulk-reserve nodes by input size so the arena vector does not
        // grow (and memmove its contents) repeatedly mid-parse. The
        // density is measured against the benchmark corpus (one node per
        // ~38.5 bytes of prose); the divisor only has to under-count,
        // extra capacity is wasted memory but not time.
        arena.reserve(src.len() / 32 + 64);
        let root = arena.alloc(Node::Document, Span::new(0, src.len()));
        Parser {
            src,
            arena,
            open: vec![Frame {
                id: root,
                kind: OpenKind::Document,
                start_line: 0,
                start_offset: 0,
                end_offset: 0,
            }],
            line_number: 0,
            link_defs: HashMap::new(),
            defs: Vec::new(),
            last_line_blank: vec![false],
            adding_lazy: false,
            line_is_src: true,
            text_maps: Vec::new(),
            extensions: None,
            visited_paragraphs: Vec::new(),
            problems: Vec::new(),
            line_consumed: false,
            extension_opens: 0,
        }
    }

    /// Sets the registered extensions for the parse; `None` (the
    /// default from [`Parser::new`]) keeps the plain path.
    fn extensions(
        mut self,
        extensions: Option<&'ext [std::sync::Arc<dyn crate::extension::Extension>]>,
    ) -> Self {
        self.extensions = extensions;
        self
    }

    /// Checks the document's beginning for a front matter prologue and
    /// records it as the root's first child when one claims it. The
    /// return value is the byte offset the line loop starts at, zero
    /// when nothing claimed a prologue.
    fn claim_front_matter(&mut self) -> usize {
        let Some(extensions) = self.extensions else {
            return 0;
        };
        for extension in extensions {
            let Some(delimiter) = extension.front_matter_delimiter() else {
                continue;
            };
            if let Some(end) = split_off_front_matter(self.src, delimiter) {
                let raw = &self.src[..end];
                // The byte order mark, when one preceded the opener,
                // is not part of the prologue's own text.
                let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
                let node = self.arena.alloc(
                    crate::node::Node::Extension(crate::extension::ExtNode {
                        kind: "front_matter".to_owned(),
                        attrs: vec![(
                            "source".to_owned(),
                            crate::extension::AttrValue::Str(raw.to_owned()),
                        )],
                    }),
                    crate::span::Span::new(0, end),
                );
                self.arena.insert_child(self.open[0].id, 0, node);
                return end;
            }
        }
        0
    }

    /// Runs the whole block parse and returns the arena, root, defs and
    /// the problems the parse noticed.
    fn run(
        mut self,
    ) -> (
        Arena<'a>,
        NodeId,
        HashMap<String, LinkDef<'a>>,
        Vec<crate::problems::Problem>,
        Vec<SpannedDef<'a>>,
    ) {
        let bytes = self.src.as_bytes();
        // A registered front matter delimiter claims a prologue before
        // any line is parsed; the rest parses as if the prologue were
        // not part of the source, while spans keep their real offsets.
        let mut line_start = self.claim_front_matter();
        while line_start < bytes.len() {
            // Lines end at `\n`, at `\r\n`, or at a bare `\r`, as in the
            // reference implementation; the carriage return itself is
            // never part of the line's content.
            let mut line_end = line_start;
            while line_end < bytes.len() && bytes[line_end] != b'\n' && bytes[line_end] != b'\r' {
                line_end += 1;
            }
            let has_newline = line_end < bytes.len();
            let mut next_start = line_end;
            if has_newline {
                next_start += 1;
                if bytes[line_end] == b'\r' && bytes.get(next_start) == Some(&b'\n') {
                    next_start += 1;
                }
            }
            let line = &bytes[line_start..line_end];
            self.line_number += 1;
            let line_stop = line_start + line.len();
            // NUL bytes become U+FFFD replacement characters before any
            // processing, as the reference implementation does when it
            // reads a line. The replaced line is no longer a slice of the
            // source, so the parser must not borrow source bytes for it.
            let has_nul = line.contains(&0u8);
            let replaced;
            let line: &[u8] = if has_nul {
                let mut out = Vec::with_capacity(line.len() + 16);
                let mut run_start = 0usize;
                for (i, &b) in line.iter().enumerate() {
                    if b == 0 {
                        out.extend_from_slice(&line[run_start..i]);
                        out.extend_from_slice(&[0xEF, 0xBF, 0xBD]);
                        run_start = i + 1;
                    }
                }
                out.extend_from_slice(&line[run_start..]);
                replaced = out;
                &replaced
            } else {
                line
            };
            self.line_is_src = !has_nul;
            self.process_line(line, line_start, line_stop, next_start);
            line_start = next_start;
        }
        while self.open.len() > 1 {
            self.close_last(None);
        }
        let root = self.open[0].id;
        self.parse_all_inlines(root);
        (
            self.arena,
            root,
            self.link_defs,
            self.problems,
            std::mem::take(&mut self.defs),
        )
    }

    /// Processes one line against the open block stack. `next_line_start`
    /// is the offset just past the line's ending, for anchors that map
    /// the buffer's joining newlines back to the line's ending bytes.
    fn process_line(
        &mut self,
        line: &[u8],
        line_offset: usize,
        line_stop: usize,
        next_line_start: usize,
    ) {
        let mut cursor = Cursor::new(line);
        self.extension_opens = 0;

        // Phase A: match the open containers. Nothing closes yet.
        match self.walk(&mut cursor, line_stop) {
            WalkResult::FenceClosed => return,
            WalkResult::ClosedAt(depth) => {
                // A closing fence consumed the line; the block closes and
                // nothing else happens to the line. The hook's block ends
                // at this line's char end, its own last content.
                self.open[depth].end_offset = line_stop;
                self.close_from(depth);
                return;
            }
            WalkResult::FailedAt(depth) => {
                // A lazy continuation: the deepest open block is a paragraph,
                // the line is not blank, and no block start interrupts it.
                // The failed containers stay open.
                if self.deepest_is_paragraph()
                    && !cursor.blank_from_here()
                    && !self.block_start_matches(&cursor)
                    && !self.extension_block_start_looks(
                        &cursor,
                        depth.checked_sub(1).map(|d| self.open[d].id),
                    )
                {
                    // The line's leading whitespace stays in the buffer;
                    // the inline phase strips it after each newline, and a
                    // consumed reference definition must not hide it.
                    let id = self.open.last().unwrap().id;
                    self.append_paragraph_rest(
                        id,
                        &cursor,
                        line_offset,
                        line_stop,
                        next_line_start,
                    );
                    self.set_frame_end(line_stop);
                    self.update_last_line_blank(self.open[depth - 1].id, Some(depth - 1), false);
                    return;
                }
                self.close_from(depth);
            }
            WalkResult::Matched => {}
        }

        // Phase B: open the new blocks the line starts.
        let opened = self.open_new_blocks(&mut cursor, line_offset, line_stop);

        // Phase C: add the remaining line to the open leaf block.
        let added = self.add_text(&mut cursor, line_offset, line_stop, next_line_start);

        // The node the last-line-blank flag lands on: the block that received
        // the line's text, or the last block this line opened.
        let target = match added {
            Some(id) => id,
            None => opened.unwrap_or_else(|| self.open.last().unwrap().id),
        };
        // A line a hook consumed leaves the parse at the line's end, and the
        // reference's flag pass reads that residual position as blank, so
        // extension blocks that take whole lines count as ending blank.
        let blank = cursor.blank_from_here();
        let on_stack = self.open.iter().position(|frame| frame.id == target);
        self.update_last_line_blank(target, on_stack, blank);
    }

    /// Matches the open containers against the line, consuming their prefix
    /// but closing nothing. Returns the depth of the first frame that fails
    /// to match, or [`WalkResult::FenceClosed`] when a fenced block's closing
    /// fence ends the line's processing.
    fn walk(&mut self, cursor: &mut Cursor<'_>, line_stop: usize) -> WalkResult {
        for depth in 0..self.open.len() {
            // Extension frames need mutable access to their state, so
            // they are matched before the shared-borrow match below.
            if let OpenKind::Extension { index, state } = &mut self.open[depth].kind {
                let (pos, _) = cursor.find_nonspace();
                let line = &cursor.line()[pos..];
                let mut inner = state.take();
                let state_ref: Option<&mut dyn std::any::Any> =
                    inner.as_deref_mut().map(|s| s as _);
                let mut ctx = crate::extension::BlockContinueCtx {
                    line,
                    state: state_ref,
                    cursor,
                };
                let outcome = self.extensions.unwrap()[*index].try_continue(&mut ctx);
                *state = inner;
                match outcome {
                    crate::extension::ContinueOutcome::Continue => {
                        // The hook consumed its prefix, and any extra
                        // columns it declared, through the context's
                        // cursor. Blank lines reach the hook here too:
                        // unlike the built-in containers, a hook decides
                        // on the whole line.
                    }
                    crate::extension::ContinueOutcome::Close => {
                        return WalkResult::FailedAt(depth);
                    }
                    crate::extension::ContinueOutcome::CloseAndConsume => {
                        // A closing fence: the line is the block's own,
                        // so it is consumed whole and starts nothing.
                        cursor.bump_all();
                        return WalkResult::ClosedAt(depth);
                    }
                }
                continue;
            }
            // Whether this frame's match consumed part of the line as its
            // own: if it did, the line extends the frame's span.
            let mut extends = false;
            let matched = match &self.open[depth].kind {
                OpenKind::Document | OpenKind::List(_) | OpenKind::Heading => true,
                OpenKind::BlockQuote => {
                    let (pos, col) = cursor.find_nonspace();
                    if col - cursor.col() > 3 {
                        false
                    } else if cursor.line().get(pos) == Some(&b'>') {
                        cursor.set_pos_col(pos, col);
                        cursor.bump();
                        if matches!(cursor.peek(), Some(b' ') | Some(b'\t')) {
                            cursor.advance_cols(1);
                        }
                        extends = true;
                        true
                    } else {
                        false
                    }
                }
                OpenKind::Item { content_offset } => {
                    if cursor.blank_from_here() {
                        let id = self.open[depth].id;
                        if self.arena.children(id).is_empty() {
                            // A list item that began blank cannot continue
                            // across a blank line.
                            false
                        } else {
                            cursor.skip_to_content();
                            true
                        }
                    } else {
                        let (_, col) = cursor.find_nonspace();
                        if col >= cursor.col() + *content_offset {
                            cursor.advance_cols(*content_offset);
                            extends = true;
                            true
                        } else {
                            false
                        }
                    }
                }
                OpenKind::Paragraph { .. } => !cursor.blank_from_here(),
                OpenKind::CodeBlock { fenced: false, .. } => {
                    let (_, col) = cursor.find_nonspace();
                    if col - cursor.col() >= 4 {
                        cursor.advance_cols(4);
                        extends = true;
                        true
                    } else if cursor.blank_from_here() {
                        cursor.skip_to_content();
                        true
                    } else {
                        false
                    }
                }
                OpenKind::CodeBlock {
                    fenced: true,
                    fence_char,
                    fence_len,
                    fence_offset,
                    ..
                } => {
                    let (ch, len, offset) = (*fence_char, *fence_len, *fence_offset);
                    if leaf::fence_close(cursor, ch, len) {
                        self.close_last(Some(line_stop));
                        return WalkResult::FenceClosed;
                    }
                    cursor.skip_up_to_cols(offset);
                    true
                }
                OpenKind::HtmlBlock { kind, .. } => {
                    // Kinds 6 and 7 end at a blank line; kinds 1 to 5 accept
                    // blank lines as content.
                    if *kind >= 6 {
                        !cursor.blank_from_here()
                    } else {
                        true
                    }
                }
                // Extension frames were handled above, with mutable
                // access to their state.
                OpenKind::Extension { .. } => unreachable!("handled above"),
            };
            if !matched {
                return WalkResult::FailedAt(depth);
            }
            if extends {
                self.open[depth].end_offset = line_stop;
            }
        }
        WalkResult::Matched
    }

    /// Whether the deepest open block is a paragraph.
    fn deepest_is_paragraph(&self) -> bool {
        matches!(self.open.last().unwrap().kind, OpenKind::Paragraph { .. })
    }

    /// Whether a registered extension would start a block on this line
    /// against the container the line still matches; the lazy
    /// continuation check consults it after the built-in starts. The
    /// container is the parent of the first block that failed to
    /// continue — the same container the block-start phase would see
    /// once the failed blocks close — so the hook's answer matches
    /// what its `try_start` will do.
    fn extension_block_start_looks(&self, cursor: &Cursor<'_>, container: Option<NodeId>) -> bool {
        let Some(extensions) = self.extensions else {
            return false;
        };
        let Some(container) = container else {
            return false;
        };
        let (pos, _) = cursor.find_nonspace();
        if pos >= cursor.line().len() {
            return false;
        }
        let line = &cursor.line()[pos..];
        let (_, nonspace_col) = cursor.find_nonspace();
        let ctx = crate::extension::LookCtx {
            line,
            indented: nonspace_col - cursor.col() >= 4,
            column: nonspace_col,
            arena: &self.arena,
            container,
        };
        extensions
            .iter()
            .any(|extension| extension.block_start_looks(&ctx))
    }

    /// Whether the deepest open frame is an extension block that holds
    /// other blocks; such a frame is a container for the parser, like a
    /// block quote or a list item.
    fn deepest_extension_contains_blocks(&self) -> bool {
        let Some(extensions) = self.extensions else {
            return false;
        };
        match &self.open.last().unwrap().kind {
            OpenKind::Extension { index, .. } => extensions[*index].contains_blocks(),
            _ => false,
        }
    }

    /// Whether the deepest open block is a leaf that takes no new children.
    fn deepest_is_code_or_html(&self) -> bool {
        matches!(
            self.open.last().unwrap().kind,
            OpenKind::CodeBlock { .. } | OpenKind::HtmlBlock { .. }
        )
    }

    /// Whether any new block start matches the line at the cursor; used for
    /// the lazy continuation check. Setext headings are excluded because they
    /// need an open paragraph in the matched chain, indented code blocks are
    /// excluded because they cannot break a lazy continuation, and HTML kind
    /// 7 is excluded because it cannot interrupt a paragraph.
    fn block_start_matches(&self, cursor: &Cursor<'_>) -> bool {
        let (pos, col) = cursor.find_nonspace();
        if pos >= cursor.line().len() {
            return false;
        }
        if col - cursor.col() >= 4 {
            return false;
        }
        let mut at = cursor.clone();
        at.set_pos_col(pos, col);
        at.peek() == Some(b'>')
            || leaf::atx_heading(&at).is_some()
            || leaf::fence_start(&at).is_some()
            || leaf::html_block_start(&at, false).is_some()
            || leaf::thematic_break(&at)
            || self.item_start(&at, false).is_some()
    }

    /// Opens every new block the line starts, consuming their prefixes.
    /// Returns the id of the last block opened, if any.
    fn open_new_blocks(
        &mut self,
        cursor: &mut Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
    ) -> Option<NodeId> {
        let mut opened: Option<NodeId> = None;
        // A line can continue an open paragraph lazily until something new
        // opens.
        let mut maybe_lazy = self.deepest_is_paragraph();
        loop {
            if self.deepest_is_code_or_html() {
                break;
            }
            // The scanners work on a view positioned at the first non-space
            // byte; the real cursor only moves inside the branch that fires,
            // the way the reference implementation advances its offset.
            let (pos, col) = cursor.find_nonspace();
            if pos >= cursor.line().len() {
                break;
            }
            let indent = col - cursor.col();
            let scan_is_para = self.deepest_is_paragraph();
            let mut at = cursor.clone();
            at.set_pos_col(pos, col);

            // Block quote. Four or more spaces of indentation form an
            // indented code block instead.
            if indent < 4 && at.peek() == Some(b'>') {
                self.interrupt_paragraph();
                self.close_until_can_contain();
                let id = self.open_block_quote(line_offset + pos, line_stop);
                cursor.set_pos_col(pos, col);
                cursor.bump();
                if matches!(cursor.peek(), Some(b' ') | Some(b'\t')) {
                    cursor.advance_cols(1);
                }
                opened = Some(id);
                maybe_lazy = false;
                continue;
            }

            // ATX heading.
            if indent < 4 {
                if let Some((level, text, content_pos)) = leaf::atx_heading(&at) {
                    self.interrupt_paragraph();
                    self.close_until_can_contain();
                    let id = self.open_atx_heading(
                        level,
                        text,
                        content_pos,
                        line_offset,
                        line_offset + pos,
                        line_stop,
                    );
                    self.raise_ancestor_ends(line_stop);
                    cursor.bump_all();
                    return Some(id);
                }
            }

            // Fenced code. The info string stays on the line; it becomes the
            // literal's first line and is extracted when the block closes.
            if indent < 4 {
                if let Some(fence) = leaf::fence_start(&at) {
                    self.interrupt_paragraph();
                    self.close_until_can_contain();
                    let id = self.open_fenced_code(&fence, indent, line_offset + pos);
                    cursor.set_pos_col(pos, col);
                    cursor.bump_n(fence.len);
                    return Some(id);
                }
            }

            // HTML block. Kind 7 needs an open container chain that is not a
            // paragraph and not a possible lazy continuation. The tag is
            // part of the text, so the cursor stays where it is.
            if indent < 4 {
                if let Some(start) = leaf::html_block_start(&at, !maybe_lazy && !scan_is_para) {
                    self.interrupt_paragraph();
                    self.close_until_can_contain();
                    let id = self.open_html_block(start.kind, line_offset + cursor.pos());
                    return Some(id);
                }
            }

            // Setext heading, which needs the open paragraph of the matched
            // chain. An underline indented four or more columns does not
            // interrupt.
            if scan_is_para && indent < 4 {
                if let Some(level) = leaf::setext_underline(&at) {
                    match self.setext_heading_or_defs(level, line_stop) {
                        SetextOutcome::Converted(id) => {
                            cursor.bump_all();
                            return Some(id);
                        }
                        SetextOutcome::Dissolved => break,
                        SetextOutcome::NotSetext => {}
                    }
                }
            }

            // Thematic break. It cannot appear where a paragraph failed to
            // match a container prefix, which cannot happen here: a scanned
            // paragraph always matched.
            if indent < 4 && leaf::thematic_break(&at) {
                self.interrupt_paragraph();
                self.close_until_can_contain();
                let id = self.open_thematic_break(line_offset + pos, line_stop);
                self.raise_ancestor_ends(line_stop);
                cursor.bump_all();
                return Some(id);
            }

            // List item.
            if indent < 4 {
                if let Some(marker) = self.item_start(&at, scan_is_para) {
                    cursor.set_pos_col(pos, col);
                    self.open_list_item(&marker, cursor, indent, line_offset);
                    opened = Some(self.open.last().unwrap().id);
                    maybe_lazy = false;
                    continue;
                }
            }

            // Indented code: four columns come off the line from where the
            // container prefixes left it, not from the first non-space byte.
            if !maybe_lazy && !cursor.blank_from_here() && indent >= 4 {
                self.close_until_can_contain();
                let id = self.open_indented_code(line_offset + cursor.pos());
                cursor.advance_cols(4);
                return Some(id);
            }

            // Extension block starts: each registered hook in registration
            // order, until one opens a block or converts the paragraph.
            // The loop then continues with the rest of the line, the way
            // the reference implementation's loop does: a marker that
            // consumed only its own prefix leaves the rest free to open
            // further blocks — `: - item` opens the list inside the
            // definition.
            if self.extensions.is_some() {
                if let Some(id) = self.extension_starts(cursor, line_offset, line_stop, indent) {
                    opened = Some(id);
                    maybe_lazy = false;
                    continue;
                }
            }

            break;
        }
        opened
    }

    /// Runs the registered extensions' block-start hooks for the
    /// current line, in registration order, until one opens a block.
    /// Returns the new block's node then.
    fn extension_starts(
        &mut self,
        cursor: &mut Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
        indent: usize,
    ) -> Option<NodeId> {
        // The per-line cap: past it, this line opens no further
        // extension blocks, so a hook that re-matches text it did not
        // consume cannot spin the parse.
        if self.extension_opens >= EXTENSION_OPENS_PER_LINE {
            return None;
        }
        let (pos, _) = cursor.find_nonspace();
        if pos >= cursor.line().len() {
            return None;
        }
        let line_blank = cursor.blank_from_here();
        let line = &cursor.line()[pos..];
        let extensions = self.extensions.unwrap();
        // A paragraph that already declined a hook is not offered to
        // hooks again.
        let visited = matches!(self.open.last().unwrap().kind, OpenKind::Paragraph { .. })
            && self
                .visited_paragraphs
                .contains(&self.open.last().unwrap().id);
        for (index, extension) in extensions.iter().enumerate() {
            if visited {
                break;
            }
            let inner = StartInner {
                parser: self,
                cursor,
                line_offset,
                line_stop,
                line,
                line_blank,
                indented: indent >= 4,
                indent,
                extension: index,
                fired: false,
                fired_node: None,
            };
            let mut ctx = crate::extension::BlockStartCtx { inner };
            extension.try_start(&mut ctx);
            let StartInner {
                fired, fired_node, ..
            } = ctx.inner;
            if fired {
                self.extension_opens += 1;
                return fired_node;
            }
        }
        None
    }

    /// Scans a list item start at the cursor, honoring the rules for
    /// interrupting a paragraph. The cursor is not advanced.
    fn item_start(&self, cursor: &Cursor<'_>, para_open: bool) -> Option<leaf::ListMarker> {
        let marker = leaf::list_marker(cursor)?;
        if !leaf::marker_followed_by_space(cursor, marker.width) {
            return None;
        }
        if para_open {
            let mut probe = cursor.clone();
            probe.bump_n(marker.width);
            probe.skip_spaces();
            if probe.blank_from_here() || (marker.ordered && marker.start != 1) {
                return None;
            }
        }
        Some(marker)
    }

    /// Closes an open paragraph (extracting link definitions), so a new
    /// block can interrupt it. The paragraph keeps its own end: the char
    /// end of its last content line, not the interrupting line's.
    fn interrupt_paragraph(&mut self) {
        if self.deepest_is_paragraph() {
            self.close_last(None);
        }
    }

    /// Closes open frames until the deepest frame can hold a new child,
    /// mirroring the reference algorithm's add-child walk: only the
    /// document, block quotes and list items accept arbitrary blocks, a
    /// list accepts items only, and leaf frames accept nothing. Closed
    /// frames keep their own ends (see `close_last`).
    fn close_until_can_contain(&mut self) {
        while !matches!(
            self.open.last().unwrap().kind,
            OpenKind::Document | OpenKind::BlockQuote | OpenKind::Item { .. }
        ) && !self.deepest_extension_contains_blocks()
        {
            self.close_last(None);
        }
    }

    /// Closes the paragraph and any block in the way of a new list,
    /// mirroring the reference's add-child walk: a list may live under
    /// a document, block quote or item, so everything else closes.
    /// Closed frames keep their own ends (see `close_last`).
    fn make_room_for_list(&mut self) {
        while !matches!(
            self.open.last().unwrap().kind,
            OpenKind::Document | OpenKind::BlockQuote | OpenKind::Item { .. }
        ) && !self.deepest_extension_contains_blocks()
        {
            self.close_last(None);
        }
    }

    /// Raises every open frame's recorded end to `end`: an atomic block
    /// (a thematic break or an ATX heading) takes the line without a
    /// frame of its own, so the containers it landed in keep the line as
    /// their last consumed content.
    fn raise_ancestor_ends(&mut self, end: usize) {
        for frame in self.open.iter_mut() {
            frame.end_offset = frame.end_offset.max(end);
        }
    }

    /// Opens a list item at the cursor, reusing or opening the list it
    /// belongs to, and consumes the marker plus its padding.
    ///
    /// `indent` is the marker's column past the parent container's content
    /// column; the item's content offset is relative to the same column.
    fn open_list_item(
        &mut self,
        marker: &leaf::ListMarker,
        cursor: &mut Cursor<'_>,
        indent: usize,
        line_offset: usize,
    ) {
        // The item's span starts at its marker, the item's own first
        // byte; capture it before the cursor moves into the padding.
        let at_marker = line_offset + cursor.pos();
        // The item's padding: spaces and tabs after the marker, up to five
        // columns; more than four, none, or nothing but blankness rewind to
        // one column past the marker.
        cursor.bump_n(marker.width);
        let after_marker = cursor.clone();
        let save_col = cursor.col();
        let mut padding = 0usize;
        while cursor.col() - save_col <= 5 && matches!(cursor.peek(), Some(b' ') | Some(b'\t')) {
            cursor.advance_cols(1);
            padding += 1;
        }
        // The padding the line actually carried, for the item's span: the
        // rewind below changes the content offset, not the span.
        let mut padding_span = padding;
        if !(1..5).contains(&padding) || cursor.blank_from_here() {
            // The padding the line actually carried, for the item's span.
            let consumed = cursor.pos() - after_marker.pos();
            *cursor = after_marker;
            if padding > 0 {
                cursor.advance_cols(1);
            }
            // The reference algorithm rewinds to one column past the marker
            // and counts a single column of padding.
            padding = 1;
            padding_span = consumed;
        }
        let same_type = match &self.open.last().unwrap().kind {
            OpenKind::List(list) => list.ordered == marker.ordered && list.marker == marker.char,
            _ => false,
        };
        if !same_type {
            self.make_room_for_list();
            let parent = self.open.last().unwrap().id;
            let list_id = self.arena.alloc(
                Node::List(ListData {
                    ordered: marker.ordered,
                    marker: marker.char,
                    start: marker.start,
                    tight: true,
                }),
                Span::empty(line_offset),
            );
            self.arena.push_child(parent, list_id);
            self.last_line_blank.push(false);
            self.open.push(Frame {
                id: list_id,
                kind: OpenKind::List(ListData {
                    ordered: marker.ordered,
                    marker: marker.char,
                    start: marker.start,
                    tight: true,
                }),
                start_line: self.line_number,
                start_offset: line_offset,
                end_offset: line_offset,
            });
        }
        let list_id = self.open.last().unwrap().id;
        if !same_type {
            // The list's span starts at its first item's marker, the
            // list's own first byte.
            self.arena.set_span(list_id, Span::empty(at_marker));
            if let Some(Frame { start_offset, .. }) = self.open.last_mut() {
                *start_offset = at_marker;
            }
        }
        let id = self
            .arena
            .alloc(Node::Item(ItemData { padding: 0 }), Span::empty(at_marker));
        self.arena.push_child(list_id, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::Item {
                content_offset: indent + marker.width + padding,
            },
            start_line: self.line_number,
            start_offset: at_marker,
            // An empty item's span covers its marker and the padding the
            // line actually carried (before the rewind), so `-\n` spans
            // its marker alone and `- \n` its marker and its space.
            end_offset: at_marker + marker.width + padding_span,
        });
    }

    /// Opens an indented code block under the deepest container.
    /// `at_indent` is the source offset of the block's own four-space
    /// indent, where the block's span starts.
    fn open_indented_code(&mut self, at_indent: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::CodeBlock(CodeBlockData {
                fenced: false,
                fence_char: 0,
                info: None,
                literal: Cow::Owned(String::new()),
            }),
            Span::empty(at_indent),
        );
        self.arena.push_child(parent, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::CodeBlock {
                fenced: false,
                fence_char: 0,
                fence_len: 0,
                fence_offset: 0,
                literal: String::new(),
            },
            start_line: self.line_number,
            start_offset: at_indent,
            end_offset: at_indent,
        });
        id
    }

    /// Opens a fenced code block. `fence_offset` is the fence's column past
    /// the parent container's content column, which fixes how much
    /// indentation comes off each content line, and `at_fence` is the
    /// fence character's source offset, where the block's span starts.
    fn open_fenced_code(
        &mut self,
        fence: &leaf::FenceStart,
        fence_offset: usize,
        at_fence: usize,
    ) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::CodeBlock(CodeBlockData {
                fenced: true,
                fence_char: fence.ch,
                info: None,
                literal: Cow::Owned(String::new()),
            }),
            Span::empty(at_fence),
        );
        self.arena.push_child(parent, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::CodeBlock {
                fenced: true,
                fence_char: fence.ch,
                fence_len: fence.len,
                fence_offset,
                literal: String::new(),
            },
            start_line: self.line_number,
            start_offset: at_fence,
            end_offset: at_fence,
        });
        id
    }

    /// Opens an HTML block of the given kind. `at_start` is the source
    /// offset of the block's own first byte, where the block's span
    /// starts: the line's leading whitespace belongs to the block,
    /// container prefixes do not.
    fn open_html_block(&mut self, kind: u8, at_start: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::HtmlBlock(HtmlBlockData {
                kind,
                literal: Cow::Owned(String::new()),
            }),
            Span::empty(at_start),
        );
        self.arena.push_child(parent, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::HtmlBlock {
                kind,
                literal: String::new(),
            },
            start_line: self.line_number,
            start_offset: at_start,
            end_offset: at_start,
        });
        id
    }

    /// Opens a thematic break, closed at once. `at_marker` is the first
    /// marker character's source offset, where the break's span starts.
    fn open_thematic_break(&mut self, at_marker: usize, line_stop: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self
            .arena
            .alloc(Node::ThematicBreak, Span::new(at_marker, line_stop));
        self.arena.push_child(parent, id);
        id
    }

    /// Opens a block quote at the `>` the cursor found. `at_marker` is
    /// the marker's source offset, where the block's span starts, and
    /// `line_stop` is the line's char end, where the span ends: the
    /// marker line is the quote's own first line.
    fn open_block_quote(&mut self, at_marker: usize, line_stop: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(Node::BlockQuote, Span::empty(at_marker));
        self.arena.push_child(parent, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::BlockQuote,
            start_line: self.line_number,
            start_offset: at_marker,
            end_offset: line_stop,
        });
        id
    }

    /// Opens an ATX heading with its text, closed at once. `content_pos`
    /// is the text's absolute start in the line, so the text child's
    /// buffer maps linearly onto that source region.
    fn open_atx_heading(
        &mut self,
        level: u8,
        text: String,
        content_pos: usize,
        line_offset: usize,
        at_hash: usize,
        line_stop: usize,
    ) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::Heading(HeadingData {
                level,
                setext: false,
            }),
            // The heading's span starts at its `#`: the marker is the
            // heading's own first byte.
            Span::new(at_hash, line_stop),
        );
        let content_start = line_offset + content_pos;
        let content_end = content_start + text.len();
        let (text_span, anchor) = if self.line_is_src {
            (
                Span::new(content_start, content_end),
                LineAnchor::whole_line(0, content_start, content_end),
            )
        } else {
            // The line was rebuilt with U+FFFD replacements, so the text's
            // source offsets come from a per-byte walk of the original
            // line, in replaced-line coordinates.
            let entries =
                self.replacement_range_map(line_offset, line_stop, content_pos, text.len());
            let start = entries.first().copied().unwrap_or(content_start);
            let end = entries.last().copied().unwrap_or(content_start) + 1;
            (
                Span::new(start, end),
                LineAnchor {
                    buf_start: 0,
                    src_start: start,
                    lin_start: start,
                    tab_span: 0,
                    src_end: end,
                    nul_map: Some(entries),
                },
            )
        };
        self.text_map_slot(id)
            .get_or_insert_with(Default::default)
            .record(text.len(), anchor);
        let text_id = self.arena.alloc(Node::Text(Cow::Owned(text)), text_span);
        self.arena.push_child(id, text_id);
        self.arena.push_child(parent, id);
        id
    }

    /// Replaces each leaf block's text child with parsed inline nodes.
    ///
    /// This runs after the whole block parse, so every reference
    /// definition in the document is visible to every leaf block's inline
    /// content, as in the reference parser. The walk is iterative so
    /// arbitrarily deep nesting cannot overflow the call stack.
    fn parse_all_inlines(&mut self, root: NodeId) {
        let mut session = crate::inline::InlineSession::new();
        // One join buffer reused across every leaf block's inline text.
        let mut text = String::new();
        // Which extension kinds hold inline text, from their
        // declarations.
        let mut inline_kinds: Vec<String> = Vec::new();
        if let Some(extensions) = self.extensions {
            for extension in extensions {
                for decl in extension.kinds() {
                    if decl.inline_text && !inline_kinds.contains(&decl.kind) {
                        inline_kinds.push(decl.kind);
                    }
                }
            }
        }
        let mut stack: Vec<NodeId> = self.arena.children(root).to_vec();
        while let Some(id) = stack.pop() {
            let is_leaf = match self.arena.kind(id) {
                Node::Paragraph | Node::Heading(_) => true,
                Node::Extension(ext) => inline_kinds.iter().any(|k| k == &ext.kind),
                _ => false,
            };
            if is_leaf {
                // One append per text part: the parts already borrow the
                // source, so no intermediate strings are built.
                text.clear();
                for child in self.arena.children(id) {
                    if let Node::Text(part) = self.arena.kind(*child) {
                        text.push_str(part);
                    }
                }
                self.arena.entry_mut(id).children.clear();
                let block_span = self.arena.span(id);
                let span_map = self.text_map_slot(id).take();
                crate::inline::parse_inlines_into(
                    &mut session,
                    &text,
                    &self.link_defs,
                    &mut self.arena,
                    id,
                    block_span,
                    &mut self.problems,
                    self.extensions,
                    span_map.as_ref(),
                );
            } else {
                stack.extend(self.arena.children(id).iter().copied());
            }
        }
    }

    /// Converts the open paragraph into a setext heading of `level`, first
    /// extracting leading link reference definitions.
    fn setext_heading_or_defs(&mut self, level: u8, line_stop: usize) -> SetextOutcome {
        let (frame_text, id, frame_start) = match self.open.last_mut() {
            Some(Frame {
                kind: OpenKind::Paragraph { text },
                id,
                start_offset,
                ..
            }) => (std::mem::take(text), *id, *start_offset),
            _ => return SetextOutcome::NotSetext,
        };
        // The map leaves its slot for the extraction (the call needs
        // `&mut self`) and returns below: the heading keeps it.
        let mut map = self.text_map_slot(id).take();
        let consumed = self.extract_link_defs(&frame_text, map.as_ref(), frame_start);
        let remainder = frame_text[consumed..].to_string();
        if remainder.is_empty() {
            // The paragraph was nothing but definitions; it stays open
            // with empty text and the line continues it. The map stays
            // taken: nothing holds a text child for it.
            return SetextOutcome::Dissolved;
        }
        self.arena.set_kind(
            id,
            Node::Heading(HeadingData {
                level,
                setext: true,
            }),
        );
        // The map shifts so it covers exactly the bytes the remaining
        // text child holds, which is what the inline phase's cell
        // offsets index into.
        let text_span = map.as_mut().map_or_else(
            || Span::empty(frame_start),
            |map| {
                map.shift(consumed);
                map.map_span(0, remainder.len(), frame_start)
            },
        );
        if let Some(map) = map {
            *self.text_map_slot(id) = Some(map);
        }
        let text_id = self
            .arena
            .alloc(Node::Text(Cow::Owned(remainder)), text_span);
        self.arena.push_child(id, text_id);
        // The frame now only carries the span; the node itself is
        // finished, so the closing step must not treat it as an empty
        // paragraph.
        if let Some(Frame { kind, .. }) = self.open.last_mut() {
            if matches!(kind, OpenKind::Paragraph { .. }) {
                *kind = OpenKind::Heading;
            }
        }
        self.close_last(Some(line_stop));
        SetextOutcome::Converted(id)
    }

    /// Adds the remaining line to the open leaf block, opening a paragraph
    /// under the deepest container when none is open. Returns the id of the
    /// node that received the line, or `None` when the line was blank and
    /// added nothing.
    fn add_text(
        &mut self,
        cursor: &mut Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
        next_line_start: usize,
    ) -> Option<NodeId> {
        let id = self.open.last().unwrap().id;
        match &self.open.last().unwrap().kind {
            OpenKind::Paragraph { .. } => {
                // A normal continuation line: its leading whitespace is
                // consumed here, as the reference parser does. Lazy lines
                // (handled in the walk) keep theirs.
                cursor.skip_to_content();
                self.append_paragraph_rest(id, cursor, line_offset, line_stop, next_line_start);
                Some(id)
            }
            OpenKind::Extension { index, .. } => {
                // An extension block that holds other blocks is a
                // container here, like the block quote and item arms
                // below: blank lines add nothing, other lines start a
                // paragraph inside it. Line-accumulator blocks get the
                // whole leftover line; should a hook decline after its
                // continuation matched, that is the leftover's only use.
                if self
                    .extensions
                    .is_some_and(|extensions| extensions[*index].contains_blocks())
                {
                    if cursor.blank_from_here() {
                        None
                    } else {
                        cursor.skip_to_content();
                        let (text, anchor) =
                            self.anchored_rest(cursor, line_offset, line_stop, next_line_start);
                        Some(self.open_paragraph(&text, line_offset, line_stop, anchor))
                    }
                } else {
                    if cursor.rest_len() > 0 {
                        cursor.bump_all();
                    }
                    self.set_frame_end(line_stop);
                    Some(id)
                }
            }
            OpenKind::CodeBlock { .. } => {
                self.append_code_rest(id, cursor, line_offset, line_stop, next_line_start);
                Some(id)
            }
            OpenKind::HtmlBlock { kind, .. } => {
                let kind = *kind;
                self.append_html_rest(id, cursor, line_offset);
                self.set_frame_end(line_stop);
                if kind <= 5 {
                    let text = self.line_rest_bytes(cursor);
                    if leaf::html_block_end(kind, leaf::trim_ascii(&text)) {
                        self.close_last(Some(line_stop));
                    }
                }
                Some(id)
            }
            _ => {
                // A container: blank lines add nothing; other lines start a
                // paragraph. A list cannot hold a paragraph, so enclosing
                // lists close first (the reference algorithm's add-child
                // walk).
                if cursor.blank_from_here() {
                    None
                } else {
                    cursor.skip_to_content();
                    let (text, anchor) =
                        self.anchored_rest(cursor, line_offset, line_stop, next_line_start);
                    self.close_until_can_contain();
                    Some(self.open_paragraph(&text, line_offset, line_stop, anchor))
                }
            }
        }
    }

    /// Opens a paragraph whose first line may be a lazy continuation.
    ///
    /// A lazily added line means the line was paragraph text of an
    /// enclosing container's existing paragraph child, so that child
    /// continues here instead of a new paragraph starting.
    fn open_paragraph(
        &mut self,
        text: &str,
        line_offset: usize,
        line_stop: usize,
        anchor: LineAnchor,
    ) -> NodeId {
        // The paragraph's span starts at its own first byte: the anchor's
        // source start, which maps the content exactly (tabs and NUL
        // replacements included).
        let content_start = anchor.src_start;
        if self.adding_lazy {
            self.adding_lazy = false;
            let parent = self.open.last().unwrap().id;
            if let Some(&last) = self.arena.children(parent).last() {
                if *self.arena.kind(last) == Node::Paragraph {
                    self.append_paragraph_text(last, text, anchor);
                    self.open.push(Frame {
                        id: last,
                        kind: OpenKind::Paragraph {
                            text: String::new(),
                        },
                        start_line: self.line_number,
                        start_offset: content_start,
                        end_offset: line_stop,
                    });
                    return last;
                }
            }
        }
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(Node::Paragraph, Span::empty(line_offset));
        self.arena.push_child(parent, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::Paragraph {
                text: text.to_string(),
            },
            start_line: self.line_number,
            // The paragraph's span starts at its own first byte: its
            // content, which the anchor maps exactly (tabs and NUL
            // replacements included), not at the line's container prefixes.
            start_offset: content_start,
            end_offset: line_stop,
        });
        self.text_map_slot(id)
            .get_or_insert_with(Default::default)
            .record(text.len(), anchor);
        id
    }

    /// Appends one line of text to a paragraph's buffer, recording the
    /// line's anchor after any joining newline.
    fn append_paragraph_text(&mut self, id: NodeId, text: &str, anchor: LineAnchor) {
        let mut joined = false;
        for frame in self.open.iter_mut().rev() {
            if frame.id == id {
                if let OpenKind::Paragraph { text: buf } = &mut frame.kind {
                    if !buf.is_empty() {
                        buf.push('\n');
                        joined = true;
                    }
                    buf.push_str(text);
                    break;
                }
            }
        }
        let map = self.text_map_slot(id).get_or_insert_with(Default::default);
        if joined {
            map.record_after_join(text.len(), anchor);
        } else {
            map.record(text.len(), anchor);
        }
    }

    /// The map slot for `id`, grown to cover the arena's allocations.
    fn text_map_slot(&mut self, id: NodeId) -> &mut Option<TextMap> {
        let index = id.index();
        if self.text_maps.len() <= index {
            self.text_maps.resize(index + 1, None);
        }
        &mut self.text_maps[index]
    }

    /// The `len` source bytes at `offset` as a `&str`, borrowed when the
    /// range sits on char boundaries (every scanner stops at an ASCII
    /// byte, which is always one) and lossy-converted otherwise, matching
    /// what the byte-oriented call sites produced before.
    fn src_from(&self, offset: usize, len: usize) -> std::borrow::Cow<'a, str> {
        match self.src.get(offset..offset + len) {
            Some(text) => std::borrow::Cow::Borrowed(text),
            None => String::from_utf8_lossy(&self.src.as_bytes()[offset..offset + len]),
        }
    }

    /// The cursor's remaining line as a `&str`: borrowed from the source
    /// when the rest is a plain slice (no partially consumed tab) and the
    /// tab-expanded bytes converted otherwise.
    fn line_rest(&self, cursor: &Cursor<'_>, line_offset: usize) -> std::borrow::Cow<'a, str> {
        if self.line_is_src && cursor.rest_is_slice() {
            self.src_from(line_offset + cursor.pos(), cursor.rest_len())
        } else {
            std::borrow::Cow::Owned(String::from_utf8_lossy(&cursor.rest()).into_owned())
        }
    }

    /// The cursor's remaining line as bytes, for byte-oriented checks.
    fn line_rest_bytes(&self, cursor: &Cursor<'_>) -> Vec<u8> {
        cursor.rest()
    }

    /// The cursor's remaining line with the anchor that maps its buffer
    /// bytes back to the source. `line_stop` is the line's own end and
    /// `next_line_start` the offset just past its ending.
    fn anchored_rest(
        &self,
        cursor: &Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
        next_line_start: usize,
    ) -> (std::borrow::Cow<'a, str>, LineAnchor) {
        let text = self.line_rest(cursor, line_offset);
        let anchor = if self.line_is_src && cursor.rest_is_slice() {
            LineAnchor::whole_line(0, line_offset + cursor.pos(), next_line_start)
        } else if self.line_is_src {
            // A partially consumed tab: the rest starts with synthetic
            // spaces standing for the tab's remaining columns, then the
            // bytes after the tab byte itself.
            let tab = cursor.partial_tab();
            LineAnchor {
                buf_start: 0,
                src_start: line_offset + cursor.pos(),
                lin_start: line_offset + cursor.pos() + 1,
                tab_span: tab as u16,
                src_end: next_line_start,
                nul_map: None,
            }
        } else {
            let nul_map = self.replacement_map(cursor, line_offset, line_stop);
            let src_start = nul_map.first().copied().unwrap_or(next_line_start);
            LineAnchor {
                buf_start: 0,
                src_start,
                lin_start: src_start,
                tab_span: cursor.partial_tab() as u16,
                src_end: next_line_start,
                nul_map: Some(nul_map),
            }
        };
        (text, anchor)
    }

    /// The per-buffer-byte source offsets of the cursor's remaining line
    /// when the line was replaced before processing: each NUL byte became
    /// a three-byte U+FFFD, so the rest is no longer a linear slice. The
    /// cursor's position counts replaced bytes, so it is translated back
    /// through the expansions before the source walk starts.
    fn replacement_map(
        &self,
        cursor: &Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
    ) -> Box<[usize]> {
        let line = &self.src.as_bytes()[line_offset..line_stop];
        let mut map = Vec::with_capacity(cursor.rest_len() + cursor.partial_tab());
        // Walk the original line to the original position of the cursor's
        // replaced position.
        let mut replaced = 0usize;
        let mut i = 0usize;
        while i < line.len() && replaced < cursor.pos() {
            replaced += usize::from(line[i] == 0) * 2 + 1;
            i += 1;
        }
        for _ in 0..cursor.partial_tab() {
            map.push(line_offset + i);
        }
        if cursor.partial_tab() > 0 {
            i += 1;
        }
        while i < line.len() {
            if line[i] == 0 {
                map.extend(std::iter::repeat_n(line_offset + i, 3));
            } else {
                map.push(line_offset + i);
            }
            i += 1;
        }
        map.into_boxed_slice()
    }

    /// The per-byte source offsets for `len` buffer bytes of a replaced
    /// line's content starting at replaced-line position `start_pos`:
    /// walks the source line, counting the NUL expansions, until the
    /// replaced position reaches the region's end.
    fn replacement_range_map(
        &self,
        line_offset: usize,
        line_stop: usize,
        start_pos: usize,
        len: usize,
    ) -> Box<[usize]> {
        let line = &self.src.as_bytes()[line_offset..line_stop];
        let mut map = Vec::with_capacity(len);
        let mut replaced = 0usize;
        let mut i = 0usize;
        while i < line.len() && replaced < start_pos {
            replaced += usize::from(line[i] == 0) * 2 + 1;
            i += 1;
        }
        while i < line.len() && map.len() < len {
            if line[i] == 0 {
                map.extend(std::iter::repeat_n(line_offset + i, 3));
            } else {
                map.push(line_offset + i);
            }
            i += 1;
        }
        map.into_boxed_slice()
    }

    /// Appends the cursor's remaining line to the open paragraph's text.
    fn append_paragraph_rest(
        &mut self,
        id: NodeId,
        cursor: &Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
        next_line_start: usize,
    ) {
        let (text, anchor) = self.anchored_rest(cursor, line_offset, line_stop, next_line_start);
        self.append_paragraph_text(id, &text, anchor);
        // The paragraph's span ends at its last content line's char end,
        // trailing whitespace included; the text child's own span covers
        // the trimmed content.
        self.set_frame_end(line_stop);
    }

    /// Appends the cursor's remaining line to a code block's literal.
    fn append_code_rest(
        &mut self,
        id: NodeId,
        cursor: &Cursor<'_>,
        line_offset: usize,
        line_stop: usize,
        next_line_start: usize,
    ) {
        let text = self.line_rest(cursor, line_offset);
        for frame in self.open.iter_mut().rev() {
            if frame.id == id {
                if let OpenKind::CodeBlock {
                    literal, fenced, ..
                } = &mut frame.kind
                {
                    literal.push_str(&text);
                    literal.push('\n');
                    // A fenced block's span covers its literal's line ends,
                    // but only at the document's end, where the flow's
                    // final newline belongs to the unclosed fence; a fence
                    // that stops mid-document ends at its last line's char
                    // end. An indented block's span always ends at its last
                    // content char, and a blank line adds nothing.
                    if *fenced {
                        frame.end_offset = if next_line_start == self.src.len() {
                            next_line_start
                        } else {
                            line_stop
                        };
                    } else if !text.is_empty() {
                        frame.end_offset = line_stop;
                    }
                    return;
                }
            }
        }
    }

    /// Appends the cursor's remaining line to an HTML block's literal.
    fn append_html_rest(&mut self, id: NodeId, cursor: &Cursor<'_>, line_offset: usize) {
        let text = self.line_rest(cursor, line_offset);
        for frame in self.open.iter_mut().rev() {
            if frame.id == id {
                if let OpenKind::HtmlBlock { literal, .. } = &mut frame.kind {
                    literal.push_str(&text);
                    literal.push('\n');
                    return;
                }
            }
        }
    }

    /// Extends the deepest frame's span to `end`.
    fn set_frame_end(&mut self, end: usize) {
        self.open.last_mut().unwrap().end_offset = end;
    }

    /// Records whether a node's last processed line was blank, clearing the
    /// flags of its ancestors. A blank line also marks the container's last
    /// child blank, which is how tight-versus-loose decisions see blank
    /// lines inside blocks that closed on the blank line. Block quotes,
    /// headings, thematic breaks, fenced code blocks and freshly opened
    /// empty list items are never counted as blank.
    fn update_last_line_blank(&mut self, target: NodeId, on_stack: Option<usize>, blank: bool) {
        if blank {
            if let Some(&last) = self.arena.children(target).last() {
                if self.last_line_blank.len() <= last.index() {
                    self.last_line_blank.resize(last.index() + 1, false);
                }
                self.last_line_blank[last.index()] = true;
            }
        }
        let flag = blank
            && match self.arena.kind(target) {
                Node::BlockQuote | Node::Heading(_) | Node::ThematicBreak => false,
                Node::CodeBlock(data) => !data.fenced,
                Node::HtmlBlock(data) => data.kind >= 6,
                Node::Item(_) => {
                    // An item just opened on a blank-content line does not
                    // count as blank.
                    let fresh = on_stack.is_some_and(|index| {
                        self.open[index].start_line == self.line_number
                            && matches!(self.open[index].kind, OpenKind::Item { .. })
                            && self.arena.children(self.open[index].id).is_empty()
                    });
                    !fresh
                }
                _ => true,
            };
        if self.last_line_blank.len() <= target.index() {
            self.last_line_blank.resize(target.index() + 1, false);
        }
        self.last_line_blank[target.index()] = flag;
        // The node's ancestors are never blank.
        let limit = on_stack.unwrap_or(self.open.len());
        for frame in self.open.iter().take(limit) {
            if self.last_line_blank.len() <= frame.id.index() {
                self.last_line_blank.resize(frame.id.index() + 1, false);
            }
            self.last_line_blank[frame.id.index()] = false;
        }
    }

    /// Closes frames from `depth` to the end of the open stack, each at its
    /// own recorded end (see `close_last`).
    fn close_from(&mut self, depth: usize) {
        while self.open.len() > depth.max(1) {
            self.close_last(None);
        }
    }

    /// Closes the deepest open block, finalizing it. `line_stop` is the
    /// current line's char end; when given, the frame's end is at least
    /// that, because the frame consumed the line itself (a closing fence,
    /// a setext underline). When `None`, the frame keeps its own recorded
    /// end: the char end of the last line its subtree consumed, which a
    /// blank or sibling line must not extend. Either way, the closed
    /// frame's end raises its parent's recorded end, so a container's end
    /// covers its last consumed descendant line.
    fn close_last(&mut self, line_stop: Option<usize>) {
        let mut frame = self.open.pop().expect("the document frame stays open");
        if let Some(line_stop) = line_stop {
            frame.end_offset = line_stop.max(frame.end_offset);
        }
        let end = frame.end_offset;
        match &mut frame.kind {
            OpenKind::Paragraph { text } => {
                let text = std::mem::take(text);
                // The map leaves its slot for the extraction (the call
                // needs `&mut self`) and returns only when a text child
                // keeps needing it.
                let mut map = self.text_map_slot(frame.id).take();
                let consumed = self.extract_link_defs(&text, map.as_ref(), frame.start_offset);
                let remainder = &text[consumed..];
                if remainder.is_empty() {
                    self.remove_last_child_of_parent(frame.id);
                } else {
                    // The map shifts so it covers exactly the bytes the
                    // remaining text child holds, which is what the
                    // inline phase's cell offsets index into.
                    let text_span = map.as_mut().map_or_else(
                        || Span::empty(frame.start_offset),
                        |map| {
                            map.shift(consumed);
                            map.map_span(0, remainder.len(), frame.start_offset)
                        },
                    );
                    if let Some(map) = map {
                        *self.text_map_slot(frame.id) = Some(map);
                    }
                    let text_id = self
                        .arena
                        .alloc(Node::Text(Cow::Owned(remainder.to_string())), text_span);
                    self.arena.push_child(frame.id, text_id);
                }
            }
            OpenKind::CodeBlock {
                fenced: false,
                literal,
                ..
            } => {
                // Remove trailing blank lines: walk back over spaces, tabs
                // and line ends to the last real byte, then cut at the next
                // line end and re-add one, as the reference finalize does.
                let mut text = std::mem::take(literal);
                let mut end = text.len();
                while end > 0 && matches!(text.as_bytes()[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
                    end -= 1;
                }
                if end == 0 {
                    self.remove_last_child_of_parent(frame.id);
                } else {
                    // The first line end at or after the last real byte
                    // marks the start of the trailing blank lines.
                    let cut = text.as_bytes()[end..]
                        .iter()
                        .position(|&b| b == b'\n' || b == b'\r')
                        .map_or(text.len(), |pos| end + pos);
                    text.truncate(cut);
                    text.push('\n');
                    self.arena.set_kind(
                        frame.id,
                        Node::CodeBlock(CodeBlockData {
                            fenced: false,
                            fence_char: 0,
                            info: None,
                            literal: Cow::Owned(text),
                        }),
                    );
                }
            }
            OpenKind::CodeBlock {
                fenced: true,
                fence_char,
                literal,
                ..
            } => {
                let (fence_char, text) = (*fence_char, std::mem::take(literal));
                // The literal's first line is the opening fence's info
                // string: HTML-unescaped, trimmed, then backslash-unescaped.
                let end = text.find('\n').unwrap_or(text.len());
                let raw_info = &text[..end];
                let info = decode_info_string(raw_info);
                let rest = if end < text.len() {
                    text[end + 1..].to_string()
                } else {
                    String::new()
                };
                self.arena.set_kind(
                    frame.id,
                    Node::CodeBlock(CodeBlockData {
                        fenced: true,
                        fence_char,
                        info: info.map(Cow::Owned),
                        literal: Cow::Owned(rest),
                    }),
                );
            }
            OpenKind::HtmlBlock { kind, literal } => {
                let (kind, text) = (*kind, std::mem::take(literal));
                self.arena.set_kind(
                    frame.id,
                    Node::HtmlBlock(HtmlBlockData {
                        kind,
                        literal: Cow::Owned(text),
                    }),
                );
            }
            OpenKind::List(_) => {
                self.finalize_list_tightness(frame.id);
            }
            OpenKind::Extension { index, state } => {
                let mut state = state.take().expect("an extension frame carries its state");
                let extensions = self.extensions.unwrap();
                let mut ctx = crate::extension::CloseCtx {
                    arena: &mut self.arena,
                    text_maps: &mut self.text_maps,
                    node: frame.id,
                    start_offset: frame.start_offset,
                    end_offset: end,
                };
                extensions[*index].close(state.as_mut(), &mut ctx);
            }
            _ => {}
        }
        if let Some(parent) = self.open.last_mut() {
            parent.end_offset = parent.end_offset.max(end);
        }
        self.arena
            .set_span(frame.id, Span::new(frame.start_offset, end));
    }

    /// Removes a node from its parent's children (the parent is the frame
    /// just below the closed one, or the document root).
    fn remove_last_child_of_parent(&mut self, id: NodeId) {
        let parent = self
            .open
            .last()
            .expect("the document frame remains open")
            .id;
        if let Some(last) = self.arena.last_child(parent) {
            if last == id {
                self.arena.pop_child(parent);
            }
        }
    }

    /// Computes a list's tightness from its items' blank-line flags: a list
    /// is loose when a non-final item ends with a blank line, or when any
    /// item but the last, or any child of an item but the last child of the
    /// last item, ends with a blank line.
    fn finalize_list_tightness(&mut self, list_id: NodeId) {
        let items = self.arena.children(list_id).to_vec();
        let mut tight = true;
        'items: for (index, &item) in items.iter().enumerate() {
            let item_has_next = index + 1 < items.len();
            if item_has_next && self.is_last_line_blank(item) {
                tight = false;
                break;
            }
            let subs = self.arena.children(item).to_vec();
            for (sub_index, &sub) in subs.iter().enumerate() {
                let sub_has_next = sub_index + 1 < subs.len();
                if (item_has_next || sub_has_next) && self.ends_with_blank_line(sub) {
                    tight = false;
                    break 'items;
                }
            }
        }
        if let Node::List(data) = &mut self.arena.entry_mut(list_id).kind {
            data.tight = tight;
        }
    }

    /// Reads a node's last-line-blank flag.
    fn is_last_line_blank(&self, id: NodeId) -> bool {
        self.last_line_blank
            .get(id.index())
            .copied()
            .unwrap_or(false)
    }

    /// Whether the block ends with a blank line, following the last-child
    /// chain through lists and items.
    fn ends_with_blank_line(&self, id: NodeId) -> bool {
        if self.is_last_line_blank(id) {
            return true;
        }
        match self.arena.kind(id) {
            Node::List(_) | Node::Item(_) => match self.arena.last_child(id) {
                Some(child) => self.ends_with_blank_line(child),
                None => false,
            },
            _ => false,
        }
    }

    /// Extracts the link reference definitions at the start of a
    /// paragraph's text, recording them and returning the consumed byte
    /// count. Only whitespace may separate consecutive definitions; the
    /// first non-definition text stops the scan.
    fn extract_link_defs(
        &mut self,
        text: &str,
        map: Option<&TextMap>,
        fallback_start: usize,
    ) -> usize {
        // Definitions start with `[` after leading whitespace, and the
        // scan only continues after a successful definition, so when the
        // first non-whitespace byte is not `[` nothing can be consumed.
        // The pre-check skips the scanner for those paragraphs.
        let bytes = text.as_bytes();
        let mut probe = 0usize;
        while matches!(bytes.get(probe), Some(b' ') | Some(b'\t') | Some(b'\n')) {
            probe += 1;
        }
        if bytes.get(probe) != Some(&b'[') {
            return 0;
        }
        let mut consumed = 0usize;
        loop {
            let mut skip = 0usize;
            let bytes = &text.as_bytes()[consumed..];
            while matches!(bytes.get(skip), Some(b' ') | Some(b'\t') | Some(b'\n')) {
                skip += 1;
            }
            let Some(scanned) = leaf::scan_link_def(&text[consumed + skip..]) else {
                break;
            };
            let buf_start = consumed + skip;
            let buf_end = buf_start + scanned.consumed;
            let consumed_len = scanned.consumed;
            let def = LinkDef {
                label: Cow::Owned(scanned.label),
                destination: Cow::Owned(crate::inline::clean_url(&scanned.destination)),
                title: scanned
                    .title
                    .and_then(|t| crate::inline::clean_title(&t))
                    .map(Cow::Owned),
            };
            // The definition's span: the scanned buffer bytes mapped back
            // to the source, then trimmed of the line ending the scan
            // consumed so the span stops at the line's last byte.
            let mut span = match map {
                Some(map) => map.map_span(buf_start, buf_end, fallback_start),
                None => Span::new(fallback_start + buf_start, fallback_start + buf_end),
            };
            while span.end > span.start
                && matches!(
                    self.src.as_bytes().get(span.end - 1),
                    Some(b'\n') | Some(b'\r')
                )
            {
                span.end -= 1;
            }
            self.defs.push(SpannedDef {
                span,
                def: def.clone(),
                titled: scanned.titled,
            });
            let key = normalize_label(&def.label);
            if let std::collections::hash_map::Entry::Vacant(e) = self.link_defs.entry(key) {
                e.insert(def);
            }
            consumed += skip + consumed_len;
        }
        consumed
    }
}

/// Decodes a fenced code block's info string: HTML entities, then trimming,
/// then backslash escapes. Empty results become `None`.
fn decode_info_string(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let unescaped = crate::inline::unescape_entities(raw);
    // The trim follows the reference's `cmark_isspace`, whose class is
    // the spec's whitespace definition: vertical tab and form feed are
    // whitespace and trim away.
    let trimmed =
        unescaped.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\x0B' | '\x0C' | '\r' | '\n'));
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let final_unescaped = leaf::unescape(bytes);
    if final_unescaped.is_empty() {
        None
    } else {
        Some(final_unescaped)
    }
}

/// Normalizes a link label: Unicode case folding with ASCII whitespace
/// runs collapsed to a single space, as the reference parser does.
pub fn normalize_label(label: &str) -> String {
    let mut out = String::new();
    normalize_label_into(label, &mut out);
    out
}

/// Resolves a link label's escapes and character references in one pass,
/// as the reference reads labels; the rules are the inline phase's label
/// decoder, applied at definition-insert time.
#[must_use]
pub fn decode_label(label: &str) -> String {
    crate::inline::decode_label(label)
}

/// Writes the normalized form of `label` into `out`, which is cleared
/// first; reuse the buffer to skip the allocation per lookup.
pub(crate) fn normalize_label_into(label: &str, out: &mut String) {
    out.clear();
    let mut pending_ws = false;
    for ch in label.chars() {
        // The reference's label whitespace is `cmark_isspace`, whose
        // class is the spec's whitespace definition: vertical tab and
        // form feed are whitespace in a label too.
        if matches!(ch, ' ' | '\t' | '\x0B' | '\x0C' | '\n' | '\r') {
            pending_ws = true;
            continue;
        }
        if pending_ws && !out.is_empty() {
            out.push(' ');
        }
        pending_ws = false;
        let overrides = crate::inline::fold::FOLD_OVERRIDES;
        if let Ok(idx) = overrides.binary_search_by(|(cp, _)| (*cp).cmp(&(ch as u32))) {
            out.push_str(overrides[idx].1);
        } else {
            out.extend(ch.to_lowercase());
        }
    }
}
