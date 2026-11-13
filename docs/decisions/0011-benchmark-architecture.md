# 0011 — Benchmark architecture: separate bench crate, local wall clock, CI instruction counts

## Context

M4 requires throughput comparisons against `pulldown-cmark`, `comrak`
and `cmark`, an instruction-count regression gate in CI, and a corpus
pinned well enough that numbers stay comparable across machines and
commits. Benchmark-only dependencies (competitor parsers, criterion,
iai-callgrind) must not leak into the published crates or weigh down
every development build.

## Decision

1. **Geist4's own numbers** live in `crates/geist4-core/benches/`
   (`criterion`, `harness = false`), over the pinned corpus: the
   CommonMark specification itself and the generated documents under
   `bench/corpus/` (SHA-256 manifest, regenerated as a no-op by
   `tools/gen_bench_corpus.py`). The benchmark reports throughput
   (bytes per second) for parse-only and parse-plus-render.
2. **Competitor comparisons** live in a separate workspace member,
   `crates/geist4-bench`, that is never published; it depends on the
   competitor crates so they never enter the other crates' dependency
   graphs. Results go to `docs/benchmarks.md` with machine, toolchain
   and date, and are local-only numbers.
3. **CI runs instruction-count benchmarks** (`iai-callgrind`, Linux
   only) over a small fixed input set, with thresholds that fail the
   build on a significant regression. Wall-clock benchmarks never gate
   CI: shared runners are too noisy, as the M3 scaling-threshold work
   demonstrated.
4. The pathological families are benchmarked at small fixed sizes as a
   smoke metric; their scaling property is asserted by the M3 test
   suite, not by benchmark timing.

## Consequences

- `cargo bench` in `geist4-core` needs only `criterion`; the
  competitor comparison is an explicit second step (`cargo run -p
  geist4-bench --release`), keeping competitor churn out of the main
  build.
- `iai-callgrind` needs nightly and valgrind in CI; that job is Linux
  and allowed to be slow, and its thresholds are set from measured
  baselines, not guesses.
- The corpus checksum manifest makes benchmark regressions
  distinguishable from corpus drift: an input change is a reviewable
  diff like any other generated file (decision 0009's rule applies).
