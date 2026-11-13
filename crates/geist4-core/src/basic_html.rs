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

use crate::arena::{Arena, NodeId};
use crate::node::{ListData, Node};

/// Renders a parsed document to HTML the way milestone M1 does.
///
/// The output carries no final newline, matching the specification suite's
/// expected values.
#[must_use]
pub fn render(document: &crate::parser::Document<'_>) -> String {
    let mut out = Renderer {
        arena: &document.arena,
        out: String::new(),
    };
    for child in out.arena.children(document.root) {
        let child = *child;
        out.block(child, false);
    }
    if out.out.ends_with('\n') {
        out.out.pop();
    }
    out.out
}

struct Renderer<'a> {
    arena: &'a Arena<'a>,
    out: String,
}

impl Renderer<'_> {
    /// Writes a newline unless the output is empty or already ends with one.
    fn cr(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    /// Renders one block node. `tight` is true for direct children of a
    /// tight list's item, where paragraphs lose their tags.
    fn block(&mut self, id: NodeId, tight: bool) {
        match self.arena.kind(id) {
            Node::Document => unreachable!("the document root has no parent to render it"),
            Node::BlockQuote => {
                self.cr();
                self.out.push_str("<blockquote>\n");
                for child in self.arena.children(id) {
                    let child = *child;
                    self.block(child, false);
                }
                self.cr();
                self.out.push_str("</blockquote>\n");
            }
            Node::List(data) => {
                let tight = data.tight;
                self.cr();
                self.list_open_tag(data);
                for item in self.arena.children(id) {
                    let item = *item;
                    self.cr();
                    self.out.push_str("<li>");
                    for child in self.arena.children(item) {
                        let child = *child;
                        self.block(child, tight);
                    }
                    self.out.push_str("</li>\n");
                }
                self.out
                    .push_str(if data.ordered { "</ol>\n" } else { "</ul>\n" });
            }
            Node::Item(_) => {
                // Items are rendered by their list.
                unreachable!("an item is always inside a list");
            }
            Node::Heading(data) => {
                let level = data.level;
                self.cr();
                self.out.push_str("<h");
                self.out.push(char::from(b'0' + level));
                self.out.push('>');
                self.inline(id);
                self.out.push_str("</h");
                self.out.push(char::from(b'0' + level));
                self.out.push_str(">\n");
            }
            Node::ThematicBreak => {
                self.cr();
                self.out.push_str("<hr />\n");
            }
            Node::Paragraph => {
                if tight {
                    self.inline(id);
                } else {
                    self.cr();
                    self.out.push_str("<p>");
                    self.inline(id);
                    self.out.push_str("</p>\n");
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

    /// Writes the opening tag of a list: `<ol>` with a `start` attribute
    /// when the first item is not numbered one.
    fn list_open_tag(&mut self, data: &ListData) {
        if data.ordered {
            self.out.push_str("<ol");
            if data.start != 1 {
                self.out.push_str(" start=\"");
                self.out.push_str(&data.start.to_string());
                self.out.push('"');
            }
            self.out.push_str(">\n");
        } else {
            self.out.push_str("<ul>\n");
        }
    }

    /// Renders a leaf block's inline content.
    fn inline(&mut self, id: NodeId) {
        for child in self.arena.children(id) {
            let child = *child;
            match self.arena.kind(child) {
                Node::Text(text) => self.out.push_str(&escape_html(text)),
                Node::CodeSpan(text) => {
                    self.out.push_str("<code>");
                    self.out.push_str(&escape_html(text));
                    self.out.push_str("</code>");
                }
                Node::InlineHtml(raw) => self.out.push_str(raw),
                Node::SoftBreak => self.out.push('\n'),
                Node::HardBreak => self.out.push_str("<br />\n"),
                Node::Emphasis => self.container(child, "<em>", "</em>"),
                Node::Strong => self.container(child, "<strong>", "</strong>"),
                Node::Link(data) if data.image => {
                    self.out.push_str("<img src=\"");
                    if !data.url.is_empty() && !is_dangerous_url(&data.url) {
                        self.out.push_str(&escape_href(&data.url));
                    }
                    self.out.push_str("\" alt=\"");
                    self.out.push_str(&escape_html(&self.plain_text(child)));
                    if let Some(title) = &data.title {
                        self.out.push_str("\" title=\"");
                        self.out.push_str(&escape_html(title));
                    }
                    self.out.push_str("\" />");
                }
                Node::Link(data) => {
                    self.out.push_str("<a href=\"");
                    if !data.url.is_empty() && !is_dangerous_url(&data.url) {
                        self.out.push_str(&escape_href(&data.url));
                    }
                    if let Some(title) = &data.title {
                        self.out.push_str("\" title=\"");
                        self.out.push_str(&escape_html(title));
                    }
                    self.out.push_str("\">");
                    self.inline(child);
                    self.out.push_str("</a>");
                }
                _ => {}
            }
        }
    }

    /// Renders an inline container's children between two tags.
    fn container(&mut self, id: NodeId, open: &str, close: &str) {
        self.out.push_str(open);
        self.inline(id);
        self.out.push_str(close);
    }

    /// The plain text of an inline subtree, as image alt text.
    fn plain_text(&self, id: NodeId) -> String {
        let mut out = String::new();
        for child in self.arena.children(id) {
            let child = *child;
            match self.arena.kind(child) {
                Node::Text(text) | Node::CodeSpan(text) | Node::InlineHtml(text) => {
                    out.push_str(text)
                }
                Node::SoftBreak | Node::HardBreak => out.push(' '),
                _ => out.push_str(&self.plain_text(child)),
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
