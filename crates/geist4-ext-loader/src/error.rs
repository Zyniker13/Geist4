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

/// A stable, platform-independent message for a common I/O failure.
///
/// The raw `std::io::Error` text differs by operating system: a
/// missing file reads "No such file or directory (os error 2)" on
/// Unix and "The system cannot find the file specified. (os error 2)"
/// on Windows. The loader's messages are a cross-platform contract
/// pinned by the conformance kit, so the common kinds get fixed
/// wording through this helper. The operating-system error code is
/// dropped: the code for the same kind differs by platform (access
/// denied is 13 on Unix, 5 on Windows). Rarer kinds keep the
/// operating system's own text, which nothing pins.
pub(crate) fn io_message(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => "no such file or directory".to_string(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    }
}
