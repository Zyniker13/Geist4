"""Generates crates/geist4-core/src/inline/unicode.rs.

The module carries the Unicode classification the emphasis algorithm and
the scanners need: whitespace (Zs/Zl/Zp plus tab, LF, FF, CR) and
punctuation-or-symbol (P*/S*) code point ranges, from the Unicode
character database via the Python standard library's unicodedata.

Run from the repository root: python3 tools/gen_unicode.py
"""

import unicodedata

OUT = "crates/geist4-core/src/inline/unicode.rs"


def ranges_for(pred):
    out = []
    start = None
    for cp in range(0x110000):
        if pred(chr(cp)):
            if start is None:
                start = cp
        else:
            if start is not None:
                out.append((start, cp - 1))
                start = None
    if start is not None:
        out.append((start, 0x10FFFF))
    return out


def is_space(c):
    return unicodedata.category(c) in ("Zs", "Zl", "Zp") or c in "\t\n\f\r "


def is_punct_or_symbol(c):
    return unicodedata.category(c)[0] in ("P", "S")


ws = ranges_for(is_space)
ps = ranges_for(is_punct_or_symbol)

header = '''//! Unicode classification data and helpers for the inline phase.
//!
//! Generated from the Unicode character database (general categories);
//! regenerate with the repository's table generator rather than editing.

/// Whether a code point falls in a sorted, non-overlapping range table.
fn in_ranges(ranges: &[(u32, u32)], cp: u32) -> bool {
    let mut low = 0usize;
    let mut high = ranges.len();
    while low < high {
        let mid = low + (high - low) / 2;
        let (start, end) = ranges[mid];
        if cp < start {
            high = mid;
        } else if cp > end {
            low = mid + 1;
        } else {
            return true;
        }
    }
    false
}

/// Whether the character counts as whitespace for emphasis flanking:
/// Unicode Zs, Zl and Zp plus tab, newline, form feed and carriage return.
pub(crate) fn is_space(c: char) -> bool {
    in_ranges(&SPACE_RANGES, c as u32)
}

/// Whether the character is Unicode punctuation (P*) or a symbol (S*),
/// the classes that count as punctuation for emphasis flanking.
pub(crate) fn is_punct_or_symbol(c: char) -> bool {
    in_ranges(&PUNCT_OR_SYMBOL_RANGES, c as u32)
}

/// Whether an ASCII byte is punctuation in the escape sense: any ASCII
/// punctuation character.
pub(crate) fn is_ascii_punct_byte(b: u8) -> bool {
    matches!(b,
        b'!' | b'"' | b'#' | b'$' | b'%' | b'&' | b'\\'' | b'(' | b')'
        | b'*' | b'+' | b',' | b'-' | b'.' | b'/' | b':' | b';' | b'<'
        | b'=' | b'>' | b'?' | b'@' | b'[' | b'\\\\' | b']' | b'^' | b'_'
        | b'`' | b'{' | b'|' | b'}' | b'~')
}
'''

parts = [header]

for const_name, doc, pairs in (
    (
        "PUNCT_OR_SYMBOL_RANGES",
        "Code point ranges for Unicode punctuation (P*) and symbol (S*)\ngeneral categories.",
        ps,
    ),
    (
        "SPACE_RANGES",
        "Code point ranges for characters the reference algorithm treats as\nwhitespace: Unicode Zs, Zl and Zp plus tab, newline, form feed and\ncarriage return.",
        ws,
    ),
):
    parts.append("\n".join("/// " + line for line in doc.splitlines()))
    parts.append(f"const {const_name}: [(u32, u32); {len(pairs)}] = [")
    for a, b in pairs:
        parts.append(f"    (0x{a:X}, 0x{b:X}),")
    parts.append("];")
    parts.append("")

with open(OUT, "w") as f:
    f.write("\n".join(parts))

print(f"ws={len(ws)} ps={len(ps)}")
