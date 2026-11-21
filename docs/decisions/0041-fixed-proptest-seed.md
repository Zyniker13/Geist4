# 0041 — Fixed proptest seed

## Context

Mutation-testing triage kept classifying the same mutants differently between runs: a
mutant recorded as caught in one run flipped to missed in a rerun with no code change.
The cause was the property battery's OS-random seed — `proptest!`'s default config picks
a fresh random seed per run, so each run executes a different 512 random cases, and
catches that depend on the sampled inputs churn between runs. This made survivor
lists non-reproducible: triage done against one run's misses could be invalidated by
the next run's different sample.

## Decision

The property battery pins a fixed seed in code
(`crates/geist4-core/tests/properties.rs`, `PROPTEST_SEED = 2026_1121`), passed as
`rng_seed: RngSeed::Fixed(...)` in the `proptest!` config. Every run of the battery —
CI, local, and mutation testing — executes the same 512 cases.

## Consequences

- Mutation-testing classifications are reproducible: a rerun's catch set is directly
  comparable to a previous run's, which is what survivor triage across runs requires.
- The deterministic batteries (spec, differential, corpus) already carry the real
  spec-coverage; the property battery is a safety net and a fixed seed does not weaken
  it in a way that matters for this project — every reported catch is reproducible,
  which is worth more than a widening random sample.
- If the property suite is extended, the fixed seed does not prevent new inputs: added
  properties run their own 512 fixed cases. To re-randomize deliberately, bump the
  seed constant and record it here.
