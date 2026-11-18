# 0030 — One problem per mistake: the retry records nothing

## Context

Every broken full reference — `[text][missing]` with no definition —
recorded its problem twice. The CommonMark algorithm rewinds after a
failed attempt and re-scans the text after the literal `]`, so the
following label became a fresh bracket group whose attempt failed the
same way; the parse recorded both attempts. An earlier milestone pinned
the double and left deduplication to reporters: `geist4 check` already
deduplicated by message and position. With m7's bindings exposing
problems to C and JavaScript hosts, every host would have to know to
deduplicate, and a checker UI would show one mistake twice unless it did.

## Decision

The parse reports one problem per author mistake. A bracket group
opening exactly at the rewind point of a just-failed attempt is that
attempt's own retry: its failure records no problem. The marker arms
only when a failure is actually reported, so groups further away are
reported as the fresh mistakes they are — `[a][b][c]` reports `'b'` for
the full reference and `'c'` for the shortcut group, not one problem for
the chain and not three for the parse's internal attempts.

Spans are unchanged: still the block's span, per the m6-5 design, since
inline nodes keep no spans of their own.

## Consequences

Problem counts for broken references halved, and the pinned tests now
assert one problem with the message and span spelled out; a new test
pins the retry suppression and the distinct-mistakes case end to end.
`geist4 check`'s deduplication stays as a second line of defense for any
future problem source, but the channel itself no longer emits
duplicates, so binding hosts need no dedup logic. Output HTML is
untouched — this changes only what the problems channel records.
