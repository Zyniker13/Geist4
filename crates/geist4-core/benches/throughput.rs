//! The throughput benchmarks: parsing and the full pipeline over a
//! fixed corpus. The corpus is pinned — the CommonMark specification
//! itself and the generated documents under `bench/corpus/`, whose
//! checksums live in `bench/corpus/MANIFEST.sha256` — so numbers stay
//! comparable across machines and commits. Wall-clock numbers are for
//! local use; CI's regression gate uses instruction counts instead
//! (see docs/benchmarks.md).

// criterion's macros expand undocumented items; benches are not a
// published API, so the missing-docs lint does not apply here.
#![allow(missing_docs)]

use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};

const SPEC: &str = include_str!("../../../spec/spec.txt");

/// Reads the generated corpus files next to the repository root.
fn corpus() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/corpus");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("the benchmark corpus exists (see tools/gen_bench_corpus.py)")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort();
    for path in entries {
        let text = std::fs::read_to_string(&path).expect("read corpus file");
        out.push((path.display().to_string(), text));
    }
    assert!(!out.is_empty(), "the corpus is not empty");
    out
}

fn bench_spec(c: &mut Criterion) {
    let mut group = c.benchmark_group("spec");
    group.throughput(criterion::Throughput::Bytes(SPEC.len() as u64));
    group.bench_function("parse", |b| {
        b.iter(|| geist4_core::parser::parse_document(black_box(SPEC)))
    });
    group.bench_function("parse and render html", |b| {
        b.iter(|| {
            let document = geist4_core::parser::parse_document(black_box(SPEC));
            geist4_core::basic_html::render(black_box(&document))
        })
    });
    group.finish();
}

fn bench_corpus(c: &mut Criterion) {
    let mut group = c.benchmark_group("corpus");
    for (name, text) in corpus() {
        let id = name.rsplit('/').next().unwrap_or(&name).to_owned();
        group.throughput(criterion::Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            criterion::BenchmarkId::new("parse and render html", id),
            &text,
            |b, text| {
                b.iter(|| {
                    let document = geist4_core::parser::parse_document(black_box(text));
                    geist4_core::basic_html::render(black_box(&document))
                })
            },
        );
    }
    group.finish();
}

fn bench_pathological(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathological");
    // Fixed-size representatives of the pathological families; the
    // sizes are small so the suite stays quick, and linear scaling is
    // asserted by tests/pathological.rs, not here.
    let nested_emphasis = format!("{}a{}", " *".repeat(1_000), "* ".repeat(1_000));
    let nested_quotes = format!("{}{}", ">".repeat(1_000), "a*".repeat(1_000));
    let unclosed_brackets = "[".repeat(2_000);
    for (name, text) in [
        ("nested emphasis", &nested_emphasis[..]),
        ("nested quotes", &nested_quotes[..]),
        ("unclosed brackets", &unclosed_brackets[..]),
    ] {
        group.throughput(criterion::Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            criterion::BenchmarkId::new("parse and render html", name),
            text,
            |b, text| {
                b.iter(|| {
                    let document = geist4_core::parser::parse_document(black_box(text));
                    geist4_core::basic_html::render(black_box(&document))
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_spec, bench_corpus, bench_pathological);
criterion_main!(benches);
