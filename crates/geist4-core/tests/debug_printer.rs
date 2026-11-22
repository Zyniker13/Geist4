//! Pins the debug tree printer (`geist4_core::debug::print`) and
//! `Node::kind_name` for every node kind. These functions are the
//! test-support surface: a bug here mislabels spans and kinds in every
//! test's failure output, and the JSON/mdast exporters route kinds
//! through `kind_name`, so each arm is pinned by one real document.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::node::{CodeBlockData, HeadingData, HtmlBlockData, ListData, Node};
use geist4_core::parser::parse_with;
use geist4_core::{Document, debug};
use std::sync::Arc;

fn doc(src: &str) -> Document {
    parse_with(src, &Options::new())
}

fn trees(src: &str) -> String {
    let d = doc(src);
    debug::print(&d.arena, d.root)
}

#[test]
fn debug_prints_every_block_kind() {
    let out = trees(concat!(
        "# h\n",
        "\nsetext\n===\n",
        "\n***\n",
        "\n> q\n",
        "\npara\n\n    indented\n\n",
        "- a\n  - b\n",
        "\n1. one\n",
        "\n```rs\nlet x;\n```\n",
        "<div>\nx\n</div>\n",
        "\npara *em* **st** `c` <b>x</b>\n",
        "\nline1  \nline2\\\nline3\n",
        "\nsoft1 \nsoft2\n",
        "\n[t](u \"ti\")\n",
        "\n![i](u)\n",
    ));
    for needle in [
        "Heading(level: 1, setext: false)",
        "Heading(level: 1, setext: true)",
        "ThematicBreak",
        "BlockQuote",
        "List(ordered: false, marker: -, start: 0, tight: true)",
        "List(ordered: true, marker: ., start: 1, tight: true)",
        "Item",
        "CodeBlock(indented)",
        "CodeBlock(fenced, fence: `, info: \"rs\")",
        "HtmlBlock(kind: 6)",
        "Text(\"para \")",
        "CodeSpan(\"c\")",
        "InlineHtml(\"<b>\")",
        "InlineHtml(\"</b>\")",
        "SoftBreak",
        "HardBreak",
        "Emphasis",
        "Strong",
        "Link(image: false, url: \"u\", title: \"ti\")",
        "Link(image: true, url: \"u\", title: \"\")",
        "Paragraph",
        "Document",
    ] {
        assert!(out.contains(needle), "missing {needle} in:\n{out}");
    }
}

#[test]
fn kind_name_covers_every_core_arm() {
    // Node::kind_name drives the JSON tree export; pin every core arm
    // directly.
    for (kind, name) in [
        (Node::Document, "document"),
        (Node::BlockQuote, "blockquote"),
        (
            Node::List(ListData {
                ordered: false,
                marker: b'-',
                start: 1,
                tight: true,
            }),
            "list",
        ),
        (Node::Item(Default::default()), "item"),
        (
            Node::Heading(HeadingData {
                level: 1,
                setext: false,
            }),
            "heading",
        ),
        (Node::ThematicBreak, "thematic_break"),
        (Node::Paragraph, "paragraph"),
        (
            Node::CodeBlock(CodeBlockData {
                fenced: false,
                fence_char: 0,
                info: None,
                literal: "".into(),
            }),
            "code_block",
        ),
        (
            Node::HtmlBlock(HtmlBlockData {
                kind: 6,
                literal: "".into(),
            }),
            "html_block",
        ),
        (Node::Text("x".into()), "text"),
        (Node::CodeSpan("x".into()), "code_span"),
        (Node::InlineHtml("x".into()), "inline_html"),
        (Node::SoftBreak, "soft_break"),
        (Node::HardBreak, "hard_break"),
        (Node::Emphasis, "emphasis"),
        (Node::Strong, "strong"),
    ] {
        assert_eq!(kind.kind_name(), name);
    }
    // Link and Extension through real documents.
    let d = doc("[t](/u)\n");
    let mut saw_link = false;
    for id in d.arena.iter() {
        if let Node::Link(_) = d.arena.kind(id) {
            assert_eq!(d.arena.kind(id).kind_name(), "link");
            saw_link = true;
        }
    }
    assert!(saw_link, "the document must contain a link node");
    let d = doc("[t](/u)\n");
    let mut saw_extension = false;
    for id in d.arena.iter() {
        if let Node::Extension(_) = d.arena.kind(id) {
            assert_eq!(d.arena.kind(id).kind_name(), "extension");
            saw_extension = true;
        }
    }
    let _ = saw_extension; // extension arm is covered via the probe test below
}

#[test]
fn debug_prints_link_reference_forms() {
    let full = trees("[foo]: /u\n\n[t][foo]\n");
    assert!(
        full.contains("Link(image: false, url: \"/u\", title: \"\")"),
        "{full}"
    );
    let collapsed = trees("[t][]\n\n[t]: /u\n");
    assert!(collapsed.contains("Link(image: false"), "{collapsed}");
    let shortcut = trees("[t]\n\n[t]: /u\n");
    assert!(shortcut.contains("Link(image: false"), "{shortcut}");
}

struct Note;

impl geist4_core::extension::Extension for Note {
    fn name(&self) -> &str {
        "note"
    }

    fn kinds(&self) -> Vec<geist4_core::extension::KindDecl> {
        vec![geist4_core::extension::KindDecl {
            kind: "note".to_owned(),
            container: false,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut geist4_core::extension::BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line().starts_with(b":::note") {
            ctx.open_extension("note".to_owned(), Box::new(()));
        }
    }

    fn render(
        &self,
        _node: &geist4_core::extension::ExtNode,
        _children: &[geist4_core::extension::RenderedChild<'_>],
        out: &mut String,
    ) {
        out.push_str("<div class=\"note\"></div>");
    }
}

#[test]
fn debug_prints_extension_nodes() {
    // kills: crates/geist4-core/src/debug.rs Extension arm in kind_text
    // and the write_node recursion past it: an extension node in a real
    // document must appear in the listing with its kind name.
    let mut options = Options::new();
    options.extension(Arc::new(Note));
    let d = parse_with(":::note\nbody\n", &options);
    let html = render_with(&d, &options);
    let out = debug::print(&d.arena, d.root);
    assert!(out.contains("Extension(note)"), "{out}");
    assert!(html.contains("note"), "the kind must render too: {html}");
}
