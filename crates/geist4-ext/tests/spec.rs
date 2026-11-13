//! Runs the built-in extensions' spec files through the engine with each
//! extension enabled.
//!
//! Every expected output in these files was verified against the reference
//! implementations the extensions follow, and the files are generated from
//! that comparison, so a failure here is a real behavioral change.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_spec_harness::{parse, run};

const STRIKETHROUGH_SPEC: &str = include_str!("../spec/strikethrough.md");
const TABLES_SPEC: &str = include_str!("../spec/tables.md");
const AUTOLINK_SPEC: &str = include_str!("../spec/autolink.md");
const TASKLIST_SPEC: &str = include_str!("../spec/tasklist.md");
const TAGFILTER_SPEC: &str = include_str!("../spec/tagfilter.md");
const FOOTNOTES_SPEC: &str = include_str!("../spec/footnotes.md");

/// Runs a spec file with `options` and requires every example to pass.
fn run_spec(name: &str, spec: &str, options: &Options) {
    let examples = parse(spec).unwrap_or_else(|error| panic!("{name} spec must parse: {error}"));
    assert!(
        !examples.is_empty(),
        "{name} spec must hold at least one example"
    );
    let mut render = |example: &geist4_spec_harness::Example| -> String {
        render_with(&parse_with(&example.markdown, options), options)
    };
    let report = run(&examples, &mut render);
    assert_eq!(
        report.failed(),
        0,
        "{name} spec has failing examples:\n{report}"
    );
}

#[test]
fn strikethrough_spec() {
    let mut options = Options::new();
    options.extension(geist4_ext::strikethrough());
    run_spec("strikethrough", STRIKETHROUGH_SPEC, &options);
}

#[test]
fn tables_spec() {
    let mut options = Options::new();
    options.extension(geist4_ext::tables());
    run_spec("tables", TABLES_SPEC, &options);
}

#[test]
fn autolink_spec() {
    let mut options = Options::new();
    options.extension(geist4_ext::autolink());
    run_spec("autolink", AUTOLINK_SPEC, &options);
}

#[test]
fn tasklist_spec() {
    let mut options = Options::new();
    options.extension(geist4_ext::tasklist());
    run_spec("tasklist", TASKLIST_SPEC, &options);
}

#[test]
fn tagfilter_spec() {
    let mut options = Options::new();
    options.extension(geist4_ext::tagfilter());
    run_spec("tagfilter", TAGFILTER_SPEC, &options);
}

#[test]
fn footnotes_spec() {
    let mut options = Options::new();
    options.extension(geist4_ext::footnotes());
    run_spec("footnotes", FOOTNOTES_SPEC, &options);
}

/// With no extensions, blacklisted raw HTML passes through untouched:
/// the same sources the filter rewrites with the extension must not
/// change plain CommonMark output.
#[test]
fn tagfilter_off_leaves_plain_commonmark() {
    let cases = [
        "<script>alert(1)</script>\n",
        "a <title>t</title> b\n",
        "<style>\nx\n</style>\n",
    ];
    for source in cases {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        assert!(
            !plain.contains("&lt;"),
            "without the extension the tags stay raw: {source:?}"
        );
        let mut options = Options::new();
        options.extension(geist4_ext::tagfilter());
        let filtered = render_with(&parse_with(source, &options), &options);
        assert_ne!(
            plain, filtered,
            "the extension must change this output: {source:?}"
        );
    }
}

/// With no extensions, `[ ]`/`[x]` runs stay plain text: the same sources
/// that carry a checkbox with the extension must not change plain
/// CommonMark output.
#[test]
fn tasklist_off_leaves_plain_commonmark() {
    let cases = ["- [ ] foo\n", "- [x] foo\n", "1. [X] bar\n"];
    for source in cases {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        assert!(
            plain.contains("[") && !plain.contains("checkbox"),
            "without the extension the box stays literal: {source:?}"
        );
        let mut options = Options::new();
        options.extension(geist4_ext::tasklist());
        let boxed = render_with(&parse_with(source, &options), &options);
        assert_ne!(
            plain, boxed,
            "the extension must change this output: {source:?}"
        );
    }
}

/// With no extensions, `www.` domains, scheme URLs and email addresses
/// stay plain text: the same sources that link with the extension must
/// not change plain CommonMark output.
#[test]
fn autolink_off_leaves_plain_commonmark() {
    let cases = [
        "www.example.com\n",
        "http://example.com\n",
        "a@b.com\n",
        "mail a@example.com now\n",
    ];
    for source in cases {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        let mut options = Options::new();
        options.extension(geist4_ext::autolink());
        let linked = render_with(&parse_with(source, &options), &options);
        assert!(
            !plain.contains("<a href"),
            "without the extension the candidates stay literal: {source:?}"
        );
        assert_ne!(
            plain, linked,
            "the extension must change this output: {source:?}"
        );
    }
}

/// With no extensions, pipe rows stay plain paragraphs: the same sources
/// that build tables with the extension must not change plain output.
#[test]
fn tables_off_leaves_plain_commonmark() {
    let cases = [
        "| a | b |\n| --- | --- |\n| c | d |\n",
        "text\n| a | b |\n| --- | --- |\n| c | d |\n",
    ];
    for source in cases {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        let mut options = Options::new();
        options.extension(geist4_ext::tables());
        let tabled = render_with(&parse_with(source, &options), &options);
        assert!(
            plain.contains("<p>"),
            "without the extension the rows stay a paragraph: {source:?}"
        );
        assert_ne!(
            plain, tabled,
            "the extension must change this output: {source:?}"
        );
    }
}

/// With no extensions, tilde runs are plain text: the same sources that
/// strike with the extension must not change plain CommonMark output.
#[test]
fn strikethrough_off_leaves_plain_commonmark() {
    let cases = ["~~Hi~~ Hello, world!\n", "~x~\n", "~~a\nb~~\n"];
    for source in cases {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        let mut options = Options::new();
        options.extension(geist4_ext::strikethrough());
        let struck = render_with(&parse_with(source, &options), &options);
        assert!(
            plain.contains('~'),
            "without the extension the tildes stay literal: {source:?}"
        );
        assert_ne!(
            plain, struck,
            "the extension must change this output: {source:?}"
        );
    }
}

/// With no extensions, `[^label]` runs stay literal text: the same
/// sources that carry references and definitions with the extension
/// must not change plain CommonMark output. A definition line reads as
/// a link reference definition without the extension, and its bracket
/// then resolves as a reference link — plain CommonMark behavior the
/// extension replaces with the footnote. A reference with no
/// definition at all renders the same both ways.
#[test]
fn footnotes_off_leaves_plain_commonmark() {
    for source in ["x[^1]\n\n[^1]: note\n", "a[^x] b[^x]\n\n[^x]: y\n"] {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        let mut options = Options::new();
        options.extension(geist4_ext::footnotes());
        let footnoted = render_with(&parse_with(source, &options), &options);
        assert_ne!(
            plain, footnoted,
            "the extension must change this output: {source:?}"
        );
    }
    for source in ["x[^1]\n", "[^1]\n"] {
        let plain = render_with(&parse_with(source, &Options::new()), &Options::new());
        assert!(
            plain.contains("[^"),
            "without the extension the marker stays literal: {source:?}"
        );
    }
}
