//! Profiling driver: parse and render the pinned benchmark corpus in a
//! loop so a sampling profiler sees the hot paths. Run with
//! `cargo run --release -p geist4-core --example profile -- [seconds]`
//! and sample the process (`sample profile <seconds> -file out.txt`).

use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

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

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(15);
    let text = corpus_text();
    println!("corpus bytes: {}", text.len());
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut rounds = 0u64;
    let mut checksum = 0u64;
    while Instant::now() < deadline {
        let doc = geist4_core::parser::parse_document(&text);
        let html = geist4_core::basic_html::render(&doc);
        if rounds == 0 {
            eprintln!("nodes: {}", doc.arena.len());
        }
        checksum = checksum.wrapping_add(html.len() as u64);
        rounds += 1;
    }
    black_box(checksum);
    println!("rounds: {rounds}, checksum: {checksum}");
}
