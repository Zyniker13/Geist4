# 0016 — Whitespace classes follow cmark 0.31.2, not the cmark-gfm fork

## Context

The two reference implementations the project verifies against disagree
on what counts as whitespace at the byte level. cmark 0.31.2's
`cmark_ctype` table classifies vertical tab (0x0B) and form feed (0x0C)
as whitespace; the cmark-gfm fork's copy of the same table does not
(its base predates the upstream change). The difference is visible in
plain paragraphs: `a<VT>\n` renders as `<p>a</p>` under cmark 0.31.2
and as `<p>a<VT></p>` under cmark-gfm, because the inline subject's
trailing-whitespace trim runs over the byte class. Extension-side, the
cmark-gfm tasklist scanner's required whitespace after the checkbox
does accept a VT or FF (its re2c `spacechar` class is
space/tab/VT/FF), so the divergence shows up in both directions.

The M3 work locked the engine's plain-output behavior to cmark 0.31.2
byte for byte over a 962-input seed differential and a 400-input
random differential; changing the core byte class to match the fork
would break that locked conformance, and the spec's own whitespace
definition (which names form feed) matches the newer upstream table.

## Decision

The engine follows cmark 0.31.2's whitespace classes everywhere:
vertical tab and form feed are whitespace for block starts, line
trims, fence-info trims, link destinations, label normalization and
the inline subject. Extension scanners that the reference defines with
the wider re2c class keep that wider class at the point of scanning
(the tasklist box accepts a VT or FF as its required space), because
that is a property of the reference's scanner, not of the fork's byte
table. Where the fork's older table changes an item's rendered
content — an item whose content ends in VT or FF renders the byte in
cmark-gfm but is trimmed here — the engine keeps the 0.31.2 behavior
and the extension's spec file documents the difference instead of
embedding an example the engine must not match.

## Consequences

- The tasklist spec file (`crates/geist4-ext/spec/tasklist.md`)
  excludes the two battery cases whose required space is a VT or FF
  and states the divergence in its header; the battery scripts keep
  the cases marked as the known divergence.
- The plain-output differential and corpus suites stay untouched and
  green; no special-case paths were added to the core.
- If a future consumer needs the fork's class behavior (for example to
  reproduce GitHub's output on control characters byte for byte), the
  right place is an explicit option, not a silent core change; none
  is planned for 0.1.0.
