//! The mdast export: a Geist document as the tree shape the reference
//! JavaScript implementation (`mdast-util-from-markdown`, the parser behind
//! remark) produces for the same source.
//!
//! The export is pure tree-in, tree-out: it reads a source string and its
//! parsed [`geist4_core::Document`] and builds mdast-shaped nodes with
//! `position` points mapped from the source bytes through a line index.
//! Nothing here parses; nothing here does I/O. The tests compare the
//! serialized trees byte for byte against the reference implementation's
//! trees for every CommonMark spec example and a set of focused probes.
//!
//! The reference behaviors that the Geist tree alone does not show — the
//! merged text runs across soft line breaks, the `spread` flags, the
//! `code` value's trailing newline, the definition and reference node
//! shapes — are derived here from the tree and the source, and each is
//! pinned by a fixture or a unit test.

mod nodes;
mod render;

pub use nodes::Mdast;
pub use render::to_mdast;

/// A `position` point as mdast serializes it: 1-based `line`, 1-based
/// `column` counted in UTF-16 code units since the line start, and the
/// 0-based `offset` in UTF-16 code units into the whole source.
///
/// The reference implementation sees the source as a JavaScript string, so
/// offsets and columns count UTF-16 code units: one for a BMP character,
/// two for a surrogate pair. Tabs are one column; they are not expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Point {
    /// The 1-based line number.
    pub line: u32,
    /// The 1-based column in UTF-16 code units.
    pub column: u32,
    /// The 0-based offset in UTF-16 code units.
    pub offset: u32,
}

/// A `position` object: the `start` and `end` points of a node's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Position {
    /// Where the node starts, at its first source character.
    pub start: Point,
    /// Where the node ends, one past its last source character.
    pub end: Point,
}

/// Maps byte offsets in one source to [`Point`]s.
///
/// Lines are separated by the spec's line endings: `\n`, `\r\n` or a lone
/// `\r`. The index records where each line starts in bytes and in UTF-16
/// code units, so a point is a binary search plus a scan of one line.
pub struct LineIndex {
    /// The byte offset where each line starts; `starts[0]` is 0. A source
    /// ending with a line ending records the empty final line's start.
    starts: Vec<usize>,
    /// The UTF-16 offset where each line starts, parallel to `starts`.
    utf16_starts: Vec<u32>,
}

impl LineIndex {
    /// Indexes `src`.
    #[must_use]
    pub fn new(src: &str) -> Self {
        let bytes = src.as_bytes();
        let mut starts = vec![0usize];
        let mut utf16_starts = vec![0u32];
        let mut utf16 = 0u32;
        let mut i = 0usize;
        while i < bytes.len() {
            let byte = bytes[i];
            if byte == b'\n' {
                i += 1;
                utf16 += 1;
            } else if byte == b'\r' {
                let steps = if bytes.get(i + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                i += steps;
                utf16 += steps as u32;
            } else {
                // One UTF-16 code unit per BMP character, two per
                // surrogate pair; the chunk to the next line ending is
                // valid UTF-8, so char boundaries are intact.
                let line_end = bytes[i..]
                    .iter()
                    .position(|b| matches!(b, b'\n' | b'\r'))
                    .map_or(bytes.len(), |at| i + at);
                utf16 += src[i..line_end].chars().map(char::len_utf16).sum::<usize>() as u32;
                i = line_end;
                continue;
            }
            starts.push(i);
            utf16_starts.push(utf16);
        }
        LineIndex {
            starts,
            utf16_starts,
        }
    }

    /// The point for the byte offset `at`, which must be at a character
    /// boundary or the source's end.
    #[must_use]
    pub fn point(&self, src: &str, at: usize) -> Point {
        let line = match self.starts.binary_search(&at) {
            Ok(found) => found,
            Err(insert) => insert - 1,
        };
        let line_start = self.starts[line];
        let column_units = utf16_len(&src[line_start..at]);
        Point {
            line: line as u32 + 1,
            column: column_units + 1,
            offset: self.utf16_starts[line] + column_units,
        }
    }

    /// The index of the line whose start is the greatest one at or before
    /// `at`.
    pub(crate) fn line_containing(&self, at: usize) -> usize {
        match self.starts.binary_search(&at) {
            Ok(found) => found,
            Err(insert) => insert - 1,
        }
    }

    /// The byte offset where line `index` starts.
    pub(crate) fn line_start(&self, index: usize) -> usize {
        self.starts[index]
    }

    /// How many lines the index holds.
    pub(crate) fn line_count(&self) -> usize {
        self.starts.len()
    }
}

/// The UTF-16 code-unit length of a string slice.
fn utf16_len(s: &str) -> u32 {
    s.chars().map(char::len_utf16).sum::<usize>() as u32
}

/// The `position` for the byte span `[start, end)` of `src`.
pub(crate) fn position_for(src: &str, lines: &LineIndex, start: usize, end: usize) -> Position {
    Position {
        start: lines.point(src, start),
        end: lines.point(src, end),
    }
}
