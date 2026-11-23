//! The `safe` module's contract: the same behavior the C ABI pins, from
//! ordinary safe Rust — no raw pointers anywhere in this file. Whatever
//! a binding (the WebAssembly layer, a tool, a test harness) gets from
//! `geist4_ffi::safe` is pinned here.

use geist4_ffi::safe::{Options, OwnedDocument, ProblemReport, render_html};

#[test]
fn plain_parse_renders_byte_for_byte() {
    let options = Options::new();
    let document = OwnedDocument::parse("# hi\n", Some(&options)).expect("parse cannot fail");
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<h1>hi</h1>\n"
    );
}

#[test]
fn empty_source_parses_and_renders_empty() {
    let options = Options::new();
    let document = OwnedDocument::parse("", Some(&options)).expect("parse cannot fail");
    assert_eq!(document.render_html().expect("render cannot fail"), "");
    assert_eq!(document.problems(), Vec::new());
}

#[test]
fn the_gfm_bundle_enables_strikethrough() {
    let mut options = Options::new();
    options.enable_builtin("gfm").expect("gfm is a valid name");
    let document = OwnedDocument::parse("~~gone~~\n", Some(&options)).expect("parse cannot fail");
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<p><del>gone</del></p>\n"
    );
}

#[test]
fn an_unknown_name_is_rejected_with_the_valid_names() {
    let mut options = Options::new();
    let error = options
        .enable_builtin("nope")
        .expect_err("nope is not a name");
    assert_eq!(error.name, "nope");
    assert!(error.to_string().contains("valid names"), "{}", error);
}

#[test]
fn enabling_a_bundle_twice_registers_nothing_twice() {
    // A bundle enabled twice must not double-register its members: the
    // engine would run every hook twice.
    let mut options = Options::new();
    options.enable_builtin("gfm").expect("gfm is a valid name");
    options.enable_builtin("gfm").expect("gfm is a valid name");
    options
        .enable_builtin("strikethrough")
        .expect("strikethrough is a valid name");
    let document = OwnedDocument::parse("~~x~~\n", Some(&options)).expect("parse cannot fail");
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<p><del>x</del></p>\n"
    );
}

#[test]
fn problems_carry_spans_inside_the_source() {
    let mut options = Options::new();
    options
        .enable_builtin("footnotes")
        .expect("footnotes is a valid name");
    let source = "text [missing][nope] end\n";
    let document = OwnedDocument::parse(source, Some(&options)).expect("parse cannot fail");
    let problems: Vec<ProblemReport> = document.problems();
    assert!(!problems.is_empty(), "the broken reference is a problem");
    for problem in &problems {
        assert!(problem.start < problem.end, "{problem:?}");
        assert!(problem.end <= source.len(), "{problem:?}");
    }
    assert!(
        problems
            .iter()
            .any(|problem| problem.message.contains("nope")),
        "a problem names the missing reference"
    );
}

#[test]
fn the_document_outlives_its_options() {
    // The registration is cloned into the document at parse time, so
    // dropping the options value afterwards changes nothing.
    let mut options = Options::new();
    options
        .enable_builtin("strikethrough")
        .expect("strikethrough is a valid name");
    let document = OwnedDocument::parse("~~x~~\n", Some(&options)).expect("parse cannot fail");
    drop(options);
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<p><del>x</del></p>\n"
    );
}

#[test]
fn a_document_can_be_moved_after_parsing() {
    // The erased borrows point into the input's heap allocation, which
    // moving the Box does not move. This pins that: parse inside a
    // closure, move the value out, render it far from where it was
    // created.
    let document = {
        let options = Options::new();
        OwnedDocument::parse("moved\n", Some(&options)).expect("parse cannot fail")
    };
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<p>moved</p>\n"
    );
}

#[test]
fn the_one_shot_path_needs_no_options() {
    assert_eq!(
        render_html("# one\n", None).expect("render cannot fail"),
        "<h1>one</h1>\n"
    );
}

#[test]
fn the_one_shot_path_takes_options() {
    let mut options = Options::new();
    options
        .enable_builtin("tasklist")
        .expect("tasklist is a valid name");
    assert_eq!(
        render_html("- [x] done\n", Some(&options)).expect("render cannot fail"),
        "<ul>\n<li><input type=\"checkbox\" checked=\"\" disabled=\"\" /> done</li>\n</ul>\n"
    );
}

#[test]
fn a_nul_byte_is_ordinary_content() {
    let document = OwnedDocument::parse("a\0b\n", None).expect("parse cannot fail");
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<p>a\u{FFFD}b</p>\n"
    );
}

#[test]
fn version_matches_the_crate() {
    assert_eq!(geist4_ffi::safe::version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn highlighting_off_renders_plain_code() {
    let mut options = Options::new();
    options.enable_builtin("gfm").expect("gfm is a valid name");
    let document = OwnedDocument::parse("```rust\nlet x = 1;\n```\n", Some(&options))
        .expect("parse cannot fail");
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<pre><code class=\"language-rust\">let x = 1;\n</code></pre>\n"
    );
}

#[test]
fn highlighting_on_wraps_tokens_in_class_spans() {
    let mut options = Options::new();
    options.enable_builtin("gfm").expect("gfm is a valid name");
    options.enable_highlighting();
    let document = OwnedDocument::parse("```rust\nlet x = 1;\n```\n", Some(&options))
        .expect("parse cannot fail");
    let html = document.render_html().expect("render cannot fail");
    assert!(
        html.contains("<span class=\"storage\">let</span>"),
        "the keyword token gets its class: {html}"
    );
    assert!(
        html.contains("<span class=\"numeric\">1</span>"),
        "the numeric token gets its class: {html}"
    );
    // The wrapper and the escaping are unchanged.
    assert!(html.starts_with("<pre><code class=\"language-rust\">"));
    assert!(html.ends_with("</code></pre>\n"));
}

#[test]
fn highlighting_on_unknown_language_stays_plain() {
    let mut options = Options::new();
    options.enable_highlighting();
    let document = OwnedDocument::parse("```notalanguage\n<a>&b\n```\n", Some(&options))
        .expect("parse cannot fail");
    assert_eq!(
        document.render_html().expect("render cannot fail"),
        "<pre><code class=\"language-notalanguage\">&lt;a&gt;&amp;b\n</code></pre>\n"
    );
}
