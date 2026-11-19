//! Round-trip gate: every CommonMark spec example must survive being
//! written back to Markdown and re-parsed, both in rendered HTML and in
//! the writer's own output.

use geist4_core::basic_html::render;
use geist4_core::parser::parse_document;
use geist4_spec_harness::parse as parse_spec;
use geist4_writer::to_common_mark;

/// Returns the example numbers failing HTML round-trip or idempotence, with
/// a short diff-style summary for each.
fn collect_failures() -> Vec<(usize, &'static str, String)> {
    let spec = include_str!("../../../spec/spec.txt");
    let examples = match parse_spec(spec) {
        Ok(examples) => examples,
        Err(error) => panic!("spec.txt must parse: {error}"),
    };
    let mut failures = Vec::new();
    for example in &examples {
        let source = &example.markdown;
        let doc1 = parse_document(source);
        let written1 = to_common_mark(&doc1);
        let html1 = render(&doc1);
        let doc2 = parse_document(&written1);
        let html2 = render(&doc2);
        if html1 != html2 {
            failures.push((
                example.number,
                "html",
                format!(
                    "source:\n{source}\nwritten:\n{written1}\nhtml1:\n{html1}\nhtml2:\n{html2}"
                ),
            ));
            continue;
        }
        let written2 = to_common_mark(&doc2);
        if written2 != written1 {
            failures.push((
                example.number,
                "idempotence",
                format!("written1:\n{written1}\nwritten2:\n{written2}"),
            ));
        }
    }
    failures
}

#[test]
fn spec_examples_round_trip() {
    let spec = include_str!("../../../spec/spec.txt");
    let count = parse_spec(spec).expect("spec parses").len();
    assert_eq!(count, 652, "spec example count drifted");
    let failures = collect_failures();
    if !failures.is_empty() {
        for (number, kind, detail) in &failures {
            eprintln!("example {number} [{kind}]\n{detail}\n---");
        }
        let numbers: Vec<String> = failures.iter().map(|(n, _, _)| n.to_string()).collect();
        panic!(
            "{} failing examples: {}",
            failures.len(),
            numbers.join(", ")
        );
    }
}

/// The writer for `source`, for use in the golden tests below.
fn written(source: &str) -> String {
    to_common_mark(&parse_document(source))
}

#[test]
fn setext_headings_normalize_to_atx() {
    assert_eq!(
        written("Title\n=====\n\nSection\n---\n"),
        "# Title\n\n## Section\n"
    );
}

#[test]
fn bullet_and_ordered_markers_normalize() {
    assert_eq!(written("* a\n+ b\n"), "- a\n\n+ b\n");
    assert_eq!(written("3) a\n4) b\n"), "3. a\n4. b\n");
}

#[test]
fn adjacent_lists_rotate_markers_to_stay_separate() {
    assert_eq!(written("- a\n\n+ b\n\n* c\n"), "- a\n\n+ b\n\n* c\n");
    assert_eq!(written("1. a\n\n1) b\n"), "1. a\n\n1) b\n");
}

#[test]
fn thematic_breaks_normalize() {
    assert_eq!(written("***\n_ _ _\n"), "---\n\n---\n");
}

#[test]
fn fence_sizing_grows_with_inner_runs() {
    assert_eq!(written("```\nplain\n```\n"), "```\nplain\n```\n");
    assert_eq!(written("```\na ``` b\n```\n"), "````\na ``` b\n````\n");
    assert_eq!(written("    indented\n"), "```\nindented\n```\n");
}

#[test]
fn code_spans_keep_their_content() {
    assert_eq!(written("` raw `"), "` raw `\n");
    assert_eq!(written("``two ` ``"), "``two ` ``\n");
    assert_eq!(written("`` `x` ``"), "`` `x` ``\n");
}

#[test]
fn text_escaping_stays_minimal() {
    assert_eq!(written("a * b\n"), "a * b\n");
    assert_eq!(written("\\*a\\*\n"), "\\*a\\*\n");
    assert_eq!(written("2 &lt; 3\n"), "2 \\< 3\n");
    assert_eq!(written("a<br>b\n"), "a<br>b\n"); // inline html is verbatim
}

#[test]
fn nested_emphasis_switches_delimiters() {
    assert_eq!(written("***a***\n"), "***a***\n");
    assert_eq!(written("**_a_**\n"), "**_a_**\n");
    assert_eq!(written("*a **b** c*\n"), "*a **b** c*\n");
}

#[test]
fn reference_definitions_are_reemitted() {
    assert_eq!(
        written("[a][b]\n\n[b]: /u \"t\"\n"),
        "[a](/u \"t\")\n\n[b]: /u \"t\"\n"
    );
}

#[test]
fn empty_containers_survive() {
    assert_eq!(written(">\n"), ">\n");
    assert_eq!(written("-\n\n  foo\n"), "- \n\nfoo\n");
    assert_eq!(written("- foo\n-\n- bar\n"), "- foo\n- \n- bar\n");
}

#[test]
fn thematic_breaks_avoid_marker_and_setext_collisions() {
    // `- ---` is itself a thematic break, so the rule character differs
    // from the marker character.
    assert_eq!(written("- Foo\n- * * *\n"), "- Foo\n- ***\n");
    // After paragraph content with no blank line, `---` would be a setext
    // underline.
    assert_eq!(written("- a\n  ***\n"), "- a\n  ***\n");
}

#[test]
fn heading_content_escapes_closing_sequences() {
    assert_eq!(written("# \\# foo\n"), "# \\# foo\n");
    assert_eq!(written("# a \\#\n"), "# a \\#\n");
    assert_eq!(written("# a#b\n"), "# a#b\n");
}

#[test]
fn heading_content_stays_on_one_line() {
    // A setext heading covering two lines with emphasis spanning them.
    assert_eq!(written("Foo *bar\nbaz*\n===\n"), "# Foo *bar&#10;baz*\n");
    // A hard break inside a two-line setext heading keeps the setext form.
    assert_eq!(written("Foo  \nbar\n===\n"), "Foo  \nbar\n===\n");
}

#[test]
fn unusual_destinations_reparse() {
    assert_eq!(
        written("[a](b%20c\\)d \"ti\\\"tle\")\n"),
        "[a](<b%20c)d> \"ti\\\"tle\")\n"
    );
    assert_eq!(written("[a](x&amp;y)\n"), "[a](x&amp;y)\n");
}

/// Adversarial inputs beyond the spec corpus must also round-trip.
#[test]
fn stress_inputs_round_trip() {
    let inputs = [
        "> - a\n>   - b\n>     - c\n",
        "- > quoted\n> more\n",
        "1. a\n\n   1. b\n\n      1. c\n",
        "- ```\n  code\n  ```\n- after\n",
        "> ```\n> x\n> ```\n",
        "******deep******\n",
        "****both****\n",
        "***nested*** ***pairs***\n",
        "**a *b* c** *d **e** f*\n",
        "*_a_* __b__ _c_\n",
        "[a](<>) [b](< >) [c](x\\ y)\n",
        "[a]: <>\n\n[b]: x \"a\\\"b\"\n\n[c]: &#10;\n",
        "[a\\]b]: u\n\n[a\\]b]\n",
        "|not|a|table|\n",
        "~~~\ntilde ~~~ block\n~~~\n",
        "``` a&#96;b\ninfo with backtick\n```\n",
        "<div>\n\n  <span>x</span>\n\n</div>\n",
        "a  \nb  \n",
        "term\n: def\n",
        "&#35;&#35; not heading\n",
        "1\\. not list\n",
        "\\- not list\n",
        "foo&#10;&#10;bar\n",
        "- \n\n  x\n",
        "> > > deep\n",
        "* * *\n\n---\n\n___\n",
        "[x](y \"t\") *em* `c` <b>r</b>\n",
        "h\u{e9}llo — em\u{2014}dash ✓\n",
        "a\\\nb\n",
        "- a\n  ---\n",
    ];
    for source in inputs {
        let doc1 = parse_document(source);
        let once = to_common_mark(&doc1);
        let html1 = render(&doc1);
        let doc2 = parse_document(&once);
        assert_eq!(
            render(&doc2),
            html1,
            "html round-trip failed for {source:?} (written {once:?})"
        );
        let twice = to_common_mark(&doc2);
        assert_eq!(
            twice, once,
            "idempotence failed for {source:?} (written {once:?})"
        );
    }
}
