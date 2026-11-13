//! Head-to-head throughput: the same pinned corpus through geist4 and
//! the other CommonMark implementations, all on the criterion harness
//! so the numbers share measurement methodology. The `cmark` rows spawn
//! the reference CLI per iteration; process spawn cost is amortized by
//! the corpus documents' size and noted in docs/benchmarks.md.
//!
//! Run with `cargo bench -p geist4-bench` and record the machine,
//! toolchain and date with the results in docs/benchmarks.md.

// The benchmark APIs of the competitor crates differ from each other;
// this file is comparison glue, not a published API.
#![allow(missing_docs)]

use std::hint::black_box;
use std::path::Path;
use std::process::Command;

use comrak::{Arena, ComrakOptions, parse_document};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pulldown_cmark::{Options, Parser};

const SPEC: &str = include_str!("../../../spec/spec.txt");

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
    out
}

fn geist4_html(text: &str) -> String {
    geist4_core::basic_html::render(&geist4_core::parser::parse_document(text))
}

fn pulldown_html(text: &str) -> String {
    let parser = Parser::new_ext(text, Options::all());
    let mut out = String::with_capacity(text.len());
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

fn comrak_html<'a>(arena: &'a Arena<comrak::nodes::AstNode<'a>>, text: &str) -> String {
    let root = parse_document(arena, text, &ComrakOptions::default());
    let mut out = Vec::with_capacity(text.len());
    comrak::format_html(root, &ComrakOptions::default(), &mut out).expect("html render");
    String::from_utf8_lossy(&out).into_owned()
}

fn cmark_html(text: &str) -> String {
    let mut child = Command::new("cmark")
        .args(["--to", "html", "--width", "0", "--unsafe"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn cmark");
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for cmark");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn group_inputs(c: &mut Criterion, label: &str, inputs: &[(String, String)]) {
    let mut group = c.benchmark_group(label);
    for (name, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        let id = name.rsplit('/').next().unwrap_or(name).to_owned();

        group.bench_with_input(BenchmarkId::new("geist4", &id), text, |b, text| {
            b.iter(|| geist4_html(black_box(text)))
        });
        group.bench_with_input(BenchmarkId::new("pulldown-cmark", &id), text, |b, text| {
            b.iter(|| pulldown_html(black_box(text)))
        });
        group.bench_with_input(BenchmarkId::new("comrak", &id), text, |b, text| {
            b.iter(|| {
                let arena = Arena::new();
                comrak_html(black_box(&arena), black_box(text))
            })
        });
        group.bench_with_input(BenchmarkId::new("cmark cli", &id), text, |b, text| {
            b.iter(|| cmark_html(black_box(text)))
        });
    }
    group.finish();
}

fn bench_compare(c: &mut Criterion) {
    let mut inputs = vec![("spec.txt".to_owned(), SPEC.to_owned())];
    inputs.extend(corpus());
    group_inputs(c, "compare", &inputs);
}

criterion_group!(benches, bench_compare);
criterion_main!(benches);
