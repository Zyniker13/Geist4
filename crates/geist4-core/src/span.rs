//! Byte-offset spans into source text.

/// A half-open byte range `[start, end)` into the source input.
///
/// Every node in the document tree records the span of source text it was
/// parsed from, so tools can map output back to the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// The first byte of the span.
    pub start: usize,
    /// One past the last byte of the span.
    pub end: usize,
}

impl Span {
    /// Creates a span covering `[start, end)`.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The empty span at `at`.
    #[must_use]
    pub fn empty(at: usize) -> Self {
        Span { start: at, end: at }
    }

    /// The number of bytes covered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span covers no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// The smallest span covering both `self` and `other`.
    #[must_use]
    pub fn join(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
