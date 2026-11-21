//! Regression tests for missed mutants in the inline scanner, from a
//! cargo-mutants run: the delimiter walk-through, bracket spans,
//! autolink children, and the link label scanner's 1000-byte cap and
//! whitespace trimming. Each test pins behavior at a boundary the
//! mutants moved (`>` vs `>=`, `+1` vs `+0`, trim direction), so a
//! mutated scanner produces visibly wrong output or never terminates.

use geist4_core::Document;
use geist4_core::arena::NodeId;
use geist4_core::basic_html::render_with;
use geist4_core::extension::{Extension, Options};
use geist4_core::node::Node;
use geist4_core::parser::parse_with;
use std::sync::Arc;

fn html_of(src: &str) -> String {
    render_with(&parse_with(src, &Options::new()), &Options::new())
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

/// A minimal extension that registers `~` as an inline trigger byte.
struct Tilde;

impl Extension for Tilde {
    fn name(&self) -> &str {
        "tilde"
    }

    fn inline_triggers(&self) -> &[u8] {
        b"~"
    }
}

#[test]
fn delimiter_measurement_walks_through_trigger_bytes() {
    // kills: crates/geist4-core/src/inline/mod.rs:1411:17: replace += with *= in Parser<'defs, 'a, 'ext>::char_at
    // Measuring the `*` run, `char_at` must step over the extension's
    // trigger byte and report `a` as the after-character. A walk whose
    // cursor never advances spins forever on the `~` it is stepping
    // over, so this test hangs instead of passing under the mutant.
    let mut options = Options::new();
    options.extension(Arc::new(Tilde));
    let html = render_with(&parse_with("*~a\n", &options), &options);
    assert_eq!(html, "<p>*~a</p>\n");
}

#[test]
fn unmatched_open_bracket_text_covers_one_byte() {
    // kills: crates/geist4-core/src/inline/mod.rs:1426:84: replace + with * in Parser<'defs, 'a, 'ext>::handle_open_bracket
    // The bracket's text node must cover exactly the `[` byte. A span
    // whose end equals its start hides the bracket's source position.
    let src = "[*a*\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text.starts_with('[') {
                let (slice, span) = span_of(src, &doc, child);
                assert_eq!(slice, "[");
                assert_eq!(span, (0, 1));
                return;
            }
        }
    }
    panic!("no bracket text node under the paragraph");
}

#[test]
fn autolink_text_child_covers_the_url_only() {
    // kills: crates/geist4-core/src/inline/mod.rs:2172:27: replace - with + in Parser<'defs, 'a, 'ext>::append_autolink
    // kills: crates/geist4-core/src/inline/mod.rs:2172:27: replace - with / in Parser<'defs, 'a, 'ext>::append_autolink
    // The child text inside `<...>` must span from the byte after `<`
    // to the byte before `>`: one short hides the closing bracket, one
    // long pokes past the source (or swallows the `>`).
    let src = "<https://example.com>\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if matches!(doc.arena.kind(child), Node::Link(_)) {
            let children = doc.arena.children(child);
            assert_eq!(children.len(), 1, "the autolink has one text child");
            let (slice, span) = span_of(src, &doc, children[0]);
            assert_eq!(slice, "https://example.com");
            assert_eq!(span, (1, 20));
            return;
        }
    }
    panic!("no link node under the paragraph");
}

#[test]
fn a_label_of_exactly_1000_bytes_is_a_reference() {
    // kills: crates/geist4-core/src/inline/mod.rs:2418:19: replace > with == in scan_link_label
    // kills: crates/geist4-core/src/inline/mod.rs:2418:19: replace > with >= in scan_link_label
    // The cap rejects labels over 1000 bytes, so exactly 1000 bytes is
    // valid: the full form links. At `==` or `>=` the scan rejects this
    // label, the def-side scan drops the definition, and no link forms.
    let label = "x".repeat(1000);
    let src = format!("[t][{label}]\n\n[{label}]: /u\n");
    assert_eq!(html_of(&src), "<p><a href=\"/u\">t</a></p>\n");
}

#[test]
fn an_escaped_label_over_the_cap_falls_back() {
    // kills: crates/geist4-core/src/inline/mod.rs:2409:20: replace += with *= in scan_link_label
    // kills: crates/geist4-core/src/inline/mod.rs:2412:24: replace += with *= in scan_link_label
    // kills: crates/geist4-core/src/inline/mod.rs:2412:24: replace += with -= in scan_link_label
    // Every byte of `\!`×501 (1002 bytes) is counted: the escape arm
    // and the punctuation arm each add one. With either arm's increment
    // dead the label scans as under the cap, the full form wins, and
    // the fallback shortcut that matches the definition never happens.
    let label = "\\!".repeat(501);
    let src = format!("[t][{label}]\n\n[{label}]: /u\n");
    assert_eq!(
        html_of(&src),
        format!("<p>[t]<a href=\"/u\">{}</a></p>\n", "!".repeat(501))
    );
}

#[test]
fn a_plain_label_over_the_cap_falls_back() {
    // kills: crates/geist4-core/src/inline/mod.rs:2416:20: replace += with *= in scan_link_label
    // A label of plain bytes over the cap counts on the default arm.
    // With that increment dead the scan never grows, the over-cap label
    // scans as valid, and the full form wins over the fallback.
    let label = "x".repeat(1001);
    let src = format!("[t][{label}]\n\n[{label}]: /u\n");
    assert_eq!(
        html_of(&src),
        format!("<p>[t]<a href=\"/u\">{label}</a></p>\n")
    );
}

#[test]
fn broken_reference_label_is_trimmed_on_both_sides() {
    // kills: crates/geist4-core/src/inline/mod.rs:2433:17: replace < with > in trim_ascii_spaces
    // kills: crates/geist4-core/src/inline/mod.rs:2436:15: replace > with < in trim_ascii_spaces
    // The reported label is trimmed on both sides before it lands in
    // the problem message. A reversed loop leaves a space on one side,
    // which the message then shows verbatim.
    let src = "[t][ x ]\n";
    let doc = parse_with(src, &Options::new());
    assert_eq!(doc.problems.len(), 1);
    assert_eq!(
        doc.problems[0].message,
        "no link reference definition for 'x'"
    );
    assert_eq!(html_of(src), "<p>[t][ x ]</p>\n");
}
// ==== hard_break_span_covers_dropped_trailing_spaces ====
#[test]
fn hard_break_span_covers_dropped_trailing_spaces() {
    // kills: crates/geist4-core/src/inline/mod.rs:1140:26: replace > with == in Parser<'defs, 'a, 'ext>::handle_newline
    // A hard break's cell must cover the trailing whitespace the
    // preceding text run dropped plus the line ending. With `==` the
    // backwalk never runs and the cell starts at the line ending.
    let src = "a  \nb\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if matches!(doc.arena.kind(child), Node::HardBreak) {
            let (slice, span) = span_of(src, &doc, child);
            assert_eq!(slice, "  \n");
            assert_eq!(span, (1, 4));
            return;
        }
    }
    panic!("no hard break under the paragraph");
}

// ==== hard_break_backwalk_stops_at_zero ====
#[test]
fn hard_break_backwalk_stops_at_zero() {
    // kills: crates/geist4-core/src/inline/mod.rs:1140:26: replace > with >= in Parser<'defs, 'a, 'ext>::handle_newline
    // The backwalk must stop at byte 0: `\x0B` counts as break-adjacent
    // whitespace here, so the cell legitimately starts at 0. With `>=`
    // the usize guard is always true and the walk indexes byte -1,
    // panicking instead of parsing.
    let src = "\u{0b}  \nb\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if matches!(doc.arena.kind(child), Node::HardBreak) {
            let (slice, span) = span_of(src, &doc, child);
            assert_eq!(slice, "\u{0b}  \n");
            assert_eq!(span, (0, 4));
            return;
        }
    }
    panic!("no hard break under the paragraph");
}

// ==== escaped_char_in_link_label_keeps_link_span ====
#[test]
fn numeric_entity_decodes_decimal_references() {
    // kills: crates/geist4-core/src/inline/entities.rs:44:17: replace < with == in scan_numeric_entity
    assert_eq!(html_of("&#65;\n"), "<p>A</p>\n");
}

#[test]
fn a_leading_tab_makes_an_indented_code_block() {
    // kills: crates/geist4-core/src/parser/cursor.rs:65:38: replace + with * in Cursor<'t>::bump
    // kills: crates/geist4-core/src/parser/cursor.rs:65:43: replace * with / in Cursor<'t>::bump
    assert_eq!(
        html_of("\tfoo\n"),
        "<pre><code>foo\n</code></pre>\n",
        "one tab expands to four columns, which is an indented code block"
    );
}

#[test]
fn list_item_tab_padding_continues_at_its_tab_stop() {
    // kills: crates/geist4-core/src/parser/cursor.rs:65:43: replace * with + in Cursor<'t>::bump
    // The item's content column is the tab stop after the marker; a
    // continuation line indented by one tab must reach it exactly.
    assert_eq!(
        html_of("-\tfoo\n\tbar\n"),
        "<ul>\n<li>foo\nbar</li>\n</ul>\n",
        "the tab-continuation must join the item, not fall out into an indented code block"
    );
}

#[test]
fn escaped_char_in_link_label_keeps_link_span() {
    // kills: crates/geist4-core/src/inline/mod.rs:1172:44: replace - with / in Parser<'defs, 'a, 'ext>::handle_backslash
    // The escaped-character wrapper's span must start at its backslash.
    // A wrapper inside a link label is a live child when the link cell's
    // span is folded from its children, so a wrapper start computed by
    // other means (here pos/2) drags the link's span start to that
    // value; a link opener is never later than its first child.
    let src = "abcdefghij[\\*x](u)\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Link(_) = doc.arena.kind(child) {
            let (slice, span) = span_of(src, &doc, child);
            assert_eq!(slice, "[\\*x](u)");
            assert_eq!(span, (10, 18));
            return;
        }
    }
    panic!("no link node under the paragraph");
}

// ==== escaped_char_at_line_start_covers_both_bytes ====
#[test]
fn escaped_char_at_line_start_covers_both_bytes() {
    // kills: crates/geist4-core/src/inline/mod.rs:1181:44: replace - with / in Parser<'defs, 'a, 'ext>::handle_backslash
    // The wrapper dissolves, so the surviving text node must carry the
    // whole escape's span: backslash plus character. A start computed
    // by other means (here pos/2) hides the backslash's position.
    let src = "\\*a\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text == "*" {
                let (slice, span) = span_of(src, &doc, child);
                assert_eq!(slice, "\\*");
                assert_eq!(span, (0, 2));
                return;
            }
        }
    }
    panic!("no escaped text node under the paragraph");
}

// ==== literal_backslash_span_covers_one_byte ====
#[test]
fn literal_backslash_span_covers_one_byte() {
    // kills: crates/geist4-core/src/inline/mod.rs:1195:74: replace - with / in Parser<'defs, 'a, 'ext>::handle_backslash
    // A backslash that escapes nothing is literal text covering exactly
    // its own byte. A start computed by other means (here pos/1, i.e.
    // the cursor) yields an empty span that hides the backslash.
    let src = "a\\ b\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text == "\\" {
                let (slice, span) = span_of(src, &doc, child);
                assert_eq!(slice, "\\");
                assert_eq!(span, (1, 2));
                return;
            }
        }
    }
    panic!("no backslash text node under the paragraph");
}

// ==== run_of_1001_backticks_is_literal_text ====
#[test]
fn run_of_1001_backticks_is_literal_text() {
    // kills: crates/geist4-core/src/inline/mod.rs:1243:21: replace > with == in Parser<'defs, 'a, 'ext>::handle_backticks
    // Runs longer than MAXBACKTICKS are literal text even when a closer
    // exists. With `==` a 1001-tick opener passes the guard (a closer
    // exists, so optimistic_fail is false) and pairs up as a code span.
    let ticks = "`".repeat(1001);
    let src = format!("{ticks}x{ticks}\n");
    assert_eq!(html_of(&src), format!("<p>{ticks}x{ticks}</p>\n"));
}

// ==== long_backtick_run_span_starts_at_run_start ====
#[test]
fn long_backtick_run_span_starts_at_run_start() {
    // kills: crates/geist4-core/src/inline/mod.rs:1244:35: replace - with / in Parser<'defs, 'a, 'ext>::handle_backticks
    // A literal over-cap run's text node must start at the run's first
    // backtick (pos - numticks), not at a value computed by other means
    // (here pos/numticks), which points into the prefix and mis-frames
    // the run.
    let ticks = "`".repeat(1001);
    let src = format!("abc{ticks}\n");
    let doc = parse_with(&src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if *text == ticks {
                let (slice, span) = span_of(&src, &doc, child);
                assert_eq!(slice, ticks);
                assert_eq!(span, (3, 1004));
                return;
            }
        }
    }
    panic!("no backtick run text node under the paragraph");
}

// ==== unmatched_backtick_run_span_covers_the_ticks ====
#[test]
fn unmatched_backtick_run_span_covers_the_ticks() {
    // kills: crates/geist4-core/src/inline/mod.rs:1263:30: replace - with / in Parser<'defs, 'a, 'ext>::handle_backticks
    // When the remembered opener run is emitted as literal text its node
    // must cover exactly the ticks (startpos - numticks). Dividing
    // instead of subtracting shifts the start into the following text.
    let src = "``x\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text == "``" {
                let (slice, span) = span_of(src, &doc, child);
                assert_eq!(slice, "``");
                assert_eq!(span, (0, 2));
                return;
            }
        }
    }
    panic!("no backtick run text node under the paragraph");
}

// ==== entity_span_covers_the_reference ====
#[test]
fn entity_span_covers_the_reference() {
    // kills: crates/geist4-core/src/inline/mod.rs:1304:28: replace - with / in Parser<'defs, 'a, 'ext>::handle_entity
    // The entity's text node must start at the `&` (amp = pos - 1 after
    // the cursor stepped past it). pos/1 is the cursor itself, so the
    // span would start after the ampersand and stop covering it.
    let src = "a&amp;b\n";
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text == "&" {
                let (slice, span) = span_of(src, &doc, child);
                assert_eq!(slice, "&amp;");
                assert_eq!(span, (1, 6));
                return;
            }
        }
    }
    panic!("no entity text node under the paragraph");
}

// ==== disabled_link_close_bracket_text_covers_one_byte ====
#[test]
fn disabled_link_close_bracket_text_covers_one_byte() {
    // kills: crates/geist4-core/src/inline/mod.rs:1442:72: replace - with + in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // kills: crates/geist4-core/src/inline/mod.rs:1442:72: replace - with / in Parser<'defs, 'a, 'ext>::handle_close_bracket
    // Inside a disabled link, the unmatched outer `]` is literal text
    // covering exactly its own byte. `+` and `/` both move the span off
    // the bracket, hiding where the disabled link ended.
    let src = "[a [b](c) d]\n";
    assert_eq!(html_of(src), "<p>[a <a href=\"c\">b</a> d]</p>\n");
    let doc = parse_with(src, &Options::new());
    let para = paragraph(&doc);
    let mut found: Option<NodeId> = None;
    for &child in doc.arena.children(para) {
        if let Node::Text(text) = doc.arena.kind(child) {
            if text == "]" {
                found = Some(child);
            }
        }
    }
    let id = found.expect("no stray close bracket text under the paragraph");
    let (slice, span) = span_of(src, &doc, id);
    assert_eq!(slice, "]");
    assert_eq!(span, (11, 12));
}
