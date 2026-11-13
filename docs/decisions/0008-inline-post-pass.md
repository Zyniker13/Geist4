# Decision 0008: Inline parsing runs as a post-pass

Date: 2026-11-12

## Context

The first integration parsed each leaf block's inline content at the moment
the block closed, with only the reference definitions collected so far. The
spec suite requires the opposite: a definition may appear in a later block
and still be used by an earlier one (examples 23, 33), because the reference
parser resolves reference links against the complete definition map after
the whole document is parsed.

## Decision

The block phase stores each paragraph's and heading's text as raw text
children. After the block parse finishes, a post-pass walks the tree in
document order and replaces those text children with parsed inline nodes,
passing the complete reference-definition map. Definition extraction stays
in the block phase (definitions are still stripped from paragraph text as
the reference parser strips them), and the inline parser receives the
document's full map.

## Consequences

- Reference definitions are visible to every leaf block regardless of
  position, matching the reference parser.
- Inline content is parsed once per leaf block, after all block decisions
  are final; no re-parsing is needed when later definitions arrive.
- Inline nodes currently carry empty spans; source spans for inline content
  are deferred to the span work in a later milestone.
- The full HTML5 entity table and the Unicode case-fold table generated
  from the Unicode character database back the definition-side cleaning
  (`clean_url`, `clean_title`) and label normalization.
