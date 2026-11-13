# Status

- **Current milestone:** M3 (Hardening) — M1 and M2 complete
- **Last done:** All 652 CommonMark 0.31.2 examples pass (`spec/progress.json`). CI green on all three OSes + MSRV (run 43360678153, commit `43c9ac3`) after replacing the failing toolchain-setup action with plain rustup steps and fixing the cargo-deny version pin. 16 block-phase unit tests; `scripts/check.sh` green locally.
- **Next step:** M3 hardening — pathological-input tests with linear-scaling checks, cargo-fuzz targets, proptest property tests, differential tests against cmark, security defaults and `docs/security-model.md`, regression corpus.
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
