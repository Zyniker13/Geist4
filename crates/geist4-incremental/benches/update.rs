//! The incremental-update benchmarks: what an editor keystroke costs
//! against a full reparse. The scenario is the editor's dominant one —
//! a single paragraph line replaced in the middle of a large document —
//! run as a chain, so each update builds on the previous document the
//! way a real editing session does. The full-parse baseline is
//! `parse_with` on the same source chain. Both paths must produce the
//! same observable document; the differential corpus proves that
//! equality separately (tests/differential.rs).

// criterion's macros expand undocumented items; benches are not a
// published API, so the missing-docs lint does not apply here.
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_incremental::Editor;

/// Builds a document of `paragraphs` paragraphs, each a few words on
/// one line, separated by blank lines.
fn build(paragraphs: usize) -> String {
    let mut out = String::new();
    for index in 0..paragraphs {
        out.push_str("Paragraph the ");
        let mut digits = [0u8; 20];
        let mut len = 0;
        let mut value = index;
        if value == 0 {
            digits[0] = b'0';
            len = 1;
        }
        while value > 0 {
            digits[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        for byte in digits[..len].iter().rev() {
            out.push(*byte as char);
        }
        out.push_str(" in a long document, holding text for the benchmark.\n\n");
    }
    out
}

/// Rewrites paragraph `index`'s line in `source` (the blank line after
/// it is kept), returning the new source string. The edit changes
/// exactly one line, in the middle of the document.
fn rewrite(source: &str, index: usize) -> String {
    let mut out = String::with_capacity(source.len());
    for (line_index, line) in source.split_inclusive('\n').enumerate() {
        if line_index == index * 2 {
            out.push_str("Paragraph the rewritten line number ");
            out.push_str(&index.to_string());
            out.push_str(" now holds different benchmark text.\n");
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Chains `steps` single-paragraph rewrites through `update`, returning
/// nothing: the work is the measurement. Each step keeps the previous
/// document, as an editor session does. The whole source chain is
/// pre-generated so every document can borrow its source for the
/// entire run (the API ties the returned document to its source).
fn chain_update(editor: &Editor, options: &Options, source: &str, steps: usize, span: usize) {
    let sources = chain_sources(source, steps, span);
    let mut previous = parse_with(&sources[0], options);
    for window in sources.windows(2) {
        let (document, changed) = editor.update(&previous, &window[0], &window[1]);
        black_box(&document);
        black_box(&changed);
        previous = document;
    }
}

/// The baseline: the same source chain through full parses only.
fn chain_full(options: &Options, source: &str, steps: usize, span: usize) {
    let sources = chain_sources(source, steps, span);
    for source in &sources {
        let document = parse_with(source, options);
        black_box(&document);
    }
}

/// Pre-generates the edit chain: `sources[0]` is the pristine document,
/// each next source rewrites one more paragraph in the middle region.
fn chain_sources(source: &str, steps: usize, span: usize) -> Vec<String> {
    let mut sources = Vec::with_capacity(steps + 1);
    sources.push(source.to_string());
    for step in 0..steps {
        let index = span / 2 + step;
        let next = rewrite(sources.last().expect("at least one source"), index);
        sources.push(next);
    }
    sources
}

fn update_vs_full(criterion: &mut Criterion) {
    let options = Options::new();
    let editor = Editor::new(options.clone());
    // 10 KiB-ish and 100 KiB-ish documents; 16 chained keystrokes
    // covers an editing burst without stretching the benchmark run.
    for paragraphs in [200, 2_000] {
        let source = build(paragraphs);
        let label = format!("update-chain-{paragraphs}");
        let span = paragraphs;
        let steps = 16;
        criterion.bench_function(&label, |bencher| {
            bencher.iter(|| {
                chain_update(
                    black_box(&editor),
                    black_box(&options),
                    black_box(&source),
                    steps,
                    span,
                )
            });
        });
        let full_label = format!("full-chain-{paragraphs}");
        criterion.bench_function(&full_label, |bencher| {
            bencher.iter(|| chain_full(black_box(&options), black_box(&source), steps, span));
        });
    }
}

criterion_group!(benches, update_vs_full);
criterion_main!(benches);
