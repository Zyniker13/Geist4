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
