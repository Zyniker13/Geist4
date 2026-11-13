//! A block-level HTML renderer: milestone M1's bridge to the specification
//! suite.
//!
//! It renders the block tree with the reference implementation's exact
//! line-breaking conventions (`cr` writes a newline only when the output
//! does not already end with one, and the closing tags of lists, items,
//! headings and paragraphs carry their own newline), so block-structure
//! examples compare byte for byte. Inline content is escaped and emitted as
//! plain text; the full inline parser and the real HTML renderer arrive in
//! milestone M2, when this module is replaced by `geist4-html`.
//!
//! The walk is explicit: both the block and the inline recursion are
//! flattened into a work stack, so arbitrarily deep nesting cannot
//! overflow the call stack.

use crate::arena::{Arena, NodeId};
use crate::node::{ListData, Node};

/// Renders a parsed document to HTML the way milestone M1 does.
///
/// The output carries no final newline, matching the specification suite's
/// expected values.
#[must_use]
pub fn render(document: &crate::parser::Document<'_>) -> String {
    let mut renderer = Renderer {
        arena: &document.arena,
        out: String::new(),
    };
    let mut stack: Vec<Step> = document
        .arena
        .children(document.root)
        .iter()
        .rev()
        .map(|child| Step::Block(*child, false))
        .collect();
    while let Some(step) = stack.pop() {
        renderer.run(step, &mut stack);
    }
    if renderer.out.ends_with('\n') {
        renderer.out.pop();
    }
    renderer.out
}

/// One pending piece of rendering work.
enum Step {
    /// Writes a newline unless the output already ends with one.
    Cr,
    /// Writes pre-rendered text.
    Text(String),
    /// Renders one block node; `tight` is true inside a tight list's item.
    Block(NodeId, bool),
    /// Renders one list item, whose children get `tight`.
    Item(NodeId, bool),
    /// Renders one node's inline children.
    Inline(NodeId),
}

struct Renderer<'a> {
    arena: &'a Arena<'a>,
    out: String,
}

impl Renderer<'_> {
    fn run(&mut self, step: Step, stack: &mut Vec<Step>) {
        match step {
            Step::Cr => self.cr(),
            Step::Text(text) => self.out.push_str(&text),
            Step::Block(id, tight) => self.block(id, tight, stack),
            Step::Item(id, tight) => self.item(id, tight, stack),
            Step::Inline(id) => self.inline(id, stack),
        }
    }

    /// Writes a newline unless the output is empty or already ends with one.
    fn cr(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    /// Pushes `id`'s children as block steps, in document order.
    fn push_children(&self, id: NodeId, tight: bool, stack: &mut Vec<Step>) {
        for child in self.arena.children(id).iter().rev() {
            stack.push(Step::Block(*child, tight));
        }
    }

    /// Expands one block node into work-stack steps.
    fn block(&mut self, id: NodeId, tight: bool, stack: &mut Vec<Step>) {
        match self.arena.kind(id) {
            Node::Document => unreachable!("the document root has no parent to render it"),
            Node::BlockQuote => {
                self.cr();
                self.out.push_str("<blockquote>\n");
                stack.push(Step::Text("</blockquote>\n".to_owned()));
                stack.push(Step::Cr);
                self.push_children(id, false, stack);
            }
            Node::List(data) => {
                let item_tight = data.tight;
                let open = self.list_open_tag(data);
                self.cr();
                self.out.push_str(&open);
                stack.push(Step::Text(if data.ordered {
                    "</ol>\n".to_owned()
                } else {
                    "</ul>\n".to_owned()
                }));
                for item in self.arena.children(id).iter().rev() {
                    stack.push(Step::Item(*item, item_tight));
                }
            }
            Node::Item(_) => {
                // Items are rendered by their list.
                unreachable!("an item is always inside a list");
            }
            Node::Heading(data) => {
                let level = data.level;
                let level_char = char::from(b'0' + level);
                self.cr();
                self.out.push_str("<h");
                self.out.push(level_char);
                self.out.push('>');
                stack.push(Step::Text(format!("</h{level_char}>\n")));
                stack.push(Step::Inline(id));
            }
            Node::ThematicBreak => {
                self.cr();
                self.out.push_str("<hr />\n");
            }
            Node::Paragraph => {
                if tight {
                    stack.push(Step::Inline(id));
                } else {
                    self.cr();
                    self.out.push_str("<p>");
                    stack.push(Step::Text("</p>\n".to_owned()));
                    stack.push(Step::Inline(id));
                }
            }
            Node::CodeBlock(data) => {
                self.cr();
                self.out.push_str("<pre><code");
                if let Some(info) = &data.info {
                    if !info.is_empty() {
                        self.out.push_str(" class=\"");
                        if !info.starts_with("language-") {
                            self.out.push_str("language-");
                        }
                        let word_end = info.find([' ', '\t']).unwrap_or(info.len());
                        self.out.push_str(&escape_html(&info[..word_end]));
                        self.out.push('"');
                    }
                }
                self.out.push('>');
                self.out.push_str(&escape_html(&data.literal));
                self.out.push_str("</code></pre>\n");
            }
            Node::HtmlBlock(data) => {
                self.cr();
                self.out.push_str(data.literal.trim_end_matches('\n'));
                self.out.push('\n');
            }
            Node::Text(_) => {
                // Text outside a leaf is not rendered at block level.
            }
            _ => {
                // Inline nodes only appear inside leaf blocks.
            }
        }
    }

    /// Expands one list item: `<li>`, its children, then `</li>`.
    fn item(&mut self, id: NodeId, tight: bool, stack: &mut Vec<Step>) {
        self.cr();
        self.out.push_str("<li>");
        stack.push(Step::Text("</li>\n".to_owned()));
        self.push_children(id, tight, stack);
    }

    /// Writes the opening tag of a list: `<ol>` with a `start` attribute
    /// when the first item is not numbered one.
    fn list_open_tag(&self, data: &ListData) -> String {
        if data.ordered {
            let mut tag = String::from("<ol");
            if data.start != 1 {
                tag.push_str(" start=\"");
                tag.push_str(&data.start.to_string());
                tag.push('"');
            }
            tag.push_str(">\n");
            tag
        } else {
            String::from("<ul>\n")
        }
    }

    /// Expands a leaf block's inline content into work-stack steps.
    fn inline(&mut self, id: NodeId, stack: &mut Vec<Step>) {
        for child in self.arena.children(id).iter().rev() {
            let child = *child;
            match self.arena.kind(child) {
                Node::Text(text) => stack.push(Step::Text(escape_html(text))),
                Node::CodeSpan(text) => {
                    stack.push(Step::Text(format!("<code>{}</code>", escape_html(text))))
                }
                Node::InlineHtml(raw) => stack.push(Step::Text(raw.to_string())),
                Node::SoftBreak => stack.push(Step::Text("\n".to_owned())),
                Node::HardBreak => stack.push(Step::Text("<br />\n".to_owned())),
                Node::Emphasis => {
                    stack.push(Step::Text("</em>".to_owned()));
                    stack.push(Step::Inline(child));
                    stack.push(Step::Text("<em>".to_owned()));
                }
                Node::Strong => {
                    stack.push(Step::Text("</strong>".to_owned()));
                    stack.push(Step::Inline(child));
                    stack.push(Step::Text("<strong>".to_owned()));
                }
                Node::Link(data) if data.image => {
                    let mut tag = String::from("<img src=\"");
                    if !data.url.is_empty() && !is_dangerous_url(&data.url) {
                        tag.push_str(&escape_href(&data.url));
                    }
                    tag.push_str("\" alt=\"");
                    tag.push_str(&escape_html(&self.plain_text(child)));
                    if let Some(title) = &data.title {
                        tag.push_str("\" title=\"");
                        tag.push_str(&escape_html(title));
                    }
                    tag.push_str("\" />");
                    stack.push(Step::Text(tag));
                }
                Node::Link(data) => {
                    let mut tag = String::from("<a href=\"");
                    if !data.url.is_empty() && !is_dangerous_url(&data.url) {
                        tag.push_str(&escape_href(&data.url));
                    }
                    if let Some(title) = &data.title {
                        tag.push_str("\" title=\"");
                        tag.push_str(&escape_html(title));
                    }
                    tag.push_str("\">");
                    stack.push(Step::Text("</a>".to_owned()));
                    stack.push(Step::Inline(child));
                    stack.push(Step::Text(tag));
                }
                _ => {}
            }
        }
    }

    /// The plain text of an inline subtree, as image alt text. The walk is
    /// iterative so deeply nested images cannot overflow the stack.
    fn plain_text(&self, id: NodeId) -> String {
        let mut out = String::new();
        let mut stack: Vec<NodeId> = self.arena.children(id).iter().rev().copied().collect();
        while let Some(node) = stack.pop() {
            match self.arena.kind(node) {
                Node::Text(text) | Node::CodeSpan(text) | Node::InlineHtml(text) => {
                    out.push_str(text)
                }
                Node::SoftBreak | Node::HardBreak => out.push(' '),
                _ => stack.extend(self.arena.children(node).iter().rev().copied()),
            }
        }
        out
    }
}

/// Whether a URL is one the default (safe) rendering suppresses: the
/// `javascript:`, `vbscript:`, `file:` and `data:` schemes, except the
/// image data schemes. Matching is by bytes so multi-byte characters in
/// the URL cannot split a comparison.
fn is_dangerous_url(url: &str) -> bool {
    let bytes = url.as_bytes();
    let has_prefix =
        |prefix: &[u8]| bytes.len() >= prefix.len() && bytes[..prefix.len()] == *prefix;
    if has_prefix(b"data:image/") {
        let rest = &bytes[11..];
        for ext in [&b"png"[..], &b"gif"[..], &b"jpeg"[..], &b"webp"[..]] {
            if rest.len() >= ext.len() && rest[..ext.len()] == *ext {
                return false;
            }
        }
        return true;
    }
    has_prefix(b"javascript:")
        || has_prefix(b"vbscript:")
        || has_prefix(b"file:")
        || has_prefix(b"data:")
}

/// Escapes a URL for use in an `href` or `src` attribute: bytes outside
/// the safe set percent-encode, and `&` and `'` become entities.
fn escape_href(url: &str) -> String {
    const HREF_SAFE: [bool; 256] = {
        let mut table = [false; 256];
        let mut i = 0usize;
        while i < 256 {
            let c = i as u8;
            table[i] = c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    b'-' | b'_'
                        | b'.'
                        | b'+'
                        | b'!'
                        | b'*'
                        | b'('
                        | b')'
                        | b','
                        | b'%'
                        | b'#'
                        | b'@'
                        | b'?'
                        | b'='
                        | b';'
                        | b':'
                        | b'/'
                        | b'$'
                        | b'~'
                );
            i += 1;
        }
        table
    };
    let mut out = String::with_capacity(url.len());
    for &byte in url.as_bytes() {
        if HREF_SAFE[byte as usize] {
            out.push(byte as char);
        } else if byte == b'&' {
            out.push_str("&amp;");
        } else if byte == b'\'' {
            out.push_str("&#x27;");
        } else {
            out.push('%');
            out.push(
                char::from_digit(u32::from(byte >> 4), 16)
                    .unwrap_or('0')
                    .to_ascii_uppercase(),
            );
            out.push(
                char::from_digit(u32::from(byte & 0xF), 16)
                    .unwrap_or('0')
                    .to_ascii_uppercase(),
            );
        }
    }
    out
}

/// Escapes text the way the reference implementation does: `&`, `<`, `>` and
/// `"` become entities; `'` and `/` stay raw.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
