# 0042 — Coverage measures with all features; the sandbox lane is the measurement

Date: 2026-11-23

## Context

`geist4-ext-loader`'s `sandbox` and `adapter` modules (the tier-2
WebAssembly path) sit behind the `sandbox` feature. The coverage job ran
`cargo llvm-cov` without `--all-features`, so those modules compiled out
entirely and measured 0% — invisible, not tested. The M9 roadmap item
asked for a decision: measure with `--all-features` (wasmtime compiles on
every coverage run, slower) or add a separate lane.

Measured facts: with the sandbox feature on, the full battery adds
wasmtime to the build (the coverage run goes from ~4 to ~6 minutes) and
the three sandbox test suites (sandbox, adapter, hostile) all run;
`sandbox.rs` measures 89.5%, `adapter.rs` 83.9%, and `package.rs` rises
from 54.8% to 97.1% because its tier-2 branch compiles in.

## Decision

The coverage job (and the TESTING.md measurement command) runs with
`--all-features`. There is no separate lane: one command, one report,
one gate. The gate function in `tools/check_coverage.py` stays
per-crate; `geist4-ext` joins the gated set once its achieved level is
recorded.

Rationale: the sandbox is the security-critical tier — the one place
untrusted code runs — and a measurement that silently skips it would
give the illusion of coverage where it matters most. Two lanes would
mean two numbers to keep honest and a CI step that can rot separately.
The extra two minutes of CI is the whole cost, and it is bounded.

## Consequences

- The coverage numbers in TESTING.md are `--all-features` numbers; the
  pre-0.1.0 numbers in that file were measured without the feature and
  are superseded.
- The loader's line percentage is computed over a larger denominator
  (the gated tier-2 code compiles in), which is the honest denominator.
- wasmtime is already a reviewed dependency (decisions 0027 and 0029,
  the tier-2 sandbox and its binding); no new dependency enters with
  this change.
