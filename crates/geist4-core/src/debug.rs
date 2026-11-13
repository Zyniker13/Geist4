//! A debug printer for document trees, used in tests.
//!
//! The output is one line per node, indented by depth, with each node's
//! source span as `[start..end]` so tests can show exactly what the parser
//! built and where it put it.

use crate::arena::{Arena, NodeId};
use crate::node::Node;
use std::fmt::Write as _;

/// Renders the tree rooted at `root` as a debug listing.
#[must_use]
pub fn print(arena: &Arena<'_>, root: NodeId) -> String {
    let mut out = String::new();
    write_node(arena, root, 0, &mut out);
    out
}

fn write_node(arena: &Arena<'_>, id: NodeId, depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    let span = arena.span(id);
    write!(
        out,
        "{} [{}, {}]",
        kind_text(arena.kind(id)),
        span.start,
        span.end
    )
    .expect("writing to a string cannot fail");
    out.push('\n');
    for child in arena.children(id) {
        write_node(arena, *child, depth + 1, out);
    }
}

fn kind_text(kind: &Node<'_>) -> String {
    match kind {
        Node::Document => "Document".to_string(),
        Node::BlockQuote => "BlockQuote".to_string(),
        Node::List(data) => format!(
            "List(ordered: {}, marker: {}, start: {}, tight: {})",
            data.ordered,
            char::from(data.marker),
            data.start,
            data.tight
        ),
        Node::Item(_) => "Item".to_string(),
        Node::Heading(data) => {
            format!("Heading(level: {}, setext: {})", data.level, data.setext)
        }
        Node::ThematicBreak => "ThematicBreak".to_string(),
        Node::Paragraph => "Paragraph".to_string(),
        Node::CodeBlock(data) => {
            if data.fenced {
                format!(
                    "CodeBlock(fenced, fence: {}, info: {:?})",
                    char::from(data.fence_char),
                    data.info.as_deref().unwrap_or(""),
                )
            } else {
                "CodeBlock(indented)".to_string()
            }
        }
        Node::HtmlBlock(data) => format!("HtmlBlock(kind: {})", data.kind),
        Node::Text(text) => format!("Text({text:?})"),
        Node::CodeSpan(text) => format!("CodeSpan({text:?})"),
        Node::InlineHtml(text) => format!("InlineHtml({text:?})"),
        Node::SoftBreak => "SoftBreak".to_string(),
        Node::HardBreak => "HardBreak".to_string(),
        Node::Emphasis => "Emphasis".to_string(),
        Node::Strong => "Strong".to_string(),
        Node::Link(data) => format!(
            "Link(image: {}, url: {:?}, title: {:?})",
            data.image,
            &*data.url,
            data.title.as_deref().unwrap_or("")
        ),
    }
}
