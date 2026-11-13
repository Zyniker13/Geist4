//! Runs the vendored CommonMark specification through the core and enforces
//! the pass count recorded in `spec/progress.json`.
//!
//! In milestone M1 the render function is the block-level renderer with
//! plain-text inline content, so only block-structure behavior is measured;
//! the count grows as milestones replace it with the full renderer.

use geist4_core::{basic_html, parse_document};
use geist4_spec_harness::{Example, parse, run};

/// The vendored spec must keep exactly this many examples. See
/// `docs/decisions/0003` for the version policy: upgrading the spec is a
/// decision record and a milestone task, never a silent change.
const SPEC_EXAMPLES: usize = 652;

const SPEC: &str = include_str!("../../../spec/spec.txt");
const PROGRESS: &str = include_str!("../../../spec/progress.json");

/// Reads a `key: number` pair out of `spec/progress.json`. The file is small
/// and fixed in shape, so it is parsed by hand instead of with a JSON
/// dependency.
fn json_number(text: &str, key: &str) -> Option<usize> {
    let needle = format!("\"{key}\"");
    let at = text.find(&needle)? + needle.len();
    let rest = &text[at..];
    let colon = rest.find(':')?;
    let digits: String = rest[colon + 1..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Rewrites `spec/progress.json` with the count just measured. Run locally
/// with `GEIST4_UPDATE_PROGRESS=1 cargo test -p geist4-core`; CI never sets
/// the variable and only ever compares.
fn write_progress(passed: usize) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/progress.json");
    let contents = format!(
        "{{\n  \"spec\": \"CommonMark 0.31.2\",\n  \"total_examples\": {SPEC_EXAMPLES},\n  \"passed\": {passed}\n}}\n"
    );
    std::fs::write(path, contents).expect("writing spec/progress.json");
}

#[test]
fn commonmark_spec_progress() {
    let examples = parse(SPEC).expect("the vendored spec.txt must parse");
    assert_eq!(
        examples.len(),
        SPEC_EXAMPLES,
        "the vendored CommonMark 0.31.2 spec holds 652 examples; if the spec \
         was upgraded, that is a decision record plus an update to this \
         constant and to spec/progress.json"
    );

    let mut render =
        |example: &Example| -> String { basic_html::render(&parse_document(&example.markdown)) };
    let report = run(&examples, &mut render);

    let recorded_total = json_number(PROGRESS, "total_examples")
        .expect("spec/progress.json must record total_examples");
    assert_eq!(recorded_total, SPEC_EXAMPLES);

    if std::env::var_os("GEIST4_UPDATE_PROGRESS").is_some() {
        write_progress(report.passed());
        println!(
            "updated spec/progress.json to passed {} of {}",
            report.passed(),
            SPEC_EXAMPLES
        );
        return;
    }

    let floor = json_number(PROGRESS, "passed").expect("spec/progress.json must record passed");
    assert!(
        report.passed() >= floor,
        "the CommonMark pass count went down: {} is below the recorded \
         floor of {}. Counts never go down; if a change is intentional, it \
         is a decision record and the floor changes with it.\n{report}",
        report.passed(),
        floor,
    );
    assert_eq!(
        report.passed(),
        floor,
        "spec/progress.json is stale: the suite now passes {} of {}. \
         Update the file with: GEIST4_UPDATE_PROGRESS=1 cargo test -p geist4-core\n{report}",
        report.passed(),
        SPEC_EXAMPLES,
    );
}
