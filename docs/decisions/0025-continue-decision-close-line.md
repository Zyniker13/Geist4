# 0025: `continue-decision` gains a close-and-consume outcome

## Context

A tier-2 block container that ends with a closing fence — the way a
fenced code block ends with a fence line, or the declarative
container ends with a bare `:::` — could not be expressed in the
extension world. The WIT's `continue-decision` was a record with a
single field (`consume`), so `block-continue` could only keep the
line as content, or return `none` to close the block and leave the
line free to start other blocks. A closing fence needs a third
outcome: close the block and consume the line whole, so the fence
itself never becomes content and never starts anything else.

The gap was found by the C `shout` example's conformance spec: the
guest's `block-continue` kept swallowing the closing `:::` line, so
the container never ended where the spec said it should. The
declarative tier had already worked around the missing outcome
internally (consume the whole line, then decline), which is exactly
the shape the world lacked.

## Decision

`continue-decision` becomes a variant with two cases:

- `keep(consume)` — the block continues, the line taken as `consume`
  names (the old record's behavior);
- `close-line` — the block closes and the line is consumed whole.

`none` from `block-continue` keeps its meaning: close the block and
leave the line free to start other blocks.

The core trait's `try_continue` mirrors the outcome as
`ContinueOutcome`: `Continue`, `CloseAndConsume`, `Close`. The
engine's container walk gained a `ClosedAt` result: the line is
consumed, the frames at and below the closing block close, and the
line's processing ends. The declarative container's close fence now
reports `CloseAndConsume` through the trait instead of consuming the
line itself and declining.

## Consequences

- A pre-release WIT break, taken now on purpose: 0.1.0 is not
  published, and a close-fence container is a fixture of the format
  the examples demonstrate. Every Wasm guest was rebuilt against the
  new world in the same change.
- The engine consumes the closing fence line itself, so a guest can
  no longer get the fence-line consumption wrong; the trait documents
  that `CloseAndConsume` is a closing fence.
- The declarative tier's behavior is unchanged: its close fence still
  consumes its line and closes its container.
