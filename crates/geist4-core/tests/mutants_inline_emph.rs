//! Regression tests for missed mutants in the emphasis and delimiter
//! machinery of the inline parser, from a cargo-mutants run: the
//! delimiter stack's cell wiring, trigger-byte walk-through during
//! flanking measurement, reference-kind selection, the inline-bracket
//! claim's content splitting, extension pair splicing, and emphasis
//! span bookkeeping. Each test pins behavior at a boundary the mutants
//! moved, so a mutated parser produces visibly wrong output, a wrong
//! span, or a panic.

use geist4_core::Document;
use geist4_core::arena::NodeId;
use geist4_core::basic_html::render_with;
use geist4_core::extension::{Extension, InlineBracketCtx, InlineTriggerCtx, KindDecl, Options};
use geist4_core::node::{LinkRefKind, Node};
use geist4_core::parser::parse_with;
use std::sync::Arc;

fn html_of(src: &str) -> String {
    render_with(&parse_with(src, &Options::new()), &Options::new())
}

fn html_of_with(src: &str, options: &Options) -> String {
    render_with(&parse_with(src, options), options)
}

fn paragraph(doc: &Document<'_>) -> NodeId {
    for &child in doc.arena.children(doc.root) {
        if matches!(doc.arena.kind(child), Node::Paragraph) {
            return child;
        }
    }
    panic!("no paragraph under the root");
}

/// The span of `id` as a range pair plus the source slice it covers.
/// Asserting the numeric pair catches spans that poke past the input,
/// which slicing alone would panic on.
fn span_of<'a>(src: &'a str, doc: &Document<'_>, id: NodeId) -> (&'a str, (usize, usize)) {
    let span = doc.arena.span(id);
    assert!(
        span.end <= src.len(),
        "span {}..{} pokes past the {}-byte input",
        span.start,
        span.end,
        src.len()
    );
    (&src[span.start..span.end], (span.start, span.end))
}

/// The first extension node under `para`: its kind name, its node id,
/// and its children's texts in order.
fn ext_under(doc: &Document<'_>, para: NodeId) -> (String, NodeId, Vec<String>) {
    for &child in doc.arena.children(para) {
        if let Node::Extension(ext) = doc.arena.kind(child) {
            let texts = doc
                .arena
                .children(child)
                .iter()
                .map(|&c| match doc.arena.kind(c) {
                    Node::Text(t) => t.to_string(),
                    other => panic!("unexpected extension child {other:?}"),
                })
                .collect();
            return (ext.kind.clone(), child, texts);
        }
    }
    panic!("no extension node under the paragraph");
}

/// Registers `%` as an inline trigger byte whose hook does nothing, so
/// the byte stays literal text while splitting text runs at it.
struct Pct;

impl Extension for Pct {
    fn name(&self) -> &str {
        "pct"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"%"
    }
}

/// Pairs `~` runs into `strike` extension nodes, as a strikethrough
/// extension would. Any other trigger byte registered alongside it
/// stays literal text, this extension's hook never firing for it.
struct Strike;

impl Extension for Strike {
    fn name(&self) -> &str {
        "strike"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"~"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "strike".to_owned(),
            container: true,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let run = ctx.scan_run();
        ctx.push_delim(&run, "strike");
    }
}

/// Claims every `[`…`]` group whose content starts with `prefix` as a
/// `claimed` node holding the content's first `end` bytes.
struct Claimer {
    prefix: &'static [u8],
    end: usize,
}

impl Extension for Claimer {
    fn name(&self) -> &str {
        "claimer"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "claimed".to_owned(),
            container: true,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        if ctx.content_starts_with(self.prefix) {
            ctx.claim_with_content("claimed".to_owned(), Vec::new(), self.end);
        }
    }
}

#[test]
fn full_reference_links_carry_the_full_kind() {
    // kills: crates/geist4-core/src/inline/mod.rs:1517:40: delete ! in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // A full reference `[t][ref]` scans a non-empty label and must take
    // the `Full` arm; a collapsed `[t][]` scans an empty one and must
    // not. Dropping the `!` swaps the two: the full form reports
    // `Collapsed` and the collapsed form reports `Full`.
    let src = "[ref]: /u\n[t]: /v\n\n[t][ref]\n\n[t][]\n";
    let doc = parse_with(src, &Options::new());
    let mut links = Vec::new();
    for &block in doc.arena.children(doc.root) {
        if matches!(doc.arena.kind(block), Node::Paragraph) {
            for &child in doc.arena.children(block) {
                if let Node::Link(data) = doc.arena.kind(child) {
                    links.push((
                        data.url.to_string(),
                        data.reference
                            .as_ref()
                            .map(|r| (r.label.to_string(), r.kind)),
                    ));
                }
            }
        }
    }
    assert_eq!(links.len(), 2, "both paragraphs hold one link");
    assert_eq!(links[0].0, "/u");
    assert_eq!(links[0].1, Some(("ref".to_owned(), LinkRefKind::Full)));
    assert_eq!(links[1].0, "/v");
    assert_eq!(links[1].1, Some(("t".to_owned(), LinkRefKind::Collapsed)));
}

#[test]
fn a_closer_after_a_trigger_byte_looks_past_it() {
    // kills: crates/geist4-core/src/inline/mod.rs:1378:17: delete ! in Parser<'defs, 'a, 'ext>::is_walk_through
    // Measuring the second `*` run's flanks, `char_before` must step
    // over the literal `%` the trigger hook left behind and report `a`.
    // With the `!` gone the walk stops on `%`, the punctuation
    // before-character breaks right-flanking, and the pair never forms.
    let mut options = Options::new();
    options.extension(Arc::new(Pct));
    assert_eq!(html_of_with("*a%*b\n", &options), "<p><em>a%</em>b</p>\n");
}

#[test]
fn registered_triggers_do_not_make_every_byte_transparent() {
    // kills: crates/geist4-core/src/inline/mod.rs:1378:48: replace && with || in Parser<'defs, 'a, 'ext>::is_walk_through
    // With a trigger byte registered, ordinary bytes must stay opaque:
    // the closer of `*a*` measures `a` behind it and the pair forms.
    // With `&&` as `||` every byte walks through, the closer measures
    // whitespace on both sides, can neither open nor close, and the
    // emphasis disappears.
    let mut options = Options::new();
    options.extension(Arc::new(Pct));
    assert_eq!(html_of_with("*a*\n", &options), "<p><em>a</em></p>\n");
}

#[test]
fn claimed_node_span_covers_the_claimed_bytes() {
    // The claimed node covers the bracket group, with its end on the
    // boundary between kept and refused bytes: for `[abcdef]` with end
    // 2 that is bytes 0..3, starting at the `[` and ending between `b`
    // and `c` in the source. An arithmetic slip in the node's end
    // formula pushes the span past the boundary, into the bytes the
    // hook refused; a start slip loses the opener bracket.
    let mut options = Options::new();
    options.extension(Arc::new(Claimer {
        prefix: b"ab",
        end: 2,
    }));
    let src = "[abcdef]\n";
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    let (kind, ext, children) = ext_under(&doc, para);
    assert_eq!(kind, "claimed");
    assert_eq!(children, vec!["ab".to_owned()]);
    let (_, ext_span) = span_of(src, &doc, ext);
    assert_eq!(ext_span, (0, 3));
}

#[test]
fn claim_split_keeps_the_head_and_hands_the_rest_back() {
    // kills: crates/geist4-core/src/inline/mod.rs:1662:45: replace >= with < in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1667:45: replace + with * in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1667:45: replace + with - in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1669:46: replace += with *= in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1685:51: replace && with || in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1685:54: delete ! in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1697:36: delete ! in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1704:59: replace + with - in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1686:45: replace - with + in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1686:45: replace - with / in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // Splitting the single cell `abcdef` at claimed end 2 keeps `ab` as
    // the node's child and hands `cdef` back as literal text spanning
    // the bytes it came from. The consumed total must grow by the cell
    // length (not stall at a product), the cut must survive at 2 for a
    // clean boundary (not be walked down to nothing or capped away),
    // the rest node must be created with the span its bytes occupy,
    // and the kept list's last entry must be read with a plain index.
    // The node's span ends at the kept bytes: byte 3, not the opener
    // plus the named end.
    let mut options = Options::new();
    options.extension(Arc::new(Claimer {
        prefix: b"ab",
        end: 2,
    }));
    let src = "[abcdef]\n";
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    let children = doc.arena.children(para);
    assert_eq!(children.len(), 2, "node plus the literal rest");
    let (kind, ext, texts) = ext_under(&doc, para);
    assert_eq!(kind, "claimed");
    assert_eq!(texts, vec!["ab".to_owned()]);
    let (_, ext_span) = span_of(src, &doc, ext);
    assert_eq!(ext_span, (0, 3));
    let (rest, rest_span) = span_of(src, &doc, children[1]);
    assert_eq!(rest, "cdef");
    assert_eq!(rest_span, (3, 7));
}

#[test]
fn claim_split_in_a_later_cell_cuts_where_the_end_names() {
    // kills: crates/geist4-core/src/inline/mod.rs:1681:55: replace - with + in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1682:44: replace > with < in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // The scan coalesces the trigger byte into the following run, so
    // the cells are `ab` and `%cdef`. The walk reaches `%cdef` with
    // two content bytes consumed, so the cut lands at `end - consumed`
    // = 3 with claimed end 5: `%cd` stays in the node, `ef` comes back
    // literal. With `-` as `+` the cut overshoots the cell and the cap
    // swallows the rest whole; with `>` as `<` the cap fires on every
    // clean cut and keeps the whole cell. Both lose the literal `ef`.
    let mut options = Options::new();
    options.extension(Arc::new(Pct));
    options.extension(Arc::new(Claimer {
        prefix: b"ab",
        end: 5,
    }));
    let src = "[ab%cdef]\n";
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    let children = doc.arena.children(para);
    assert_eq!(children.len(), 2, "node plus the literal rest");
    let (kind, ext, texts) = ext_under(&doc, para);
    assert_eq!(kind, "claimed");
    assert_eq!(texts, vec!["ab".to_owned(), "%cd".to_owned()]);
    let (_, ext_span) = span_of(src, &doc, ext);
    assert_eq!(ext_span, (0, 6));
    let (rest, rest_span) = span_of(src, &doc, children[1]);
    assert_eq!(rest, "ef");
    assert_eq!(rest_span, (6, 8));
}

#[test]
fn claim_split_rounds_a_cut_down_to_a_character_boundary() {
    // kills: crates/geist4-core/src/inline/mod.rs:1685:47: replace > with < in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1686:45: replace -= with /= in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // The claimed end 3 lands inside the second `é` of six bytes. The
    // boundary walk must step the cut back to byte 2, keeping `é` as
    // the node's child and handing `éé` back whole. With `<` for `>`
    // the walk never runs and the split slices mid-character; with
    // `/=` for `-=` the walk cannot move and spins forever.
    let mut options = Options::new();
    options.extension(Arc::new(Claimer {
        prefix: "\u{e9}".as_bytes(),
        end: 3,
    }));
    let src = "[ééé]\n";
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    let children = doc.arena.children(para);
    assert_eq!(children.len(), 2, "node plus the literal rest");
    let (kind, ext, texts) = ext_under(&doc, para);
    assert_eq!(kind, "claimed");
    assert_eq!(texts, vec!["é".to_owned()]);
    let (rest, rest_span) = span_of(src, &doc, children[1]);
    assert_eq!(rest, "éé");
    assert_eq!(rest_span, (3, 7));
    // The claimed group starts at the opener bracket (byte 0) and its
    // end lands on the rounded cut (byte 3).
    let (_, ext_span) = span_of(src, &doc, ext);
    assert_eq!(ext_span, (0, 3));
}

#[test]
fn an_extension_pair_adopts_every_node_between_its_runs() {
    // kills: crates/geist4-core/src/inline/mod.rs:1967:64: replace == with != in Parser<'defs, 'a, 'ext>::insert_extension
    // The three nodes between the two `~~` runs move into the `strike`
    // node as its children in order; adjacent text cells coalesce, so
    // the trigger byte's `%` and the following `cd` arrive as one cell.
    // With `==` flipped to `!=` the wiring inverts: the first child's
    // next is cut and the last child keeps pointing at the run that is
    // about to be killed, so the middle nodes drop out of the tree.
    let mut options = Options::new();
    options.extension(Arc::new(Strike));
    options.extension(Arc::new(Pct));
    let src = "~~ab%cd~~\n";
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    let (kind, _, texts) = ext_under(&doc, para);
    assert_eq!(kind, "strike");
    assert_eq!(texts, vec!["ab".to_owned(), "%cd".to_owned()]);
}

#[test]
fn nested_emphasis_leftovers_nest_by_position() {
    // kills: crates/geist4-core/src/inline/mod.rs:2030:24: replace += with -= in Parser<'defs, 'a, 'ext>::insert_emphasis
    // After the strong pair takes two delimiters off each run, the
    // closer's leftover keeps its tail: its start moves two bytes
    // forward, so the emph pair around the leftovers spans the whole
    // source run pair, 0..7. With `-=` the leftover's start moves back
    // and the emph span collapses to 0..3.
    let src = "***a***\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    let children = doc.arena.children(para);
    assert_eq!(children.len(), 1);
    assert!(matches!(doc.arena.kind(children[0]), Node::Emphasis));
    let (_, emph_span) = span_of(src, &doc, children[0]);
    assert_eq!(emph_span, (0, 7));
    let inner = doc.arena.children(children[0]);
    assert_eq!(inner.len(), 1);
    assert!(matches!(doc.arena.kind(inner[0]), Node::Strong));
    let (_, strong_span) = span_of(src, &doc, inner[0]);
    assert_eq!(strong_span, (1, 6));
    let text = doc.arena.children(inner[0]);
    assert_eq!(text.len(), 1);
    let (slice, text_span) = span_of(src, &doc, text[0]);
    assert_eq!(slice, "a");
    assert_eq!(text_span, (3, 4));
    assert_eq!(html_of(src), "<p><em><strong>a</strong></em></p>\n");
}
// ==== content_reports_the_groups_text_or_none ====
/// Claims every `[`…`]` group whose content is exactly "foo" as a
/// marker node of kind `foo`, discarding the content.
struct ContentClaimer;

impl Extension for ContentClaimer {
    fn name(&self) -> &str {
        "content-claimer"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "foo".to_owned(),
            container: true,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        if ctx.content() == Some("foo") {
            ctx.claim("foo".to_owned(), Vec::new());
        }
    }
}

#[test]
fn content_reports_the_groups_text_or_none() {
    // kills: crates/geist4-core/src/inline/mod.rs:551:9: replace BracketInner<'p, 'defs, 'a, 'ext>::build_content -> Option<String> with None
    // kills: crates/geist4-core/src/inline/mod.rs:551:9: replace BracketInner<'p, 'defs, 'a, 'ext>::build_content -> Option<String> with Some(String::new())
    // kills: crates/geist4-core/src/inline/mod.rs:551:9: replace BracketInner<'p, 'defs, 'a, 'ext>::build_content -> Option<String> with Some("xyzzy".into())
    // kills: crates/geist4-core/src/inline/mod.rs:559:17: delete match arm crate::inline::Kind::Text(t) | crate::inline::Kind::Html(t) in BracketInner<'p, 'defs, 'a, 'ext>::build_content
    // kills: crates/geist4-core/src/inline/mod.rs:590:9: replace crate::extension::InlineBracketCtx<'p, 'defs, 'a, 'ext>::content -> Option<&str> with None
    // kills: crates/geist4-core/src/inline/mod.rs:590:9: replace crate::extension::InlineBracketCtx<'p, 'defs, 'a, 'ext>::content -> Option<&str> with Some("")
    // kills: crates/geist4-core/src/inline/mod.rs:590:9: replace crate::extension::InlineBracketCtx<'p, 'defs, 'a, 'ext>::content -> Option<&str> with Some("xyzzy")
    // The content hook hands the hook the bracket group's text: `foo`
    // for `[foo]`, and `None` when a non-text node sits inside the
    // brackets. The extension claims only when the content is exactly
    // `foo`, so a build that never assembles the text — a constant
    // return, a dropped Text/Html match arm — or a content() pinned to
    // a constant leaves `[foo]` literal, which the pinned output
    // rejects. The code-span case pins the None half of the contract.
    let mut options = Options::new();
    options.extension(Arc::new(ContentClaimer));
    assert_eq!(html_of_with("[foo]\n", &options), "<p></p>\n");
    let doc = parse_with("[foo]\n", &options);
    let para = paragraph(&doc);
    let children = doc.arena.children(para);
    assert_eq!(children.len(), 1, "the claimed marker replaces the group");
    assert!(
        matches!(doc.arena.kind(children[0]), Node::Extension(ext) if ext.kind == "foo"),
        "the claimed marker node holds the group"
    );
    // A code span inside the brackets is a non-text node: the content
    // is None and the group stays literal.
    assert_eq!(
        html_of_with("[`b`]\n", &options),
        "<p>[<code>b</code>]</p>\n"
    );
}

// ==== the_bracket_hook_sees_the_openers_position ====
/// Claims the bracket group only when the hook reports the opener
/// three bytes into the subject (a `[` sitting at byte 2).
struct PositionClaimer;

impl Extension for PositionClaimer {
    fn name(&self) -> &str {
        "position-claimer"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "placed".to_owned(),
            container: true,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        if ctx.opener_position() == 3 {
            ctx.claim("placed".to_owned(), Vec::new());
        }
    }
}

#[test]
fn the_bracket_hook_sees_the_openers_position() {
    // kills: crates/geist4-core/src/inline/mod.rs:597:9: replace crate::extension::InlineBracketCtx<'p, 'defs, 'a, 'ext>::opener_position -> usize with 0
    // kills: crates/geist4-core/src/inline/mod.rs:597:9: replace crate::extension::InlineBracketCtx<'p, 'defs, 'a, 'ext>::opener_position -> usize with 1
    // The bracket stack records the position just past the `[` — the
    // link span's `opener_position - 1` start depends on it — so for
    // `x [foo]`, with the `[` at byte 2, the hook sees 3. A constant 0
    // or 1 misreports it, the gate stays shut, and the group stays
    // literal text, which the pinned HTML rejects.
    let mut options = Options::new();
    options.extension(Arc::new(PositionClaimer));
    assert_eq!(html_of_with("x [foo]\n", &options), "<p>x </p>\n");
}

// ==== the_trigger_hook_sees_its_own_byte ====
/// Links its trigger byte, colored by which byte it was: `~` links to
/// `/t`, `^` to `/c`.
struct BytePairLinker;

impl Extension for BytePairLinker {
    fn name(&self) -> &str {
        "byte-pair-linker"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"~^"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        match ctx.byte() {
            b'~' => ctx.emit_link(0, 1, "/t".to_owned()),
            b'^' => ctx.emit_link(0, 1, "/c".to_owned()),
            _ => {}
        }
    }
}

#[test]
fn the_trigger_hook_sees_its_own_byte() {
    // kills: crates/geist4-core/src/inline/mod.rs:643:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::byte -> u8 with 0
    // byte() is the trigger byte the hook was called for. One hook
    // registered for both `~` and `^` must link each byte with its own
    // destination; with byte() pinned to 0 neither arm matches, the
    // hook consumes nothing, and both bytes stay literal text.
    let mut options = Options::new();
    options.extension(Arc::new(BytePairLinker));
    assert_eq!(
        html_of_with("x ~y ^z\n", &options),
        "<p>x <a href=\"/t\">~</a>y <a href=\"/c\">^</a>z</p>\n"
    );
}

// ==== the_hook_sees_its_own_trigger_byte ====
/// Branches on the trigger byte it was called for: `@` becomes an
/// `at` link, `#` a `hash` link. Any other byte the hook could see
/// (there is none in correct code) links nothing.
struct PairLinker;

impl Extension for PairLinker {
    fn name(&self) -> &str {
        "pair-linker"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"@#"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        match ctx.byte() {
            b'@' => ctx.emit_link(0, 1, "/at".to_owned()),
            b'#' => ctx.emit_link(0, 1, "/hash".to_owned()),
            _ => {}
        }
    }
}

#[test]
fn the_hook_sees_its_own_trigger_byte() {
    // kills: crates/geist4-core/src/inline/mod.rs:643:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::byte -> u8 with 1
    // The byte the hook was called for is the trigger byte itself. With
    // one hook registered for both `@` and `#`, `@` must link as `at`
    // and `#` as `hash`; a byte() pinned to 1 matches neither arm, the
    // hook takes nothing, and both bytes stay literal text.
    let mut options = Options::new();
    options.extension(Arc::new(PairLinker));
    assert_eq!(
        html_of_with("x@y#z\n", &options),
        "<p>x<a href=\"/at\">@</a>y<a href=\"/hash\">#</a>z</p>\n"
    );
}

// ==== the_hook_sees_the_trigger_bytes_position ====
/// Emits a link whose destination records the byte position the hook
/// was called at.
struct PositionLinker;

impl Extension for PositionLinker {
    fn name(&self) -> &str {
        "position-linker"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"@"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        ctx.emit_link(0, 1, format!("/p{}", ctx.position()));
    }
}

#[test]
fn the_hook_sees_the_trigger_bytes_position() {
    // kills: crates/geist4-core/src/inline/mod.rs:649:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::position -> usize with 0
    // kills: crates/geist4-core/src/inline/mod.rs:649:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::position -> usize with 1
    // The cursor's position is the byte offset of the trigger byte in
    // the inline subject: for `ab@c` that is 2, which the hook records
    // in its link destination. A constant 0 or 1 misstates it and the
    // destination shows the wrong offset.
    let mut options = Options::new();
    options.extension(Arc::new(PositionLinker));
    assert_eq!(
        html_of_with("ab@c\n", &options),
        "<p>ab<a href=\"/p2\">@</a>c</p>\n"
    );
}

// ==== a_scanned_runs_text_node_spans_the_run ====
/// Scans a `~` run and leaves it pending: the run stays literal text
/// whose text node carries the run's byte range.
struct TildeScan;

impl Extension for TildeScan {
    fn name(&self) -> &str {
        "tilde-scan"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"~"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let _ = ctx.scan_run();
    }
}

#[test]
fn a_scanned_runs_text_node_spans_the_run() {
    // kills: crates/geist4-core/src/inline/mod.rs:664:35: replace - with + in crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::scan_run
    // kills: crates/geist4-core/src/inline/mod.rs:664:35: replace - with / in crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::scan_run
    // The text node a scan_run pushes covers exactly the run's bytes:
    // for the two-tilde run of `abc~~c` that is 3..5. With `+` the
    // start lands past the run (an inverted range that maps to an
    // empty span), with `/` it starts two bytes early and the node
    // covers `c~~`.
    let src = "abc~~c\n";
    let mut options = Options::new();
    options.extension(Arc::new(TildeScan));
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text.as_ref() == "~~" {
                let (slice, span) = span_of(src, &doc, child);
                assert_eq!(slice, "~~");
                assert_eq!(span, (3, 5));
                return;
            }
        }
    }
    panic!("no run text node under the paragraph");
}

// ==== rest_of_line_runs_to_the_line_end ====
/// An autolink-ish extension: at `u` it eats the rest of the line as a
/// URL and emits a link whose destination records what rest_of_line
/// handed it, framed with `v`s so an empty slice shows up.
struct LineSitter;

impl Extension for LineSitter {
    fn name(&self) -> &str {
        "line-sitter"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"u"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let rest = ctx.rest_of_line();
        let seen = String::from_utf8_lossy(rest).into_owned();
        ctx.emit_link(0, rest.len(), format!("v{seen}v"));
    }
}

#[test]
fn rest_of_line_runs_to_the_line_end() {
    // kills: crates/geist4-core/src/inline/mod.rs:709:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::rest_of_line -> &'a[u8] with Vec::leak(Vec::new())
    // kills: crates/geist4-core/src/inline/mod.rs:709:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::rest_of_line -> &'a[u8] with Vec::leak(vec![0])
    // kills: crates/geist4-core/src/inline/mod.rs:709:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::rest_of_line -> &'a[u8] with Vec::leak(vec![1])
    // kills: crates/geist4-core/src/inline/mod.rs:761:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::rest_of_line -> &'a[u8] with Vec::leak(Vec::new())
    // kills: crates/geist4-core/src/inline/mod.rs:761:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::rest_of_line -> &'a[u8] with Vec::leak(vec![0])
    // kills: crates/geist4-core/src/inline/mod.rs:761:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::rest_of_line -> &'a[u8] with Vec::leak(vec![1])
    // kills: crates/geist4-core/src/inline/mod.rs:766:49: replace + with - in TriggerInner<'p, 'defs, 'a, 'ext>::rest_of_line
    // kills: crates/geist4-core/src/inline/mod.rs:766:49: replace + with * in TriggerInner<'p, 'defs, 'a, 'ext>::rest_of_line
    // rest_of_line spans the trigger byte through the bytes before the
    // newline: for `see u://x` at the `u` that is `u://x`, five bytes,
    // which the link swallows and records in its destination. An empty
    // slice, a one-byte NUL or SOH slice, or an end computed as
    // `start - offset` (which underflows) or `start * offset` (which
    // runs past the buffer) cannot reproduce it.
    let mut options = Options::new();
    options.extension(Arc::new(LineSitter));
    assert_eq!(
        html_of_with("see u://x\nafter\n", &options),
        "<p>see <a href=\"vu://xv\">u://x</a>\nafter</p>\n"
    );
}

// ==== preceding_reports_the_byte_before_the_cursor ====
/// Emits a link at `@` whose destination records the byte before the
/// cursor, with `?` standing in for `None`.
struct PrecedingLinker;

impl Extension for PrecedingLinker {
    fn name(&self) -> &str {
        "preceding-linker"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"@"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let seen = match ctx.preceding() {
            Some(byte) => byte as char,
            None => '?',
        };
        ctx.emit_link(0, 1, format!("v{seen}v"));
    }
}

#[test]
fn preceding_reports_the_byte_before_the_cursor() {
    // kills: crates/geist4-core/src/inline/mod.rs:724:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::preceding -> Option<u8> with None
    // kills: crates/geist4-core/src/inline/mod.rs:724:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::preceding -> Option<u8> with Some(0)
    // kills: crates/geist4-core/src/inline/mod.rs:724:9: replace crate::extension::InlineTriggerCtx<'p, 'defs, 'a, 'ext>::preceding -> Option<u8> with Some(1)
    // kills: crates/geist4-core/src/inline/mod.rs:772:9: replace TriggerInner<'p, 'defs, 'a, 'ext>::preceding -> Option<u8> with None
    // The byte before the cursor is `a` for `a@b`, which the
    // destination records. `None`, NUL, or SOH record something else.
    let mut options = Options::new();
    options.extension(Arc::new(PrecedingLinker));
    assert_eq!(
        html_of_with("a@b\n", &options),
        "<p>a<a href=\"vav\">@</a>b</p>\n"
    );
}

// ==== a_claim_takes_the_groups_delimiters_off_the_stack ====
#[test]
fn a_claim_takes_the_groups_delimiters_off_the_stack() {
    // kills: crates/geist4-core/src/inline/mod.rs:1051:9: replace Parser<'defs, 'a, 'ext>::remove_delimiters_since with ()
    // kills: crates/geist4-core/src/inline/mod.rs:1057:30: replace <= with > in Parser<'defs, 'a, 'ext>::remove_delimiters_since
    // The `~` run inside the claimed group ends after the opener's
    // position, so the claim must drop it; the run before the group ends
    // before the opener's position and must stay. With the removal dead
    // — or the boundary flipped, which stops at the first inside run and
    // never removes anything — the final `~` pairs with the inside run
    // instead of the leading one and the strike wraps the wrong bytes.
    struct ClaimStrike;
    impl Extension for ClaimStrike {
        fn name(&self) -> &str {
            "claim-strike"
        }

        fn kinds(&self) -> Vec<KindDecl> {
            vec![
                KindDecl {
                    kind: "s".to_owned(),
                    container: true,
                    inline_text: true,
                    attrs: Vec::new(),
                },
                KindDecl {
                    kind: "claimed".to_owned(),
                    container: true,
                    inline_text: true,
                    attrs: Vec::new(),
                },
            ]
        }

        fn inline_triggers(&self) -> &[u8] {
            b"~"
        }

        fn inline_trigger(
            &self,
            ctx: &mut geist4_core::extension::InlineTriggerCtx<'_, '_, '_, '_>,
        ) {
            let run = ctx.scan_run();
            ctx.push_delim(&run, "s");
        }

        fn inline_bracket(
            &self,
            ctx: &mut geist4_core::extension::InlineBracketCtx<'_, '_, '_, '_>,
        ) {
            ctx.claim_with_content("claimed".to_owned(), Vec::new(), 64);
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(ClaimStrike));
    let src = "~x[~a]b~\n";
    let doc = parse_with(src, &options);
    let para = paragraph(&doc);
    let kids = doc.arena.children(para);
    assert_eq!(kids.len(), 1, "the strike takes the whole line");
    let kind = match doc.arena.kind(kids[0]) {
        Node::Extension(ext) => ext.kind.clone(),
        other => panic!("expected the s node, got {other:?}"),
    };
    assert_eq!(kind, "s");
    let s_kids = doc.arena.children(kids[0]);
    assert_eq!(s_kids.len(), 3, "x, the claimed node, b");
    let (head, _) = span_of(src, &doc, s_kids[0]);
    assert_eq!(head, "x");
    let claimed_kind = match doc.arena.kind(s_kids[1]) {
        Node::Extension(ext) => ext.kind.clone(),
        other => panic!("expected the claimed node, got {other:?}"),
    };
    assert_eq!(claimed_kind, "claimed");
    let claimed_texts: Vec<String> = doc
        .arena
        .children(s_kids[1])
        .iter()
        .map(|&c| match doc.arena.kind(c) {
            Node::Text(t) => t.to_string(),
            other => panic!("unexpected claimed child {other:?}"),
        })
        .collect();
    assert_eq!(claimed_texts, vec!["~".to_owned(), "a".to_owned()]);
    let (tail, _) = span_of(src, &doc, s_kids[2]);
    assert_eq!(tail, "b");
}
