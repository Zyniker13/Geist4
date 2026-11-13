//! The GFM autolinks built-in extension: bare `www` domains, full URLs
//! with a safe scheme, and email addresses become links.
//!
//! The `w` and `:` matchers run at those trigger bytes and scan the raw
//! subject to the end of the line, swallowing every byte they accept —
//! including ordinary characters that would otherwise start emphasis,
//! code spans or entities, which the specification's examples show
//! inside matched URLs. The email matcher is a post-process pass over
//! consolidated text nodes. All three port the reference's
//! `extensions/autolink.c` exactly, including its punctuation trimming
//! and its guards against pathological input.

use geist4_core::arena::NodeId;
use geist4_core::extension::{Extension, InlineTriggerCtx, TreeCtx};
use geist4_core::node::{LinkData, Node};
use geist4_core::unicode::{is_punct_or_symbol, is_space};
use std::borrow::Cow;

/// The GFM autolinks extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Autolink;

/// The autolink extension's name.
pub const NAME: &str = "autolink";

/// ASCII whitespace, the class the matchers' byte tests use
/// (`cmark_isspace`: space, tab, newline, carriage return — vertical
/// tab and form feed are ordinary bytes to it).
fn is_space_byte(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Decodes the UTF-8 character at the start of `data`, as the
/// reference's `cmark_utf8proc_iterate` does over a valid subject.
/// Returns `None` where the iterate reports an encoding error, which
/// counts as an invalid host character.
fn decode_char(data: &[u8]) -> Option<(char, usize)> {
    let first = *data.first()?;
    let len = match first {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    if data.len() < len {
        return None;
    }
    let text = std::str::from_utf8(&data[..len]).ok()?;
    let ch = text.chars().next()?;
    Some((ch, len))
}

/// Whether the character at the start of `data` counts as a valid host
/// character: neither whitespace nor punctuation nor a symbol
/// (`is_valid_hostchar`).
fn is_valid_hostchar(data: &[u8]) -> bool {
    match decode_char(data) {
        Some((ch, _)) => !is_space(ch) && !is_punct_or_symbol(ch),
        None => false,
    }
}

/// Whether the URL starts with a safe scheme followed by a valid host
/// character: http, https or ftp, compared case-insensitively as the
/// reference's `strncasecmp` (`sd_autolink_issafe`).
fn sd_autolink_issafe(url: &[u8]) -> bool {
    for scheme in [
        b"http://".as_slice(),
        b"https://".as_slice(),
        b"ftp://".as_slice(),
    ] {
        if url.len() > scheme.len()
            && url[..scheme.len()].eq_ignore_ascii_case(scheme)
            && is_valid_hostchar(&url[scheme.len()..])
        {
            return true;
        }
    }
    false
}

/// The reference's `check_domain`: how much of the slice's start is a
/// domain body. Tracks the underscore rule that rejects URLs with an
/// underscore in one of their last two segments — unless the URL has
/// more than ten segments, where the reference accepts anyway to keep
/// the check linear against denial of service. `allow_short` selects
/// the scheme-URL form, whose host needs no dot.
///
/// The scan stops one byte before the end on purpose; the caller
/// extends the match to the line end afterwards, so the excluded byte
/// is covered there. A one-byte slice returns zero, where the
/// reference reads past its buffer; no input reaches that path, and
/// declining keeps the Rust port memory-safe.
fn check_domain(data: &[u8], allow_short: bool) -> usize {
    let size = data.len();
    let mut np = 0usize;
    let mut uscore1 = 0usize;
    let mut uscore2 = 0usize;
    let mut i = 1usize;
    while i < size.saturating_sub(1) {
        if data[i] == b'\\' && i < size - 2 {
            i += 1;
        }
        if data[i] == b'_' {
            uscore2 += 1;
        } else if data[i] == b'.' {
            uscore1 = uscore2;
            uscore2 = 0;
            np += 1;
        } else if !is_valid_hostchar(&data[i..]) && data[i] != b'-' {
            break;
        }
        i += 1;
    }

    if (uscore1 > 0 || uscore2 > 0) && np <= 10 {
        return 0;
    }

    if allow_short {
        // A scheme URL's host needs no dot; the length of the valid
        // sequence is enough.
        if size <= 1 {
            return 0;
        }
        return i;
    }
    // A valid domain needs at least a dot.
    if np > 0 && size > 1 { i } else { 0 }
}

/// The reference's `autolink_delim`: trims the matched end. Balanced
/// trailing parentheses stay; each unbalanced closer costs one
/// character; the sentence punctuation `?!,.:*_'";` is cut; and a
/// trailing `&name;` entity is cut whole. A `<` inside the match ends
/// it there.
fn autolink_delim(data: &[u8], mut link_end: usize) -> usize {
    let mut closing = 0usize;
    let mut opening = 0usize;

    for (i, byte) in data.iter().enumerate().take(link_end) {
        match byte {
            b'<' => {
                link_end = i;
                break;
            }
            b'(' => opening += 1,
            b')' => closing += 1,
            _ => {}
        }
    }

    while link_end > 0 {
        match data[link_end - 1] {
            b')' => {
                // Any number of matching brackets at the end stays;
                // each extra closer costs one character.
                if closing <= opening {
                    return link_end;
                }
                closing -= 1;
                link_end -= 1;
            }
            b'?' | b'!' | b'.' | b',' | b':' | b'*' | b'_' | b'~' | b'\'' | b'"' => {
                link_end -= 1;
            }
            b';' => {
                let mut new_end = link_end - 2;
                while new_end > 0 && data[new_end].is_ascii_alphabetic() {
                    new_end -= 1;
                }
                if new_end < link_end - 2 && data[new_end] == b'&' {
                    link_end = new_end;
                } else {
                    link_end -= 1;
                }
            }
            _ => return link_end,
        }
    }

    link_end
}

/// The `w` trigger: matches `www.` domains, links them as `http://...`
/// and extends the match to the end of the line (`www_match`).
fn www_match(ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
    // The match may start at the subject's beginning, after whitespace,
    // or after any of the characters the reference scans specially;
    // otherwise it declines. A newline before the trigger passes, as
    // the reference's NUL-terminated line buffers do.
    match ctx.preceding() {
        None => {}
        Some(b'*' | b'_' | b'~' | b'(') => {}
        Some(byte) if is_space_byte(byte) => {}
        _ => return,
    }

    let data = ctx.rest_of_line();
    if data.len() < 4 || !data.starts_with(b"www.") {
        return;
    }

    let mut link_end = check_domain(data, false);
    if link_end == 0 {
        return;
    }

    while link_end < data.len() && !is_space_byte(data[link_end]) && data[link_end] != b'<' {
        link_end += 1;
    }

    link_end = autolink_delim(data, link_end);
    if link_end == 0 {
        return;
    }

    let url = format!("http://{}", String::from_utf8_lossy(&data[..link_end]));
    ctx.emit_link(0, link_end, url);
}

/// The `:` trigger: matches a scheme URL whose rewind crosses only
/// ASCII letters and whose scheme is one the reference accepts
/// (`url_match`).
fn url_match(ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
    let data = ctx.rest_of_line();
    if data.len() < 4 || data[1] != b'/' || data[2] != b'/' {
        return;
    }

    let before = ctx.line_prefix();
    let max_rewind = before.len();
    let mut rewind = 0usize;
    while rewind < max_rewind && before[max_rewind - rewind - 1].is_ascii_alphabetic() {
        rewind += 1;
    }

    let mut full = Vec::with_capacity(rewind + data.len());
    full.extend_from_slice(&before[max_rewind - rewind..]);
    full.extend_from_slice(data);
    if !sd_autolink_issafe(&full) {
        return;
    }

    let mut link_end = 3; // "://"
    let domain_len = check_domain(&data[link_end..], true);
    if domain_len == 0 {
        return;
    }

    link_end += domain_len;
    while link_end < data.len() && !is_space_byte(data[link_end]) && data[link_end] != b'<' {
        link_end += 1;
    }

    link_end = autolink_delim(data, link_end);
    if link_end == 0 {
        return;
    }

    let url = String::from_utf8_lossy(&full[..rewind + link_end]).into_owned();
    ctx.emit_link(rewind, link_end, url);
}

/// Whether the text before an `@` spells `protocol` and ends exactly
/// at the rewound run (`validate_protocol`). `window` is the candidate
/// window's first byte, `at` the `@`'s index, and `rewind` the length
/// of the run the rewind scan accepted before the colon.
fn validate_protocol(
    protocol: &[u8],
    data: &[u8],
    window: usize,
    at: usize,
    rewind: usize,
) -> bool {
    let len = protocol.len();
    if len > at - rewind - window {
        return false;
    }

    if data[at - rewind - len..at - rewind] != *protocol {
        return false;
    }

    if len == at - rewind - window {
        return true;
    }

    // The character before the protocol must not be alphanumeric.
    let prev = data[at - rewind - len - 1];
    !prev.is_ascii_alphanumeric()
}

/// One found email span: where it starts and ends in the text node and
/// the URL it links to.
struct EmailSpan {
    start: usize,
    end: usize,
    url: String,
}

/// Scans one text node's content for email addresses, the reference's
/// `postprocess_text`, and returns the spans in order. Each span's
/// text is the rewound local part, the `@` and the domain, trimmed the
/// same way the URL matchers trim.
fn scan_emails(content: &[u8]) -> Vec<EmailSpan> {
    let total = content.len();
    let mut spans = Vec::new();
    // `start` is where the unscanned suffix begins; `offset` skips
    // rejected candidates inside it. Their sum plus a candidate's
    // length never passes the node's end, mirroring the reference's
    // `start`/`remaining` walk.
    let mut start = 0usize;
    let mut offset = 0usize;

    loop {
        if offset >= total - start {
            break;
        }

        let Some(at_offset) = content[start + offset..total]
            .iter()
            .position(|byte| *byte == b'@')
        else {
            break;
        };
        let mut max_rewind = at_offset;

        // The reference's `found_at` label: a second `@` inside a
        // candidate restarts here with the new anchor.
        'found_at: loop {
            let window = start + offset;
            let at = window + max_rewind;
            let mut rewind = 0usize;
            let mut auto_mailto = true;
            let mut is_xmpp = false;
            while rewind < max_rewind {
                let byte = content[at - rewind - 1];
                if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-' | b'_') {
                    rewind += 1;
                    continue;
                }
                if byte == b':' {
                    if validate_protocol(b"mailto:", content, window, at, rewind) {
                        auto_mailto = false;
                        rewind += 1;
                        continue;
                    }
                    if validate_protocol(b"xmpp:", content, window, at, rewind) {
                        auto_mailto = false;
                        is_xmpp = true;
                        rewind += 1;
                        continue;
                    }
                }
                break;
            }

            if rewind == 0 {
                offset += max_rewind + 1;
                break 'found_at;
            }

            // The forward scan runs to the node's end; a second `@`
            // restarts with it as the anchor.
            let mut link_end = 1usize;
            let mut np = 0usize;
            let mut retry = false;
            while link_end < total - at {
                let byte = content[at + link_end];
                if byte.is_ascii_alphanumeric() {
                    link_end += 1;
                    continue;
                }
                if byte == b'@' {
                    offset += max_rewind + 1;
                    max_rewind = link_end - 1;
                    retry = true;
                    break;
                } else if byte == b'.'
                    && link_end < total - at - 1
                    && content[at + link_end + 1].is_ascii_alphanumeric()
                {
                    np += 1;
                    link_end += 1;
                } else if byte == b'/' && is_xmpp {
                    link_end += 1;
                } else if byte != b'-' && byte != b'_' {
                    break;
                } else {
                    link_end += 1;
                }
            }

            if retry {
                continue 'found_at;
            }

            let last = content[at + link_end - 1];
            if link_end < 2 || np == 0 || (!last.is_ascii_alphabetic() && last != b'.') {
                offset += max_rewind + link_end;
                break 'found_at;
            }

            let trimmed = autolink_delim(&content[at..], link_end);
            if trimmed == 0 {
                offset += max_rewind + 1;
                break 'found_at;
            }

            let begin = at - rewind;
            let end = at + trimmed;
            let mut url = String::new();
            if auto_mailto {
                url.push_str("mailto:");
            }
            url.push_str(&String::from_utf8_lossy(&content[begin..end]));
            spans.push(EmailSpan {
                start: begin,
                end,
                url,
            });

            start = at + trimmed;
            offset = 0;
            break 'found_at;
        }
    }

    spans
}

impl Autolink {
    /// Merges every run of adjacent text children into the first node,
    /// the reference's `cmark_consolidate_text_nodes` before the email
    /// pass, so a split text run scans as one.
    fn consolidate(&self, ctx: &mut TreeCtx<'_, '_, '_>, id: NodeId) {
        let mut children: Vec<NodeId> = ctx.arena.children(id).to_vec();
        let mut index = 0usize;
        while index + 1 < children.len() {
            let merged = match (
                ctx.arena.kind(children[index]),
                ctx.arena.kind(children[index + 1]),
            ) {
                (Node::Text(a), Node::Text(b)) => {
                    let mut joined = String::with_capacity(a.len() + b.len());
                    joined.push_str(a);
                    joined.push_str(b);
                    Some(joined)
                }
                _ => None,
            };
            if let Some(joined) = merged {
                ctx.arena
                    .set_kind(children[index], Node::Text(Cow::Owned(joined)));
                ctx.arena.remove_child(id, index + 1);
                children.remove(index + 1);
            } else {
                index += 1;
            }
        }
        for child in children {
            self.consolidate(ctx, child);
        }
    }

    /// Scans the tree's text nodes for email addresses, skipping
    /// everything inside a link: an address already in a link stays as
    /// it is. Images are descended into; their alt text renders as
    /// text, so an address there links too.
    fn scan_tree(&self, ctx: &mut TreeCtx<'_, '_, '_>, id: NodeId, inside_link: bool) {
        let in_link = inside_link || matches!(ctx.arena.kind(id), Node::Link(_));
        let children: Vec<NodeId> = ctx.arena.children(id).to_vec();
        for child in children {
            let is_text = matches!(ctx.arena.kind(child), Node::Text(_));
            if is_text && !in_link {
                self.split_text_at_emails(ctx, child, id);
            } else {
                self.scan_tree(ctx, child, in_link);
            }
        }
    }

    /// Replaces one text node with its email matches: the node keeps
    /// the text before the first match, and each match's link plus the
    /// text up to the next one is spliced in after it, the reference's
    /// node splitting in `postprocess_text`. An empty first fragment
    /// stays, as the reference's zero-length chunk does.
    fn split_text_at_emails(&self, ctx: &mut TreeCtx<'_, '_, '_>, id: NodeId, parent: NodeId) {
        let content = match ctx.arena.kind(id) {
            Node::Text(Cow::Borrowed(text)) => (*text).to_owned(),
            Node::Text(Cow::Owned(text)) => text.clone(),
            _ => return,
        };
        let spans = scan_emails(content.as_bytes());
        if spans.is_empty() {
            return;
        }

        let span = ctx.arena.span(id);
        let prefix = content[..spans[0].start].to_owned();
        ctx.arena.set_kind(id, Node::Text(Cow::Owned(prefix)));

        let mut insert_at = ctx
            .arena
            .children(parent)
            .iter()
            .position(|child| *child == id)
            .expect("the parent holds the text node")
            + 1;

        for (index, email) in spans.iter().enumerate() {
            let link = ctx.arena.alloc(
                Node::Link(LinkData {
                    url: Cow::Owned(email.url.clone()),
                    title: None,
                    image: false,
                }),
                span,
            );
            let text = content[email.start..email.end].to_owned();
            let text_id = ctx.arena.alloc(Node::Text(Cow::Owned(text)), span);
            ctx.arena.push_child(link, text_id);
            ctx.arena.insert_child(parent, insert_at, link);
            insert_at += 1;

            let next_start = spans
                .get(index + 1)
                .map_or(content.len(), |next| next.start);
            if email.end < next_start {
                let after = ctx.arena.alloc(
                    Node::Text(Cow::Owned(content[email.end..next_start].to_owned())),
                    span,
                );
                ctx.arena.insert_child(parent, insert_at, after);
                insert_at += 1;
            }
        }
    }
}

impl Extension for Autolink {
    fn name(&self) -> &str {
        NAME
    }

    fn inline_triggers(&self) -> &[u8] {
        // `w` starts a www domain; `:` starts a scheme URL. Email
        // addresses need no trigger: the post-process pass finds them.
        b"w:"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        // Both match kinds decline inside a link or image bracket, so
        // a bare URL in link syntax stays literal.
        if ctx.in_bracket() {
            return;
        }
        match ctx.byte() {
            b':' => url_match(ctx),
            b'w' => www_match(ctx),
            _ => {}
        }
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_>) {
        self.consolidate(ctx, ctx.root);
        self.scan_tree(ctx, ctx.root, false);
    }
}
