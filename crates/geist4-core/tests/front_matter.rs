//! The front matter hook's byte-level cases: carriage returns and byte
//! order marks, which the spec example format cannot carry (its line
//! reader strips a carriage return before a line feed). Every expected
//! output here was verified byte for byte against the comrak 0.51.0
//! library parser, the oracle recorded in
//! `docs/decisions/0018-footnotes-follow-comrak.md`; the built-in
//! extension is exercised by the spec suite and the differential
//! battery, and behaves identically through the same hook.

use geist4_core::basic_html::render_with;
use geist4_core::extension::{ExtNode, Extension, KindDecl, Options};
use geist4_core::parser::parse_with;

/// A front matter extension with a fixed delimiter, as any loaded
/// extension would declare one.
struct Delimited(&'static str);

impl Extension for Delimited {
    fn name(&self) -> &str {
        "front-matter-test"
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

fn options() -> Options {
    let mut options = Options::new();
    options.extension(std::sync::Arc::new(Delimited("---")));
    options
}

fn html(source: &str) -> String {
    render_with(&parse_with(source, &options()), &options())
}

#[test]
fn crlf_throughout() {
    // The prologue's opener, body and closer all use CRLF; the closer's
    // line ending belongs to the prologue.
    assert_eq!(html("---\r\na: b\r\n---\r\ntext\r\n"), "<p>text</p>\n");
}

#[test]
fn carriage_return_closer_wins_over_earlier_line_feed() {
    // The reference searches the three closer shapes in a fixed order,
    // so a CRLF closer anywhere in the body wins over an earlier
    // line-feed-only closer and the body extends through it.
    assert_eq!(html("---\na\n\n---\nzzz\n---\r\ntext\n"), "<p>text</p>\n");
}

#[test]
fn closer_at_end_of_input_with_bare_carriage_return_is_not_front_matter() {
    // A closer candidate whose delimiter is followed by a lone
    // carriage return is not a closer: the document parses plainly.
    assert_eq!(html("---\na\n---\r"), "<hr />\n<h2>a</h2>\n");
}

#[test]
fn byte_order_mark_before_the_opener() {
    // One byte order mark may precede the opener and is not part of
    // the prologue's own text.
    assert_eq!(html("\u{feff}---\na: b\n---\ntext\n"), "<p>text</p>\n");
}

#[test]
fn prologue_node_holds_the_raw_text_and_span() {
    // The claimed prologue is the root's first child: a `front_matter`
    // node whose `source` attribute is the raw prologue text, spanning
    // its real bytes.
    let doc = parse_with("---\na: b\n---\ntext\n", &options());
    let children = doc.arena.children(doc.root);
    assert_eq!(children.len(), 2);
    match doc.arena.kind(children[0]) {
        geist4_core::node::Node::Extension(ext) => {
            assert_eq!(ext.kind, "front_matter");
            assert_eq!(ext.attrs[0].0, "source");
            assert_eq!(
                ext.attrs[0].1,
                geist4_core::extension::AttrValue::Str("---\na: b\n---\n".to_owned())
            );
        }
        other => panic!("expected an extension node, got {other:?}"),
    }
    assert_eq!(
        doc.arena.span(children[0]),
        geist4_core::span::Span::new(0, 13)
    );
    // The rest of the document keeps its real offsets: the paragraph
    // starts at byte 13, right after the prologue.
    assert_eq!(doc.arena.span(children[1]).start, 13);
}

#[test]
fn no_prologue_leaves_no_node() {
    // Without a matching prologue the document is untouched.
    let doc = parse_with("text\n", &options());
    assert_eq!(doc.arena.children(doc.root).len(), 1);
    assert_eq!(html("text\n"), "<p>text</p>\n");
}
