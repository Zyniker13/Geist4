# 0045 — Incremental reparsing: line-checkpointed full block reparse per edit region

## Context

M11 (the last 0.2.0 milestone) calls for incremental reparsing for the
editor use case: given the previous parse's structure, reparse only the
blocks whose source spans changed, keeping the tree and re-running
inlines only inside changed leaves. The roadmap pins the contract: for
a fuzzed corpus of edit sequences, incremental parses must be
byte-identical to full parses, a benchmark must show a measured win,
and the API must work from every binding with an update call taking
(previous document, new source) and returning the new document plus a
changed-spans list.

The parser's shape (studied in the M11 kickoff, all references
`crates/geist4-core/src/parser/mod.rs` unless noted) bounds what can be
reused cheaply:

- The block phase is a single tip/continuation pass over lines
  (`run` at :655, `process_line` at :731). Nothing persists a
  line→node map; the parser's `Frame.start_line` (:69) dies with the
  parse. What survives is per-node byte spans (`Arena` entries,
  `arena.rs:24-31`) and per-leaf buffer→source `TextMap`s that the
  inline phase consumes (:1628, gone afterwards).
- Inlines are a document-wide post-pass (`parse_all_inlines`, :1594)
  run after every block closes so link reference definitions are
  document-visible. One leaf's inline parse needs: the joined text,
  the leaf's `TextMap`, `link_defs`, the leaf's span, and the
  extensions slice.
- Extension state is per-parse: `begin_parse` fires once per parse
  (:148-151), block state is per-frame `Box<dyn Any + Send>`
  (:59-62), `close` hooks get `&mut text_maps` (:2265-2272), and
  `post_process` walks the finished tree with no region notion
  (footnotes rewrites definitions and references document-wide,
  `footnotes.rs:170-198`).
- The arena is a flat `Vec<NodeEntry>` indexed by `NodeId` (:8-20)
  with working insert/remove/set_kind/set_span surgery
  (`arena.rs:212-269`) — but it is append-only and never reused for a
  second parse.
- `Document<'a>` borrows its source (`Parser.src: &'a str`, :248);
  node text can be `Cow::Borrowed` slices of it (`src_from`, :1881).

## Decision

Incrementality is built as **line-checkpointed reparse of the edit
region, plus document-wide re-derivation of what genuinely depends on
the whole document** — not as a persistent parse tree that edits mutate
in place. Concretely, a new `crates/geist4-incremental` crate owns the
mechanism; the core parser gains exactly one reusable primitive:

**1. The parser learns to start from a checkpoint and stop at one.**
`parse_with` keeps its signature and stays the truth. A new internal
entry (`pub(crate)`, exposed through the incremental crate, not the
public core API) runs the same `Parser` machinery with:

- a **prefix summary** — the previous parse's outermost block list,
  each entry carrying (kind discriminator, span, the fields an
  unchanged block must reproduce verbatim: ListData, HeadingData,
  CodeBlockData, HtmlBlockData, the extension index), and
- a **line range** to parse — everything else comes from re-scanning
  the new source's lines in the region.

The output is ordinary arena nodes for the reparsed region. The
incremental crate splices that region's nodes into a freshly built
arena for the new document: unchanged blocks' entries are copied
(`Node` is `Clone`; `Cow::Borrowed` payloads re-borrow the new source
only when the region proved byte-identical — otherwise the copy forces
`Cow::Owned`, the conservative and always-correct choice).

**2. Checkpoint placement is conservative: an edit invalidates its
containing top-level block.** The diff of old/new source (a line-level
diff over line hashes) yields the changed line range. That range is
expanded to the union of top-level blocks intersecting it, **plus**
every top-level block after it *when* the change could shift their
start offsets (insertion/deletion of bytes) — a pure replacement of
equal length still invalidates only the intersecting blocks. Any
top-level block whose *preceding* sibling changed in byte length
shifts; span correctness demands reparsing it. The unchanged prefix
before the region contributes nothing but its copied nodes.

**3. Link definitions and extension post-processing re-derive
document-wide.** `link_defs` is document-visible, so the reparsed
region's definitions merge into a fresh map built from: unchanged
blocks' *stored* definitions (each top-level block's def entries are
kept in the checkpoint, with their spans — a shifted-but-unchanged
block's def spans adjust by the byte delta) plus the region's freshly
scanned ones. `begin_parse`, `close`, and `post_process` extension
hooks run exactly as a full parse runs them: the region reparse calls
`begin_parse` for its own parse, region extension blocks close with
their hooks, and `post_process` runs over the assembled tree. An
extension whose `post_process` is not idempotent under this scheme
cannot produce a different result than a full parse, because
`post_process` always sees the fully assembled tree — the same input a
full parse would give it. Region-local `close` state starts fresh,
which is sound for every current built-in (each region is parsed as a
self-contained block sequence) and is the documented constraint for
host extensions.

**4. Inlines re-run only in changed leaves — with one deliberate
exception.** After assembly, the incremental pass walks the tree and
re-runs `parse_inlines_into` for every leaf whose block was reparsed
*or* whose joined text resolves a reference that appears in the
changed region's def set (add, remove, or redefine of a label). The
conservative rule: if the region touched any link definition at all,
every leaf re-runs inlines (defs are cheap to detect: compare the
merged def map against the checkpoint's). Leaves outside the region
whose text is byte-identical and whose def environment is unchanged
keep their inline children, copied with the block.

**5. The API.** `geist4_incremental::Editor::new(options)` holds one
options value; `update(&self, previous: &Document, new_source: &str)
-> (Document, Vec<Span>)` returns the new document (owning arena,
fresh NodeIds — callers never see ids from the old document except
through the returned changed-spans list, which reports *new-source*
byte spans of reparsed top-level blocks) plus the changed regions. A
document that did not come from the same options value, or whose
checkpoint is missing (first call), falls back to a full parse,
byte-identical by construction. Cross-binding: FFI gains
`geist4_editor_*` handles, wasm/python gain the same surface; all
route through the same safe module.

**6. Correctness bar and fallback.** Every path that cannot *prove*
the region boundary is safe (an extension block spanning the region
edge, a fenced block whose fence closed after the region, a setext
underline whose paragraph started before it) falls back to a full
reparse of everything from the last safe checkpoint. The full parse
is the oracle: the fuzzed differential corpus compares `update`
against `parse_with` byte for byte, including problems, defs and
JSON trees.

This is not the theoretically maximal scheme — it does not reuse
incomplete block tails or splice mid-container. It reuses what this
parser can prove (whole top-level blocks outside the edit region) and
re-derives what it cannot (defs, post-processing, shifted spans). For
the editor's dominant edit (keystrokes inside one paragraph of a
multi-KiB document), the win is real: the block phase re-scans only
the edited paragraph's lines plus shifted blocks' bookkeeping is
avoided by copying, and inlines re-run only in the edited leaves.

## Consequences

- `parse_with` remains the single source of truth; the incremental
  crate is a consumer of one new internal parser entry point, not a
  second parser. A divergence shows up immediately in the differential
  corpus.
- The checkpoint format is internal to the incremental crate and
  derived from a finished `Document` — no parser state survives a
  parse, so the checkpoint carries (top-level block summaries, def
  entries with spans, source length, the options fingerprint).
- The arena stays append-only; every update builds a fresh arena
  (copy-unchanged + splice-region). NodeIds are not stable across
  updates; the changed-spans list, in new-source coordinates, is the
  stable patch address for a renderer.
- The benchmark records before/after numbers in `docs/benchmarks.md`
  per the roadmap: full `parse_with` versus `update` on edit
  sequences over 10 KiB–1 MiB documents.
- The wasm size budget (0029) is untouched: the incremental crate is
  pure computation over existing types, no new dependency.
- Binding exposure (FFI, wasm, python) follows once the core is
  proven; the roadmap's done-when needs at least the JavaScript
  binding exercised.
