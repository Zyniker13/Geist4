# Decision 0012: iai-callgrind for the instruction-count gate

Date: 2026-11-13
Status: accepted

## Context

M4 needs machine-independent instruction counts in CI so a change that
makes the engine slower than a set threshold fails the build. Shared
runners make wall-clock numbers useless for gating. The de facto
standard is `iai-callgrind` (valgrind's callgrind, driven per benchmark
function). Its macro layer depends on `proc-macro-error2`, which is
[RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173):
unmaintained (with a future-incompatibility warning), no vulnerable code
known, no migration yet upstream.

## Decision

- Use `iai-callgrind` 0.14.2 as a dev-dependency of the benchmark-only
  crate `geist4-bench`, with `benches/iai.rs` measuring one
  parse-to-HTML pass over the spec and the pinned corpus.
- CI (ubuntu) installs `iai-callgrind-runner@0.14.2`, checked against
  `Cargo.lock`, runs the bench, and compares instruction counts against
  the checked-in `bench/iai-baseline.txt` via `tools/check_iai.py`
  (fail above baseline × 1.10, or on a benchmark missing from either
  side). Until the baseline is committed, the check runs in bootstrap
  mode (prints counts, passes) so the first numbers can be captured.
- Ignore RUSTSEC-2026-0173 (proc-macro-error2) and RUSTSEC-2025-0141
  (bincode 1.x, used by iai-callgrind to store saved baselines) in
  `deny.toml`: both are compile-time or benchmark-harness-only
  dependencies, not part of any published geist4 artifact. Revisit both
  when iai-callgrind migrates.
- Wall-clock throughput comparisons (`benches/compare.rs`) stay
  local-only and are recorded in `docs/benchmarks.md`.

## Consequences

- The gate is machine-independent and noise-free; a regression above
  10% fails CI with no flakiness.
- `cargo deny check advisories` passes with one documented, scoped
  exception, reviewed on every iai-callgrind upgrade.
- The baseline must be deliberately updated with
  `tools/update_iai_baseline.py`; a stale baseline fails honestly when
  benchmarks are renamed.
