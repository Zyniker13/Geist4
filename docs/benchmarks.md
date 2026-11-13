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
