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
    // The claimed node spans exactly the bytes its hook kept: for
    // `[abcdef]` with end 2 that is bytes 1..3, ending on the
    // boundary between `b` and `c` in the source. An arithmetic slip
    // in the node's end formula pushes the span past the boundary,
    // into the bytes the hook refused.
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
    assert_eq!(ext_span, (1, 3));
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
    assert_eq!(ext_span, (1, 3));
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
    assert_eq!(ext_span, (1, 6));
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
    let (_, ext_span) = span_of(src, &doc, ext);
    assert_eq!(ext_span, (1, 3));
    let (rest, rest_span) = span_of(src, &doc, children[1]);
    assert_eq!(rest, "éé");
    assert_eq!(rest_span, (3, 7));
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
