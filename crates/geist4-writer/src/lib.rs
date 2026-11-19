//! The Geist4 CommonMark writer: serializes a parsed document tree back to
//! normalized CommonMark source.
//!
//! The writer walks the arena tree the parser produced and emits normalized
//! Markdown: ATX headings, `-` bullets, fenced code blocks, inline links and
//! images, and conservative escaping, so that re-parsing the output yields a
//! tree that renders to the same HTML as the original document.
//!
//! Scope: the seventeen core node kinds. Extension nodes fall back to their
//! children (or nothing when they are leaves); no output shape is promised
//! for them.

use geist4_core::arena::{Arena, NodeId};
use geist4_core::node::{CodeBlockData, ListData, Node, SpannedDef};
use geist4_core::parser::Document;
use geist4_core::unicode::{is_punct_or_symbol, is_space};

/// Serializes `document` back to normalized CommonMark source.
///
/// The output is deterministic: the same tree always serializes to the same
/// text. It ends with a single newline after the last block, and every link
/// reference definition the document carries is re-emitted at the end in
/// `[label]: destination "title"` form.
#[must_use]
pub fn to_common_mark(document: &Document<'_>) -> String {
    let mut writer = Writer {
        arena: &document.arena,
        out: String::new(),
        last_list_marker: None,
    };
    let roots: Vec<NodeId> = document.arena.children(document.root).to_vec();
    writer.blocks(&roots, "", "", Sep::Blank);
    if !document.defs.is_empty() {
        if !writer.out.is_empty() {
            writer.out.push('\n');
        }
        for def in &document.defs {
            writer.write_def(def);
        }
    }
    writer.out
}

/// Whether sibling blocks inside a container are separated by blank lines.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sep {
    /// A blank line between sibling blocks.
    Blank,
    /// No blank line: the blocks of one tight list item.
    Tight,
}

/// The character-class context the escaper and the emphasis writer track at
/// each position.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// The start or end of the rendered content: behaves like whitespace for
    /// the flanking rules.
    Edge,
    /// Unicode whitespace.
    Ws,
    /// Unicode punctuation or symbol.
    Punct,
    /// Anything else: letters, digits and the rest.
    Other,
}

fn class_of(c: char) -> Class {
    if is_punct_or_symbol(c) {
        Class::Punct
    } else if is_space(c) {
        Class::Ws
    } else {
        Class::Other
    }
}

impl Class {
    /// The whitespace-like view used by the flanking rules, where line edges
    /// count as whitespace.
    fn as_ws(self) -> Class {
        match self {
            Class::Edge => Class::Ws,
            other => other,
        }
    }
}

/// A run of same-character emphasis delimiters at one boundary: the widths
/// (1 for emphasis, 2 for strong) of the nested emphasis nodes, outermost
/// first, and — for a closing run — the character that follows the
/// outermost closer, which decides whether a member may switch to the
/// other marker character.
struct Touch {
    ch: char,
    widths: Vec<u8>,
    after: Option<char>,
}

/// The canonical nesting a fully matched delimiter run of `total` characters
/// splits into, outermost first: pairs of two close as strong from the
/// inside out, and a leftover single character wraps as emphasis.
fn canonical_run(total: u8) -> Vec<u8> {
    let mut seq: Vec<u8> = Vec::new();
    let mut rest = total;
    while rest >= 2 {
        seq.push(2);
        rest -= 2;
    }
    if rest == 1 {
        seq.push(1);
    }
    seq.reverse();
    seq
}

fn could_flank(c: char, prev: Class, next: Class) -> bool {
    let prev = prev.as_ws();
    let next = next.as_ws();
    let left = next != Class::Ws && (next != Class::Punct || prev != Class::Other);
    let right =
        prev != Class::Ws && (prev != Class::Punct || matches!(next, Class::Ws | Class::Punct));
    if c == '*' {
        left || right
    } else {
        let open = left && (!right || prev == Class::Punct);
        let close = right && (!left || next == Class::Punct);
        open || close
    }
}

struct Writer<'a> {
    arena: &'a Arena<'a>,
    out: String,
    /// The marker the writer chose for the list it emitted last, so a
    /// following sibling list can pick a marker that keeps the two from
    /// re-parsing as one.
    last_list_marker: Option<(bool, char)>,
}

impl<'a> Writer<'a> {
    /// Writes a run of sibling blocks into the current container.
    fn blocks(&mut self, ids: &[NodeId], lp: &str, flp: &str, sep: Sep) {
        let mut prev_para = false;
        let mut prev_kind_list = false;
        let mut prev_list_marker: Option<(bool, char)> = None;
        for (index, &id) in ids.iter().enumerate() {
            if index > 0 && sep == Sep::Blank {
                self.separator(lp);
            }
            let first = if index == 0 { flp } else { lp };
            let adjacency = if index > 0 && prev_kind_list {
                prev_list_marker
            } else {
                None
            };
            self.block(id, lp, first, prev_para && sep == Sep::Tight, adjacency);
            prev_kind_list = matches!(self.arena.kind(id), Node::List(_));
            prev_list_marker = if prev_kind_list {
                self.last_list_marker
            } else {
                None
            };
            prev_para = matches!(self.arena.kind(id), Node::Paragraph);
        }
    }

    /// Writes one blank separator line at the container's own level.
    fn separator(&mut self, lp: &str) {
        self.out.push_str(lp);
        self.out.push('\n');
    }

    /// Writes one block. `lp` prefixes every line, `flp` the block's first
    /// line (a list item's marker line); `after_para` reports that a
    /// paragraph immediately precedes with no blank line between, which
    /// makes a `---` rule read as a setext underline; `adjacency` carries
    /// the previous sibling list's marker so this list can pick a different
    /// one.
    fn block(
        &mut self,
        id: NodeId,
        lp: &str,
        flp: &str,
        after_para: bool,
        adjacency: Option<(bool, char)>,
    ) {
        match self.arena.kind(id).clone() {
            Node::Document => {}
            Node::BlockQuote => {
                let kids: Vec<NodeId> = self.arena.children(id).to_vec();
                if kids.is_empty() {
                    // An empty quote still has to exist after re-parsing.
                    self.out.push_str(flp);
                    self.out.push('>');
                    self.out.push('\n');
                } else {
                    let inner_lp = format!("{lp}> ");
                    let inner_flp = format!("{flp}> ");
                    self.blocks(&kids, &inner_lp, &inner_flp, Sep::Blank);
                }
            }
            Node::List(data) => self.list(id, &data, lp, flp, adjacency),
            Node::Heading(data) => {
                if self.heading_has_hard_break(id) {
                    // A hard break renders as <br />, which cannot be
                    // encoded inside a single ATX line; a setext underline
                    // can cover several paragraph lines, so keep that
                    // shape.
                    let body = self.inline_body(id, false);
                    self.emit_multiline(&body, lp, flp);
                    self.out.push_str(lp);
                    self.out
                        .push_str(if data.level <= 1 { "===" } else { "---" });
                    self.out.push('\n');
                } else {
                    let body = self.inline_body(id, true);
                    let body = guard_heading_hashes(body);
                    let hashes = "#".repeat(usize::from(data.level));
                    self.out.push_str(flp);
                    self.out.push_str(&hashes);
                    self.out.push(' ');
                    self.out.push_str(&body);
                    self.out.push('\n');
                }
            }
            Node::ThematicBreak => {
                self.out.push_str(flp);
                self.out.push_str(thematic_rule(flp, after_para));
                self.out.push('\n');
            }
            Node::Paragraph => {
                let body = self.inline_body(id, false);
                self.emit_multiline(&body, lp, flp);
            }
            Node::CodeBlock(data) => self.code_block(&data, lp, flp),
            Node::HtmlBlock(data) => {
                let lines: Vec<&str> = data.literal.split('\n').collect();
                let body = if lines.last().is_some_and(|last| last.is_empty()) {
                    &lines[..lines.len() - 1]
                } else {
                    &lines[..]
                };
                for (index, line) in body.iter().enumerate() {
                    if index == 0 {
                        self.out.push_str(flp);
                    } else {
                        self.out.push_str(lp);
                    }
                    self.out.push_str(line);
                    self.out.push('\n');
                }
            }
            Node::Extension(_) => {
                let kids: Vec<NodeId> = self.arena.children(id).to_vec();
                if !kids.is_empty() {
                    self.blocks(&kids, lp, flp, Sep::Blank);
                }
            }
            // Leaf inline kinds never appear at block level; render them as
            // a one-line paragraph so nothing is silently dropped.
            _ => {
                let holder = [id];
                let mut body = String::new();
                self.inlines(
                    &holder,
                    Class::Edge,
                    false,
                    None,
                    None,
                    None,
                    true,
                    false,
                    &mut body,
                );
                self.emit_multiline(&body, lp, flp);
            }
        }
    }

    /// Writes one fenced code block (indented code blocks are normalized to
    /// fenced form, which renders identically).
    fn code_block(&mut self, data: &CodeBlockData<'_>, lp: &str, flp: &str) {
        let info = data.info.as_deref().unwrap_or("");
        let ch: char = if info.contains('`') { '~' } else { '`' };
        let longest = longest_run(&data.literal, ch);
        let fence_len = 3.max(longest + 1);
        let fence: String = ch.to_string().repeat(fence_len);
        self.out.push_str(flp);
        self.out.push_str(&fence);
        // The stored info has no leading or trailing whitespace (the
        // parser strips it), so fence and info sit flush — unless the
        // info itself starts with the fence character, where the
        // opening run would swallow it: `~~~~` before `~~~a` reads as a
        // five-tilde fence. One space keeps the run's edge.
        if info.starts_with(ch) {
            self.out.push(' ');
        }
        if !info.is_empty() {
            self.out.push_str(&encode_info(info));
        }
        self.out.push('\n');
        // The literal always ends with a newline; only the trailing split
        // artifact is dropped, interior blank lines stay.
        let lines: Vec<&str> = data.literal.split('\n').collect();
        let body = if lines.last().is_some_and(|last| last.is_empty()) {
            &lines[..lines.len() - 1]
        } else {
            &lines[..]
        };
        for line in body {
            self.out.push_str(lp);
            self.out.push_str(line);
            self.out.push('\n');
        }
        self.out.push_str(lp);
        self.out.push_str(&fence);
        self.out.push('\n');
    }

    /// Writes one list with its items, preserving tightness and `start`.
    /// When the previous sibling is a list, the marker rotates so the two
    /// do not re-parse as one list.
    fn list(
        &mut self,
        id: NodeId,
        data: &ListData,
        lp: &str,
        flp: &str,
        adjacency: Option<(bool, char)>,
    ) {
        let marker_char = if data.ordered {
            match adjacency {
                Some((true, '.')) => ')',
                Some((true, ')')) | Some((true, _)) | Some((false, _)) | None => '.',
            }
        } else {
            match adjacency {
                Some((false, '-')) => '+',
                Some((false, '+')) => '*',
                Some((false, '*')) | Some((false, _)) | Some((true, _)) | None => '-',
            }
        };
        self.last_list_marker = Some((data.ordered, marker_char));
        let items: Vec<NodeId> = self.arena.children(id).to_vec();
        for (index, &item) in items.iter().enumerate() {
            if index > 0 && !data.tight {
                self.separator(lp);
            }
            let marker = if data.ordered {
                format!("{}{marker_char} ", data.start + index as u64)
            } else {
                format!("{marker_char} ")
            };
            let pad = " ".repeat(marker.len());
            let item_lp = format!("{lp}{pad}");
            let item_flp = format!("{flp}{marker}");
            let kids: Vec<NodeId> = self.arena.children(item).to_vec();
            if kids.is_empty() {
                // An empty item is still an item: emit the bare marker.
                self.out.push_str(&item_flp);
                self.out.push('\n');
            } else {
                self.item_blocks(&kids, &item_lp, &item_flp, data.tight);
            }
        }
    }

    /// Writes the blocks of one list item; `tight` suppresses the blank
    /// lines that would loosen the list.
    fn item_blocks(&mut self, ids: &[NodeId], lp: &str, flp: &str, tight: bool) {
        let mut prev_para = false;
        let mut prev_kind_list = false;
        let mut prev_list_marker: Option<(bool, char)> = None;
        for (index, &id) in ids.iter().enumerate() {
            if index > 0 && !tight {
                self.separator(lp);
            }
            let first = if index == 0 { flp } else { lp };
            let adjacency = if index > 0 && prev_kind_list {
                prev_list_marker
            } else {
                None
            };
            self.block(id, lp, first, prev_para && tight, adjacency);
            prev_kind_list = matches!(self.arena.kind(id), Node::List(_));
            prev_list_marker = if prev_kind_list {
                self.last_list_marker
            } else {
                None
            };
            prev_para = matches!(self.arena.kind(id), Node::Paragraph);
        }
    }

    /// Writes one link reference definition at the end of the document.
    fn write_def(&mut self, def: &SpannedDef<'_>) {
        self.out.push('[');
        self.out.push_str(&def.def.label);
        self.out.push_str("]: ");
        self.out.push_str(&encode_destination(&def.def.destination));
        if let Some(title) = &def.def.title {
            self.out.push_str(" \"");
            self.out.push_str(&encode_title(title));
            self.out.push('"');
        }
        self.out.push('\n');
    }

    /// Renders a leaf block's inline content into a prefix-free string.
    /// `single_line` forces breaks into numeric references so the content
    /// stays on one line (ATX headings).
    fn inline_body(&mut self, id: NodeId, single_line: bool) -> String {
        let kids: Vec<NodeId> = self.arena.children(id).to_vec();
        let mut out = String::new();
        self.inlines(
            &kids,
            Class::Edge,
            false,
            None,
            None,
            None,
            true,
            single_line,
            &mut out,
        );
        out
    }

    /// Emits a rendered inline body line by line under the container's
    /// prefixes.
    fn emit_multiline(&mut self, body: &str, lp: &str, flp: &str) {
        let mut lines = body.split('\n');
        if let Some(first) = lines.next() {
            self.out.push_str(flp);
            self.out.push_str(first);
            self.out.push('\n');
        }
        for line in lines {
            self.out.push_str(lp);
            self.out.push_str(line);
            self.out.push('\n');
        }
    }

    /// Whether a hard break node sits anywhere inside a heading's content.
    fn heading_has_hard_break(&self, id: NodeId) -> bool {
        for &kid in self.arena.children(id) {
            match self.arena.kind(kid) {
                Node::HardBreak => return true,
                Node::Emphasis | Node::Strong | Node::Link(_) | Node::Extension(_)
                    if self.heading_has_hard_break(kid) =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Whether any emphasis node nested inside `id` opens right after a
    /// non-whitespace character. Such an opener could close an enclosing
    /// emphasis of the same character early, so the enclosure must not
    /// share that character.
    fn descendant_opener_after_content(&self, id: NodeId) -> bool {
        for &kid in self.arena.children(id) {
            if !matches!(
                self.arena.kind(kid),
                Node::Emphasis | Node::Strong | Node::Link(_) | Node::Extension(_)
            ) {
                continue;
            }
            if self.descendant_opener_after_content(kid) {
                return true;
            }
            if !matches!(self.arena.kind(kid), Node::Emphasis | Node::Strong) {
                continue;
            }
            // Only an opener that follows content inside `kid` can close
            // `kid`'s opener early. An opener that touches `kid`'s own
            // opener is handled by the canonical run logic instead.
            let grandkids = self.arena.children(kid);
            for position in 1..grandkids.len() {
                let &d = &grandkids[position];
                if matches!(self.arena.kind(d), Node::Emphasis | Node::Strong) {
                    let (last, _) = self.boundary(grandkids[position - 1]);
                    if let Some(last) = last {
                        if class_of(last) != Class::Ws && class_of(last) != Class::Edge {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Whether everything rendered from sibling `from` onwards, up to the
    /// next line break or the end of the inline content, is whitespace:
    /// a space there would be swallowed with it.
    fn trailing_eats_space(&self, ids: &[NodeId], from: usize, close: Option<char>) -> bool {
        let mut index = from;
        loop {
            if index >= ids.len() {
                return matches!(close, None | Some('*') | Some('_'));
            }
            match self.arena.kind(ids[index]) {
                Node::Text(value) if value.chars().all(|c| c == ' ' || c == '\t') => index += 1,
                Node::SoftBreak | Node::HardBreak => return true,
                _ => return false,
            }
        }
    }

    /// Whether a node's rendered form ends with a line break.
    fn ends_line(&self, id: NodeId) -> bool {
        match self.arena.kind(id) {
            Node::SoftBreak | Node::HardBreak => true,
            Node::Emphasis | Node::Strong | Node::Link(_) | Node::Extension(_) => self
                .arena
                .children(id)
                .last()
                .is_some_and(|&last| self.ends_line(last)),
            _ => false,
        }
    }

    /// The `(first, last)` characters a node renders with, computed
    /// structurally. Escaping preserves character classes at boundaries, so
    /// these logical characters stand in for the rendered ones everywhere
    /// the flanking and entity rules look at a neighbor.
    fn boundary(&self, id: NodeId) -> (Option<char>, Option<char>) {
        match self.arena.kind(id) {
            Node::Text(value) => (value.chars().next(), value.chars().last()),
            Node::CodeSpan(_) => (Some('`'), Some('`')),
            Node::InlineHtml(value) => (value.chars().next(), value.chars().last()),
            Node::SoftBreak => (Some('\n'), Some('\n')),
            Node::HardBreak => (Some(' '), Some('\n')),
            Node::Emphasis | Node::Strong => (Some('*'), Some('*')),
            Node::Link(_) => (Some('['), Some(']')),
            Node::Extension(_) => {
                let kids = self.arena.children(id);
                if kids.is_empty() {
                    (None, None)
                } else {
                    let first = self.boundary(kids[0]).0;
                    let last = self.boundary(kids[kids.len() - 1]).1;
                    (first, last)
                }
            }
            _ => (None, None),
        }
    }

    /// Walks back from `index` to the previous rendered boundary class.
    fn prev_class(bounds: &[(Option<char>, Option<char>)], index: usize, start: Class) -> Class {
        for prior in bounds[..index].iter().rev() {
            if let Some(c) = prior.1 {
                return class_of(c);
            }
        }
        start
    }

    /// Walks forward from `index` to the next rendered boundary character.
    fn next_char(
        bounds: &[(Option<char>, Option<char>)],
        index: usize,
        close: Option<char>,
    ) -> Option<char> {
        for later in &bounds[index + 1..] {
            if let Some(c) = later.0 {
                return Some(c);
            }
        }
        close
    }

    /// Renders inline nodes into `out`.
    ///
    /// `prev0` classifies the character before the first node and
    /// `prev_delim` reports that it is an emphasis delimiter character;
    /// `touch` carries the same-character delimiter run opening at the
    /// first node's position, `close_touch` the run closing at the last
    /// node's position; `close` is the enclosing delimiter that follows
    /// the last node; `at_start` reports that the position opens a line;
    /// and `single_line` forbids real line breaks (ATX heading content).
    #[allow(clippy::too_many_arguments)]
    fn inlines(
        &mut self,
        ids: &[NodeId],
        prev0: Class,
        prev_delim: bool,
        touch: Option<&Touch>,
        close_touch: Option<&Touch>,
        close: Option<char>,
        at_start: bool,
        single_line: bool,
        out: &mut String,
    ) {
        let bounds: Vec<(Option<char>, Option<char>)> =
            ids.iter().map(|&id| self.boundary(id)).collect();
        let mut line_start = at_start;
        let mut line_empty = at_start;
        let mut pending = false;
        let mut prev_close: Option<char> = None;
        let mut prev_was_delim = prev_delim;
        for (index, &id) in ids.iter().enumerate() {
            let prev = Self::prev_class(&bounds, index, prev0);
            let next = Self::next_char(&bounds, index, close);
            let node_touch = if index == 0 { touch } else { None };
            let node_close_touch = if index + 1 == ids.len() {
                close_touch
            } else {
                None
            };
            // A space directly before this node's position would merge
            // into a line break; a space directly before the enclosing
            // emphasis closer would break its right-flanking rule. Both
            // must be written as a numeric reference. A space before a
            // sibling emphasis opener is fine: the opener only needs a
            // non-whitespace character after it.
            let next_eats_space = if index + 1 < ids.len() {
                self.trailing_eats_space(ids, index + 1, close)
            } else {
                matches!(close, None | Some('*') | Some('_'))
            };
            match self.arena.kind(id).clone() {
                Node::Text(value) => {
                    escape_text(
                        &value,
                        prev,
                        next,
                        next_eats_space,
                        prev_was_delim,
                        &mut line_start,
                        &mut line_empty,
                        &mut pending,
                        out,
                    );
                }
                Node::CodeSpan(content) => out.push_str(&encode_code_span(&content)),
                Node::InlineHtml(value) => out.push_str(&value),
                Node::SoftBreak => {
                    let next_is_html = index + 1 < ids.len()
                        && matches!(self.arena.kind(ids[index + 1]), Node::InlineHtml(_));
                    if single_line || line_empty || next_is_html {
                        // A leading break would leave an empty first line
                        // that the parser drops, and a break before inline
                        // HTML would let the tag start an HTML block; an
                        // encoded newline keeps the rendered character.
                        out.push_str("&#10;");
                    } else {
                        out.push('\n');
                        line_start = true;
                        line_empty = true;
                    }
                }
                Node::HardBreak => {
                    if single_line {
                        // Unreachable for well-formed headings; keep the
                        // content on one line regardless.
                        out.push_str("&#10;");
                    } else if line_empty {
                        // `  ` alone would be a blank line; the backslash
                        // form still makes a hard break there.
                        out.push_str("\\\n");
                        line_start = true;
                        line_empty = true;
                    } else {
                        out.push_str("  \n");
                        line_start = true;
                        line_empty = true;
                    }
                }
                Node::Emphasis | Node::Strong => {
                    let width: u8 = if matches!(self.arena.kind(id), Node::Strong) {
                        2
                    } else {
                        1
                    };
                    let kids: Vec<NodeId> = self.arena.children(id).to_vec();
                    // A switch to the other marker character only survives
                    // when the underscore's intraword rules hold: it cannot
                    // open right after a letter (its opener's left neighbor)
                    // nor close right before one (its closer's right
                    // neighbor).
                    let opener_ok = prev != Class::Other;
                    let closer_ok = next.is_none_or(|c| class_of(c) != Class::Other);
                    // Opener side: a same-character run opening here must
                    // split into the canonical nesting for its width.
                    let (mut ch, mut chain) = match node_touch {
                        Some(t) if t.ch == '*' => {
                            let mut widths = t.widths.clone();
                            widths.push(width);
                            if canonical_run(widths.iter().copied().sum()) == widths {
                                ('*', widths)
                            } else if opener_ok && closer_ok {
                                ('_', vec![width])
                            } else {
                                ('*', widths)
                            }
                        }
                        _ => ('*', vec![width]),
                    };
                    // If this node merged with its parent's opener run, its
                    // first emphasis child extends the run further. When
                    // that child could not switch markers itself (its
                    // closer would sit before a letter), switch this node
                    // instead so the runs separate here.
                    if ch == '*' && !chain.is_empty() {
                        if let Some(&first) = kids.first() {
                            let child_width = match self.arena.kind(first) {
                                Node::Emphasis => Some(1u8),
                                Node::Strong => Some(2u8),
                                _ => None,
                            };
                            if let Some(child_width) = child_width {
                                let mut extended = chain.clone();
                                extended.push(child_width);
                                if canonical_run(extended.iter().copied().sum()) != extended {
                                    // The child's closer is followed by the
                                    // next sibling's first character, or by
                                    // this node's own closer when it is the
                                    // last child.
                                    let child_next =
                                        kids.get(1).and_then(|&sibling| self.boundary(sibling).0);
                                    let child_viable = match child_next {
                                        Some(c) => class_of(c) != Class::Other,
                                        None => closer_ok,
                                    };
                                    let self_viable =
                                        opener_ok && closer_ok && prev != Class::Other;
                                    if !child_viable && self_viable {
                                        ch = '_';
                                        chain = vec![width];
                                    }
                                }
                            }
                        }
                    }
                    // Closer side: the same check for the run closing here.
                    // Switching this node to the other marker splits the
                    // run, which leaves the enclosing closer preceded by
                    // punctuation; that is only safe when what follows the
                    // closer keeps it right-flanking, so an unusable
                    // switch keeps the merged run instead.
                    if let Some(t) = node_close_touch {
                        if t.ch == ch {
                            let mut widths = t.widths.clone();
                            widths.push(width);
                            if canonical_run(widths.iter().copied().sum()) == widths {
                                chain = widths;
                            } else if t.after.is_none_or(|c| class_of(c) != Class::Other)
                                && opener_ok
                            {
                                ch = if ch == '*' { '_' } else { '*' };
                                chain = vec![width];
                            } else {
                                chain = widths;
                            }
                        }
                    }
                    // A descendant emphasis whose opener directly follows
                    // non-whitespace could act as a closer for this node's
                    // opener when they share a character, closing this node
                    // early. In the source tree the two used different
                    // characters; keep them different by moving this node
                    // to the underscore.
                    if ch == '*'
                        && self.descendant_opener_after_content(id)
                        && opener_ok
                        && closer_ok
                    {
                        ch = '_';
                        chain = vec![width];
                    }
                    // Adjacent siblings must not run their delimiters
                    // together: `*a**b*` re-parses as one mangled
                    // emphasis, so one of the pair must switch. Prefer
                    // switching here when the next node's own switch would
                    // not survive (its closer would sit right before a
                    // letter).
                    if ch == '*' && opener_ok {
                        if let Some(&next_sibling) = ids.get(index + 1) {
                            if matches!(
                                self.arena.kind(next_sibling),
                                Node::Emphasis | Node::Strong
                            ) {
                                let after_next = ids
                                    .get(index + 2)
                                    .and_then(|&far| self.boundary(far).0)
                                    .or(close);
                                let next_viable =
                                    after_next.is_none_or(|c| class_of(c) != Class::Other);
                                if !next_viable {
                                    ch = '_';
                                    chain = vec![width];
                                }
                            }
                        }
                    }
                    if prev_close == Some(ch) && closer_ok {
                        ch = if ch == '*' { '_' } else { '*' };
                        chain = vec![width];
                    }
                    let markers = ch.to_string().repeat(usize::from(width));
                    out.push_str(&markers);
                    let after = next;
                    let child_touch = Touch {
                        ch,
                        widths: chain.clone(),
                        after: None,
                    };
                    let child_close = Touch {
                        ch,
                        widths: chain,
                        after,
                    };
                    self.inlines(
                        &kids,
                        Class::Punct,
                        true,
                        Some(&child_touch),
                        Some(&child_close),
                        Some(ch),
                        false,
                        single_line,
                        out,
                    );
                    out.push_str(&markers);
                    // A space after this node's closer is fine as-is; only
                    // the position right after an opener (passed to the
                    // children above) needs the encoded form.
                    prev_close = Some(ch);
                    prev_was_delim = false;
                }
                Node::Link(data) => {
                    if data.image {
                        out.push('!');
                    }
                    out.push('[');
                    let kids: Vec<NodeId> = self.arena.children(id).to_vec();
                    self.inlines(
                        &kids,
                        Class::Punct,
                        false,
                        None,
                        None,
                        Some(']'),
                        false,
                        single_line,
                        out,
                    );
                    out.push_str("](");
                    out.push_str(&encode_destination(&data.url));
                    if let Some(title) = &data.title {
                        out.push_str(" \"");
                        out.push_str(&encode_title(title));
                        out.push('"');
                    }
                    out.push(')');
                    prev_close = None;
                    prev_was_delim = false;
                }
                Node::Extension(_) => {
                    let kids: Vec<NodeId> = self.arena.children(id).to_vec();
                    if !kids.is_empty() {
                        self.inlines(
                            &kids,
                            prev,
                            prev_was_delim,
                            None,
                            None,
                            next,
                            line_start,
                            single_line,
                            out,
                        );
                    }
                    prev_close = None;
                    prev_was_delim = false;
                }
                // Block kinds never appear among inlines; skip them rather
                // than emit broken lines.
                _ => {
                    prev_close = None;
                    prev_was_delim = false;
                }
            }
            let was_emphasis = matches!(self.arena.kind(id), Node::Emphasis | Node::Strong);
            if !was_emphasis {
                prev_close = None;
                prev_was_delim = false;
            }
            if !matches!(
                self.arena.kind(id),
                Node::Text(_) | Node::SoftBreak | Node::HardBreak
            ) {
                line_empty = false;
            }
            line_start = if matches!(self.arena.kind(id), Node::Text(_)) {
                line_start
            } else {
                self.ends_line(id)
            };
        }
    }
}

/// The maximal trailing run of `#` characters in a heading body must not be
/// readable as the closing sequence: when a space (or the content start)
/// precedes the run, escape the run so the backslash breaks it up.
fn guard_heading_hashes(body: String) -> String {
    let trimmed = body.len() - body.trim_end_matches('#').len();
    if trimmed == 0 {
        return body;
    }
    let content = &body[..body.len() - trimmed];
    let before = content
        .chars()
        .next_back()
        .is_none_or(|c| c == ' ' || c == '\t');
    if !before {
        return body;
    }
    let mut out = content.to_string();
    for _ in 0..trimmed {
        out.push('\\');
        out.push('#');
    }
    out
}

/// Picks the rule characters for a thematic break. After a paragraph with
/// no blank line, `---` would read as a setext underline; and on a list
/// item's marker line the rule characters would merge with the marker into
/// one same-character run that re-parses as a document-level rule (a line
/// like `- ---` is itself a thematic break), so the rule character must
/// differ from the marker character that starts the line.
fn thematic_rule(flp: &str, after_para: bool) -> &'static str {
    let marker = flp.chars().find(|c| *c == '-' || *c == '*');
    for candidate in ["---", "***", "___"] {
        let ch = candidate.chars().next().unwrap_or_default();
        if after_para && ch == '-' {
            continue;
        }
        if marker == Some(ch) {
            continue;
        }
        return candidate;
    }
    "___"
}

/// The longest run of `ch` anywhere in `text`.
fn longest_run(text: &str, ch: char) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for c in text.chars() {
        if c == ch {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// Wraps raw code span content in a backtick fence that cannot collide with
/// it, padding with one space on both sides when either edge is a backtick.
fn encode_code_span(content: &str) -> String {
    let fence_len = 1.max(longest_run(content, '`') + 1);
    let fence = '`'.to_string().repeat(fence_len);
    let pad = content.starts_with('`') || content.ends_with('`');
    if pad {
        format!("{fence} {content} {fence}")
    } else {
        format!("{fence}{content}{fence}")
    }
}

/// Re-encodes a cleaned (entity-decoded, backslash-unescaped) info string so
/// it parses back to the same value: backslashes double, entity-starting
/// ampersands and edge whitespace become numeric references.
fn encode_info(info: &str) -> String {
    let mut out = String::with_capacity(info.len());
    for (index, c) in info.char_indices() {
        match c {
            '\\' => out.push_str("\\\\"),
            '&' => push_ampersand(info, index, &mut out),
            ' ' | '\t' if index == 0 || index + c.len_utf8() == info.len() => {
                out.push_str(if c == ' ' { "&#32;" } else { "&#9;" });
            }
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
    out
}

/// Re-encodes a cleaned link destination: the pointy `<...>` form when the
/// value holds whitespace, parentheses or angle brackets (with `<`, `>`,
/// `\` escaped and edge whitespace encoded), backslash-escaped parens
/// otherwise.
fn encode_destination(dest: &str) -> String {
    let needs_pointy = dest.is_empty()
        || dest.chars().any(|c| {
            matches!(c, '(' | ')' | '<' | '>' | '\n' | '\r') || is_space(c) || c.is_ascii_control()
        });
    let mut body = String::with_capacity(dest.len());
    let chars: Vec<(usize, char)> = dest.char_indices().collect();
    for (position, &(index, c)) in chars.iter().enumerate() {
        match c {
            '\\' => body.push_str("\\\\"),
            '&' => push_ampersand(dest, index, &mut body),
            ' ' | '\t' if position == 0 || position + 1 == chars.len() => {
                body.push_str(if c == ' ' { "&#32;" } else { "&#9;" });
            }
            '\n' => body.push_str("&#10;"),
            '\r' => body.push_str("&#13;"),
            '<' if needs_pointy => body.push_str("\\<"),
            '>' if needs_pointy => body.push_str("\\>"),
            '(' | ')' if !needs_pointy => {
                body.push('\\');
                body.push(c);
            }
            _ => body.push(c),
        }
    }
    if needs_pointy {
        format!("<{body}>")
    } else {
        body
    }
}

/// Re-encodes a cleaned link title in double quotes: backslashes and quotes
/// escaped, entity-starting ampersands and line breaks encoded.
fn encode_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for (index, c) in title.char_indices() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '&' => push_ampersand(title, index, &mut out),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
    out
}

/// Pushes `&` as `&amp;` when the following character could start an entity
/// reference, and as a plain ampersand otherwise.
fn push_ampersand(source: &str, index: usize, out: &mut String) {
    let rest = &source[index + 1..];
    let risky = rest
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '#');
    if risky {
        out.push_str("&amp;");
    } else {
        out.push('&');
    }
}

#[allow(clippy::too_many_arguments)]
/// Escapes decoded text so it re-parses to the same value: backslashes and
/// backticks always; `*` and `_` when they could open or close emphasis at
/// their position; brackets always; `<` as `\<`; ampersands that could
/// start an entity; `!` before a bracket. Embedded line breaks and leading
/// whitespace become numeric references, and the first printable character
/// of a line is escaped when it could start a block construct.
fn escape_text(
    value: &str,
    prev0: Class,
    next_bound: Option<char>,
    next_eats_space: bool,
    prev_after_delim: bool,
    at_start: &mut bool,
    line_empty: &mut bool,
    pending: &mut bool,
    out: &mut String,
) {
    let chars: Vec<char> = value.chars().collect();
    let mut prev = prev0;
    let mut prev_is_delim = prev_after_delim;
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        let next: Option<char> = chars.get(index + 1).copied().or(next_bound);
        let next2: Option<char> = chars.get(index + 2).copied();
        let next_class = next.map_or(Class::Edge, class_of);
        let rest_ws = chars[index + 1..].iter().all(|w| *w == ' ' || *w == '\t');
        let mut skip = 1usize;
        if *pending {
            // A previous text node ended with a line-start digit; this
            // node's first character is the marker punctuation that must
            // not complete a list marker.
            *pending = false;
            *at_start = false;
            *line_empty = false;
            out.push('\\');
            out.push(c);
            prev = class_of(c);
            prev_is_delim = c == '*' || c == '_';
            index += skip;
            continue;
        }
        if c == ' ' || c == '\t' {
            // A space the parser would not keep verbatim must travel as a
            // numeric reference: leading whitespace is stripped or
            // re-interpreted, trailing whitespace is stripped at the block
            // edge, a run before a hard break is swallowed by the break,
            // and a space next to an emphasis delimiter changes the
            // delimiter's flanking.
            let eaten = (rest_ws && next_eats_space)
                || *at_start
                || prev_is_delim
                || matches!(next, Some('\n') | Some('\r'));
            if eaten {
                out.push_str(if c == ' ' { "&#32;" } else { "&#9;" });
                prev = Class::Punct;
                prev_is_delim = false;
                // The rendered line now carries content, even though the
                // logical leading-whitespace run continues.
                *line_empty = false;
                index += skip;
                continue;
            }
            out.push(c);
            prev = Class::Ws;
            prev_is_delim = false;
            *at_start = false;
            *line_empty = false;
            index += skip;
            continue;
        }
        if *at_start && matches!(c, '-' | '+' | '*' | '_' | '=' | '#' | '>' | '~') {
            out.push('\\');
            out.push(c);
            *at_start = false;
            *line_empty = false;
            prev = class_of(c);
            prev_is_delim = c == '*' || c == '_';
            index += skip;
            continue;
        }
        if *at_start
            && c.is_ascii_digit()
            && matches!(next, Some('.') | Some(')'))
            && next2.is_none_or(|n| n == ' ' || n == '\t')
        {
            // A leading `1.` style marker would re-open an ordered list;
            // escaping the punctuation breaks the marker match. When the
            // punctuation lives in the next text node, only flag it.
            *at_start = false;
            *line_empty = false;
            if index + 1 < chars.len() {
                let marker = next.unwrap_or_default();
                out.push(c);
                out.push('\\');
                out.push(marker);
                prev = class_of(marker);
                prev_is_delim = false;
                skip = 2usize;
            } else {
                out.push(c);
                *pending = true;
                prev = class_of(c);
                prev_is_delim = false;
            }
            index += skip;
            continue;
        }
        *at_start = false;
        *line_empty = false;
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '*' | '_' => {
                if could_flank(c, prev, next_class) {
                    out.push('\\');
                }
                out.push(c);
            }
            '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            '<' => out.push_str("\\<"),
            '&' => {
                let risky = matches!(next, Some(n) if n.is_ascii_alphanumeric() || n == '#');
                if risky {
                    out.push_str("&amp;");
                } else {
                    out.push('&');
                }
            }
            '\n' => {
                // Embedded newlines stay invisible in the output: a blank
                // line would split the paragraph, and even a single one
                // would change line-start context.
                out.push_str("&#10;");
                prev = Class::Punct;
                prev_is_delim = false;
                index += skip;
                continue;
            }
            '\r' => {
                out.push_str("&#13;");
                prev = Class::Punct;
                prev_is_delim = false;
                index += skip;
                continue;
            }
            '!' => {
                if next == Some('[') {
                    out.push_str("\\!");
                } else {
                    out.push('!');
                }
            }
            _ => out.push(c),
        }
        prev = class_of(c);
        prev_is_delim = c == '*' || c == '_';
        index += skip;
    }
}
