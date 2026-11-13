//! The Geist4 core: a CommonMark parser, its document tree, options, limits,
//! and the extension API.
//!
//! Geist4 parses Markdown in the two phases the specification itself
//! describes: first the block structure, line by line; then the inline
//! content of each leaf block, with the delimiter-stack algorithm from the
//! specification's appendix. The document is an arena tree whose nodes carry
//! source spans, so renderers and tools can map output back to the input.

pub mod arena;
pub mod basic_html;
pub mod debug;
mod inline;
pub mod node;
pub mod parser;
pub mod span;

pub use parser::{Document, parse_document};

/// The version of this crate, from its manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
