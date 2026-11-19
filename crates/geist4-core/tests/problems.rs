//! Problems: diagnostics a parse records about its source, with the
//! output unchanged. Broken references are the load-bearing case: the
//! bracket group stays literal text, and the parse records a problem
//! so a checker can surface it with its source span.

use geist4_core::basic_html::render_with;
use geist4_core::debug;
use geist4_core::extension::{AttrValue, Extension, InlineBracketCtx, Options};
use geist4_core::parser::parse_with;

fn html_of(src: &str) -> String {
    render_with(&parse_with(src, &Options::new()), &Options::new())
}

#[test]
fn broken_reference_is_a_problem_with_unchanged_output() {
    let src = "see [text][missing]\n";
    let document = parse_with(src, &Options::new());
    assert_eq!(html_of(src), "<p>see [text][missing]</p>\n");
    // One problem for the author's one mistake. The failed attempt's
    // rewind re-scans the following label as its own bracket group, and
    // that retry records nothing — the parse's artifact, not a second
    // mistake. The span is the block's (inline nodes keep no spans): the
    // paragraph ends at its last content byte, before the final newline.
    assert_eq!(document.problems.len(), 1);
    assert_eq!(
        document.problems[0].message,
        "no link reference definition for 'missing'"
    );
    assert_eq!(
        document.problems[0].span,
        geist4_core::span::Span::new(0, src.len() - 1)
    );
}

#[test]
fn broken_image_reference_is_a_problem() {
    let document = parse_with("![alt][gone]\n", &Options::new());
    assert_eq!(document.problems.len(), 1);
    assert_eq!(
        document.problems[0].message,
        "no link reference definition for 'gone'"
    );
}

#[test]
fn a_retry_after_a_failure_is_not_a_second_problem() {
    // The rewind after a failed attempt re-scans `[missing]` as a
    // shortcut group, which fails the same way; the parse reports the
    // mistake once. Distinct groups further on are distinct mistakes.
    let document = parse_with("[a][missing] and [b][gone]\n", &Options::new());
    let messages: Vec<&str> = document
        .problems
        .iter()
        .map(|problem| problem.message.as_str())
        .collect();
    assert_eq!(
        messages,
        vec![
            "no link reference definition for 'missing'",
            "no link reference definition for 'gone'",
        ]
    );
    // A whole chain of failed groups stays one problem per distinct
    // label: the full reference, then each group the re-scan reaches.
    let chain = parse_with("[a][b][c]\n", &Options::new());
    let messages: Vec<&str> = chain
        .problems
        .iter()
        .map(|problem| problem.message.as_str())
        .collect();
    assert_eq!(
        messages,
        vec![
            "no link reference definition for 'b'",
            "no link reference definition for 'c'",
        ]
    );
}

#[test]
fn defined_references_produce_no_problems() {
    let src = "[text][ref]\n\n[ref]: https://example.com/x\n";
    let document = parse_with(src, &Options::new());
    assert_eq!(
        html_of(src),
        "<p><a href=\"https://example.com/x\">text</a></p>\n"
    );
    assert!(document.problems.is_empty());
}

#[test]
fn inline_links_produce_no_problems() {
    let document = parse_with("[text](https://example.com/x)\n", &Options::new());
    assert!(document.problems.is_empty());
}

#[test]
fn shortcut_reference_without_definition_is_a_problem() {
    let document = parse_with("plain [word] text\n", &Options::new());
    assert_eq!(document.problems.len(), 1);
    assert_eq!(
        document.problems[0].message,
        "no link reference definition for 'word'"
    );
    // The output keeps the literal text: a false alarm in prose stays
    // prose, only the problem records it.
    assert_eq!(html_of("plain [word] text\n"), "<p>plain [word] text</p>\n");
}

/// An extension that counts parses and claims every bracket group as
/// one inline node, to see the problems channel around hooks.
struct Claiming {
    parses: std::sync::atomic::AtomicUsize,
}

impl Extension for Claiming {
    fn name(&self) -> &'static str {
        "claiming"
    }

    fn begin_parse(&self) {
        self.parses
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        ctx.claim(
            "claim".to_string(),
            vec![("k".to_string(), AttrValue::Str("v".to_string()))],
        );
    }
}

#[test]
fn begin_parse_runs_once_per_parse_in_order() {
    let extension = std::sync::Arc::new(Claiming {
        parses: std::sync::atomic::AtomicUsize::new(0),
    });
    let mut options = Options::new();
    options.extension(extension.clone());
    let _ = parse_with("a [b] c\n", &options);
    let _ = parse_with("d [e] f\n", &options);
    assert_eq!(
        extension.parses.load(std::sync::atomic::Ordering::SeqCst),
        2
    );
}

#[test]
fn claimed_groups_produce_no_reference_problems() {
    let mut options = Options::new();
    options.extension(std::sync::Arc::new(Claiming {
        parses: std::sync::atomic::AtomicUsize::new(0),
    }));
    let document = parse_with("a [b] c\n", &options);
    assert!(document.problems.is_empty());
    let tree = debug::print(&document.arena, document.root);
    assert!(tree.contains("claim"), "the group became one node: {tree}");
}

#[test]
fn problems_are_empty_for_clean_documents() {
    let document = parse_with(
        "# Title\n\ntext with [a link](https://example.com/x)\n",
        &Options::new(),
    );
    assert!(document.problems.is_empty());
}
