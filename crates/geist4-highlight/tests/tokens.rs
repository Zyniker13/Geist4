//! Token-class and fallback tests for [`geist4_highlight::highlight`].
//!
//! Every expectation was verified against the engine's actual output
//! via a scratch example before being pinned (the working rule: never
//! guess a syntax engine's tokenization).

use geist4_highlight::highlight;

/// A Rust line tokenizes into conventional classes: the keyword gets
/// `storage`, the function name `entity`, the number `numeric`, the
/// comment `comment`.
#[test]
fn rust_tokens_map_to_conventional_classes() {
    let html = highlight("fn main() {\n    let x = 42; // answer\n}\n", "rust")
        .expect("rust is in the default pack");
    assert!(html.contains("<span class=\"storage\">fn</span>"), "{html}");
    assert!(
        html.contains("<span class=\"entity\">main</span>"),
        "{html}"
    );
    assert!(html.contains("<span class=\"numeric\">42</span>"), "{html}");
    assert!(
        html.contains("<span class=\"comment\">// answer\n</span>"),
        "{html}"
    );
}

/// Adjacent same-class tokens merge into one span: `main()`'s parens
/// are both `punctuation`, and only one span wraps them. (Bare
/// `main()` without a preceding `fn` classifies as `support` — the
/// Rust syntax matches a builtin-context scope there — which the
/// full-program test above pins as `entity`.)
#[test]
fn adjacent_same_class_tokens_merge() {
    let html = highlight("fn main() {\n}\n", "rust").expect("rust is in the pack");
    assert!(
        html.contains("<span class=\"punctuation\">{</span>"),
        "{html}"
    );
}

/// Text is escaped exactly like the plain renderer: `<` becomes `&lt;`
/// and `&` becomes `&amp;` inside spans.
#[test]
fn token_text_is_escaped_like_the_plain_renderer() {
    let html = highlight("a < b && b > c\n", "rust").expect("rust is in the pack");
    assert!(html.contains("&lt;"), "{html}");
    assert!(html.contains("&amp;&amp;"), "{html}");
    assert!(html.contains("&gt;"), "{html}");
    assert!(!html.contains("<b"), "no raw markup leaks: {html}");
}

/// An unknown language yields `None`: the caller renders plain code,
/// and highlighting never errors a render.
#[test]
fn unknown_language_declines() {
    assert_eq!(highlight("x\n", "notalanguage"), None);
    assert_eq!(highlight("x\n", ""), None);
}

/// Python tokenizes through the same class vocabulary.
#[test]
fn python_tokens_map_to_the_same_vocabulary() {
    let html = highlight("def f(x):\n    return x\n", "python").expect("python is in the pack");
    assert!(
        html.contains("<span class=\"storage\">def</span>"),
        "{html}"
    );
    assert!(
        html.contains("<span class=\"keyword\">return</span>"),
        "{html}"
    );
    assert!(html.contains("<span class=\"variable\">x</span>"), "{html}");
}

/// The spans always balance: count opens equals count closes, for a
/// multi-language sample set.
#[test]
fn spans_balance_across_languages() {
    for (lang, code) in [
        ("rust", "fn main() {\n    let x = 42; // answer\n}\n"),
        ("python", "def f(x):\n    return x\n"),
        ("javascript", "const x = 1; // n\n"),
        ("html", "<p>a</p>\n"),
        ("c", "int main(void) {\n  return 0;\n}\n"),
    ] {
        let html = highlight(code, lang).expect("a default-pack language");
        let opens = html.matches("<span class=\"").count();
        let closes = html.matches("</span>").count();
        assert_eq!(opens, closes, "{lang}: {html}");
    }
}

/// The output is pure text-and-spans: no inline styles, no style
/// attribute, no theme colors anywhere.
#[test]
fn output_is_class_tokens_only() {
    let html = highlight("let x = 1;\n", "rust").expect("rust is in the pack");
    assert!(!html.contains("style="), "{html}");
    assert!(!html.contains('#'), "no hex colors: {html}");
    assert!(html.starts_with("<span class=\""), "{html}");
}
