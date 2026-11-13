//! Front matter: a delimited metadata prologue at the very beginning
//! of a document, parsed out before any block runs and held in a
//! `front_matter` node whose raw text is the node's `source`
//! attribute. The node renders as nothing, so HTML output is exactly
//! the output of the document without its prologue.
//!
//! The splitting rules follow the reference implementation, quirks
//! included: one leading byte order mark may precede the opener; the
//! opener is the delimiter followed by a line ending (the delimiter
//! must end its line — `--- text` does not open); the closer is the
//! first later occurrence of a line ending, the delimiter, and then a
//! line ending or the end of input. The three closer shapes are
//! searched in the reference's priority order, so a
//! carriage-return-line-feed closer anywhere in the body wins over an
//! earlier line-feed-only closer. The closer's own line ending and one
//! immediately following blank line belong to the prologue. An empty
//! body never closes: the closer search needs a line ending inside the
//! body, so `---\n---\ntext` parses as two thematic breaks.
//!
//! Unlike the reference, which splits the source before parsing and
//! shifts every later offset, the prologue is claimed by byte offset
//! and the rest of the document parses from its real position, so
//! spans after the prologue stay true to the source.

use geist4_core::extension::{ExtNode, Extension, KindDecl};

/// The front matter extension with the default `---` delimiter.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrontMatter;

impl FrontMatter {
    /// The front matter extension's node kind name.
    pub const KIND: &'static str = "front_matter";
}

/// The front matter extension for a caller-chosen delimiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontMatterWith {
    /// The delimiter that opens and closes the prologue.
    delimiter: String,
}

impl Extension for FrontMatterWith {
    fn name(&self) -> &str {
        "front_matter"
    }

    fn front_matter_delimiter(&self) -> Option<&str> {
        Some(&self.delimiter)
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: FrontMatter::KIND.to_owned(),
            container: false,
            inline_text: false,
            attrs: vec!["source".to_owned()],
        }]
    }

    fn render(
        &self,
        node: &ExtNode,
        _children: &[geist4_core::extension::RenderedChild<'_>],
        out: &mut String,
    ) {
        if node.kind != FrontMatter::KIND {
            return;
        }
        let _ = out;
    }
}

impl Extension for FrontMatter {
    fn name(&self) -> &str {
        "front_matter"
    }

    fn front_matter_delimiter(&self) -> Option<&str> {
        Some("---")
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: Self::KIND.to_owned(),
            container: false,
            inline_text: false,
            attrs: vec!["source".to_owned()],
        }]
    }

    fn render(
        &self,
        node: &ExtNode,
        _children: &[geist4_core::extension::RenderedChild<'_>],
        out: &mut String,
    ) {
        if node.kind != Self::KIND {
            return;
        }
        let _ = out;
    }
}

/// The front matter extension as a shared extension value, with the
/// default `---` delimiter.
#[must_use]
pub fn front_matter() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(FrontMatter)
}

/// The front matter extension as a shared extension value, opening
/// and closing the prologue with a caller-chosen delimiter.
#[must_use]
pub fn front_matter_with(delimiter: impl Into<String>) -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(FrontMatterWith {
        delimiter: delimiter.into(),
    })
}
