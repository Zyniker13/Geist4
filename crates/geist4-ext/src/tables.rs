//! GFM tables.
//!
//! The semantics follow the reference implementation: a header row and
//! a delimiter row turn the open paragraph into a table; the header is
//! the paragraph's last row-shaped line, any earlier lines stay a
//! paragraph in front of it; every later line that no built-in block
//! start claims joins the table as a body row. Cells split on
//! unescaped pipes, are trimmed, and are padded to or cut at the
//! delimiter row's column count. The renderer wraps the header in
//! `thead` and the body in `tbody`, marks header cells `th`, and
//! carries each column's alignment as an `align` attribute, which is
//! what the GFM specification examples and the reference implementation
//! in their default configuration emit.

use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ExtNode, Extension, KindDecl,
    RenderedChild, StartContainer,
};
use geist4_core::node::Node;
use geist4_core::span::Span;
use std::borrow::Cow;
use std::sync::Arc;

/// GFM tables: pipe tables with an optional per-column alignment.
#[derive(Debug)]
pub struct Tables;

impl Tables {
    /// The table kind's name.
    pub const KIND: &'static str = "table";
    const KIND_ROW: &'static str = "table_row";
    const KIND_CELL: &'static str = "table_cell";

    /// Tables as a shared extension value.
    #[must_use]
    pub fn extension() -> Arc<dyn Extension> {
        Arc::new(Tables)
    }
}

/// A column's alignment from its delimiter cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    fn as_str(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }

    /// The alignment a delimiter cell describes: colons at the cell's
    /// edges, as the reference reads them.
    fn from_cell(cell: &str) -> Option<Align> {
        let bytes = cell.as_bytes();
        let left = bytes.first() == Some(&b':');
        let right = bytes.last() == Some(&b':');
        match (left, right) {
            (true, true) => Some(Align::Center),
            (true, false) => Some(Align::Left),
            (false, true) => Some(Align::Right),
            (false, false) => None,
        }
    }
}

/// The open table's parsing state: its column count and each column's
/// alignment, one entry per column.
#[derive(Debug, Default)]
struct TableState {
    columns: usize,
    aligns: Vec<Option<Align>>,
}

impl Extension for Tables {
    fn name(&self) -> &str {
        "tables"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![
            KindDecl {
                kind: Self::KIND.to_owned(),
                container: true,
                inline_text: false,
                attrs: vec!["aligns".to_owned()],
            },
            KindDecl {
                kind: Self::KIND_ROW.to_owned(),
                container: true,
                inline_text: false,
                attrs: vec!["head".to_owned()],
            },
            KindDecl {
                kind: Self::KIND_CELL.to_owned(),
                container: true,
                inline_text: true,
                attrs: vec!["head".to_owned(), "align".to_owned()],
            },
        ]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.indented() {
            return;
        }
        match ctx.container() {
            StartContainer::Paragraph => self.try_header(ctx),
            StartContainer::Extension(index) if index == ctx.extension() => self.try_row(ctx),
            _ => {}
        }
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_>) -> bool {
        // The table continues on any line that parses as a row with at
        // least one cell; a blank line or anything else closes it.
        scan_row(ctx.line()).is_some()
    }

    fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        // The columns' alignments travel on the table node for
        // renderers beyond this one.
        let state = match state.downcast_ref::<TableState>() {
            Some(state) => state,
            None => return,
        };
        if state.columns == 0 {
            return;
        }
        let joined: Vec<&str> = (0..state.columns)
            .map(
                |index| match state.aligns.get(index).and_then(|align| *align) {
                    Some(align) => align.as_str(),
                    None => "",
                },
            )
            .collect();
        if let Node::Extension(ext) = ctx.arena.kind(ctx.node) {
            let mut attrs = ext.attrs.clone();
            attrs.push(("aligns".to_owned(), AttrValue::Str(joined.join(","))));
            ctx.arena.set_kind(
                ctx.node,
                Node::Extension(ExtNode {
                    kind: ext.kind.clone(),
                    attrs,
                }),
            );
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        match node.kind.as_str() {
            Self::KIND => render_table(children, out),
            Self::KIND_ROW => render_row(children, out),
            Self::KIND_CELL => render_cell(node, children, out),
            _ => {}
        }
    }
}

impl Tables {
    /// The paragraph-to-table conversion: the current line is the
    /// delimiter row, the paragraph's last row-shaped line the header.
    fn try_header(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        let line = ctx.line();
        // The line must be a complete delimiter row; a non-matching
        // line does not spend the paragraph's one chance.
        if !scan_delimiter_row(line) {
            return;
        }
        let Some((delim_cells, _)) = scan_row(line) else {
            return;
        };
        let Some(text) = ctx.paragraph_text() else {
            return;
        };
        let text = text.to_owned();
        // The reference's paragraph content ends with a newline; the
        // row scan needs it to see the last line's row end.
        let mut scanned = text.clone();
        if !scanned.ends_with('\n') {
            scanned.push('\n');
        }
        // The header row is the paragraph's last row-shaped line;
        // scanning the whole accumulated text rewinds to it and
        // reports where it begins.
        let Some((header_cells, pre_len)) = scan_row(scanned.as_bytes()) else {
            ctx.visit_paragraph();
            return;
        };
        if header_cells.len() != delim_cells.len() {
            ctx.visit_paragraph();
            return;
        }
        let aligns: Vec<Option<Align>> = delim_cells
            .iter()
            .map(|cell| Align::from_cell(cell))
            .collect();
        let table = ctx.convert_paragraph(
            Self::KIND.to_owned(),
            pre_len,
            Box::new(TableState {
                columns: delim_cells.len(),
                aligns: aligns.clone(),
            }),
        );
        // The header row and its cells join the table node.
        let arena = ctx.arena();
        let row = arena.alloc(
            Node::Extension(ExtNode {
                kind: Self::KIND_ROW.to_owned(),
                attrs: vec![("head".to_owned(), AttrValue::Str("true".to_owned()))],
            }),
            Span::empty(0),
        );
        arena.push_child(table, row);
        for (index, cell) in header_cells.iter().enumerate() {
            let mut attrs = vec![("head".to_owned(), AttrValue::Str("true".to_owned()))];
            if let Some(align) = aligns.get(index).and_then(|align| *align) {
                attrs.push((
                    "align".to_owned(),
                    AttrValue::Str(align.as_str().to_owned()),
                ));
            }
            let cell_id = arena.alloc(
                Node::Extension(ExtNode {
                    kind: Self::KIND_CELL.to_owned(),
                    attrs,
                }),
                Span::empty(0),
            );
            arena.push_child(row, cell_id);
            let text_id = arena.alloc(Node::Text(Cow::Owned(cell.clone())), Span::empty(0));
            arena.push_child(cell_id, text_id);
        }
    }

    /// A body row: the open table's continuation.
    fn try_row(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        let Some((cells, _)) = scan_row(ctx.line()) else {
            return;
        };
        let Some(table) = ctx.open_node() else {
            return;
        };
        let Some(state) = ctx.state::<TableState>() else {
            return;
        };
        let aligns = state.aligns.clone();
        let columns = aligns.len();
        let arena = ctx.arena();
        let row = arena.alloc(
            Node::Extension(ExtNode {
                kind: Self::KIND_ROW.to_owned(),
                attrs: Vec::new(),
            }),
            Span::empty(0),
        );
        arena.push_child(table, row);
        // Extra cells are dropped, missing cells padded empty.
        for index in 0..columns {
            let content = cells.get(index).cloned().unwrap_or_default();
            let mut attrs = Vec::new();
            if let Some(align) = aligns.get(index).and_then(|align| *align) {
                attrs.push((
                    "align".to_owned(),
                    AttrValue::Str(align.as_str().to_owned()),
                ));
            }
            let cell_id = arena.alloc(
                Node::Extension(ExtNode {
                    kind: Self::KIND_CELL.to_owned(),
                    attrs,
                }),
                Span::empty(0),
            );
            arena.push_child(row, cell_id);
            let text_id = arena.alloc(Node::Text(Cow::Owned(content)), Span::empty(0));
            arena.push_child(cell_id, text_id);
        }
        ctx.absorb_line();
    }
}

/// The scanner space set: the reference `ext_scanners`' `spacechar`,
/// which is space, tab, vertical tab and form feed.
fn is_scanner_space(byte: Option<&u8>) -> bool {
    matches!(
        byte,
        Some(b' ') | Some(b'\t') | Some(b'\x0b') | Some(b'\x0c')
    )
}

/// The cell-trim set: the reference's `cmark_isspace`, which for cells
/// comes down to spaces and tabs.
fn is_trim_space(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

/// `\|` unescapes to `|` inside a cell.
fn unescape_pipes(raw: &[u8]) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'\\' && raw.get(index + 1) == Some(&b'|') {
            out.push('|');
            index += 2;
        } else {
            out.push(raw[index] as char);
            index += 1;
        }
    }
    out
}

/// Builds a cell's text: pipe unescaping, then trimming.
fn cell_text(raw: &[u8]) -> String {
    let text = unescape_pipes(raw);
    let start = text
        .bytes()
        .position(|byte| !is_trim_space(byte))
        .unwrap_or(text.len());
    let trimmed = &text[start..];
    let end = trimmed
        .bytes()
        .rposition(|byte| !is_trim_space(byte))
        .map_or(0, |position| position + 1);
    trimmed[..end].to_owned()
}

/// The delimiter row's shape: an optional leading pipe, one or more
/// `:?-+:?` markers separated by single pipes, an optional trailing
/// pipe, then spaces to the line's end.
fn scan_delimiter_row(input: &[u8]) -> bool {
    let mut index = usize::from(input.first() == Some(&b'|'));
    if !marker_at(input, &mut index) {
        return false;
    }
    loop {
        if input.get(index) == Some(&b'|') {
            let mut after = index + 1;
            if marker_at(input, &mut after) {
                index = after;
                continue;
            }
            // The pipe may be the optional trailing pipe.
            after = index + 1;
            while is_scanner_space(input.get(after)) {
                after += 1;
            }
            return after == input.len();
        }
        break;
    }
    while is_scanner_space(input.get(index)) {
        index += 1;
    }
    index == input.len()
}

/// One `spacechar* [:]? [-]+ [:]? spacechar*` marker at `index`.
fn marker_at(input: &[u8], index: &mut usize) -> bool {
    let len = input.len();
    while is_scanner_space(input.get(*index)) {
        *index += 1;
    }
    if input.get(*index) == Some(&b':') {
        *index += 1;
    }
    let dash_start = *index;
    while input.get(*index) == Some(&b'-') {
        *index += 1;
    }
    if *index == dash_start {
        return false;
    }
    if input.get(*index) == Some(&b':') {
        *index += 1;
    }
    while is_scanner_space(input.get(*index)) {
        *index += 1;
    }
    *index <= len
}

/// A pipe followed by spaces: the reference's `scan_table_cell_end`.
fn scan_cell_end(input: &[u8], offset: usize) -> usize {
    if input.get(offset) != Some(&b'|') {
        return 0;
    }
    let mut end = offset + 1;
    while is_scanner_space(input.get(end)) {
        end += 1;
    }
    end - offset
}

/// A run of cell characters: everything but pipes and line ends, with
/// backslash escapes as single units. The reference's
/// `scan_table_cell`.
fn scan_cell(input: &[u8], offset: usize) -> usize {
    let mut end = offset;
    while let Some(&byte) = input.get(end) {
        if byte == b'|' || byte == b'\n' || byte == b'\r' {
            break;
        }
        if byte == b'\\' && matches!(input.get(end + 1), Some(b'|')) {
            end += 2;
        } else {
            end += 1;
        }
    }
    end - offset
}

/// Spaces up to a line end: the reference's `scan_table_row_end`,
/// whose match includes the newline byte itself. At the input's true
/// end, which stands in for a missing terminator, nothing matches.
fn scan_row_end(input: &[u8], offset: usize) -> usize {
    let mut end = offset;
    while is_scanner_space(input.get(end)) {
        end += 1;
    }
    match input.get(end) {
        Some(b'\n') => end + 1 - offset,
        Some(b'\r') if input.get(end + 1) == Some(&b'\n') => end + 2 - offset,
        _ => 0,
    }
}

/// Scans a row out of `input`, the reference's `row_from_string`.
///
/// A row ends at a pipe-less cell's end or at the input's end. A row
/// end in the middle of the input belongs to an earlier line of the
/// paragraph; the scan then restarts after it, so the returned cells
/// are the input's last row-shaped line and the offset just before it.
/// Returns the cells, trimmed, and that offset.
fn scan_row(input: &[u8]) -> Option<(Vec<String>, usize)> {
    let len = input.len();
    let mut offset = scan_cell_end(input, 0);
    let mut cells: Vec<String> = Vec::new();
    let mut paragraph_offset = 0;
    let mut expect_more = true;
    while offset < len && expect_more {
        let cell_matched = scan_cell(input, offset);
        let pipe_matched = scan_cell_end(input, offset + cell_matched);
        if cell_matched > 0 || pipe_matched > 0 {
            // Either cell content or an empty cell with its pipe.
            cells.push(cell_text(&input[offset..offset + cell_matched]));
        }
        offset += cell_matched + pipe_matched;
        if pipe_matched > 0 {
            expect_more = true;
        } else {
            let row_end = scan_row_end(input, offset);
            offset += row_end;
            if row_end > 0 && offset != len {
                // An earlier line's end: restart after it.
                paragraph_offset = offset;
                cells.clear();
                offset += scan_cell_end(input, offset);
            } else {
                expect_more = false;
            }
        }
    }
    if offset != len || cells.is_empty() {
        return None;
    }
    Some((cells, paragraph_offset))
}

/// The table's rendered HTML: `thead` around the header row, `tbody`
/// around the body rows.
fn render_table(children: &[RenderedChild<'_>], out: &mut String) {
    out.push_str("<table>\n");
    let mut body_open = false;
    for child in children {
        if is_head_row(child) {
            out.push_str("<thead>\n");
            out.push_str(&child.html);
            out.push_str("\n</thead>");
        } else {
            if !body_open {
                out.push_str("\n<tbody>\n");
                body_open = true;
            } else {
                out.push('\n');
            }
            out.push_str(&child.html);
        }
    }
    if body_open {
        out.push_str("\n</tbody>\n");
    } else {
        out.push('\n');
    }
    out.push_str("</table>\n");
}

/// A row's rendered HTML: one cell per line.
fn render_row(children: &[RenderedChild<'_>], out: &mut String) {
    out.push_str("<tr>\n");
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&child.html);
    }
    out.push_str("\n</tr>");
}

/// A cell's rendered HTML: `th` in the header row, `td` in the body,
/// with the column's alignment as a style when it has one.
fn render_cell(node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
    let head = node
        .attrs
        .iter()
        .any(|(name, value)| name == "head" && value == &AttrValue::Str("true".to_owned()));
    let tag = if head { "th" } else { "td" };
    out.push('<');
    out.push_str(tag);
    if let Some((_, AttrValue::Str(align))) = node.attrs.iter().find(|(name, _)| name == "align") {
        out.push_str(" align=\"");
        out.push_str(align);
        out.push('"');
    }
    out.push('>');
    for child in children {
        out.push_str(&child.html);
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

/// Whether a rendered child is a header row.
fn is_head_row(child: &RenderedChild<'_>) -> bool {
    child.node.is_some_and(|node| {
        node.kind == "table_row"
            && node
                .attrs
                .iter()
                .any(|(name, value)| name == "head" && value == &AttrValue::Str("true".to_owned()))
    })
}
