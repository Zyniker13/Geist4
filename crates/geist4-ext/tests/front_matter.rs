//! The front matter extension's surface beyond the spec examples: the
//! custom-delimiter variant, the render guard, and the `source`
//! attribute the parser sets on the claimed prologue. The spec file
//! (`spec/front_matter.md`, run by `spec.rs`) pins the splitting
//! rules themselves; these tests pin the extension plumbing around
//! them.

use geist4_core::basic_html::render_with;
use geist4_core::extension::{AttrValue, Options};
use geist4_core::node::Node;
use geist4_core::parser::parse_with;

/// Finds the `front_matter` node in a parsed document, if any.
fn front_matter_node<'a>(doc: &'a geist4_core::Document<'_>) -> Option<&'a (String, AttrValue)> {
    for id in doc.arena.iter() {
        if let Node::Extension(ext) = doc.arena.kind(id) {
            if ext.kind == "front_matter" {
                return ext.attrs.first();
            }
        }
    }
    None
}

/// The custom-delimiter variant claims a prologue with the caller's
/// delimiter, and the claimed text lands in the node's `source`
/// attribute.
#[test]
fn custom_delimiter_claims_prologue() {
    let mut options = Options::new();
    options.extension(geist4_ext::front_matter_with("~~~"));
    let src = "~~~\nkey: value\n~~~\n\nbody\n";
    let doc = parse_with(src, &options);
    let attr = front_matter_node(&doc).expect("the prologue must be claimed");
    assert_eq!(attr.0, "source");
    assert_eq!(
        attr.1,
        AttrValue::Str("~~~\nkey: value\n~~~\n\n".to_owned())
    );
    let html = render_with(&doc, &options);
    assert_eq!(html, "<p>body</p>\n", "the prologue renders as nothing");
}

/// The default and custom delimiters coexist: each extension claims
/// only its own opener, and the first extension in registration order
/// that claims wins.
#[test]
fn each_delimiter_claims_its_own() {
    let mut options = Options::new();
    options.extension(geist4_ext::front_matter_with("~~~"));
    options.extension(geist4_ext::front_matter());
    let doc = parse_with("---\na: b\n---\n\ntext\n", &options);
    let html = render_with(&doc, &options);
    assert_eq!(html, "<p>text</p>\n", "the --- prologue is claimed");
    let doc = parse_with("+++\na: b\n+++\n\ntext\n", &options);
    let html = render_with(&doc, &options);
    assert!(
        html.contains("+++"),
        "a +++ block with no +++ extension is ordinary Markdown: {html}"
    );
}

/// The render guard: a non-front-matter node handed to the extension's
/// renderer renders nothing, and the prologue node itself renders
/// nothing either. The node case is covered through the tree: an
/// unrelated extension's nodes are never routed to front matter's
/// renderer, so here the guard is pinned by rendering a document whose
/// only extension node is the prologue (renders nothing) and one whose
/// prologue node exists under a custom kind name is impossible by
/// construction — the parser hardcodes the kind.
#[test]
fn prologue_renders_nothing_with_and_without_body() {
    for src in [
        "---\na: b\n---\n",         // nothing after the prologue
        "---\na: b\n---\n\ntext\n", // body after
    ] {
        let mut options = Options::new();
        options.extension(geist4_ext::front_matter());
        let doc = parse_with(src, &options);
        let html = render_with(&doc, &options);
        assert_eq!(
            html,
            if src.ends_with("text\n") {
                "<p>text</p>\n"
            } else {
                ""
            },
            "prologue contributes nothing: {src:?} -> {html}"
        );
    }
}

/// The `source` attribute holds the raw prologue text including both
/// delimiter lines and the trailing blank line, and excludes a byte
/// order mark.
#[test]
fn source_attribute_holds_raw_prologue() {
    let mut options = Options::new();
    options.extension(geist4_ext::front_matter());
    let doc = parse_with("---\nx: 1\n---\n\n", &options);
    let attr = front_matter_node(&doc).expect("the prologue must be claimed");
    assert_eq!(attr.1, AttrValue::Str("---\nx: 1\n---\n\n".to_owned()));

    let doc = parse_with("\u{feff}---\nx: 1\n---\n\n", &options);
    let attr = front_matter_node(&doc).expect("the BOM prologue must be claimed");
    assert_eq!(
        attr.1,
        AttrValue::Str("---\nx: 1\n---\n\n".to_owned()),
        "the byte order mark is not part of the prologue's text"
    );
}

/// An unclosed prologue claims nothing: no front_matter node exists and
/// the document renders as if the extension were absent.
#[test]
fn unclosed_prologue_claims_nothing() {
    let mut options = Options::new();
    options.extension(geist4_ext::front_matter());
    let doc = parse_with("---\na: b\ntext\n", &options);
    assert!(
        front_matter_node(&doc).is_none(),
        "an unclosed opener claims nothing"
    );
}

/// The public constructor produces a shared extension value usable
/// through the normal registration path (name reported to the options).
#[test]
fn constructors_build_registered_extensions() {
    let default = geist4_ext::front_matter();
    assert_eq!(default.name(), "front_matter");
    assert_eq!(default.front_matter_delimiter(), Some("---"));
    let custom = geist4_ext::front_matter_with("###");
    assert_eq!(custom.front_matter_delimiter(), Some("###"));
    let mut options = Options::new();
    options.extension(custom);
    let doc = parse_with("###\na: b\n###\n\ntext\n", &options);
    assert_eq!(render_with(&doc, &options), "<p>text</p>\n");
}

/// The `FrontMatterWith` type is public and its trait surface reports
/// the configured delimiter and name; the guard branch of its render
/// (a node of another kind) renders nothing.
#[test]
fn front_matter_with_type_surface() {
    use geist4_core::extension::{ExtNode, Extension, RenderedChild};

    let custom = geist4_ext::front_matter::FrontMatterWith::new("~~~");
    assert_eq!(Extension::name(&custom), "front_matter");
    assert_eq!(custom.front_matter_delimiter(), Some("~~~"));

    let kinds = custom.kinds();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0].kind, "front_matter");
    assert_eq!(kinds[0].attrs, vec!["source".to_owned()]);

    let mut out = String::new();
    let node = ExtNode {
        kind: "not_front_matter".to_owned(),
        attrs: Vec::new(),
    };
    custom.render(&node, &[] as &[RenderedChild<'_>], &mut out);
    assert_eq!(out, "", "a foreign kind renders nothing");

    let prologue = ExtNode {
        kind: "front_matter".to_owned(),
        attrs: Vec::new(),
    };
    custom.render(&prologue, &[] as &[RenderedChild<'_>], &mut out);
    assert_eq!(out, "", "the prologue itself renders nothing");

    let default = geist4_ext::front_matter::FrontMatter;
    assert_eq!(Extension::name(&default), "front_matter");
    assert_eq!(default.front_matter_delimiter(), Some("---"));
    let mut out2 = String::new();
    default.render(&node, &[] as &[RenderedChild<'_>], &mut out2);
    assert_eq!(
        out2, "",
        "a foreign kind renders nothing on the default too"
    );
}
