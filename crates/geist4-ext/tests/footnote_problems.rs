//! Footnote problems: a reference whose definition is missing goes
//! back to literal text and records a problem; a defined reference
//! records none. The output is unchanged either way.

use geist4_core::basic_html::render_with;
use geist4_core::parser::parse_with;

fn options() -> geist4_core::extension::Options {
    let mut options = geist4_core::extension::Options::new();
    options.extension(geist4_ext::footnotes());
    options
}

#[test]
fn undefined_footnote_reference_is_a_problem_with_unchanged_output() {
    let src = "text[^1]\n";
    let document = parse_with(src, &options());
    assert_eq!(render_with(&document, &options()), "<p>text[^1]</p>\n");
    assert_eq!(document.problems.len(), 1);
    assert_eq!(
        document.problems[0].message,
        "no footnote definition for footnote reference '[^1]'"
    );
    // The problem carries the span of the block that holds the
    // reference; inline nodes keep no spans. The paragraph ends at its
    // last content byte, before the final newline.
    assert_eq!(
        document.problems[0].span,
        geist4_core::span::Span::new(0, src.len() - 1)
    );
}

#[test]
fn defined_footnote_reference_produces_no_problems() {
    let src = "text[^1]\n\n[^1]: note\n";
    let document = parse_with(src, &options());
    assert!(document.problems.is_empty());
    assert!(render_with(&document, &options()).contains("footnote"));
}

#[test]
fn a_reference_with_several_text_cells_keeps_the_rest_of_the_paragraph() {
    // The label here parses into three text cells (`^a`, `*`, `b` —
    // the unmatched `*` stays literal). The claim consumes them one by
    // one; the text after the reference must still join the paragraph
    // at the right place.
    let src = "text[^a*b] tail\n\n[^a*b]: note\n";
    let document = parse_with(src, &options());
    assert!(document.problems.is_empty());
    let html = render_with(&document, &options());
    assert_eq!(html.matches("tail").count(), 1, "{html}");
    assert!(!html.contains("[^a*b]"), "{html}");
    assert!(html.contains("footnote-ref"), "{html}");
    assert!(html.contains("note"), "{html}");
}
