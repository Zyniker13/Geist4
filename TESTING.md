# Testing

The engine is meant to be absolutely reliable, so testing is a first-class part of the design, not a step at the end. This file describes every test suite, what it proves, and how to run it. Keep it true: when a suite is added, changed, or moved, update this file in the same commit.

## The Rules

1. **Every behavior has a test.** A rule from a spec has a spec example; a rule we decide ourselves has a unit test and a decision record.
2. **Every bug gets a regression test** in `tests/regressions/` before it is fixed, with a comment saying what it caught and which commit fixed it.
3. **Counts never go down.** The CommonMark pass count, each extension's pass count, and coverage are checked in and enforced by CI.
4. **Local and CI are the same.** `scripts/check.sh` runs exactly what CI runs. A change is pushed only after it passes there.
5. **No test is skipped to get green.** A test that fails is fixed, or the behavior it checks is changed by a decision record. Ignored tests need a reason in a comment and an issue.

## Suites

Each suite below says where it lives and how it runs. Paths and commands are the plan; fill in the real ones as each suite is built.

| Suite | Proves | Where | Runs |
| --- | --- | --- | --- |
| CommonMark spec | Exact conformance: all 652 examples | `spec/`, harness in `crates/spec-harness` | every CI run |
| Extension specs | Each extension's own examples, plus examples showing plain Markdown is unchanged | `crates/geist4-ext/spec/*.md`, run by `crates/geist4-ext/tests/spec.rs` | every CI run |
| GFM spec | GFM extensions conform to the GFM spec's examples | `spec/gfm/` | every CI run |
| Unit tests | Each scanner, rule and edge case on its own | next to the code | every CI run |
| Property tests | No panic on any input; well-formed HTML; deterministic output; valid spans | `tests/properties/` (`proptest`) | every CI run, more cases locally |
| Pathological inputs | Linear time on hostile input; limits behave as documented | `tests/pathological/` | every CI run |
| Differential tests | Same output as the reference C implementation on generated input, except documented differences | `tests/differential/` | every CI run |
| Fuzzing | No crash, hang or limit breach on mutated input | `fuzz/` (`cargo-fuzz`) | a few minutes per target in CI; an hour per target locally per milestone |
| Cross-binding | JavaScript, Swift, Python and C give byte-identical output to the core on every spec | `tests/bindings/` | every CI run |
| Extension tiers | An extension's `spec.md` passes whichever tier implements it | `tests/ext-conformance/` | every CI run |
| Round trip | `format` is idempotent and keeps the rendered HTML the same | `tests/roundtrip/` | every CI run |
| JSON schema | Every JSON tree validates against the published schema | `tests/json/` | every CI run |
| Extension sandbox | Hostile extensions are stopped by fuel and memory limits | `tests/sandbox/` | every CI run |
| Benchmarks | Speed against the corpus and other engines | `benches/` (`criterion`), instruction counts in CI | instruction counts every CI run; wall clock locally |
| Mutation testing | The tests notice when the code is changed | `cargo-mutants` | before each release, results below |
| Coverage | Which lines no test reaches | `cargo-llvm-cov` | every CI run, threshold enforced |

## Running Everything

```sh
scripts/check.sh            # exactly what CI runs
```

### Fuzzing

The `fuzz/` crate holds the targets; it builds with nightly (a scoped
`fuzz/rust-toolchain.toml`) and is excluded from the workspace. Run
from inside `fuzz/`:

```sh
cd fuzz
cargo fuzz run to_html                 -- -max_total_time=3600  # the full pipeline
cargo fuzz run parse_document          -- -max_total_time=3600  # parser only
cargo fuzz run parse_with_extension    -- -max_total_time=3600  # extension hooks active
cargo fuzz cmin to_html                                         # minimize the corpus
```

A three-minute run per target is what CI (`.github/workflows/fuzz.yml`)
executes on every push; the hour-long runs are for after each milestone,
with results recorded under Results below. The extension target
(`parse_with_extension`) parses and renders with the strikethrough
built-in registered, so the inline trigger dispatch, the extension
delimiter machinery and the extension renderer get mutated input.

The minimized corpus is not committed in full (it is tens of megabytes
of near-duplicate files); instead a deterministic subset is checked in
under `fuzz/seeds/`, produced by selecting one file per coverage edge
from the minimized corpus, so every local run starts from the fuzzer's
learned edges. `tests/differential.rs` runs the entire seed set against
cmark 0.31.2 on every `cargo test`, byte-identical output required, so
fuzzer-found inputs become a permanent differential fixture.

Document each other command here as it exists (benchmarks, mutation
testing), with the time it takes.

## Known Differences

List here every case where the output deliberately differs from the reference implementation in the differential suite, with the reason and a link to a decision record. The list starts empty and should stay short.

## Results

Latest long-run results (2026-11-13):

- `to_html` (full pipeline): 1 hour, 8.27M executions, 0 crashes, 0
  hangs, 0 OOMs. One real panic found in an earlier run (title
  post-processing slicing past a one-byte quoted title) fixed and made a
  regression test.
- `parse_document` (parser only): 1 hour, 11.67M executions, 0 crashes,
  0 hangs, 0 OOMs.
- Corpora after `cargo fuzz cmin`: 9,334 files (`to_html`), 8,919 files
  (`parse_document`), from ~35k raw files per target.
- Seed subset committed: 490 files (`to_html`), 472 files
  (`parse_document`), one per coverage edge; all 962 pass the
  differential test.

Mutation testing, coverage numbers and benchmark numbers are recorded
here as those workstreams come online.
