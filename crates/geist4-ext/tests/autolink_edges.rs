//! The autolink matchers' edge branches the spec file does not reach:
//! the host-character decoder, the scheme guard, `check_domain`'s
//! underscore/segment rules, the delimiter trimmer's entity and
//! bracket paths, the trigger decliners, and the email scanner's
//! restart/skip decisions. Spec examples stay in `spec/autolink.md`;
//! these tests call the internals directly where they are `pub` and
//! otherwise drive them through `render_with` on precise inputs.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;

fn autolink_html(src: &str) -> String {
    let mut options = Options::new();
    options.extension(geist4_ext::autolink());
    render_with(&parse_with(src, &options), &options)
}

/// `check_domain`'s guards: a one-byte slice is zero, the
/// allow-short form needs no dot, the non-short form needs one, and
/// more than ten segments accepts an underscore anyway.
#[test]
fn check_domain_rules() {
    // The scan excludes the last byte on purpose; the caller extends
    // to the line end, so a short subject still links.
    assert_eq!(
        autolink_html("www.a"),
        "<p><a href=\"http://www.a\">www.a</a></p>\n",
        "the excluded last byte is added by the line-end extension"
    );
    // A one-byte host declines (check_domain's scan window is empty),
    // but two bytes suffice.
    assert_eq!(
        autolink_html("http://a"),
        "<p>http://a</p>\n",
        "a one-byte host declines"
    );
    assert_eq!(
        autolink_html("http://ab"),
        "<p><a href=\"http://ab\">http://ab</a></p>\n"
    );
    // The www. prefix itself supplies the dot, so a short tail links.
    assert_eq!(
        autolink_html("www.a2"),
        "<p><a href=\"http://www.a2\">www.a2</a></p>\n"
    );
    // A bare www.: no dot inside the scan window, no link.
    assert_eq!(
        autolink_html("www."),
        "<p>www.</p>\n",
        "no dot inside the scan window, no link"
    );
    // The >10-segment escape: an underscore late in a long URL links.
    assert_eq!(
        autolink_html("www.1.2.3.4.5.6.7.8.9.10.11.12_a_b.com"),
        "<p><a href=\"http://www.1.2.3.4.5.6.7.8.9.10.11.12_a_b.com\">www.1.2.3.4.5.6.7.8.9.10.11.12_a_b.com</a></p>\n",
        "more than ten segments accepts the underscore"
    );
}

/// `check_domain` stops at an invalid host character; a `!` ends the
/// domain part of a www match.
#[test]
fn check_domain_stops_at_invalid_hostchar() {
    assert_eq!(
        autolink_html("www.a!b.com"),
        "<p><a href=\"http://www.a!b.com\">www.a!b.com</a></p>\n",
        "the scan breaks but the position reached counts, so the caller's line-end extension swallows the rest"
    );
}

/// `sd_autolink_issafe`: only the three safe schemes with a following
/// valid host character link; the scheme itself is case-insensitive.
#[test]
fn scheme_guard_cases() {
    assert_eq!(
        autolink_html("hTtPs://example.com"),
        "<p><a href=\"hTtPs://example.com\">hTtPs://example.com</a></p>\n",
        "the comparison is case-insensitive; the URL text is verbatim, never normalized"
    );
    // A scheme with no host character after it declines.
    assert_eq!(
        autolink_html("https://"),
        "<p>https://</p>\n",
        "no host character after the scheme, no link"
    );
    // An unsafe scheme is literal.
    assert_eq!(
        autolink_html("gopher://example.com"),
        "<p>gopher://example.com</p>\n"
    );
}

/// `autolink_delim`'s entity branch: a whole trailing `&name;` is cut,
/// but `&` without a `;` suffix cuts one byte at a time like the other
/// punctuation.
#[test]
fn delimiter_entity_trimming() {
    assert_eq!(
        autolink_html("www.example.com/a&amp;"),
        "<p><a href=\"http://www.example.com/a\">www.example.com/a</a>&amp;</p>\n",
        "a trailing entity is cut whole (the &amp; itself renders as &)"
    );
    // A trailing ; whose rewind stops at a non-& byte cuts just the ;.
    assert_eq!(
        autolink_html("www.example.com/a.&;"),
        "<p><a href=\"http://www.example.com/a.&amp;\">www.example.com/a.&amp;</a>;</p>\n",
        "a non-& where the rewind stops cuts one byte (the ; alone)"
    );
    // Not trailing: the trimmer only looks at the last byte, so an
    // entity in the middle stays.
    assert_eq!(
        autolink_html("www.example.com/&amp;x"),
        "<p><a href=\"http://www.example.com/&amp;amp;x\">www.example.com/&amp;amp;x</a></p>\n"
    );
}

/// `autolink_delim`'s bracket accounting: an unbalanced closing run
/// shrinks one byte per extra closer, and balanced parens stay.
#[test]
fn delimiter_bracket_accounting() {
    assert_eq!(
        autolink_html("www.example.com/a)b)c)"),
        "<p><a href=\"http://www.example.com/a)b)c\">www.example.com/a)b)c</a>)</p>\n",
        "two extra closers cost two bytes; the last closer stays outside"
    );
    assert_eq!(
        autolink_html("(www.example.com/a(b)c)"),
        "<p>(<a href=\"http://www.example.com/a(b)c\">www.example.com/a(b)c</a>)</p>\n",
        "balanced parens stay whole"
    );
}

/// A `<` inside the match ends it there (the spec's he<lp case has
/// company: it applies to scheme URLs too).
#[test]
fn less_than_ends_scheme_match() {
    assert_eq!(
        autolink_html("https://example.com/a<b"),
        "<p><a href=\"https://example.com/a\">https://example.com/a</a>&lt;b</p>\n"
    );
}

/// The `w` trigger's preceding-byte gate: only line start, whitespace,
/// `*_~(` pass; an ordinary letter before `www.` declines.
#[test]
fn www_trigger_preceding_gate() {
    assert_eq!(
        autolink_html("word www.example.com"),
        "<p>word <a href=\"http://www.example.com\">www.example.com</a></p>\n",
        "space before the trigger passes"
    );
    assert_eq!(
        autolink_html("(www.example.com"),
        "<p>(<a href=\"http://www.example.com\">www.example.com</a></p>\n",
        "an open paren before the trigger passes"
    );
    assert_eq!(
        autolink_html("*www.example.com*"),
        "<p><em><a href=\"http://www.example.com\">www.example.com</a></em></p>\n",
        "an emphasis opener before the trigger passes"
    );
    assert_eq!(
        autolink_html("awww.example.com"),
        "<p>awww.example.com</p>\n",
        "a letter before www. declines"
    );
    // `awww.` inside a word: the trigger byte w sits mid-word, so the
    // preceding gate declines it.
    assert_eq!(
        autolink_html("xawww.example.com"),
        "<p>xawww.example.com</p>\n"
    );
}

/// A www match that fails `check_domain` after the opener declines
/// without consuming anything.
#[test]
fn www_declines_after_domain_check() {
    assert_eq!(
        autolink_html("www._b.com"),
        "<p>www._b.com</p>\n",
        "an underscore in the last segment declines"
    );
}

/// The `:` trigger's shape check: `://` is required at bytes 1-2.
#[test]
fn url_trigger_shape() {
    assert_eq!(
        autolink_html("x:/nope"),
        "<p>x:/nope</p>\n",
        "one slash is not a URL"
    );
    assert_eq!(
        autolink_html("x:///nope"),
        "<p>x:///nope</p>\n",
        "an unsafe scheme is literal even with two slashes"
    );
}

/// The rewind run before `://` crosses only ASCII letters.
#[test]
fn url_rewind_is_letters_only() {
    assert_eq!(
        autolink_html("a1b://example.com"),
        "<p>a1b://example.com</p>\n",
        "a digit in the rewind run stops the rewind, so issafe fails"
    );
    assert_eq!(
        autolink_html("abc://example.com"),
        "<p>abc://example.com</p>\n",
        "a letter run rewinds, but abc is not a safe scheme"
    );
}

/// `validate_protocol`'s window rule: the protocol must end exactly
/// where the rewind run begins, and the byte before it must not be
/// alphanumeric.
#[test]
fn email_protocol_window() {
    // `xmp:` is not mailto/xmpp, so the address links with mailto but
    // the xmp: prefix stays literal (spec case repeated for context).
    assert_eq!(
        autolink_html("xmp:+foo@bar.example.com"),
        "<p>xmp:<a href=\"mailto:+foo@bar.example.com\">+foo@bar.example.com</a></p>\n"
    );
    // A letter before `mailto:` inside the rewind run breaks the
    // window: the address still links, as plain mailto.
    assert_eq!(
        autolink_html("xmailto:foo@bar.baz"),
        "<p>xmailto:<a href=\"mailto:foo@bar.baz\">foo@bar.baz</a></p>\n"
    );
}

/// The email scanner's forward-scan decisions: a second `@` restarts
/// with it as the anchor; a run ending in a non-letter non-dot
/// declines; `-` and `_` continue.
#[test]
fn email_forward_scan_edges() {
    // Two @s: the first is inside the rewind run of the second.
    assert_eq!(
        autolink_html("a@b@c.example"),
        "<p>a@<a href=\"mailto:b@c.example\">b@c.example</a></p>\n",
        "the later @ becomes the anchor; the earlier one stays text"
    );
    // A trailing `-` makes the last byte non-letter: declined.
    assert_eq!(autolink_html("foo@bar.com-"), "<p>foo@bar.com-</p>\n");
    // A trailing dot is fine (the dot is the `last != b'.'` branch).
    assert_eq!(
        autolink_html("foo@bar.baz."),
        "<p><a href=\"mailto:foo@bar.baz\">foo@bar.baz</a>.</p>\n"
    );
}

/// An address inside an existing link is left alone; an image's alt
/// text stays plain (the renderer's alt extraction takes text only,
/// even though the scan descends into images).
#[test]
fn email_scan_tree_links_and_images() {
    assert_eq!(
        autolink_html("[foo@bar.com](mailto:foo@bar.com)"),
        "<p><a href=\"mailto:foo@bar.com\">foo@bar.com</a></p>\n",
        "inside a link the text stays untouched"
    );
    assert_eq!(
        autolink_html("![foo@bar.com](x.png)"),
        "<p><img src=\"x.png\" alt=\"foo@bar.com\" /></p>\n",
        "an image's alt text renders as plain text"
    );
}

/// The text splitter's boundaries: an address at the very start and at
/// the very end of the node, and two addresses with text between.
#[test]
fn email_split_boundaries() {
    assert_eq!(
        autolink_html("foo@bar.com tail"),
        "<p><a href=\"mailto:foo@bar.com\">foo@bar.com</a> tail</p>\n",
        "address at the start leaves no prefix node"
    );
    assert_eq!(
        autolink_html("head foo@bar.com"),
        "<p>head <a href=\"mailto:foo@bar.com\">foo@bar.com</a></p>\n",
        "address at the end leaves no suffix node"
    );
    assert_eq!(
        autolink_html("a@b.com mid c@d.com"),
        "<p><a href=\"mailto:a@b.com\">a@b.com</a> mid <a href=\"mailto:c@d.com\">c@d.com</a></p>\n",
        "the text between two addresses stays one node"
    );
}
