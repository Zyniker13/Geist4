# Decision 0009: Generated tables are checked in, not built at build time

Date: 2026-11-13

## Context

The roadmap asks for the full HTML5 entity table to be "generated at build
time from a vendored JSON file, not hand-copied". Building tables during
`cargo build` would require a Python (or similar) interpreter and network
or vendored data files at build time, which conflicts with reproducible
offline builds and with the MSRV job building nothing but Rust.

## Decision

The Unicode tables (whitespace, punctuation/symbols, case-fold overrides),
the HTML5 entity table, and the raw entity data they were generated from
are produced by generator scripts under `tools/` and their outputs are
checked in. Regeneration is a deliberate step, not part of the build. The
generators document their data source (WHATWG `entities.json`, the Unicode
character database via the Python standard library) so the tables can be
reproduced byte for byte.

## Consequences

- Builds stay pure Rust with no script or data dependency.
- The checked-in tables are the artifact of record; the generators make
  them auditable and regenerable.
- Upgrading the underlying data (a Unicode release, an entity list update)
  is an explicit commit that the diff makes reviewable.
