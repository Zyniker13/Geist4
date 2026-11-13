//! The GFM disallowed raw HTML filter: raw HTML tags on a fixed
//! blacklist — `title`, `textarea`, `style`, `xmp`, `iframe`,
//! `noembed`, `noframes`, `script`, `plaintext` — have their `<`
//! replaced with `&lt;` on output, in HTML blocks and in inline raw
//! HTML alike. Filtering happens at render time over the literals the
//! core already produced; parsing is untouched.
//!
//! The matcher follows the reference's `is_tag` byte for byte: the
//! literal must start at `<`, an optional `/` allows closing tags, the
//! tag name matches case-insensitively in full, and the byte after it
//! must be whitespace, `>` or the `/>` of a self-closing tag. The
//! boundary whitespace is the reference's `cmark_isspace` class —
//! space, tab, carriage return and line feed; vertical tab and form
//! feed do not count there even though the core treats them as
//! whitespace (decision 0016 covers the class difference).

use geist4_core::extension::Extension;

/// The tags whose raw start tags are filtered.
const BLACKLIST: [&str; 9] = [
    "title",
    "textarea",
    "style",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "script",
    "plaintext",
];

/// The GFM disallowed raw HTML filter extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tagfilter;

impl Extension for Tagfilter {
    fn name(&self) -> &str {
        "tagfilter"
    }

    fn filters_html(&self) -> bool {
        true
    }

    fn html_filter(&self, literal: &[u8]) -> bool {
        BLACKLIST.iter().all(|tag| !starts_with_tag(literal, tag))
    }
}

/// Whether `literal` starts at a `<` that opens a raw tag named `tag`
/// per the reference's `is_tag`.
fn starts_with_tag(literal: &[u8], tag: &str) -> bool {
    if literal.len() < 3 || literal[0] != b'<' {
        return false;
    }
    let mut i = 1;
    if literal[i] == b'/' {
        i += 1;
    }
    let tag = tag.as_bytes();
    for expected in tag {
        let Some(&byte) = literal.get(i) else {
            return false;
        };
        if byte.to_ascii_lowercase() != *expected {
            return false;
        }
        i += 1;
    }
    // The tag name ran out: the next byte decides. A literal that ends
    // right after the name has no boundary byte and is not filtered.
    let Some(&boundary) = literal.get(i) else {
        return false;
    };
    is_boundary_space(boundary)
        || boundary == b'>'
        || (boundary == b'/' && literal.get(i + 1) == Some(&b'>'))
}

/// The reference's `cmark_isspace` as tagfilter sees it: space, tab,
/// carriage return and line feed.
fn is_boundary_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

/// The disallowed raw HTML filter as a shared extension value.
#[must_use]
pub fn tagfilter() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(Tagfilter)
}
