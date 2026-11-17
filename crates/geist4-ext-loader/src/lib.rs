//! Loads Geist4 extensions in the standard, versioned extension format,
//! including the WebAssembly sandbox for tier-2 extensions.
//!
//! See `docs/extensions/format.md` for the format this crate implements.
//! The sandbox (an optional feature, backed by a WebAssembly runtime) runs
//! extensions with no imports beyond the defined interface, a fuel limit and
//! a memory limit, so a hostile extension is stopped cleanly instead of
//! hanging or exhausting the host.

pub mod compile;
pub mod decl_block;
pub mod decl_inline;
pub mod error;
pub mod manifest;
pub mod package;
pub mod types;

#[cfg(feature = "sandbox")]
pub mod sandbox;
