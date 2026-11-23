# 0043 — Syntax highlighting via syntect, fancy-regex engine, class-based output

Date: 2026-11-23

## Context

M10 asks for fenced-code highlighting behind an option, choosing
between `syntect` (the Rust standard for Sublime-syntax highlighting)
and a tree-sitter grammar, with a dependency/security review and an
explicit wasm story.

Constraints from the roadmap: the option is off by default (spec
output stays byte-identical); the emitted HTML uses class-based tokens,
not inline colors; unknown languages render as plain code, never an
error; the wasm build must either work or be documented out.

## Investigation (measured, this machine)

- `syntect` 5.3.0 (published 2025-09-27, MIT, 43M downloads): with
  `default-features = false, features = ["default-fancy"]` it builds
  for `wasm32-unknown-unknown` — measured, clean build. The default
  `onig` engine does NOT build for wasm (its C `onig_sys` build script
  fails there), so fancy-regex is the only wasm-viable engine; the
  project itself supports this combination.
- Weight: the wasm rlib set for syntect + fancy-regex + regex-syntax +
  plist adds roughly 19 MB of debug rlibs; a full 75-syntax pack dump
  is 0.3 MiB serialized (341 KiB) and reloads in ~5 ms. A stripped
  probe binary was ~0.5 MiB — the real cost lands in the syntax pack,
  not the code.
- tree-sitter 0.27 core (59M downloads) is per-language C grammars:
  each language is a separate C library compiled per target, so the
  wasm build multiplies by every language shipped, and the class of
  "no tree-sitter grammar for this language" grows. The engine also
  assumes no `unsafe` in *our* code but pulls `-sys` crates with C.
- Probe of the API surface: `ParseState::parse_line` +
  `RangedHighlightIterator` yields (style, text, byte range) per
  token; the parse ops separately yield `ScopeStackOp`s whose byte
  offsets let us track the live scope stack and derive a class name
  from the topmost scope (`storage.type.rust` → class `type`).
  Unknown language: `find_syntax_by_token` returns `None` — a clean
  fallback point.

## Decision

Use **syntect with the fancy-regex engine**, wired as follows:

- A new crate `geist4-highlight` (highlighting is a feature, not part
  of the core render path): `syntect` with
  `default-features = false, features = ["default-fancy"]`. No `onig`.
- The syntax pack is built at startup from the bundled
  `.sublime-syntax` files we choose to ship (starting from syntect's
  default pack); wasm uses the same code path. Nothing requires
  onig-only features; if a syntax we ship needs them, it is dropped.
- The renderer emits `<span class="...">` tokens; classes come from
  scope names, not theme colors, so themes are CSS. Unknown language →
  plain escaped code, byte-identical to today's output for that block.
- The option lives on the render entry: off by default everywhere;
  the CLI gains `--highlight`.

Dependency review: `syntect` 5.3 (MIT), `fancy-regex` 0.20 (MIT),
`regex-syntax`/`regex-automata` (MIT OR Apache-2.0), `plist` (MIT) —
all permissive and on crates.io; `cargo deny` gates the set like every
dependency. `onig` is explicitly NOT pulled (its C source is the
security-relevant piece we avoid); `bincode` is only needed for dump
files, which we do not read at runtime in the default configuration —
if the pack is pre-built, the dump path stays behind a feature.

## Consequences

- The wasm build can carry highlighting; the size budget (checked in
  the wasm crate's tests) decides whether the full pack ships or a
  reduced one — that check happens when the feature wires in.
- Highlighting quality matches Sublime/cmark-highlighting expectations,
  not IDE-accurate parse trees; tree-sitter would buy accuracy at the
  cost of per-language C and wasm bloat, wrong trade for now.
- The class vocabulary is syntect scope tails (e.g. `keyword`,
  `string`, `comment`, `constant`); a theme maps classes to colors.
