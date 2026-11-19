//! The exporter: the Geist tree in, mdast nodes out.
//!
//! Positions come from the nodes' source spans plus small scans of the
//! source, replicating the reference's token extents: a paragraph extends
//! over its last line's trailing whitespace, a heading over its ATX line
//! or setext underline, a code block through its closing fence, and list
//! items and block quotes end with their last child.

use geist4_core::Document;
use geist4_core::arena::NodeId;
use geist4_core::node::{
    CodeBlockData, HeadingData, HtmlBlockData, LinkData, LinkRefKind, ListData, Node, SpannedDef,
};
use geist4_core::span::Span;

use crate::nodes::{
    Break, COLLAPSED, Code, Definition, Emphasis, FULL, Html, Image, ImageReference, InlineCode,
    Link, LinkReference, List, ListItem, Mdast, Paragraph, Root, SHORTCUT, Strong, Text,
    ThematicBreak,
};
use crate::{LineIndex, Position, position_for};

/// Exports a parsed document as an mdast tree.
///
/// `src` is the exact source the document was parsed from: positions
/// point back into it. The result is a pure tree; serialize it with
/// `serde_json` for the reference implementation's exact byte shape.
#[must_use]
pub fn to_mdast(src: &str, doc: &Document<'_>) -> Mdast {
    let lines = LineIndex::new(src);
    let cx = Cx {
        src,
        bytes: src.as_bytes(),
        lines: &lines,
        doc,
    };
    let children = cx.block_children(doc.root);
    Mdast::Root(Root {
        type_: "root",
        children,
        position: cx.position(0, src.len()),
    })
}

/// The source context of one export.
struct Cx<'a> {
    src: &'a str,
    bytes: &'a [u8],
    lines: &'a LineIndex,
    doc: &'a Document<'a>,
}

/// The inline export of one leaf's children: the mdast nodes, plus the
/// source offsets of the first and last exported character.
struct Inlines {
    nodes: Vec<Mdast>,
    first: Option<usize>,
    last: Option<usize>,
}

impl<'a> Cx<'a> {
    /// The `position` for the byte span `[start, end)`.
    fn position(&self, start: usize, end: usize) -> Position {
        position_for(self.src, self.lines, start, end)
    }

    /// The node's span.
    fn span(&self, id: NodeId) -> Span {
        self.doc.arena.span(id)
    }

    /// The node's kind.
    fn kind(&self, id: NodeId) -> &Node<'a> {
        self.doc.arena.kind(id)
    }

    /// Whether the kind splices into its parent's children, dropping the
    /// node itself: the document root and transparent extension nodes.
    /// Block quotes, lists and items wrap their children and keep their
    /// own nodes.
    fn is_splice(kind: &Node<'a>) -> bool {
        matches!(kind, Node::Document | Node::Extension(_))
    }

    /// Whether the kind can hold block children, and so can be a link
    /// reference definition's home: the document root, block quotes,
    /// lists, items and transparent extension nodes.
    fn is_container(kind: &Node<'a>) -> bool {
        matches!(
            kind,
            Node::Document | Node::BlockQuote | Node::List(_) | Node::Item(_) | Node::Extension(_)
        )
    }

    /// Exports one container's children, interleaving the link reference
    /// definitions homed here among them in source order.
    fn block_children(&self, id: NodeId) -> Vec<Mdast> {
        let defs = self.defs_homed_at(id);
        let kids = self.doc.arena.children(id);
        let mut out = Vec::with_capacity(kids.len() + defs.len());
        let mut next = 0usize;
        for &kid in kids {
            let kid_start = self.span(kid).start;
            while next < defs.len() && defs[next].span.start <= kid_start {
                out.push(self.def_node(defs[next]));
                next += 1;
            }
            if Self::is_splice(self.kind(kid)) {
                out.extend(self.block_children(kid));
            } else {
                out.push(self.block_entry(kid));
            }
        }
        for &def in &defs[next..] {
            out.push(self.def_node(def));
        }
        out
    }

    /// The link reference definitions whose home is this container: the
    /// deepest container chain that encloses the definition's span. When
    /// several siblings' spans enclose one definition (list items' spans
    /// reach into their successors), the last one wins.
    fn defs_homed_at(&self, id: NodeId) -> Vec<&'a SpannedDef<'a>> {
        self.doc
            .defs
            .iter()
            .filter(|def| self.def_home(def) == id)
            .collect()
    }

    /// The container a definition belongs under, walking down from the
    /// root through the containers whose spans enclose the definition.
    fn def_home(&self, def: &SpannedDef<'a>) -> NodeId {
        let mut current = self.doc.root;
        loop {
            let mut best: Option<(NodeId, usize)> = None;
            for &kid in self.doc.arena.children(current) {
                if !Self::is_container(self.kind(kid)) {
                    continue;
                }
                let span = self.span(kid);
                if span.start <= def.span.start
                    && def.span.end <= span.end
                    && best.is_none_or(|(_, start)| span.start > start)
                {
                    best = Some((kid, span.start));
                }
            }
            match best {
                Some((kid, _)) => current = kid,
                None => return current,
            }
        }
    }

    /// One block child as mdast: a container's own node or a leaf's.
    fn block_entry(&self, id: NodeId) -> Mdast {
        match self.kind(id) {
            Node::BlockQuote => self.block_quote(id),
            Node::List(_) => self.list(id),
            Node::Item(_) => self.item(id),
            _ => self.block_node(id),
        }
    }

    /// One block leaf as mdast.
    fn block_node(&self, id: NodeId) -> Mdast {
        match self.kind(id) {
            Node::Paragraph => self.paragraph(id),
            Node::Heading(data) => self.heading(id, data),
            Node::CodeBlock(data) => self.code_block(id, data),
            Node::HtmlBlock(data) => self.html_block(id, data),
            Node::ThematicBreak => self.thematic_break(id),
            kind => unreachable!("unexpected block kind {kind:?}"),
        }
    }

    /// A block quote: its children, spanning the quote's own span — the
    /// `>` markers' lines through the last line its flow consumed.
    fn block_quote(&self, id: NodeId) -> Mdast {
        let span = self.span(id);
        Mdast::BlockQuote(crate::nodes::BlockQuote {
            type_: "blockquote",
            children: self.block_children(id),
            position: self.position(span.start, span.end),
        })
    }

    /// A list: `ordered` and `start` from the list data, `spread` when a
    /// blank line sits right before any item's marker, spanning to the
    /// last item.
    fn list(&self, id: NodeId) -> Mdast {
        let span = self.span(id);
        let ListData { ordered, start, .. } = match self.kind(id) {
            Node::List(data) => data,
            _ => unreachable!("list kind checked"),
        };
        let end = self.extend_list_end(id, span.end);
        Mdast::List(List {
            type_: "list",
            ordered: *ordered,
            start: ordered.then(|| *start),
            spread: self.list_spread(id),
            children: self.block_children(id),
            position: self.position(span.start, end),
        })
    }

    /// Whether the list is spread: a blank line at the list's own level in
    /// the gap between two consecutive items' spans. Lines inside an item
    /// (a block quote's blank continuation, an extracted definition) do
    /// not spread the list; trailing blank lines after the last item do
    /// not either.
    fn list_spread(&self, id: NodeId) -> bool {
        let kids = self.doc.arena.children(id);
        for pair in kids.windows(2) {
            let gap = Span::new(self.span(pair[0]).end, self.span(pair[1]).start);
            if gap.start < gap.end && self.has_container_blank_line(gap) {
                return true;
            }
        }
        false
    }

    /// A list item: `spread` from the blank lines between its children,
    /// `checked` always `None` (task lists land with their extension),
    /// spanning the item's own span: its marker through its last
    /// consumed content.
    fn item(&self, id: NodeId) -> Mdast {
        let span = self.span(id);
        let _ = match self.kind(id) {
            Node::Item(data) => data,
            _ => unreachable!("item kind checked"),
        };
        Mdast::ListItem(ListItem {
            type_: "listItem",
            spread: self.item_spread(id),
            checked: None,
            children: self.block_children(id),
            position: self.position(span.start, span.end),
        })
    }

    /// The mdast end of a list item with no content: its marker plus the
    /// single padding column the parse keeps, when the marker's line has
    /// one.
    fn empty_item_end(&self, span: Span) -> usize {
        let line_end = self.line_content_end(span.start).min(span.end);
        let mut i = span.start;
        if matches!(self.bytes.get(i), Some(b'-') | Some(b'+') | Some(b'*')) {
            i += 1;
        } else {
            while matches!(self.bytes.get(i), Some(b) if b.is_ascii_digit()) {
                i += 1;
            }
            if matches!(self.bytes.get(i), Some(b'.') | Some(b')')) {
                i += 1;
            }
        }
        if matches!(self.bytes.get(i), Some(b' ') | Some(b'\t')) {
            i += 1;
        }
        i.min(line_end).max(span.start)
    }

    /// Whether a blank line separates any two of the item's children:
    /// the reference marks an item spread when a blank line falls between
    /// its own blocks, not when one separates it from its siblings. The
    /// children run in output order, with the item's homed reference
    /// definitions spliced in by span start, so a blank line before a
    /// definition spreads the item too. A line holding only container
    /// prefixes and whitespace (the `>`s of enclosing block quotes) is
    /// blank at the item's own level.
    fn item_spread(&self, id: NodeId) -> bool {
        let kids = self.doc.arena.children(id);
        let defs = self.defs_homed_at(id);
        let mut prev_end: Option<usize> = None;
        let mut next_def = 0usize;
        for &kid in kids {
            let kid_start = self.span(kid).start;
            while next_def < defs.len() && defs[next_def].span.start <= kid_start {
                let def = &defs[next_def];
                if self.spread_gap(prev_end, def.span.start) {
                    return true;
                }
                prev_end = Some(def.span.end);
                next_def += 1;
            }
            if self.spread_gap(prev_end, kid_start) {
                return true;
            }
            prev_end = Some(self.subtree_end(kid).unwrap_or_else(|| self.span(kid).end));
        }
        for &def in &defs[next_def..] {
            if self.spread_gap(prev_end, def.span.start) {
                return true;
            }
            prev_end = Some(def.span.end);
        }
        false
    }

    /// Whether the gap between two children's extents holds a blank line
    /// at the container's own level.
    fn spread_gap(&self, prev_end: Option<usize>, next_start: usize) -> bool {
        match prev_end {
            Some(prev_end) if prev_end < next_start => {
                self.has_container_blank_line(Span::new(prev_end, next_start))
            }
            _ => false,
        }
    }

    /// Whether a line that is blank at this container level — only
    /// whitespace and container prefixes — starts within
    /// `[span.start, span.end)`.
    fn has_container_blank_line(&self, span: Span) -> bool {
        let mut index = self.lines.line_containing(span.start);
        if self.lines.line_start(index) != span.start {
            index += 1;
        }
        while index < self.lines.line_count() && self.lines.line_start(index) < span.end {
            let start = self.lines.line_start(index);
            let end = self.line_content_end(start);
            if self.bytes[start..end]
                .iter()
                .all(|byte| is_blank_byte(*byte) || *byte == b'>')
            {
                return true;
            }
            index += 1;
        }
        false
    }

    /// A paragraph: its merged inline children, spanning from the first
    /// child to the end of its last line's content. The reference's
    /// paragraph position covers exactly the text that survives
    /// definition extraction, so it derives from the inline children, not
    /// from the parse's own span.
    fn paragraph(&self, id: NodeId) -> Mdast {
        let span = self.span(id);
        let inl = self.inlines(id);
        let start = inl.first.unwrap_or(span.start);
        let end = inl.last.map_or_else(
            || self.line_content_end(span.start),
            |last| self.line_content_end(last),
        );
        Mdast::Paragraph(Paragraph {
            type_: "paragraph",
            children: inl.nodes,
            position: self.position(start, end),
        })
    }

    /// A heading: `depth` from the level, spanning the heading's own span:
    /// an ATX line from its `#` to the line's char end, or, for setext,
    /// the paragraph's content start through the underline's char end.
    fn heading(&self, id: NodeId, data: &HeadingData) -> Mdast {
        let span = self.span(id);
        let inl = self.inlines(id);
        Mdast::Heading(crate::nodes::Heading {
            type_: "heading",
            depth: data.level,
            children: inl.nodes,
            position: self.position(span.start, span.end),
        })
    }

    /// A code block: the literal without its final newline, spanning the
    /// block's own span — its first byte through its last consumed line's
    /// char end (a closing fence's, or past the final newline at the
    /// source's end for an unclosed fence).
    fn code_block(&self, id: NodeId, data: &CodeBlockData<'a>) -> Mdast {
        let span = self.span(id);
        let value = data.literal.strip_suffix('\n').unwrap_or(&data.literal);
        let (lang, meta) = split_info(data.info.as_deref());
        Mdast::Code(Code {
            type_: "code",
            lang,
            meta,
            value: value.to_string(),
            position: self.position(span.start, span.end),
        })
    }

    /// A raw HTML block: the literal without its final newline, spanning
    /// the block's own span — its first byte through its last line's char
    /// end.
    fn html_block(&self, id: NodeId, data: &HtmlBlockData<'a>) -> Mdast {
        let span = self.span(id);
        let value = data.literal.strip_suffix('\n').unwrap_or(&data.literal);
        Mdast::Html(Html {
            type_: "html",
            value: value.to_string(),
            position: self.position(span.start, span.end),
        })
    }

    /// A thematic break: the marker character through the line's char end.
    fn thematic_break(&self, id: NodeId) -> Mdast {
        let span = self.span(id);
        Mdast::ThematicBreak(ThematicBreak {
            type_: "thematicBreak",
            position: self.position(span.start, span.end),
        })
    }

    /// One link reference definition as mdast. The label is the source
    /// text between the brackets, untrimmed across its lines, with
    /// escapes and character references resolved; the identifier is the
    /// normalized form of the raw label.
    fn def_node(&self, def: &SpannedDef<'a>) -> Mdast {
        Mdast::Definition(Definition {
            type_: "definition",
            identifier: geist4_core::parser::normalize_label(&def.def.label),
            label: geist4_core::parser::decode_label(&self.raw_def_label(def)),
            title: def
                .titled
                .then(|| def.def.title.as_deref().map(str::to_string))
                .flatten(),
            url: def.def.destination.to_string(),
            position: self.position(def.span.start, def.span.end),
        })
    }

    /// The definition's label as the source has it, between the brackets:
    /// to the first unescaped `]`, untrimmed.
    fn raw_def_label(&self, def: &SpannedDef<'a>) -> String {
        let bytes = self.bytes;
        let mut i = def.span.start + 1;
        while i < def.span.end {
            match bytes[i] {
                b'\\' => i += 2,
                b']' => return self.src[def.span.start + 1..i].to_string(),
                _ => i += 1,
            }
        }
        def.def.label.to_string()
    }

    /// The mdast end of one block child, for its container's position:
    /// the child's own end, through the deepest splice.
    fn subtree_end(&self, id: NodeId) -> Option<usize> {
        match self.kind(id) {
            Node::List(_) => {
                let kids = self.doc.arena.children(id);
                match kids.last() {
                    Some(last) => Some(self.extend_list_end(id, self.subtree_end(*last)?)),
                    None => None,
                }
            }
            Node::Document | Node::BlockQuote | Node::Item(_) | Node::Extension(_) => {
                let kids = self.doc.arena.children(id);
                match kids.last() {
                    Some(last) => self.subtree_end(*last),
                    None => match self.kind(id) {
                        Node::Item(_) => Some(self.empty_item_end(self.span(id))),
                        _ => None,
                    },
                }
            }
            Node::Paragraph => {
                let span = self.span(id);
                Some(match self.inline_last_offset(id) {
                    Some(last) => self.line_content_end(last),
                    None => self.line_content_end(span.start),
                })
            }
            Node::Heading(data) => {
                let span = self.span(id);
                Some(if data.setext {
                    self.trim_line_ending(span.end)
                } else {
                    self.line_content_end(span.start)
                })
            }
            Node::CodeBlock(_) => Some(self.span(id).end),
            Node::HtmlBlock(_) => Some(self.trim_line_ending(self.span(id).end)),
            Node::ThematicBreak => Some(self.line_content_end(self.span(id).start)),
            _ => None,
        }
    }

    /// Trailing lines holding nothing but the list's enclosing block
    /// quotes' markers, whose source line a proper ancestor's frame
    /// claimed, stay inside the list's span: the ancestor's flow holds
    /// them, so the list extends to each line's content end. True blank
    /// lines never extend the list.
    fn extend_list_end(&self, id: NodeId, mut end: usize) -> usize {
        let mut index = self.lines.line_containing(end);
        if self.lines.line_start(index) < end {
            index += 1;
        }
        while index < self.lines.line_count() {
            let start = self.lines.line_start(index);
            let content_end = self.line_content_end(start);
            let line = &self.bytes[start..content_end];
            let marker_only = !line.is_empty()
                && line.contains(&b'>')
                && line.iter().all(|byte| matches!(byte, b'>' | b' ' | b'\t'));
            if !marker_only {
                break;
            }
            if !self.claimed_by_ancestor(id, content_end) {
                break;
            }
            end = content_end;
            index += 1;
        }
        end
    }

    /// Whether a proper ancestor's frame reaches `content_end`: the line
    /// ending there sits inside an ancestor's flow, so a list below it
    /// keeps the line. The document never claims a line.
    fn claimed_by_ancestor(&self, id: NodeId, content_end: usize) -> bool {
        let mut current = id;
        while let Some(parent) = self.doc.arena.parent(current) {
            if matches!(self.kind(parent), Node::Document) {
                return false;
            }
            if self.span(parent).end >= content_end {
                return true;
            }
            current = parent;
        }
        false
    }

    /// The source end of a leaf's last inline child, if any.
    fn inline_last_offset(&self, id: NodeId) -> Option<usize> {
        let mut last = None;
        for &kid in self.doc.arena.children(id) {
            match self.kind(kid) {
                Node::Text(_)
                | Node::SoftBreak
                | Node::HardBreak
                | Node::CodeSpan(_)
                | Node::InlineHtml(_)
                | Node::Emphasis
                | Node::Strong
                | Node::Link(_) => {
                    last = Some(self.span(kid).end);
                }
                Node::Extension(_) => {
                    last = self.inline_last_offset(kid).or(last);
                }
                _ => {}
            }
        }
        last
    }

    /// Exports one leaf's inline children: adjacent text and soft breaks
    /// merge into one `text` node, as the reference emits no soft-break
    /// node.
    fn inlines(&self, id: NodeId) -> Inlines {
        let mut nodes = Vec::new();
        let mut first = None;
        let mut last = None;
        fn track(start: usize, end: usize, first: &mut Option<usize>, last: &mut Option<usize>) {
            if first.is_none() {
                *first = Some(start);
            }
            *last = Some(end);
        }
        let kids = self.doc.arena.children(id);
        let mut i = 0usize;
        while i < kids.len() {
            let kid = kids[i];
            match self.kind(kid) {
                Node::Text(_) | Node::SoftBreak => {
                    let start = self.span(kid).start;
                    let mut value = String::new();
                    let mut end = start;
                    while i < kids.len() {
                        match self.kind(kids[i]) {
                            Node::Text(text) => {
                                value.push_str(text);
                                end = self.span(kids[i]).end;
                            }
                            Node::SoftBreak => {
                                // The break contributes the source's own
                                // line ending bytes: `\r` and `\r\n` stay
                                // themselves in the value.
                                let span = self.span(kids[i]);
                                value.push_str(self.line_ending_text(span.end));
                                end = span.end;
                            }
                            _ => break,
                        }
                        i += 1;
                    }
                    track(start, end, &mut first, &mut last);
                    nodes.push(Mdast::Text(Text {
                        type_: "text",
                        value,
                        position: self.position(start, end),
                    }));
                }
                Node::HardBreak => {
                    let span = self.span(kid);
                    track(span.start, span.end, &mut first, &mut last);
                    nodes.push(Mdast::Break(Break {
                        type_: "break",
                        position: self.position(span.start, span.end),
                    }));
                    i += 1;
                }
                Node::CodeSpan(value) => {
                    let span = self.span(kid);
                    track(span.start, span.end, &mut first, &mut last);
                    nodes.push(Mdast::InlineCode(InlineCode {
                        type_: "inlineCode",
                        // The reference's value: the raw content with one
                        // leading and trailing whitespace byte removed.
                        value: geist4_core::code_span_value(value),
                        position: self.position(span.start, span.end),
                    }));
                    i += 1;
                }
                Node::InlineHtml(value) => {
                    let span = self.span(kid);
                    track(span.start, span.end, &mut first, &mut last);
                    nodes.push(Mdast::Html(Html {
                        type_: "html",
                        value: value.to_string(),
                        position: self.position(span.start, span.end),
                    }));
                    i += 1;
                }
                Node::Emphasis => {
                    let span = self.span(kid);
                    track(span.start, span.end, &mut first, &mut last);
                    nodes.push(Mdast::Emphasis(Emphasis {
                        type_: "emphasis",
                        children: self.inlines(kid).nodes,
                        position: self.position(span.start, span.end),
                    }));
                    i += 1;
                }
                Node::Strong => {
                    let span = self.span(kid);
                    track(span.start, span.end, &mut first, &mut last);
                    nodes.push(Mdast::Strong(Strong {
                        type_: "strong",
                        children: self.inlines(kid).nodes,
                        position: self.position(span.start, span.end),
                    }));
                    i += 1;
                }
                Node::Link(data) => {
                    let span = self.span(kid);
                    track(span.start, span.end, &mut first, &mut last);
                    nodes.push(self.link_node(kid, data, span));
                    i += 1;
                }
                Node::Extension(_) => {
                    let sub = self.inlines(kid);
                    if first.is_none() {
                        first = sub.first;
                    }
                    if let Some(end) = sub.last {
                        last = Some(end);
                    }
                    nodes.extend(sub.nodes);
                    i += 1;
                }
                kind => unreachable!("unexpected inline kind {kind:?}"),
            }
        }
        Inlines { nodes, first, last }
    }

    /// One link or image as mdast: reference forms become
    /// `linkReference`/`imageReference`, images flatten their label to
    /// `alt`, and links keep their children.
    fn link_node(&self, id: NodeId, data: &LinkData<'a>, span: Span) -> Mdast {
        let position = self.position(span.start, span.end);
        let url = data.url.to_string();
        let title = data.title.as_deref().map(str::to_string);
        if data.image {
            let alt = self.alt_children(id);
            return match &data.reference {
                Some(reference) => Mdast::ImageReference(ImageReference {
                    type_: "imageReference",
                    alt,
                    position,
                    label: geist4_core::parser::decode_label(&reference.label),
                    identifier: geist4_core::parser::normalize_label(&reference.label),
                    reference_type: ref_type(reference.kind).to_string(),
                }),
                None => Mdast::Image(Image {
                    type_: "image",
                    title,
                    url,
                    alt,
                    position,
                }),
            };
        }
        let children = self.inlines(id).nodes;
        match &data.reference {
            Some(reference) => Mdast::LinkReference(LinkReference {
                type_: "linkReference",
                children,
                position,
                label: geist4_core::parser::decode_label(&reference.label),
                identifier: geist4_core::parser::normalize_label(&reference.label),
                reference_type: ref_type(reference.kind).to_string(),
            }),
            None => Mdast::Link(Link {
                type_: "link",
                title,
                url,
                children,
                position,
            }),
        }
    }

    /// An image or image reference's `alt`: the label flattened, with
    /// soft breaks as newlines, hard breaks dropped, code spans and raw
    /// HTML verbatim, and nested constructs recursed.
    fn alt_children(&self, id: NodeId) -> String {
        let mut out = String::new();
        for &kid in self.doc.arena.children(id) {
            self.alt_into(kid, &mut out);
        }
        out
    }

    /// Flattens one inline node into `out`.
    fn alt_into(&self, id: NodeId, out: &mut String) {
        match self.kind(id) {
            Node::Text(text) => out.push_str(text),
            Node::SoftBreak => out.push_str(self.line_ending_text(self.span(id).end)),
            // A hard break contributes nothing to `alt`.
            Node::HardBreak => {}
            Node::CodeSpan(value) => out.push_str(value),
            Node::InlineHtml(value) => out.push_str(value),
            Node::Emphasis | Node::Strong | Node::Link(_) | Node::Extension(_) => {
                for &kid in self.doc.arena.children(id) {
                    self.alt_into(kid, out);
                }
            }
            kind => unreachable!("unexpected alt kind {kind:?}"),
        }
    }

    /// The line ending that ends at `at`: the bytes of a `\n`, a `\r` or
    /// a `\r\n` pair, whichever the source wrote there.
    fn line_ending_text(&self, at: usize) -> &'static str {
        match (self.bytes.get(at - 1), self.bytes.get(at.wrapping_sub(2))) {
            (Some(b'\n'), Some(b'\r')) => "\r\n",
            (Some(b'\n'), _) => "\n",
            (Some(b'\r'), _) => "\r",
            _ => "\n",
        }
    }

    /// The end of the line's content at or after `at`: the offset of the
    /// line's ending, or the source's end.
    fn line_content_end(&self, at: usize) -> usize {
        let mut i = at;
        while i < self.bytes.len() && !matches!(self.bytes[i], b'\n' | b'\r') {
            i += 1;
        }
        i
    }

    /// Drops one line ending from `end`: both bytes of a `\r\n`.
    fn trim_line_ending(&self, end: usize) -> usize {
        let mut end = end;
        while end > 0 && matches!(self.bytes[end - 1], b'\n' | b'\r') {
            end -= 1;
        }
        end
    }
}

/// A byte that makes a line blank: the spec's whitespace except the line
/// endings themselves.
fn is_blank_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\x0B' | b'\x0C')
}

/// Splits a code block's info string into `lang` and `meta` at the first
/// space or tab.
fn split_info(info: Option<&str>) -> (Option<String>, Option<String>) {
    match info {
        None => (None, None),
        Some(info) => match info.find([' ', '\t']) {
            Some(at) => (
                Some(info[..at].to_string()),
                Some(info[at + 1..].to_string()),
            ),
            None => (Some(info.to_string()), None),
        },
    }
}

/// The `referenceType` string for a reference form.
fn ref_type(kind: LinkRefKind) -> &'static str {
    match kind {
        LinkRefKind::Full => FULL,
        LinkRefKind::Collapsed => COLLAPSED,
        LinkRefKind::Shortcut => SHORTCUT,
    }
}

/// The mdast tree as JSON text, compact or pretty.
///
/// The same serialization the reference implementation's fixture
/// comparisons use: `serde_json` on the tree. This is what
/// `geist4 render --format mdast` prints.
#[must_use]
pub fn to_json(mdast: &Mdast, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(mdast).expect("mdast JSON serializes")
    } else {
        serde_json::to_string(mdast).expect("mdast JSON serializes")
    }
}
