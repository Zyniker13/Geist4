//! Instruction-count benchmarks: the pinned corpus and the spec through
//! one parse-to-HTML pass each, counted with valgrind's callgrind so the
//! numbers are machine-independent. CI compares the instruction counts
//! against the checked-in baseline (`bench/iai-baseline.txt`); a change
//! that exceeds the threshold fails the build. Wall-clock throughput is
//! measured by `benches/compare.rs` instead.
//!
//! iai-callgrind needs valgrind and runs only on Linux; elsewhere the
//! harness exits without running. Run on Linux with:
//! `cargo bench -p geist4-bench --bench iai`
//! and update the baseline with `tools/update_iai_baseline.py` after a
//! deliberate, reviewed performance change.

// Comparison-glue file, not a published API; the argument expressions
// run outside valgrind's counting window, so corpus loading is excluded
// from the measured instruction counts.
#![allow(missing_docs)]

use std::path::Path;

use geist4_core::basic_html::render;
use geist4_core::parser::parse_document;
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};

const SPEC: &str = include_str!("../../../spec/spec.txt");

fn corpus_text() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/corpus");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("the benchmark corpus exists (see tools/gen_bench_corpus.py)")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort();
    let mut out = String::new();
    for path in entries {
        out.push_str(&std::fs::read_to_string(&path).expect("read corpus file"));
        out.push('\n');
    }
    out
}

// The return value keeps the renderer from being optimized away; the
// instruction count is the metric, not the returned length.
#[library_benchmark]
#[bench::spec()]
fn spec_parse_to_html() -> usize {
    black_box(render(&parse_document(SPEC)).len())
}

#[library_benchmark]
#[bench::corpus(corpus_text())]
fn corpus_parse_to_html(text: String) -> usize {
    black_box(render(&parse_document(&text)).len())
}

library_benchmark_group!(
    name = parse_to_html_group;
    benchmarks = spec_parse_to_html, corpus_parse_to_html
);

main!(library_benchmark_groups = parse_to_html_group);
