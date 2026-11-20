# 0040 — Mutation-testing policy for 0.1.0 sign-off

## Context

M8's gate calls for a mutation pass over the parser core with every survivor triaged and killed or justified. cargo-mutants 27.1.0 runs each mutant against the full workspace battery. Two facts shaped the policy: (a) a small set of mutants in parser-loop and handler code makes the test binary allocate without bound (tens of GB observed), which on a shared build machine is fatal; (b) with the battery at ~60 s, any mutant whose behavior makes tests run forever shows up as a timeout rather than a normal result, and at two jobs those timeouts dominate wall time.

## Decision

- Runs are scoped one source file at a time (`cargo mutants --file ...`) and serialized; at most one run at a time.
- The runner is always invoked through a guard wrapper that injects `--jobs 2 --timeout 60 --minimum-test-timeout 20` and kills any test process exceeding 6 GB. These limits are not raised per run; the wrapper, not the config, enforces them (cargo-mutants 27.1.0 has no `jobs` config key).
- Timeouts are a hang class, not survivors and not kills: a mutant that pushes the battery past the 60 s cap is excluded from later runs by exact mutant name, with `exclude_re` entries generated programmatically from that run's `outcomes.json`. Function-level excludes are avoided unless a function times out wholesale — a partially-timing-out function keeps its caught mutants as kill signal.
- Every missed mutant is triaged to KILLABLE (a test asserting correct behavior would catch it) or EQUIVALENT (no observable behavior change), with a recorded rationale per mutant. Kill tests live in `crates/geist4-core/tests/mutants_*.rs`, each marked with a `// kills:` comment naming the mutants it targets. They assert spec-correct behavior — never "mutant dies" as such — and existing tests are never weakened to raise the score.
- Before each rerun, the exclude set is verified against the filtered mutant list: every excluded timeout matched, and no non-timeout mutant excluded.

## Consequences

- Runs stay inside the machine's memory budget; the guard killing an allocation-unbounded mutant is expected and cargo-mutants records it as failed/timed out while continuing.
- The reported kill score excludes the hang class by construction; TESTING.md states this so the score is not oversold.
- Reruns after kill tests land should converge to the excluded set. If new timeouts appear outside the hang class, that is a signal to investigate the mutant, not to exclude it.
- The wrapper and the per-mutant excludes are build-machine hygiene and test-infrastructure config, not shipped code; the excludes do not touch production behavior.
