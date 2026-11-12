# 0001 — Language and license

## Context
The engine must run everywhere: native apps, WebAssembly, scripting bindings. The license must allow embedding in proprietary applications without forcing them to open their code. AGPL was considered and rejected: its network-use clause reaches applications that serve rendered output, which would stop adoption in web services and editors — the primary users of a Markdown engine.

## Decision
Rust with a stable toolchain and edition 2024. License: `MIT OR Apache-2.0` (the Rust ecosystem's standard dual license). `deny.toml` enforces a whitelist of permissive dependency licenses (`cargo deny check` runs in CI): Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, CC0-1.0, ISC, MIT, MPL-2.0, Unicode-3.0, Zlib.

## Consequences
Any new dependency must be vetted against the allow-list. A non-conforming license is a blocker and must be documented in a decision record before adding the dependency.
