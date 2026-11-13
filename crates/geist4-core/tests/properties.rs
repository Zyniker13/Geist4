//! Property tests: no input panics the parser, rendering is
//! deterministic, the emitted Markdown tags balance, and every block
//! span lies inside the input.

use geist4_core::arena::NodeId;
use geist4_core::basic_html::render;
use geist4_core::parser::{Document, parse_document};
use proptest::prelude::*;

/// Renders and returns the output, panicking on none.
fn parse_and_render(src: &str) -> String {
    render(&parse_document(src))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn no_input_panics(src in any::<String>()) {
        let _ = parse_and_render(&src);
    }

    #[test]
    fn rendering_is_deterministic(src in any::<String>()) {
        let once = parse_and_render(&src);
        let twice = parse_and_render(&src);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn markdown_tags_balance(src in any::<String>()) {
        let out = parse_and_render(&src);
        // Raw HTML may be unbalanced by design, so only the tags the
        // renderer emits for Markdown structure are counted.
        for tag in [
            "p", "em", "strong", "a", "li", "ul", "ol", "blockquote",
            "h1", "h2", "h3", "h4", "h5", "h6",
        ] {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            let opens = out.matches(&open).count();
            let closes = out.matches(&close).count();
            let message = format!("unbalanced <{tag}> in {out}");
            prop_assert_eq!(opens, closes, "{}", message);
        }
    }

    #[test]
    fn spans_lie_inside_the_input(src in any::<String>()) {
        let doc = parse_document(&src);
        check_spans(&doc, doc.root, src.len());
    }
}

/// Every node's span lies inside the input, and every non-empty child
/// span lies inside its parent's span. Inline nodes currently carry
/// empty spans (decision 0008), which count as "no span recorded".
fn check_spans(doc: &Document<'_>, id: NodeId, len: usize) {
    let span = doc.arena.span(id);
    assert!(span.start <= span.end);
    assert!(span.end <= len, "span {span:?} outside input of {len}");
    for child in doc.arena.children(id) {
        let child_span = doc.arena.span(*child);
        assert!(child_span.start <= child_span.end);
        assert!(child_span.end <= len);
        let empty = child_span.start == 0 && child_span.end == 0;
        if !empty {
            assert!(
                child_span.start >= span.start && child_span.end <= span.end,
                "child span {child_span:?} outside parent span {span:?}"
            );
        }
        check_spans(doc, *child, len);
    }
}
