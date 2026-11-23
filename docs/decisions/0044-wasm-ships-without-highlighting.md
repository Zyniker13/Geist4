# 0044 — The wasm build ships without the syntax-highlighting engine

## Context

M10 wires the highlighter through every binding (decision 0043 chose
syntect with the fancy-regex backend). The web binding is the one that
pays for every byte: decision 0029 fixes the module's size budget at
512 KiB raw / 176 KiB gzipped, the transfer size an editor's page load
pays, and states that growing past either budget is a deliberate act —
bump the budget in a decision with a reason, never silently in a build
script.

With the syntect engine compiled in, the optimized wasm module measures
2,037,864 bytes raw / 969,910 gzipped — four times the raw budget. The
weight is the default syntax pack's tables (75 syntaxes, ~1.5 MiB as
compiled data), not the engine code. No smaller syntect configuration
fixes this: the pack is all-or-nothing, a reduced pack means picking
languages by hand and drifting from the native builds, and lazily
fetching dumps from JS would put a second parser engine on the page.

## Decision

`geist4-ffi` grows a `highlight` feature (default on) that carries the
`geist4-highlight` dependency and the syntect engine. Every native
binding — the C library, the Python wheel, the CLI — builds with it, so
`enable_highlighting()` works there out of the box.

`geist4-wasm` builds with `--no-default-features` (set in
`scripts/check.sh`, which is what CI runs): the module ships without
the engine. The API surface is unchanged across feature sets —
`enableHighlighting` exists in both — but without the feature it is a
no-op in the safe API and `geist4_options_enable_highlighting` reports
the new `GEIST4_ERR_UNSUPPORTED` (7). An HTML render never errors, so
fenced blocks simply stay plain. A page that wants highlighted fenced
blocks registers a JS host hook against the same `CodeHighlighter`
contract (or post-processes the emitted
`<pre><code class="language-…">` with its own highlighter in the
browser), which keeps the module at the decision-0029 budget.

## Consequences

- The checked-in `pkg/` artifact stays at 415,020 raw / 170,665
  gzipped — under both 0029 lines, no budget bump needed. The module is
  actually 3,453 bytes smaller than the m7-5 measurement because the
  highlighter hook plumbing (a few pointer-sized bytes) replaced
  nothing and the pack never enters the build.
- Native callers get highlighting with one call and no size pressure.
- The wasm/JS story for highlighting is: shell out to the host hook.
  This is the ROADMAP M10 "document why the wasm build opts out" path;
  if a future milestone wants highlighting inside the module, the
  budget conversation happens in a new decision with measured numbers,
  not here.
- `geist4-ffi`'s C-ABI status enum gains `GEIST4_ERR_UNSUPPORTED = 7`,
  the first status that depends on the build configuration, not the
  call's arguments.
