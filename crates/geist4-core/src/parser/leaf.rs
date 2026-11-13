//! Hand-written scanners for block-start and end conditions.
//!
//! No regular expressions: every check here walks bytes directly, so
//! running time is predictable.

use super::cursor::Cursor;

/// The block-level HTML tag names that start an HTML block of kind 6.
const BLOCK_TAGS: [&str; 62] = [
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// A list marker found at the start of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ListMarker {
    /// Whether the marker is ordered (`1.`) rather than a bullet (`-`).
    pub ordered: bool,
    /// The marker character: `-`, `+`, `*`, or the ordered delimiter `.`/`)`.
    pub char: u8,
    /// The width of the marker in bytes.
    pub width: usize,
    /// The start number, for ordered markers.
    pub start: u64,
}

/// A fenced code block start found at the start of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FenceStart {
    /// The fence character (backtick or tilde).
    pub ch: u8,
    /// The number of fence characters.
    pub len: usize,
}

/// Which of the seven HTML block kinds a line starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HtmlStart {
    /// The kind, 1 to 7.
    pub kind: u8,
}

/// Whether the line at the cursor (positioned at its first non-space byte)
/// is a thematic break: three or more `-`, `*` or `_` characters with only
/// spaces and tabs between them.
pub(super) fn thematic_break(c: &Cursor) -> bool {
    let Some(first) = c.peek() else {
        return false;
    };
    if !matches!(first, b'-' | b'*' | b'_') {
        return false;
    }
    let mut count = 0usize;
    let mut pos = c.pos();
    let line = c.line();
    while let Some(&b) = line.get(pos) {
        if b == first {
            count += 1;
            pos += 1;
        } else if b == b' ' || b == b'\t' {
            pos += 1;
        } else {
            return false;
        }
    }
    count >= 3
}

/// Whether the line at the cursor is a setext heading underline, and which
/// level it makes: `=` gives level 1, `-` gives level 2.
pub(super) fn setext_underline(c: &Cursor) -> Option<u8> {
    let first = c.peek()?;
    if first != b'=' && first != b'-' {
        return None;
    }
    let mut pos = c.pos();
    let line = c.line();
    while line.get(pos) == Some(&first) {
        pos += 1;
    }
    while matches!(line.get(pos), Some(b' ') | Some(b'\t')) {
        pos += 1;
    }
    if pos >= line.len() {
        Some(if first == b'=' { 1 } else { 2 })
    } else {
        None
    }
}

/// Scans an ATX heading at the cursor: one to six `#` characters followed by
/// a space, a tab, or the end of the line. Returns the level and the heading
/// text with any closing hash run removed.
pub(super) fn atx_heading(c: &Cursor) -> Option<(u8, String)> {
    let line = c.line();
    let mut pos = c.pos();
    let mut level = 0u8;
    while line.get(pos) == Some(&b'#') {
        level += 1;
        pos += 1;
    }
    if level == 0 || level > 6 {
        return None;
    }
    match line.get(pos) {
        None => {}
        Some(b' ') | Some(b'\t') => {
            pos += 1;
            while matches!(line.get(pos), Some(b' ') | Some(b'\t')) {
                pos += 1;
            }
        }
        Some(_) => return None,
    }
    let mut content: &[u8] = &line[pos..];
    // Strip a closing run of `#` that is preceded by a space or tab, or that
    // makes up the whole content.
    let mut end = content.len();
    while end > 0 && (content[end - 1] == b' ' || content[end - 1] == b'\t') {
        end -= 1;
    }
    let mut hash_end = end;
    while hash_end > 0 && content[hash_end - 1] == b'#' {
        hash_end -= 1;
    }
    if hash_end < end {
        let preceded_by_space =
            hash_end > 0 && (content[hash_end - 1] == b' ' || content[hash_end - 1] == b'\t');
        if hash_end == 0 || preceded_by_space {
            content = &content[..hash_end];
        }
    }
    let mut text = String::from_utf8_lossy(content).into_owned();
    while text.ends_with(' ') || text.ends_with('\t') {
        text.pop();
    }
    Some((level, text))
}

/// Scans a fenced code block start at the cursor: three or more backticks or
/// tildes. For backtick fences the info string must not contain a backtick.
pub(super) fn fence_start(c: &Cursor) -> Option<FenceStart> {
    let first = c.peek()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let line = c.line();
    let mut pos = c.pos();
    while line.get(pos) == Some(&first) {
        pos += 1;
    }
    let len = pos - c.pos();
    if len < 3 {
        return None;
    }
    let info_bytes = &line[pos..];
    if first == b'`' && info_bytes.contains(&b'`') {
        return None;
    }
    let trimmed = trim_ascii(info_bytes);
    if first == b'`' && trimmed.contains(&b'`') {
        return None;
    }
    Some(FenceStart { ch: first, len })
}

/// Checks for a fenced code block closing fence at the cursor, given the
/// opening fence's character and length. The cursor may carry up to three
/// columns of indentation before the fence.
pub(super) fn fence_close(c: &Cursor, ch: u8, len: usize) -> bool {
    let (pos, col) = c.find_nonspace();
    if col - c.col() > 3 {
        return false;
    }
    let line = c.line();
    let mut p = pos;
    let mut run = 0usize;
    while line.get(p) == Some(&ch) {
        run += 1;
        p += 1;
    }
    if run < len {
        return false;
    }
    while matches!(line.get(p), Some(b' ') | Some(b'\t')) {
        p += 1;
    }
    p >= line.len()
}

/// Scans an HTML block start at the cursor. `allow_kind7` is false when the
/// line might be a lazy continuation or interrupt a paragraph, which rules
/// out kind 7.
pub(super) fn html_block_start(c: &Cursor, allow_kind7: bool) -> Option<HtmlStart> {
    let line = c.line();
    let start = c.pos();
    if line.get(start) != Some(&b'<') {
        return None;
    }
    let rest = &line[start..];
    let lower = rest.to_ascii_lowercase();
    // Kind 1: <pre, <script, <style, <textarea plus space, tab, > or EOL.
    for tag in ["<pre", "<script", "<style", "<textarea"] {
        if lower.starts_with(tag.as_bytes()) {
            let after = tag.len();
            match rest.get(after) {
                None => return Some(HtmlStart { kind: 1 }),
                Some(b' ') | Some(b'\t') | Some(b'>') => return Some(HtmlStart { kind: 1 }),
                Some(_) => {}
            }
        }
    }
    if rest.starts_with(b"<!--") {
        return Some(HtmlStart { kind: 2 });
    }
    if rest.starts_with(b"<?") {
        return Some(HtmlStart { kind: 3 });
    }
    if rest.starts_with(b"<![CDATA[") {
        return Some(HtmlStart { kind: 5 });
    }
    if rest.starts_with(b"<!") {
        if let Some(&b) = rest.get(2) {
            if b.is_ascii_alphabetic() {
                return Some(HtmlStart { kind: 4 });
            }
        }
    }
    // Kind 6: `<` or `</` plus a block tag name.
    if block_tag_at(&rest[1..]).is_some()
        || (rest.starts_with(b"</") && block_tag_at(&rest[2..]).is_some())
    {
        return Some(HtmlStart { kind: 6 });
    }
    // Kind 7: a complete open or closing tag alone on the line.
    if !allow_kind7 {
        return None;
    }
    if complete_tag_alone(rest) {
        return Some(HtmlStart { kind: 7 });
    }
    None
}

/// Checks whether the bytes after `<` or `</` name a block tag and are
/// followed by a space, a tab, `>`, `/>`, or the end of the line.
fn block_tag_at(rest: &[u8]) -> Option<u8> {
    let mut name_end = 0usize;
    while name_end < rest.len() && rest[name_end].is_ascii_alphanumeric() {
        name_end += 1;
    }
    if name_end == 0 {
        return None;
    }
    let name = rest[..name_end].to_ascii_lowercase();
    if !BLOCK_TAGS.iter().any(|tag| tag.as_bytes() == &name[..]) {
        return None;
    }
    match rest.get(name_end) {
        None => Some(6),
        Some(b' ') | Some(b'\t') | Some(b'>') => Some(6),
        Some(b'/') if rest.get(name_end + 1) == Some(&b'>') => Some(6),
        Some(_) => None,
    }
}

/// Whether `rest` is exactly a complete open tag or closing tag followed by
/// only spaces and tabs to the end of the line.
fn complete_tag_alone(rest: &[u8]) -> bool {
    let consumed = if rest.starts_with(b"</") {
        closing_tag_len(rest)
    } else {
        open_tag_len(rest)
    };
    let Some(consumed) = consumed else {
        return false;
    };
    rest[consumed..].iter().all(|&b| b == b' ' || b == b'\t')
}

/// The length of a closing tag (`</name>`) at the start of `rest`.
fn closing_tag_len(rest: &[u8]) -> Option<usize> {
    if !rest.starts_with(b"</") {
        return None;
    }
    let mut pos = 2usize;
    let name_len = tag_name_len(&rest[pos..]);
    if name_len == 0 {
        return None;
    }
    pos += name_len;
    while matches!(rest.get(pos), Some(b' ') | Some(b'\t')) {
        pos += 1;
    }
    if rest.get(pos) == Some(&b'>') {
        Some(pos + 1)
    } else {
        None
    }
}

/// The length of an open tag (`<name attributes>`) at the start of `rest`.
fn open_tag_len(rest: &[u8]) -> Option<usize> {
    if rest.first() != Some(&b'<') {
        return None;
    }
    let mut pos = 1usize;
    let name_len = tag_name_len(&rest[pos..]);
    if name_len == 0 {
        return None;
    }
    pos += name_len;
    loop {
        let ws_start = pos;
        while matches!(rest.get(pos), Some(b' ') | Some(b'\t')) {
            pos += 1;
        }
        if rest.get(pos) == Some(&b'>') {
            return Some(pos + 1);
        }
        if rest.get(pos) == Some(&b'/') && rest.get(pos + 1) == Some(&b'>') {
            return Some(pos + 2);
        }
        if pos == ws_start || !attribute_len(rest, pos) {
            // No attribute here: the tag must close now or it is invalid.
            return None;
        }
        // Advance past the attribute; `attribute_len` reported validity, so
        // rescan to find the new position.
        pos = skip_attribute(rest, pos)?;
    }
}

/// The length of a tag name: an ASCII letter, then letters, digits or `-`.
fn tag_name_len(rest: &[u8]) -> usize {
    if rest.first().is_none_or(|b| !b.is_ascii_alphabetic()) {
        return 0;
    }
    let mut len = 1usize;
    while rest
        .get(len)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-')
    {
        len += 1;
    }
    len
}

/// Whether an attribute starts at `pos` (an attribute name, optionally
/// followed by `=` and a value).
fn attribute_len(rest: &[u8], pos: usize) -> bool {
    match rest.get(pos) {
        Some(b) if b.is_ascii_alphabetic() || *b == b'_' || *b == b':' => {}
        _ => return false,
    }
    let mut p = pos + 1;
    while rest
        .get(p)
        .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
    {
        p += 1;
    }
    // Optional =value.
    let after_name = p;
    while matches!(rest.get(p), Some(b' ') | Some(b'\t')) {
        p += 1;
    }
    if rest.get(p) == Some(&b'=') {
        p += 1;
        while matches!(rest.get(p), Some(b' ') | Some(b'\t')) {
            p += 1;
        }
        match rest.get(p) {
            Some(b'\'') => {
                p += 1;
                while rest.get(p).is_some_and(|&b| b != b'\'') {
                    p += 1;
                }
                if rest.get(p) != Some(&b'\'') {
                    return false;
                }
            }
            Some(b'"') => {
                p += 1;
                while rest.get(p).is_some_and(|&b| b != b'"') {
                    p += 1;
                }
                if rest.get(p) != Some(&b'"') {
                    return false;
                }
            }
            Some(_) => {
                while rest.get(p).is_some_and(|&b| {
                    !matches!(b, b' ' | b'\t' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
                }) {
                    p += 1;
                }
                if p == after_name + 1 {
                    // The `=` with an empty value is still a value.
                }
            }
            None => return false,
        }
    }
    true
}

/// Skips one attribute at `pos` and returns the position after it.
fn skip_attribute(rest: &[u8], pos: usize) -> Option<usize> {
    let mut p = pos + 1;
    while rest
        .get(p)
        .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
    {
        p += 1;
    }
    let after_name = p;
    while matches!(rest.get(p), Some(b' ') | Some(b'\t')) {
        p += 1;
    }
    if rest.get(p) == Some(&b'=') {
        p += 1;
        while matches!(rest.get(p), Some(b' ') | Some(b'\t')) {
            p += 1;
        }
        match rest.get(p) {
            Some(b'\'') => {
                p += 1;
                while rest.get(p).is_some_and(|&b| b != b'\'') {
                    p += 1;
                }
                p += 1;
            }
            Some(b'"') => {
                p += 1;
                while rest.get(p).is_some_and(|&b| b != b'"') {
                    p += 1;
                }
                p += 1;
            }
            Some(_) => {
                while rest.get(p).is_some_and(|&b| {
                    !matches!(b, b' ' | b'\t' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
                }) {
                    p += 1;
                }
            }
            None => return None,
        }
        let _ = after_name;
    }
    Some(p)
}

/// Whether the line at the cursor is an HTML block end condition for `kind`.
/// Only kinds 1 to 5 end this way; kinds 6 and 7 end at a blank line.
pub(super) fn html_block_end(kind: u8, line: &[u8]) -> bool {
    let lower = line.to_ascii_lowercase();
    match kind {
        1 => ["</pre>", "</script>", "</style>", "</textarea>"]
            .iter()
            .any(|tag| contains_substring(&lower, tag.as_bytes())),
        2 => contains_substring(&lower, b"-->"),
        3 => contains_substring(&lower, b"?>"),
        4 => lower.contains(&b'>'),
        5 => contains_substring(&lower, b"]]>"),
        _ => false,
    }
}

/// Whether `haystack` contains `needle`.
fn contains_substring(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Scans a list marker at the cursor. The marker must be followed by a
/// space, a tab, or the end of the line.
pub(super) fn list_marker(c: &Cursor) -> Option<ListMarker> {
    let line = c.line();
    let pos = c.pos();
    let first = line.get(pos).copied()?;
    if matches!(first, b'-' | b'+') {
        // A bullet marker may not be followed by a digit, so that `-1.` is
        // not a list item.
        if line.get(pos + 1).is_some_and(|b| b.is_ascii_digit()) {
            return None;
        }
        return Some(ListMarker {
            ordered: false,
            char: first,
            width: 1,
            start: 0,
        });
    }
    if first == b'*' {
        return Some(ListMarker {
            ordered: false,
            char: first,
            width: 1,
            start: 0,
        });
    }
    if !first.is_ascii_digit() {
        return None;
    }
    let mut digits = 0usize;
    let mut value: u64 = 0;
    while line.get(pos + digits).is_some_and(|b| b.is_ascii_digit()) {
        value = value * 10 + u64::from(line[pos + digits] - b'0');
        digits += 1;
        if digits > 9 {
            return None;
        }
    }
    let delimiter = line.get(pos + digits).copied()?;
    if delimiter != b'.' && delimiter != b')' {
        return None;
    }
    Some(ListMarker {
        ordered: true,
        char: delimiter,
        width: digits + 1,
        start: value,
    })
}

/// Whether the marker at the cursor is followed by a space, a tab, or the
/// end of the line, which a list marker requires.
pub(super) fn marker_followed_by_space(c: &Cursor, width: usize) -> bool {
    match c.peek_at(width) {
        None => true,
        Some(b' ') | Some(b'\t') => true,
        Some(_) => false,
    }
}

/// Trims ASCII spaces and tabs from both ends.
pub(super) fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && (bytes[start] == b' ' || bytes[start] == b'\t') {
        start += 1;
    }
    while end > start && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\t') {
        end -= 1;
    }
    &bytes[start..end]
}

/// One link reference definition found by [`scan_link_def`].
pub(super) struct ScannedDef {
    /// The label between the brackets.
    pub label: String,
    /// The destination, with backslash escapes resolved.
    pub destination: String,
    /// The optional title, with backslash escapes resolved.
    pub title: Option<String>,
    /// How many bytes of `text` the definition consumed, through the end of
    /// the line the definition ends on.
    pub consumed: usize,
}

/// Scans one link reference definition at the start of `text`.
///
/// `text` holds paragraph content: each line's leading whitespace is already
/// stripped and the lines are joined with `\n`, so a blank line can never
/// appear and the parts may be separated by at most one newline.
pub(super) fn scan_link_def(text: &str) -> Option<ScannedDef> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    // The label: to the first unescaped `]`, with no unescaped `[` inside.
    let mut i = 1usize;
    let mut label_end = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += escape_step(bytes, i),
            b'[' => return None,
            b']' => {
                label_end = Some(i);
                break;
            }
            _ => i += 1,
        }
    }
    let label_end = label_end?;
    if label_end == 1 {
        // The label may not be empty.
        return None;
    }
    // The label is trimmed; a label that trims to nothing is invalid.
    let mut ls = 1usize;
    let mut le = label_end;
    while ls < le && bytes[ls].is_ascii_whitespace() {
        ls += 1;
    }
    while le > ls && bytes[le - 1].is_ascii_whitespace() {
        le -= 1;
    }
    if le == ls {
        return None;
    }
    let label = text[ls..le].to_string();
    let mut i = label_end + 1;
    if bytes.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    i = scan_ws_gap(bytes, i);
    let (destination, dest_len) = scan_destination(&text[i..])?;
    i += dest_len;
    // After the destination, the reference algorithm's sequence is: spaces
    // and tabs, at most one newline, spaces and tabs; then an optional
    // title; then spaces and a line end. When the direct end check fails
    // but a title was scanned, the scan rewinds before the title and
    // requires the destination's own line to end: the title stays captured
    // while the title's line remains paragraph text.
    let mut j = i;
    while matches!(bytes.get(j), Some(b' ') | Some(b'\t')) {
        j += 1;
    }
    let mut advanced = j > i;
    if bytes.get(j) == Some(&b'\n') {
        j += 1;
        advanced = true;
        while matches!(bytes.get(j), Some(b' ') | Some(b'\t')) {
            j += 1;
        }
    }
    let mut title: Option<(String, usize)> = None;
    if advanced {
        match scan_title(&text[j..]) {
            Some((found, len)) => {
                title = Some((found, len));
                j += len;
            }
            // A failed title scan rewinds to right after the destination.
            None => j = i,
        }
    }
    // Spaces, then a line end or the end of the text, must follow.
    let end_ok = |mut k: usize| -> Option<usize> {
        while matches!(bytes.get(k), Some(b' ') | Some(b'\t')) {
            k += 1;
        }
        match bytes.get(k) {
            None => Some(k),
            Some(b'\n') => Some(k + 1),
            Some(_) => None,
        }
    };
    if let Some(consumed) = end_ok(j) {
        return Some(ScannedDef {
            label,
            destination,
            title: title.map(|(found, _)| found),
            consumed,
        });
    }
    if title.is_some() {
        if let Some(consumed) = end_ok(i) {
            return Some(ScannedDef {
                label,
                destination,
                title: title.map(|(found, _)| found),
                consumed,
            });
        }
    }
    None
}

/// Skips the whitespace gap between definition parts: spaces and tabs, plus
/// at most one newline with more spaces and tabs after it.
fn scan_ws_gap(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b' ') | Some(b'\t')) {
        i += 1;
    }
    if bytes.get(i) == Some(&b'\n') {
        i += 1;
        while matches!(bytes.get(i), Some(b' ') | Some(b'\t')) {
            i += 1;
        }
    }
    i
}

/// Scans a link destination: an angle-bracket form or a bare form. Returns
/// the unescaped destination and its byte length.
fn scan_destination(text: &str) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.first() == Some(&b'<') {
        let mut i = 1usize;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += escape_step(bytes, i),
                b'>' => return Some((String::from_utf8_lossy(&bytes[1..i]).into_owned(), i + 1)),
                b'<' | b'\n' => return None,
                b if b.is_ascii_control() => return None,
                _ => i += 1,
            }
        }
        return None;
    }
    let mut i = 0usize;
    let mut depth = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += escape_step(bytes, i),
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                i += 1;
            }
            b' ' | b'\t' | b'\n' => break,
            b if b.is_ascii_control() => break,
            _ => i += 1,
        }
    }
    if i == 0 || depth != 0 {
        return None;
    }
    Some((String::from_utf8_lossy(&bytes[..i]).into_owned(), i))
}

/// Scans a link title in double quotes, single quotes or parentheses.
/// Returns the unescaped title and its byte length.
fn scan_title(text: &str) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let close = match bytes.first()? {
        b'"' => b'"',
        b'\'' => b'\'',
        b'(' => b')',
        _ => return None,
    };
    let mut i = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += escape_step(bytes, i),
            b if b == close => {
                // The quoted span is returned whole, outer quotes
                // included: the caller strips exactly one pair, so
                // title content that itself starts and ends with the
                // same quote character is preserved.
                return Some((String::from_utf8_lossy(&bytes[..=i]).into_owned(), i + 1));
            }
            _ => i += 1,
        }
    }
    None
}

/// How far to skip at a backslash: two bytes for an escaped ASCII
/// punctuation character, one byte otherwise.
fn escape_step(bytes: &[u8], at: usize) -> usize {
    if bytes.get(at + 1).is_some_and(|b| b.is_ascii_punctuation()) {
        2
    } else {
        1
    }
}

/// Resolves backslash escapes: `\x` for ASCII punctuation `x` becomes `x`,
/// and any other backslash stays.
pub(super) fn unescape(raw: &[u8]) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut i = 0usize;
    while i < raw.len() {
        if raw[i] == b'\\' && raw.get(i + 1).is_some_and(|b| b.is_ascii_punctuation()) {
            out.push(raw[i + 1]);
            i += 2;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
