# The extension-format conformance kit

This directory holds the format's conformance corpus: cases that any
implementation of the Geist4 extension format can run against its own
loader and engine, with expected outcomes stated so nothing depends on
this repository's internals. The runner is `run.sh` (POSIX sh); it
uses this repository's CLI by default and honors `GEIST4=path/to/cli`
to test a different binary.

Two kinds of cases live here.

## Manifest cases (`manifest/`)

Each `manifest/NN-name/` directory holds one `extension.toml` (plus a
placeholder `spec.md`, because a package carries both) and an
`expected.txt` stating what a conforming loader must do with it:

- line 1: `ok` or `error`;
- for `error`, line 2: `reason: <token>` — the stable reason, one of
  the tokens listed below, which any implementation's outcome maps to;
- for `error`, line 3: the exact message this repository's loader
  produced when the case was captured. This line is informational for
  other implementations and a verbatim regression pin for this one.

The cases cover the validation rules of `docs/extensions/format.md`
in its declared order: required fields and their shapes, the format
version rule (same major, minor not greater), reserved and malformed
names, unknown hooks, kind declaration and cross-reference rules,
delimiter and marker ranges, template substitutions, the one-tier
rule, tier-2 path and trigger rules, and option typing. `01` and `04`
are `ok` cases; every other case must be rejected.

Tokens: `format-unsupported`, `reserved-name`, `bad-name`,
`unknown-hook`, `duplicate-kind`, `unreferenced-kind`, `unknown-kind`,
`bad-delimiter`, `kind-not-container`, `unknown-substitution`,
`wasm-path-missing`, `wasm-file-missing`, `bad-inline-trigger`,
`tier-conflict`, `option-default-type`, `bad-replacement`,
`bad-marker`.

A conforming implementation must agree with line 1 for every case and
should agree with line 2. Line 3 is this implementation's wording.

## Behavior packages (`behavior/`)

Each `behavior/<name>/` directory is a complete tier-1 (declarative)
package — `extension.toml` plus `spec.md` — whose `spec.md` examples
pin one feature's normative semantics in the CommonMark example
format: `inline-spans`, `block-containers`, `replacements`,
`attribute-lists`, `wiki-links`, `html-filter`. Any implementation
must render every example byte for byte as the `spec.md` states.

The corpus is declarative on purpose: the same `spec.md` must pass
whichever tier implements an extension, so tier-1 packages run
everywhere, with no sandbox required. The tier-2 surface is exercised
by the examples under `examples/extensions/` (including hostile
guests under `fuzz/`), which depend on this repository's sandbox and
are not part of the portable corpus.

## Running

```sh
sh conformance/run.sh
```

or, against another implementation:

```sh
GEIST4=/path/to/its/cli sh conformance/run.sh
```

The script prints one line per failure and a summary; it exits zero
only when every case agrees. CI runs it as part of `scripts/check.sh`.
