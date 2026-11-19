# Changelog

All notable changes to Geist4 are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
the project does not use Semantic Versioning's breaking-change discipline
before 1.0 (versioning lives in decisions 0002 and 0003).

## [Unreleased]

Nothing yet.

## [0.1.0] - 2026-11-19

First release: the full engine per the roadmap's M0 through M8.

### Added

- The full CommonMark 0.31.2 engine: block and inline phases matching the
  specification byte for byte (652/652 examples), with source spans on
  every node.
- Built-in extensions, each with a reference-verified spec suite:
  `strikethrough`, `autolink`, `tables`, `tasklist`, `tagfilter`
  (cmark-gfm semantics), `footnotes`, `front_matter`,
  `heading_identifiers` (GitHub slug semantics), `description_lists`
  (comrak semantics).
- The loadable extension format (decision 0023): declarative packages
  (inline spans, wiki links, fenced block containers, replacements,
  attribute lists) and sandboxed Wasm components with fuel and memory
  limits, validated manifests, and a conformance kit in `conformance/`.
- Host extension hooks for Swift, Python, JavaScript and C
  (decision 0035).
- Bindings: C ABI (`geist4-ffi`, the safety boundary), wasm/JavaScript
  (`geist4-wasm`), Python (`geist4-python`), Swift (`bindings/swift`),
  each verified byte-for-byte against the core across the whole spec and
  extension corpus.
- The mdast export (`geist4-mdast`): the tree as
  `mdast-util-from-markdown` shapes it, with UTF-16 positions; 667
  fixtures compared byte for byte against the reference implementation.
- The JSON tree format (`geist4-json`, `geist4/tree@1`) with a published
  JSON Schema (`docs/schema/geist4-tree-v1.json`) validated against
  every cross-binding case.
- The CLI: `render` (HTML, JSON, mdast), `ast`, `check`, `ext check`,
  `ext manifest`, `format`.
- The CommonMark writer (`geist4-writer`, `geist4 format`): normalized
  output with HTML round-trip and idempotence guarantees over the whole
  spec.
- The cross-binding comparison: 1158 cases every binding must render
  byte for byte like the core.
- The fuzz matrix (five libFuzzer targets), the property and differential
  suites, the pathological scaling suite, and the regression corpus
  (see `TESTING.md`).
- The instruction-count performance gate (iai-callgrind) and the wasm
  size budget (decision 0029).

[Unreleased]: https://github.com/Zyniker13/Geist4/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Zyniker13/Geist4/releases/tag/v0.1.0
