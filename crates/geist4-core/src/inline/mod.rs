//! Phase 2 of the parser: inline content.
//!
//! The implementation follows the reference algorithm's inline phase: a
//! single left-to-right scan over the block's text that emits inline nodes
//! into a linked list, records emphasis delimiters and link brackets as it
//! goes, and resolves both stacks at the end. Emphasis handling mirrors the
//! delimiter-stack algorithm exactly, including the "rule of three" and the
//! per-delimiter-class opener boundaries that keep lookups linear.

mod entities;
mod entity_table;
pub(crate) mod fold;
pub(crate) mod unicode;

use std::collections::HashMap;

use crate::node::{LinkDef, Node};
use unicode::{is_ascii_punct_byte, is_punct_or_symbol, is_space};

/// The longest link label the inline phase will scan.
const MAX_LINK_LABEL_LENGTH: usize = 1000;

/// An inline node in the parser's working pool.
struct Cell {
    alive: bool,
    kind: Kind,
    prev: Option<u32>,
    next: Option<u32>,
    first_child: Option<u32>,
    last_child: Option<u32>,
    /// The container holding this cell, if any; lets `unlink` repair the
    /// container's child endpoints in constant time.
    parent: Option<u32>,
    /// The cell's byte range in the block's inline text (not the source
    /// document: the offset mapping to source coordinates happens when
    /// the pool is attached to the tree). Containers' ranges cover their
    /// delimiters; break cells cover the trimmed whitespace and the
    /// line ending they stand for.
    start: u32,
    end: u32,
}

/// What a [`Cell`] holds.
pub(crate) enum Kind {
    /// Literal text; emphasis processing shortens these in place.
    Text(String),
    /// A code span's content, already normalized.
    Code(String),
    /// Raw inline HTML, verbatim.
    Html(String),
    /// A line break inside a paragraph.
    SoftBreak,
    /// A hard line break.
    HardBreak,
    /// Emphasis; children are its content.
    Emph,
    /// Strong emphasis; children are its content.
    Strong,
    /// A backslash-escaped character; its only child is the character
    /// as text. The wrapper keeps hooks that read the sibling chain —
    /// the footnote claim, for one — from seeing the escaped character
    /// as ordinary text, exactly as the reference's own wrapper does.
    Escaped,
    /// An extension's node; children are its content, and the kind
    /// name selects the extension's renderer.
    Ext {
        /// The kind name, as the extension declared it.
        kind: String,
        /// Attributes the creating hook set; empty for the built-in
        /// wraps.
        attrs: Vec<(String, crate::extension::AttrValue)>,
    },
    /// A link or image; children are its content.
    Link {
        url: String,
        title: Option<String>,
        image: bool,
        /// The reference provenance when the link resolved through a
        /// link reference definition, `None` for inline links.
        reference: Option<crate::node::LinkRef<'static>>,
    },
}

/// One emphasis delimiter run.
pub(crate) struct DelimCell {
    alive: bool,
    ch: u8,
    can_open: bool,
    can_close: bool,
    length: usize,
    /// The position in the input the run starts at.
    position: usize,
    /// The text node holding the delimiter characters.
    node: u32,
    /// For a run an extension pushed: the extension node kind its
    /// pairs wrap their content in. `None` for the built-in emphasis
    /// delimiters, whose pairing the engine owns.
    ext_kind: Option<String>,
    prev: Option<u32>,
    next: Option<u32>,
}

/// The delimiter stack, a doubly-linked list walked from the tail.
#[derive(Default)]
struct DelimList {
    cells: Vec<DelimCell>,
    last: Option<u32>,
}

impl DelimList {
    fn push(&mut self, cell: DelimCell) {
        let id = self.cells.len() as u32;
        self.cells.push(DelimCell {
            prev: self.last,
            next: None,
            ..cell
        });
        if let Some(prev) = self.last {
            self.cells[prev as usize].next = Some(id);
        }
        self.last = Some(id);
    }

    fn remove(&mut self, id: u32) {
        let (prev, next) = {
            let cell = &self.cells[id as usize];
            (cell.prev, cell.next)
        };
        if let Some(p) = prev {
            self.cells[p as usize].next = next;
        }
        if let Some(n) = next {
            self.cells[n as usize].prev = prev;
        }
        if self.last == Some(id) {
            self.last = prev;
        }
        self.cells[id as usize].alive = false;
    }
}

/// One open link or image bracket.
struct Bracket {
    image: bool,
    /// The text node holding `[`.
    node: u32,
    /// For an image bracket, the text node holding the `!` before the
    /// `[`. A footnote claim consumes the bracket but leaves the
    /// marker; a matched link removes both.
    bang: Option<u32>,
    /// The input position of the bracket's start.
    position: usize,
    /// Whether another bracket opened between this one and the current
    /// position.
    bracket_after: bool,
    /// Whether this bracket is the retry a failed attempt's rewind
    /// opened: the same author mistake seen twice, so its failure
    /// records no second problem.
    rescued: bool,
}

/// The inline parser state.
struct Parser<'defs, 'a, 'ext> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    pool: Vec<Cell>,
    /// The first and last nodes of the output list.
    head: Option<u32>,
    tail: Option<u32>,
    delims: DelimList,
    brackets: Vec<Bracket>,
    no_link_openers: bool,
    /// Raw HTML scans that failed for lack of a closer, per the reference
    /// parser's guards against rescanning them.
    skip_comment: bool,
    skip_cdata: bool,
    skip_declaration: bool,
    skip_pi: bool,
    /// The registered extensions, or `None` on the plain path; the
    /// hook sites short-circuit on `None`.
    extensions: Option<&'ext [std::sync::Arc<dyn crate::extension::Extension>]>,
    /// `(trigger byte, extension index)` pairs, in registration order;
    /// empty on the plain path.
    triggers: Vec<(u8, usize)>,
    /// The distinct trigger bytes; text runs stop at them so their
    /// hooks can fire. Empty on the plain path.
    trigger_bytes: Vec<u8>,
    /// Positions of backtick runs seen while scanning for code-span
    /// closers, as in the reference parser's memo table, with the
    /// recording call's generation in the high bits. Entries persist
    /// across scans, including successful ones that exited early, which
    /// is what the reference implementation does; the generation makes
    /// entries from earlier subjects read as "never seen" without
    /// re-zeroing the table.
    backticks_memo: Vec<u64>,
    memo_gen: u64,
    /// Whether a code-span scan has run to the end of this subject
    /// without finding a closer; only then do memo entries prove
    /// absence, exactly as the reference parser gates them.
    scanned_for_backticks: bool,
    link_defs: &'defs HashMap<String, LinkDef<'a>>,
    /// Reusable buffer for normalized link-label lookups; taken from the
    /// session so one document's lookups share its capacity.
    label_scratch: String,
    /// The span of the leaf block whose inline text this is; the span
    /// recorded for problems found at inline level, since inline nodes
    /// keep no spans of their own.
    block_span: crate::span::Span,
    /// Problems noticed while parsing this block's inline content,
    /// drained by the caller after `run`.
    problems: Vec<crate::problems::Problem>,
    /// Where a failed bracket attempt rewound to, so the bracket the
    /// re-scan opens there is recognized as the parser's own retry —
    /// the same author mistake seen twice — and stays silent.
    rescue_position: Option<usize>,
}

/// Backtick runs longer than this are never scanned for closers, as in
/// the reference implementation's `MAXBACKTICKS`.
const MAXBACKTICKS: usize = 1000;

/// The bits of a memo entry that hold the recorded run position; the
/// bits above them hold the recording call's generation.
const MEMO_POS_MASK: u64 = (1 << 40) - 1;

/// Parses one block's inline content.
///
/// Trailing whitespace comes off the subject first, as the reference
/// parser rtrims each block's inline text. `link_defs` holds the
/// document's reference definitions; every leaf block sees them all.
/// Scratch buffers reused across one document's inline parses: the cell
/// pool, delimiter list, bracket stack and backtick memo would otherwise
/// be reallocated for every leaf block.
pub(crate) struct InlineSession {
    pool: Vec<Cell>,
    delims: DelimList,
    brackets: Vec<Bracket>,
    backticks_memo: Vec<u64>,
    memo_gen: u64,
    /// Scratch buffer reused for normalized link-label lookups.
    label_scratch: String,
}

impl InlineSession {
    pub(crate) fn new() -> Self {
        InlineSession {
            pool: Vec::new(),
            delims: DelimList::default(),
            brackets: Vec::new(),
            backticks_memo: Vec::new(),
            memo_gen: 0,
            label_scratch: String::new(),
        }
    }
}

// The parse-scoped arguments are all needed and read in one place;
// bundling them would trade a clear call for an args struct.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_inlines_into(
    session: &mut InlineSession,
    text: &str,
    link_defs: &HashMap<String, LinkDef<'_>>,
    arena: &mut crate::arena::Arena<'_>,
    parent: crate::arena::NodeId,
    block_span: crate::span::Span,
    problems: &mut Vec<crate::problems::Problem>,
    extensions: Option<&[std::sync::Arc<dyn crate::extension::Extension>]>,
    span_map: Option<&crate::parser::TextMap>,
) {
    // The paragraph's text loses trailing whitespace, as the reference's
    // `cmark_isspace` rtrim does; the spec's whitespace definition
    // includes vertical tab and form feed.
    let text = text.trim_end_matches([' ', '\t', '\x0B', '\x0C', '\r', '\n']);
    // Each extension's trigger bytes join the dispatch table; the same
    // byte from several extensions calls each hook in registration
    // order.
    let mut triggers: Vec<(u8, usize)> = Vec::new();
    if let Some(extensions) = extensions {
        for (index, ext) in extensions.iter().enumerate() {
            for byte in ext.inline_triggers() {
                triggers.push((*byte, index));
            }
        }
    }
    let trigger_bytes: Vec<u8> = {
        let mut bytes = Vec::new();
        for (byte, _) in &triggers {
            if !bytes.contains(byte) {
                bytes.push(*byte);
            }
        }
        bytes
    };
    let mut parser = Parser {
        input: text,
        bytes: text.as_bytes(),
        pos: 0,
        pool: std::mem::take(&mut session.pool),
        head: None,
        tail: None,
        delims: std::mem::take(&mut session.delims),
        brackets: std::mem::take(&mut session.brackets),
        no_link_openers: true,
        skip_comment: false,
        skip_cdata: false,
        skip_declaration: false,
        skip_pi: false,
        extensions,
        triggers,
        trigger_bytes,
        backticks_memo: std::mem::take(&mut session.backticks_memo),
        memo_gen: session.memo_gen.wrapping_add(1),
        scanned_for_backticks: false,
        link_defs,
        label_scratch: std::mem::take(&mut session.label_scratch),
        block_span,
        problems: Vec::new(),
        rescue_position: None,
    };
    session.memo_gen = parser.memo_gen;
    parser.run();
    problems.append(&mut parser.problems);
    // A container's span must enclose its children even when delimiter
    // runs interleave across nested matches, so widen each container's
    // range over its subtree before the ranges are mapped to the source.
    widen_container_spans(&mut parser.pool);
    // Move the surviving cells straight into the arena: no intermediate
    // node tree, no second copy of the text. Cells' byte ranges map back
    // to source offsets through the block's text map, falling back to the
    // block's own span when no map covers this text.
    attach_pool(
        &mut parser.pool,
        parser.head,
        arena,
        parent,
        span_map,
        block_span.start,
    );
    // Give the grown buffers back to the session so the next leaf block
    // reuses their capacity instead of allocating from scratch.
    parser.pool.clear();
    session.pool = parser.pool;
    parser.delims.cells.clear();
    parser.delims.last = None;
    session.delims = parser.delims;
    parser.brackets.clear();
    session.brackets = parser.brackets;
    session.backticks_memo = parser.backticks_memo;
    session.label_scratch = parser.label_scratch;
}

/// Widens every alive container cell's byte range to cover its whole
/// subtree, bottom-up. Nested emphasis matches can share a delimiter run,
/// so a child's matched range may reach past its parent's own closer;
/// the union keeps every container's span enclosing its content.
fn widen_container_spans(pool: &mut [Cell]) {
    enum Visit {
        Enter(u32),
        Exit(u32),
    }
    // Dead cells' ranges are stale; only alive subtrees matter, and the
    // attach walk below visits exactly those.
    let mut stack: Vec<Visit> = Vec::new();
    for (id, cell) in pool.iter().enumerate() {
        if cell.alive && cell.parent.is_none() {
            stack.push(Visit::Enter(id as u32));
        }
    }
    while let Some(step) = stack.pop() {
        match step {
            Visit::Enter(id) => {
                stack.push(Visit::Exit(id));
                let mut child = pool[id as usize].first_child;
                while let Some(current) = child {
                    if pool[current as usize].alive {
                        stack.push(Visit::Enter(current));
                    }
                    child = pool[current as usize].next;
                }
            }
            Visit::Exit(id) => {
                let mut start = pool[id as usize].start;
                let mut end = pool[id as usize].end;
                let mut child = pool[id as usize].first_child;
                while let Some(current) = child {
                    if pool[current as usize].alive {
                        let (cs, ce) = (pool[current as usize].start, pool[current as usize].end);
                        // A (0, 0) range means "never recorded", not "the
                        // source's first byte"; leave it out.
                        if !(cs == 0 && ce == 0) {
                            if cs < start {
                                start = cs;
                            }
                            if ce > end {
                                end = ce;
                            }
                        }
                    }
                    child = pool[current as usize].next;
                }
                let cell = &mut pool[id as usize];
                cell.start = start;
                cell.end = end;
            }
        }
    }
}

/// Moves the surviving pool cells into the arena as children of `parent`,
/// in document order. The walk is iterative — children queue behind their
/// siblings on one stack — so a 10,000-deep emphasis tree cannot overflow
/// the call stack. Consumed cells become empty text cells; the pool is
/// cleared right after, so the placeholder is never observed.
fn attach_pool(
    pool: &mut [Cell],
    head: Option<u32>,
    arena: &mut crate::arena::Arena<'_>,
    parent: crate::arena::NodeId,
    span_map: Option<&crate::parser::TextMap>,
    fallback_start: usize,
) {
    let mut work: Vec<(crate::arena::NodeId, u32)> = Vec::new();
    let mut kids: Vec<u32> = Vec::new();
    // Roots in reverse, so the LIFO stack emits document order.
    let mut id = head;
    while let Some(current) = id {
        if pool[current as usize].alive {
            kids.push(current);
        }
        id = pool[current as usize].next;
    }
    kids.reverse();
    work.extend(kids.drain(..).map(|cell| (parent, cell)));
    while let Some((arena_parent, cell_id)) = work.pop() {
        let cell = &mut pool[cell_id as usize];
        let kind = std::mem::replace(&mut cell.kind, Kind::Text(String::new()));
        let first_child = cell.first_child;
        // An escaped character's wrapper dissolves here: its text
        // joins the wrapper's siblings in its place, so nothing past
        // the inline phase ever sees the wrapper.
        if matches!(kind, Kind::Escaped) {
            let mut kid = first_child;
            while let Some(child) = kid {
                if pool[child as usize].alive {
                    kids.push(child);
                }
                kid = pool[child as usize].next;
            }
            kids.reverse();
            work.extend(kids.drain(..).map(|child| (arena_parent, child)));
            continue;
        }
        let node = node_from_kind(kind);
        let span = match span_map {
            Some(map) => {
                let cell = &pool[cell_id as usize];
                map.map_span(cell.start as usize, cell.end as usize, fallback_start)
            }
            None => {
                let cell = &pool[cell_id as usize];
                crate::span::Span::new(
                    fallback_start + cell.start as usize,
                    fallback_start + cell.end as usize,
                )
            }
        };
        let id = arena.alloc(node, span);
        arena.push_child(arena_parent, id);
        // Children in reverse, for the same reason as the roots.
        let mut kid = first_child;
        while let Some(child) = kid {
            if pool[child as usize].alive {
                kids.push(child);
            }
            kid = pool[child as usize].next;
        }
        kids.reverse();
        work.extend(kids.drain(..).map(|child| (id, child)));
    }
}

/// The node of one pool cell, moving the cell's strings into it.
fn node_from_kind(kind: Kind) -> Node<'static> {
    match kind {
        Kind::Text(text) => Node::Text(text.into()),
        Kind::Code(text) => Node::CodeSpan(text.into()),
        Kind::Html(text) => Node::InlineHtml(text.into()),
        Kind::SoftBreak => Node::SoftBreak,
        Kind::HardBreak => Node::HardBreak,
        Kind::Emph => Node::Emphasis,
        Kind::Strong => Node::Strong,
        // Dissolved in `attach_pool` before this match; a stray one
        // would be a wrapper the walk skipped.
        Kind::Escaped => unreachable!("escaped wrapper survived to the arena"),
        Kind::Ext { kind, attrs } => Node::Extension(crate::extension::ExtNode { kind, attrs }),
        Kind::Link {
            url,
            title,
            image,
            reference,
        } => Node::Link(crate::node::LinkData {
            url: url.into(),
            title: title.map(std::borrow::Cow::Owned),
            image,
            reference,
        }),
    }
}

/// The state an inline bracket hook drives. Private to the engine; the
/// public wrapper is `crate::extension::InlineBracketCtx`.
pub(crate) struct BracketInner<'p, 'defs, 'a, 'ext> {
    parser: &'p mut Parser<'defs, 'a, 'ext>,
    /// The text node holding the bracket's `[`.
    opener_node: u32,
    /// The input position of the bracket's start.
    opener_position: usize,
    /// The group's text content, built on first use: `None` is not yet
    /// built, `Some(None)` is a group with a non-text node inside,
    /// `Some(Some)` is the built text.
    content: Option<Option<String>>,
    /// Set by a claim: the kind and attributes of the node that
    /// replaces the group.
    claimed: Option<BracketClaim>,
}

/// A bracket claim: the node's kind, its attributes, and the content
/// bytes a decision keeps as the node's inline children — `None`
/// discards the content, the footnote marker's shape.
type BracketClaim = (
    String,
    Vec<(String, crate::extension::AttrValue)>,
    Option<usize>,
);

impl<'p, 'defs, 'a, 'ext> BracketInner<'p, 'defs, 'a, 'ext> {
    /// The group's first content node's text, if any.
    fn first_text(&self) -> Option<&str> {
        let next = self.parser.pool[self.opener_node as usize].next?;
        match &self.parser.pool[next as usize].kind {
            crate::inline::Kind::Text(t) | crate::inline::Kind::Html(t) => Some(t),
            _ => None,
        }
    }

    /// Builds the group's text content, or `None` when a non-text node
    /// sits inside the brackets.
    fn build_content(&mut self) -> Option<String> {
        if let Some(cached) = &self.content {
            return cached.clone();
        }
        let mut text = String::new();
        let mut ok = true;
        let mut id = self.parser.pool[self.opener_node as usize].next;
        while let Some(cur) = id {
            match &self.parser.pool[cur as usize].kind {
                crate::inline::Kind::Text(t) | crate::inline::Kind::Html(t) => text.push_str(t),
                _ => {
                    ok = false;
                    break;
                }
            }
            id = self.parser.pool[cur as usize].next;
        }
        let built = ok.then_some(text);
        self.content = Some(built.clone());
        built
    }
}

impl<'p, 'defs, 'a, 'ext> crate::extension::InlineBracketCtx<'p, 'defs, 'a, 'ext> {
    /// Whether the `[`…`]` group's text content starts with `prefix`.
    /// This reads only the group's first node, so a hook that checks it
    /// before building the content stays cheap on bracket-heavy input.
    pub fn content_starts_with(&mut self, prefix: &[u8]) -> bool {
        self.inner
            .first_text()
            .is_some_and(|t| t.as_bytes().starts_with(prefix))
    }

    /// The `[`…`]` group's text content: every Text and inline HTML
    /// node between the brackets, concatenated, already entity-decoded
    /// where the inline phase decoded it. `None` when a code span,
    /// emphasis, break or other non-text node sits inside the brackets.
    /// Build this only after [`content_starts_with`](Self::content_starts_with)
    /// passed, so bracket-heavy input stays linear.
    pub fn content(&mut self) -> Option<&str> {
        self.inner.build_content();
        self.inner.content.as_ref().and_then(|c| c.as_deref())
    }

    /// The input position of the bracket's opening `[`.
    #[must_use]
    pub fn opener_position(&self) -> usize {
        self.inner.opener_position
    }

    /// Claims the whole `[`…`]` group — the opening bracket, its
    /// content nodes and the closing bracket — as one inline node of
    /// `kind` with `attrs`, discarding the content: the marker-node
    /// shape the footnote reference uses, where the label lives in the
    /// extension's own state, not in the tree.
    pub fn claim(&mut self, kind: String, attrs: Vec<(String, crate::extension::AttrValue)>) {
        if self.inner.claimed.is_none() {
            self.inner.claimed = Some((kind, attrs, None));
        }
    }

    /// Claims the `[`…`]` group as one inline node of `kind` with
    /// `attrs`, keeping the group's content: the content's bytes up to
    /// `end` become the node's inline children (the whole content when
    /// `end` reaches past it); bytes from `end` on stay literal text
    /// after the node. The world's `inline-bracket` node decision
    /// carries this `end`.
    pub fn claim_with_content(
        &mut self,
        kind: String,
        attrs: Vec<(String, crate::extension::AttrValue)>,
        end: usize,
    ) {
        if self.inner.claimed.is_none() {
            self.inner.claimed = Some((kind, attrs, Some(end)));
        }
    }
}

/// The state an inline trigger hook drives. Private to the engine; the
/// public wrapper is `crate::extension::InlineTriggerCtx`.
pub(crate) struct TriggerInner<'p, 'defs, 'a, 'ext> {
    /// The byte the hook was called for.
    byte: u8,
    /// The run `scan_run` created but `push_delim` has not consumed.
    pending: Option<(u32, usize)>,
    parser: &'p mut Parser<'defs, 'a, 'ext>,
}

impl<'p, 'defs, 'a, 'ext> crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext> {
    /// The trigger byte the hook was called for.
    #[must_use]
    pub fn byte(&self) -> u8 {
        self.inner.byte
    }

    /// The cursor's position in the inline subject, in bytes.
    #[must_use]
    pub fn position(&self) -> usize {
        self.inner.parser.pos
    }

    /// Scans a delimiter run of the trigger byte at the cursor: the
    /// run's characters are consumed and their text node is pushed,
    /// exactly as the emphasis machinery scans `*` and `_` runs,
    /// including the flanking rules. The run stays pending;
    /// [`push_delim`](crate::extension::InlineTriggerCtx::push_delim)
    /// decides whether it joins the delimiter stack.
    pub fn scan_run(&mut self) -> crate::extension::Run {
        let (length, can_open, can_close) = self.inner.parser.scan_delims(self.inner.byte);
        let text = self.inner.parser.input[self.inner.parser.pos - length..self.inner.parser.pos]
            .to_string();
        let node = self.inner.parser.push_spanned(
            Kind::Text(text),
            self.inner.parser.pos - length,
            self.inner.parser.pos,
        );
        self.inner.pending = Some((node, self.inner.parser.pos));
        crate::extension::Run {
            length,
            can_open,
            can_close,
        }
    }

    /// Puts the pending run from
    /// [`scan_run`](crate::extension::InlineTriggerCtx::scan_run) on
    /// the delimiter stack, tagged with `kind`. When a later run of the
    /// same byte and the same kind closes against it, the content
    /// between the two runs is wrapped in an extension node of `kind`.
    ///
    /// # Panics
    ///
    /// Panics when no run was scanned, or when `scan_run` was followed
    /// by another `scan_run` without an intervening `push_delim`.
    pub fn push_delim(&mut self, run: &crate::extension::Run, kind: &str) {
        let Some((node, position)) = self.inner.pending.take() else {
            panic!("push_delim without a pending scan_run");
        };
        self.inner.parser.delims.push(DelimCell {
            alive: true,
            ch: self.inner.byte,
            can_open: run.can_open,
            can_close: run.can_close,
            length: run.length,
            position,
            node,
            ext_kind: Some(kind.to_owned()),
            prev: None,
            next: None,
        });
    }

    /// The subject's bytes from the trigger position to the end of the
    /// current line, as the reference scanners work on them: a match
    /// extends past ordinary bytes to a line end, not to the next
    /// special character.
    #[must_use]
    pub fn rest_of_line(&self) -> &'a [u8] {
        self.inner.rest_of_line()
    }

    /// The subject's bytes from the start of the current line to the
    /// cursor, as the reference's chunk view lets a matcher rewind
    /// before the trigger byte: a newline or carriage return bounds
    /// the view on the left, the cursor bounds it on the right.
    #[must_use]
    pub fn line_prefix(&self) -> &'a [u8] {
        self.inner.line_prefix()
    }

    /// The byte before the cursor, when there is one.
    #[must_use]
    pub fn preceding(&self) -> Option<u8> {
        self.inner.preceding()
    }

    /// Whether an open link or image bracket encloses the position.
    /// Matches never fire inside brackets, so a bare URL in unresolved
    /// or resolved link syntax stays literal.
    #[must_use]
    pub fn in_bracket(&self) -> bool {
        self.inner.in_bracket()
    }

    /// Unwrites `n` bytes of trailing text: walks back over the text
    /// cells before the cursor shortening them, as the reference's
    /// node_unput does for an extended URL autolink's scheme. Stops at
    /// a non-text cell or the list's start; a multi-byte character is
    /// never split because the count crosses ASCII bytes only.
    pub fn unput(&mut self, n: usize) {
        self.inner.unput(n);
    }

    /// Emits a link whose text is the source bytes from `rewind` bytes
    /// before the cursor through `take` bytes after it, consuming that
    /// span. The `rewind` bytes must be trailing text, unwritten first
    /// as by [`unput`](Self::unput); the link's destination is `url`.
    /// A link's children are its text, so emphasis runs inside the span
    /// never pair, exactly as the reference's inline autolinks swallow
    /// the characters they match.
    pub fn emit_link(&mut self, rewind: usize, take: usize, url: String) {
        self.inner.emit_link(rewind, take, url);
    }
}

impl<'p, 'defs, 'a, 'ext> TriggerInner<'p, 'defs, 'a, 'ext> {
    /// The subject's bytes from the cursor to the end of the current
    /// line: a newline or carriage return bounds the view, as the
    /// reference's NUL-terminated per-line buffers do.
    fn rest_of_line(&self) -> &'a [u8] {
        let start = self.parser.pos;
        let bytes = self.parser.bytes;
        let end = bytes[start..]
            .iter()
            .position(|byte| matches!(byte, b'\n' | b'\r'))
            .map_or(bytes.len(), |offset| start + offset);
        &bytes[start..end]
    }

    /// The byte before the cursor, when there is one.
    fn preceding(&self) -> Option<u8> {
        self.parser
            .bytes
            .get(self.parser.pos.checked_sub(1)?)
            .copied()
    }

    /// The subject's bytes from the start of the current line to the
    /// cursor: the mirror of `rest_of_line` on the left, for matchers
    /// that rewind before the trigger byte, as the scheme autolink's
    /// `cmark_chunk_strchr` does over the reference's chunk.
    fn line_prefix(&self) -> &'a [u8] {
        let end = self.parser.pos;
        let bytes = self.parser.bytes;
        let start = bytes[..end]
            .iter()
            .rposition(|byte| matches!(byte, b'\n' | b'\r'))
            .map_or(0, |offset| offset + 1);
        &bytes[start..end]
    }

    /// Whether any link or image bracket is open: the reference's
    /// in-bracket flags accumulate over the whole bracket stack, and
    /// both match kinds decline when either is set.
    fn in_bracket(&self) -> bool {
        !self.parser.brackets.is_empty()
    }

    /// Unwrites `n` bytes of trailing text, walking back over text
    /// cells exactly as the reference's node_unput does.
    fn unput(&mut self, mut n: usize) {
        let mut cursor = self.parser.tail;
        while n > 0 {
            let Some(id) = cursor else { break };
            let cell = &mut self.parser.pool[id as usize];
            if !matches!(cell.kind, Kind::Text(_)) {
                break;
            }
            let Kind::Text(text) = &mut cell.kind else {
                unreachable!("checked above");
            };
            if text.len() < n {
                n -= text.len();
                text.clear();
                cell.end = cell.start;
                cursor = cell.prev;
            } else {
                text.truncate(text.len() - n);
                cell.end -= n as u32;
                n = 0;
            }
        }
    }

    /// Emits a link around the span `[pos - rewind, pos + take)`,
    /// unwriting the rewind first and consuming the whole span.
    fn emit_link(&mut self, rewind: usize, take: usize, url: String) {
        if rewind > 0 {
            self.unput(rewind);
        }
        let text = self.parser.input[self.parser.pos - rewind..self.parser.pos + take].to_string();
        let link = self.parser.alloc(Kind::Link {
            url,
            title: None,
            image: false,
            reference: None,
        });
        let child = self.parser.alloc(Kind::Text(text));
        {
            let (start, end) = (
                (self.parser.pos - rewind) as u32,
                (self.parser.pos + take) as u32,
            );
            let lc = &mut self.parser.pool[link as usize];
            lc.start = start;
            lc.end = end;
            let cc = &mut self.parser.pool[child as usize];
            cc.start = start;
            cc.end = end;
        }
        self.parser.pool[link as usize].first_child = Some(child);
        self.parser.pool[link as usize].last_child = Some(child);
        self.parser.pool[child as usize].parent = Some(link);
        // Append the link to the open list after whatever precedes it.
        let id = link;
        self.parser.pool[id as usize].prev = self.parser.tail;
        if let Some(prev) = self.parser.tail {
            self.parser.pool[prev as usize].next = Some(id);
        } else {
            self.parser.head = Some(id);
        }
        self.parser.tail = Some(id);
        self.parser.pos += take;
    }
}

impl<'defs, 'a, 'ext> Parser<'defs, 'a, 'ext> {
    /// The main scan loop.
    fn run(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'\n' | b'\r' => self.handle_newline(),
                b'`' => self.handle_backticks(),
                b'\\' => self.handle_backslash(),
                b'&' => self.handle_entity(),
                b'<' => self.handle_pointy_brace(),
                b'*' | b'_' => self.handle_delim(self.bytes[self.pos]),
                b'[' => self.handle_open_bracket(),
                b'!' => {
                    if self.bytes.get(self.pos + 1) == Some(&b'[') {
                        // The image marker stays a text node of its own,
                        // as the reference keeps it: a footnote claim
                        // leaves it behind, a matched link removes it.
                        let bang =
                            self.push_spanned(Kind::Text("!".to_string()), self.pos, self.pos + 1);
                        self.pos += 1;
                        let id =
                            self.push_spanned(Kind::Text("[".to_string()), self.pos, self.pos + 1);
                        self.pos += 1;
                        self.push_bracket(true, id, Some(bang));
                    } else {
                        self.pos += 1;
                        self.push_spanned(Kind::Text("!".to_string()), self.pos - 1, self.pos);
                    }
                }
                b']' => self.handle_close_bracket(),
                _ => self.handle_text_or_trigger(),
            }
        }
        self.process_emphasis(0);
    }

    /// Either hands the cursor to the extensions whose trigger byte is
    /// here, or takes the byte as literal text.
    fn handle_text_or_trigger(&mut self) {
        let byte = self.bytes[self.pos];
        if self.triggers.is_empty() {
            self.handle_text();
            return;
        }
        let hits: Vec<usize> = self
            .triggers
            .iter()
            .filter(|(trigger, _)| *trigger == byte)
            .map(|(_, index)| *index)
            .collect();
        if hits.is_empty() {
            self.handle_text();
            return;
        }
        for index in hits {
            let before = self.pos;
            let ext =
                std::sync::Arc::clone(&self.extensions.expect("triggers imply extensions")[index]);
            let mut ctx = crate::extension::InlineTriggerCtx {
                inner: TriggerInner {
                    byte,
                    pending: None,
                    parser: self,
                },
            };
            ext.inline_trigger(&mut ctx);
            // A hook that leaves the cursor at the trigger byte took
            // nothing; the byte is literal text.
            if self.pos == before {
                self.handle_text();
                return;
            }
        }
    }

    /// Allocates a cell and appends it to the output list.
    fn push(&mut self, kind: Kind) -> u32 {
        let id = self.pool.len() as u32;
        self.pool.push(Cell {
            alive: true,
            kind,
            prev: self.tail,
            next: None,
            first_child: None,
            last_child: None,
            parent: None,
            start: 0,
            end: 0,
        });
        if let Some(prev) = self.tail {
            self.pool[prev as usize].next = Some(id);
        } else {
            self.head = Some(id);
        }
        self.tail = Some(id);
        id
    }

    /// Allocates a cell with its byte range in the inline text and
    /// appends it to the output list.
    fn push_spanned(&mut self, kind: Kind, start: usize, end: usize) -> u32 {
        let id = self.push(kind);
        let cell = &mut self.pool[id as usize];
        cell.start = start as u32;
        cell.end = end as u32;
        id
    }

    /// Allocates a detached container cell (not in any list yet).
    fn alloc(&mut self, kind: Kind) -> u32 {
        let id = self.pool.len() as u32;
        self.pool.push(Cell {
            alive: true,
            kind,
            prev: None,
            next: None,
            first_child: None,
            last_child: None,
            parent: None,
            start: 0,
            end: 0,
        });
        id
    }

    /// Inserts an allocated container before `at` in its list.
    fn insert_before(&mut self, id: u32, at: u32) {
        let prev = self.pool[at as usize].prev;
        self.pool[id as usize].prev = prev;
        self.pool[id as usize].next = Some(at);
        self.pool[at as usize].prev = Some(id);
        match prev {
            Some(p) => self.pool[p as usize].next = Some(id),
            None => {
                if self.head == Some(at) {
                    self.head = Some(id);
                }
            }
        }
    }

    /// Removes a cell from its list without freeing it. The container
    /// holding this cell has its child endpoints repaired through the
    /// cell's parent pointer, in constant time.
    fn unlink(&mut self, id: u32) {
        let (prev, next, parent) = {
            let cell = &self.pool[id as usize];
            (cell.prev, cell.next, cell.parent)
        };
        if let Some(p) = parent {
            let container = &mut self.pool[p as usize];
            if container.first_child == Some(id) {
                container.first_child = next;
            }
            if container.last_child == Some(id) {
                container.last_child = prev;
            }
        }
        if let Some(p) = prev {
            self.pool[p as usize].next = next;
        } else if self.head == Some(id) {
            self.head = next;
        }
        if let Some(n) = next {
            self.pool[n as usize].prev = prev;
        } else if self.tail == Some(id) {
            self.tail = prev;
        }
        let cell = &mut self.pool[id as usize];
        cell.prev = None;
        cell.next = None;
    }

    /// Marks a cell dead and drops it from its list. `unlink` already
    /// repaired any container that held it as a child endpoint.
    fn kill(&mut self, id: u32) {
        self.unlink(id);
        self.pool[id as usize].alive = false;
    }

    /// Removes every delimiter run opened after `position`, the way
    /// the reference parser cleans up when a bracket group is claimed:
    /// the runs inside the group never pair with anything after it.
    fn remove_delimiters_since(&mut self, position: usize) {
        let mut id = self.delims.last;
        while let Some(cur) = id {
            let (prev, cell_position) = {
                let cell = &self.delims.cells[cur as usize];
                (cell.prev, cell.position)
            };
            if cell_position <= position {
                break;
            }
            self.delims.remove(cur);
            id = prev;
        }
    }

    /// Pushes a bracket onto the bracket stack.
    fn push_bracket(&mut self, image: bool, node: u32, bang: Option<u32>) {
        if let Some(last) = self.brackets.last_mut() {
            last.bracket_after = true;
        }
        // A bracket opening exactly where a failed attempt rewound is
        // that attempt's own retry, not a second mistake: flag it and
        // consume the marker. Anything opening elsewhere clears it.
        // `self.pos` sits just past the `[` here, so the `[` right
        // after the rewind matches `rescue + 1`.
        let rescued = self.rescue_position.take() == Some(self.pos - 1);
        self.brackets.push(Bracket {
            image,
            node,
            bang,
            position: self.pos,
            bracket_after: false,
            rescued,
        });
        if !image {
            self.no_link_openers = false;
        }
    }

    /// Scans plain text up to the next special character.
    fn handle_text(&mut self) {
        let start = self.pos;
        let mut end = self.pos + 1;
        while end < self.bytes.len()
            && !SPECIAL_CHARS[self.bytes[end] as usize]
            && (self.triggers.is_empty() || !self.trigger_bytes.contains(&self.bytes[end]))
        {
            end += 1;
        }
        let mut text = self.input[start..end].to_string();
        // A text run ending at a newline loses its trailing whitespace,
        // as the reference's newline handling does; the spec's whitespace
        // definition includes vertical tab and form feed.
        if end < self.bytes.len() && matches!(self.bytes[end], b'\n' | b'\r') {
            while matches!(
                text.as_bytes().last(),
                Some(b' ') | Some(b'\t') | Some(b'\x0B') | Some(b'\x0C')
            ) {
                text.pop();
            }
        }
        let text_len = text.len();
        self.pos = end;
        // A run that was all trailing whitespace contributes nothing; its
        // bytes belong to no node, so the following break or text keeps
        // the reference's span.
        if text_len == 0 {
            return;
        }
        self.push_spanned(Kind::Text(text), start, start + text_len);
    }

    /// Handles a line break inside the block's text.
    fn handle_newline(&mut self) {
        let nlpos = self.pos;
        if self.bytes[nlpos] == b'\r' {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
        }
        let nl_end = self.pos;
        let hard = nlpos >= 2 && self.bytes[nlpos - 1] == b' ' && self.bytes[nlpos - 2] == b' ';
        // The break cell covers the trailing whitespace the preceding
        // text run dropped plus the line ending itself — for a hard break,
        // whose two spaces are its own syntax. A soft break's dropped
        // whitespace belongs to nobody: the cell starts at the line
        // ending, so text merged across it starts there too.
        let mut bstart = nlpos;
        if hard {
            while bstart > 0 && matches!(self.bytes[bstart - 1], b' ' | b'\t' | b'\x0B' | b'\x0C') {
                bstart -= 1;
            }
        }
        self.push_spanned(
            if hard {
                Kind::HardBreak
            } else {
                Kind::SoftBreak
            },
            bstart,
            nl_end,
        );
        // Leading whitespace of the next line does not belong to the text.
        while matches!(self.bytes.get(self.pos), Some(b' ') | Some(b'\t')) {
            self.pos += 1;
        }
    }

    /// Handles a backslash escape, hard break or literal backslash.
    fn handle_backslash(&mut self) {
        self.pos += 1;
        match self.bytes.get(self.pos).copied() {
            Some(next) if is_ascii_punct_byte(next) => {
                self.pos += 1;
                // The escaped character sits in a wrapper of its own,
                // matching the reference's node for an escape: hooks
                // reading the sibling chain see the wrapper, not text.
                let esc = self.alloc(Kind::Escaped);
                let text = self.alloc(Kind::Text((next as char).to_string()));
                {
                    let cell = &mut self.pool[esc as usize];
                    cell.start = (self.pos - 2) as u32;
                    cell.end = self.pos as u32;
                }
                {
                    let cell = &mut self.pool[text as usize];
                    // The character's span covers its backslash too: the
                    // wrapper dissolves, and the surviving text node's
                    // position then covers the escape as the source wrote
                    // it, as the reference's text chunk does.
                    cell.start = (self.pos - 2) as u32;
                    cell.end = self.pos as u32;
                }
                self.pool[text as usize].parent = Some(esc);
                self.pool[esc as usize].first_child = Some(text);
                self.pool[esc as usize].last_child = Some(text);
                self.append(esc);
            }
            Some(b'\r') | Some(b'\n') => {
                let bs = (self.pos - 1) as u32;
                let nl_end = self.handle_newline_raw() as u32;
                self.push_spanned(Kind::HardBreak, bs as usize, nl_end as usize);
            }
            _ => {
                self.push_spanned(Kind::Text("\\".to_string()), self.pos - 1, self.pos);
            }
        }
    }

    /// Consumes a line ending at the cursor for a backslash break.
    /// Returns the position just past the line ending, before any
    /// leading whitespace of the next line.
    fn handle_newline_raw(&mut self) -> usize {
        if self.bytes.get(self.pos) == Some(&b'\r') {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
        }
        let nl_end = self.pos;
        while matches!(self.bytes.get(self.pos), Some(b' ') | Some(b'\t')) {
            self.pos += 1;
        }
        nl_end
    }

    /// Handles a code span or a run of literal backticks, following the
    /// reference parser: scan forward for a closer of exactly the opener's
    /// length, remembering the position of every run passed on the way. If
    /// no closer exists, remember that fact and treat the opener as literal.
    fn handle_backticks(&mut self) {
        let mut numticks = 0usize;
        while self.bytes.get(self.pos) == Some(&b'`') {
            self.pos += 1;
            numticks += 1;
        }
        let startpos = self.pos;
        // The reference parser's optimistic check: only after a scan has
        // run to the end without a closer does the memo prove absence.
        // The generation makes entries from earlier subjects read as
        // zero — the fresh-table state — without re-zeroing the buffer.
        let optimistic_fail = if self.scanned_for_backticks && numticks <= MAXBACKTICKS {
            let entry = self.backticks_memo.get(numticks).copied().unwrap_or(0);
            let last_seen = if entry >> 40 == self.memo_gen {
                (entry & MEMO_POS_MASK) as usize
            } else {
                0
            };
            last_seen <= startpos
        } else {
            false
        };
        if numticks > MAXBACKTICKS || optimistic_fail {
            let tstart = self.pos - numticks;
            self.push_spanned(Kind::Text("`".repeat(numticks)), tstart, self.pos);
            return;
        }
        match self.scan_to_closing_backticks(numticks) {
            Some(closer) => {
                // The cell keeps the code span's raw source content — line
                // endings and edge whitespace included — because the mdast
                // export's `value` is the raw text; the renderers normalize
                // per CommonMark when they emit.
                let content = &self.input[startpos..closer - numticks];
                self.pos = closer;
                self.push_spanned(Kind::Code(content.to_string()), startpos - numticks, closer);
            }
            None => {
                self.scanned_for_backticks = true;
                self.pos = startpos;
                self.push_spanned(
                    Kind::Text("`".repeat(numticks)),
                    startpos - numticks,
                    startpos,
                );
            }
        }
    }

    /// Scans forward for a backtick run of exactly `length` backticks.
    /// Returns the position just past the closing run, or `None`. Records
    /// every run it passes in the memo table, exactly as the reference
    /// scanner does — including on the way to a successful match.
    fn scan_to_closing_backticks(&mut self, length: usize) -> Option<usize> {
        let mut pos = self.pos;
        while pos < self.bytes.len() {
            if self.bytes[pos] == b'`' {
                let run_start = pos;
                while self.bytes.get(pos) == Some(&b'`') {
                    pos += 1;
                }
                let numticks = pos - run_start;
                if numticks <= MAXBACKTICKS {
                    if self.backticks_memo.len() <= numticks {
                        self.backticks_memo.resize(numticks + 1, 0);
                    }
                    self.backticks_memo[numticks] =
                        (self.memo_gen << 40) | (run_start as u64 & MEMO_POS_MASK);
                }
                if numticks == length {
                    self.pos = pos;
                    return Some(pos);
                }
            } else {
                pos += 1;
            }
        }
        None
    }

    /// Handles an entity reference or a literal ampersand.
    fn handle_entity(&mut self) {
        self.pos += 1;
        let amp = self.pos - 1;
        match entities::scan_entity(&self.input[self.pos..]) {
            Some((text, len)) => {
                self.pos += len;
                self.push_spanned(Kind::Text(text), amp, self.pos);
            }
            None => {
                self.push_spanned(Kind::Text("&".to_string()), amp, self.pos);
            }
        }
    }

    /// Scans an emphasis delimiter run and records it.
    fn handle_delim(&mut self, ch: u8) {
        let (numdelims, can_open, can_close) = self.scan_delims(ch);
        let text = self.input[self.pos - numdelims..self.pos].to_string();
        let id = self.push_spanned(Kind::Text(text), self.pos - numdelims, self.pos);
        if can_open || can_close {
            self.delims.push(DelimCell {
                alive: true,
                ch,
                can_open,
                can_close,
                length: numdelims,
                // The position after the run, as in the reference parser.
                position: self.pos,
                node: id,
                ext_kind: None,
                prev: None,
                next: None,
            });
        }
    }

    /// Measures a delimiter run at the cursor, applying the flanking rules.
    ///
    /// Extension trigger bytes are transparent when measuring the
    /// characters around a run, as in the reference implementations'
    /// skip-character walk: in `a\~~b` the run's preceding character is
    /// `a`, not the escaped tilde.
    fn scan_delims(&mut self, ch: u8) -> (usize, bool, bool) {
        let before_char = self.char_before(self.pos);
        let mut numdelims = 0usize;
        while self.bytes.get(self.pos) == Some(&ch) {
            self.pos += 1;
            numdelims += 1;
        }
        let after_char = self.char_at(self.pos);
        let left_flanking = numdelims > 0
            && !after_char.is_some_and(is_space)
            && (!after_char.is_some_and(is_punct_or_symbol)
                || before_char.is_some_and(is_space)
                || before_char.is_some_and(is_punct_or_symbol));
        let right_flanking = numdelims > 0
            && !before_char.is_some_and(is_space)
            && (!before_char.is_some_and(is_punct_or_symbol)
                || after_char.is_some_and(is_space)
                || after_char.is_some_and(is_punct_or_symbol));
        let (can_open, can_close) = if ch == b'_' {
            (
                left_flanking && (!right_flanking || before_char.is_some_and(is_punct_or_symbol)),
                right_flanking && (!left_flanking || after_char.is_some_and(is_punct_or_symbol)),
            )
        } else {
            (left_flanking, right_flanking)
        };
        (numdelims, can_open, can_close)
    }

    /// Whether a byte is transparent in the flanking walk: a UTF-8
    /// continuation byte, or an extension trigger byte on the
    /// extension path.
    fn is_walk_through(&self, byte: u8) -> bool {
        byte & 0xC0 == 0x80
            || (!self.trigger_bytes.is_empty() && self.trigger_bytes.contains(&byte))
    }

    /// The character before `pos`, or a newline stand-in at the start.
    ///
    /// The walk steps over transparent bytes; when it reaches the
    /// subject's start through them, the before-character is the
    /// newline stand-in, as in the reference implementation.
    fn char_before(&self, pos: usize) -> Option<char> {
        if pos == 0 {
            return Some('\n');
        }
        let bytes = &self.bytes[..pos];
        // Step back over transparent bytes.
        let mut start = pos - 1;
        while start > 0 && self.is_walk_through(bytes[start]) {
            start -= 1;
        }
        if self.is_walk_through(bytes[start]) {
            return Some('\n');
        }
        std::str::from_utf8(&bytes[start..])
            .ok()
            .and_then(|s| s.chars().next())
            .or(Some('\n'))
    }

    /// The character at `pos`, or a newline stand-in at the end of the
    /// input, as the reference parser's decoder yields. Transparent
    /// bytes are stepped over first.
    fn char_at(&self, pos: usize) -> Option<char> {
        let mut pos = pos;
        while pos < self.bytes.len() && self.is_walk_through(self.bytes[pos]) {
            pos += 1;
        }
        if pos >= self.bytes.len() {
            return Some('\n');
        }
        let bytes = &self.bytes[pos..];
        let len = utf8_len(bytes[0]);
        std::str::from_utf8(&bytes[..len.min(bytes.len())])
            .ok()
            .and_then(|s| s.chars().next())
            .or(Some('\n'))
    }

    /// Opens a link or image bracket.
    fn handle_open_bracket(&mut self) {
        let id = self.push_spanned(Kind::Text("[".to_string()), self.pos, self.pos + 1);
        self.pos += 1;
        self.push_bracket(false, id, None);
    }

    /// Closes a bracket: an inline link, a reference link, or plain text.
    fn handle_close_bracket(&mut self) {
        self.pos += 1;
        let initial_pos = self.pos;
        let Some(opener_index) = (self.brackets.len()).checked_sub(1) else {
            self.push_spanned(Kind::Text("]".to_string()), initial_pos - 1, initial_pos);
            return;
        };
        let is_image = self.brackets[opener_index].image;
        if !is_image && self.no_link_openers {
            self.brackets.pop();
            self.push_spanned(Kind::Text("]".to_string()), initial_pos - 1, initial_pos);
            return;
        }
        let after_link_text_pos = self.pos;

        // An inline link: `(url "title")`.
        let mut matched: Option<(String, Option<String>)> = None;
        if self.bytes.get(self.pos) == Some(&b'(') {
            let after_paren = self.pos + 1;
            let sps = scan_spacechars(self.bytes, after_paren);
            let url_scan_pos = after_paren + sps;
            if let Some((total, url_start, url_len)) = scan_link_url(self.bytes, url_scan_pos) {
                let endurl = url_scan_pos + total;
                let starttitle = endurl + scan_spacechars(self.bytes, endurl);
                // A title must be separated from the destination by spaces.
                let endtitle = if starttitle == endurl {
                    starttitle
                } else {
                    starttitle + scan_link_title(self.bytes, starttitle).unwrap_or(0)
                };
                let endall = endtitle + scan_spacechars(self.bytes, endtitle);
                if self.bytes.get(endall) == Some(&b')') {
                    let title_raw = if endtitle > starttitle {
                        Some(&self.input[starttitle..endtitle])
                    } else {
                        None
                    };
                    matched = Some((
                        clean_url(&self.input[url_start..url_start + url_len]),
                        title_raw.and_then(clean_title),
                    ));
                    self.pos = endall + 1;
                }
            }
        }

        // The reference-style label, when the inline form failed and a
        // lookup was attempted; kept for the problem message when
        // nothing matched.
        let mut broken_label: Option<String> = None;
        // The reference provenance, when the link resolved through a
        // definition: the exported trees show `linkReference` nodes for
        // these, with the label as written and the reference form the
        // source used.
        let mut reference: Option<crate::node::LinkRef<'static>> = None;
        if matched.is_none() {
            // A reference link: a following label or the link text itself.
            self.pos = after_link_text_pos;
            let mut scanned_label: Option<String> = None;
            if let Some((label, len)) = scan_link_label(self.bytes, self.pos) {
                self.pos += len;
                scanned_label = Some(label);
            } else {
                self.pos = initial_pos;
            }
            let shortcut = scanned_label.as_ref().is_none_or(String::is_empty)
                && !self.brackets[opener_index].bracket_after;
            let mut raw_label = scanned_label.clone();
            if shortcut {
                // The link text itself becomes the label.
                let text_start = self.brackets[opener_index].position;
                raw_label = Some(self.input[text_start..initial_pos - 1].to_string());
            }
            if let Some(label) = &raw_label {
                self.label_scratch.clear();
                crate::parser::normalize_label_into(label, &mut self.label_scratch);
                if let Some(def) = self.link_defs.get(self.label_scratch.as_str()) {
                    matched = Some((
                        def.destination.to_string(),
                        def.title.as_ref().map(|t| t.to_string()),
                    ));
                    // The scanned label decides the form: a non-empty
                    // one is a full reference, an empty one a collapsed
                    // reference, none a shortcut reference.
                    let kind = match &scanned_label {
                        Some(label) if !label.is_empty() => crate::node::LinkRefKind::Full,
                        Some(_) => crate::node::LinkRefKind::Collapsed,
                        None => crate::node::LinkRefKind::Shortcut,
                    };
                    reference = Some(crate::node::LinkRef {
                        label: raw_label.clone().unwrap_or_default().into(),
                        kind,
                    });
                }
            }
            broken_label = raw_label;
        }

        match matched {
            Some((url, title)) => {
                let opener_node = self.brackets[opener_index].node;
                let opener_position = self.brackets[opener_index].position;
                let link_id = self.alloc(Kind::Link {
                    url,
                    title,
                    image: is_image,
                    reference,
                });
                // The link covers the whole group: the image marker and
                // the opening bracket when there is one, through the
                // closing `)` or the reference label. The bracket stack
                // records the position just past the `[`.
                {
                    let start = if is_image {
                        opener_position.saturating_sub(2)
                    } else {
                        opener_position.saturating_sub(1)
                    };
                    let cell = &mut self.pool[link_id as usize];
                    cell.start = start as u32;
                    cell.end = self.pos as u32;
                }
                // Move every node after the opener's text node into the
                // link, as the reference parser does.
                let first = self.pool[opener_node as usize].next;
                let last = self.tail;
                if let Some(f) = first {
                    self.pool[link_id as usize].first_child = Some(f);
                    self.pool[link_id as usize].last_child = last;
                    self.pool[f as usize].prev = None;
                    let mut id = Some(f);
                    while let Some(current) = id {
                        self.pool[current as usize].parent = Some(link_id);
                        id = self.pool[current as usize].next;
                    }
                    if let Some(l) = last {
                        self.pool[l as usize].next = None;
                    }
                }
                self.pool[opener_node as usize].next = None;
                self.tail = Some(opener_node);
                self.insert_before(link_id, opener_node);
                self.kill(opener_node);
                // A matched image takes its marker with it.
                if let Some(bang) = self.brackets[opener_index].bang {
                    self.kill(bang);
                }
                self.process_emphasis(opener_position);
                self.brackets.pop();
                if !is_image {
                    self.no_link_openers = true;
                }
            }
            None => {
                // An extension may claim the group as one inline node.
                // The hooks materialize the group's text lazily —
                // through content_starts_with first — so the plain
                // path and bracket-heavy input stay linear.
                let opener_node = self.brackets[opener_index].node;
                let opener_position = self.brackets[opener_index].position;
                let mut claimed: Option<BracketClaim> = None;
                if let Some(extensions) = self.extensions {
                    let inner = BracketInner {
                        parser: &mut *self,
                        opener_node,
                        opener_position,
                        content: None,
                        claimed: None,
                    };
                    let mut ctx = crate::extension::InlineBracketCtx { inner };
                    for extension in extensions {
                        extension.inline_bracket(&mut ctx);
                    }
                    claimed = ctx.inner.claimed;
                }
                if let Some((kind, attrs, end)) = claimed {
                    self.pos = initial_pos;
                    let ext_id = self.alloc(Kind::Ext { kind, attrs });
                    // The node covers the bracket group up to the
                    // content bytes the hook consumed. A claim's real
                    // end arrives from the walk below; this stays
                    // zero-width until then.
                    {
                        let cell = &mut self.pool[ext_id as usize];
                        cell.start = opener_position as u32;
                        cell.end = match end {
                            None => initial_pos as u32,
                            Some(_) => opener_position as u32,
                        };
                    }
                    self.insert_before(ext_id, opener_node);
                    match end {
                        // The marker-node claim: the group's content is
                        // consumed, the node stays empty.
                        None => {
                            let mut id = self.pool[opener_node as usize].next;
                            while let Some(cur) = id {
                                let next = self.pool[cur as usize].next;
                                self.kill(cur);
                                id = next;
                            }
                            self.tail = Some(ext_id);
                        }
                        // The world's inline-bracket decision: the
                        // content's bytes up to `end` become the node's
                        // inline children; the rest stays literal text
                        // after the node. The content cells are the
                        // plain text the hook saw, so byte offsets
                        // accumulate over their text.
                        Some(end) => {
                            let mut kept: Vec<u32> = Vec::new();
                            let mut literal_first: Option<u32> = None;
                            let mut consumed = 0usize;
                            let mut id = self.pool[opener_node as usize].next;
                            while let Some(cur) = id {
                                id = self.pool[cur as usize].next;
                                if literal_first.is_some() {
                                    continue;
                                }
                                let text = match &self.pool[cur as usize].kind {
                                    Kind::Text(t) | Kind::Html(t) => t,
                                    // Unreachable through the world — a
                                    // decision needs `content`, which is
                                    // `None` then — but stay safe: the
                                    // cell and the rest stay literal.
                                    _ => {
                                        literal_first = Some(cur);
                                        continue;
                                    }
                                };
                                if consumed >= end {
                                    literal_first = Some(cur);
                                    continue;
                                }
                                let len = text.len();
                                if consumed + len <= end {
                                    kept.push(cur);
                                    consumed += len;
                                    continue;
                                }
                                // The cell straddles `end`: split it.
                                // A cut inside a character rounds down,
                                // so both halves stay valid UTF-8 no
                                // matter what `end` named.
                                let (head, rest, cut) = {
                                    let text = match &self.pool[cur as usize].kind {
                                        Kind::Text(t) | Kind::Html(t) => t,
                                        _ => unreachable!("handled above"),
                                    };
                                    let mut cut = end - consumed;
                                    if cut > text.len() {
                                        cut = text.len();
                                    }
                                    while cut > 0 && !text.is_char_boundary(cut) {
                                        cut -= 1;
                                    }
                                    (text[..cut].to_owned(), text[cut..].to_owned(), cut)
                                };
                                self.pool[cur as usize].kind = Kind::Text(head);
                                {
                                    let cell = &mut self.pool[cur as usize];
                                    cell.end = cell.start + cut as u32;
                                }
                                kept.push(cur);
                                consumed = end;
                                if !rest.is_empty() {
                                    let rest_len = rest.len() as u32;
                                    let rest_id = self.alloc(Kind::Text(rest));
                                    {
                                        let head_end = self.pool[cur as usize].end;
                                        let rc = &mut self.pool[rest_id as usize];
                                        rc.start = head_end;
                                        rc.end = head_end + rest_len;
                                    }
                                    let cur_next = self.pool[cur as usize].next;
                                    self.pool[cur as usize].next = Some(rest_id);
                                    self.pool[rest_id as usize].prev = Some(cur);
                                    self.pool[rest_id as usize].next = cur_next;
                                    match cur_next {
                                        Some(n) => self.pool[n as usize].prev = Some(rest_id),
                                        None => self.tail = Some(rest_id),
                                    }
                                    literal_first = Some(rest_id);
                                }
                            }
                            // The opener dies before the rewiring below:
                            // killing it splices it out of the root
                            // chain, which would otherwise overwrite
                            // the `ext.next` the adoption sets.
                            self.kill(opener_node);
                            if let Some(&first_kept) = kept.first() {
                                let last_kept = kept[kept.len() - 1];
                                self.pool[ext_id as usize].first_child = Some(first_kept);
                                self.pool[ext_id as usize].last_child = Some(last_kept);
                                self.pool[first_kept as usize].prev = None;
                                self.pool[last_kept as usize].next = None;
                                for &cell in &kept {
                                    self.pool[cell as usize].parent = Some(ext_id);
                                }
                                // The claimed cells leave the root chain
                                // with the node: the literal remainder
                                // follows it instead. Left in the root
                                // chain as well, the cells would stay
                                // reachable twice and the tree walk
                                // would visit them a second time as
                                // empty husks under the paragraph.
                                match literal_first {
                                    Some(l) => {
                                        self.pool[ext_id as usize].next = Some(l);
                                        self.pool[l as usize].prev = Some(ext_id);
                                    }
                                    None => {
                                        self.pool[ext_id as usize].next = None;
                                        self.tail = Some(ext_id);
                                    }
                                }
                            }
                            // The node spans exactly the bytes its hook
                            // kept: the last kept cell's end, or zero
                            // width at the content start when nothing
                            // was kept. The `end` the hook named is the
                            // cut it asked for, not the byte the walk
                            // actually reached.
                            let kept_end = match kept.last() {
                                Some(&last) => self.pool[last as usize].end,
                                None => opener_position as u32,
                            };
                            self.pool[ext_id as usize].end = kept_end;
                        }
                    }
                    self.remove_delimiters_since(opener_position);
                    self.brackets.pop();
                } else {
                    // A retried group — the parser's own re-scan after
                    // the rewind below — has already had its author
                    // mistake reported by the attempt it retries; its
                    // failure records nothing new.
                    let retried = self.brackets[opener_index].rescued;
                    self.brackets.pop();
                    self.pos = initial_pos;
                    self.push_spanned(Kind::Text("]".to_string()), initial_pos - 1, initial_pos);
                    if !retried {
                        // The group matched nothing: no inline link, no
                        // reference definition, no extension claim. The
                        // output stays exactly the literal text
                        // CommonMark asks for, and the broken reference
                        // is recorded as a problem for checkers to
                        // surface. The rewind re-scans the text after
                        // the literal `]`; the marker arms so the
                        // bracket opening at the rewind point — the
                        // parser's retry of this same mistake — stays
                        // silent, while anything further away is
                        // reported as the fresh mistake it is.
                        self.rescue_position = Some(initial_pos);
                        let message = match broken_label.filter(|label| !label.is_empty()) {
                            Some(label) => {
                                format!("no link reference definition for '{label}'")
                            }
                            None => {
                                String::from("no link reference definition for the bracket group")
                            }
                        };
                        self.problems
                            .push(crate::problems::Problem::new(message, self.block_span));
                    }
                }
            }
        }
    }

    /// Resolves the delimiter stack from `stack_bottom` onward.
    ///
    /// The built-in emphasis delimiters follow the CommonMark 0.31.2
    /// reference's openers-bottom indexing; extension delimiters follow
    /// the reference implementations' `[length % 3][char]` indexing and
    /// the same odd-match rule, which is what makes `~a~~b~` wrap
    /// around the middle run.
    fn process_emphasis(&mut self, stack_bottom: usize) {
        let mut openers_bottom = [stack_bottom; 15];
        let mut ext_openers_bottom = [[stack_bottom; 256]; 3];
        // Move back to the first relevant delimiter: the last one whose
        // position is at or after the stack bottom.
        let mut closer: Option<u32> = None;
        let mut candidate = self.delims.last;
        while let Some(id) = candidate {
            let cell = &self.delims.cells[id as usize];
            if cell.position < stack_bottom {
                break;
            }
            closer = Some(id);
            candidate = cell.prev;
        }
        // Now move forward, looking for closers and handling each.
        while let Some(closer_id) = closer {
            let closer_cell = &self.delims.cells[closer_id as usize];
            let (closer_can_close, closer_ch, closer_can_open, closer_len, closer_position) = (
                closer_cell.can_close,
                closer_cell.ch,
                closer_cell.can_open,
                closer_cell.length,
                closer_cell.position,
            );
            let closer_ext_kind = closer_cell.ext_kind.clone();
            let next_of = |parser: &Self, id: u32| parser.delims.cells[id as usize].next;
            if !closer_can_close {
                closer = next_of(self, closer_id);
                continue;
            }
            let openers_bottom_index = match closer_ch {
                b'"' => 0,
                b'\'' => 1,
                b'_' => 2 + (if closer_can_open { 3 } else { 0 }) + (closer_len % 3),
                b'*' => 8 + (if closer_can_open { 3 } else { 0 }) + (closer_len % 3),
                _ => 0,
            };
            let ext_bottom_len = closer_len % 3;
            let ext_bottom_char = closer_ch as usize;
            let is_ext = closer_ext_kind.is_some();
            // Look backwards for the first matching opener.
            let mut opener = self.delims.cells[closer_id as usize].prev;
            let mut opener_found = false;
            let mut opener_id = None;
            while let Some(o) = opener {
                let cell = &self.delims.cells[o as usize];
                let bottom = if is_ext {
                    ext_openers_bottom[ext_bottom_len][ext_bottom_char]
                } else {
                    openers_bottom[openers_bottom_index]
                };
                if cell.position < bottom {
                    break;
                }
                if cell.can_open
                    && cell.ch == closer_ch
                    && cell.ext_kind == closer_ext_kind
                    && (!(closer_can_open || cell.can_close)
                        || closer_len % 3 == 0
                        || (cell.length + closer_len) % 3 != 0)
                {
                    opener_found = true;
                    opener_id = Some(o);
                    break;
                }
                opener = cell.prev;
            }
            let old_closer = closer_id;
            if let Some(kind) = closer_ext_kind {
                if opener_found {
                    let opener_id = opener_id.expect("checked above");
                    closer = self.insert_extension(opener_id, closer_id, &kind);
                } else {
                    closer = next_of(self, closer_id);
                }
            } else if closer_ch == b'*' || closer_ch == b'_' {
                if opener_found {
                    let opener_id = opener_id.unwrap();
                    closer = self.insert_emphasis(opener_id, closer_id);
                } else {
                    closer = next_of(self, closer_id);
                }
            } else {
                closer = next_of(self, closer_id);
            }
            if !opener_found {
                if is_ext {
                    ext_openers_bottom[ext_bottom_len][ext_bottom_char] = closer_position;
                } else {
                    openers_bottom[openers_bottom_index] = closer_position;
                }
                if !closer_can_open {
                    self.delims.remove(old_closer);
                }
            }
        }
        // Free all delimiters from stack_bottom on.
        while let Some(id) = self.delims.last {
            if self.delims.cells[id as usize].position < stack_bottom {
                break;
            }
            self.delims.remove(id);
        }
    }

    /// Wraps the content between an extension's opener and closer runs
    /// in an extension node of `kind`, the way the reference
    /// implementations pair extension delimiters: the opener's whole
    /// text node becomes the node, the closer's whole text node
    /// disappears, and every delimiter between the two is freed. A
    /// mismatched pair — different run lengths — wraps nothing but
    /// still clears the whole range, so a failed match cannot rescan.
    /// Returns the delimiter to continue from.
    fn insert_extension(&mut self, opener: u32, closer: u32, kind: &str) -> Option<u32> {
        let opener_cell = &self.delims.cells[opener as usize];
        let opener_node = opener_cell.node;
        let closer_node = self.delims.cells[closer as usize].node;
        let opener_len = self.text_len(opener_node);
        let closer_len = self.text_len(closer_node);
        // The container covers the whole matched range, delimiters
        // included, captured before the closer's node is freed.
        let ext_start = self.pool[opener_node as usize].start;
        let ext_end = self.pool[closer_node as usize].end;
        let next_after_closer = self.delims.cells[closer as usize].next;
        if opener_len == closer_len {
            // Remove the delimiters between opener and closer.
            let mut between = self.delims.cells[closer as usize].prev;
            while let Some(id) = between {
                if id == opener {
                    break;
                }
                between = self.delims.cells[id as usize].prev;
                self.delims.remove(id);
            }
            // The opener's text node becomes the extension container.
            self.pool[opener_node as usize].kind = Kind::Ext {
                kind: kind.to_owned(),
                attrs: Vec::new(),
            };
            {
                let cell = &mut self.pool[opener_node as usize];
                cell.start = ext_start;
                cell.end = ext_end;
            }
            let first = self.pool[opener_node as usize].next;
            let last = self.pool[closer_node as usize].prev;
            if first.is_some() && first != Some(closer_node) {
                self.pool[opener_node as usize].first_child = first;
                self.pool[opener_node as usize].last_child = last;
                let mut id = first;
                while let Some(current) = id {
                    if current == closer_node {
                        break;
                    }
                    let next = self.pool[current as usize].next;
                    self.pool[current as usize].prev = None;
                    self.pool[current as usize].parent = Some(opener_node);
                    self.pool[current as usize].next = if next == Some(closer_node) {
                        None
                    } else {
                        next
                    };
                    id = next;
                }
            }
            // Splice through the closer's node and let `kill` unlink it:
            // its neighbors' pointers then come out right, exactly as in
            // `insert_emphasis`.
            self.pool[opener_node as usize].next = Some(closer_node);
            self.pool[closer_node as usize].prev = Some(opener_node);
            self.kill(closer_node);
            self.delims.remove(opener);
            self.delims.remove(closer);
        } else {
            // A failed match still clears the whole range, as in the
            // reference implementations.
            let mut delim = Some(closer);
            while let Some(id) = delim {
                if id == opener {
                    break;
                }
                delim = self.delims.cells[id as usize].prev;
                self.delims.remove(id);
            }
            self.delims.remove(opener);
        }
        next_after_closer
    }

    /// Splices one emphasis or strong node between an opener and a closer.
    /// Returns the delimiter to continue from.
    fn insert_emphasis(&mut self, opener: u32, closer: u32) -> Option<u32> {
        let opener_node = self.delims.cells[opener as usize].node;
        let closer_node = self.delims.cells[closer as usize].node;
        let opener_len = self.text_len(opener_node);
        let closer_len = self.text_len(closer_node);
        let use_delims = if closer_len >= 2 && opener_len >= 2 {
            2
        } else {
            1
        };
        // The emphasis's span is the delimiters it consumes: the opener
        // run's last `use_delims` bytes and the closer run's first
        // `use_delims` bytes. A run's leftover keeps the run's other end —
        // the opener's leftover is the run's head, the closer's leftover
        // its tail — so constructs sharing a run nest by position, the
        // later pair's delimiters enclosing the earlier pair's.
        let emph_start = self.pool[opener_node as usize].end - use_delims as u32;
        let emph_end = self.pool[closer_node as usize].start + use_delims as u32;
        let new_opener_len = opener_len - use_delims;
        let new_closer_len = closer_len - use_delims;
        self.set_text_len(opener_node, new_opener_len);
        {
            // The closer's leftover run is its tail: drop the consumed
            // bytes from the text's head and move the span's start past
            // them.
            let cell = &mut self.pool[closer_node as usize];
            if let Kind::Text(text) = &mut cell.kind {
                text.replace_range(..use_delims, "");
            }
            cell.start += use_delims as u32;
        }
        // Free the delimiters between opener and closer.
        let mut between = self.delims.cells[closer as usize].prev;
        while let Some(id) = between {
            if id == opener {
                break;
            }
            between = self.delims.cells[id as usize].prev;
            self.delims.remove(id);
        }
        let emph = self.alloc(if use_delims == 1 {
            Kind::Emph
        } else {
            Kind::Strong
        });
        {
            let cell = &mut self.pool[emph as usize];
            cell.start = emph_start;
            cell.end = emph_end;
        }
        // Move the nodes between the two text nodes into the emphasis.
        let first = self.pool[opener_node as usize].next;
        let last = self.pool[closer_node as usize].prev;
        let emph_parent = self.pool[opener_node as usize].parent;
        self.pool[emph as usize].parent = emph_parent;
        if first.is_some() && first != Some(closer_node) {
            self.pool[emph as usize].first_child = first;
            self.pool[emph as usize].last_child = last;
            let mut id = first;
            while let Some(current) = id {
                if current == closer_node {
                    break;
                }
                let next = self.pool[current as usize].next;
                self.pool[current as usize].prev = None;
                self.pool[current as usize].parent = Some(emph);
                self.pool[current as usize].next = if next == Some(closer_node) {
                    None
                } else {
                    next
                };
                id = next;
            }
            self.pool[opener_node as usize].next = Some(emph);
            self.pool[emph as usize].prev = Some(opener_node);
            self.pool[emph as usize].next = Some(closer_node);
            self.pool[closer_node as usize].prev = Some(emph);
        } else {
            self.pool[opener_node as usize].next = Some(emph);
            self.pool[emph as usize].prev = Some(opener_node);
            self.pool[emph as usize].next = Some(closer_node);
            self.pool[closer_node as usize].prev = Some(emph);
        }
        if new_opener_len == 0 {
            self.kill(opener_node);
            self.delims.remove(opener);
        }
        if new_closer_len == 0 {
            let next = self.delims.cells[closer as usize].next;
            self.kill(closer_node);
            self.delims.remove(closer);
            return next;
        }
        Some(closer)
    }

    /// A text cell's current length.
    fn text_len(&self, id: u32) -> usize {
        match &self.pool[id as usize].kind {
            Kind::Text(text) => text.len(),
            _ => 0,
        }
    }

    /// Shortens a text cell in place, keeping the first `len` bytes.
    fn set_text_len(&mut self, id: u32, len: usize) {
        if let Kind::Text(text) = &mut self.pool[id as usize].kind {
            text.truncate(len);
        }
        let cell = &mut self.pool[id as usize];
        cell.end = cell.start + len as u32;
    }

    /// Links an already-allocated cell at the end of the output list.
    fn append(&mut self, id: u32) {
        self.pool[id as usize].prev = self.tail;
        self.pool[id as usize].next = None;
        if let Some(prev) = self.tail {
            self.pool[prev as usize].next = Some(id);
        } else {
            self.head = Some(id);
        }
        self.tail = Some(id);
    }

    /// Handles `<...>`: autolinks, raw HTML, or a literal `<`.
    fn handle_pointy_brace(&mut self) {
        self.pos += 1; // past '<'
        let lt = self.pos - 1;
        if let Some(len) = scan_autolink_uri(self.bytes, self.pos) {
            let contents = self.input[self.pos..self.pos + len - 1].to_string();
            self.pos += len;
            let url = clean_autolink(&contents, false);
            let text = entities::unescape_html_f(&contents);
            self.append_autolink(url, text, lt, self.pos);
            return;
        }
        if let Some(len) = scan_autolink_email(self.bytes, self.pos) {
            let contents = self.input[self.pos..self.pos + len - 1].to_string();
            self.pos += len;
            let url = clean_autolink(&contents, true);
            let text = entities::unescape_html_f(&contents);
            self.append_autolink(url, text, lt, self.pos);
            return;
        }
        if let Some(len) = self.scan_inline_html() {
            let raw = self.input[self.pos - 1..self.pos - 1 + len].to_string();
            self.pos += len - 1;
            self.push_spanned(Kind::Html(raw), lt, self.pos);
        } else {
            self.push_spanned(Kind::Text("<".to_string()), lt, self.pos);
        }
    }

    /// Builds an autolink node with its single text child and appends it.
    /// The node's range is `[lt, end)`; the child covers the angle
    /// brackets' contents.
    fn append_autolink(&mut self, url: String, text: String, lt: usize, end: usize) {
        let link = self.alloc(Kind::Link {
            url,
            title: None,
            image: false,
            reference: None,
        });
        let text_id = self.alloc(Kind::Text(text));
        {
            let lc = &mut self.pool[link as usize];
            lc.start = lt as u32;
            lc.end = end as u32;
            let tc = &mut self.pool[text_id as usize];
            tc.start = (lt + 1) as u32;
            tc.end = (end - 1) as u32;
        }
        self.pool[text_id as usize].parent = Some(link);
        self.pool[link as usize].first_child = Some(text_id);
        self.pool[link as usize].last_child = Some(text_id);
        self.append(link);
    }

    /// Scans raw inline HTML starting at the byte after a `<`.
    /// Returns the total length including that `<`.
    fn scan_inline_html(&mut self) -> Option<usize> {
        let len = self.bytes.len();
        let &c = self.bytes.get(self.pos)?;
        let at = |i: usize| self.bytes.get(i).copied();
        // A construct of `total` bytes starting at the `<` (index `pos - 1`)
        // must fit inside the input.
        let fits = |total: usize| self.pos - 1 + total <= len;
        if c == b'!' {
            if at(self.pos + 1) == Some(b'-') && at(self.pos + 2) == Some(b'-') {
                // The two minimal comments HTML5 accepts, then full ones.
                if at(self.pos + 3) == Some(b'>') {
                    return Some(5);
                }
                if at(self.pos + 3) == Some(b'-') && at(self.pos + 4) == Some(b'>') {
                    return Some(6);
                }
                if !self.skip_comment {
                    if let Some(body) = scan_html_comment(self.bytes, self.pos + 3) {
                        let total = body + 7; // `<!--` + body + `-->`
                        if fits(total) {
                            return Some(total);
                        }
                    }
                    // No closing `-->` anywhere: later `<!--` scans would
                    // fail again, so remember that.
                    self.skip_comment = true;
                }
                return None;
            }
            if at(self.pos + 1) == Some(b'[') {
                if !self.skip_cdata {
                    if let Some(body) = scan_html_cdata(self.bytes, self.pos + 2) {
                        let total = body + 6; // `<![` + body + `]]>`
                        if fits(total) {
                            return Some(total);
                        }
                        self.skip_cdata = true;
                    }
                }
                return None;
            }
            if !self.skip_declaration {
                if let Some(body) = scan_html_declaration(self.bytes, self.pos + 1) {
                    let total = body + 3; // `<!` + body + `>`
                    if fits(total) {
                        return Some(total);
                    }
                    self.skip_declaration = true;
                }
            }
            return None;
        }
        if c == b'?' {
            if !self.skip_pi {
                // An empty processing instruction body is allowed.
                let body = scan_html_pi(self.bytes, self.pos + 1).unwrap_or(0);
                let total = body + 4; // `<?` + body + `?>`
                if fits(total) {
                    return Some(total);
                }
                self.skip_pi = true;
            }
            return None;
        }
        scan_html_tag(self.bytes, self.pos).map(|tag| tag + 1)
    }
}

/// The byte length of the UTF-8 sequence starting with `first`.
fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// The mdast value of a code span: the raw source content with one
/// leading and one trailing whitespace byte (a space or a line ending)
/// removed when both edges have one and the content is not all
/// whitespace. Line endings inside stay as the source wrote them.
pub fn code_span_value(content: &str) -> String {
    let bytes = content.as_bytes();
    let strip = |byte: u8| matches!(byte, b' ' | b'\n' | b'\r');
    let all_strip = bytes.iter().all(|byte| strip(*byte));
    if bytes.len() >= 2 && !all_strip && strip(bytes[0]) && strip(bytes[bytes.len() - 1]) {
        content[1..bytes.len() - 1].to_string()
    } else {
        content.to_string()
    }
}

/// Normalizes code span content for rendering: line endings become
/// spaces, and one leading and trailing space come off when the content
/// is not all spaces. Inline nodes carry the raw source content, so every
/// renderer that emits code span text applies this.
pub fn normalize_code(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut contains_nonspace = false;
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' | '\n' => out.push(' '),
            other => out.push(other),
        }
        if c != ' ' {
            contains_nonspace = true;
        }
    }
    if contains_nonspace && out.starts_with(' ') && out.ends_with(' ') && out.len() >= 2 {
        out.remove(0);
        out.pop();
    }
    out
}

/// Scans whitespace, including newlines. Returns the number of bytes.
fn scan_spacechars(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\x0B' | b'\x0C' | b'\r' | b'\n') {
        i += 1;
    }
    i - start
}

/// Scans a link destination. Returns the total bytes consumed (including
/// pointy brackets when present), the content's start and its length.
fn scan_link_url(bytes: &[u8], start: usize) -> Option<(usize, usize, usize)> {
    let len = bytes.len();
    let mut i = start;
    if bytes.get(i) == Some(&b'<') {
        i += 1;
        while i < len {
            match bytes[i] {
                b'>' => {
                    i += 1;
                    break;
                }
                b'\\' => i += 2,
                b'\n' | b'<' => return None,
                _ => i += 1,
            }
        }
        if i >= len {
            return None;
        }
        Some((i - start, start + 1, i - 2 - start))
    } else {
        // A bare destination: balanced parentheses, no unescaped spaces,
        // backslash escapes.
        let mut nb_parens = 0usize;
        while i < len {
            if bytes[i] == b'\\' && i + 1 < len && is_ascii_punct_byte(bytes[i + 1]) {
                i += 2;
            } else if bytes[i] == b'(' {
                nb_parens += 1;
                i += 1;
                if nb_parens > 32 {
                    return None;
                }
            } else if bytes[i] == b')' {
                if nb_parens == 0 {
                    break;
                }
                nb_parens -= 1;
                i += 1;
            } else if matches!(bytes[i], b' ' | b'\t' | b'\x0B' | b'\x0C' | b'\r' | b'\n') {
                if i == start {
                    return None;
                }
                break;
            } else {
                i += 1;
            }
        }
        if i >= len || nb_parens != 0 {
            return None;
        }
        Some((i - start, start, i - start))
    }
}

/// Scans a link title in single quotes, double quotes or parentheses.
/// Returns the number of bytes consumed, including the quotes.
fn scan_link_title(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let open = *bytes.get(start)?;
    let close = match open {
        b'"' => b'"',
        b'\'' => b'\'',
        b'(' => b')',
        _ => return None,
    };
    let mut i = start + 1;
    while i < len {
        if bytes[i] == b'\\' && i + 1 < len && is_ascii_punct_byte(bytes[i + 1]) {
            i += 2;
        } else if bytes[i] == close {
            return Some(i + 1 - start);
        } else if open == b'(' && bytes[i] == b'(' {
            return None;
        } else {
            i += 1;
        }
    }
    None
}

/// Scans a link label. Returns the raw label text and the number of bytes
/// consumed, including the brackets. Unescaped brackets are not allowed.
fn scan_link_label(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let len = bytes.len();
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut i = start + 1;
    let mut length = 0usize;
    while i < len {
        let c = bytes[i];
        if c == b'[' || c == b']' {
            break;
        }
        if c == b'\\' {
            i += 1;
            length += 1;
            if i < len && is_ascii_punct_byte(bytes[i]) {
                i += 1;
                length += 1;
            }
        } else {
            i += 1;
            length += 1;
        }
        if length > MAX_LINK_LABEL_LENGTH {
            return None;
        }
    }
    if bytes.get(i) == Some(&b']') {
        let label = trim_ascii_spaces(&bytes[start + 1..i]);
        return Some((label, i + 1 - start));
    }
    None
}

/// Trims ASCII whitespace from both ends of a byte slice.
fn trim_ascii_spaces(bytes: &[u8]) -> String {
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

/// Decodes every HTML entity in `text` using the full entity table.
pub(crate) fn unescape_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp + 1..];
        if let Some((decoded, len)) = entities::scan_entity(rest) {
            out.push_str(&decoded);
            rest = &rest[len..];
        } else {
            out.push('&');
        }
    }
    out.push_str(rest);
    out
}

/// Resolves a link label's escapes the way the reference reads labels: one
/// pass where a backslash before ASCII punctuation yields the character
/// and a character reference decodes, with the backslash winning where
/// both could apply. `a\&b` is `a&b`, `a&amp;b` is `a&b`, and a backslash
/// before a non-punctuation character stays literal.
pub(crate) fn decode_label(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if bytes.get(i + 1).is_some_and(|b| b.is_ascii_punctuation()) => {
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            b'&' => match entities::scan_entity(&raw[i + 1..]) {
                Some((decoded, len)) => {
                    out.push_str(&decoded);
                    i += 1 + len;
                }
                None => {
                    out.push('&');
                    i += 1;
                }
            },
            _ => {
                let ch = raw[i..].chars().next().expect("a byte inside a char");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// Cleans a link destination: trims whitespace, decodes entities, then
/// removes backslash escapes.
pub(crate) fn clean_url(url: &str) -> String {
    // The trim follows the reference's `cmark_isspace`, whose class is
    // the spec's whitespace definition: vertical tab and form feed are
    // whitespace and trim away.
    let trimmed =
        url.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\x0B' | '\x0C' | '\r' | '\n'));
    let decoded = entities::unescape_html_f(trimmed);
    unescape_backslashes(&decoded)
}

/// Cleans a link title: removes the surrounding quotes when present, then
/// decodes entities and removes backslash escapes.
pub(crate) fn clean_title(title: &str) -> Option<String> {
    let bytes = title.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let inner = match (bytes.first(), bytes.last()) {
        (Some(&b'\''), Some(&b'\'')) | (Some(&b'('), Some(&b')')) | (Some(&b'"'), Some(&b'"'))
            if bytes.len() >= 2 =>
        {
            &title[1..title.len() - 1]
        }
        _ => title,
    };
    let decoded = entities::unescape_html_f(inner);
    Some(unescape_backslashes(&decoded))
}

/// Cleans an autolink destination: trims, prepends `mailto:` for email
/// autolinks, then decodes entities.
fn clean_autolink(url: &str, is_email: bool) -> String {
    // The trim follows the reference's `cmark_isspace`, whose class is
    // the spec's whitespace definition: vertical tab and form feed are
    // whitespace and trim away.
    let trimmed =
        url.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\x0B' | '\x0C' | '\r' | '\n'));
    let mut out = String::new();
    if is_email {
        out.push_str("mailto:");
    }
    out.push_str(&entities::unescape_html_f(trimmed));
    out
}

/// Removes backslashes that escape ASCII punctuation.
fn unescape_backslashes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && is_ascii_punct_byte(bytes[i + 1]) {
            out.push(bytes[i + 1] as char);
            i += 2;
        } else {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] & 0xC0 == 0x80 {
                i += 1;
            }
            out.push_str(std::str::from_utf8(&bytes[start..i]).unwrap_or("\u{FFFD}"));
        }
    }
    out
}

/// Scans a URI autolink body (after `<`). Returns the length including
/// the closing `>`.
fn scan_autolink_uri(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    // scheme: one letter, then one to thirty-one letters, digits, dots,
    // pluses or minuses.
    if !bytes.get(i).is_some_and(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    i += 1;
    let mut scheme_len = 1usize;
    while i < len
        && scheme_len < 32
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'.' | b'+' | b'-'))
    {
        i += 1;
        scheme_len += 1;
    }
    if scheme_len < 2 || bytes.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    while i < len && !matches!(bytes[i], b'<' | b'>') && bytes[i] > 0x20 {
        i += 1;
    }
    if bytes.get(i) == Some(&b'>') {
        Some(i + 1 - start)
    } else {
        None
    }
}

/// Scans an email autolink body (after `<`). Returns the length including
/// the closing `>`.
fn scan_autolink_email(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let is_email_char = |b: u8| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'.' | b'!'
                    | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'/'
                    | b'='
                    | b'?'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'{'
                    | b'|'
                    | b'}'
                    | b'~'
                    | b'-'
            )
    };
    let mut i = start;
    while i < len && is_email_char(bytes[i]) {
        i += 1;
    }
    if i == start || bytes.get(i) != Some(&b'@') {
        return None;
    }
    i += 1;
    // One or more domain labels, each `label = [a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?`,
    // separated by dots.
    loop {
        i = scan_email_domain_label(bytes, i)?;
        if bytes.get(i) == Some(&b'.') {
            i += 1;
        } else {
            break;
        }
    }
    if bytes.get(i) != Some(&b'>') {
        return None;
    }
    Some(i + 1 - start)
}

/// Scans one domain label of an email autolink.
fn scan_email_domain_label(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    if !bytes.get(i).is_some_and(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    i += 1;
    let mut run = 0usize;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') && run < 62 {
        i += 1;
        run += 1;
    }
    if run >= 2 && bytes[i - 1] == b'-' {
        // The label must end in an alphanumeric, not a hyphen: back up one.
        i -= 1;
    }
    Some(i)
}

/// Scans an HTML comment body (after `<!--`). Returns the length of the
/// body, excluding the `<!--` prefix and `-->` suffix.
fn scan_html_comment(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    while i < len {
        // Runs of non-dash non-NUL characters, a dash followed by a
        // non-dash, or two dashes followed by anything but `>`. A NUL
        // byte is not comment content in the reference scanner.
        if bytes[i] == 0 {
            return None;
        }
        if bytes[i] != b'-' {
            while i < len && bytes[i] != b'-' && bytes[i] != 0 {
                i += 1;
            }
        } else if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            match bytes.get(i + 2) {
                Some(&b'>') => return Some(i - start),
                // `--` followed by any other byte (a dash included) is
                // comment content, as in the reference scanner.
                Some(&b) if b != 0 => i += 3,
                _ => return None,
            }
        } else if bytes.get(i + 1).is_some_and(|&b| b != b'-' && b != 0) {
            i += 2;
        } else {
            // A dash at end of input.
            return None;
        }
    }
    None
}

/// Scans a processing instruction body (after `<?`). Returns the body's
/// length, excluding the `?>` suffix.
fn scan_html_pi(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    while i < len {
        // A NUL byte cannot appear in the instruction body.
        if bytes[i] == 0 {
            return None;
        }
        if bytes[i] == b'?' {
            if bytes.get(i + 1) == Some(&b'>') {
                return Some(i - start);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    // An unterminated instruction matches only at end of input; the
    // caller's fit check then rejects it.
    Some(i - start)
}

/// Scans an HTML declaration body (after `<!`). Returns the body's length,
/// excluding the `>` suffix.
fn scan_html_declaration(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    while i < len && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == start {
        return None;
    }
    // The declaration body stops at `>` and cannot contain a NUL byte.
    while i < len && bytes[i] != b'>' {
        if bytes[i] == 0 {
            return None;
        }
        i += 1;
    }
    Some(i - start)
}

/// Scans a CDATA body (after `<![`). Returns the body's length, excluding
/// the `]]>` suffix.
fn scan_html_cdata(bytes: &[u8], start: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"CDATA[";
    if bytes.len() < start + PREFIX.len() || &bytes[start..start + PREFIX.len()] != PREFIX {
        return None;
    }
    let mut i = start + PREFIX.len();
    let len = bytes.len();
    while i < len {
        if bytes[i] == 0 {
            return None;
        }
        if bytes[i] != b']' {
            while i < len && bytes[i] != b']' && bytes[i] != 0 {
                i += 1;
            }
        } else if bytes.get(i + 1) == Some(&b']') {
            match bytes.get(i + 2) {
                Some(&b'>') => return Some(i - start),
                Some(&b) if b != 0 => i += 3,
                _ => return None,
            }
        } else if bytes.get(i + 1).is_some_and(|&b| b != b']' && b != 0) {
            i += 2;
        } else {
            return None;
        }
    }
    None
}

/// Scans an inline HTML open or close tag. Returns the length from the
/// tag's first byte through the closing `>`.
fn scan_html_tag(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    if bytes.get(i) == Some(&b'/') {
        // Close tag: `/` tagname spacechar* `>`.
        i += 1;
        i += scan_tagname(bytes, i)?;
        i += count_html_space(bytes, i);
        if bytes.get(i) == Some(&b'>') {
            return Some(i + 1 - start);
        }
        return None;
    }
    // Open tag: tagname attribute* spacechar* `/`? `>`.
    i += scan_tagname(bytes, i)?;
    loop {
        let save = i;
        let spaces = count_html_space(bytes, i);
        if spaces == 0 {
            break;
        }
        i += spaces;
        match scan_attribute_name(bytes, i) {
            Some(name_len) => {
                i += name_len;
                // An optional value specification.
                let before_value = i;
                i += count_html_space(bytes, i);
                if bytes.get(i) == Some(&b'=') {
                    i += 1;
                    i += count_html_space(bytes, i);
                    match scan_attribute_value(bytes, i) {
                        Some(value_len) => i += value_len,
                        None => i = before_value,
                    }
                } else {
                    i = before_value;
                }
            }
            None => {
                // The spaces belong to the tag's trailing whitespace.
                i = save;
                break;
            }
        }
    }
    i += count_html_space(bytes, i);
    if bytes.get(i) == Some(&b'/') {
        i += 1;
    }
    if bytes.get(i) == Some(&b'>') {
        Some(i + 1 - start)
    } else {
        None
    }
}

/// Scans a tag name: a letter, then letters, digits or hyphens.
fn scan_tagname(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    if !bytes.get(i).is_some_and(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    i += 1;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    Some(i - start)
}

/// Counts HTML whitespace (` \t\v\f\r\n`) from `start`.
fn count_html_space(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\x0B' | b'\x0C' | b'\r' | b'\n') {
        i += 1;
    }
    i - start
}

/// Scans an attribute name: a letter, `:` or `_`, then name characters.
fn scan_attribute_name(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    if !bytes
        .get(i)
        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b':' || *b == b'_')
    {
        return None;
    }
    i += 1;
    while i < len
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b':' | b'.' | b'_' | b'-'))
    {
        i += 1;
    }
    Some(i - start)
}

/// Scans an attribute value: unquoted, single-quoted or double-quoted.
fn scan_attribute_value(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    match bytes.get(start) {
        Some(&q @ (b'"' | b'\'')) => {
            let mut i = start + 1;
            while i < len && bytes[i] != q && bytes[i] != 0 {
                i += 1;
            }
            if i < len && bytes[i] == q {
                Some(i + 1 - start)
            } else {
                None
            }
        }
        Some(_) => {
            let mut i = start;
            while i < len
                && !matches!(
                    bytes[i],
                    b' ' | b'\t'
                        | b'\x0B'
                        | b'\x0C'
                        | b'\r'
                        | b'\n'
                        | b'\0'
                        | b'"'
                        | b'\''
                        | b'='
                        | b'<'
                        | b'>'
                        | b'`'
                )
            {
                i += 1;
            }
            if i == start { None } else { Some(i - start) }
        }
        None => None,
    }
}

/// Which ASCII bytes can start a special inline construct.
const SPECIAL_CHARS: [bool; 256] = {
    let mut table = [false; 256];
    table[b'\n' as usize] = true;
    table[b'\r' as usize] = true;
    table[b'!' as usize] = true;
    table[b'&' as usize] = true;
    table[b'*' as usize] = true;
    table[b'<' as usize] = true;
    table[b'[' as usize] = true;
    table[b'\\' as usize] = true;
    table[b']' as usize] = true;
    table[b'_' as usize] = true;
    table[b'`' as usize] = true;
    table
};
