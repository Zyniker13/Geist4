# 0037: The mdast export

## Context

M7 calls for an export to the widely used mdast tree format (unist), tested
against the reference JavaScript implementation's trees for the spec examples,
with differences documented. Consumers — especially editors on
JavaScript/JavaScriptCore runtimes — want trees that existing mdast tooling
can consume directly: same node `type`s, same field names, and `position`
(line/column/offset) that points back into the source.

The Geist tree is its own shape (`document`, `blockquote`, `item`,
`code_block`, snake_case kinds, arena of spans). The export is a pure
transformation plus a mapping of byte spans to unist points. The reference
implementation is `mdast-util-from-markdown` (the parser behind remark-parse),
whose behavior defines what "an mdast tree of this input" means.

## Decision

A new crate `geist4-mdast` converts a parsed Geist document into
serde-serializable nodes in mdast shape. Core stays clean: serde lives here,
the crate is pure (no I/O, no unsafe), and it depends only on `geist4-core`.

Mapping (v1, core CommonMark):

- `document`→`root`, `blockquote`→`blockquote`, `list`→`list`, `item`→`listItem`,
  `heading`→`heading` (`depth`), `thematic_break`→`thematicBreak`,
  `paragraph`→`paragraph`, `code_block`→`code` (`lang`, `meta`, `value`),
  `html_block`→`html`, `inline_html`→`html`, `code_span`→`inlineCode`,
  `emphasis`→`emphasis`, `strong`→`strong`,
  `link`→`link`/`image` (`url`, `title`, `alt` for images),
  `hard_break`→`break`.
- `soft_break` and adjacent `text` nodes merge into single `text` values with
  `\n` line breaks: mdast has no soft-break node.
- `position` is computed from Geist byte spans through a line index. Point
  semantics replicate the reference exactly, verified against generated
  fixtures: `offset` is a 0-based index counted in UTF-16 code units (the
  reference sees the source as a JavaScript string), `column` counts UTF-16
  code units since the line start, CRLF counts as one line ending, and tabs
  are one column (positions are not tab-expanded).
- Extension nodes have no mdast analog in v1: they export transparently as
  their children. Per-extension mdast mappings (`table`, `delete`,
  `listItem.checked`, footnote types, …) land with each extension's binding
  work, following the corresponding mdast-util conventions.

Fixtures: every CommonMark 0.31.2 spec example's expected tree is generated
once by a checked-in, dev-only script (`tools/gen-mdast-fixtures.mjs`) running
`mdast-util-from-markdown@2.1.0` over the example inputs; the expected trees
are checked in under `tests/fixtures/`. The Rust test parses each input with
Geist, exports, serializes, and compares byte-for-byte against the fixture.
Where the implementations legitimately differ, the difference is recorded in
`tests/differences.json` with a reason and asserted (not silently skipped).

## Consequences

- Regenerating fixtures needs npm and the pinned reference version; the
  version lives in `tools/package.json` and is bumped together with the
  fixtures, like the Binaryen pin.
- Every quirk of the reference that the converter must replicate
  (text merging, `list.spread`/`listItem.spread` semantics, `code` value
  trailing newline, URL normalization, `image.alt` construction, root
  position end) is settled by fixtures, not by guesswork; each resolved
  quirk gets a focused unit test so the fixture file alone never carries
  the knowledge.
- If a quirk proves impossible to derive from the Geist tree alone
  (per-item spread is the risk), the core tree gains the minimum extra
  state, recorded as an amendment to this decision.
- The exporter is host-callable from every binding once M7's comparison
  work exposes it, since it is pure tree-in/tree-out.
