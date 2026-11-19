//! Inline and leaf-block node spans point at the source text they were
//! parsed from.
//!
//! A node's span is checked by slicing the input at the span and
//! comparing against the text the node was parsed from, so an off-by-one
//! or a misplaced map cannot hide behind a plausible-looking offset.

use geist4_core::arena::NodeId;
use geist4_core::node::Node;
use geist4_core::{Document, parse_document};

/// The source text a node's span covers.
fn slice<'a>(src: &'a str, doc: &Document<'_>, id: NodeId) -> &'a str {
    let span = doc.arena.span(id);
    &src[span.start..span.end]
}

/// The first child of `id` matching a predicate.
fn find_child<'a>(
    src: &'a str,
    doc: &Document<'a>,
    id: NodeId,
    want: &dyn Fn(&Node<'_>) -> bool,
) -> (NodeId, &'a str) {
    for &child in doc.arena.children(id) {
        if want(doc.arena.kind(child)) {
            return (child, slice(src, doc, child));
        }
    }
    panic!("no matching child under {:?}", doc.arena.kind(id));
}

fn paragraph<'a>(doc: &Document<'a>) -> NodeId {
    let root = doc.root;
    for &child in doc.arena.children(root) {
        if matches!(doc.arena.kind(child), Node::Paragraph) {
            return child;
        }
    }
    panic!("no paragraph under the root");
}

fn is_text(text: &'static str) -> impl Fn(&Node<'_>) -> bool + 'static {
    move |node| matches!(node, Node::Text(t) if t.starts_with(text))
}

fn is_emphasis(node: &Node<'_>) -> bool {
    matches!(node, Node::Emphasis)
}

fn is_strong(node: &Node<'_>) -> bool {
    matches!(node, Node::Strong)
}

fn is_code(node: &Node<'_>) -> bool {
    matches!(node, Node::CodeSpan(_))
}

fn is_soft_break(node: &Node<'_>) -> bool {
    matches!(node, Node::SoftBreak)
}

fn is_hard_break(node: &Node<'_>) -> bool {
    matches!(node, Node::HardBreak)
}

fn is_inline_html(node: &Node<'_>) -> bool {
    matches!(node, Node::InlineHtml(_))
}

fn is_link(node: &Node<'_>) -> bool {
    matches!(node, Node::Link(_))
}

fn is_image(node: &Node<'_>) -> bool {
    matches!(node, Node::Link(data) if data.image)
}

#[test]
fn text_runs_cover_their_source() {
    let src = "hello world";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (id, text) = find_child(src, &doc, p, &is_text("hello"));
    assert_eq!(text, "hello world");
    let _ = id;
}

#[test]
fn emphasis_covers_its_delimiters() {
    let src = "a *em* b";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, before) = find_child(src, &doc, p, &is_text("a "));
    assert_eq!(before, "a ");
    let (em, covered) = find_child(src, &doc, p, &is_emphasis);
    assert_eq!(covered, "*em*");
    let (_, after) = find_child(src, &doc, p, &is_text(" b"));
    assert_eq!(after, " b");
    // The emphasis' own child is the inner text without the markers.
    let inner = doc.arena.children(em)[0];
    assert_eq!(slice(src, &doc, inner), "em");
}

#[test]
fn strong_covers_its_delimiters() {
    let src = "x **yo**";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (strong, covered) = find_child(src, &doc, p, &is_strong);
    assert_eq!(covered, "**yo**");
    let inner = doc.arena.children(strong)[0];
    assert_eq!(slice(src, &doc, inner), "yo");
}

#[test]
fn code_span_covers_its_backticks() {
    let src = "a `code` b";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, covered) = find_child(src, &doc, p, &is_code);
    assert_eq!(covered, "`code`");
}

#[test]
fn soft_break_covers_the_line_ending() {
    let src = "one\ntwo";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, brk) = find_child(src, &doc, p, &is_soft_break);
    assert_eq!(brk, "\n");
}

#[test]
fn hard_break_covers_the_backslash_and_ending() {
    let src = "one\\\ntwo";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, brk) = find_child(src, &doc, p, &is_hard_break);
    assert_eq!(brk, "\\\n");
}

#[test]
fn inline_html_covers_its_tag() {
    let src = "a <b>x</b>";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, open) = find_child(src, &doc, p, &is_inline_html);
    assert_eq!(open, "<b>");
}

#[test]
fn link_covers_its_whole_form() {
    let src = "see [text](/url) here";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (link, covered) = find_child(src, &doc, p, &is_link);
    assert_eq!(covered, "[text](/url)");
    let label = doc.arena.children(link)[0];
    assert_eq!(slice(src, &doc, label), "text");
}

#[test]
fn image_covers_its_whole_form() {
    let src = "![alt](/i.png)";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, covered) = find_child(src, &doc, p, &is_image);
    assert_eq!(covered, "![alt](/i.png)");
}

#[test]
fn multi_line_paragraph_lines_map_separately() {
    let src = "first\nsecond";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, first) = find_child(src, &doc, p, &is_text("first"));
    assert_eq!(first, "first");
    let (_, second) = find_child(src, &doc, p, &is_text("second"));
    assert_eq!(second, "second");
}

#[test]
fn lazy_continuation_lines_map_to_their_own_source() {
    let src = "> quoted\nlazy";
    let doc = parse_document(src);
    let quote = doc.arena.children(doc.root)[0];
    let p = doc.arena.children(quote)[0];
    let (_, quoted) = find_child(src, &doc, p, &is_text("quoted"));
    assert_eq!(quoted, "quoted");
    let (_, lazy) = find_child(src, &doc, p, &is_text("lazy"));
    assert_eq!(lazy, "lazy");
}

#[test]
fn partially_consumed_tab_maps_to_the_tab_byte() {
    // The list marker consumes part of the tab; the content's synthetic
    // spaces stand for the tab byte itself, and the content after them
    // is where it is in the source.
    let src = "-\titem";
    let doc = parse_document(src);
    let list = doc.arena.children(doc.root)[0];
    let item = doc.arena.children(list)[0];
    let p = doc.arena.children(item)[0];
    let (_, text) = find_child(src, &doc, p, &is_text("item"));
    assert_eq!(text, "item");
}

#[test]
fn nul_replacement_maps_to_the_nul_byte() {
    // The buffer holds one U+FFFD where the source holds one NUL byte;
    // the run's span starts at the 'a' and ends past the 'b', so the
    // replacement's three buffer bytes collapse onto the NUL's offset.
    let src = "a\u{0}b";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let texts: Vec<NodeId> = doc
        .arena
        .children(p)
        .iter()
        .copied()
        .filter(|&id| matches!(doc.arena.kind(id), Node::Text(_)))
        .collect();
    assert_eq!(texts.len(), 1, "one run spans the replacement");
    let span = doc.arena.span(texts[0]);
    assert_eq!(span.start, 0, "the run starts at the first byte");
    assert_eq!(span.end, 3, "the run ends past the last byte");
}

#[test]
fn setext_content_skips_consumed_definitions() {
    // The leading reference definition is consumed from the paragraph
    // before it becomes a setext heading; the heading's text must start
    // after the definition, not at the paragraph's start.
    let src = "[a]: /u\nhead\n===";
    let doc = parse_document(src);
    let heading = doc.arena.children(doc.root)[0];
    assert!(matches!(doc.arena.kind(heading), Node::Heading(_)));
    let (_, text) = find_child(src, &doc, heading, &is_text("head"));
    assert_eq!(text, "head");
}

#[test]
fn atx_heading_text_covers_its_content() {
    let src = "## deep";
    let doc = parse_document(src);
    let heading = doc.arena.children(doc.root)[0];
    let (_, text) = find_child(src, &doc, heading, &is_text("deep"));
    assert_eq!(text, "deep");
}

#[test]
fn atx_heading_text_drops_the_closing_run() {
    // The closing hash run is not part of the text, so the span must not
    // cover it.
    let src = "# title #";
    let doc = parse_document(src);
    let heading = doc.arena.children(doc.root)[0];
    let (_, text) = find_child(src, &doc, heading, &is_text("title"));
    assert_eq!(text, "title");
}

#[test]
fn container_spans_enclose_children_across_shared_delimiters() {
    // Nested matches can share a delimiter run: the second star of the
    // trailing run closes the inner emphasis while living outside the
    // outer's own closer. Both containers' spans must still enclose their
    // children; without the subtree union the outer emphasis' span ended
    // one byte short of its inner child's.
    let src = "*\u{11660}*\u{1555}**";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    for &child in doc.arena.children(p) {
        let span = doc.arena.span(child);
        for &grand in doc.arena.children(child) {
            let inner = doc.arena.span(grand);
            assert!(
                inner.start >= span.start && inner.end <= span.end,
                "grandchild span {inner:?} outside container span {span:?}"
            );
        }
    }
}

#[test]
fn leading_whitespace_is_outside_the_first_text() {
    let src = "   indented";
    let doc = parse_document(src);
    let p = paragraph(&doc);
    let (_, text) = find_child(src, &doc, p, &is_text("indented"));
    assert_eq!(text, "indented");
}
