//! The loader's error type: one line of what failed, and where.

use std::fmt;
use std::ops::Range;

/// A manifest load error.
///
/// The message is one line: what failed, which key, and what would
/// satisfy the rule (the wording the format spec asks for). The span,
/// when known, is the byte range in the manifest source the error
/// applies to — the offending value for validation errors, the exact
/// spot for TOML syntax errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    /// What failed, which key, and what would satisfy the rule.
    pub message: String,
    /// The byte span in the manifest source, when known.
    pub span: Option<Range<usize>>,
}

impl LoadError {
    /// A new error without a source span.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    /// A new error carrying the byte span it applies to.
    pub fn with_span(message: impl Into<String>, span: Range<usize>) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}
