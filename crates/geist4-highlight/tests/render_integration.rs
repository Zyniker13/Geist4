//! Renderer-integration tests for the highlighting option: the
//! differential check (highlighting off = byte-identical to plain) and
//! the on-mode output shape.

use geist4_core::basic_html::{escape_html, render_with};
use geist4_core::extension::{CodeHighlighter, Options};
use geist4_highlight::SyntaxHighlighter;

/// A fenced block through the plain path, for differential comparison.
fn plain_html(src: &str) -> String {
    render_with(
        &geist4_core::parser::parse_with(src, &Options::new()),
        &Options::new(),
    )
}

/// The same document parsed and rendered with highlighting registered:
/// the on mode.
fn highlighted_html(src: &str) -> String {
    let mut options = Options::new();
    options.highlighter(std::sync::Arc::new(SyntaxHighlighter));
    render_with(&geist4_core::parser::parse_with(src, &options), &options)
}

/// The differential check: with no highlighter registered — the
/// default — every document in a mixed sample renders byte-identically
/// to plain CommonMark, the spec-suite guarantee the roadmap demands.
#[test]
fn off_matches_plain_byte_for_byte() {
    for src in [
        "```rust\nfn main() {}\n```\n",
        "```\nplain fence\n```\n",
        "    indented code\n",
        "# heading\n\n```python\nx = 1\n```\n\ntail\n",
        "```notalanguage\nmystery\n```\n",
    ] {
        let options = Options::new();
        let html = render_with(&geist4_core::parser::parse_with(src, &options), &options);
        assert_eq!(html, plain_html(src), "off must equal plain: {src}");
    }
}

/// With highlighting on, a fenced block with a known language emits
/// class spans inside the same `<pre><code>` wrapper the plain path
/// uses — the info-string class and the wrapper are unchanged.
#[test]
fn on_mode_wraps_tokens_in_the_same_pre_code() {
    let html = highlighted_html("```rust\nlet x = 42;\n```\n");
    assert!(
        html.starts_with("<pre><code class=\"language-rust\">"),
        "{html}"
    );
    assert!(html.contains("<span class=\"numeric\">42</span>"), "{html}");
    assert!(html.ends_with("</code></pre>\n"), "{html}");
}

/// An unknown language with highlighting on falls back to plain escaped
/// code — the same bytes the off mode produces for that block.
#[test]
fn on_mode_unknown_language_is_plain() {
    let on = highlighted_html("```mysterylang\na < b\n```\n");
    let off = plain_html("```mysterylang\na < b\n```\n");
    assert_eq!(on, off, "unknown language must fall back to plain");
    assert!(on.contains("&lt;"), "still escaped: {on}");
}

/// An unfenced (indented) code block never highlights, even with the
/// option on.
#[test]
fn on_mode_indented_code_stays_plain() {
    let src = "    let x = 42;\n";
    assert_eq!(highlighted_html(src), plain_html(src));
}

/// A highlighter that declines everything behaves like no highlighter
/// at all — the trait contract, exercised through the renderer.
#[test]
fn a_declining_highlighter_is_transparent() {
    struct Decline;
    impl CodeHighlighter for Decline {
        fn highlight(&self, _code: &str, _language: &str) -> Option<String> {
            None
        }
    }
    let mut options = Options::new();
    options.highlighter(std::sync::Arc::new(Decline));
    let src = "```rust\nfn main() {}\n```\n";
    let html = render_with(&geist4_core::parser::parse_with(src, &options), &options);
    assert_eq!(html, plain_html(src));
    assert!(html.contains(&escape_html("fn main() {}\n")), "{html}");
}

/// A fence whose info string is empty (` ``` ` alone) stays plain.
#[test]
fn on_mode_empty_info_stays_plain() {
    let src = "```\nplain\n```\n";
    assert_eq!(highlighted_html(src), plain_html(src));
}

/// The second word of the info string (` ```rust ignore `) does not
/// change the language token: the class keeps the full info string the
/// way the plain renderer writes it, while the highlighter sees only
/// `rust`.
#[test]
fn on_mode_uses_only_the_first_info_word() {
    let html = highlighted_html("```rust ignore\nlet x = 42;\n```\n");
    assert!(
        html.starts_with("<pre><code class=\"language-rust\">"),
        "{html}"
    );
    assert!(html.contains("<span class=\"numeric\">42</span>"), "{html}");
}
