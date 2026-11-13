//! HTML entity scanning for the inline phase: numeric and named
//! character references. The name and code point tables live in the
//! generated `entity_table` module (see `tools/gen_entities.py`).

use super::entity_table::{ENTITY_CODEPOINTS, ENTITY_NAMES};

/// The longest entity name, limiting the scan for the semicolon.
const MAX_NAME_LENGTH: usize = 32;

/// Decodes one entity at the start of `src`, which directly follows an
/// `&`. Returns the decoded text and the bytes consumed, including the
/// terminating semicolon.
pub(crate) fn scan_entity(src: &str) -> Option<(String, usize)> {
    let bytes = src.as_bytes();
    if bytes.first() == Some(&b'#') && bytes.len() >= 3 {
        return scan_numeric_entity(bytes);
    }
    // A named entity: up to 32 characters, then a semicolon. The scan
    // stops at a space, as the reference parser's does.
    let max = bytes.len().min(MAX_NAME_LENGTH);
    for i in 2..max {
        match bytes[i] {
            b';' => {
                let replacement = lookup(&src[..i])?;
                let text: String = replacement
                    .iter()
                    .filter_map(|&c| char::from_u32(c))
                    .collect();
                return Some((text, i + 1));
            }
            b' ' => return None,
            _ => {}
        }
    }
    None
}

/// Decodes a numeric character reference (`&#dd;` or `&#xhh;`).
fn scan_numeric_entity(bytes: &[u8]) -> Option<(String, usize)> {
    let mut codepoint: u32 = 0;
    let mut i = 1usize;

    let (num_digits, max_digits) = if bytes.get(1).is_some_and(|b| b.is_ascii_digit()) {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            codepoint = codepoint
                .saturating_mul(10)
                .saturating_add(u32::from(bytes[i] - b'0'));
            if codepoint >= 0x110_000 {
                codepoint = 0x110_000;
            }
            i += 1;
        }
        (i - 1, 7)
    } else if bytes.get(1) == Some(&b'x') || bytes.get(1) == Some(&b'X') {
        i = 2;
        while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
            codepoint = codepoint
                .saturating_mul(16)
                .saturating_add(hex_value(bytes[i]));
            if codepoint >= 0x110_000 {
                codepoint = 0x110_000;
            }
            i += 1;
        }
        (i - 2, 6)
    } else {
        return None;
    };
    if (1..=max_digits).contains(&num_digits) && bytes.get(i) == Some(&b';') {
        // Zero, surrogates and out-of-range values become U+FFFD.
        let cp =
            if codepoint == 0 || (0xD800..0xE000).contains(&codepoint) || codepoint >= 0x110_000 {
                0xFFFD
            } else {
                codepoint
            };
        let text = char::from_u32(cp).unwrap_or('\u{FFFD}').to_string();
        return Some((text, i + 1));
    }
    None
}

/// The value of one ASCII hex digit.
fn hex_value(b: u8) -> u32 {
    match b {
        b'0'..=b'9' => u32::from(b - b'0'),
        _ => u32::from((b | 32) - b'a' + 10),
    }
}

/// Looks up a named entity by its exact name.
fn lookup(name: &str) -> Option<&'static [u32]> {
    let index = ENTITY_NAMES.binary_search(&name).ok()?;
    Some(ENTITY_CODEPOINTS[index])
}

/// Decodes every entity in `text`, leaving unparseable `&` characters
/// alone.
pub(crate) fn unescape_html_f(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            match scan_entity(&text[i + 1..]) {
                Some((replacement, consumed)) => {
                    out.push_str(&replacement);
                    i += 1 + consumed;
                }
                None => {
                    out.push('&');
                    i += 1;
                }
            }
        } else {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] & 0xC0 == 0x80 {
                i += 1;
            }
            out.push_str(&text[start..i]);
        }
    }
    out
}
