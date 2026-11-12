//! Loads Geist4 extensions in the standard, versioned extension format,
//! including the WebAssembly sandbox for tier-2 extensions.
//!
//! This crate is a shell until milestone M6; see the repository roadmap. The
//! sandbox (an optional feature, backed by a WebAssembly runtime) runs
//! extensions with no imports beyond the defined interface, a fuel limit and
//! a memory limit, so a hostile extension is stopped cleanly instead of
//! hanging or exhausting the host.
