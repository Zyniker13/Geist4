# 0029 — The WebAssembly binding: one glue, checked in, with a size budget

## Context

M7's second binding (m7-2) is JavaScript. The core already builds for
`wasm32-unknown-unknown`; the question was the boundary it binds through,
the panic strategy available on that target, and how callers get the glue
and types without a Rust toolchain. No size budget had ever been recorded
for the WebAssembly build, though the browser use case — a Markdown
editor's renderer — is exactly the one that pays for every byte.

## Decision

`geist4-wasm` binds through `geist4-ffi`'s safe module, not through the C
ABI's raw pointers and not through any core internals: one unsafe boundary
(decision 0028) shared by every binding, with this crate itself fully
safe under the workspace's `forbid(unsafe_code)`.

The crate builds as `cdylib` plus `rlib`: the cdylib is what
wasm-bindgen post-processes into JavaScript glue, the rlib lets host-side
`cargo test` pin the contract before any WebAssembly is built.

wasm-bindgen 0.2.130, crate and CLI pinned together (the CLI rejects
mismatches); CI installs the CLI at the same version.

The checked-in glue is generated with `--target web`: one ES-module API
for browsers, Node, and bundlers. `init` takes no argument in a browser
(fetch) and explicit bytes in Node or a wasm-only environment. The glue,
the TypeScript declarations, and the `.wasm` live in
`crates/geist4-wasm/pkg/`, checked in like the C header so callers never
need a Rust toolchain. `scripts/check.sh` regenerates into a scratch
directory and compares the text parts (the JS glue and both `.d.ts`
files) byte for byte; the `.wasm` cannot be compared across machines
because rustc embeds platform-specific paths in panic locations, so its
correctness is pinned by running the Node harness twice — against the
freshly built glue (current source behaves) and against the checked-in
`pkg/` (the committed artifact behaves; a stale `pkg/` fails its own run
the moment a fix changes behavior). The size budget is measured on the
fresh build.

Errors are JavaScript `Error`s: option and parse results map the safe
module's `Result`s onto thrown errors. Optional arguments use paired
functions (`parse`/`parseWith`, `renderHtml`/`renderHtmlWith`) because
wasm-bindgen cannot express `Option<&T>` for an exported struct.

Panic strategy: `panic = "unwind"` is unavailable for the prebuilt
wasm32-unknown-unknown standard library, so the build uses the default
abort. A stray panic becomes a loud `WebAssembly.RuntimeError`, never
silent corruption. The safe module's `catch_unwind` compiles unchanged;
on WebAssembly it simply never fires, and host tests keep covering it.

Size budget, enforced by `scripts/check.sh` on every run and in CI: the
checked-in `geist4_bg.wasm` stays under 512 KiB raw and 160 KiB after
`gzip -9`. The m7-2 baseline measures 418,381 bytes raw and 146,525
gzipped, so both budgets leave real headroom for growth while still
tripping on a careless dependency. Transfer size is what an editor's
page load pays; the raw bound covers uncompressed local loads.

## Consequences

Callers get `pkg/` and import it directly; a Rust toolchain is needed
only to regenerate it, and staleness is a gate failure, not a drift.
The JavaScript contract is pinned by a Node harness
(`crates/geist4-wasm/node-tests/run.mjs`) that loads the checked-in
glue with explicit bytes and asserts the same byte-exact HTML, problem
reporting, and error behavior the Rust and C tests do — so all three
bindings can never quietly diverge. Growing past either budget is a
deliberate act: bump the budget in this decision with a reason, never
silently in a build script.
