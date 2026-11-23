//! Regression tests for missed cargo-mutants survivors in the extension
//! hook contexts and the registry (`src/extension.rs`).
//!
//! These are behavioral tests written against the public API only. Each
//! test carries `// kills:` comments naming the mutant IDs (the
//! `path:line:col: description` entries reported by cargo-mutants) it is
//! designed to catch, so a future mutant run can trace every survivor to
//! the test that should have killed it.

use geist4_core::arena::NodeId;
use geist4_core::extension::{
    BlockContinueCtx, BlockStartCtx, ContinueOutcome, Extension, KindDecl, Options,
};
use geist4_core::node::Node;
use geist4_core::parser::parse_with;
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

/// The first child of `id` whose kind matches, panicking otherwise: the
/// assertions below expect the child to exist, and a panic is a failure.
fn find_child(
    doc: &geist4_core::Document<'_>,
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

// ---------------------------------------------------------------------------
// Options: is_empty, Clone and Debug are public API contracts.
// ---------------------------------------------------------------------------

/// An extension that opens one block kind: enough for registry checks.
struct Wedge;

impl Extension for Wedge {
    fn name(&self) -> &str {
        "wedge"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "wedge".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        // The line accessor hands the content after the container
        // prefixes; an empty leak there never matches.
        if ctx.line() != b"%%wedge" {
            return;
        }
        // The marker line is not blank.
        if ctx.line_blank() {
            return;
        }
        // The hook runs once per registered extension, so the first
        // registered extension sees its own index 0.
        if ctx.extension() != 0 {
            return;
        }
        let node = ctx.open_extension("wedge".to_owned(), Box::new(0usize));
        let text = ctx
            .arena()
            .alloc(Node::Text(Cow::Owned("pin".to_owned())), Span::empty(0));
        ctx.arena().push_child(node, text);
    }
}

/// A second extension: its hook only opens when it really runs at index 1.
struct Tag;

impl Extension for Tag {
    fn name(&self) -> &str {
        "tag"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "tag".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.extension() != 1 {
            return;
        }
        if ctx.line() != b"%%tag" {
            return;
        }
        ctx.open_extension("tag".to_owned(), Box::new(0usize));
    }
}

#[test]
fn options_registry_reports_and_keeps_registrations() {
    // kills: crates/geist4-core/src/extension.rs:753:9: replace Options::is_empty -> bool with false
    // kills: crates/geist4-core/src/extension.rs:764:9: replace <impl Clone for Options>::clone -> Self with Default::default()
    // kills: crates/geist4-core/src/extension.rs:772:9: replace <impl std::fmt::Debug for Options>::fmt -> std::fmt::Result with Ok(Default::default())
    let empty = Options::new();
    // is_empty is the hook sites' short-circuit: it must agree with the
    // registration list.
    assert!(empty.is_empty());
    assert_eq!(
        format!("{empty:?}"),
        "Options { extensions: [], highlighter: false }"
    );

    let mut one = Options::new();
    one.extension(Arc::new(Wedge));
    assert!(!one.is_empty());
    assert_eq!(one.extensions().len(), 1);
    // A clone keeps the registrations; parsing through the clone proves
    // it, so a default clone cannot pass unnoticed.
    let cloned = one.clone();
    assert_eq!(cloned.extensions().len(), 1);
    assert!(cloned.by_name("wedge").is_some());
    let doc = parse_with("%%wedge\n", &cloned);
    assert_eq!(extension_nodes(&doc).len(), 1, "clone lost the extension");
    // The debug form names the registered extensions, so a broken fmt
    // cannot pass unnoticed on a populated list either.
    let debug = format!("{one:?}");
    assert!(debug.starts_with("Options {"));
    assert!(debug.contains("wedge"), "debug was {debug:?}");
}

#[test]
fn start_hooks_open_only_on_their_own_index_line_and_arena() {
    // kills: crates/geist4-core/src/extension.rs:515:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::extension -> usize with 1
    // kills: crates/geist4-core/src/extension.rs:521:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::line -> &'l[u8] with Vec::leak(Vec::new())
    // kills: crates/geist4-core/src/extension.rs:526:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::line_blank -> bool with true
    // kills: crates/geist4-core/src/extension.rs:549:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::arena -> &mut crate::arena::Arena<'a> with Box::leak(Box::new(Default::default()))
    let mut options = Options::new();
    options.extension(Arc::new(Wedge));
    options.extension(Arc::new(Tag));
    // The wedge hook sees its own registration index, the real marker
    // bytes and a non-blank line: it opens and builds a child through
    // the parse's own arena.
    let doc = parse_with("%%wedge\n", &options);
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one wedge block, got {exts:?}");
    assert_eq!(exts[0].1, "wedge");
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 1, "wedge block lost its arena-built child");
    assert!(is_text_of("pin")(doc.arena.kind(kids[0])));
    // The tag hook must still open on its own line. The wedge half above
    // is what catches a wrong index read: the wedge's own index is 0, so
    // a forced 1 makes it decline its marker and no block appears.
    let tagged = parse_with("%%tag\n", &options);
    let tags = extension_nodes(&tagged);
    assert_eq!(tags.len(), 1, "expected one tag block, got {tags:?}");
    assert_eq!(tags[0].1, "tag");
}

// ---------------------------------------------------------------------------
// BlockStartCtx: repeated marker lines are absorbed into the open block.
// ---------------------------------------------------------------------------

/// An extension block that swallows repeated marker lines and closes on
/// its own fence line.
struct Ledger;

impl Extension for Ledger {
    fn name(&self) -> &str {
        "ledger"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "ledger".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%go" {
            return;
        }
        if ctx.open_node().is_some() {
            // A repeated marker line is content of the open block, not
            // a new block.
            ctx.absorb_line();
        } else {
            ctx.open_extension("ledger".to_owned(), Box::new(0usize));
        }
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        if ctx.line() == b"%%cut" {
            return ContinueOutcome::CloseAndConsume;
        }
        if ctx.pure_blank() {
            return ContinueOutcome::Close;
        }
        ContinueOutcome::Continue
    }
}

#[test]
fn repeated_marker_line_is_absorbed_into_the_open_block() {
    // kills: crates/geist4-core/src/extension.rs:557:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::open_node -> Option<crate::arena::NodeId> with None
    // kills: crates/geist4-core/src/extension.rs:577:9: replace BlockStartCtx<'p, 'a, 'l, 'ext>::absorb_line with ()
    let mut options = Options::new();
    options.extension(Arc::new(Ledger));
    let doc = parse_with("%%go\n%%go\nslot\n", &options);
    // The second marker line must be absorbed into the one open block:
    // `open_node` reports it, `absorb_line` consumes the line.
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one ledger block, got {exts:?}");
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 1, "absorbed line must add nothing");
    let para = find_child(&doc, exts[0].0, &is_paragraph);
    assert!(is_text_of("slot")(
        doc.arena.kind(doc.arena.children(para)[0])
    ));
}

#[test]
fn closing_fence_line_is_consumed_whole() {
    // kills: crates/geist4-core/src/extension.rs:680:9: replace BlockContinueCtx<'c, 's, 'l>::line -> &'c[u8] with Vec::leak(vec![0])
    let mut options = Options::new();
    options.extension(Arc::new(Ledger));
    let doc = parse_with("%%go\nbrim\n%%cut\ntail\n", &options);
    // The `%%cut` line is the block's closing fence: the continuation
    // hook must see the real line bytes to close and consume it. A
    // leaked one-byte NUL slice never matches, so the fence and the
    // tail would end up inside the block instead.
    let root_kids = doc.arena.children(doc.root);
    assert_eq!(root_kids.len(), 2, "root children: {root_kids:?}");
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one ledger block, got {exts:?}");
    // The tail paragraph is a child of the document, not of the block.
    let tail = doc.arena.children(root_kids[1])[0];
    assert!(is_text_of("tail")(doc.arena.kind(tail)));
    // The block holds only its one brim paragraph.
    let block_kids = doc.arena.children(exts[0].0);
    assert_eq!(block_kids.len(), 1, "block children: {block_kids:?}");
    let brim = doc.arena.children(block_kids[0])[0];
    assert!(is_text_of("brim")(doc.arena.kind(brim)));
}

// ---------------------------------------------------------------------------
// BlockContinueCtx: the cursor column decides whether a block continues
// under container prefixes.
// ---------------------------------------------------------------------------

/// An extension block that only continues at two or more columns of
/// container prefix.
struct Deeply;

impl Extension for Deeply {
    fn name(&self) -> &str {
        "deeply"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "deeply".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() == b"%%dip" {
            ctx.open_extension("deeply".to_owned(), Box::new(0usize));
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
    options.extension(Arc::new(Deeply));
    let doc = parse_with("> %%dip\n> cell\n\ntail\n", &options);
    // The quoted line continues the block (column two); the blank line
    // ends the lazy paragraph and closes quote and block, so the
    // unquoted tail becomes a paragraph of the document.
    let quote = find_child(&doc, doc.root, &|node: &Node<'_>| {
        matches!(node, Node::BlockQuote)
    });
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one deeply block, got {exts:?}");
    assert_eq!(doc.arena.parent(exts[0].0), Some(quote));
    let para = find_child(&doc, exts[0].0, &is_paragraph);
    assert!(is_text_of("cell")(
        doc.arena.kind(doc.arena.children(para)[0])
    ));
    let tail = find_child(&doc, doc.root, &is_paragraph);
    assert!(is_text_of("tail")(
        doc.arena.kind(doc.arena.children(tail)[0])
    ));
}
// ==== a_pre_paragraphs_inlines_span_through_the_block_start ====
/// Converts an open paragraph into a `split` block when the next line
/// is `%%split`; the whole paragraph text becomes the pre-paragraph in
/// front of the new block.
struct Splitter;

impl Extension for Splitter {
    fn name(&self) -> &str {
        "splitter"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "split".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%split" {
            return;
        }
        if let Some(text) = ctx.paragraph_text() {
            // The current line is not in the buffer yet: the whole
            // buffer becomes the pre-paragraph and the line itself the
            // new block's first line.
            ctx.convert_paragraph("split".to_owned(), text.len(), Box::new(0usize));
        }
    }
}

#[test]
fn a_pre_paragraphs_inlines_span_through_the_block_start() {
    // kills: crates/geist4-core/src/inline/mod.rs:464:36: replace + with * in attach_pool
    // kills: crates/geist4-core/src/inline/mod.rs:465:36: replace + with * in attach_pool
    // A paragraph an extension converts out of keeps no text map — its
    // pre text was unescaped and trimmed — so its inline spans come
    // from the fallback `fallback_start + cell` arithmetic in
    // `attach_pool`. For the pre paragraph `intro *b*` starting at
    // byte 0 that is 6..9 for the emphasis. With `*` for `+` the
    // fallback collapses to a product, which the pinned span rejects.
    let mut options = Options::new();
    options.extension(Arc::new(Splitter));
    let src = "intro *b*\n%%split\n";
    let doc = parse_with(src, &options);
    let pre = find_child(&doc, doc.root, &is_paragraph);
    let emph = find_child(&doc, pre, &|node| matches!(node, Node::Emphasis));
    assert_eq!(doc.arena.span(emph), Span::new(6, 9));
}
