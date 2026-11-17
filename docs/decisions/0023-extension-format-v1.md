# 0023 — Extension format v1: three tiers, decision-shaped sandbox boundary

## Context

M6 needs one standard, versioned format describing an extension —
manifest, conformance examples, and behavior — so the same extension
works everywhere the engine runs: Rust, WebAssembly in a browser or
edge runtime, Swift, Python. The behavior tier choice is forced by the
hosts: a sandbox can run natively and in the CLI but not inside
JavaScriptCore in an app build; host callbacks work everywhere but
need a defined interface; and the common patterns (highlighting, wiki
links, admonition containers, math, replacements) should not need code
at all.

## Decision

- The package is `extension.toml` + `spec.md` (+ a component for
  tier 2), specified completely in `docs/extensions/format.md`.
- Tier 1 is declarative: inline spans, fenced block containers,
  replacements, attribute lists, wiki links, HTML filters — each an
  exact, closed pattern compiled onto the public extension API. This
  is the portable tier and is deliberately not Turing-complete.
- Tier 2 is a WebAssembly component against a WIT world whose boundary
  is decision-shaped: hooks receive the line/context and return what
  to do (start/continue/close/claim decisions, rendered fragments);
  the engine owns the arena and performs the decisions. The sandbox
  (wasmtime, behind an optional feature of `geist4-ext-loader`) gets
  no imports, a fuel limit and a memory limit; exhaustion is a
  defined, span-carrying error. wasmtime is chosen for its security
  track record, its standards conformance (component model), and its
  maintenance; it stays out of the default build.
- Tier 3 is host callbacks in the binding's language, using the same
  shapes as tier 2, so one manifest describes both and bindings map
  the same records.
- The same `spec.md` runs under whichever tier implements the
  extension; `ext check` validates the manifest against the schema,
  cross-checks kinds, checks conflicts and ordering, and runs the
  examples.

## Consequences

- The declarative tier's semantics must be exact (flanking rules, math
  guards, fence-close rules), since it is spec'd for other
  implementations and the conformance kit pins them.
- The decision-shaped boundary keeps the sandbox from ever touching
  the tree, which is both the security story and the reason one
  interface serves tiers 2 and 3.
- wasmtime joins the dependency tree behind a feature; recorded here
  as security-relevant. The default build stays small.
- Extensions beyond the declarative tier's reach need the sandbox or
  the host; the format's docs say so plainly.
