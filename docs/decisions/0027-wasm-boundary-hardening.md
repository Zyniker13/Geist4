# 0027 — The Wasm boundary: fuzz targets, a hostile fixture, and kind validation

## Context

Milestone m6-8 hardens the surfaces a parser never reaches: the
manifest parser (arbitrary text) and the component boundary
(arbitrary bytes, and a component that answers however it likes).
Three failure modes had no pinned answer: a manifest parse that
panics or names a position outside its input; a component whose
answers ignore the manifest's contract; a component that loops
forever, allocates without end, or produces malformed output. The
sandbox already caps fuel and memory at load; what was missing was
proof that every abuse ends as a defined error, and one hole: the
WIT contract says a component's answers name "one of the kinds `init`
received", but nothing enforced it at runtime, so a component could
open a block or claim a span under *another* extension's kind and
route that extension's renderer against its own content.

## Decision

Five fuzz targets under `fuzz/` (libFuzzer via `cargo fuzz`, nightly
pinned by `fuzz/rust-toolchain.toml`): `to_html`, `parse_document`,
`parse_with_extension`, `parse_manifest`, `load_component`. The
manifest target runs the pure parser: any input must either parse —
with kind and behavior counts inside the documented limits — or fail
with a one-line error whose span lies inside the input; a panic or an
out-of-range span is the bug. The component target offers arbitrary
bytes to `SandboxExtension::from_bytes` at the default caps: a
component that loads must carry the caps whatever its manifest
claimed, and every load failure is one line.

The hostile fixture (`fuzz/guests/hostile`, committed as
`tests/fixtures/hostile.wasm`) is a real component with four probes,
each pinned by a test in `tests/hostile.rs`: `%%bad:` opens a block
of the undeclared kind `bogus`; `[[…]]` claims a node with `end`
999,999 over six bytes of content; a render builds a 20-megabyte
string; `%%spin` runs an endless loop inside `block-start`. The
answers: the undeclared kind latches a protocol violation and opens
nothing; the oversized claim keeps the whole content (the claim
consumer clamps, rounding cuts to character boundaries); the memory
hog ends at the memory cap as a defined error; the endless loop ends
at the fuel cap as a defined error.

The hole is closed where the answers arrive: the adapter validates
every kind a component answers with — in `block-start`,
`inline-trigger` and `inline-bracket` — against the manifest's
declared kinds, and latches a violation the same way a failed call
is latched (message recorded, instance dropped, later hooks decline).
A component may still declare zero kinds (a pure replacement
component needs none); the rule is only that *answered* kinds must be
declared.

## Consequences

A malicious component cannot borrow another extension's rendering,
crash the host, exhaust its memory, or hang a parse; each abuse ends
in one line a host can show. The manifest contract is machine-checked
under mutation for an hour per target as part of the milestone, and
the corpus seeds come from the conformance kit's manifests, so
regressions in accepted shape surface as fuzz findings. The `fuzz/`
directory pins nightly only for the runner; the hostile guest pins
stable with its own `rust-toolchain.toml`, because the wasm target is
installed on stable only.
