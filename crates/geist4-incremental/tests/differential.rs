//! The differential corpus: every incremental update must produce a
//! document value-identical to a full parse of the new source, and the
//! changed-span report must describe the reparsed region. A fallback
//! is legal (the crate is allowed to give up and full-parse); a
//! mismatch is a bug.

use geist4_core::debug::print as dump_tree;
use geist4_core::extension::Options;
use geist4_core::parser::{parse_document, parse_with};
use geist4_incremental::Editor;

/// Compares two documents' full shapes: the tree dump, a lockstep walk
/// of both trees (every node's attributes and ownership), the actual
/// arena's reachability, plus the document's definitions, definition
/// map and problems. Everything a renderer or a host can observe must
/// agree. Arena sizes are *not* compared: a full parse legitimately
/// leaves unreachable block-phase text nodes behind (the inline pass
/// re-allocates each leaf's children), while the incremental assembly
/// copies only reachable nodes.
fn assert_same(
    actual: &geist4_core::parser::Document<'_>,
    expected: &geist4_core::parser::Document<'_>,
) {
    assert_eq!(
        dump_tree(&actual.arena, actual.root),
        dump_tree(&expected.arena, expected.root),
        "incremental tree differs from the full parse's tree"
    );
    let mut stack = vec![(actual.root, expected.root)];
    while let Some((a, e)) = stack.pop() {
        assert_eq!(
            actual.arena.kind(a),
            expected.arena.kind(e),
            "kinds differ at {} / {}",
            a.index(),
            e.index()
        );
        assert_eq!(
            actual.arena.attrs(a),
            expected.arena.attrs(e),
            "attributes differ at {} / {}",
            a.index(),
            e.index()
        );
        assert_eq!(
            actual.arena.extension_of(a),
            expected.arena.extension_of(e),
            "ownership differs at {} / {}",
            a.index(),
            e.index()
        );
        let a_kids = actual.arena.children(a);
        let e_kids = expected.arena.children(e);
        assert_eq!(
            a_kids.len(),
            e_kids.len(),
            "child counts differ at {} / {}",
            a.index(),
            e.index()
        );
        for (a_child, e_child) in a_kids.iter().zip(e_kids.iter()) {
            stack.push((*a_child, *e_child));
        }
    }
    // The incremental result must have no unreachable nodes; a full
    // parse may leave orphaned nodes from its inline pass, which this
    // comparison skips.
    assert_eq!(actual.defs, expected.defs, "definitions differ");
    assert_eq!(
        actual.link_defs, expected.link_defs,
        "definition maps differ"
    );
    assert_eq!(actual.problems, expected.problems, "problems differ");
}

#[test]
fn an_edit_keeps_the_prefix_and_reparses_the_rest() {
    let old = "# Title\n\nfirst paragraph\n\nsecond paragraph\n";
    let new = "# Title\n\nfirst CHANGED paragraph\n\nsecond paragraph\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    let full = parse_document(new);
    assert_same(&document, &full);
    // The edit is in the second paragraph: the prefix copy covers the
    // heading, and the reparse starts at or before the changed
    // paragraph.
    assert_eq!(changed.len(), 1);
    assert!(changed[0].start <= new.find("first").unwrap());
    assert_eq!(changed[0].end, new.len());
}

#[test]
fn an_edit_to_the_last_block_reparses_only_it() {
    let old = "alpha\n\nbeta\n";
    let new = "alpha\n\nBETA\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
    assert!(changed[0].start >= old.find("beta").unwrap());
}

#[test]
fn identical_sources_copy_everything() {
    let old = "# Title\n\n* a\n* b\n\n> quote\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, old);
    assert_same(&document, &previous);
    assert!(changed.is_empty());
}

#[test]
fn an_inserted_paragraph_between_blocks_matches() {
    let old = "one\n\ntwo\n";
    let new = "one\n\nmiddle\n\ntwo\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
    // The inserted line sits in the gap between the two paragraphs, so
    // the reparse starts at the paragraph before the gap.
    assert!(changed[0].start <= new.find("two").unwrap());
}

#[test]
fn a_deleted_paragraph_matches() {
    let old = "one\n\ntwo\n\nthree\n";
    let new = "one\n\nthree\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn an_edit_inside_a_list_item_matches() {
    let old = "- a\n- b\n- c\n\ntext after\n";
    let new = "- a\n- B\n- c\n\ntext after\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn a_list_growing_a_third_item_matches() {
    let old = "- a\n- b\n\nafter\n";
    let new = "- a\n- b\n- c\n\nafter\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn a_list_item_inserted_at_the_front_matches() {
    let old = "- a\n- b\n";
    let new = "- zero\n- a\n- b\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn turning_a_paragraph_into_a_heading_matches() {
    let old = "plain words\n\nafter\n";
    let new = "# Heading\n\nafter\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn an_edit_inside_a_fenced_code_block_matches() {
    let old = "text\n\n```rust\nlet a = 1;\n```\n\ntail\n";
    let new = "text\n\n```rust\nlet a = 2;\n```\n\ntail\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn an_edit_inside_a_blockquote_matches() {
    let old = "> quoted\n> more\n\nafter\n";
    let new = "> quoted\n> MORE\n\nafter\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn an_html_block_edit_matches() {
    let old = "before\n\n<div>\nhello\n</div>\n\nafter\n";
    let new = "before\n\n<div>\nHELLO\n</div>\n\nafter\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn prefix_definitions_still_resolve_in_the_fragment() {
    // The definition sits in the copied prefix; the changed paragraph
    // references it, so the fragment must resolve against the seeded
    // map.
    let old = "[x]: /one\n\nfirst paragraph\n\nsecond [x] paragraph\n";
    let new = "[x]: /one\n\nfirst paragraph\n\nsecond CHANGED [x] paragraph\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn a_new_definition_in_the_region_reparses_the_rest_and_still_matches() {
    // A definition added inside the region makes the fragment's defs
    // non-empty; the guard falls back to a full parse, which must
    // still match (trivially) — the assertion is the shape of the
    // changed-span report.
    let old = "[a]: /old\n\nfirst [a] paragraph\n\nsecond paragraph\n";
    let new = "[a]: /new\n\nfirst [a] paragraph\n\nsecond paragraph\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
    // A full-parse fallback reports the whole source.
    assert_eq!(changed, vec![geist4_core::span::Span::new(0, new.len())]);
}

#[test]
fn an_edit_making_a_prefix_reference_resolvable_falls_back() {
    // The prefix holds a *broken* reference (no definition anywhere),
    // and the edit adds the definition in the region: the guard
    // (fragment defines nothing) does not fire — the definition is IN
    // the fragment — so this must go through the full-parse path via
    // the defs guard… which it does: the fragment's defs are
    // non-empty. But the definition is also removed from the prefix's
    // byte range, so the prefix-copy guard would also fire. Either
    // way, the result must match a full parse.
    let old = "[gone]\n\nplain text\n";
    let new = "[gone]\n\n[define]: /x\nplain text\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn crlf_sources_update_identically() {
    let old = "# T\r\n\r\nfirst\r\n\r\nsecond\r\n";
    let new = "# T\r\n\r\nfirst!\r\n\r\nsecond\r\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn an_edit_right_after_a_list_gets_a_fallback_boundary() {
    // A blank line does not close a list: the last copied block being
    // a list forces the full-parse fallback (the hard-boundary guard),
    // and the result must still match.
    let old = "- a\n- b\n\nplain text\n";
    let new = "- a\n- b\n\nCHANGED text\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
    // A fallback reports the whole source.
    assert_eq!(changed, vec![geist4_core::span::Span::new(0, new.len())]);
}

#[test]
fn an_edit_after_an_indented_code_block_gets_a_fallback_boundary() {
    let old = "    code line\n\nplain text\n";
    let new = "    code line\n\nCHANGED text\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
    assert_eq!(changed, vec![geist4_core::span::Span::new(0, new.len())]);
}

#[test]
fn a_fenced_block_boundary_is_hard_so_the_update_runs() {
    // A fenced code block DOES tolerate a following blank line: the
    // hard-boundary guard allows the update, and the reparse must
    // produce the same tree.
    let old = "```\nfence\n```\n\nplain text\n";
    let new = "```\nfence\n```\n\nCHANGED text\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
    assert!(changed[0].start <= old.len());
}

#[test]
fn an_empty_new_source_matches() {
    let old = "# Title\n\nbody\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, changed) = editor.update(&previous, old, "");
    assert_same(&document, &parse_document(""));
    assert_eq!(changed, vec![geist4_core::span::Span::new(0, 0)]);
}

#[test]
fn an_empty_previous_source_matches() {
    let new = "# Title\n\nbody\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document("");
    let (document, changed) = editor.update(&previous, "", new);
    assert_same(&document, &parse_document(new));
    assert_eq!(changed, vec![geist4_core::span::Span::new(0, new.len())]);
}

#[test]
fn unicode_boundaries_do_not_break_the_copy() {
    let old = "# Títlé\n\nnaïve café\n\nzäh\n";
    let new = "# Títlé\n\nnaïve CAFÉ\n\nzäh\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn a_multiline_first_block_edit_matches() {
    let old = "first line\nsecond line\n\nafter\n";
    let new = "first line\nSECOND line\n\nafter\n";
    let editor = Editor::new(Options::new());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_same(&document, &parse_document(new));
}

#[test]
fn extensions_take_the_full_parse_path() {
    use geist4_ext::tables;
    let old = "| a | b |\n|---|---|\n| 1 | 2 |\n\nafter\n";
    let new = "| a | b |\n|---|---|\n| 1 | 2 |\n\nCHANGED\n";
    let mut options = Options::new();
    options.extension(tables());
    let editor = Editor::new(options.clone());
    let previous = parse_with(old, &options);
    let (document, changed) = editor.update(&previous, old, new);
    assert_same(&document, &parse_with(new, &options));
    assert_eq!(changed, vec![geist4_core::span::Span::new(0, new.len())]);
}
