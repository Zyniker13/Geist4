# Vendored specification

This directory vendors the CommonMark specification and its test examples,
which are the engine's conformance suite. The harness in
`crates/geist4-spec-harness` runs them.

## Provenance

- Source: <https://github.com/commonmark/commonmark-spec>
- Version: 0.31.2, dated 2024-01-28 (652 examples)
- Upstream commit: `9103e341a973013013bb1a80e13567007c5cef6f` (tag `0.31.2`)
- `spec.txt` is copied verbatim. Inside its examples, the arrow character
  stands for a tab, exactly as upstream writes it.

`spec/progress.json` records the current pass count over these 652 examples;
CI fails if the count ever goes down. See `TESTING.md`.

## License

The CommonMark specification is © 2014–2024 John MacFarlane and is licensed
under the Creative Commons Attribution-ShareAlike 4.0 International license
(CC-BY-SA 4.0): <https://creativecommons.org/licenses/by-sa/4.0/>

You are free to share and adapt this material for any purpose, provided you
give appropriate credit and distribute your contributions under the same
license. This license covers the vendored specification files only; the
engine's own code remains under MIT OR Apache-2.0 (see `LICENSE-MIT` and
`LICENSE-APACHE`).
