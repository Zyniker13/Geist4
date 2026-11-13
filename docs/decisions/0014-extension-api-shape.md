# Decision 0014: the extension API shape

Date: 2026-11-13
Status: accepted

## Context

M5 requires built-in extensions (GFM, footnotes, front matter, heading
IDs, definition lists) that use only the public extension API, and M6
requires one standard, versioned extension format with three behavior
tiers: declarative definitions interpreted by the engine, WebAssembly
components, and host-language callbacks. The same API must serve all
three, and with no extensions enabled plain CommonMark parsing must be
byte-for-byte unchanged (spec stays 652 of 652).

The core today is a hand-written block state machine (`process_line`
with open frames in `OpenKind`) and an inline scanner with a per-byte
dispatch loop (`run`), feeding an arena of `Node` variants rendered by
`geist4-html`. Both are closed over Rust enums.

## Decision

- The API lives in `geist4-core` under `pub mod extension`. An
  extension is a value of a trait object, `dyn Extension`, registered
  through `Options::extension`, in registration order.

- **Node kinds are data, not Rust variants.** An extension node is
  `Node::Extension(ExtNode)` carrying the kind name, an ordered
  attribute list (`(String, AttrValue)`) and children; the extension
  declares its kind (name, whether it is a container, which attributes
  are valid) up front. This keeps the core enum closed, is exactly
  what the M6 declarative tier and the JSON/mdast exports need, and
  lets any binding construct extension nodes. Built-in extensions use
  the same generic node; their renderers are registered like any
  other.

- **Block hooks.** An extension may implement
  - `try_start(&mut BlockStartCtx) -> ControlFlow`: called in
    `open_new_blocks` for each line after the built-in scanners fail
    (built-ins keep priority; this is where tables, front matter and
    definition lists hook in). The context exposes the line as bytes
    with the current column, the open-frame summary (what kind of
    block would contain the new one, whether a paragraph is open for
    interruption rules), and consume/open primitives.
  - `try_continue(&mut BlockContinueCtx) -> BlockContinue`: called for
    an open extension frame in the container-matching walk, deciding
    consume / close / lazy.
  - `close(&mut CloseCtx)`: final adjustment when the frame closes.
  Extension frames live in a new `OpenKind::Extension` variant holding
  the extension index and an opaque state box.

- **Inline hooks.** An extension registers trigger bytes; the inline
  run loop checks its trigger table when the byte is not one of the
  built-ins'. The hook receives the subject and position and returns
  nodes to insert and/or delimiters to push, so emphasis-like rules
  (`~~`) reuse the delimiter machinery. Table row parsing needs no
  inline hook: tables are a block extension that parses cell text as
  ordinary inline subjects.

- **Post-processing.** `post_process(&mut TreeCtx)` runs after the
  tree is complete, walking the arena with parent context; hooks may
  rewrite kinds, set attributes, or splice nodes (heading IDs, task
  list items, the disallowed raw HTML filter, footnote collection).

- **Rendering.** `render(&RenderNodeCtx)` writes HTML for an extension
  node; unregistered kinds render nothing and report a problem in
  `check` (M6). Render overrides are the same hook keyed by kind name,
  with a priority.

- **Options.** Each extension carries its own typed options struct via
  a downcast slot on the extension value; document-level options
  (`Options`) hold the extension list, so `Options::gfm()` is just a
  preset registering several built-ins.

- **Determinism and purity.** Hooks receive no I/O, no global state,
  and no randomness; the same input and options always produce the
  same tree. This is what makes tier-1 declarative definitions and
  tier-2 sandboxed WASM interchangeable, and what the conformance kit
  (M6) relies on.

## Consequences

- With no extensions registered, every hook site short-circuits on an
  empty registry: the parse path is unchanged and the spec suite stays
  652 of 652 without special-casing.
- Built-in extensions prove the API: if GFM tables or footnotes need a
  hook the API lacks, the API grows before M6 freezes it.
- Extension nodes' spans follow decision 0008 (inline spans empty for
  now); block extension nodes record their source span like built-ins.
- The declarative tier (M6) compiles its patterns down to these same
  hooks, so its behavior is by construction identical to a native
  extension.
