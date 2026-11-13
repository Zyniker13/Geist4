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
pub mod extension;
mod inline;
pub mod node;
pub mod parser;
pub mod span;

/// Unicode classification for extension scanners, the same classes the
/// inline phase uses: `is_space` covers Unicode Zs, Zl and Zp plus
/// tab, newline, form feed and carriage return; `is_punct_or_symbol`
/// covers Unicode P* and S*.
pub mod unicode {
    pub use crate::inline::unicode::{is_punct_or_symbol, is_space};
}

pub use parser::{Document, parse_document};

/// The version of this crate, from its manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
