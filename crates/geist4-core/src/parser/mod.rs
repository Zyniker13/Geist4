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

use crate::arena::{Arena, NodeId};
use crate::node::{CodeBlockData, HeadingData, HtmlBlockData, ItemData, LinkDef, ListData, Node};
use crate::span::Span;
use cursor::Cursor;
use std::borrow::Cow;
use std::collections::HashMap;

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
    let (mut arena, root, link_defs) = Parser::new(src).extensions(extensions).run();
    if !options.is_empty() {
        for ext in options.extensions() {
            ext.post_process(&mut crate::extension::TreeCtx {
                arena: &mut arena,
                root,
                options,
            });
        }
    }
    Document {
        arena,
        root,
        link_defs,
    }
}

/// Parses the block structure of `src`.
struct Parser<'a, 'ext> {
    src: &'a str,
    arena: Arena<'a>,
    open: Vec<Frame>,
    line_number: usize,
    link_defs: HashMap<String, LinkDef<'a>>,
    /// Whether each node's last processed line was blank, indexed by node.
    last_line_blank: Vec<bool>,
    /// Whether the text being added is a lazy paragraph continuation.
    adding_lazy: bool,
    /// Whether the line being processed is a plain slice of the source.
    /// A line containing NUL bytes is rebuilt with U+FFFD replacements
    /// and is not, so its content must not be borrowed from the source.
    line_is_src: bool,
    /// The registered extensions, or `None` on the plain path; passed
    /// through to the inline phase's hook sites.
    extensions: Option<&'ext [std::sync::Arc<dyn crate::extension::Extension>]>,
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
            last_line_blank: vec![false],
            adding_lazy: false,
            line_is_src: true,
            extensions: None,
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

    /// Runs the whole block parse and returns the arena, root and defs.
    fn run(mut self) -> (Arena<'a>, NodeId, HashMap<String, LinkDef<'a>>) {
        let bytes = self.src.as_bytes();
        let mut line_start = 0usize;
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
            self.process_line(line, line_start, line_stop);
            line_start = next_start;
        }
        let final_offset = self.src.len();
        while self.open.len() > 1 {
            self.close_last(final_offset);
        }
        let root = self.open[0].id;
        self.parse_all_inlines(root);
        (self.arena, root, self.link_defs)
    }

    /// Processes one line against the open block stack.
    fn process_line(&mut self, line: &[u8], line_offset: usize, line_stop: usize) {
        let mut cursor = Cursor::new(line);

        // Phase A: match the open containers. Nothing closes yet.
        match self.walk(&mut cursor, line_stop) {
            WalkResult::FenceClosed => return,
            WalkResult::FailedAt(depth) => {
                // A lazy continuation: the deepest open block is a paragraph,
                // the line is not blank, and no block start interrupts it.
                // The failed containers stay open.
                if self.deepest_is_paragraph()
                    && !cursor.blank_from_here()
                    && !self.block_start_matches(&cursor)
                {
                    // The line's leading whitespace stays in the buffer;
                    // the inline phase strips it after each newline, and a
                    // consumed reference definition must not hide it.
                    let id = self.open.last().unwrap().id;
                    self.append_paragraph_rest(id, &cursor, line_offset);
                    self.set_frame_end(line_stop);
                    self.update_last_line_blank(self.open[depth - 1].id, Some(depth - 1), false);
                    return;
                }
                self.close_from(depth, line_stop);
            }
            WalkResult::Matched => {}
        }

        // Phase B: open the new blocks the line starts.
        let opened = self.open_new_blocks(&mut cursor, line_offset, line_stop);

        // Phase C: add the remaining line to the open leaf block.
        let added = self.add_text(&mut cursor, line_offset, line_stop);

        // The node the last-line-blank flag lands on: the block that received
        // the line's text, or the last block this line opened.
        let target = match added {
            Some(id) => id,
            None => opened.unwrap_or_else(|| self.open.last().unwrap().id),
        };
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
                        self.close_last(line_stop);
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
            };
            if !matched {
                return WalkResult::FailedAt(depth);
            }
        }
        WalkResult::Matched
    }

    /// Whether the deepest open block is a paragraph.
    fn deepest_is_paragraph(&self) -> bool {
        matches!(self.open.last().unwrap().kind, OpenKind::Paragraph { .. })
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
                self.interrupt_paragraph(line_stop);
                self.close_until_can_contain(line_stop);
                let id = self.open_block_quote(line_offset);
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
                if let Some((level, text)) = leaf::atx_heading(&at) {
                    self.interrupt_paragraph(line_stop);
                    self.close_until_can_contain(line_stop);
                    let id = self.open_atx_heading(level, text, line_offset, line_stop);
                    cursor.bump_all();
                    return Some(id);
                }
            }

            // Fenced code. The info string stays on the line; it becomes the
            // literal's first line and is extracted when the block closes.
            if indent < 4 {
                if let Some(fence) = leaf::fence_start(&at) {
                    self.interrupt_paragraph(line_stop);
                    self.close_until_can_contain(line_stop);
                    let id = self.open_fenced_code(&fence, indent, line_offset);
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
                    self.interrupt_paragraph(line_stop);
                    self.close_until_can_contain(line_stop);
                    let id = self.open_html_block(start.kind, line_offset);
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
                self.interrupt_paragraph(line_stop);
                self.close_until_can_contain(line_stop);
                let id = self.open_thematic_break(line_offset, line_stop);
                cursor.bump_all();
                return Some(id);
            }

            // List item.
            if indent < 4 {
                if let Some(marker) = self.item_start(&at, scan_is_para) {
                    cursor.set_pos_col(pos, col);
                    self.open_list_item(&marker, cursor, indent, line_offset, line_stop);
                    opened = Some(self.open.last().unwrap().id);
                    maybe_lazy = false;
                    continue;
                }
            }

            // Indented code: four columns come off the line from where the
            // container prefixes left it, not from the first non-space byte.
            if !maybe_lazy && !cursor.blank_from_here() && indent >= 4 {
                self.close_until_can_contain(line_stop);
                let id = self.open_indented_code(line_offset);
                cursor.advance_cols(4);
                return Some(id);
            }

            break;
        }
        opened
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
    /// block can interrupt it.
    fn interrupt_paragraph(&mut self, line_stop: usize) {
        if self.deepest_is_paragraph() {
            self.close_last(line_stop);
        }
    }

    /// Closes open frames until the deepest frame can hold a new child,
    /// mirroring the reference algorithm's add-child walk: only the
    /// document, block quotes and list items accept arbitrary blocks, a
    /// list accepts items only, and leaf frames accept nothing.
    fn close_until_can_contain(&mut self, line_stop: usize) {
        while !matches!(
            self.open.last().unwrap().kind,
            OpenKind::Document | OpenKind::BlockQuote | OpenKind::Item { .. }
        ) {
            self.close_last(line_stop);
        }
    }

    /// Closes the paragraph and any list until a container that can hold a
    /// list is open.
    fn make_room_for_list(&mut self, line_stop: usize) {
        while matches!(
            self.open.last().unwrap().kind,
            OpenKind::Paragraph { .. } | OpenKind::List(_)
        ) {
            self.close_last(line_stop);
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
        line_stop: usize,
    ) {
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
        if !(1..5).contains(&padding) || cursor.blank_from_here() {
            *cursor = after_marker;
            if padding > 0 {
                cursor.advance_cols(1);
            }
            // The reference algorithm rewinds to one column past the marker
            // and counts a single column of padding.
            padding = 1;
        }

        let same_type = match &self.open.last().unwrap().kind {
            OpenKind::List(list) => list.ordered == marker.ordered && list.marker == marker.char,
            _ => false,
        };
        if !same_type {
            self.make_room_for_list(line_stop);
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
        let id = self.arena.alloc(
            Node::Item(ItemData { padding: 0 }),
            Span::empty(line_offset),
        );
        self.arena.push_child(list_id, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::Item {
                content_offset: indent + marker.width + padding,
            },
            start_line: self.line_number,
            start_offset: line_offset,
            end_offset: line_offset,
        });
    }

    /// Opens an indented code block under the deepest container.
    fn open_indented_code(&mut self, line_offset: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::CodeBlock(CodeBlockData {
                fenced: false,
                fence_char: 0,
                info: None,
                literal: Cow::Owned(String::new()),
            }),
            Span::empty(line_offset),
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
            start_offset: line_offset,
            end_offset: line_offset,
        });
        id
    }

    /// Opens a fenced code block. `fence_offset` is the fence's column past
    /// the parent container's content column, which fixes how much
    /// indentation comes off each content line.
    fn open_fenced_code(
        &mut self,
        fence: &leaf::FenceStart,
        fence_offset: usize,
        line_offset: usize,
    ) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::CodeBlock(CodeBlockData {
                fenced: true,
                fence_char: fence.ch,
                info: None,
                literal: Cow::Owned(String::new()),
            }),
            Span::empty(line_offset),
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
            start_offset: line_offset,
            end_offset: line_offset,
        });
        id
    }

    /// Opens an HTML block of the given kind.
    fn open_html_block(&mut self, kind: u8, line_offset: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::HtmlBlock(HtmlBlockData {
                kind,
                literal: Cow::Owned(String::new()),
            }),
            Span::empty(line_offset),
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
            start_offset: line_offset,
            end_offset: line_offset,
        });
        id
    }

    /// Opens a thematic break, closed at once.
    fn open_thematic_break(&mut self, line_offset: usize, line_stop: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self
            .arena
            .alloc(Node::ThematicBreak, Span::new(line_offset, line_stop));
        self.arena.push_child(parent, id);
        id
    }

    /// Opens a block quote.
    fn open_block_quote(&mut self, line_offset: usize) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(Node::BlockQuote, Span::empty(line_offset));
        self.arena.push_child(parent, id);
        self.last_line_blank.push(false);
        self.open.push(Frame {
            id,
            kind: OpenKind::BlockQuote,
            start_line: self.line_number,
            start_offset: line_offset,
            end_offset: line_offset,
        });
        id
    }

    /// Opens an ATX heading with its text, closed at once.
    fn open_atx_heading(
        &mut self,
        level: u8,
        text: String,
        line_offset: usize,
        line_stop: usize,
    ) -> NodeId {
        let parent = self.open.last().unwrap().id;
        let id = self.arena.alloc(
            Node::Heading(HeadingData {
                level,
                setext: false,
            }),
            Span::new(line_offset, line_stop),
        );
        let text_id = self
            .arena
            .alloc(Node::Text(Cow::Owned(text)), Span::empty(line_offset));
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
        let mut stack: Vec<NodeId> = self.arena.children(root).to_vec();
        while let Some(id) = stack.pop() {
            if matches!(self.arena.kind(id), Node::Paragraph | Node::Heading(_)) {
                // One append per text part: the parts already borrow the
                // source, so no intermediate strings are built.
                text.clear();
                for child in self.arena.children(id) {
                    if let Node::Text(part) = self.arena.kind(*child) {
                        text.push_str(part);
                    }
                }
                self.arena.entry_mut(id).children.clear();
                crate::inline::parse_inlines_into(
                    &mut session,
                    &text,
                    &self.link_defs,
                    &mut self.arena,
                    id,
                    self.extensions,
                );
            } else {
                stack.extend(self.arena.children(id).iter().copied());
            }
        }
    }

    /// Converts the open paragraph into a setext heading of `level`, first
    /// extracting leading link reference definitions.
    fn setext_heading_or_defs(&mut self, level: u8, line_stop: usize) -> SetextOutcome {
        let (frame_text, id) = match self.open.last_mut() {
            Some(Frame {
                kind: OpenKind::Paragraph { text },
                id,
                ..
            }) => (std::mem::take(text), *id),
            _ => return SetextOutcome::NotSetext,
        };
        let consumed = self.extract_link_defs(&frame_text);
        let remainder = frame_text[consumed..].to_string();
        if remainder.is_empty() {
            // The paragraph was nothing but definitions; it stays open with
            // empty text and the line continues it.
            return SetextOutcome::Dissolved;
        }
        self.arena.set_kind(
            id,
            Node::Heading(HeadingData {
                level,
                setext: true,
            }),
        );
        let text_id = self
            .arena
            .alloc(Node::Text(Cow::Owned(remainder)), Span::empty(0));
        self.arena.push_child(id, text_id);
        // The frame now only carries the span; the node itself is finished,
        // so the closing step must not treat it as an empty paragraph.
        if let Some(Frame { kind, .. }) = self.open.last_mut() {
            if matches!(kind, OpenKind::Paragraph { .. }) {
                *kind = OpenKind::Heading;
            }
        }
        self.close_last(line_stop);
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
    ) -> Option<NodeId> {
        let id = self.open.last().unwrap().id;
        match &self.open.last().unwrap().kind {
            OpenKind::Paragraph { .. } => {
                // A normal continuation line: its leading whitespace is
                // consumed here, as the reference parser does. Lazy lines
                // (handled in the walk) keep theirs.
                cursor.skip_to_content();
                self.append_paragraph_rest(id, cursor, line_offset);
                Some(id)
            }
            OpenKind::CodeBlock { .. } => {
                self.append_code_rest(id, cursor, line_offset);
                Some(id)
            }
            OpenKind::HtmlBlock { kind, .. } => {
                let kind = *kind;
                self.append_html_rest(id, cursor, line_offset);
                if kind <= 5 {
                    let text = self.line_rest_bytes(cursor);
                    if leaf::html_block_end(kind, leaf::trim_ascii(&text)) {
                        self.close_last(line_stop);
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
                    let text = self.line_rest_string(cursor, line_offset);
                    self.close_until_can_contain(line_stop);
                    Some(self.open_paragraph(&text, line_offset))
                }
            }
        }
    }

    /// Opens a paragraph whose first line may be a lazy continuation.
    ///
    /// A lazily added line means the line was paragraph text of an
    /// enclosing container's existing paragraph child, so that child
    /// continues here instead of a new paragraph starting.
    fn open_paragraph(&mut self, text: &str, line_offset: usize) -> NodeId {
        if self.adding_lazy {
            self.adding_lazy = false;
            let parent = self.open.last().unwrap().id;
            if let Some(&last) = self.arena.children(parent).last() {
                if *self.arena.kind(last) == Node::Paragraph {
                    self.append_paragraph_text(last, text);
                    self.open.push(Frame {
                        id: last,
                        kind: OpenKind::Paragraph {
                            text: String::new(),
                        },
                        start_line: self.line_number,
                        start_offset: line_offset,
                        end_offset: line_offset,
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
            start_offset: line_offset,
            end_offset: line_offset,
        });
        id
    }

    /// Appends one line of text to a paragraph's buffer.
    fn append_paragraph_text(&mut self, id: NodeId, text: &str) {
        for frame in self.open.iter_mut().rev() {
            if frame.id == id {
                if let OpenKind::Paragraph { text: buf } = &mut frame.kind {
                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(text);
                    return;
                }
            }
        }
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

    /// The cursor's remaining line as owned text, for callers that need a
    /// `String` regardless.
    fn line_rest_string(&self, cursor: &Cursor<'_>, line_offset: usize) -> String {
        self.line_rest(cursor, line_offset).into_owned()
    }

    /// The cursor's remaining line as bytes, for byte-oriented checks.
    fn line_rest_bytes(&self, cursor: &Cursor<'_>) -> Vec<u8> {
        cursor.rest()
    }

    /// Appends the cursor's remaining line to the open paragraph's text.
    fn append_paragraph_rest(&mut self, id: NodeId, cursor: &Cursor<'_>, line_offset: usize) {
        let text = self.line_rest(cursor, line_offset);
        self.append_paragraph_text(id, &text);
    }

    /// Appends the cursor's remaining line to a code block's literal.
    fn append_code_rest(&mut self, id: NodeId, cursor: &Cursor<'_>, line_offset: usize) {
        let text = self.line_rest(cursor, line_offset);
        for frame in self.open.iter_mut().rev() {
            if frame.id == id {
                if let OpenKind::CodeBlock { literal, .. } = &mut frame.kind {
                    literal.push_str(&text);
                    literal.push('\n');
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

    /// Closes frames from `depth` to the end of the open stack.
    fn close_from(&mut self, depth: usize, line_stop: usize) {
        while self.open.len() > depth.max(1) {
            self.close_last(line_stop);
        }
    }

    /// Closes the deepest open block, finalizing it.
    fn close_last(&mut self, line_stop: usize) {
        let mut frame = self.open.pop().expect("the document frame stays open");
        frame.end_offset = line_stop.max(frame.end_offset);
        match &mut frame.kind {
            OpenKind::Paragraph { text } => {
                let text = std::mem::take(text);
                let consumed = self.extract_link_defs(&text);
                let remainder = &text[consumed..];
                if remainder.is_empty() {
                    self.remove_last_child_of_parent(frame.id);
                } else {
                    let text_id = self.arena.alloc(
                        Node::Text(Cow::Owned(remainder.to_string())),
                        Span::empty(frame.start_offset),
                    );
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
            _ => {}
        }
        self.arena
            .set_span(frame.id, Span::new(frame.start_offset, frame.end_offset));
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

    /// Extracts as many leading link reference definitions as the text
    /// starts with, recording them and returning the consumed byte count.
    /// Only whitespace may separate consecutive definitions; the first
    /// non-definition text stops the scan.
    fn extract_link_defs(&mut self, text: &str) -> usize {
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
            let key = normalize_label(&scanned.label);
            if let std::collections::hash_map::Entry::Vacant(e) = self.link_defs.entry(key) {
                e.insert(LinkDef {
                    label: Cow::Owned(scanned.label),
                    destination: Cow::Owned(crate::inline::clean_url(&scanned.destination)),
                    title: scanned
                        .title
                        .and_then(|t| crate::inline::clean_title(&t))
                        .map(Cow::Owned),
                });
            }
            consumed += skip + scanned.consumed;
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
    let trimmed =
        unescaped.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\u{b}' | '\u{c}' | '\r' | '\n'));
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
pub(crate) fn normalize_label(label: &str) -> String {
    let mut out = String::new();
    normalize_label_into(label, &mut out);
    out
}

/// Writes the normalized form of `label` into `out`, which is cleared
/// first; reuse the buffer to skip the allocation per lookup.
pub(crate) fn normalize_label_into(label: &str, out: &mut String) {
    out.clear();
    let mut pending_ws = false;
    for ch in label.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}') {
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
