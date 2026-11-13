//! Unit tests for the block phase, separate from the spec examples.
//!
//! Each test covers one block kind or one tricky rule: lazy continuation,
//! list-item indentation, the end conditions of HTML blocks, tab handling,
//! and link reference definitions. Trees are checked through the debug
//! printer, rendering through the HTML renderer.

use geist4_core::basic_html::render;
use geist4_core::debug;
use geist4_core::parser::parse_document;

fn html(src: &str) -> String {
    render(&parse_document(src))
}

fn tree(src: &str) -> String {
    let doc = parse_document(src);
    debug::print(&doc.arena, doc.root)
}

#[test]
fn paragraph_with_soft_break() {
    assert_eq!(html("a\nb"), "<p>a\nb</p>");
}

#[test]
fn hard_break_needs_two_spaces() {
    assert_eq!(html("a  \nb"), "<p>a<br />\nb</p>");
    // One trailing space is a soft break.
    assert_eq!(html("a \nb"), "<p>a\nb</p>");
}

#[test]
fn atx_headings() {
    assert_eq!(html("# h1"), "<h1>h1</h1>");
    assert_eq!(html("## h2 ##"), "<h2>h2</h2>");
    assert_eq!(html("# five #"), "<h1>five</h1>");
    assert_eq!(html("####### seven"), "<p>####### seven</p>");
}

#[test]
fn setext_headings() {
    assert_eq!(html("Foo\n==="), "<h1>Foo</h1>");
    assert_eq!(html("Foo\n---"), "<h2>Foo</h2>");
    // An underline indented four or more spaces is not a setext underline.
    assert_eq!(html("Foo\n    ---"), "<p>Foo\n---</p>");
}

#[test]
fn reference_whitespace_and_control_bytes() {
    // The reference scanner's whitespace includes VT and FF: list markers,
    // HTML block starts and tag scanning accept them, and a line's
    // trailing run of them is trimmed before inline parsing.
    assert_eq!(html("+\u{b}"), "<ul>\n<li></li>\n</ul>");
    assert_eq!(html("*\u{c}"), "<ul>\n<li></li>\n</ul>");
    assert_eq!(html("a\u{b} \nb"), "<p>a\nb</p>");
    assert_eq!(html("0!\n<P\u{b}"), "<p>0!</p>\n<P\u{b}");
    assert_eq!(html("a\r\rb"), "<p>a</p>\n<p>b</p>");
    assert_eq!(html("```\t\u{c}\nx\n```"), "<pre><code>x\n</code></pre>");
    // Control bytes that are not reference whitespace may appear in a
    // bare link destination; they are percent-encoded on output.
    assert_eq!(html("[a]:\u{2}\n\n[a]"), "<p><a href=\"%02\">a</a></p>");
    // An indented code block's final whitespace-only line is dropped.
    assert_eq!(html("\tx\u{1f}\n\t\t"), "<pre><code>x\u{1f}\n</code></pre>");
    // A lazy continuation line keeps its leading whitespace, and a
    // consumed reference definition does not hide it.
    assert_eq!(
        html(">[-]:x\n    >"),
        "<blockquote>\n<p>    &gt;</p>\n</blockquote>"
    );
}

#[test]
fn nul_replacement_lines() {
    // Lines containing NUL bytes are rebuilt with U+FFFD replacement
    // characters, which makes them longer than their source slice; the
    // following lines must still render their own content.
    assert_eq!(html("a\u{0}b\nc\n"), "<p>a\u{fffd}b\nc</p>");
    assert_eq!(html("x\n\n\u{0}y\nz"), "<p>x</p>\n<p>\u{fffd}y\nz</p>");
    assert_eq!(html("a\0b\0\0c"), "<p>a\u{fffd}b\u{fffd}\u{fffd}c</p>");
}

#[test]
fn code_span_backtick_memo() {
    // The reference parser's backtick memo table: after an unmatched
    // longer run has scanned the subject, later runs of a length whose
    // last recorded position is at or before the current position are
    // treated as literal, even when a closer would exist.
    assert_eq!(html("``a`b`c`d`"), "<p>``a<code>b</code>c`d`</p>");
    assert_eq!(html("a`b`c`d`"), "<p>a<code>b</code>c<code>d</code></p>");
    assert_eq!(
        html("``\u{0}`F`\u{0}`F`"),
        "<p>``\u{fffd}<code>F</code>\u{fffd}`F`</p>"
    );
}

#[test]
fn thematic_breaks() {
    assert_eq!(html("***"), "<hr />");
    assert_eq!(html("___"), "<hr />");
    assert_eq!(html("  ---"), "<hr />");
    // Not a thematic break after a paragraph line.
    assert_eq!(html("Foo\n***"), "<p>Foo</p>\n<hr />");
}

#[test]
fn indented_code_blocks() {
    assert_eq!(html("    a\n    b"), "<pre><code>a\nb\n</code></pre>");
    // Interior blank lines keep their indentation only.
    assert_eq!(html("    a\n\n    b"), "<pre><code>a\n\nb\n</code></pre>");
    // A tab counts as four columns and is stripped whole.
    assert_eq!(html("\tfoo"), "<pre><code>foo\n</code></pre>");
    // Up to three spaces of indent do not make a code block.
    assert_eq!(html("   x"), "<p>x</p>");
}

#[test]
fn fenced_code_blocks() {
    assert_eq!(html("```\na\n```"), "<pre><code>a\n</code></pre>");
    assert_eq!(html("~~~\na\n~~~"), "<pre><code>a\n</code></pre>");
    // A closing fence may be longer, never shorter, and needs no match after
    // a shorter opener's run.
    assert_eq!(
        html("```\na\n`````\n```\nb"),
        "<pre><code>a\n</code></pre>\n<pre><code>b\n</code></pre>"
    );
    // The info string becomes a language class.
    assert_eq!(
        html("``` rust\nfn x() {}\n```"),
        "<pre><code class=\"language-rust\">fn x() {}\n</code></pre>"
    );
    // An unclosed fence runs to the end of the document.
    assert_eq!(html("```\na"), "<pre><code>a\n</code></pre>");
    // Entity references in the info string are decoded.
    assert_eq!(
        html("``` f&ouml;&ouml;\nfoo\n```"),
        "<pre><code class=\"language-föö\">foo\n</code></pre>"
    );
}

#[test]
fn html_block_kinds_one_through_five() {
    // Kind 1 ends at a line containing the closing tag.
    assert_eq!(
        html("<script>\nx\n</script>\ny"),
        "<script>\nx\n</script>\n<p>y</p>"
    );
    // Kind 2 ends at the line containing `-->`.
    assert_eq!(html("<!-- c -->\nz"), "<!-- c -->\n<p>z</p>");
    // Kind 3 is a processing instruction.
    assert_eq!(html("<?php echo 1; ?>\nz"), "<?php echo 1; ?>\n<p>z</p>");
    // Kind 4 is a declaration.
    assert_eq!(html("<!DOCTYPE html>\nz"), "<!DOCTYPE html>\n<p>z</p>");
    // Kind 5 is CDATA.
    assert_eq!(html("<![CDATA[x]]>\nz"), "<![CDATA[x]]>\n<p>z</p>");
}

#[test]
fn html_block_trailing_blank_lines_are_kept() {
    // A comment block (kind 2) runs to the end of input, blank lines
    // included; the blank line inside the block survives rendering (the
    // document-final newline is stripped by the spec output convention,
    // so the block's own blank line remains).
    assert_eq!(html("<!--a\nb\n\nc\n"), "<!--a\nb\n\nc");
    assert_eq!(html("<!--a\n\n"), "<!--a\n");
}

#[test]
fn raw_html_comments_allow_dash_runs() {
    // Comment content may contain dash runs as long as no `-->` appears
    // early, matching the reference scanner.
    assert_eq!(
        html("x <!-- a ------' b --> y"),
        "<p>x <!-- a ------' b --> y</p>"
    );
    // A comment ending in `--->` is not a comment.
    assert_eq!(html("x <!-- a ---> y"), "<p>x &lt;!-- a ---&gt; y</p>");
}

#[test]
fn html_block_kind_six_ends_at_blank_line() {
    assert_eq!(
        html("<table><tr><td>\n<pre>\nx\n</pre>\n</td></tr></table>\n\nafter"),
        "<table><tr><td>\n<pre>\nx\n</pre>\n</td></tr></table>\n<p>after</p>"
    );
}

#[test]
fn html_block_kind_seven_needs_a_lone_tag() {
    assert_eq!(
        html("<span data-x=\"1\">\ntext"),
        "<span data-x=\"1\">\ntext"
    );
    // A paragraph continuation or any following text keeps it a paragraph.
    assert_eq!(html("<span>x\ny"), "<p><span>x\ny</p>");
}

#[test]
fn link_reference_definitions() {
    // A definition may appear after the paragraph that uses it.
    assert_eq!(
        html("[foo]\n\n[foo]: /bar"),
        "<p><a href=\"/bar\">foo</a></p>"
    );
    // The first definition wins.
    assert_eq!(
        html("[foo]: /one\n[foo]: /two\n\n[foo]"),
        "<p><a href=\"/one\">foo</a></p>"
    );
    // A text line before a definition keeps the definition as text.
    assert_eq!(html("x\n[foo]: /bar"), "<p>x\n[foo]: /bar</p>");
    // The title may be on the next line.
    assert_eq!(
        html("[foo]\n\n[foo]: /bar\n\"t\""),
        "<p><a href=\"/bar\" title=\"t\">foo</a></p>"
    );
    // Labels are folded case-insensitively.
    assert_eq!(html("[ẞ]\n\n[SS]: /url"), "<p><a href=\"/url\">ẞ</a></p>");
    // Title content that itself starts and ends with the outer quote
    // character is kept: only the outer pair is stripped, and content
    // that is a single quote byte must not panic the range slice.
    assert_eq!(
        html("[a]: /u \"'x'\"\n\n[a]"),
        "<p><a href=\"/u\" title=\"'x'\">a</a></p>"
    );
    assert_eq!(
        html("[a]: /u \"'\"\n\n[a]"),
        "<p><a href=\"/u\" title=\"'\">a</a></p>"
    );
}

#[test]
fn block_quotes_nest_and_take_lazy_lines() {
    // A block quote interrupts the paragraph and nests inside the quote.
    assert_eq!(
        html("> a\n> > b"),
        "<blockquote>\n<p>a</p>\n<blockquote>\n<p>b</p>\n</blockquote>\n</blockquote>"
    );
    // A lazy continuation line joins the paragraph inside the quote.
    assert_eq!(html("> a\nb"), "<blockquote>\n<p>a\nb</p>\n</blockquote>");
    // A blank line ends a block quote.
    assert_eq!(
        html("> a\n\nb"),
        "<blockquote>\n<p>a</p>\n</blockquote>\n<p>b</p>"
    );
}

#[test]
fn list_items_and_indentation() {
    assert_eq!(html("- a\n- b"), "<ul>\n<li>a</li>\n<li>b</li>\n</ul>");
    // Content beyond the marker's column continues the item.
    assert_eq!(html("- a\n  b"), "<ul>\n<li>a\nb</li>\n</ul>");
    // A nested list needs at least the item's content column.
    assert_eq!(
        html("- a\n  - b"),
        "<ul>\n<li>a\n<ul>\n<li>b</li>\n</ul>\n</li>\n</ul>"
    );
    // A blank line between items makes the list loose.
    assert_eq!(
        html("- a\n\n- b"),
        "<ul>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n</ul>"
    );
    assert_eq!(html("1. a\n2. b"), "<ol>\n<li>a</li>\n<li>b</li>\n</ol>");
}

#[test]
fn tabs_are_expanded_for_structure() {
    // A tab after the marker spaces the content to the next stop.
    assert_eq!(html("-\tfoo"), "<ul>\n<li>foo</li>\n</ul>");
    // A tab-indented block quote line still matches.
    assert_eq!(html("\t>\tfoo"), "<pre><code>&gt;\tfoo\n</code></pre>");
}

#[test]
fn debug_printer_shows_the_tree() {
    let out = tree("a");
    assert!(
        out.contains("Document"),
        "printer should name the root: {out}"
    );
    assert!(
        out.contains("Paragraph"),
        "printer should name the paragraph: {out}"
    );
}

#[test]
fn spans_cover_the_source() {
    let doc = parse_document("# hi");
    let span = doc.arena.span(doc.root);
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 4);
}
