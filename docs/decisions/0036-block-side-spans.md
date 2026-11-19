# 0036 — Block-side source spans via a per-line text map

## Context

The design promise (ROADMAP, fixed choices) is that every node records its
source span as byte offsets into the input. M6's spans milestone threaded
spans through the inline phase, but the parser's leaf-block buffer is not the
source: block prefixes are consumed, partially consumed tabs become
synthesized spaces, NUL bytes are replaced by U+FFFD, and lines are joined
with newlines. Buffer offsets could not be read as source offsets, so block
nodes and the inline nodes inside a multi-line leaf block had no faithful
spans. The M7 bindings (editor renderer, cross-binding comparison) need
spans that slice the real source.

## Decision

The parser records one anchor per appended line of a leaf block's buffer
(`parser/textmap.rs`):

- `LineAnchor { buf_start, src_start, lin_start, src_end, tab_span,
  nul_map }`: where the line's bytes sit in the buffer and where they came
  from. `src_start` is what the tab's synthesized spaces map to; `lin_start`
  is what the line's first non-tab buffer byte maps to; `nul_map` gives each
  U+FFFD byte its NUL's own source offset for NUL-replacement lines.
- `TextMap::record` / `record_after_join` at each line append; `shift`
  rebases the map when a setext heading consumes leading reference
  definitions (the map is kept, not dropped — only a paragraph dissolving
  into pure definitions loses its map); `map_span` maps a buffer range to a
  source span, with the joining newline mapping to the line ending's last
  byte.
- ATX headings record a whole-line anchor and the content start position
  (`leaf::atx_heading` returns it), so the heading's Text child gets a real
  span. Link and image cells start at their `[` / `![` marker.

Phase C: nested emphasis matches can share a delimiter run (multibyte
delimiters make a run longer than one byte), so a container's recorded range
can end before one of its children's ranges. `widen_container_spans` runs
bottom-up over the alive inline pool before `attach_pool`, widening each
container cell's range to the union of its own range and its alive
descendants' ranges, so a container's span always encloses its children.

## Consequences

Every node in the tree now carries a source span; the property test
`spans_lie_inside_the_input` covers the whole tree, and the multibyte
regression case (`*𑙠*\u{b55}**`) is saved in the proptest regressions file.
The cost is O(lines) bookkeeping at parse time; the iai instruction gate
(0031) will catch any significant regression, and a re-baseline follows the
0031 procedure if the gate trips.
