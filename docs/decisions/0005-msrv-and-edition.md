# 0005 — MSRV and edition strategy

## Context
The workspace targets edition 2024 with MSRV 1.85.0. CI must verify both the latest stable and
the minimum supported Rust version.

## Decision
- `rust-version = "1.85.0"` in the root `Cargo.toml`.
- `clippy.toml` sets `msrv = "1.85.0"`.
- CI has a dedicated MSRV job that builds the workspace with `rustc 1.85.0`.
- `rust-toolchain.toml` pins `stable` (currently 1.99.1) with `rustfmt` and `clippy` components.

## Consequences
When the MSRV needs to be raised, it must be done via a decision record. The MSRV CI job will
fail until the MSRV is updated in all three places.
