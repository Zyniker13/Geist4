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
