//! Oracle tests for the heading identifiers extension.
//!
//! `fixtures/heading_oracle.json` is machine-generated from the
//! reference pipeline (marked with marked-gfm-heading-id) by
//! `tools/heading_oracle_probe.mjs` and frozen: it is the source of
//! truth, so do not edit it to make a test pass — regenerate it from
//! the probe and investigate any change.
//!
//! The engine derives a heading's identifier from its decoded text
//! content — GitHub's DOM text semantics, recorded in decision 0021
//! (`docs/decisions/0021-heading-ids-extract-dom-text.md`) — while the
//! reference derives it from the heading's raw source text passed
//! through an entity-eating unescape and a lazy tag-strip regex. They
//! agree everywhere except the four cases in `DIVERGENCES`, each of
//! which traces to a non-conformance of the reference, not of the
//! engine.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;

const ORACLE_JSON: &str = include_str!("fixtures/heading_oracle.json");

/// Cases where the engine deliberately differs from the reference, as
/// `(case, reference ids, engine ids, reason)`. The first two columns
/// are pinned to the frozen fixture, so a regeneration that changes
/// the reference's output fails here loudly.
const DIVERGENCES: &[(&str, &[&str], &[&str], &str)] = &[
    (
        "entity-text",
        &["caf"],
        &["caf\u{e9}"],
        "the reference's unescape deletes the named entity; the engine keeps the decoded text",
    ),
    (
        "html-comment",
        &["-b----title"],
        &["title"],
        "the reference's lazy tag strip stops at the comment's first `>` and leaks its tail",
    ),
    (
        "html-processing",
        &["php--code"],
        &["code"],
        "the reference's tag strip never matches a processing instruction and leaks its inner text",
    ),
    (
        "trailing-space-tag",
        &["x-"],
        &["x"],
        "the reference slugs the stripped text without trimming; the engine trims the extracted text",
    ),
];

/// Extracts heading ids from rendered HTML, in document order: every
/// `<h1>`..`<h6>` start tag's `id` attribute.
fn heading_ids(html: &str) -> Vec<String> {
    let bytes = html.as_bytes();
    let mut ids = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let is_heading_tag = bytes[i] == b'<'
            && bytes.get(i + 1) == Some(&b'h')
            && bytes.get(i + 2).is_some_and(|b| (b'1'..=b'6').contains(b))
            && bytes
                .get(i + 3)
                .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/');
        if !is_heading_tag {
            i += 1;
            continue;
        }
        let Some(tag_end) = bytes[i..].iter().position(|b| *b == b'>') else {
            break;
        };
        let tag = &html[i..i + tag_end];
        if let Some(at) = tag.find(" id=\"") {
            let rest = &tag[at + 5..];
            if let Some(close) = rest.find('"') {
                ids.push(rest[..close].to_owned());
            }
        }
        i += tag_end + 1;
    }
    ids
}

fn rendered_ids(markdown: &str) -> String {
    let mut options = Options::new();
    options.extension(geist4_ext::heading_identifiers());
    let doc = parse_with(markdown, &options);
    heading_ids(&render_with(&doc, &options)).join(" ")
}

#[test]
fn matches_reference_except_documented_divergences() {
    let fixture: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(ORACLE_JSON).expect("valid fixture");
    for (name, case) in &fixture {
        let markdown = case["markdown"].as_str().expect("markdown");
        let oracle_ids: Vec<&str> = case["ids"]
            .as_array()
            .expect("ids array")
            .iter()
            .map(|id| id.as_str().expect("string id"))
            .collect();
        let ours = rendered_ids(markdown);
        if let Some((_, reference, engine, reason)) = DIVERGENCES
            .iter()
            .find(|(divergent, ..)| *divergent == name)
        {
            assert_eq!(
                oracle_ids, *reference,
                "{name}: the frozen reference ids drifted from the divergence record"
            );
            assert_eq!(
                ours,
                engine.join(" "),
                "{name}: the engine's ids drifted from the documented divergence ({reason})"
            );
        } else {
            assert_eq!(
                ours,
                oracle_ids.join(" "),
                "{name}: the engine's ids differ from the reference"
            );
        }
    }
}

#[test]
fn extracts_ids_across_tag_shapes() {
    assert_eq!(heading_ids("<h1 id=\"a\">x</h1>"), vec!["a"]);
    assert_eq!(heading_ids("<h2 class=\"c\" id=\"b\">x</h2>"), vec!["b"]);
    assert_eq!(heading_ids("<h6/>"), Vec::<String>::new());
    assert_eq!(heading_ids("<h12 id=\"x\">"), Vec::<String>::new());
    assert_eq!(
        heading_ids("<h1 id=\"a\">x</h1><p>h1</p><h2 id=\"b\">y</h2>"),
        vec!["a", "b"]
    );
}

/// The `Node::Extension` arm of text extraction walks descendants like
/// the other inline containers, so an inline claimed node inside a
/// heading contributes its rendered text to the identifier. A footnote
/// reference is an inline `Extension` node whose rendered text is
/// empty, so it must leave the identifier alone.
#[test]
fn inline_extension_nodes_contribute_descendants() {
    let mut options = Options::new();
    options.extension(geist4_ext::heading_identifiers());
    options.extension(geist4_ext::footnotes());
    let html = render_with(
        &parse_with("# Note[^1]\n\n[^1]: body\n", &options),
        &options,
    );
    assert!(
        html.contains("<h1 id=\"note\">"),
        "footnote reference must not inject text into the id: {html}"
    );
}
