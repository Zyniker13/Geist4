# Decision 0015: inline trigger hooks and extension delimiter pairing

Date: 2026-11-13
Status: accepted

## Context

M5's first built-in extension is GFM strikethrough. It needs the inline
parser to yield to extensions at bytes the engine does not own (`~`),
and it needs emphasis-like pairing (`~~a~~` → a node wrapping `a`)
without the engine hard-coding knowledge of strikethrough. The
extension API (decision 0014) promised "inline trigger bytes … so
emphasis-like rules reuse the delimiter machinery". The reference
behavior is defined by cmark-gfm's `strikethrough.c` and its
`process_emphasis`, which differ from plain CommonMark's in specific,
verifiable ways.

## Decision

- Extensions register trigger bytes with `inline_triggers`; at a
  non-built-in byte the inline run loop calls each owning extension's
  `inline_trigger` hook in registration order. Text runs stop at
  trigger bytes so the hooks fire. On the plain path (no extensions)
  the loop is unchanged.

- The hook context (`InlineTriggerCtx`) exposes `scan_run`, which
  measures a delimiter run with the engine's shared flanking rules and
  pushes the run's text node, and `push_delim`, which puts the run on
  the delimiter stack tagged with a node kind. The run's characters are
  consumed exactly like `*` and `_` runs.

- Flanking measurement treats registered trigger bytes as transparent
  in the before/after walks, matching cmark-gfm's `SKIP_CHARS`
  behavior: in `a\~~b`, the run's preceding character is `a`, not the
  escaped tilde. On the plain path no byte is transparent.

- Extension delimiters pair through the shared odd-match rule and the
  reference implementations' openers-bottom indexing `[length % 3]
  [character]` (not the newer 15-slot scheme plain CommonMark 0.31.2
  uses for `*` and `_`; the built-in path keeps that proven scheme).
  The shared rule is what makes `~a~~b~` wrap `a~~b`: a len-2 closer's
  failed search cannot hide a len-1 opener from a later len-1 closer.

- Pairing mechanics follow the reference's `insert` hook: the opener's
  whole text node becomes the extension node, the closer's whole text
  node disappears, and every delimiter between them is freed. A
  mismatched pair (unequal run lengths) creates nothing but still
  clears the whole range, so a failed match cannot rescan.

- Rendering goes through the renderer's work stack as a deferred step,
  so an extension node keeps its document position among its siblings;
  extension renderers run in registration order and unregistered kinds
  produce nothing.

## Consequences

- Strikethrough is ~30 lines in `geist4-ext` and shares the engine's
  delimiter machinery; the API needed no strikethrough-specific
  knowledge. The same shape will serve footnotes' inline markers if
  their matching turns out to fit this rule.

- The 38-example spec file (`crates/geist4-ext/spec/strikethrough.md`)
  was generated from a local cmark-gfm build; both engines must agree
  byte for byte, so regenerating it is the documented way to adopt a
  reference change.

- The differential suite caught a before-character walk regression
  within one run, which is the argument for keeping every scanner
  change behind that suite.

- A fuzz target (`parse_with_extension`) exercises the hook path with
  mutated input from M5 onward.
