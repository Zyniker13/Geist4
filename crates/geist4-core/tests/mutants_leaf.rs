//! Regression tests for missed cargo-mutants survivors in the leaf-block
//! scanner (`src/parser/leaf.rs`).
//!
//! These are behavioral tests written against the public API only. Each test
//! carries `// kills:` comments naming the mutant IDs (line-based IDs as
//! reported by cargo-mutants) it is designed to catch, so a future mutant
//! run can trace every survivor to the test that should have killed it.

use geist4_core::basic_html::render;
use geist4_core::parser::parse_document;

fn html(src: &str) -> String {
    // One trailing newline is normalized away, as the spec harness
    // does: the renderer ends every document with a newline, as the
    // reference implementations emit, and the assertions here are
    // written without it.
    let html = render(&parse_document(src));
    html.strip_suffix('\n').unwrap_or(&html).to_owned()
}

/// Raw-HTML scanning happens in the block phase, so a raw tag that is the
/// whole line must not leak its internals through escaped rendering: when a
/// tag fails to scan as complete, the line is a plain paragraph and every
/// `<` and `>` is escaped.
#[test]
// kills: m0428 attribute_len(body->true) and m0429 attribute_len
// kills: m0429 first-char check `==` -> `!=`
fn html_block_requires_valid_first_attribute_char() {
    // `1` is not a valid attribute-name start, so this must stay a
    // paragraph. An attribute_len that accepts anything (body replaced
    // with `true`), or an inverted `_` arm in the first-character gate,
    // turns the line into a raw-HTML block and drops the escaping.
    assert_eq!(html("<span 1=b>"), "<p>&lt;span 1=b&gt;</p>");
}

#[test]
// kills: m0398 open_tag_len `/>` return pos + 2 -> pos * 2
fn self_closing_tag_keeps_trailing_text_in_paragraph() {
    // With the correct length the `/>` ends the tag and the remainder
    // (`x`) keeps the line an incomplete-open HTML block, which stays a
    // paragraph. `pos * 2` lands past the end of the line, the scan
    // thinks the tag ends the line, and the whole line becomes a raw
    // HTML block (no escaping, no paragraph wrapper).
    assert_eq!(html("<br/>x"), "<p><br/>x</p>");
}

#[test]
// kills: m0444 attribute_len `==` -> `!=` on the `=` check after the name
fn attribute_needing_value_at_tag_end_is_still_complete() {
    // A lone name with no `=` is a bare attribute; the `=` check must
    // fall through and accept the name. Inverted, the name is treated as
    // an attribute awaiting a value, the scan runs off the end, and the
    // complete tag is misread as an unclosed one (escaped paragraph).
    assert_eq!(html("<span a >"), "<span a >");
}

#[test]
// kills: m0452 attribute_len single-quoted-value loop `&&` -> `||`
fn single_quoted_attribute_value_closes_at_quote() {
    // The content loop must stop at `'` and require it. With `&&`
    // loosened to `||` the loop can only stop at end-of-line, the closing
    // quote check always fails, and the valid tag is misread as an
    // unclosed one (escaped paragraph instead of raw HTML).
    assert_eq!(html("<span a='b'>"), "<span a='b'>");
}

#[test]
// kills: m0509 skip_attribute whitespace-skip after the name `+=` -> `-=`
fn spaces_around_equals_in_second_attribute() {
    // `a1 =b`: after the name `a1`, skip_attribute must skip forward over
    // the space to reach `=`. Walking backwards instead drops the scan
    // onto `1`, the `=` is never seen, the second attribute is dropped,
    // and the tag fails to close (escaped paragraph).
    assert_eq!(html("<span a1 =b>"), "<span a1 =b>");
}

#[test]
// kills: m0514 skip_attribute whitespace-skip after `=` `+=` -> `-=`
fn spaces_around_equals_both_sides() {
    // `a = b`: after consuming `=`, the scan must skip forward over the
    // space to reach the value `b`. Walking backwards lands back on `=`,
    // the value is never read, and the tag stops closing (paragraph).
    assert_eq!(html("<span a = b>"), "<span a = b>");
}

#[test]
// kills: m0518a skip_attribute `p += 1` past the opening `'` -> `p *= 1`
// kills: m0518b skip_attribute `p += 1` past the opening `'` -> `p -= 1`
// kills: m0519a skip_attribute single-quote loop `!=` -> `==` (stop char)
// kills: m0519b skip_attribute single-quote loop `&&` -> `||`
// kills: m0519c skip_attribute single-quote loop `!=` -> `==` (NUL guard)
// kills: m0520 skip_attribute single-quote loop `+=` -> `-=`
// kills: m0522 skip_attribute `p += 1` past the closing `'` -> `p -= 1`
fn single_quoted_attribute_in_skip_position() {
    // Two-character value: every way of mishandling the quoted value in
    // skip_attribute (not entering the value, not scanning it, scanning
    // backwards, or not stepping past the closing quote) leaves the scan
    // short of the closing `>`, so the tag fails to close and the line
    // degrades to an escaped paragraph.
    assert_eq!(html("<span a='bc'>"), "<span a='bc'>");
}

#[test]
// kills: m0533 skip_attribute unquoted-value loop `!` deleted from `!matches!`
// kills: m0549 skip_attribute unquoted-value loop `+=` -> `-=`
fn unquoted_attribute_value_in_skip_position() {
    // The unquoted value loop must walk to the first terminator (here
    // `>`). With the guard inverted it stops immediately; walking
    // backwards it stops on `=`. Either way the scan is short of the
    // closing `>` and the tag degrades to an escaped paragraph.
    assert_eq!(html("<span a=b>"), "<span a=b>");
}

#[test]
// Pins that `-1.` is not a bullet list item (the digit guard). m0594
// (`pos + 1` -> `pos * 1` in that guard) is equivalent anyway: without
// the guard, marker_followed_by_space still rejects `-1.` because a digit
// is never reference whitespace.
fn dash_number_period_is_not_a_list_marker() {
    // `-1.` starts with a dash, so it is not an ordered-list marker and
    // the bullet arm's digit guard keeps it from being a bullet item
    // either. The line must remain a paragraph.
    assert_eq!(html("-1. not a list"), "<p>-1. not a list</p>");
}

#[test]
// kills: m0709b link-def label end `ls < le` -> `ls == le`
// kills: m0712a link-def label end `le > ls` -> `le < ls`
// kills: m0712b link-def label slice `le - 1` -> `le / 1`
// (m0709a `ls < le` -> `ls <= le` is equivalent: the extra iteration only
// happens at `ls == le`, where `bytes[le]` is always the closing `]`,
// never whitespace, so the walk never extends.)
fn link_label_is_trimmed_of_surrounding_whitespace() {
    // The label scan ends before the closing `]` and the slice must end
    // one past the last kept character so trailing whitespace is dropped,
    // while leading whitespace is skipped first. Every off-by-one here
    // shows up directly in the raw stored label.
    let doc = parse_document("[ a ]: /u\n\n[a](/u)");
    let def = &doc.defs[0].def;
    assert_eq!(def.label.as_ref(), "a");
    assert_eq!(def.destination.as_ref(), "/u");
}

#[test]
// kills: m0742 link-def title scan restart `j += 1` -> `j -= 1`
fn title_may_be_on_the_next_line() {
    // A title on the line after the destination must be found by skipping
    // forward over the new line's leading whitespace. If the scan walks
    // backwards instead, it lands back on the destination's line, the
    // title is lost, and the reference renders without its title
    // attribute. (Leading spaces on the title line are what the backward
    // walk stumbles over.)
    let doc = parse_document("[a]: /u\n   \"t\"\n\n[t][a]");
    let def = &doc.defs[0].def;
    assert_eq!(def.title.as_ref().map(|t| t.as_ref()), Some("t"));
    assert_eq!(
        html("[a]: /u\n   \"t\"\n\n[t][a]"),
        "<p><a href=\"/u\" title=\"t\">t</a></p>"
    );
}

#[test]
// kills: m0818 scan_destination early `<`/newline exit arm deleted
fn pointy_destination_may_span_lines_until_newline() {
    // A `<...>` destination ends at the first newline even when the
    // closing `>` never came. If that exit is deleted the scan eats the
    // newline and keeps going, so the definition is accepted across the
    // line break (a form the spec rejects) and the reference resolves
    // instead of staying literal.
    let doc = parse_document("[a]: <x\ny>\n\n[t][a]");
    assert!(
        doc.defs.is_empty(),
        "pointy destination must not swallow the newline"
    );
    // The rejected definition stays literal; the `<x\ny>` on the link line
    // is inline raw HTML (`x` + whitespace + bare attribute `y`), so it
    // renders unescaped inside the paragraph.
    assert_eq!(
        html("[a]: <x\ny>\n\n[t][a]"),
        "<p>[a]: <x\ny></p>\n<p>[t][a]</p>"
    );
}

#[test]
fn attribute_shapes_round_trip() {
    // Every attribute shape the scanners must accept, in one line each:
    // bare, unquoted, double-quoted, single-quoted, spaces around `=`,
    // whitespace-only runs, attributes at the tag end, self-closing, and
    // a closing tag. All are complete tags on one line, so all render as
    // raw-HTML blocks.
    assert_eq!(html("<span>"), "<span>");
    assert_eq!(html("<span >"), "<span >");
    assert_eq!(html("<span a>"), "<span a>");
    // Note: two bare attributes is not a raw-HTML block on the clean tree
    // (`<span a b>` renders as a paragraph containing inline raw HTML).
    // CommonMark's open-tag production accepts it as a type-7 block; this
    // is a known divergence, reported separately. The assertion pins the
    // current behavior.
    assert_eq!(html("<span a b>"), "<p><span a b></p>");
    assert_eq!(html("<span a=1>"), "<span a=1>");
    assert_eq!(html("<span a=\"1\">"), "<span a=\"1\">");
    assert_eq!(html("<span a='1'>"), "<span a='1'>");
    assert_eq!(html("<span a = 1>"), "<span a = 1>");
    assert_eq!(html("<span a=1 >"), "<span a=1 >");
    assert_eq!(html("<span  a   =   b  >"), "<span  a   =   b  >");
    assert_eq!(html("<span a=1 b=2>"), "<span a=1 b=2>");
    assert_eq!(html("<span a=1/>x"), "<p><span a=1/>x</p>");
    assert_eq!(html("</span>"), "</span>");
    // A value-less `=` is still a value (empty unquoted, accepted by
    // design), so the tag completes.
    assert_eq!(html("<span a=>"), "<span a=>");
}

#[test]
fn attribute_shapes_rejected_at_first_char() {
    // Shapes that must NOT open a raw-HTML block: a digit-leading name
    // and an unclosed quote keep the line a paragraph (escaped).
    assert_eq!(html("<span 1=b>"), "<p>&lt;span 1=b&gt;</p>");
    assert_eq!(html("<span a=\"b>"), "<p>&lt;span a=&quot;b&gt;</p>");
}

#[test]
// Convergence evidence for mutants that survive every test by design:
// m0435/m0442/m0445/m0447/m0453/m0460/m0462 (attribute_len's position is
// local, its boolean outcome is unchanged, and skip_attribute re-scans
// honestly, so these mutations converge at open_tag_len), m0488 (empty
// if-body, accepted empty unquoted value by design), m0500 (p = pos * 1
// re-walks the same name), m0577/m0651a/m0651b/m0654 (trim_ascii /
// contains_substring sites with whitespace-insensitive callers), m0594
// (bullet digit guard redundant with marker_followed_by_space), and
// m0709a (label-trim boundary byte is always `]`). These inputs pin the
// converged behavior so any future change that breaks the convergence
// shows up.
fn attribute_scan_convergence_shapes() {
    // Name with `.`/`_`/`-`/`:` continuations still complete; the
    // attribute_len name-loop mutations converge to the same completion.
    assert_eq!(html("<span a.b-c_d:e=1>"), "<span a.b-c_d:e=1>");
    // Whitespace before `=` plus an unclosed quote: the honest re-scan in
    // skip_attribute ends the same way the clean scan does.
    assert_eq!(html("<span a =\"b>"), "<p>&lt;span a =&quot;b&gt;</p>");
    // A value-less `=` is still a value (empty unquoted).
    assert_eq!(html("<span a=b c=>"), "<span a=b c=>");
}
