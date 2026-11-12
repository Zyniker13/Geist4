//! The Geist4 core: a CommonMark parser, its document tree, options, limits,
//! and the extension API.
//!
//! Geist4 parses Markdown in the two phases the specification itself
//! describes: first the block structure, line by line; then the inline
//! content of each leaf block, with the delimiter-stack algorithm from the
//! specification's appendix. The document is an arena tree whose nodes carry
//! source spans, so renderers and tools can map output back to the input.
//!
//! This crate is a shell until milestone M1; see the repository roadmap and
//! `STATUS.md` for where the work stands. The specification test in
//! `tests/spec.rs` already runs every vendored CommonMark example through
//! this crate with the `geist4-spec-harness` harness and enforces the
//! recorded pass count in `spec/progress.json`.
