//! Unit tests for the tagfilter matcher itself (`starts_with_tag` and
//! its boundary class), exercised through the extension's public
//! `html_filter` hook. The spec file (`spec/tagfilter.md`) pins the
//! rendered output; these tests pin the matcher's branch decisions the
//! spec examples do not reach.

use geist4_core::extension::Extension;
use geist4_ext::tagfilter::Tagfilter;

/// Shorthand: does the filter reject this literal (returning true =
/// keep, so a blacklisted start returns false)?
fn keeps(literal: &[u8]) -> bool {
    Tagfilter.html_filter(literal)
}

/// Short and non-`<` literals never match: the length and first-byte
/// guards reject before any name comparison.
#[test]
fn short_and_plain_literals_pass_through() {
    assert!(keeps(b""), "an empty literal has no tag");
    assert!(keeps(b"<"), "lone < is under the 3-byte floor");
    assert!(keeps(b"<x"), "two bytes cannot carry a full name");
    assert!(keeps(b"xmp"), "no leading < means no tag");
    assert!(keeps(b" a < b"), "the < is not at the start");
}

/// The optional closing slash is part of the match.
#[test]
fn closing_slash_is_matched() {
    assert!(!keeps(b"</script>"), "a closing blacklisted tag filters");
    assert!(keeps(b"</span>"), "a closing unlisted tag passes");
    assert!(
        !keeps(b"</TITLE>x"),
        "the slash form is case-insensitive too"
    );
}

/// The name matches case-insensitively, in full, against every
/// blacklist entry.
#[test]
fn every_blacklist_entry_matches_case_insensitively() {
    for tag in [
        "title",
        "textarea",
        "style",
        "xmp",
        "iframe",
        "noembed",
        "noframes",
        "script",
        "plaintext",
    ] {
        let upper = format!("<{}/>", tag.to_ascii_uppercase());
        assert!(!keeps(upper.as_bytes()), "{tag} must filter in caps");
        let unclosed = format!("<{} ", tag);
        assert!(
            !keeps(unclosed.as_bytes()),
            "{tag} must filter unclosed-with-boundary"
        );
        assert!(
            keeps(format!("<{}", tag).as_bytes()),
            "{tag} with no boundary byte passes (the name-at-end rule)"
        );
    }
}

/// A name that diverges from a blacklist entry passes: the mismatch
/// can fall in the middle or at the last letter, and a longer literal
/// that merely starts like a blacklisted name (e.g. `<small`) passes
/// because the boundary byte decides, not the name.
#[test]
fn lookalike_names_pass_through() {
    assert!(keeps(b"<scrip>"), "a truncated name is a different tag");
    assert!(keeps(b"<styles>"), "extra letters make a different tag");
    assert!(
        keeps(b"<small>"),
        "<small starts like <s(tyle) but is not it"
    );
    assert!(keeps(b"<tittle>"), "one letter off in the middle passes");
    assert!(keeps(b"<titl>"), "a shorter name passes");
}

/// The byte after the name decides: whitespace (the reference's
/// `cmark_isspace` class), `>` and `/>` filter; other bytes do not.
#[test]
fn boundary_byte_decides() {
    assert!(!keeps(b"<script "), "trailing space is a boundary");
    assert!(!keeps(b"<script\t"), "tab is a boundary");
    assert!(!keeps(b"<script\r"), "carriage return is a boundary");
    assert!(!keeps(b"<script\n"), "line feed is a boundary");
    assert!(!keeps(b"<script>"), "> is a boundary");
    assert!(!keeps(b"<script/>"), "the self-closing form filters");
    assert!(
        keeps(b"<scriptx"),
        "a letter after the name is not a boundary"
    );
    assert!(
        keeps(b"<script_"),
        "underscore after the name is not a boundary"
    );
    // Vertical tab and form feed are NOT in the reference's boundary
    // class here (decision 0016), unlike the core's whitespace.
    assert!(keeps(b"<script\x0b"), "vertical tab is not a boundary");
    assert!(keeps(b"<script\x0c"), "form feed is not a boundary");
}

/// A literal that ends right after a complete name has no boundary
/// byte and does not filter.
#[test]
fn name_at_end_of_literal_passes() {
    assert!(keeps(b"<script"), "no boundary byte, no filter");
    assert!(keeps(b"</style"), "the same for the closing form");
}

/// The extension reports `filters_html`, and its name is `tagfilter`.
#[test]
fn surface() {
    assert!(Tagfilter.filters_html());
    assert_eq!(Tagfilter.name(), "tagfilter");
}
