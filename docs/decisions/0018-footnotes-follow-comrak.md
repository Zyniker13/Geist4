# 0018 — Footnotes follow comrak 0.51.0

## Context

GFM footnotes have no public reference implementation: cmark-gfm
carries no footnotes branch to diff against. GitHub renders footnotes
the way comrak implements them, and comrak is open source under the
same 2-clause BSD license, so its behavior can be pinned exactly.
The other M5 built-ins diffed against cmark-gfm or the CommonMark
reference; footnotes needed a different oracle.

## Decision

The `footnotes` built-in follows comrak 0.51.0 byte for byte,
verified by a 100-case differential battery against the comrak 0.51.0
CLI (`cargo install comrak --version 0.51.0 --locked`) and a 98-example
spec file generated from that comparison. Where the implementation's
internal structure shapes observable behavior, the engine mirrors the
structure: definitions are containers whose continuation consumes
exactly four columns; the reference claim requires the inline node
after the `[` to start with `^` (so an escaped `[^` stays literal);
an image-shaped `![^1]` claims but leaves the `!` marker, which is a
text node of its own; the footnote section opens at the first
definition rendered and closes at the end of the document, so
non-footnote blocks between definitions render inside the `<ol>`;
backrefs are numbered by render order, not by reference order.

The comrak version is pinned in the TESTING.md battery note; a newer
comrak may drift. The reference CLI is a dev-time oracle only — no
comrak code or dependency enters the workspace.

## Consequences

- `spec/footnotes.md` examples are exact against comrak 0.51.0,
  including its quirks (an overwritten definition renders in place
  without backrefs; the paragraph between two definitions lands
  inside the `<ol>`).
- The core inline parser gains an `Escaped` wrapper (matching
  comrak's node shape) and splits `![` into marker and bracket nodes;
  both are invisible to output (the wrapper dissolves at conversion)
  but visible to extension hooks reading sibling chains.
- comrak upgrades are not tracked automatically; the battery is the
  tripwire if the oracle is ever refreshed.
