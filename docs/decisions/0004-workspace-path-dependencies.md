# 0004 — Workspace path dependency versions

## Context
`cargo-deny` bans wildcard versions by default. Workspace path dependencies have no version
string in their `Cargo.toml` entries, which cargo-deny treats as wildcards.

## Decision
Set `wildcards = "allow"` in `deny.toml`. Workspace path dependencies use bare `path = "..."`
entries without version pins. This is safe because path deps are resolved from the local
workspace, not from crates.io.

## Consequences
The wildcard ban cannot catch accidental `version = "*"` on external dependencies. This is
acceptable during development; when external dependencies are added, the allow-list and
`cargo deny check` will catch incompatibilities.
