//! The JSON tree format: a Geist4 document as one JSON object.
//!
//! The format serializes the tree exactly as the parser built it. Every
//! node carries its kind, its position in the source and its own
//! fields; extension nodes appear as the kinds their extensions
//! declared, with their attributes in declaration order; link reference
//! definitions and parse problems ride at the top level beside the
//! root. It exists so hosts that do not speak Rust can inspect the same
//! tree the renderers walk. The mdast export remains the compatibility
//! shape for tools that expect unist.
//!
//! The format is versioned by a `format` tag, `geist4/tree@1`, and
//! described by a JSON Schema published in the repository at
//! `docs/schema/geist4-tree-v1.json`. Tests validate every serialized
//! fixture and every cross-binding case against that schema.
//!
//! Positions use the same convention as the mdast export: 1-based
//! lines, 1-based columns and 0-based offsets, all counted in UTF-16
//! code units, so a JavaScript host can index the source string with
//! them directly.
//!
//! The format's node type names mirror the core tree's node kinds one
//! to one, in snake case: `document`, `block_quote`, `list`, `item`,
//! `heading`, `thematic_break`, `paragraph`, `code_block`, `html_block`,
//! `text`, `code_span`, `inline_html`, `soft_break`, `hard_break`,
//! `emphasis`, `strong`, `link` — plus extension kinds, whose type is
//! the name the extension declared. An extension node's attributes are
//! an array of `{name, value}` pairs rather than an object so that the
//! declaration order survives JSON's unordered objects.

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::AttrValue;
use geist4_core::node::{LinkRef, LinkRefKind, Node};
use geist4_core::parser::Document;
use geist4_core::span::Span;
use geist4_mdast::{LineIndex, Point};
use serde_json::{Map, Value, json};

/// The format tag every serialization carries.
pub const FORMAT: &str = "geist4/tree@1";

/// Serializes a parsed document to the JSON tree format.
///
/// `source` is the exact string the document was parsed from: positions
/// are mapped through it. The result is one JSON object with the format
/// tag, the root node, every link reference definition in document
/// order, and the problems the parse recorded.
#[must_use]
pub fn to_value(document: &Document<'_>, source: &str) -> Value {
    let lines = LineIndex::new(source);
    let arena = &document.arena;
    let root = node_value(arena, document.root, source, &lines);
    let link_defs: Vec<Value> = document
        .defs
        .iter()
        .map(|def| {
            json!({
                "label": def.def.label,
                "url": def.def.destination,
                "title": def.def.title.as_deref(),
                "titled": def.titled,
                "position": position_value(source, &lines, def.span),
            })
        })
        .collect();
    let problems: Vec<Value> = document
        .problems
        .iter()
        .map(|problem| {
            json!({
                "message": problem.message,
                "position": position_value(source, &lines, problem.span),
            })
        })
        .collect();
    json!({
        "format": FORMAT,
        "root": root,
        "linkDefs": link_defs,
        "problems": problems,
    })
}

/// Serializes a parsed document to the JSON tree format as JSON text.
///
/// The same tree [`to_value`][self::to_value] builds, rendered compact
/// or pretty. This is what `geist4 render --format json` prints.
#[must_use]
pub fn to_json(document: &Document<'_>, source: &str, pretty: bool) -> String {
    let value = to_value(document, source);
    if pretty {
        serde_json::to_string_pretty(&value).expect("tree JSON serializes")
    } else {
        serde_json::to_string(&value).expect("tree JSON serializes")
    }
}

/// The `position` object for the byte span `[start, end)`.
fn position_value(source: &str, lines: &LineIndex, span: Span) -> Value {
    let point = |at: usize| -> Value {
        let Point {
            line,
            column,
            offset,
        } = lines.point(source, at);
        json!({
            "line": line,
            "column": column,
            "offset": offset,
        })
    };
    json!({
        "start": point(span.start),
        "end": point(span.end),
    })
}

/// The node for one arena node: its type, position, own fields and
/// children.
fn node_value(arena: &Arena<'_>, id: NodeId, source: &str, lines: &LineIndex) -> Value {
    let kind = arena.kind(id);
    let span = arena.span(id);
    let mut node = Map::new();
    match kind {
        Node::Document => {
            node.insert("type".into(), json!("document"));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::BlockQuote => {
            node.insert("type".into(), json!("block_quote"));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::List(data) => {
            node.insert("type".into(), json!("list"));
            node.insert("ordered".into(), json!(data.ordered));
            node.insert("start".into(), json!(data.start));
            node.insert("tight".into(), json!(data.tight));
            node.insert("marker".into(), json!(char::from(data.marker)));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::Item(data) => {
            node.insert("type".into(), json!("item"));
            node.insert("padding".into(), json!(data.padding));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::Heading(data) => {
            node.insert("type".into(), json!("heading"));
            node.insert("level".into(), json!(data.level));
            node.insert("setext".into(), json!(data.setext));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::ThematicBreak => {
            node.insert("type".into(), json!("thematic_break"));
        }
        Node::Paragraph => {
            node.insert("type".into(), json!("paragraph"));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::CodeBlock(data) => {
            node.insert("type".into(), json!("code_block"));
            node.insert("fenced".into(), json!(data.fenced));
            node.insert(
                "fence_char".into(),
                json!(char::from(data.fence_char).to_string()),
            );
            node.insert("info".into(), json!(data.info.as_deref()));
            node.insert("literal".into(), json!(data.literal));
        }
        Node::HtmlBlock(data) => {
            node.insert("type".into(), json!("html_block"));
            node.insert("kind".into(), json!(data.kind));
            node.insert("literal".into(), json!(data.literal));
        }
        Node::Text(text) => {
            node.insert("type".into(), json!("text"));
            node.insert("value".into(), json!(text));
        }
        Node::CodeSpan(text) => {
            node.insert("type".into(), json!("code_span"));
            // The raw source between the fence markers, backslashes and
            // all; renderers strip the edges. Same rule as the tree.
            node.insert("value".into(), json!(text));
        }
        Node::InlineHtml(text) => {
            node.insert("type".into(), json!("inline_html"));
            node.insert("value".into(), json!(text));
        }
        Node::SoftBreak => {
            node.insert("type".into(), json!("soft_break"));
        }
        Node::HardBreak => {
            node.insert("type".into(), json!("hard_break"));
        }
        Node::Emphasis => {
            node.insert("type".into(), json!("emphasis"));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::Strong => {
            node.insert("type".into(), json!("strong"));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::Link(data) => {
            node.insert("type".into(), json!("link"));
            node.insert("url".into(), json!(data.url));
            node.insert("title".into(), json!(data.title.as_deref()));
            node.insert("image".into(), json!(data.image));
            node.insert("reference".into(), reference_value(&data.reference));
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
        Node::Extension(ext) => {
            node.insert("type".into(), json!(ext.kind));
            node.insert("attrs".into(), attrs_value(&ext.attrs));
            // Extension nodes always carry children, even when empty,
            // so consumers can treat extension kinds uniformly.
            node.insert("children".into(), children_value(arena, id, source, lines));
        }
    }
    node.insert("position".into(), position_value(source, lines, span));
    Value::Object(node)
}

/// The children of one node.
fn children_value(arena: &Arena<'_>, id: NodeId, source: &str, lines: &LineIndex) -> Value {
    Value::Array(
        arena
            .children(id)
            .iter()
            .map(|child| node_value(arena, *child, source, lines))
            .collect(),
    )
}

/// The `reference` field of a link: `null` for an inline link, or the
/// reference label and how the reference was written.
fn reference_value(reference: &Option<LinkRef<'_>>) -> Value {
    match reference {
        None => Value::Null,
        Some(link_ref) => json!({
            "label": link_ref.label,
            "kind": match link_ref.kind {
                LinkRefKind::Full => "full",
                LinkRefKind::Collapsed => "collapsed",
                LinkRefKind::Shortcut => "shortcut",
            },
        }),
    }
}

/// Extension attributes as `{name, value}` pairs, in declaration order.
fn attrs_value(attrs: &[(String, AttrValue)]) -> Value {
    Value::Array(
        attrs
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    AttrValue::Str(text) => json!(text),
                    AttrValue::Int(number) => json!(number),
                    AttrValue::Bool(flag) => json!(flag),
                };
                json!({ "name": name, "value": value })
            })
            .collect(),
    )
}
