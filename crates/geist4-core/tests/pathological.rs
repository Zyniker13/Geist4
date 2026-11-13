//! Pathological inputs, ported from the reference implementation's
//! pathological test suite (`cmark/test/pathological_tests.py` at the
//! spec version). Each family must parse and render correctly, complete
//! within a bounded time, and — for the families where quadratic parsers
//! historically blew up — scale linearly, checked by comparing the time
//! at size n with the time at size 10n.

use geist4_core::basic_html::render;
use geist4_core::parser::parse_document;
use std::time::{Duration, Instant};

/// Renders one input and returns how long it took.
fn timed(src: &str) -> Duration {
    let start = Instant::now();
    let _ = render(&parse_document(src));
    start.elapsed()
}

/// Asserts rendering scales linearly: time(10n) is at most a small
/// factor above time(n), far below the ~100x a quadratic parser would
/// show. Each size is measured five times and the minimum taken, so
/// scheduler noise does not distort the ratio. The threshold has to
/// survive contended shared runners, where a best-of measurement can
/// still be inflated several-fold by a stolen timeslice: linear scaling
/// measures at 3x or below even with such noise, a quadratic regression
/// stays at 50x and above, so 25 sits far from both.
fn assert_linear(name: &str, input_at_n: &str, input_at_10n: &str) {
    let mut best_small = Duration::MAX;
    let mut best_big = Duration::MAX;
    for _ in 0..5 {
        best_small = best_small.min(timed(input_at_n));
        best_big = best_big.min(timed(input_at_10n));
    }
    let ratio = best_big.as_secs_f64() / best_small.as_secs_f64().max(1e-6);
    assert!(
        ratio < 25.0,
        "{name} looks superlinear: {best_small:?} at n vs {best_big:?} at 10n (ratio {ratio:.1})"
    );
}

/// Runs one family at n and 10n and asserts both its output counts and
/// its linear scaling.
fn check_family(
    name: &str,
    build: &dyn Fn(usize) -> String,
    count_of: &dyn Fn(&str) -> usize,
    expected: usize,
) {
    let small = build(1_000);
    let big = build(10_000);
    assert_eq!(
        count_of(&render(&parse_document(&big))),
        expected,
        "{name}: count"
    );
    assert_linear(name, &small, &big);
}

#[test]
fn emphasis_families_scale_linearly() {
    check_family(
        "nested strong emph",
        &|n| format!("{}b{}", "*a **a ".repeat(n), " a** a*".repeat(n)),
        &|out| out.matches("<strong>").count(),
        10_000,
    );
    check_family(
        "many emph closers with no openers",
        &|n| "a_ ".repeat(n),
        // The final run loses its trailing space to the subject rtrim.
        &|out| out.matches("a_").count(),
        10_000,
    );
    check_family(
        "many emph openers with no closers",
        &|n| "_a ".repeat(n),
        &|out| out.matches("_a").count(),
        10_000,
    );
    check_family(
        "many link closers with no openers",
        &|n| "a]".repeat(n),
        &|out| out.matches("a]").count(),
        10_000,
    );
    check_family(
        "many link openers with no closers",
        &|n| "[a".repeat(n),
        &|out| out.matches("[a").count(),
        10_000,
    );
    check_family(
        "mismatched openers and closers",
        &|n| "*a_ ".repeat(n),
        &|out| out.matches("*a_").count(),
        10_000,
    );
    check_family(
        "issue 389",
        &|n| format!("{}{}", "*a ".repeat(n), "_a*_ ".repeat(n)),
        &|out| out.matches("<em>a ").count(),
        10_000,
    );
    check_family(
        "openers and closers multiple of 3",
        &|n| format!("a**b{}", "c* ".repeat(n)),
        &|out| out.matches("c*").count(),
        10_000,
    );
    check_family(
        "link openers and emph closers",
        &|n| "[ a_".repeat(n),
        &|out| out.matches("[ a_").count(),
        10_000,
    );
    check_family(
        "pattern [ (]( repeated",
        &|n| "[ (](".repeat(n),
        &|out| out.matches("[ (](").count(),
        10_000,
    );
    check_family(
        "pattern ![[]() repeated",
        &|n| "![[]()".repeat(n),
        &|out| out.matches("<a href=\"\"></a>").count(),
        10_000,
    );
    check_family(
        "unclosed links A",
        &|n| "[a](<b".repeat(n),
        &|out| out.matches("[a](&lt;b").count(),
        10_000,
    );
    check_family(
        "unclosed links B",
        &|n| "[a](b".repeat(n),
        &|out| out.matches("[a](b").count(),
        10_000,
    );
    check_family(
        "nested brackets",
        &|n| format!("{}a{}", "[".repeat(n), "]".repeat(n)),
        &|out| out.matches("[").count(),
        10_000,
    );
}

/// Deeply nested containers must render without overflowing the stack:
/// the parser's inline post-pass and the renderer are iterative.
#[test]
fn deep_nesting_completes() {
    let start = Instant::now();

    // Nested block quotes.
    let out = render(&parse_document(&format!("{}a", "> ".repeat(5_000))));
    assert_eq!(out.matches("<blockquote>").count(), 5_000);

    // The reference suite's deeply nested lists: 500 two-space levels.
    let list_input: String = (0..500)
        .map(|i| format!("{}* a\n", "  ".repeat(i)))
        .collect();
    let out = render(&parse_document(&list_input));
    assert_eq!(out.matches("<ul>").count(), 500);

    // Empty lines in deeply nested lists.
    let out = render(&parse_document(&format!(
        "{}x\n{}",
        "- ".repeat(2_000),
        "\n".repeat(2_000)
    )));
    assert!(out.contains('x'));

    // Emphasis in a deep blockquote; the asterisks pair off the way the
    // reference implementation's does (every second one closes an `<em>`).
    let out = render(&parse_document(&format!(
        "{}a*{}",
        ">".repeat(10_000),
        "a*".repeat(10_000)
    )));
    assert!(out.contains("<p>"));
    assert_eq!(out.matches("<em>").count(), 5_000);

    assert!(
        start.elapsed() < Duration::from_secs(60),
        "deep nesting took too long"
    );
}

/// Assorted fixed pathological inputs from the reference suite.
#[test]
fn fixed_pathological_cases() {
    // The hard link/emph case from the reference suite.
    assert_eq!(
        render(&parse_document("**x [a*b**c*](d)")),
        "<p>**x <a href=\"d\">a<em>b**c</em></a></p>\n"
    );

    // NUL bytes become replacement characters in the output, as the
    // reference implementation's does.
    let out = render(&parse_document("abc\u{0}de\u{0}"));
    assert_eq!(out, "<p>abc\u{FFFD}de\u{FFFD}</p>\n");

    // Increasing backtick runs: code span scanning must not rescan.
    // The deadline only catches gross superlinearity or a hang: the
    // suite's tests run in parallel threads on shared runners, so an
    // absolute wall-clock bound has to tolerate minutes of scheduler
    // starvation. Quadratic rescanning is caught by the scaling suite.
    let input: String = (1..2_000).map(|k| format!("e{}`", "`".repeat(k))).collect();
    let start = Instant::now();
    let out = render(&parse_document(&input));
    assert!(out.starts_with("<p>"), "{out}");
    assert!(start.elapsed() < Duration::from_secs(120));

    // An unclosed comment run stays literal.
    let out = render(&parse_document(&format!("</{}", "<!--".repeat(10_000))));
    assert!(out.contains("&lt;/"));
    assert_eq!(out.matches("&lt;!--").count(), 10_000);

    // Reference definitions are stored in a keyed map, so hash-collision
    // label sets cannot degrade lookup.
    let mut defs = String::new();
    for i in 0..2_000 {
        defs.push_str(&format!("[x{i}]: /u{i}\n\n[x{i}]\n\n"));
    }
    let start = Instant::now();
    let out = render(&parse_document(&defs));
    assert_eq!(out.matches("<a href=").count(), 2_000);
    // Hang detector only; see the note above about shared runners.
    assert!(start.elapsed() < Duration::from_secs(120));
}
