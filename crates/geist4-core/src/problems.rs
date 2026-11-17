//! Diagnostics a parse records about its source: broken references and
//! similar problems the tree itself absorbed.
//!
//! Problems are passive data on [`Document`][crate::parser::Document]:
//! they never change the parse or the rendered HTML, and a host that
//! never reads them pays only their collection. `geist4 check` reports
//! them with their source spans.

use crate::span::Span;

/// One problem the parse noticed in its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// What the problem is, as one line: what failed and what would
    /// satisfy it.
    pub message: String,
    /// Where in the source the problem sits. Inline-level problems
    /// carry the span of the block that holds them, since inline nodes
    /// keep no spans of their own.
    pub span: Span,
}

impl Problem {
    /// A new problem: the message and the span it applies to.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Problem {
            message: message.into(),
            span,
        }
    }
}
