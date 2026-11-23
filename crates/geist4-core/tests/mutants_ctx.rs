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
use std::sync::{Arc, Mutex};

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
    assert_eq!(
        format!("{empty:?}"),
        "Options { extensions: [], highlighter: false }"
    );

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
// ==== preceding_reports_the_byte_before_the_trigger ====
#[test]
fn preceding_reports_the_byte_before_the_trigger() {
    // kills: crates/geist4-core/src/inline/mod.rs:772:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::preceding -> Option<u8> with Some(0)
    // kills: crates/geist4-core/src/inline/mod.rs:772:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::preceding -> Option<u8> with Some(1)
    // The hook links a `www` run only where the documented contract
    // allows a match to start: with no byte before the cursor, or after
    // a space. A constant `Some` reports a byte at the subject's start
    // (where `None` is due) and the wrong byte after a space, so the
    // hook declines and the link disappears.
    struct Www;
    impl Extension for Www {
        fn name(&self) -> &str {
            "www"
        }

        fn inline_triggers(&self) -> &[u8] {
            b"w"
        }

        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            match ctx.preceding() {
                None | Some(b' ') => ctx.emit_link(0, 3, "u".to_owned()),
                _ => {}
            }
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(Www));
    assert_eq!(
        render_with(&parse_with("www x\n", &options), &options),
        "<p><a href=\"u\">www</a> x</p>\n"
    );
    assert_eq!(
        render_with(&parse_with(" www x\n", &options), &options),
        "<p><a href=\"u\">www</a> x</p>\n"
    );
    assert_eq!(
        render_with(&parse_with("awww x\n", &options), &options),
        "<p>awww x</p>\n"
    );
}

// ==== line_prefix_spans_from_the_line_start_to_the_cursor ====
#[test]
fn line_prefix_spans_from_the_line_start_to_the_cursor() {
    // kills: crates/geist4-core/src/inline/mod.rs:783:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::line_prefix -> &'a[u8] with Vec::leak(Vec::new())
    // kills: crates/geist4-core/src/inline/mod.rs:783:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::line_prefix -> &'a[u8] with Vec::leak(vec![0])
    // kills: crates/geist4-core/src/inline/mod.rs:783:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::line_prefix -> &'a[u8] with Vec::leak(vec![1])
    // kills: crates/geist4-core/src/inline/mod.rs:788:40: replace + with - in TriggerInner<'p, 'defs, 'a, 'ext>::line_prefix
    // kills: crates/geist4-core/src/inline/mod.rs:788:40: replace + with * in TriggerInner<'p, 'defs, 'a, 'ext>::line_prefix
    // The hook emits a link whose destination is the line prefix it was
    // handed, so the slice reads straight off the link's url. The prefix
    // must run from the start of the cursor's line — across a soft break
    // too, and empty on the line's first byte — not from a neighbouring
    // byte, and not a leaked constant.
    struct Prefix;
    impl Extension for Prefix {
        fn name(&self) -> &str {
            "prefix"
        }

        fn inline_triggers(&self) -> &[u8] {
            b":"
        }

        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            let url = String::from_utf8_lossy(ctx.line_prefix()).into_owned();
            ctx.emit_link(0, 1, url);
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(Prefix));
    let url = |src: &str, options: &Options| {
        let doc = parse_with(src, options);
        for &block in doc.arena.children(doc.root) {
            if !matches!(doc.arena.kind(block), Node::Paragraph) {
                continue;
            }
            for &kid in doc.arena.children(block) {
                if let Node::Link(data) = doc.arena.kind(kid) {
                    return data.url.to_string();
                }
            }
        }
        panic!("no link in {src}");
    };
    assert_eq!(url("ab:cd\n", &options), "ab");
    assert_eq!(url("ab\ncd:ef\n", &options), "cd");
    assert_eq!(url("ab\n:cd\n", &options), "");
}

// ==== bracket_content_reports_the_groups_text ====
/// An extension that claims a bracket group under a kind name derived
/// from the group's text content: `got-ab` for content "ab", `got-zz`
/// for content "zz", and nothing otherwise.
struct ContentGate;

impl Extension for ContentGate {
    fn name(&self) -> &str {
        "content-gate"
    }

    fn inline_bracket(&self, ctx: &mut geist4_core::extension::InlineBracketCtx<'_, '_, '_, '_>) {
        match ctx.content() {
            Some("ab") => ctx.claim("got-ab".to_owned(), Vec::new()),
            Some("zz") => ctx.claim("got-zz".to_owned(), Vec::new()),
            _ => {}
        }
    }
}

#[test]
fn bracket_content_reports_the_groups_text() {
    let mut options = Options::new();
    options.extension(Arc::new(ContentGate));
    let doc = parse_with("x [ab] y [zz]\n", &options);
    let kinds: Vec<String> = extension_nodes(&doc)
        .into_iter()
        .map(|(_, kind)| kind)
        .collect();
    assert_eq!(kinds, ["got-ab", "got-zz"], "kinds: {kinds:?}");
}

// ==== content_starts_with_gates_on_the_first_text ====
/// An extension that claims only groups whose content starts with
/// "ab".
struct StartsWithGate;

impl Extension for StartsWithGate {
    fn name(&self) -> &str {
        "starts-with-gate"
    }

    fn inline_bracket(&self, ctx: &mut geist4_core::extension::InlineBracketCtx<'_, '_, '_, '_>) {
        if ctx.content_starts_with(b"ab") {
            ctx.claim("sw-ab".to_owned(), Vec::new());
        }
    }
}

#[test]
fn content_starts_with_gates_on_the_first_text() {
    let mut options = Options::new();
    options.extension(Arc::new(StartsWithGate));
    let doc = parse_with("[abq] and [zz]\n", &options);
    let kinds: Vec<String> = extension_nodes(&doc)
        .into_iter()
        .map(|(_, kind)| kind)
        .collect();
    assert_eq!(kinds, ["sw-ab"], "kinds: {kinds:?}");
}

// ==== opener_position_sits_just_past_the_bracket ====
/// An extension that claims every group under a kind name carrying
/// the opener's input position.
struct OpenerSpot;

impl Extension for OpenerSpot {
    fn name(&self) -> &str {
        "opener-spot"
    }

    fn inline_bracket(&self, ctx: &mut geist4_core::extension::InlineBracketCtx<'_, '_, '_, '_>) {
        ctx.claim(format!("op-{}", ctx.opener_position()), Vec::new());
    }
}

#[test]
fn opener_position_sits_just_past_the_bracket() {
    // The accessor's value feeds the link-start math
    // `opener_position - 1`, so it is the byte just past the `[`: for
    // "ab [cd]" the bracket sits at byte 3 and the accessor returns 4.
    let mut options = Options::new();
    options.extension(Arc::new(OpenerSpot));
    let doc = parse_with("ab [cd]\n", &options);
    let kinds: Vec<String> = extension_nodes(&doc)
        .into_iter()
        .map(|(_, kind)| kind)
        .collect();
    assert_eq!(kinds, ["op-4"], "kinds: {kinds:?}");
}

// ==== rest_of_line_runs_from_the_trigger_to_the_line_end ====
/// An inline trigger that emits a link over "QXY" exactly when the
/// rest of the line from the trigger position reads "QXY".
struct LineEnd;

impl Extension for LineEnd {
    fn name(&self) -> &str {
        "line-end"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"Q"
    }

    fn inline_trigger(&self, ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>) {
        if ctx.rest_of_line() == b"QXY".as_slice() {
            ctx.emit_link(0, 3, "u".to_owned());
        }
    }
}

#[test]
fn rest_of_line_runs_from_the_trigger_to_the_line_end() {
    let mut options = Options::new();
    options.extension(Arc::new(LineEnd));
    let doc = parse_with("goQXY\n", &options);
    let html = render_with(&doc, &options);
    assert_eq!(html, "<p>go<a href=\"u\">QXY</a></p>\n");
}

// ==== line_prefix_runs_from_the_line_start_to_the_trigger ====
/// An inline trigger that emits a link over the trigger byte and the
/// two bytes before it exactly when the line prefix reads "ab".
struct LineStart;

impl Extension for LineStart {
    fn name(&self) -> &str {
        "line-start"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"Q"
    }

    fn inline_trigger(&self, ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>) {
        if ctx.line_prefix() == b"ab".as_slice() {
            ctx.emit_link(2, 1, "u".to_owned());
        }
    }
}

#[test]
fn line_prefix_runs_from_the_line_start_to_the_trigger() {
    let mut options = Options::new();
    options.extension(Arc::new(LineStart));
    let doc = parse_with("abQ cd\n", &options);
    let html = render_with(&doc, &options);
    assert_eq!(html, "<p><a href=\"u\">abQ</a> cd</p>\n");
}

// ==== trigger_line_prefix_rewinds_to_the_line_start ====
// kills: crates/geist4-core/src/inline/mod.rs:718:9: replace InlineTriggerCtx::line_prefix -> &'a [u8] with Vec::leak(vec![0])
// kills: crates/geist4-core/src/inline/mod.rs:718:9: replace InlineTriggerCtx::line_prefix -> &'a [u8] with Vec::leak(vec![1])
#[test]
fn trigger_line_prefix_rewinds_to_the_line_start() {
    // A scheme-gated extension: a ":" fires and links only when the
    // bytes from the line start up to the cursor spell "http".
    struct SchemeLink;
    impl Extension for SchemeLink {
        fn name(&self) -> &str {
            "scheme-link"
        }
        fn inline_triggers(&self) -> &[u8] {
            b":"
        }
        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            let before = ctx.line_prefix();
            if before == b"http" {
                // Unwrite the scheme so the link spans "http:"; the
                // scheme bytes are trailing text, so the rewind is
                // legal here.
                ctx.emit_link(before.len(), 1, "s".to_owned());
            }
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(SchemeLink));
    // The prefix excludes the cursor byte and runs back to the line
    // start, so the link spans exactly "http:".
    assert_eq!(
        render_with(&parse_with("http:\n", &options), &options),
        "<p><a href=\"s\">http:</a></p>\n"
    );
    // The newline bounds the view on the left: on the second line the
    // prefix is just "http", not the previous line's "x\nhttp".
    assert_eq!(
        render_with(&parse_with("x\nhttp:\n", &options), &options),
        "<p>x\n<a href=\"s\">http:</a></p>\n"
    );
    // A different prefix leaves the trigger byte as literal text.
    assert_eq!(
        render_with(&parse_with("go:\n", &options), &options),
        "<p>go:</p>\n"
    );
}

// ==== trigger_preceding_reports_the_byte_before_the_cursor ====
// kills: crates/geist4-core/src/inline/mod.rs:724:9: replace InlineTriggerCtx::preceding -> Option<u8> with None
// kills: crates/geist4-core/src/inline/mod.rs:724:9: replace InlineTriggerCtx::preceding -> Option<u8> with Some(0)
// kills: crates/geist4-core/src/inline/mod.rs:724:9: replace InlineTriggerCtx::preceding -> Option<u8> with Some(1)
#[test]
fn trigger_preceding_reports_the_byte_before_the_cursor() {
    // A gate that links "Z" only when nothing precedes it on the line
    // or a space does.
    struct SpaceGate;
    impl Extension for SpaceGate {
        fn name(&self) -> &str {
            "space-gate"
        }
        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }
        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            match ctx.preceding() {
                None | Some(b' ') => ctx.emit_link(0, 1, "u".to_owned()),
                _ => {}
            }
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(SpaceGate));
    // At the very start of the line there is no preceding byte.
    assert_eq!(
        render_with(&parse_with("Zz\n", &options), &options),
        "<p><a href=\"u\">Z</a>z</p>\n"
    );
    // After a space the preceding byte is that space.
    assert_eq!(
        render_with(&parse_with("a Zz\n", &options), &options),
        "<p>a <a href=\"u\">Z</a>z</p>\n"
    );
    // Any other byte closes the gate.
    assert_eq!(
        render_with(&parse_with("aZz\n", &options), &options),
        "<p>aZz</p>\n"
    );
}

// ==== trigger_matches_stay_out_of_brackets ====
// kills: crates/geist4-core/src/inline/mod.rs:732:9: replace InlineTriggerCtx::in_bracket -> bool with true
// kills: crates/geist4-core/src/inline/mod.rs:732:9: replace InlineTriggerCtx::in_bracket -> bool with false
#[test]
fn trigger_matches_stay_out_of_brackets() {
    // A bare-link extension that declines inside link or image
    // brackets, the way an autolink must.
    struct BareLink;
    impl Extension for BareLink {
        fn name(&self) -> &str {
            "bare-link"
        }
        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }
        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            if !ctx.in_bracket() {
                ctx.emit_link(0, 1, "u".to_owned());
            }
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(BareLink));
    // Outside brackets the bare link fires.
    assert_eq!(
        render_with(&parse_with("go Z now\n", &options), &options),
        "<p>go <a href=\"u\">Z</a> now</p>\n"
    );
    // Inside an inline link the trigger declines and the bracket pair
    // resolves as the ordinary link.
    assert_eq!(
        render_with(&parse_with("[Z](/u)\n", &options), &options),
        "<p><a href=\"/u\">Z</a></p>\n"
    );
    // Inside an unresolved bracket group the content stays literal.
    assert_eq!(
        render_with(&parse_with("[Z]\n", &options), &options),
        "<p>[Z]</p>\n"
    );
}

// ==== trigger_unput_unwrites_trailing_text ====
// kills: crates/geist4-core/src/inline/mod.rs:741:9: replace InlineTriggerCtx::unput -> () with ()
#[test]
fn trigger_unput_unwrites_trailing_text() {
    // An extension that unwrites trailing text before the trigger byte
    // and takes nothing else; the bytes it unwrites vanish.
    struct Chop(usize);
    impl Extension for Chop {
        fn name(&self) -> &str {
            "chop"
        }
        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }
        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            ctx.unput(self.0);
        }
    }
    let mut options = Options::new();
    // Unwriting within one cell shortens that cell: "cd" loses both
    // bytes and the "!" text stays.
    options.extension(Arc::new(Chop(2)));
    assert_eq!(
        render_with(&parse_with("ab!cdZe\n", &options), &options),
        "<p>ab!Ze</p>\n"
    );
    // Unwriting across cells walks back and truncates the previous
    // cell: "cd" empties and the "!" cell loses its byte.
    let mut options = Options::new();
    options.extension(Arc::new(Chop(3)));
    assert_eq!(
        render_with(&parse_with("ab!cdZe\n", &options), &options),
        "<p>abZe</p>\n"
    );
}

// ==== trigger_rest_of_line_reaches_the_line_end ====
// kills: crates/geist4-core/src/inline/mod.rs:761:9: replace TriggerInner::rest_of_line -> &'a [u8] with Vec::leak(Vec::new())
// kills: crates/geist4-core/src/inline/mod.rs:761:9: replace TriggerInner::rest_of_line -> &'a [u8] with Vec::leak(vec![0])
// kills: crates/geist4-core/src/inline/mod.rs:761:9: replace TriggerInner::rest_of_line -> &'a [u8] with Vec::leak(vec![1])
#[test]
fn trigger_rest_of_line_reaches_the_line_end() {
    // A link-ish extension that links when the rest of the line after
    // the cursor is exactly "Zgo".
    struct RestLink;
    impl Extension for RestLink {
        fn name(&self) -> &str {
            "rest-link"
        }
        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }
        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            let rest = ctx.rest_of_line();
            if rest == b"Zgo" {
                ctx.emit_link(0, rest.len(), "u".to_owned());
            }
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(RestLink));
    // The view starts at the cursor (so it includes the trigger byte
    // here) and ends at the newline: the link takes exactly "Zgo" and
    // the next line stays plain text.
    assert_eq!(
        render_with(&parse_with("Zgo\nmore\n", &options), &options),
        "<p><a href=\"u\">Zgo</a>\nmore</p>\n"
    );
    // A later line bounds the view at its own newline, not the
    // paragraph's start or end.
    assert_eq!(
        render_with(&parse_with("x\nZgo\n", &options), &options),
        "<p>x\n<a href=\"u\">Zgo</a></p>\n"
    );
    // A different rest leaves the trigger byte as literal text.
    assert_eq!(
        render_with(&parse_with("Zstop\n", &options), &options),
        "<p>Zstop</p>\n"
    );
}

// ==== inline_trigger_rest_of_line_stops_at_the_next_newline ====
#[test]
fn inline_trigger_rest_of_line_stops_at_the_next_newline() {
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::parser::parse_with;

    struct RestProbe(Mutex<Vec<String>>);

    impl Extension for RestProbe {
        fn name(&self) -> &str {
            "rest-of-line-probe"
        }

        fn inline_triggers(&self) -> &[u8] {
            b":"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            let rest = ctx.rest_of_line();
            self.0
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(rest).into_owned());
        }
    }

    // A two-line paragraph's inline subject contains a literal '\n'
    // where the second line joined the first, so rest_of_line() at the
    // ':' trigger must stop at that newline and must start at the
    // cursor, i.e. include the trigger byte itself: ":cd".
    let src = "para\n\nab:cd\nef\n";
    let probe = Arc::new(RestProbe(Mutex::new(Vec::new())));
    let mut options = Options::new();
    options.extension(probe.clone());
    let _doc = parse_with(src, &options);
    assert_eq!(*probe.0.lock().unwrap(), [":cd".to_owned()]);
}

// ==== inline_trigger_line_prefix_excludes_the_preceding_newline ====
#[test]
fn inline_trigger_line_prefix_excludes_the_preceding_newline() {
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::parser::parse_with;

    struct PrefixProbe(Mutex<Vec<String>>);

    impl Extension for PrefixProbe {
        fn name(&self) -> &str {
            "line-prefix-probe"
        }

        fn inline_triggers(&self) -> &[u8] {
            b":"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            let prefix = ctx.line_prefix();
            self.0
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(prefix).into_owned());
        }
    }

    // In subject "ab\ncd:ef" the prefix before the ':' trigger on the
    // second line is "cd"; it must not begin with the '\n' that joined
    // the paragraph lines.
    let src = "para\n\nab\ncd:ef\n";
    let probe = Arc::new(PrefixProbe(Mutex::new(Vec::new())));
    let mut options = Options::new();
    options.extension(probe.clone());
    let _doc = parse_with(src, &options);
    assert_eq!(*probe.0.lock().unwrap(), ["cd".to_owned()]);
}

// ==== inline_trigger_in_bracket_is_false_outside_brackets ====
#[test]
fn inline_trigger_in_bracket_is_false_outside_brackets() {
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::parser::parse_with;

    struct BracketProbe(Mutex<Vec<bool>>);

    impl Extension for BracketProbe {
        fn name(&self) -> &str {
            "bracket-probe"
        }

        fn inline_triggers(&self) -> &[u8] {
            b"w"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            self.0.lock().unwrap().push(ctx.in_bracket());
        }
    }

    // A `w` trigger in an ordinary paragraph is not inside a bracket
    // link, so in_bracket() must report false.
    let src = "para\n\nw\n";
    let probe = Arc::new(BracketProbe(Mutex::new(Vec::new())));
    let mut options = Options::new();
    options.extension(probe.clone());
    let _doc = parse_with(src, &options);
    assert_eq!(*probe.0.lock().unwrap(), [false]);
}

// ==== inline_trigger_in_bracket_is_true_inside_a_link_label ====
#[test]
fn inline_trigger_in_bracket_is_true_inside_a_link_label() {
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::parser::parse_with;

    struct BracketProbe(Mutex<Vec<bool>>);

    impl Extension for BracketProbe {
        fn name(&self) -> &str {
            "bracket-probe"
        }

        fn inline_triggers(&self) -> &[u8] {
            b"w"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            self.0.lock().unwrap().push(ctx.in_bracket());
        }
    }

    // Inside "[a w](u)" the label is parsed while the '[' bracket is
    // open, so a trigger inside the label must see in_bracket() == true.
    let src = "para\n\n[a w](u)\n";
    let probe = Arc::new(BracketProbe(Mutex::new(Vec::new())));
    let mut options = Options::new();
    options.extension(probe.clone());
    let _doc = parse_with(src, &options);
    assert_eq!(*probe.0.lock().unwrap(), [true]);
}

// ==== inline_trigger_unput_on_an_entity_cell_keeps_the_remaining_span ====
#[test]
fn inline_trigger_unput_on_an_entity_cell_keeps_the_remaining_span() {
    use geist4_core::arena::NodeId;
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::node::Node;
    use geist4_core::parser::parse_with;

    struct RewindOne;

    impl Extension for RewindOne {
        fn name(&self) -> &str {
            "rewind-one"
        }

        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            ctx.unput(1);
        }
    }

    // In subject "OK&amp;Zq" (paragraph starts at byte 6) the entity
    // cell holds "&" over the 5-byte source span "&amp;" spanning
    // [2,7). unput(1) at the Z trigger must take the equal-length
    // branch of the walk: the emptied cell keeps a span with the
    // remaining entity width [2,6), so following spans stay aligned.
    let src = "para\n\nOK&amp;Zq\n";
    let mut options = Options::new();
    options.extension(Arc::new(RewindOne));
    let doc = parse_with(src, &options);
    let para = *doc
        .arena
        .children(doc.root)
        .iter()
        .rev()
        .find(|&&id| matches!(doc.arena.kind(id), Node::Paragraph))
        .expect("a paragraph");
    let kids = doc.arena.children(para);
    assert_eq!(kids.len(), 3, "paragraph children: {kids:?}");
    let span = |id: NodeId| {
        let s = doc.arena.span(id);
        (s.start, s.end)
    };
    assert!(matches!(doc.arena.kind(kids[0]), Node::Text(t) if t.as_ref() == "OK"));
    assert_eq!(span(kids[0]), (6, 8));
    assert!(
        matches!(doc.arena.kind(kids[1]), Node::Text(t) if t.is_empty()),
        "the entity cell is emptied"
    );
    assert_eq!(
        span(kids[1]),
        (8, 12),
        "the emptied entity cell keeps its remaining span width"
    );
    assert!(matches!(doc.arena.kind(kids[2]), Node::Text(t) if t.as_ref() == "Zq"));
    assert_eq!(span(kids[2]), (13, 15));
}

// ==== inline_trigger_unput_walks_back_across_cells ====
#[test]
fn inline_trigger_unput_walks_back_across_cells() {
    use geist4_core::arena::NodeId;
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::node::Node;
    use geist4_core::parser::parse_with;

    struct RewindSeven;

    impl Extension for RewindSeven {
        fn name(&self) -> &str {
            "rewind-seven"
        }

        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            ctx.unput(7);
        }
    }

    // In subject "abcdef&amp;xyZq" (paragraph starts at byte 6) the
    // cells at hook time are "abcdef" [0,6), the "&amp;" entity [6,11)
    // and "xy" [11,13). unput(7) must empty "xy" and the entity and
    // truncate "abcdef" to "ab" with span [0,2): the remaining-byte
    // counter shrinks by each cell's text length as the walk moves
    // back, and a cell shorter than the remainder is emptied with its
    // span collapsed to its start.
    let src = "para\n\nabcdef&amp;xyZq\n";
    let mut options = Options::new();
    options.extension(Arc::new(RewindSeven));
    let doc = parse_with(src, &options);
    let para = *doc
        .arena
        .children(doc.root)
        .iter()
        .rev()
        .find(|&&id| matches!(doc.arena.kind(id), Node::Paragraph))
        .expect("a paragraph");
    let kids = doc.arena.children(para);
    assert_eq!(kids.len(), 4, "paragraph children: {kids:?}");
    let span = |id: NodeId| {
        let s = doc.arena.span(id);
        (s.start, s.end)
    };
    assert!(matches!(doc.arena.kind(kids[0]), Node::Text(t) if t.as_ref() == "ab"));
    assert_eq!(span(kids[0]), (6, 8));
    assert!(matches!(doc.arena.kind(kids[1]), Node::Text(t) if t.is_empty()));
    assert_eq!(span(kids[1]), (12, 12), "emptied entity maps to its start");
    assert!(matches!(doc.arena.kind(kids[2]), Node::Text(t) if t.is_empty()));
    assert_eq!(span(kids[2]), (17, 17), "emptied run maps to its start");
    assert!(matches!(doc.arena.kind(kids[3]), Node::Text(t) if t.as_ref() == "Zq"));
    assert_eq!(span(kids[3]), (19, 21));
}

// ==== inline_trigger_emit_link_text_covers_the_span_from_rewind ====
#[test]
fn inline_trigger_emit_link_text_covers_the_span_from_rewind() {
    use geist4_core::arena::NodeId;
    use geist4_core::extension::{Extension, InlineTriggerCtx, Options};
    use geist4_core::node::Node;
    use geist4_core::parser::parse_with;

    struct SpanLink;

    impl Extension for SpanLink {
        fn name(&self) -> &str {
            "span-link"
        }

        fn inline_triggers(&self) -> &[u8] {
            b"Z"
        }

        fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
            ctx.emit_link(2, 1, "u".to_owned());
        }
    }

    // In subject "abcdefgZq" (paragraph starts at byte 6) emit_link
    // (rewind 2, take 1) at the Z trigger unwrites the two bytes
    // before the cursor into a final "abcde" text cell [0,5) mapped to
    // (6,11), then emits a link spanning [5,8) mapped to (11,14) whose
    // child text is exactly the span's bytes "fgZ", leaving "q" [8,9)
    // mapped to (14,15) as the tail.
    let src = "para\n\nabcdefgZq\n";
    let mut options = Options::new();
    options.extension(Arc::new(SpanLink));
    let doc = parse_with(src, &options);
    let para = *doc
        .arena
        .children(doc.root)
        .iter()
        .rev()
        .find(|&&id| matches!(doc.arena.kind(id), Node::Paragraph))
        .expect("a paragraph");
    let kids = doc.arena.children(para);
    assert_eq!(kids.len(), 3, "paragraph children: {kids:?}");
    let span = |id: NodeId| {
        let s = doc.arena.span(id);
        (s.start, s.end)
    };
    assert!(matches!(doc.arena.kind(kids[0]), Node::Text(t) if t.as_ref() == "abcde"));
    assert_eq!(span(kids[0]), (6, 11));
    assert!(matches!(doc.arena.kind(kids[1]), Node::Link(_)));
    assert_eq!(span(kids[1]), (11, 14));
    let link_kids = doc.arena.children(kids[1]);
    assert_eq!(link_kids.len(), 1);
    assert!(
        matches!(doc.arena.kind(link_kids[0]), Node::Text(t) if t.as_ref() == "fgZ"),
        "the link text is the emitted span's bytes"
    );
    assert!(matches!(doc.arena.kind(kids[2]), Node::Text(t) if t.as_ref() == "q"));
    assert_eq!(span(kids[2]), (14, 15));
}
