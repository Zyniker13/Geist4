//! Regression tests for missed cargo-mutants survivors in the parser's
//! free functions and its block-start context plumbing
//! (`src/parser/mod.rs`): the front-matter offset arithmetic in
//! `split_off_front_matter`, the table-cell pipe unescaping in
//! `unescape_pipes_trim`, and the `StartInner` accessors that feed the
//! extension block-start hook context.
//!
//! These are behavioral tests written against the public API only. Each
//! test carries `// kills:` comments naming the mutant positions (the
//! `path:line:col` entries reported by cargo-mutants) it is designed to
//! catch, so a future mutant run can trace every survivor to the test
//! that should have killed it.

use geist4_core::arena::NodeId;
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, ContinueOutcome, ExtNode, Extension, KindDecl,
    Options,
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

/// The root's first child as a `(span, source)` pair of the
/// `front_matter` node, panicking when that child is not one: every
/// assertion below expects the front matter to be claimed.
fn front_matter_node(doc: &geist4_core::Document<'_>) -> (Span, String) {
    let children = doc.arena.children(doc.root);
    let id = children[0];
    match doc.arena.kind(id) {
        Node::Extension(ext) => {
            assert_eq!(ext.kind, "front_matter");
            assert_eq!(ext.attrs[0].0, "source");
            match &ext.attrs[0].1 {
                AttrValue::Str(source) => (doc.arena.span(id), source.clone()),
                other => panic!("expected a string attribute, got {other:?}"),
            }
        }
        other => panic!("expected an extension node as the first child, got {other:?}"),
    }
}

/// The first child of `id` as text, panicking when there is none: the
/// assertions below expect each paragraph to hold exactly one text run.
fn first_text(doc: &geist4_core::Document<'_>, id: NodeId) -> String {
    match doc.arena.kind(doc.arena.children(id)[0]) {
        Node::Text(t) => t.as_ref().to_owned(),
        other => panic!("expected a text child, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// split_off_front_matter: the byte offsets of the claimed prologue.
// ---------------------------------------------------------------------------

/// A front matter extension with a fixed delimiter, as any loaded
/// extension would declare one.
struct Delimited(&'static str);

impl Extension for Delimited {
    fn name(&self) -> &str {
        "front-matter-probe"
    }

    fn front_matter_delimiter(&self) -> Option<&str> {
        Some(self.0)
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "front_matter".to_owned(),
            container: false,
            inline_text: false,
            attrs: vec!["source".to_owned()],
        }]
    }

    fn render(
        &self,
        node: &ExtNode,
        _children: &[geist4_core::extension::RenderedChild<'_>],
        _out: &mut String,
    ) {
        let _ = node;
    }
}

fn front_matter_options() -> Options {
    let mut options = Options::new();
    options.extension(Arc::new(Delimited("---")));
    options
}

#[test]
fn front_matter_span_and_source_survive_offset_arithmetic() {
    // kills: crates/geist4-core/src/parser/mod.rs:195:12: replace `+=` with `-=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:195:12: replace `+=` with `*=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:197:12: replace `+=` with `-=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:197:12: replace `+=` with `*=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:212:12: replace `+=` with `-=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:212:12: replace `+=` with `*=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:219:12: replace `+=` with `-=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:219:12: replace `+=` with `*=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:221:12: replace `+=` with `-=` in split_off_front_matter
    // kills: crates/geist4-core/src/parser/mod.rs:221:12: replace `+=` with `*=` in split_off_front_matter
    let cases: &[(&str, usize, &str, &str)] = &[
        // (source, span end, prologue text, following body text)
        // The plain case: opener, one body line, closer, body.
        ("---\nb\n---\nbody\n", 10, "---\nb\n---\n", "body"),
        // The body's first line is a delimiter line: the closer search
        // must start past the opener's line ending, or it anchors one
        // line early and claims only two delimiter lines.
        ("---\n---\n---\nz\n", 12, "---\n---\n---\n", "z"),
        // A CRLF opener: the opener scan must step over both bytes, or
        // the body search starts inside the carriage return and finds
        // an earlier closer.
        ("---\r\n---\n---\nzz\n", 13, "---\r\n---\n---\n", "zz"),
        // An empty first body line after the CRLF opener: with the
        // search started one byte late the only `\n---` candidate is
        // consumed by the line feed the opener already stepped over,
        // and nothing is claimed at all.
        ("---\r\n\n---\nbody\n", 10, "---\r\n\n---\n", "body"),
        // Blank lines before the body: the closer's line ending
        // handling must eat exactly the one ending it stands on, so
        // both blank lines stay behind the prologue.
        ("---\nb\n---\n\n\nbody\n", 11, "---\nb\n---\n\n", "body"),
        // A CRLF closer: its carriage return belongs to the prologue.
        ("---\nb\n---\r\nbody\n", 11, "---\nb\n---\r\n", "body"),
        // A line feed followed by a blank carriage-return line: the
        // closer's line feed is eaten, the next line's carriage return
        // and line feed stay in the body.
        ("---\nb\n---\n\r\nbody\n", 12, "---\nb\n---\n\r\n", "body"),
    ];
    for &(source, end, prologue, body) in cases {
        let doc = parse_with(source, &front_matter_options());
        let (span, attr) = front_matter_node(&doc);
        assert_eq!(span, Span::new(0, end), "span for {source:?}");
        assert_eq!(attr, prologue, "source attr for {source:?}");
        // The prologue is the root's first child; the body paragraph
        // starts at its first non-blank line, which sits at or after
        // the prologue end.
        let children = doc.arena.children(doc.root);
        assert_eq!(children.len(), 2, "root children for {source:?}");
        assert!(
            doc.arena.span(children[1]).start >= end,
            "body offset for {source:?}"
        );
        assert_eq!(first_text(&doc, children[1]), body, "body for {source:?}");
    }
}

// ---------------------------------------------------------------------------
// unescape_pipes_trim: the paragraph text in front of a converted block.
// ---------------------------------------------------------------------------

/// An extension that converts the open paragraph into a block when a
/// delimiter-only line arrives; everything accumulated before that line
/// moves in front of the new block.
struct Tableish;

impl Extension for Tableish {
    fn name(&self) -> &str {
        "tableish"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "tableish".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b":---" {
            return;
        }
        let Some(text) = ctx.paragraph_text() else {
            return;
        };
        // Everything accumulated before the delimiter row becomes the
        // paragraph in front of the new block.
        let pre_len = text.len();
        ctx.convert_paragraph("tableish".to_owned(), pre_len, Box::new(0usize));
    }

    fn try_continue(&self, _ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        // The block stays open until the end of the document.
        ContinueOutcome::Continue
    }
}

#[test]
fn pre_table_cell_unescapes_pipes_and_trims() {
    // kills: crates/geist4-core/src/parser/mod.rs:229:5: replace unescape_pipes_trim -> String with String::new()
    // kills: crates/geist4-core/src/parser/mod.rs:229:5: replace unescape_pipes_trim -> String with "xyzzy".into()
    // kills: crates/geist4-core/src/parser/mod.rs:233:13: replace `<` with `==` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:233:13: replace `<` with `<=` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:233:13: replace `<` with `>` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:234:21: replace `==` with `!=` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:234:30: replace `&&` with `||` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:234:45: replace `+` with `-` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:234:45: replace `+` with `*` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:234:50: replace `==` with `!=` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:236:15: replace `+=` with `-=` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:236:15: replace `+=` with `*=` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:240:15: replace `+=` with `-=` in unescape_pipes_trim
    // kills: crates/geist4-core/src/parser/mod.rs:240:15: replace `+=` with `*=` in unescape_pipes_trim
    let mut options = Options::new();
    options.extension(Arc::new(Tableish));
    // The paragraph before the delimiter row holds an escaped pipe and
    // a multibyte character: the escape must become a literal pipe, not
    // stay escaped or swallow its neighbours. The last line before the
    // delimiter is the block's header row; earlier lines move into the
    // paragraph in front of the block.
    let doc = parse_with("\\|é b\nintro2\n:---\nroster\n", &options);
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one tableish block, got {exts:?}");
    assert_eq!(exts[0].1, "tableish");
    let children = doc.arena.children(doc.root);
    assert_eq!(children.len(), 2, "root children: {children:?}");
    assert!(matches!(doc.arena.kind(children[0]), Node::Paragraph));
    assert_eq!(
        first_text(&doc, children[0]),
        "|é b",
        "the escape must become a literal pipe, not stay escaped or vanish"
    );
    // The converted node keeps its identity as the block that follows.
    assert_eq!(
        exts[0].0, children[1],
        "the converted paragraph becomes the block"
    );
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 1, "block children: {kids:?}");
    assert!(matches!(doc.arena.kind(kids[0]), Node::Paragraph));
    assert_eq!(first_text(&doc, kids[0]), "roster");
}

// ---------------------------------------------------------------------------
// StartInner accessors: what the block-start hook context reports.
// ---------------------------------------------------------------------------

/// An extension that declines every line; it only exists to occupy a
/// registration slot ahead of the keeper.
struct Filler;

impl Extension for Filler {
    fn name(&self) -> &str {
        "filler"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        Vec::new()
    }

    fn try_start(&self, _ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {}
}

/// An extension that opens only when its hook really runs at index two
/// with the marker bytes on a non-blank line.
struct Keeper;

impl Extension for Keeper {
    fn name(&self) -> &str {
        "keeper"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "keeper".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        // The hook runs once per registered extension, so the third
        // registered extension sees its own index 2.
        if ctx.extension() != 2 {
            return;
        }
        // The line accessor hands the content after the container
        // prefixes; an empty or one-byte-NUL leak there never matches.
        if ctx.line() != b"%%k" {
            return;
        }
        // The marker line is not blank.
        if ctx.line_blank() {
            return;
        }
        ctx.open_extension("keeper".to_owned(), Box::new(0usize));
    }
}

#[test]
fn start_hook_sees_its_own_extension_index_line_and_blank_flag() {
    // kills: crates/geist4-core/src/parser/mod.rs:314:9: replace StartInner::extension_index -> usize with 0
    // kills: crates/geist4-core/src/parser/mod.rs:314:9: replace StartInner::extension_index -> usize with 1
    // kills: crates/geist4-core/src/parser/mod.rs:320:9: replace StartInner::line -> &'l[u8] with Vec::leak(Vec::new())
    // kills: crates/geist4-core/src/parser/mod.rs:320:9: replace StartInner::line -> &'l[u8] with Vec::leak(vec![0])
    // kills: crates/geist4-core/src/parser/mod.rs:325:9: replace StartInner::line_blank -> bool with true
    let mut options = Options::new();
    options.extension(Arc::new(Filler));
    options.extension(Arc::new(Filler));
    options.extension(Arc::new(Keeper));
    let doc = parse_with("%%k\n", &options);
    // The keeper opens exactly once, and the marker line never becomes
    // a paragraph.
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one keeper block, got {exts:?}");
    assert_eq!(exts[0].1, "keeper");
    assert_eq!(
        doc.arena.children(doc.root).len(),
        1,
        "the marker line must not become a paragraph"
    );
}

/// An extension that opens a block only on an indented marker line that
/// arrives as the lazy continuation of an open paragraph.
struct Slope;

impl Extension for Slope {
    fn name(&self) -> &str {
        "slope"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "slope".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%i" {
            return;
        }
        // With a paragraph open, four leading spaces do not open an
        // indented code block; the context must report the line as
        // indented all the same.
        if !ctx.indented() {
            return;
        }
        ctx.open_extension("slope".to_owned(), Box::new(0usize));
    }
}

#[test]
fn start_hook_indented_flag_survives_lazy_paragraph_line() {
    // kills: crates/geist4-core/src/parser/mod.rs:331:9: replace StartInner::indented -> bool with false
    let mut options = Options::new();
    options.extension(Arc::new(Slope));
    let doc = parse_with("para\n    %%i\n", &options);
    // The hook opens on the indented line, and the paragraph it
    // interrupts stays in front of the block.
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one slope block, got {exts:?}");
    assert_eq!(exts[0].1, "slope");
    let children = doc.arena.children(doc.root);
    assert_eq!(children.len(), 2, "root children: {children:?}");
    assert!(matches!(doc.arena.kind(children[0]), Node::Paragraph));
    assert_eq!(first_text(&doc, children[0]), "para");
}

/// An extension that opens a block only when the marker line starts at
/// column two.
struct Plinth;

impl Extension for Plinth {
    fn name(&self) -> &str {
        "plinth"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "plinth".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%p" {
            return;
        }
        // Two leading spaces must be reported as two columns of indent.
        if ctx.indent() != 2 {
            return;
        }
        ctx.open_extension("plinth".to_owned(), Box::new(0usize));
    }
}

#[test]
fn start_hook_indent_column_count_is_exact() {
    // kills: crates/geist4-core/src/parser/mod.rs:337:9: replace StartInner::indent -> usize with 1
    let mut options = Options::new();
    options.extension(Arc::new(Plinth));
    let doc = parse_with("  %%p\n", &options);
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one plinth block, got {exts:?}");
    assert_eq!(exts[0].1, "plinth");
    assert_eq!(
        doc.arena.children(doc.root).len(),
        1,
        "the marker line must not become a paragraph"
    );
}

/// An extension that builds its first child through the parse's own
/// arena during the start hook.
struct Quarry;

impl Extension for Quarry {
    fn name(&self) -> &str {
        "quarry"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "quarry".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%q" {
            return;
        }
        let node = ctx.open_extension("quarry".to_owned(), Box::new(0usize));
        let text = ctx.arena().alloc(
            Node::Text(Cow::Owned("made-by-hook".to_owned())),
            Span::empty(0),
        );
        ctx.arena().push_child(node, text);
    }
}

#[test]
fn start_hook_allocations_land_in_the_document() {
    // kills: crates/geist4-core/src/parser/mod.rs:342:9: replace StartInner::arena_mut -> &mut crate::arena::Arena<'a> with Box::leak(Box::new(Default::default()))
    let mut options = Options::new();
    options.extension(Arc::new(Quarry));
    let doc = parse_with("%%q\n", &options);
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one quarry block, got {exts:?}");
    assert_eq!(exts[0].1, "quarry");
    // The hook built the child through the parse's own arena: a leaked
    // stand-in arena would leave the block empty.
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 1, "quarry block lost its arena-built child");
    assert!(matches!(
        doc.arena.kind(kids[0]),
        Node::Text(t) if t.as_ref() == "made-by-hook"
    ));
}

/// An extension block that swallows repeated marker lines.
struct Vault;

impl Extension for Vault {
    fn name(&self) -> &str {
        "vault"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "vault".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%v" {
            return;
        }
        if ctx.open_node().is_some() {
            // A repeated marker line is content of the open block, not
            // a new block.
            ctx.absorb_line();
        } else {
            ctx.open_extension("vault".to_owned(), Box::new(0usize));
        }
    }

    fn try_continue(&self, _ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        // The block stays open until the end of the document.
        ContinueOutcome::Continue
    }
}

#[test]
fn repeated_marker_line_absorbs_into_the_open_block() {
    // kills: crates/geist4-core/src/parser/mod.rs:347:9: replace StartInner::open_node -> Option<crate::arena::NodeId> with None
    let mut options = Options::new();
    options.extension(Arc::new(Vault));
    let doc = parse_with("%%v\n%%v\n", &options);
    // The second marker line is absorbed into the one open block: it
    // must not open a second block and must leave no paragraph behind,
    // neither in the document nor inside the block.
    let exts = extension_nodes(&doc);
    assert_eq!(exts.len(), 1, "expected one vault block, got {exts:?}");
    assert_eq!(exts[0].1, "vault");
    assert_eq!(
        doc.arena.children(doc.root).len(),
        1,
        "the absorbed line must leave no paragraph in the document"
    );
    let kids = doc.arena.children(exts[0].0);
    assert_eq!(kids.len(), 0, "absorbed line must add nothing: {kids:?}");
}

// ---------------------------------------------------------------------------
// Span accuracy: node spans must pin to the source bytes each node was parsed
// from, and list re-entry must keep consecutive same-marker items in one list.
// ---------------------------------------------------------------------------

/// The first direct child of `id` matching `want`.
fn child_of_kind(
    doc: &geist4_core::Document<'_>,
    id: NodeId,
    want: impl Fn(&Node<'_>) -> bool,
) -> Option<NodeId> {
    doc.arena
        .children(id)
        .iter()
        .copied()
        .find(|child| want(doc.arena.kind(*child)))
}

/// Slices `src` at the span of `id`.
fn span_slice<'a>(src: &'a str, doc: &geist4_core::Document<'_>, id: NodeId) -> &'a str {
    let span = doc.arena.span(id);
    &src[span.start..span.end]
}

#[test]
fn html_block_span_starts_at_its_opening_bracket() {
    // kills: crates/geist4-core/src/parser/mod.rs:1104:75: replace + with * in Parser<'a, 'ext>::open_new_blocks
    let src = "x\n<!-- a\n-->\n";
    let doc = parse_with(src, &Options::new());
    let html = child_of_kind(&doc, doc.root, |n| matches!(n, Node::HtmlBlock(_)))
        .expect("html block under root");
    assert_eq!(
        span_slice(src, &doc, html),
        "<!-- a\n-->",
        "the html block span must start at its '<', not earlier"
    );
}

#[test]
fn container_spans_cover_their_last_child() {
    // kills: crates/geist4-core/src/parser/mod.rs:1297:9: replace Parser<'a, 'ext>::raise_ancestor_ends with ()
    let src = "- # h\n";
    let doc = parse_with(src, &Options::new());
    let list =
        child_of_kind(&doc, doc.root, |n| matches!(n, Node::List(_))).expect("list under root");
    assert_eq!(
        span_slice(src, &doc, list),
        "- # h",
        "the list span must cover its heading child to the end of the line"
    );
    let item = child_of_kind(&doc, list, |n| matches!(n, Node::Item(_))).expect("item in list");
    assert_eq!(
        span_slice(src, &doc, item),
        "- # h",
        "the item span must cover the whole list line"
    );
}

#[test]
fn list_items_start_at_their_own_marker() {
    // kills: crates/geist4-core/src/parser/mod.rs:1316:37: replace + with * in Parser<'a, 'ext>::open_list_item
    let src = "x\n- a\n- b\n";
    let doc = parse_with(src, &Options::new());
    let list =
        child_of_kind(&doc, doc.root, |n| matches!(n, Node::List(_))).expect("list under root");
    assert_eq!(
        span_slice(src, &doc, list),
        "- a\n- b",
        "the list span must start at the first item's marker"
    );
    let items = doc.arena.children(list);
    assert_eq!(items.len(), 2, "two items in one list");
    assert_eq!(
        span_slice(src, &doc, items[0]),
        "- a",
        "first item must start at its own marker byte"
    );
    assert_eq!(
        span_slice(src, &doc, items[1]),
        "- b",
        "second item must start at its own marker byte"
    );
}

#[test]
fn consecutive_same_marker_items_share_one_list() {
    // kills: crates/geist4-core/src/parser/mod.rs:1375:12: delete ! in Parser<'a, 'ext>::open_list_item
    let doc = parse_with("x\n- a\n- b\n", &Options::new());
    assert_eq!(
        geist4_core::basic_html::render(&doc),
        "<p>x</p>\n<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n",
        "consecutive items with the same marker belong to a single list"
    );
}

#[test]
fn empty_item_span_covers_marker_and_padding() {
    // kills: crates/geist4-core/src/parser/mod.rs:1398:50: replace + with * in Parser<'a, 'ext>::open_list_item
    let src = "- \n";
    let doc = parse_with(src, &Options::new());
    let list =
        child_of_kind(&doc, doc.root, |n| matches!(n, Node::List(_))).expect("list under root");
    assert_eq!(
        span_slice(src, &doc, list),
        "- ",
        "an empty item's list span must cover the marker plus its padding space"
    );
    let item = child_of_kind(&doc, list, |n| matches!(n, Node::Item(_))).expect("item in list");
    assert_eq!(
        span_slice(src, &doc, item),
        "- ",
        "an empty item's span must cover the marker plus its padding space"
    );
}

#[test]
fn lazy_continuation_text_starts_after_the_tab() {
    // kills: crates/geist4-core/src/parser/mod.rs:1925:40 and :1925:55: replace + with * in Parser<'a, 'ext>::anchored_rest
    let src = "> 1. item\n>\tzz\n";
    let doc = parse_with(src, &Options::new());
    let quote = child_of_kind(&doc, doc.root, |n| matches!(n, Node::BlockQuote))
        .expect("block quote under root");
    let list = child_of_kind(&doc, quote, |n| matches!(n, Node::List(_))).expect("list in quote");
    let item = child_of_kind(&doc, list, |n| matches!(n, Node::Item(_))).expect("item in list");
    let para =
        child_of_kind(&doc, item, |n| matches!(n, Node::Paragraph)).expect("paragraph in item");
    let text = doc
        .arena
        .children(para)
        .iter()
        .copied()
        .find(|id| matches!(doc.arena.kind(*id), Node::Text(t) if t.starts_with("zz")))
        .expect("lazy text node in paragraph");
    assert_eq!(
        span_slice(src, &doc, text),
        "zz",
        "the lazy continuation's text span must point past the tab, not at it"
    );
}

#[test]
fn replacement_map_offsets_skip_the_consumed_prefix() {
    // kills: crates/geist4-core/src/parser/mod.rs:1962:17 (< with ==), :1962:42 (< with >), :1963:55 (+ with *)
    let src = "> \u{0}x\n";
    let doc = parse_with(src, &Options::new());
    let quote = child_of_kind(&doc, doc.root, |n| matches!(n, Node::BlockQuote))
        .expect("block quote under root");
    assert_eq!(
        span_slice(src, &doc, quote),
        "> \u{0}x",
        "the quote's span must start at its '>' marker"
    );
    let para =
        child_of_kind(&doc, quote, |n| matches!(n, Node::Paragraph)).expect("paragraph in quote");
    assert_eq!(
        span_slice(src, &doc, para),
        "\u{0}x",
        "the paragraph span must skip the consumed '> ' prefix"
    );
    let text =
        child_of_kind(&doc, para, |n| matches!(n, Node::Text(_))).expect("text in paragraph");
    assert_eq!(
        span_slice(src, &doc, text),
        "\u{0}x",
        "the text span must include the NUL at its first content byte"
    );
}

#[test]
fn paragraph_span_starts_after_the_quote_prefix() {
    // kills: crates/geist4-core/src/parser/mod.rs:1964:15: replace += with *= in Parser<'a, 'ext>::replacement_map
    let src = "> x\u{0}y\n";
    let doc = parse_with(src, &Options::new());
    let quote = child_of_kind(&doc, doc.root, |n| matches!(n, Node::BlockQuote))
        .expect("block quote under root");
    let para =
        child_of_kind(&doc, quote, |n| matches!(n, Node::Paragraph)).expect("paragraph in quote");
    assert_eq!(
        span_slice(src, &doc, para),
        "x\u{0}y",
        "the paragraph span must start at the first content byte after '> '"
    );
}

#[test]
fn inline_span_after_nul_maps_to_real_source_bytes() {
    // kills: crates/geist4-core/src/parser/mod.rs:1973:24: replace == with != in Parser<'a, 'ext>::replacement_map
    let src = "> x\u{0}y *em*\n";
    let doc = parse_with(src, &Options::new());
    let quote = child_of_kind(&doc, doc.root, |n| matches!(n, Node::BlockQuote))
        .expect("block quote under root");
    let para =
        child_of_kind(&doc, quote, |n| matches!(n, Node::Paragraph)).expect("paragraph in quote");
    let em = child_of_kind(&doc, para, |n| matches!(n, Node::Emphasis)).expect("emphasis");
    assert_eq!(
        span_slice(src, &doc, em),
        "*em*",
        "inline spans after a NUL replacement must map to the real source bytes"
    );
}

#[test]
fn heading_text_span_starts_at_the_first_content_byte() {
    // kills: crates/geist4-core/src/parser/mod.rs:1998:17 (< with ==), :1998:42 (< with >),
    // :1999:22 (+= with *=), :1999:45 (== with !=), :1999:51 (* with +) in replacement_range_map
    let src = "# x\u{0}y\n";
    let doc = parse_with(src, &Options::new());
    let heading =
        child_of_kind(&doc, doc.root, |n| matches!(n, Node::Heading(_))).expect("heading at root");
    let text = child_of_kind(&doc, heading, |n| matches!(n, Node::Text(_))).expect("heading text");
    assert_eq!(
        span_slice(src, &doc, text),
        "x\u{0}y",
        "the heading text span must start at the first content byte and cover the whole text"
    );
}
