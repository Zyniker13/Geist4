# 0026: The extension-format conformance kit

## Context

The roadmap asks M6 to ship "a conformance kit: the harness plus a
set of format tests that any other implementation of the extension
format could run." Two requirements follow: the expected outcomes
must not depend on this repository's internals, and the corpus must
be runnable without this engine's sandbox.

## Decision

The kit is the corpus under `conformance/` plus the POSIX runner
`conformance/run.sh`:

- `manifest/NN-name/` cases pin what a conforming loader must do
  with a manifest: `expected.txt` states `ok` or `error` on line 1,
  a stable reason token on line 2 (for errors), and this loader's
  exact message on line 3. Line 1 is the portable assertion; line 2
  is the recommended granularity for other implementations; line 3
  pins our own wording against regressions. The corpus was captured
  from the real loader by `tools/conformance_gen.py`, never written
  by hand.
- `behavior/<name>/` packages pin each tier-1 feature's semantics
  with `spec.md` examples any implementation must render byte for
  byte. The corpus is deliberately tier-1 only: tier-1 packages run
  without a sandbox, so the set is tier-agnostic by construction;
  the tier-2 surface stays covered by `examples/extensions/` and the
  hostile guests of the fuzz milestone.
- `run.sh` drives both kinds through the CLI (`ext manifest`,
  `ext check`), honors `GEIST4` to test another implementation, and
  is part of `scripts/check.sh`, so CI enforces the corpus.
- A new `geist4 ext manifest` subcommand is the validation half of
  `ext check` (manifest parse, behavior validation, tier-2 load;
  no spec run), which is what the manifest cases need.

Authoring the corpus found a real bug: the loader never compared a
manifest's `format` version against the version it implements, so
`format = "2.0"` was accepted contrary to the format's version rule.
Fixed with regression tests (`format_version_rules`) and a new
`check_format_version` rejecting a different major or a greater
minor.

## Consequences

- The loader's one-line messages are now pinned verbatim by the
  corpus; changing one means regenerating `expected.txt` with
  `tools/conformance_gen.py` in the same commit, which keeps
  message drift deliberate and visible.
- Another implementation's pass criterion is precise without
  matching our words: outcome on every case, reason token when it
  can.
- The corpus grows with every format rule added later; the runner
  picks cases up by directory, so adding one is a directory, not a
  code change.
