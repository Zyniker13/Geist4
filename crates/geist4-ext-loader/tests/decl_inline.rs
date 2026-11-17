//! Tests for the declarative inline tier: the `[[inline_span]]` and
//! `[[wiki_link]]` semantics of `docs/extensions/format.md`, exercised
//! end to end through parse and render.

use std::collections::BTreeMap;
use std::sync::Arc;

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_ext_loader::decl_inline::InlineTier;
use geist4_ext_loader::types::{self, Behavior, KindEntry, Manifest};

const MARK: &str = "<mark>{children}</mark>";
const DEL: &str = "<del>{children}</del>";
const MATH: &str = "<span>{children}</span>";
const WIKI: &str = "<a href=\"{target}\" class=\"wikilink\">{children}</a>";

/// A minimal manifest carrying `kinds` and `behavior`.
fn manifest(kinds: Vec<KindEntry>, behavior: Vec<Behavior>) -> Manifest {
    Manifest {
        extension: types::Extension {
            name: "test".to_owned(),
            version: "1.0.0".to_owned(),
            format: "1.0".to_owned(),
            description: String::new(),
            license: "MIT".to_owned(),
            homepage: None,
            hooks: Vec::new(),
            after: Vec::new(),
            conflicts: Vec::new(),
        },
        kinds,
        options: BTreeMap::new(),
        behavior,
        wasm: None,
        host: false,
    }
}

/// A `[[kinds]]` entry for `name`: a container of inline text with no
/// attributes.
fn kind(name: &str) -> KindEntry {
    KindEntry {
        kind: name.to_owned(),
        container: true,
        inline_text: true,
        attrs: Vec::new(),
    }
}

/// A `[[kinds]]` entry declaring `attrs`.
fn kind_with_attrs(name: &str, attrs: &[&str]) -> KindEntry {
    KindEntry {
        kind: name.to_owned(),
        container: true,
        inline_text: true,
        attrs: attrs.iter().map(|attr| (*attr).to_owned()).collect(),
    }
}

/// An `[[inline_span]]` entry that opens, closes and crosses lines.
fn span(kind_name: &str, open: &str, close: &str, html: &str) -> Behavior {
    Behavior::InlineSpan(types::InlineSpan {
        kind: kind_name.to_owned(),
        open: open.to_owned(),
        close: close.to_owned(),
        html: html.to_owned(),
        can_open: true,
        can_close: true,
        multiline: true,
        math: false,
    })
}

/// An `[[inline_span]]` entry with the multiline flag chosen.
fn span_lines(kind_name: &str, open: &str, close: &str, html: &str, multiline: bool) -> Behavior {
    Behavior::InlineSpan(types::InlineSpan {
        kind: kind_name.to_owned(),
        open: open.to_owned(),
        close: close.to_owned(),
        html: html.to_owned(),
        can_open: true,
        can_close: true,
        multiline,
        math: false,
    })
}

/// An `[[inline_span]]` entry following the inline math guards.
fn math_span(kind_name: &str, open: &str, close: &str, html: &str) -> Behavior {
    Behavior::InlineSpan(types::InlineSpan {
        kind: kind_name.to_owned(),
        open: open.to_owned(),
        close: close.to_owned(),
        html: html.to_owned(),
        can_open: true,
        can_close: true,
        multiline: true,
        math: true,
    })
}

/// A `[[wiki_link]]` entry.
fn wiki(kind_name: &str, prefix: &str, suffix: &str, html: &str) -> Behavior {
    Behavior::WikiLink(types::WikiLink {
        kind: kind_name.to_owned(),
        prefix: prefix.to_owned(),
        suffix: suffix.to_owned(),
        html: html.to_owned(),
    })
}

/// Parses and renders `source` with one `InlineTier` over `manifest`.
fn render(manifest: &Manifest, source: &str) -> String {
    let mut options = Options::new();
    options.extension(Arc::new(InlineTier::from_manifest(manifest)));
    let document = parse_with(source, &options);
    render_with(&document, &options)
}

/// One `==…==` span entry over the `mark` kind.
fn mark_manifest() -> Manifest {
    manifest(vec![kind("mark")], vec![span("mark", "==", "==", MARK)])
}

#[test]
fn span_wraps_content() {
    assert_eq!(render(&mark_manifest(), "==x=="), "<p><mark>x</mark></p>\n");
}

#[test]
fn span_unmatched_opener_is_literal() {
    assert_eq!(render(&mark_manifest(), "==x"), "<p>==x</p>\n");
}

#[test]
fn span_unmatched_closer_is_literal() {
    assert_eq!(render(&mark_manifest(), "a=="), "<p>a==</p>\n");
}

#[test]
fn span_escaped_opener_stays_literal() {
    // The escaped `=` cannot start the opener run; the later `==` pair
    // has no opener left and stays literal too.
    assert_eq!(render(&mark_manifest(), "\\==x=="), "<p>==x==</p>\n");
}

#[test]
fn span_escaped_closer_inside_keeps_span() {
    // The escaped `==` does not close; the leftover single `=` becomes
    // part of the span's content.
    assert_eq!(
        render(&mark_manifest(), "==a\\==b=="),
        "<p><mark>a==b</mark></p>\n"
    );
}

#[test]
fn span_flanking_spaces_block() {
    // `a == b`: the opener candidate is followed by a space and the
    // closer candidate preceded by one, so neither flank works.
    assert_eq!(render(&mark_manifest(), "a == b"), "<p>a == b</p>\n");
}

#[test]
fn span_flanking_word_edges_pair() {
    assert_eq!(
        render(&mark_manifest(), "a ==b== c"),
        "<p>a <mark>b</mark> c</p>\n"
    );
}

#[test]
fn span_flanking_line_edges_stay_literal() {
    // An opener at a line's start cannot open; the closer stays
    // unmatched and both delimiters render as written.
    assert_eq!(render(&mark_manifest(), "==\nx=="), "<p>==\nx==</p>\n");
}

#[test]
fn one_byte_delimiters_pair_as_runs() {
    // One-byte delimiters count runs as units, like emphasis: equal
    // runs pair whatever their length.
    let manifest = manifest(vec![kind("del")], vec![span("del", "%", "%", DEL)]);
    assert_eq!(render(&manifest, "%a%"), "<p><del>a</del></p>\n");
    assert_eq!(render(&manifest, "%%a%%"), "<p><del>a</del></p>\n");
    assert_eq!(render(&manifest, "%%%a%%%"), "<p><del>a</del></p>\n");
}

#[test]
fn one_byte_delimiter_inside_words() {
    // Both-flanking single-byte runs pair, rule of three permitting.
    assert_eq!(
        render(
            &manifest(vec![kind("del")], vec![span("del", "%", "%", DEL)]),
            "x%y%z"
        ),
        "<p>x<del>y</del>z</p>\n"
    );
}

#[test]
fn one_byte_rule_of_three_blocks_pair() {
    // `x%%y%z`: both runs flank on both sides, and 2 + 1 is divisible
    // by three, so the pair is blocked and everything stays literal.
    assert_eq!(
        render(
            &manifest(vec![kind("del")], vec![span("del", "%", "%", DEL)]),
            "x%%y%z"
        ),
        "<p>x%%y%z</p>\n"
    );
}

#[test]
fn two_byte_delimiter_spans_whole_runs() {
    // A two-byte delimiter is one unit: the four-`=` runs are not four
    // units but one each, and the whole runs pair.
    assert_eq!(
        render(&mark_manifest(), "====x===="),
        "<p><mark>x</mark></p>\n"
    );
}

#[test]
fn two_byte_delimiter_needs_full_run() {
    // A single `=` is not an occurrence of the two-byte delimiter.
    assert_eq!(render(&mark_manifest(), "=x="), "<p>=x=</p>\n");
}

#[test]
fn two_byte_mismatched_runs_stay_literal() {
    // Three `=` against two: the runs match positionally but not in
    // length, nothing wraps, and both runs render as written.
    assert_eq!(render(&mark_manifest(), "===x=="), "<p>===x==</p>\n");
}

#[test]
fn multiline_span_crosses_soft_break() {
    assert_eq!(
        render(&mark_manifest(), "==a\nb=="),
        "<p><mark>a\nb</mark></p>\n"
    );
}

#[test]
fn single_line_span_unwraps_across_soft_break() {
    // The delimiter machinery pairs across the soft break; the
    // multiline rule unwraps the node back to literal delimiters.
    let manifest = manifest(
        vec![kind("mark")],
        vec![span_lines("mark", "==", "==", MARK, false)],
    );
    assert_eq!(render(&manifest, "==a\nb=="), "<p>==a\nb==</p>\n");
}

#[test]
fn single_line_span_within_one_line() {
    let manifest = manifest(
        vec![kind("mark")],
        vec![span_lines("mark", "==", "==", MARK, false)],
    );
    assert_eq!(render(&manifest, "==a b=="), "<p><mark>a b</mark></p>\n");
}

#[test]
fn single_line_span_unwraps_around_inner_span() {
    // Only the crossing node unwraps; the inner one-line span keeps
    // its markup.
    let manifest = manifest(
        vec![kind("mark")],
        vec![span_lines("mark", "==", "==", MARK, false)],
    );
    assert_eq!(
        render(&manifest, "==a ==b==\nc=="),
        "<p>==a <mark>b</mark>\nc==</p>\n"
    );
}

#[test]
fn math_span_wraps() {
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "$x$"), "<p><span>x</span></p>\n");
}

#[test]
fn math_open_space_stays_literal() {
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "$ x$"), "<p>$ x$</p>\n");
}

#[test]
fn math_close_space_stays_literal() {
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "$x $"), "<p>$x $</p>\n");
}

#[test]
fn math_open_digit_stays_literal() {
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "$5$"), "<p>$5$</p>\n");
}

#[test]
fn math_digit_before_open_stays_literal() {
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "5$x$"), "<p>5$x$</p>\n");
}

#[test]
fn math_after_digit_stays_literal() {
    // `a$5`: the open is followed by a digit, so it cannot open and
    // the lone `$` renders as written.
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "a$5"), "<p>a$5</p>\n");
}

#[test]
fn math_interior_spaces_allowed() {
    let manifest = manifest(vec![kind("math")], vec![math_span("math", "$", "$", MATH)]);
    assert_eq!(render(&manifest, "$a b$"), "<p><span>a b</span></p>\n");
}

#[test]
fn wiki_link_renders_target() {
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(
        render(&manifest, "[[Page]]"),
        "<p><a href=\"Page\" class=\"wikilink\">Page</a></p>\n"
    );
}

#[test]
fn wiki_link_label_overrides_target() {
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(
        render(&manifest, "[[Page|see the page]]"),
        "<p><a href=\"Page\" class=\"wikilink\">see the page</a></p>\n"
    );
}

#[test]
fn real_link_wins_over_wiki() {
    // A `(` after the close bracket makes the whole thing an inline
    // link first; the wiki hook never sees the group.
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(
        render(&manifest, "[[Page]](x)"),
        "<p><a href=\"x\">[Page]</a></p>\n"
    );
}

#[test]
fn wiki_target_percent_encodes_spaces() {
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(
        render(&manifest, "[[Page name]]"),
        "<p><a href=\"Page%20name\" class=\"wikilink\">Page name</a></p>\n"
    );
}

#[test]
fn wiki_target_encodes_hash_and_query() {
    // `#` and `?` are plain href-safe bytes for the core escaper; the
    // wiki rules encode them anyway so a page name stays one page.
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(
        render(&manifest, "[[a#b?c]]"),
        "<p><a href=\"a%23b%3Fc\" class=\"wikilink\">a#b?c</a></p>\n"
    );
}

#[test]
fn wiki_suffix_mismatch_stays_literal() {
    // The group's own closing bracket is never part of the content, so
    // the content must end with the suffix's remainder by way of an
    // inner bracket: `[[a]b]` closes with content `[a]b`, which does
    // not end in `]`, and nothing is claimed.
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(render(&manifest, "[[a]b]"), "<p>[[a]b]</p>\n");
}

#[test]
fn wiki_claim_happens_before_emphasis() {
    // The claim runs at the close bracket, before the emphasis
    // machinery pairs, so the claimed content is plain text with the
    // delimiter characters literal; emphasis inside a claimed wiki
    // label never renders as markup.
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(
        render(&manifest, "[[a *b* c]]"),
        "<p><a href=\"a%20*b*%20c\" class=\"wikilink\">a *b* c</a></p>\n"
    );
}

#[test]
fn wiki_group_clears_inner_span_delims() {
    // The claimed group's content is plain text — the `==` runs inside
    // contributed text nodes whose delimiters the claim cleared — so
    // the target keeps them literally.
    let manifest = manifest(
        vec![kind("wiki"), kind("mark")],
        vec![
            wiki("wiki", "[[", "]]", WIKI),
            span("mark", "==", "==", MARK),
        ],
    );
    assert_eq!(
        render(&manifest, "[[a ==b== c]]"),
        "<p><a href=\"a%20==b==%20c\" class=\"wikilink\">a ==b== c</a></p>\n"
    );
}

#[test]
fn wiki_unterminated_group_stays_text() {
    let manifest = manifest(vec![kind("wiki")], vec![wiki("wiki", "[[", "]]", WIKI)]);
    assert_eq!(render(&manifest, "[[Page"), "<p>[[Page</p>\n");
}

#[test]
fn span_declared_attr_absent_renders_empty() {
    let manifest = manifest(
        vec![kind_with_attrs("mark", &["note"])],
        vec![span(
            "mark",
            "==",
            "==",
            "<mark data-note=\"{note}\">{children}</mark>",
        )],
    );
    assert_eq!(
        render(&manifest, "==x=="),
        "<p><mark data-note=\"\">x</mark></p>\n"
    );
}

#[test]
fn span_renders_nested_inline() {
    assert_eq!(
        render(&mark_manifest(), "==a *b* c=="),
        "<p><mark>a <em>b</em> c</mark></p>\n"
    );
}

#[test]
fn span_pairs_nearest_opener() {
    // The machinery treats each closer-flanking run as a closer first:
    // the middle run of `==a==b==` is both-flanking, so it closes the
    // opener before it, wrapping `a`, and the last `==` stays literal.
    assert_eq!(
        render(&mark_manifest(), "==a==b=="),
        "<p><mark>a</mark>b==</p>\n"
    );
}

#[test]
fn entries_share_trigger_by_length() {
    // Two entries on the `=` trigger: the two-byte rule takes runs of
    // two or more, the one-byte rule takes the rest.
    let manifest = manifest(
        vec![kind("hi"), kind("accent")],
        vec![
            span("hi", "==", "==", "<b>{children}</b>"),
            span("accent", "=", "=", "<i>{children}</i>"),
        ],
    );
    assert_eq!(render(&manifest, "=x="), "<p><i>x</i></p>\n");
    assert_eq!(render(&manifest, "==x=="), "<p><b>x</b></p>\n");
}

#[test]
fn span_stops_at_paragraph_boundary() {
    // Spans are inline: a blank line splits the paragraph and the
    // delimiters never meet.
    assert_eq!(
        render(&mark_manifest(), "==a\n\nb=="),
        "<p>==a</p>\n<p>b==</p>\n"
    );
}

#[test]
fn span_wraps_wiki_link() {
    let manifest = manifest(
        vec![kind("mark"), kind("wiki")],
        vec![
            span("mark", "==", "==", MARK),
            wiki("wiki", "[[", "]]", WIKI),
        ],
    );
    assert_eq!(
        render(&manifest, "==a [[b]] c=="),
        "<p><mark>a <a href=\"b\" class=\"wikilink\">b</a> c</mark></p>\n"
    );
}

#[test]
fn template_unknown_slot_stays_literal() {
    let manifest = manifest(
        vec![kind("mark")],
        vec![span("mark", "==", "==", "<mark>{children}{unknown}</mark>")],
    );
    assert_eq!(
        render(&manifest, "==x=="),
        "<p><mark>x{unknown}</mark></p>\n"
    );
}
