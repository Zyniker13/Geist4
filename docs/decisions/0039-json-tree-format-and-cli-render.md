# 0039 — JSON tree format, CLI render and ast, and moving inline source maps

## Context

M8 requires a JSON tree format with a published JSON Schema, the CLI's
`render` (HTML, CommonMark, JSON, mdast) and `ast` subcommands, and a
schema-validation test over the engine's whole corpus. The mdast export
(decision 0037) already established the position convention: 1-based
lines, 1-based columns, 0-based offsets, all counted in UTF-16 code
units, so a JavaScript host can index the source string with them.
Adding the second exporter exposed a latent parser bug: any test that
checks spans across the extension corpus was new, and it immediately
failed.

## Decision

- **The JSON tree format, version 1** (`geist4/tree@1`): one JSON object
  with `format`, `root`, `linkDefs` (every definition in document order,
  with its span and its `titled` flag) and `problems` (message and
  span). Node `type` names mirror the core tree's node kinds one to one
  in snake case (`block_quote`, `code_block`, `code_span`, ...); an
  extension node's `type` is the kind name its extension declared.
  Extension attributes are an array of `{name, value}` pairs rather than
  an object, so declaration order survives JSON's unordered objects.
  Nodes carry `position` in the mdast convention, so the two exports
  share one mental model. The tree's own semantics are serialized as the
  parser built them: code spans keep their raw source, list markers keep
  their character, links carry their reference kind.
- **A new crate, `geist4-json`**, owns the format: it depends on
  `geist4-mdast` for the line index and position mapping, since both
  exporters must agree on one source-view convention. The serializer
  builds `serde_json` values directly; there is no parallel node type
  layer to keep in sync.
- **The schema** is published in the repository at
  `docs/schema/geist4-tree-v1.json` (JSON Schema draft-07). The
  `jsonschema` crate (dev-dependency only) validates it against every
  golden fixture and every cross-binding case — the full CommonMark spec
  plus every built-in extension's spec, 1158 documents — in the test
  suite, so schema and serializer cannot drift apart silently.
- **CLI**: `render FILE --format html|json|mdast [--pretty] [--ext]` and
  `ast FILE [--ext]`, where `ast` is `render --format json --pretty` —
  the familiar name for "show me the tree". The `format` subcommand that
  writes normalized CommonMark is a later M8 item, not part of this
  decision.
- **Every span the parser records must stay inside the source**. The new
  invariant test walks every node of every corpus case through
  `Arena::iter` (added for exactly this: exporters need to check
  invariants over the whole tree, including nodes not reachable from the
  root).
- **The bug the invariant found**: when the description-lists extension
  built a term from the paragraph before it, its close hook moved the
  paragraph's text children under the `description_term` node, but the
  paragraph's inline source map stayed keyed to the removed paragraph.
  The inline phase then fell back to the term's own empty span as the
  offset base and shifted every inline by the marker line's start —
  correct HTML, wrong spans. Fix: `TextMap` became public-but-opaque and
  `CloseCtx` gained `take_text_map`/`set_text_map`, so a close hook that
  moves text children between nodes moves the source mapping with them.
  The deflists close hook does exactly that. HTML output is unchanged;
  spans and mdast/tree exports are corrected.
- **Crate layout note**: the HTML renderer lives in
  `geist4_core::basic_html`; the `geist4-html` crate remains a reserved,
  empty shell. Moving the renderer out of the core would touch about
  thirty files this close to 0.1.0 for no functional gain, so it is
  deferred; the coverage and mutation-testing work treats the renderer
  as part of the core crate.

## Consequences

- Hosts in any language can consume the tree the renderers walk,
  extension nodes included, with positions they can index with.
- The schema is the format's contract: a change to the shape must pass
  the schema tests or bump the format tag.
- `CloseCtx` grew two methods; extension authors who restructure leaf
  text must move maps with the text, and the format's documentation
  should say so when the extension format guide is written.
- The `description_lists` spec cases now pass the span invariant; the
  corpus-wide check guards every future extension against the same trap.
