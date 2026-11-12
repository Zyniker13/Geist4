# Status

- **Current milestone:** M0 (Foundation)
- **Last done:** Workspace scaffolding, spec-harness crate, core spec test, check.sh, CI workflow, 5 decision records. `check.sh` passes locally (fmt, clippy, test, doc, cargo-deny).
- **Next step:** Push to GitHub, verify CI green on all 3 OSes + MSRV, then start M1 (core parser).
- **Blocked on:** nothing

## Log

- 2026-11-12: Repository created.
- 2026-11-12: M0 scaffold — root Cargo.toml, rust-toolchain.toml, rustfmt.toml, clippy.toml, deny.toml, 8 crate shells.
- 2026-11-12: M0 spec — vendored CommonMark 0.31.2 (`spec/spec.txt`, 652 examples), provenance in `spec/README.md`.
- 2026-11-12: M0 harness — `geist4-spec-harness` crate with parse/run API, 7 unit tests, 1 doctest.
- 2026-11-12: M0 progress — `geist4-core/tests/spec.rs` wired to `spec/progress.json` (passed: 0).
- 2026-11-12: M0 check — `scripts/check.sh` green (fmt, clippy, test, doc, deny).
- 2026-11-12: M0 decisions — 5 decision records created (0001-0005).
- 2026-11-12: M0 CI — workflow with 3-OS matrix + MSRV job created.
