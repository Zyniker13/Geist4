# Benchmarks

Throughput comparisons between geist4 and the other CommonMark
implementations, all run through the same criterion harness
(`cargo bench -p geist4-bench --bench compare`). The corpus is the
checked-in generated document set under `bench/corpus/` (40 documents,
checksums in `bench/corpus/MANIFEST.sha256`) plus `spec/spec.txt`.
The competitor numbers use each library's default feature set
(`pulldown-cmark` with all extensions on, `comrak` with default
options); `cmark` rows spawn the reference CLI per iteration, so they
are dominated by process spawn cost and serve only as a floor.

Results are medians over 100 samples per document; the summary number
is the geometric mean of the per-document throughputs.

## Results

### 2026-11-13 — first recorded run (before M4 optimization)

- Machine: Apple M6, 12 cores, macOS 27.2
- Toolchain: rustc 1.99.1, release profile, criterion 0.7
- Repository state: `543f61d3`

| implementation   | geomean throughput |
| ---------------- | ------------------ |
| pulldown-cmark   | 517.3 MiB/s        |
| comrak           | 174.6 MiB/s        |
| geist4           | 131.5 MiB/s        |
| cmark (CLI)      | 2.7 MiB/s          |

geist4 is 3.9x behind pulldown-cmark on this machine. The M4 work that
follows this baseline: profile-guided removal of per-node allocations,
byte-scanning with memchr, and branch-light hot paths, with the spec
suite required to stay at 652 of 652 after every change.

Per-document numbers for this run are in the criterion output; the
throughput gate for CI is the instruction-count benchmark
(`benches/iai.rs`), not these wall-clock numbers.

### 2026-11-13 — after the M4 optimization rounds

- Machine: Apple M6, 12 cores, macOS 27.2
- Toolchain: rustc 1.99.1, release profile, criterion 0.7
- Repository state: `c010b013`

| implementation   | geomean throughput |
| ---------------- | ------------------ |
| pulldown-cmark   | 540.8 MiB/s        |
| geist4           | 210.4 MiB/s        |
| comrak           | 192.7 MiB/s        |
| cmark (CLI)      | 2.8 MiB/s          |

The optimization rounds (arena pre-reserve, definition-scan guard,
inline buffer reuse with a generation-tagged memo, batched HTML
escaping, borrowed source slices, direct pool-to-arena attachment,
reusable label and join buffers) took geist4 from 131.5 to 210.4
MiB/s — a 1.60x improvement, now ahead of comrak and 2.6x behind
pulldown-cmark. Decision 0013 records the approach and the reasons
the remaining gap is not closed.

## Incremental updates (M11)

What an editor keystroke costs in `geist4-incremental`, measured as a
chained editing session (`crates/geist4-incremental/benches/update.rs`):
16 single-paragraph rewrites in the middle of a paragraph-only
document, each update building on the previous document; the baseline
is a full `parse_with` over the same source chain. Criterion, 100
samples, medians. The differential corpus
(`tests/differential.rs`, 24 tests) proves the incremental result is
value-identical to the full parse; the benchmark measures only cost.

### 2026-11-23 — after the checkpoint stage (`a676b589`)

- Machine: Apple M6, 12 cores, macOS 27.2
- Toolchain: rustc 1.99.1, bench profile, criterion 0.7
- Design: full-subtree checkpoint clones per update, reparse from the
  first changed block to end-of-document

| benchmark          | median  |
| ------------------ | ------- |
| update-chain-200   | 1.0283 ms |
| full-chain-200     | 0.7550 ms |
| update-chain-2000  | 10.130 ms |
| full-chain-2000    | 7.5382 ms |

Per update (16 steps): 64.3 µs vs 47.2 µs (200-paragraph, ~10 KiB) and
633.1 µs vs 471.1 µs (2000-paragraph, ~100 KiB) — the incremental path
was 1.35x *slower* than a full parse. Blame: `checkpoint_from`
snapshot-cloned every top-level block's subtree on every call.

### 2026-11-23 — after the metadata-checkpoint + copied-tail stage (`9d1d7ffa`)

- Machine: Apple M6, 12 cores, macOS 27.2
- Toolchain: rustc 1.99.1, bench profile, criterion 0.7
- Design: metadata-only checkpoint (span, root id, continuation flag)
  built once per parse; blocks mount from the previous arena; the
  reparse window stops at the first hard block boundary after the
  edit, and the unchanged tail mounts with shifted spans

| benchmark          | median  | vs full |
| ------------------ | ------- | ------- |
| update-chain-200   | 0.3850 ms | 2.24x faster |
| full-chain-200     | 0.8623 ms | — |
| update-chain-2000  | 3.7486 ms | 2.27x faster |
| full-chain-2000    | 8.5201 ms | — |

Per update: 24.1 µs vs 53.9 µs and 234.3 µs vs 532.5 µs. The update
itself is 2.67x (200) and 2.70x (2000) faster than in the checkpoint
stage. The update-vs-full ratio flipped from 0.74x to ~2.25x between
the stages; the reparse-window change contributes more than the
checkpoint change at 2000 paragraphs because the window stops the
reparse at the first blank-line-bounded boundary after the edit while
the prefix copy shrinks to the same span. The instruction-count CI
gate (`benches/iai.rs`) is unchanged by this work.
