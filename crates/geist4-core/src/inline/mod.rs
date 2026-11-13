//! Phase 2 of the parser: inline content.
//!
//! The implementation follows the reference algorithm's inline phase: a
//! single left-to-right scan over the block's text that emits inline nodes
//! into a linked list, records emphasis delimiters and link brackets as it
//! goes, and resolves both stacks at the end. Emphasis handling mirrors the
//! delimiter-stack algorithm exactly, including the "rule of three" and the
//! per-delimiter-class opener boundaries that keep lookups linear.

mod entities;
pub(crate) mod fold;
mod unicode;

use std::collections::HashMap;

use crate::node::{LinkDef, Node};
use unicode::{is_ascii_punct_byte, is_punct_or_symbol, is_space};

/// The longest link label the inline phase will scan.
const MAX_LINK_LABEL_LENGTH: usize = 1000;

/// An inline node, ready to become an arena child.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InlineNode {
    /// The node itself; strings are owned.
    pub node: Node<'static>,
    /// The node's children, in order, for container inlines.
    pub children: Vec<InlineNode>,
}

/// An inline node in the parser's working pool.
struct Cell {
    alive: bool,
    kind: Kind,
    prev: Option<u32>,
    next: Option<u32>,
    first_child: Option<u32>,
    last_child: Option<u32>,
}

/// What a [`Cell`] holds.
enum Kind {
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
    /// A link or image; children are its content.
    Link {
        url: String,
        title: Option<String>,
        image: bool,
    },
}

/// One emphasis delimiter run.
struct DelimCell {
    alive: bool,
    ch: u8,
    can_open: bool,
    can_close: bool,
    length: usize,
    /// The position in the input the run starts at.
    position: usize,
    /// The text node holding the delimiter characters.
    node: u32,
    prev: Option<u32>,
    next: Option<u32>,
}

/// The delimiter stack, a doubly-linked list walked from the tail.
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
    /// The text node holding `[` or `![`.
    node: u32,
    /// The input position of the bracket's start.
    position: usize,
    /// Whether another bracket opened between this one and the current
    /// position.
    bracket_after: bool,
}

/// The inline parser state.
struct Parser<'defs, 'a> {
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
    link_defs: &'defs HashMap<String, LinkDef<'a>>,
}

/// Parses one block's inline content.
///
/// Trailing whitespace comes off the subject first, as the reference
/// parser rtrims each block's inline text. `link_defs` holds the
/// document's reference definitions; every leaf block sees them all.
pub(crate) fn parse_inlines(
    text: &str,
    link_defs: &HashMap<String, LinkDef<'_>>,
) -> Vec<InlineNode> {
    let text = text.trim_end_matches([' ', '\t', '\u{b}', '\u{c}', '\r', '\n']);
    let mut parser = Parser {
        input: text,
        bytes: text.as_bytes(),
        pos: 0,
        pool: Vec::new(),
        head: None,
        tail: None,
        delims: DelimList {
            cells: Vec::new(),
            last: None,
        },
        brackets: Vec::new(),
        no_link_openers: true,
        skip_comment: false,
        skip_cdata: false,
        skip_declaration: false,
        skip_pi: false,
        link_defs,
    };
    parser.run();
    let pool = std::mem::take(&mut parser.pool);
    let head = parser.head;
    to_nodes(&pool, head)
}

/// Converts the surviving pool cells of one list into inline nodes.
fn to_nodes(pool: &[Cell], head: Option<u32>) -> Vec<InlineNode> {
    let mut out = Vec::new();
    let mut id = head;
    while let Some(current) = id {
        let cell = &pool[current as usize];
        if cell.alive {
            out.push(to_node(pool, current));
        }
        id = cell.next;
    }
    out
}

/// Converts one pool cell and its children.
fn to_node(pool: &[Cell], id: u32) -> InlineNode {
    let cell = &pool[id as usize];
    let node = match &cell.kind {
        Kind::Text(text) => Node::Text(text.clone().into()),
        Kind::Code(text) => Node::CodeSpan(text.clone().into()),
        Kind::Html(text) => Node::InlineHtml(text.clone().into()),
        Kind::SoftBreak => Node::SoftBreak,
        Kind::HardBreak => Node::HardBreak,
        Kind::Emph => Node::Emphasis,
        Kind::Strong => Node::Strong,
        Kind::Link { url, title, image } => Node::Link(crate::node::LinkData {
            url: url.clone().into(),
            title: title.clone().map(std::borrow::Cow::Owned),
            image: *image,
        }),
    };
    let children = to_nodes(pool, cell.first_child);
    InlineNode { node, children }
}

impl<'defs, 'a> Parser<'defs, 'a> {
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
                        let id = self.push(Kind::Text("![".to_string()));
                        self.pos += 2;
                        self.push_bracket(true, id);
                    } else {
                        self.pos += 1;
                        self.push(Kind::Text("!".to_string()));
                    }
                }
                b']' => self.handle_close_bracket(),
                _ => self.handle_text(),
            }
        }
        self.process_emphasis(0);
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
        });
        if let Some(prev) = self.tail {
            self.pool[prev as usize].next = Some(id);
        } else {
            self.head = Some(id);
        }
        self.tail = Some(id);
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

    /// Removes a cell from its list without freeing it.
    fn unlink(&mut self, id: u32) {
        let (prev, next) = {
            let cell = &self.pool[id as usize];
            (cell.prev, cell.next)
        };
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

    /// Marks a cell dead and drops it from its list. Containers that held
    /// this node as their first or last child are updated to hold its
    /// former neighbor instead, so a killed delimiter text node does not
    /// sever a container's child chain.
    fn kill(&mut self, id: u32) {
        let (prev, next) = {
            let cell = &self.pool[id as usize];
            (cell.prev, cell.next)
        };
        self.unlink(id);
        for cell in self.pool.iter_mut() {
            if cell.first_child == Some(id) {
                cell.first_child = next;
            }
            if cell.last_child == Some(id) {
                cell.last_child = prev;
            }
        }
        self.pool[id as usize].alive = false;
    }

    /// Pushes a bracket onto the bracket stack.
    fn push_bracket(&mut self, image: bool, node: u32) {
        if let Some(last) = self.brackets.last_mut() {
            last.bracket_after = true;
        }
        self.brackets.push(Bracket {
            image,
            node,
            position: self.pos,
            bracket_after: false,
        });
        if !image {
            self.no_link_openers = false;
        }
    }

    /// Scans plain text up to the next special character.
    fn handle_text(&mut self) {
        let start = self.pos;
        let mut end = self.pos + 1;
        while end < self.bytes.len() && !SPECIAL_CHARS[self.bytes[end] as usize] {
            end += 1;
        }
        let mut text = self.input[start..end].to_string();
        // A text run ending at a newline loses its trailing spaces and tabs.
        if end < self.bytes.len() && matches!(self.bytes[end], b'\n' | b'\r') {
            while text.ends_with(' ') || text.ends_with('\t') {
                text.pop();
            }
        }
        self.pos = end;
        self.push(Kind::Text(text));
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
        let hard = nlpos >= 2 && self.bytes[nlpos - 1] == b' ' && self.bytes[nlpos - 2] == b' ';
        self.push(if hard {
            Kind::HardBreak
        } else {
            Kind::SoftBreak
        });
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
                self.push(Kind::Text((next as char).to_string()));
            }
            Some(b'\r') | Some(b'\n') => {
                self.handle_newline_raw();
                self.push(Kind::HardBreak);
            }
            _ => {
                self.push(Kind::Text("\\".to_string()));
            }
        }
    }

    /// Consumes a line ending at the cursor for a backslash break.
    fn handle_newline_raw(&mut self) {
        if self.bytes.get(self.pos) == Some(&b'\r') {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
        }
        while matches!(self.bytes.get(self.pos), Some(b' ') | Some(b'\t')) {
            self.pos += 1;
        }
    }

    /// Handles a code span or a run of literal backticks.
    fn handle_backticks(&mut self) {
        let start = self.pos;
        let mut numticks = 0usize;
        while self.bytes.get(self.pos) == Some(&b'`') {
            self.pos += 1;
            numticks += 1;
        }
        if let Some(closer) = self.scan_to_closing_backticks(numticks) {
            let content = &self.input[start + numticks..closer - numticks];
            let normalized = normalize_code(content);
            self.pos = closer;
            self.push(Kind::Code(normalized));
        } else {
            // No closer: the opening run is literal text, and the scan
            // resumes after it.
            self.pos = start + numticks;
            self.push(Kind::Text("`".repeat(numticks)));
        }
    }

    /// Scans forward for a backtick run of exactly `length` backticks.
    /// Returns the position just past the closing run, or `None`.
    fn scan_to_closing_backticks(&mut self, length: usize) -> Option<usize> {
        let mut pos = self.pos;
        while pos < self.bytes.len() {
            if self.bytes[pos] == b'`' {
                let run_start = pos;
                while self.bytes.get(pos) == Some(&b'`') {
                    pos += 1;
                }
                if pos - run_start == length {
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
        match entities::scan_entity(&self.input[self.pos..]) {
            Some((text, len)) => {
                self.pos += len;
                self.push(Kind::Text(text));
            }
            None => {
                self.push(Kind::Text("&".to_string()));
            }
        }
    }

    /// Scans an emphasis delimiter run and records it.
    fn handle_delim(&mut self, ch: u8) {
        let (numdelims, can_open, can_close) = self.scan_delims(ch);
        let text = self.input[self.pos - numdelims..self.pos].to_string();
        let id = self.push(Kind::Text(text));
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
                prev: None,
                next: None,
            });
        }
    }

    /// Measures a delimiter run at the cursor, applying the flanking rules.
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

    /// The character before `pos`, or a newline stand-in at the start.
    fn char_before(&self, pos: usize) -> Option<char> {
        if pos == 0 {
            return Some('\n');
        }
        let bytes = &self.bytes[..pos];
        // Step back over UTF-8 continuation bytes.
        let mut start = pos - 1;
        while start > 0 && bytes[start] & 0xC0 == 0x80 {
            start -= 1;
        }
        std::str::from_utf8(&bytes[start..])
            .ok()
            .and_then(|s| s.chars().next())
            .or(Some('\n'))
    }

    /// The character at `pos`, or a newline stand-in at the end of the
    /// input, as the reference parser's decoder yields.
    fn char_at(&self, pos: usize) -> Option<char> {
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
        let id = self.push(Kind::Text("[".to_string()));
        self.pos += 1;
        self.push_bracket(false, id);
    }

    /// Closes a bracket: an inline link, a reference link, or plain text.
    fn handle_close_bracket(&mut self) {
        self.pos += 1;
        let initial_pos = self.pos;
        let Some(opener_index) = (self.brackets.len()).checked_sub(1) else {
            self.push(Kind::Text("]".to_string()));
            return;
        };
        let is_image = self.brackets[opener_index].image;
        if !is_image && self.no_link_openers {
            self.brackets.pop();
            self.push(Kind::Text("]".to_string()));
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

        if matched.is_none() {
            // A reference link: a following label or the link text itself.
            self.pos = after_link_text_pos;
            let mut raw_label: Option<String> = None;
            if let Some((label, len)) = scan_link_label(self.bytes, self.pos) {
                self.pos += len;
                raw_label = Some(label);
            } else {
                self.pos = initial_pos;
            }
            let shortcut = raw_label.as_ref().is_none_or(String::is_empty)
                && !self.brackets[opener_index].bracket_after;
            if shortcut {
                // The link text itself becomes the label.
                let text_start = self.brackets[opener_index].position;
                raw_label = Some(self.input[text_start..initial_pos - 1].to_string());
            }
            if let Some(label) = raw_label {
                let key = normalize_label(&label);
                if let Some(def) = self.link_defs.get(&key) {
                    matched = Some((
                        def.destination.to_string(),
                        def.title.as_ref().map(|t| t.to_string()),
                    ));
                }
            }
        }

        match matched {
            Some((url, title)) => {
                let opener_node = self.brackets[opener_index].node;
                let opener_position = self.brackets[opener_index].position;
                let link_id = self.alloc(Kind::Link {
                    url,
                    title,
                    image: is_image,
                });
                // Move every node after the opener's text node into the
                // link, as the reference parser does.
                let first = self.pool[opener_node as usize].next;
                let last = self.tail;
                if let Some(f) = first {
                    self.pool[link_id as usize].first_child = Some(f);
                    self.pool[link_id as usize].last_child = last;
                    self.pool[f as usize].prev = None;
                    if let Some(l) = last {
                        self.pool[l as usize].next = None;
                    }
                }
                self.pool[opener_node as usize].next = None;
                self.tail = Some(opener_node);
                self.insert_before(link_id, opener_node);
                self.kill(opener_node);
                self.process_emphasis(opener_position);
                self.brackets.pop();
                if !is_image {
                    self.no_link_openers = true;
                }
            }
            None => {
                self.brackets.pop();
                self.pos = initial_pos;
                self.push(Kind::Text("]".to_string()));
            }
        }
    }

    /// Resolves the delimiter stack from `stack_bottom` onward.
    fn process_emphasis(&mut self, stack_bottom: usize) {
        let mut openers_bottom = [stack_bottom; 15];
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
            // Look backwards for the first matching opener.
            let mut opener = self.delims.cells[closer_id as usize].prev;
            let mut opener_found = false;
            let mut opener_id = None;
            while let Some(o) = opener {
                let cell = &self.delims.cells[o as usize];
                if cell.position < openers_bottom[openers_bottom_index] {
                    break;
                }
                if cell.can_open
                    && cell.ch == closer_ch
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
            if closer_ch == b'*' || closer_ch == b'_' {
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
                openers_bottom[openers_bottom_index] = closer_position;
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
        let new_opener_len = opener_len - use_delims;
        let new_closer_len = closer_len - use_delims;
        self.set_text_len(opener_node, new_opener_len);
        self.set_text_len(closer_node, new_closer_len);
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
        // Move the nodes between the two text nodes into the emphasis.
        let first = self.pool[opener_node as usize].next;
        let last = self.pool[closer_node as usize].prev;
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
        if let Some(len) = scan_autolink_uri(self.bytes, self.pos) {
            let contents = self.input[self.pos..self.pos + len - 1].to_string();
            self.pos += len;
            let url = clean_autolink(&contents, false);
            let text = entities::unescape_html_f(&contents);
            self.append_autolink(url, text);
            return;
        }
        if let Some(len) = scan_autolink_email(self.bytes, self.pos) {
            let contents = self.input[self.pos..self.pos + len - 1].to_string();
            self.pos += len;
            let url = clean_autolink(&contents, true);
            let text = entities::unescape_html_f(&contents);
            self.append_autolink(url, text);
            return;
        }
        if let Some(len) = self.scan_inline_html() {
            let raw = self.input[self.pos - 1..self.pos - 1 + len].to_string();
            self.pos += len - 1;
            self.push(Kind::Html(raw));
        } else {
            self.push(Kind::Text("<".to_string()));
        }
    }

    /// Builds an autolink node with its single text child and appends it.
    fn append_autolink(&mut self, url: String, text: String) {
        let link = self.alloc(Kind::Link {
            url,
            title: None,
            image: false,
        });
        let text_id = self.alloc(Kind::Text(text));
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

/// Normalizes code span content: line endings become spaces, and one
/// leading and trailing space come off when the content is not all spaces.
fn normalize_code(content: &str) -> String {
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

/// Normalizes a link label for lookup; shared with the block parser so
/// definitions and uses always agree.
use crate::parser::normalize_label;

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

/// Cleans a link destination: trims whitespace, decodes entities, then
/// removes backslash escapes.
pub(crate) fn clean_url(url: &str) -> String {
    let trimmed =
        url.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\u{b}' | '\u{c}' | '\r' | '\n'));
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
        (Some(&b'\''), Some(&b'\'')) | (Some(&b'('), Some(&b')')) | (Some(&b'"'), Some(&b'"')) => {
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
    let trimmed =
        url.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\u{b}' | '\u{c}' | '\r' | '\n'));
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
        // Runs of non-dash characters, a dash followed by a non-dash, or
        // two dashes followed by anything but `>`.
        if bytes[i] != b'-' {
            while i < len && bytes[i] != b'-' {
                i += 1;
            }
        } else if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            match bytes.get(i + 2) {
                Some(&b'>') => return Some(i - start),
                Some(&b'-') => return None,
                Some(_) => i += 3,
                None => return None,
            }
        } else if bytes.get(i + 1).is_some_and(|&b| b != b'-') {
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
    while i < len && bytes[i] != b'>' {
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
        if bytes[i] != b']' {
            while i < len && bytes[i] != b']' {
                i += 1;
            }
        } else if bytes.get(i + 1) == Some(&b']') {
            match bytes.get(i + 2) {
                Some(&b'>') => return Some(i - start),
                Some(_) => i += 3,
                None => return None,
            }
        } else if bytes.get(i + 1).is_some_and(|&b| b != b']') {
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
            while i < len && bytes[i] != q {
                i += 1;
            }
            if i < len { Some(i + 1 - start) } else { None }
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
