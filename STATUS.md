# Status

- **Current milestone:** M3 (Hardening) — M1 and M2 complete
- **Last done:** M3 second wave landed: differential tests against cmark 0.31.2 (`tests/differential.rs`, 400 deterministic generated cases, byte-identical output, skips when the reference binary is absent). The first run found two conformance divergences, both fixed with regression tests in `tests/blocks.rs`: inline-HTML comment content may contain dash runs (reference scanner semantics), and an HTML block's literal now renders as-is so trailing blank lines inside the block survive. Spec stays 652/652; `scripts/check.sh` green locally. Earlier wave: property tests, the pathological suite, iterative deep-nesting paths, parent-pointer delimiter removal, NUL → U+FFFD (commit `244df44`, CI green).
- **Next step:** M3 continued — cargo-fuzz targets (parser, renderer) with a short CI fuzz run and longer local runs, wire the cmark package into CI for ubuntu/macOS, `docs/security-model.md` covering the URL policy and raw-HTML defaults, and a regression corpus layout under `tests/regressions/`.
- **Blocked on:** nothing

## Log

- 2026-11-12: Repository created.
- 2026-11-12: M0 scaffold — root Cargo.toml, rust-toolchain.toml, rustfmt.toml, clippy.toml, deny.toml, 8 crate shells.
- 2026-11-12: M0 spec — vendored CommonMark 0.31.2 (`spec/spec.txt`, 652 examples), provenance in `spec/README.md`.
- 2026-11-12: M0 harness — `geist4-spec-harness` crate with parse/run API, 7 unit tests, 1 doctest.
- 2026-11-12: M0 progress — `geist4-core/tests/spec.rs` wired to `spec/progress.json`.
- 2026-11-12: M0 check — `scripts/check.sh` green (fmt, clippy, test, doc, deny).
- 2026-11-12: M0 decisions — decision records 0001–0005.
- 2026-11-12: M0 CI — workflow with 3-OS matrix + MSRV job created; decisions 0006 (path deps), 0007 (MSRV/edition) recorded.
- 2026-11-12: M0 pushed (`7243bac`); CI run 43330465121 success on all 3 OSes + MSRV. M0 done.
- 2026-11-12: M1 block phase — arena/node/span, cursor, block scanner modules, cmark-faithful `Parser` (container walk, close-until-can-contain, list padding rewind, HTML kinds 1–7, link-def extraction with same-line/next-line title rules). 324 of 652 with plain-text inlines.
- 2026-11-12: M1 inline phase — `inline/` module: full inline parser (text, backslash escapes, entities via the generated full HTML5 table, code spans, emphasis with the delimiter algorithm and rule of three, links/images inline + reference + shortcut, autolinks, raw HTML, hard/soft breaks), generated Unicode space/punctuation tables, generated case-fold override table. Integrated through `basic_html::render`.
- 2026-11-12: Fixed the inline-parser hang at example 14 (failed backtick run rewound forever) and made EOF-adjacent delimiters see a newline stand-in like the reference parser.
- 2026-11-12: Decision 0008 — inline parsing runs as a post-pass over the finished block tree, so every leaf block sees the document's complete reference definitions (spec examples 23, 33).
- 2026-11-12: Reference-def cleaning moved to definition-insert time (`clean_url`/`clean_title`: trim, entity-decode, backslash-unescape); destinations/titles stored raw by the block scanner; info strings decoded with the full entity table; labels trimmed with empty-label rejection.
- 2026-11-12: **All 652 of 652 spec examples pass.** progress.json updated. 16 block-phase unit tests added (`tests/blocks.rs`, separate from the spec examples, exercising lazy continuation, list indentation, HTML end conditions, tabs, definitions). Warnings eliminated; clippy clean under -D warnings.
- 2026-11-12: `scripts/check.sh` green at 652/652. Temporary probe/bisect/dump examples removed before the milestone commit.
- 2026-11-13: CI discovery — the CI workflow had never passed (the earlier green run was the CodeQL setup workflow). The toolchain-setup action failed on every run on all systems, and the pinned cargo-deny version did not exist. CI rewritten with plain rustup and cargo steps; run 43360678153 green on all three OSes + MSRV.
- 2026-11-13: M1 and M2 complete per their done-when lines: every block-section example passes, the renderer matches the spec byte for byte, and all 652 examples pass with none skipped or special-cased, on all three systems.
- 2026-11-13: Decision 0009 — generated tables are checked in; generators and the WHATWG entity data live under `tools/`; regeneration reproduces the checked-in files byte for byte (`a54add0`, CI run 43362090828 green).
- 2026-11-13: M3 wave 1 — property tests (proptest) and the pathological suite (ported from the reference implementation's test file, linear-scaling assertions at n vs 10n). Found and fixed: renderer/post-pass/conversion/drop recursion (stack overflow at ~16k nesting depth), O(depth) paragraph-text append scans, the O(n²) full-pool scan in inline-cell removal (replaced with parent pointers and constant-time unlink repair), and NUL handling (U+FFFD replacement per line). Deep-nesting cases at 500–10,000 levels complete; the 14 emphasis/bracket/link families scale linearly; spec remains 652/652.
- 2026-11-13: M3 wave 2 — differential tests against the reference implementation over 400 deterministic generated inputs, byte-identical output required. Fixed the two divergences found: inline-HTML comment content follows the reference scanner's dash-run rules (`--` may appear inside a comment when not ending it, `--->` does not close), and an HTML block's literal renders as-is so blank lines inside the block reach the output. Regression tests added for both; spec remains 652/652; check.sh green.
