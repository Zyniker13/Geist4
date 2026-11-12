# 0001 — Dependency licensing

## Context
The engine ships under `MIT OR Apache-2.0`, so every dependency must be compatible with that dual license.

## Decision
Use `deny.toml` to enforce a whitelist of permissive licenses. The allow-list includes:
`Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `CC0-1.0`,
`ISC`, `MIT`, `MPL-2.0`, `Unicode-3.0`, `Zlib`.
`cargo deny check` runs in CI.

## Consequences
Any new dependency must be vetted against the allow-list. A non-conforming license is a blocker
and must be documented in a decision record before adding the dependency.
