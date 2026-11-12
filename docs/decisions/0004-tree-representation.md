# 0004 — Tree representation

## Context
The parser must produce a document tree that is fast to build, small in memory, and carries source spans for error reporting and tooling.

## Decision
Use an arena (a single `Vec` of nodes) with index-based references. Text is borrowed from the input wherever possible (`Cow<'a, str>` only where the spec requires changes such as entity or backslash handling). Every node records its byte-range span. The arena is owned by the `Parser` and lives for the duration of parsing.

## Consequences
No reference counting, no per-node allocation during parsing. Spans are always available. The arena model constrains the API: nodes are immutable once created, and the parser must build top-down (blocks first, then inline).
