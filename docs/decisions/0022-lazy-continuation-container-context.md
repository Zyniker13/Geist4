# 0022 — Lazy continuation consults extension block starts with container context

## Context

The description-lists extension exposed a gap in the lazy-continuation
rule. A marker line that follows an open definition item (`: Definition
1` then `: Definition 2`) must open a sibling item, not continue the
item's paragraph. The extension's `try_start` claims the second marker,
but the parser's lazy branch fired first: it checked only that a
registered extension *could* start a block on the line, and declined
lazy continuation on that basis — while the item's continuation
(holding the frame at indent) succeeded, so the walk never failed and
`try_start` was never consulted. The line lazily joined the paragraph.

The reference implementation (comrak 0.51.0, the GFM description-list
oracle) applies a stricter rule: the lazy branch fires only when the
block-start phase, given the deepest container that still matches the
line, opened nothing. Its lazy condition is checked after
`open_new_blocks` ran with `container` equal to
`last_matched_container`; a block start that opened a new chain makes
the condition false.

The old extension look hook (`block_start_looks(&[u8])`) could not
answer this question, because it saw only the line and not the
container the block-start phase would try against.

## Decision

- `Extension::block_start_looks` now receives a `LookCtx`: the line
  from the content position, whether it is indented four or more
  columns, the parse arena, and `container` — the parent of the first
  block that failed to continue, i.e. the deepest container that still
  matches the line.
- The parser's lazy-continuation branch requires, as before, that the
  deepest open block is a paragraph, the line is not blank, no built-in
  block start matches, and no extension look hook claims the line —
  but the look hooks now see the container above the failed block, so
  their answers can match what `try_start` will do once the failed
  blocks close.
- The block-start loop no longer stops after an extension start: it
  continues with the rest of the line, the way the reference
  implementation's loop does, so a marker that consumed only its own
  prefix leaves the rest free to open further blocks (`: - item` opens
  a list inside the definition).

## Consequences

- Extensions whose block starts must interrupt an open container's
  paragraph-lazy rule implement the look hook against the container
  context (description lists: a marker line over a container whose
  last child is the extension's own item, or its list with an open
  definition, is claimable and therefore blocks lazy continuation).
- Extensions that only need the line (footnotes) keep the same
  behavior with the new signature.
- Byte-for-byte agreement with comrak 0.51.0 on the full
  description-list battery, including sibling definitions, quotes
  (`> Term` + lazy `: def` stays one paragraph), and lists inside
  definitions.
