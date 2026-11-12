# 0002 — Product and crate names

## Context
The engine needs a product name and crate names, free on the registries it will publish to (crates.io, npm, PyPI).

## Decision
Product name: **Geist4**. Rust workspace members use the `geist4-` prefix: `geist4-core`, `geist4-html`, `geist4-ext`, `geist4-ext-loader`, `geist4-cli` (binary `geist4`), `geist4-ffi`, `geist4-wasm`, `geist4-spec-harness`. Future bindings follow the same scheme (`geist4-python`, Swift package `Geist4`).

## Decision (availability, checked 2026-11-12)
`geist4` is free on crates.io and npm; `geist4-core` probed free on crates.io.

## Consequences
Publishing to crates.io, npm, PyPI, or a Swift registry needs the maintainer's accounts; the engine does not publish itself.
