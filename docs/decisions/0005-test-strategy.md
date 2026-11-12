# 0005 — Test strategy

## Context
The engine must be correct (CommonMark 0.31.2, GFM extensions), secure (no panics on untrusted input), and fast. Testing must catch regressions early.

## Decision
Three test layers:
1. **Spec suites** — the CommonMark 0.31.2 examples (652) and each extension's spec file, run through the spec harness. These are the gold standard; no example is skipped or special-cased.
2. **Unit tests** — per-block and per-inline unit tests for tricky rules (lazy continuation, list indentation, delimiter algorithm edge cases).
3. **Fuzzing and property tests** — `cargo-fuzz` for the parser and renderer, `proptest` for no-panic and well-formed output guarantees.

Every bug fix starts with a regression test. The `spec/progress.json` file tracks the CommonMark pass count; CI fails if it goes down.

## Consequences
Test coverage is measured with `cargo-llvm-cov` (target > 95% for core and html crates). Mutation testing with `cargo-mutants` is run at M8. The test strategy is detailed in `TESTING.md`.
