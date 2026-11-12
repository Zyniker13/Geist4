# Roadmap

Geist4 is a CommonMark-conformant Markdown engine in Rust with a standard, loadable extension format. This roadmap runs from an empty repository to version 0.1.0 in nine milestones. Each milestone ends with a **Done when** line, and a milestone is done only when that line is met and shown with real output from the same checks CI runs.

Priorities, in order, when they conflict: **correctness, then security, then speed, then features.** A faster engine that fails a spec example is a regression.

## Fixed Choices

These were set when the repository was created. Change one only with a decision record that explains why.

- **Language:** Rust, stable toolchain, edition 2024. The minimum supported Rust version is set in M0 and tested in CI.
- **License:** MIT OR Apache-2.0 (the Rust ecosystem's standard dual license), so the engine can be embedded anywhere.
- **Spec:** CommonMark 0.31.2 (`spec.txt`, 652 examples), vendored under `spec/` with its CC BY-SA 4.0 notice. When a newer spec is released, upgrading is a decision record and a milestone task, never a silent change.
- **No `unsafe` in the core.** Core crates use `#![forbid(unsafe_code)]`. If a later speed-up truly needs `unsafe`, it lives in a separate, optional crate with a decision record, Miri in CI, and its own fuzz target.
- **No regular expressions in parsing.** Hand-written scanners over bytes (with `memchr` allowed), so running time is predictable.
- **No network, file system or environment access from the core.** The core takes a `&str` and returns a document or output. Loading files is the CLI's job.
- **One engine for every platform.** The same core must run, with the same output, in browsers, JavaScript server runtimes (including edge runtimes with no threads and no file system), native apps on macOS and iOS, and scripting languages. So the core builds for `wasm32-unknown-unknown` with no extra features, keeps its WebAssembly size small (a budget is set in M0 and enforced in CI), and never depends on threads. Bindings are thin layers over one C-compatible API and one WebAssembly API; behavior never forks per platform.
- **Extensions are portable.** An extension written once in the standard format must work in every binding (see M6), so a document gives the same result whichever platform renders it.

## Architecture (Starting Point)

The design below is the starting point. Improve it where the work shows a better way, and record the change.

- **Parsing follows the spec's own two phases.** Phase 1 builds the block structure line by line; phase 2 parses inline content in each leaf block, using the delimiter-stack algorithm from the spec's appendix for emphasis and links.
- **The document is an arena tree.** Nodes live in one `Vec` and refer to each other by index, so there is no reference counting and no per-node allocation. Every node records its source span as byte offsets into the input, so tools can map output back to the source.
- **Text is borrowed.** Node text refers to slices of the input wherever possible (`Cow<'a, str>` only where the spec requires changes, such as entity and backslash handling).
- **Renderers walk the tree.** M2 ships the HTML renderer that matches the spec byte for byte. Later renderers: CommonMark (normalized Markdown out, so `format` is possible), a stable JSON form of the tree with a published schema, and an export to the widely used `mdast` tree format (unist), so existing JavaScript tree tooling can consume the engine's output during a migration.
- **Hooks for hosts.** Renderers accept per-node overrides from the caller (for example, rewrite a link, add a class, or render a node kind differently), so an application can customize output without forking the renderer.
- **Limits are part of the API.** Options include maximum nesting depth, maximum input size and maximum extension fuel, all with safe defaults. Hitting a limit gives a defined, documented result, never a panic.
- **Extensions hook into both phases** (block starts, block continuation, inline triggers, post-processing passes, and rendering of new node kinds). Built-in extensions use the same hooks as loaded ones.

### Crates

Names below are placeholders. Pick the final product name in M0 (check that it is free on crates.io and npm) and record it.

| Crate | Purpose |
| --- | --- |
| `core` | Parser, tree, options, limits, the extension API |
| `html` | HTML renderer and the URL and raw-HTML safety policy |
| `ext` | Built-in extensions (GFM and the rest) |
| `ext-loader` | Loading extensions in the standard format, including the WebAssembly sandbox (optional feature) |
| `cli` | The command-line tool: `render`, `check`, `ast`, `format`, `ext check` |
| `ffi` | A stable C-compatible API, the base for native bindings |
| `wasm` | Bindings for JavaScript through WebAssembly, with TypeScript types |
| `swift` (package) | A Swift package over `ffi`, shipped as an XCFramework for macOS and iOS |
| `python` | Python bindings (PyO3), built as wheels |
| `spec-harness` | Reads spec files in the CommonMark example format and runs them; used by the core, every extension, and third-party extensions |

## Milestones

### M0: Foundation

- Cargo workspace with the crates above as empty shells, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, and `deny.toml` for `cargo-deny` (licenses, advisories, duplicate and banned crates).
- CI on GitHub Actions: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features` on Linux, macOS and Windows, the minimum Rust version build, `cargo deny check`, and `cargo doc` with warnings denied. Pin every action to a full commit SHA, and give workflows `permissions: contents: read`. Put the same checks in `scripts/check.sh`, and keep the script and CI identical.
- Vendor `spec.txt` for CommonMark 0.31.2 under `spec/`, with its license notice and the upstream commit it came from.
- The spec harness: parse spec files in the example format (the `example` fenced blocks with `→` for tabs), run each example through a render function, and report passed, failed and the failing example numbers by section. It must be usable by extensions and by outside projects, so give it a small public API and a test of its own.
- A spec test in the core that runs the harness and records the current pass count in a checked-in file (`spec/progress.json`), so CI fails if the count ever goes down.
- Decision records 0001-0005 (see [Decisions to Record](#decisions-to-record)).

**Done when:** CI is green on all three systems, and the harness runs all 652 examples and reports the pass count (zero is fine here).

### M1: Block Structure

- Phase 1 of the spec: thematic breaks, ATX and setext headings, indented and fenced code blocks, HTML blocks (all seven kinds), link reference definitions, paragraphs, blank lines, block quotes, list items and lists (tight and loose), with tab handling exactly as specified.
- The arena tree with source spans, and a debug printer for the tree used in tests.
- Unit tests for each block kind and each tricky rule (lazy continuation, list-item indentation, the end conditions of HTML blocks), separate from the spec examples.

**Done when:** every example in the block sections of the spec passes (inline content may still be rendered as plain text where an example has none), and `spec/progress.json` records it.

### M2: Inline Content and Full Conformance

- Phase 2: backslash escapes, entity and numeric character references (the full HTML5 entity table, generated at build time from a vendored JSON file, not hand-copied), code spans, emphasis and strong emphasis with the delimiter algorithm, links and images (inline, full, collapsed and shortcut references, with link label normalization by Unicode case fold), autolinks, raw HTML, hard and soft line breaks.
- The HTML renderer, matching the spec's output byte for byte, including its URL-escaping rules.

**Done when:** all 652 spec examples pass, with no example skipped or special-cased, on all three systems.

### M3: Hardening

- **Pathological inputs.** Port the reference implementation's pathological tests (deep nesting, many unclosed brackets and delimiters, long runs of backticks, nested block quotes and lists, huge link reference tables) and add more. Each runs under a time limit and must scale linearly; add a test that measures time at sizes n and 10n and fails if the ratio is far above 10.
- **Fuzzing.** `cargo-fuzz` targets for the parser, the HTML renderer, the CommonMark renderer once it exists, and the spec harness. A short fuzz run (a few minutes per target) in CI, and a long run you start locally after each milestone. Keep the corpus in the repository, minimized. Every crash becomes a regression test.
- **Property tests** with `proptest`: no input panics; output HTML is well-formed (balanced tags) for every input; rendering is deterministic; spans always lie inside the input and nest properly.
- **Differential tests** against the reference C implementation (`cmark`, installed from a package manager in CI and locally): generated and fuzzed inputs must render identically, except for documented, intentional differences.
- **Safety policy and limits.** Default URL policy that drops `javascript:`, `vbscript:`, `file:` and non-image `data:` URLs; a raw-HTML policy (`escape`, `omit` or `allow`) with a safe default; the nesting and size limits. Write `docs/security-model.md`: the threat model, each default and why, and what a caller must still do.
- **Regression corpus:** `tests/regressions/`, one file per bug found, each with a comment saying what it caught.

**Done when:** the pathological suite passes with linear scaling, every fuzz target has run for at least one hour locally with no findings (output recorded in `STATUS.md`), the property and differential suites pass in CI, and `docs/security-model.md` exists.

### M4: Speed

- A benchmark suite with `criterion` over a fixed corpus: the spec itself, a large real-world style document set you generate and check in (or fetch by a pinned script with a checksum), and the pathological inputs.
- Comparisons against `pulldown-cmark`, `comrak` and `cmark` in a benchmark-only crate, with results written to `docs/benchmarks.md` including the machine, toolchain and date.
- Instruction-count benchmarks (for example with `iai-callgrind` on Linux) in CI, so a change that makes the engine slower than a set threshold fails CI. Wall-clock numbers stay local, because shared CI machines are too noisy for them.
- Profile and optimize: avoid per-node allocations, scan with `memchr`, keep hot paths branch-light. Every optimization keeps the spec suite at 652 of 652.

**Done when:** throughput on the benchmark corpus is at least that of `pulldown-cmark` on the same machine (or the gap and its reason are recorded in a decision record), and the instruction-count check runs in CI.

### M5: Extension API and Built-In Extensions

- The extension API: hooks for block starts and continuation, inline triggers, post-processing passes over the tree, new node kinds with their own renderers, and options. Built-in extensions use only this public API.
- Built-in extensions, each with its own spec file in the example format and run by the harness: GitHub Flavored Markdown (tables, strikethrough, extended autolinks, task list items, the disallowed raw HTML filter; use the GFM spec's own examples), footnotes, front matter (YAML/TOML blocks kept as raw text; parsing them is out of scope), heading IDs, and definition lists.
- Extensions must never change how plain CommonMark parses: with no extensions enabled, the spec suite is still 652 of 652, and each extension's spec file includes examples showing that ordinary Markdown is unchanged.
- Start `docs/extensions/` with a guide to the API.

**Done when:** every built-in extension's spec file passes, the GFM spec examples for the GFM extensions pass, and the CommonMark suite is still 652 of 652 with all extensions off.

### M6: The Loadable Extension Format

This is what makes the engine more than a parser: one standard way to describe an addition to Markdown, so any tool using the engine can check, process and render it.

- **The format, versioned (v1)**, specified in `docs/extensions/format.md`. An extension is a directory or archive containing:
  - `extension.toml`: name, version, format version, description, license, the node kinds it adds (each with a schema for its attributes), which hooks it uses, its options, and which other extensions it conflicts with or must run after;
  - `spec.md`: its examples in the CommonMark example format, which are its conformance tests;
  - its behavior, in one of three tiers, chosen so an extension works everywhere the engine runs:
    1. a **declarative definition** in the manifest for the common patterns (delimited inline spans like `==highlight==` or `[[wiki links]]`, fenced or container blocks with a name like `:::note`, attribute blocks, inline and block math delimiters, simple replacements). The engine interprets these itself, so they work identically in every binding, including WebAssembly in a browser or edge runtime. Make this tier as expressive as it safely can be, since it is the portable one.
    2. a **WebAssembly component** with an interface defined in WIT, for logic the declarative tier can't express, where the host can run a sandbox (native builds and the CLI);
    3. a **host extension**: the same manifest and spec file, with the behavior supplied as callbacks from the binding's language (JavaScript, Swift or Python) through a defined interface, for hosts that can't run a sandbox (WebAssembly builds) or need application data (such as resolving a link to another document).
  - The same `spec.md` must pass whichever tier implements an extension, and the conformance kit checks that.
- **The sandbox.** WebAssembly extensions run in `wasmtime` (behind the `ext-loader` crate's optional feature, so the core stays small) with no imports beyond the defined interface: no file system, network, clock or randomness. Each call has a fuel limit and a memory limit; running out gives a defined error, not a hang. Record the `wasmtime` choice in a decision record, with its security track record.
- **Checking.** `cli ext check <path>` validates the manifest against the format's schema, checks for conflicts with other loaded extensions, and runs the extension's `spec.md`. `cli check <file> --ext <path>` parses a document with extensions and reports problems (unknown node kinds, broken references, limit hits) with source spans.
- **Examples** under `examples/extensions/`: at least two declarative extensions and two WebAssembly ones (one in Rust, one in another language that compiles to a component), each passing `ext check`.
- **A conformance kit:** the harness plus a set of format tests that any other implementation of the extension format could run.
- Fuzz the manifest parser and the WebAssembly boundary, and add hostile example extensions (infinite loops, memory hogs, malformed output) to the test suite, each stopped cleanly.

**Done when:** `docs/extensions/format.md` specifies v1 completely, all example extensions pass `ext check`, the hostile extensions are all stopped by their limits in tests, and the fuzz targets for the format have run for an hour with no findings.

### M7: Bindings Everywhere

- The C-compatible API (`ffi`): opaque handles, explicit ownership, UTF-8 in and out, no panics across the boundary (every entry point catches and returns an error code), a generated header with `cbindgen`, and its own tests in C.
- WebAssembly for JavaScript (`wasm-bindgen`) with TypeScript types, tested in CI in Node and in a headless browser, plus a test that it loads and runs in a WebAssembly-only environment with no threads, file system or Node APIs. Enforce the size budget.
- A Swift package over `ffi`, built as an XCFramework for macOS and iOS, tested on macOS in CI (and on the iOS simulator where CI allows).
- Python bindings (PyO3 with `maturin`), tested in CI.
- Host extensions (M6 tier 3) and render overrides working from JavaScript, Swift and Python, each with an example.
- **One output everywhere:** a cross-binding test runs the full CommonMark suite and every built-in extension's spec through each binding and compares the output with the Rust core's, byte for byte.
- The `mdast` export, with tests against the reference JavaScript implementation's trees for the spec examples (differences documented).

**Done when:** every binding passes the cross-binding suite in CI, and the WebAssembly build is within its size budget.

### M8: Tools and 0.1.0

- The CLI: `render` (HTML, CommonMark, JSON, mdast), `check`, `ast`, `format` (normalized CommonMark out; round-trip tests show `format` is idempotent and preserves the rendered HTML), `ext check`.
- The JSON tree format, with a published JSON Schema and tests that every output validates against it.
- A migration guide for projects moving from other engines (markdown-it, marked, micromark/remark, python-markdown, league/commonmark, cmark): option mappings, extension equivalents, and the known output differences.
- Mutation testing with `cargo-mutants` over the core; investigate surviving mutants and add tests until the survival rate is recorded and low, with the remaining survivors explained in `TESTING.md`.
- Line coverage over 95% for the core and HTML crates with `cargo-llvm-cov`, reported in CI.
- API docs for every public item (`#![deny(missing_docs)]`), `CHANGELOG.md`, and a complete `TESTING.md`.
- A security review of your own: read every public entry point and every limit again, and write what you checked in `docs/security-model.md`.

**Done when:** every check above passes in CI, `TESTING.md` describes every suite as it really runs, and the `v0.1.0` tag is pushed. Publishing to crates.io, npm, PyPI or a Swift package registry needs the maintainer's accounts, so stop and ask for that step in one line; don't publish yourself.

## Decisions to Record

Write these in M0 under `docs/decisions/`, numbered from 0001, one file each, in the shape: Context, Decision, Consequences.

- 0001 Language and license (Rust; MIT OR Apache-2.0), and why AGPL was not chosen for a library.
- 0002 Product and crate names.
- 0003 Spec version and how spec upgrades happen.
- 0004 Tree representation (arena, spans, borrowed text).
- 0005 Test strategy summary, pointing to `TESTING.md`.

Later decisions are numbered on from there.

## After 0.1.0

Not in scope until 0.1.0 is tagged: other language bindings (PHP, Go, Ruby, Kotlin), syntax highlighting, incremental reparsing for editors, and a language server.
