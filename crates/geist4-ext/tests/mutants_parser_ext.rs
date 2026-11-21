//! Kill tests for `crates/geist4-core/src/parser/mod.rs` mutants whose
//! observable behavior only exists when a built-in extension is active
//! (tables, task lists, definition lists).
//!
//! Each test asserts the correct behavior first; the `kills:` comment names
//! the mutants that this test catches.

use geist4_core::arena::NodeId;
use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::node::Node;
use geist4_core::parser::parse_with;
use geist4_ext::{deflists, tables, tasklist};

/// All built-in extensions used by these tests.
fn options() -> Options {
    let mut options = Options::new();
    options.extension(tables());
    options.extension(tasklist());
    options.extension(deflists());
    options
}

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
        .find(|child| want(&doc.arena.kind(*child)))
}

/// Slices `src` at the span of `id`.
fn span_slice<'a>(src: &'a str, doc: &geist4_core::Document<'_>, id: NodeId) -> &'a str {
    let span = doc.arena.span(id);
    &src[span.start..span.end]
}

#[test]
fn escaped_asterisk_preheader_stays_outside_the_table() {
    // kills: crates/geist4-core/src/parser/mod.rs:234:30: replace && with || in unescape_pipes_trim
    let doc = parse_with("\\*\n| a |\n| - |\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<p>*</p>\n<table>\n<thead>\n<tr>\n<th>a</th>\n</tr>\n</thead>\n</table>\n",
        "only \\| unescapes to a literal pipe; \\* must keep its backslash in the pre-header paragraph"
    );
}

#[test]
fn table_frame_rows_build_a_full_table() {
    // kills: crates/geist4-core/src/parser/mod.rs:356:9 (frame_state -> None) and
    // :357:13 (delete OpenKind::Extension match arm): body lines after the
    // delimiter row must reach the table extension through its frame state.
    let doc = parse_with("x | y\n--- | ---\na | b\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<table>\n<thead>\n<tr>\n<th>x</th>\n<th>y</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>a</td>\n<td>b</td>\n</tr>\n</tbody>\n</table>\n",
        "every line after the delimiter row must become a table row"
    );
}

#[test]
fn tasklist_marker_becomes_a_checkbox() {
    // kills: crates/geist4-core/src/parser/mod.rs:377:9 (consume -> ()), :384:9
    // (raw_line -> leak of [] / [0] / [1]): the hook must see the real line
    // bytes and the marker must be consumed before the item text. The same
    // input also kills the same-shaped crates/geist4-core/src/extension.rs
    // mutants: 586:9 (consume -> ()), 596:11 (raw_line -> leak of [] / [0] /
    // [1]), and 603:11 (container_node -> None).
    let doc = parse_with("- [ ] foo\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<ul>\n<li><input type=\"checkbox\" disabled=\"\" /> foo</li>\n</ul>\n",
        "the [ ] marker must render as a checkbox input and be consumed"
    );
}

#[test]
fn second_definition_marker_starts_a_new_definition() {
    // kills: crates/geist4-core/src/parser/mod.rs:389:9 (container_node -> None) and
    // :953:9 (extension_block_start_looks -> false): a definition marker line
    // after an open definition must open a second <dd>, not continue lazily.
    let doc = parse_with("Term\n: def1\n: def2\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<dl>\n<dt>Term</dt>\n<dd>def1</dd>\n<dd>def2</dd>\n</dl>\n",
        "each definition marker opens its own <dd> in the same <dl>"
    );
}

#[test]
fn paragraph_before_delimiter_row_becomes_the_header() {
    // kills: crates/geist4-core/src/parser/mod.rs:403:13: delete OpenKind::Paragraph arm in container
    let doc = parse_with("| x |\n| - |\n| y |\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<table>\n<thead>\n<tr>\n<th>x</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>y</td>\n</tr>\n</tbody>\n</table>\n",
        "the open paragraph must convert to the table header when the delimiter row arrives"
    );
}

#[test]
fn table_body_rows_join_the_open_table() {
    // kills: crates/geist4-core/src/parser/mod.rs:404:13: delete OpenKind::Extension arm in container
    let doc = parse_with("| h |\n| - |\n| c |\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<table>\n<thead>\n<tr>\n<th>h</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>c</td>\n</tr>\n</tbody>\n</table>\n",
        "body rows after the delimiter row must join the open table frame"
    );
}

#[test]
fn a_declined_paragraph_is_not_reoffered() {
    // kills: crates/geist4-core/src/parser/mod.rs:421:9: replace visit_paragraph with ()
    let doc = parse_with("foo\n| h |\n| - | - |\n| b | c |\n| - | - |\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<p>foo\n| h |\n| - | - |\n| b | c |\n| - | - |</p>\n",
        "once a paragraph declined a hook it must not be offered to hooks again"
    );
}

#[test]
fn pre_paragraph_span_covers_the_text_before_the_table() {
    // kills: crates/geist4-core/src/parser/mod.rs:457:70: replace + with * in convert_paragraph
    let src = "foo\n| h |\n| - |\n";
    let doc = parse_with(src, &options());
    let para = child_of_kind(&doc, doc.root, |n| matches!(n, Node::Paragraph))
        .expect("pre-paragraph kept before the table");
    assert_eq!(
        span_slice(src, &doc, para),
        "foo\n",
        "the pre-paragraph span must cover the text before the header row"
    );
}

#[test]
fn quoted_definition_list_keeps_paragraphs_in_definitions() {
    // kills: crates/geist4-core/src/parser/mod.rs:967:36: replace - with + in extension_block_start_looks
    let doc = parse_with("> Term\n>\n> : def1\n> : def2\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<blockquote>\n<dl>\n<dt>Term</dt>\n<dd>\n<p>def1</p>\n</dd>\n<dd>\n<p>def2</p>\n</dd>\n</dl>\n</blockquote>\n",
        "definition markers at the quote's content column must open their own <dd>"
    );
}

#[test]
fn nested_list_inside_a_definition() {
    // kills: crates/geist4-core/src/parser/mod.rs:981:9: replace deepest_extension_contains_blocks -> bool with false
    let doc = parse_with("Term\n: def1\n  - x\n", &options());
    assert_eq!(
        render_with(&doc, &options()),
        "<dl>\n<dt>Term</dt>\n<dd>def1\n<ul>\n<li>x</li>\n</ul>\n</dd>\n</dl>\n",
        "a nested list at the definition's content indent opens inside the <dd>"
    );
}
