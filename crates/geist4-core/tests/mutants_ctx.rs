//! Regression tests for missed cargo-mutants survivors in the extension
//! context accessors (`src/extension.rs`), the text-buffer source map
//! (`src/parser/textmap.rs`), `Node::kind_name` (`src/node.rs`) and the
//! arena's parent tracking (`src/arena.rs`).
//!
//! These are behavioral tests written against the public API only. Each
//! test carries `// kills:` comments naming the mutant IDs (the
//! `path:line:col: description` entries reported by cargo-mutants) it is
//! designed to catch, so a future mutant run can trace every survivor to
//! the test that should have killed it.

use geist4_core::arena::{Arena, NodeId};
use geist4_core::basic_html::render_with;
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ContinueOutcome, ExtNode, Extension,
    KindDecl, Options, TreeCtx,
};
use geist4_core::node::Node;
use geist4_core::parser::{TextMap, parse_with};
use geist4_core::span::Span;
use std::borrow::Cow;
use std::sync::Arc;

/// Every extension node in the document, in allocation order.
fn extension_nodes(doc: &geist4_core::Document<'_>) -> Vec<(NodeId, String)> {
    doc.arena
        .iter()
        .filter_map(|id| match doc.arena.kind(id) {
            Node::Extension(ext) => Some((id, ext.kind.clone())),
            _ => None,
        })
        .collect()
}

/// The first child of `id` whose kind matches, with its text when it is
/// a text node.
fn find_child<'a>(
    doc: &'a geist4_core::Document<'_>,
    id: NodeId,
    want: &dyn Fn(&Node<'_>) -> bool,
) -> NodeId {
    for &child in doc.arena.children(id) {
        if want(doc.arena.kind(child)) {
            return child;
        }
    }
    panic!("no matching child under {:?}", doc.arena.kind(id));
}

fn is_paragraph(node: &Node<'_>) -> bool {
    matches!(node, Node::Paragraph)
}

fn is_text_of(text: &str) -> impl Fn(&Node<'_>) -> bool + '_ {
    move |node| matches!(node, Node::Text(t) if t.as_ref() == text)
}

fn is_empty_text(node: &Node<'_>) -> bool {
    matches!(node, Node::Text(t) if t.is_empty())
}

fn is_link(node: &Node<'_>) -> bool {
    matches!(node, Node::Link(_))
}

// ---------------------------------------------------------------------------
// Options: is_empty, Clone and Debug are public API contracts.
// ---------------------------------------------------------------------------

#[test]
fn options_is_empty_clone_and_debug_report_registrations() {
    // kills: crates/geist4-core/src/extension.rs:753:9: replace Options::is_empty -> bool with false
    // kills: crates/geist4-core/src/extension.rs:764:9: replace <impl Clone for Options>::clone -> Self with Default::default()
    // kills: crates/geist4-core/src/extension.rs:772:9: replace <impl std::fmt::Debug for Options>::fmt -> std::fmt::Result with Ok(Default::default())
    let empty = Options::new();
    // is_empty is the hook sites' short-circuit: it must agree with the
    // registration list.
    assert!(empty.is_empty());
    assert!(empty.extensions().is_empty());
    assert_eq!(format!("{empty:?}"), "Options { extensions: [] }");

    let mut one = Options::new();
    one.extension(Arc::new(Fallback));
    assert!(!one.is_empty());
    // A clone keeps the registrations; the hook sites would lose every
    // extension's behavior on a default clone.
    let cloned = one.clone();
    assert!(!cloned.is_empty());
    assert_eq!(cloned.extensions().len(), 1);
    assert!(cloned.by_name("fallback").is_some());
    // The debug form names the registered extensions, so a broken fmt
    // cannot pass unnoticed on a populated list either.
    let debug = format!("{one:?}");
    assert!(debug.starts_with("Options {"));
    assert!(debug.contains("fallback"), "debug was {debug:?}");
}

// ---------------------------------------------------------------------------
// BlockStartCtx accessors: extension index, line, blankness, arena.
// ---------------------------------------------------------------------------

/// An extension whose start hook reads `extension()`, `line()`,
/// `line_blank()` and builds a child through `arena()`.
struct Probe;

impl Extension for Probe {
    fn name(&self) -> &str {
        "probe"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "probe".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        // The line accessor hands the content after the container
        // prefixes; an empty leak there never matches.
        if ctx.line() != b"@@aa" {
            return;
        }
        // The marker line is not blank.
        if ctx.line_blank() {
            return;
        }
        // The hook runs once per registered extension, so the first (and
        // here only) registered extension sees its own index 0.
        if ctx.extension() != 0 {
            return;
        }
        let node = ctx.open_extension("probe".to_owned(), Box::new(0usize));
        let text = ctx
            .arena()
            .alloc(Node::Text(Cow::Owned("kid".to_owned())), Span::empty(0));
        ctx.arena().push_child(node, text);
    }
}

#[test]
fn block_start_accessors_open_the_probe_block() {
    // kills: crates/geist4-core/src/extension.rs:515:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::extension -> usize with 1
    // kills: crates/geist4-core/src/extension.rs:521:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::line -> &'l[u8] with Vec::leak(Vec::new())
    // kills: crates/geist4-core/src/extension.rs:526:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::line_blank -> bool with true
    // kills: crates/geist4-core/src/extension.rs:549:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::arena -> &mut crate::arena::Arena<'a> with Box::leak(Box::new(Default::default()))
    let mut options = Options::new();
    options.extension(Arc::new(Probe));
    let doc = parse_with("@@aa\n", &options);
    let exts = extension_nodes(&doc);
    // Every accessor feeds the open decision: a wrong index, an empty
    // line slice or a forced blank flag means the hook declines and no
    // block appears at all.
    assert_eq!(exts.len(), 1, "expected one probe block, got {exts:?}");
    assert_eq!(exts[0].1, "probe");
    // The child built through `arena()` must land in the parse's arena:
    // a leaked default arena loses it (or panics on the cross-arena id).
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 1, "probe block lost its arena-built child");
    assert!(is_text_of("kid")(doc.arena.kind(kids[0])));
}

// ---------------------------------------------------------------------------
// BlockStartCtx::open_node and absorb_line: appending to an open block.
// ---------------------------------------------------------------------------

/// An extension block that swallows repeated marker lines into itself.
struct Note;

impl Extension for Note {
    fn name(&self) -> &str {
        "note"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "note".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"@@go" {
            return;
        }
        if ctx.open_node().is_some() {
            // A repeated marker line is content of the open block, not a
            // new block.
            ctx.absorb_line();
        } else {
            ctx.open_extension("note".to_owned(), Box::new(0usize));
        }
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        if ctx.line() == b"!!" {
            return ContinueOutcome::CloseAndConsume;
        }
        if ctx.pure_blank() {
            return ContinueOutcome::Close;
        }
        ContinueOutcome::Continue
    }
}

#[test]
fn repeated_marker_appends_to_the_open_block() {
    // kills: crates/geist4-core/src/extension.rs:557:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::open_node -> Option<crate::arena::NodeId> with None
    // kills: crates/geist4-core/src/extension.rs:577:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::absorb_line with ()
    let mut options = Options::new();
    options.extension(Arc::new(Note));
    let doc = parse_with("@@go\n@@go\ntail\n", &options);
    // The second marker line must be absorbed into the one open block:
    // `open_node` reports it, `absorb_line` consumes the line.
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one note block, got {exts:?}");
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 1, "absorbed line must add nothing");
    let para = find_child(&doc, exts[0].0, &is_paragraph);
    assert!(is_text_of("tail")(
        doc.arena.kind(doc.arena.children(para)[0])
    ));
}

#[test]
fn closing_fence_line_consumed_whole() {
    // kills: crates/geist4-core/src/extension.rs:680:9: replace BlockContinueCtx<'c, 's, 'l>::line -> &'c[u8] with Vec::leak(vec![0])
    let mut options = Options::new();
    options.extension(Arc::new(Note));
    let doc = parse_with("@@go\nmid\n!!\ntail\n", &options);
    // The `!!` line is the block's closing fence: the continuation hook
    // must see the real line bytes to close and consume it. A leaked
    // one-byte NUL slice never matches, the block stays open, and the
    // fence plus the tail end up as paragraphs inside the block.
    let root_kids = doc.arena.children(doc.root);
    assert_eq!(root_kids.len(), 2, "root children: {root_kids:?}");
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one note block, got {exts:?}");
    assert_eq!(doc.arena.parent(root_kids[1]), Some(doc.root));
    // The tail paragraph is a child of the document, not of the block.
    let tail = doc.arena.children(root_kids[1])[0];
    assert!(is_text_of("tail")(doc.arena.kind(tail)));
    // The block holds only its one mid paragraph.
    let block_kids = doc.arena.children(exts[0].0);
    assert_eq!(block_kids.len(), 1, "block children: {block_kids:?}");
    let mid = doc.arena.children(block_kids[0])[0];
    assert!(is_text_of("mid")(doc.arena.kind(mid)));
}

// ---------------------------------------------------------------------------
// BlockContinueCtx::column.
// ---------------------------------------------------------------------------

/// An extension block that only continues at two or more columns of
/// container prefix.
struct Deep;

impl Extension for Deep {
    fn name(&self) -> &str {
        "deep"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "deep".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() == b"@@go" {
            ctx.open_extension("deep".to_owned(), Box::new(0usize));
        }
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        // Inside a block quote the prefixes leave the cursor at column
        // two; a top-level line sits at column zero and closes.
        if ctx.column() >= 2 {
            ContinueOutcome::Continue
        } else {
            ContinueOutcome::Close
        }
    }
}

#[test]
fn continuation_column_counts_the_container_prefixes() {
    // kills: crates/geist4-core/src/extension.rs:705:9: replace BlockContinueCtx<'c, 's, 'l>::column -> usize with 0
    // kills: crates/geist4-core/src/extension.rs:705:9: replace BlockContinueCtx<'c, 's, 'l>::column -> usize with 1
    let mut options = Options::new();
    options.extension(Arc::new(Deep));
    let doc = parse_with("> @@go\n> deep\n\ntail\n", &options);
    // The quoted line continues the block (column two); the blank line
    // ends the lazy paragraph and closes quote and block, so the unquoted
    // tail becomes a paragraph of the document.
    let quote = find_child(&doc, doc.root, &|node: &Node<'_>| {
        matches!(node, Node::BlockQuote)
    });
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one deep block, got {exts:?}");
    assert_eq!(doc.arena.parent(exts[0].0), Some(quote));
    let para = find_child(&doc, exts[0].0, &is_paragraph);
    assert!(is_text_of("deep")(
        doc.arena.kind(doc.arena.children(para)[0])
    ));
    let tail = find_child(&doc, doc.root, &is_paragraph);
    assert!(is_text_of("tail")(
        doc.arena.kind(doc.arena.children(tail)[0])
    ));
}

// ---------------------------------------------------------------------------
// kind_name reaches the renderer's RenderedChild::core_kind.
// ---------------------------------------------------------------------------

/// Wraps each paragraph's inline children in a `marked` node whose
/// render echoes the children's core kind names.
struct Marker;

impl Extension for Marker {
    fn name(&self) -> &str {
        "marker"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "marked".to_owned(),
            container: true,
            inline_text: true,
            attrs: vec!["label".to_owned()],
        }]
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        let mut stack: Vec<NodeId> = ctx.arena.children(ctx.root).to_vec();
        while let Some(id) = stack.pop() {
            if !matches!(ctx.arena.kind(id), Node::Paragraph) {
                stack.extend(ctx.arena.children(id).iter().copied());
                continue;
            }
            let kids = ctx.arena.children(id).to_vec();
            let ext_id = ctx.arena.alloc(
                Node::Extension(ExtNode {
                    kind: "marked".to_owned(),
                    attrs: vec![("label".to_owned(), AttrValue::Str("seen".to_owned()))],
                }),
                Span::empty(0),
            );
            ctx.arena.entry_mut(id).children.clear();
            ctx.arena.push_child(id, ext_id);
            for kid in kids {
                ctx.arena.push_child(ext_id, kid);
            }
        }
    }

    fn render(
        &self,
        node: &ExtNode,
        children: &[geist4_core::extension::RenderedChild<'_>],
        out: &mut String,
    ) {
        if node.kind != "marked" {
            return;
        }
        out.push_str("<marked k=\"");
        for child in children {
            if let Some(core_kind) = child.core_kind {
                out.push_str(core_kind);
            }
        }
        out.push_str("\">");
        for child in children {
            out.push_str(&child.html);
        }
        out.push_str("</marked>");
    }
}

#[test]
fn kind_name_flows_to_extension_render_children() {
    // kills: crates/geist4-core/src/node.rs:182:9: replace Node<'a>::kind_name -> &'static str with ""
    // kills: crates/geist4-core/src/node.rs:182:9: replace Node<'a>::kind_name -> &'static str with "xyzzy"
    let mut options = Options::new();
    options.extension(Arc::new(Marker));
    let doc = parse_with("hi\n", &options);
    assert_eq!(
        render_with(&doc, &options),
        "<p><marked k=\"text\">hi</marked></p>\n"
    );
    // The stable names are a public contract of their own.
    assert_eq!(Node::Document.kind_name(), "document");
    assert_eq!(Node::ThematicBreak.kind_name(), "thematic_break");
    assert_eq!(Node::CodeSpan(Cow::Borrowed("x")).kind_name(), "code_span");
}

// ---------------------------------------------------------------------------
// Arena parent tracking.
// ---------------------------------------------------------------------------

#[test]
fn arena_parent_tracks_attach_and_detach() {
    // kills: crates/geist4-core/src/arena.rs:127:9: replace Arena<'a>::parent -> Option<NodeId> with None
    let mut arena = Arena::new();
    let parent = arena.alloc(Node::Paragraph, Span::empty(0));
    let child = arena.alloc(Node::Text(Cow::Borrowed("x")), Span::empty(0));
    // A node still held but never attached has no parent.
    assert_eq!(arena.parent(child), None);
    arena.push_child(parent, child);
    assert_eq!(arena.parent(child), Some(parent));
    assert_eq!(arena.parent(parent), None);
    // insert_child tracks the same way, for tree-surgery hooks.
    let other = arena.alloc(Node::Paragraph, Span::empty(0));
    let moved = arena.alloc(Node::Text(Cow::Borrowed("y")), Span::empty(0));
    arena.insert_child(other, 0, moved);
    assert_eq!(arena.parent(moved), Some(other));
    // Detaching clears the record again.
    assert_eq!(arena.pop_child(parent), Some(child));
    assert_eq!(arena.parent(child), None);
    assert_eq!(arena.remove_child(other, 0), Some(moved));
    assert_eq!(arena.parent(moved), None);
}

// ---------------------------------------------------------------------------
// TextMap: byte_before and first_src, through an inline trigger's unput.
// ---------------------------------------------------------------------------

/// An inline trigger that rewinds five bytes of text and emits a link
/// over the span, leaving the unwritten text cells empty.
struct XLink;

impl Extension for XLink {
    fn name(&self) -> &str {
        "xlink"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"X"
    }

    fn inline_trigger(&self, ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>) {
        ctx.emit_link(5, 2, "u".to_owned());
    }
}

#[test]
fn unwritten_text_cells_map_to_their_preceding_byte() {
    // kills: crates/geist4-core/src/parser/textmap.rs:114:9: replace TextMap::byte_before -> usize with 0
    // kills: crates/geist4-core/src/parser/textmap.rs:114:14: replace == with != in TextMap::byte_before
    // kills: crates/geist4-core/src/parser/textmap.rs:117:25: replace - with + in TextMap::byte_before
    // kills: crates/geist4-core/src/parser/textmap.rs:208:9: replace TextMap::first_src -> usize with 0
    // The paragraph starts at source offset 6, after the "para\n\n"
    // prefix, so its buffer byte `k` maps to source `6 + k`.
    let src = "para\n\nab!cdXef\n";
    let mut options = Options::new();
    options.extension(Arc::new(XLink));
    let doc = parse_with(src, &options);
    // The trigger paragraph is the document's second paragraph; the
    // first ("para") shares the source with nothing else.
    let para = *doc
        .arena
        .children(doc.root)
        .iter()
        .rev()
        .find(|&&id| is_paragraph(doc.arena.kind(id)))
        .expect("a trigger paragraph");
    let kids = doc.arena.children(para);
    // The trigger rewinds five bytes ("ab", "!" and "cd" were separate
    // text cells), empties all three cells, then emits the link over
    // buffer [0, 7) and leaves "f" as the tail cell.
    assert_eq!(
        kids.len(),
        5,
        "paragraph children: {:?}",
        doc.arena.children(para)
    );
    let span = |id: NodeId| {
        let s = doc.arena.span(id);
        (s.start, s.end)
    };
    // An emptied first cell maps through first_src: the buffer's first
    // byte sits at source offset 6, not 0.
    assert!(is_empty_text(doc.arena.kind(kids[0])));
    assert_eq!(span(kids[0]), (6, 6), "first emptied cell");
    // The emptied `!` cell at buffer 2 maps just before buffer byte 1,
    // i.e. source 8; the emptied `cd` cell at buffer 3 maps to 9. An
    // off-by-one in byte_before moves these to a neighbouring byte.
    assert!(is_empty_text(doc.arena.kind(kids[1])));
    assert_eq!(span(kids[1]), (8, 8), "second emptied cell");
    assert!(is_empty_text(doc.arena.kind(kids[2])));
    assert_eq!(span(kids[2]), (9, 9), "third emptied cell");
    // The link and the tail text keep their own mapped spans, so the
    // fixtures around the emptied cells are sound.
    assert!(is_link(doc.arena.kind(kids[3])));
    assert_eq!(span(kids[3]), (6, 13), "emitted link");
    assert!(is_text_of("f")(doc.arena.kind(kids[4])));
    assert_eq!(span(kids[4]), (13, 14), "tail text");
}

// ---------------------------------------------------------------------------
// TextMap::map_span's no-anchor fallback, through a close hook's map move.
// ---------------------------------------------------------------------------

/// An extension block whose close hook gives it one text child and an
/// empty source map: the child's bytes came straight from one source
/// region, so offsets shift by the block's start.
struct Fallback;

impl Extension for Fallback {
    fn name(&self) -> &str {
        "fallback"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "copy".to_owned(),
            container: true,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() == b"%%" {
            ctx.open_extension("copy".to_owned(), Box::new(0usize));
        }
    }

    fn close(&self, _state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        // The escape keeps the text in three separate cells.
        let text = ctx.arena.alloc(
            Node::Text(Cow::Owned("ab\\-cd".to_owned())),
            Span::empty(ctx.start_offset),
        );
        ctx.arena.push_child(ctx.node, text);
        ctx.set_text_map(ctx.node, TextMap::default());
    }
}

#[test]
fn empty_map_fallback_offsets_shift_by_the_block_start() {
    // kills: crates/geist4-core/src/parser/textmap.rs:100:58: replace + with - in TextMap::map_span
    // kills: crates/geist4-core/src/parser/textmap.rs:100:82: replace + with - in TextMap::map_span
    let src = "para\n\n%%\n";
    let mut options = Options::new();
    options.extension(Arc::new(Fallback));
    let doc = parse_with(src, &options);
    // The `%%` line starts at source offset 6; with an empty map the
    // cell offsets shift by exactly that.
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one copy block, got {exts:?}");
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 3, "copy block children: {kids:?}");
    let span = |id: NodeId| {
        let s = doc.arena.span(id);
        (s.start, s.end)
    };
    // "ab" starts at buffer 0, so only its end is observable; the later
    // cells pin both ends of the shift.
    assert!(is_text_of("ab")(doc.arena.kind(kids[0])));
    assert_eq!(span(kids[0]), (6, 8), "first cell");
    assert!(is_text_of("-")(doc.arena.kind(kids[1])));
    assert_eq!(span(kids[1]), (8, 10), "second cell");
    assert!(is_text_of("cd")(doc.arena.kind(kids[2])));
    assert_eq!(span(kids[2]), (10, 12), "third cell");
}
