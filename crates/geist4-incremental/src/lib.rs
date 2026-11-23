//! Incremental reparsing for editor use.
//!
//! An editor re-renders on every keystroke, but almost every keystroke
//! changes one block of a document whose other blocks are identical.
//! This crate turns a previous parse and the new source into a new
//! parse that reuses the unchanged blocks, per decision 0045: the edit
//! region (the top-level blocks a line diff marks as changed, plus
//! everything whose byte offsets shifted) is reparsed through the
//! core's fragment parser, unchanged blocks are copied with their
//! spans shifted, and what genuinely depends on the whole document —
//! link reference definitions, extension post-processing — is
//! re-derived document-wide.
//!
//! The full parse is the oracle: [`Editor::update`] falls back to it
//! whenever the region choice cannot be proven safe, and the fuzzed
//! differential corpus compares every incremental result against
//! `parse_with` byte for byte.

#![forbid(unsafe_code)]

use geist4_core::extension::Options;
use geist4_core::node::{CodeBlockData, HeadingData, HtmlBlockData, ListData, Node, SpannedDef};
use geist4_core::parser::{Document, parse_with};
use geist4_core::span::Span;

/// What one top-level block of the checkpoint remembers, so an
/// unchanged copy reproduces the block byte for byte and a shifted
/// copy can have its spans adjusted.
#[derive(Debug, Clone)]
enum BlockSummary<'a> {
    /// A block with no payload: blockquote, thematic break, paragraph.
    /// The paragraph's inline children ride along inside the copy.
    Plain,
    /// A list; the marker data and tightness reproduce verbatim.
    List(ListData),
    /// A list item; the padding reproduces verbatim.
    Item(u32),
    /// A heading; level and setext-ness reproduce verbatim.
    Heading(HeadingData),
    /// A code block; fence shape, info string and literal.
    Code(CodeBlockData<'a>),
    /// A raw HTML block; kind and literal.
    Html(HtmlBlockData<'a>),
    /// An extension's node: the kind name, attributes and the
    /// extension's index in the options list.
    Extension { kind: String, index: Option<usize> },
}

/// One checkpointed top-level block.
#[derive(Debug, Clone)]
struct CheckpointBlock<'a> {
    #[allow(dead_code)]
    summary: BlockSummary<'a>,
    /// The block's source span in the checkpoint's source.
    #[allow(dead_code)]
    span: Span,
    /// The link reference definitions this block contributed, in
    /// document order, with their spans.
    #[allow(dead_code)]
    defs: Vec<SpannedDef<'a>>,
    /// The block's full arena slice in allocation order: the block's
    /// own id first, then every descendant, depth-first. A copy walks
    /// this list and re-allocates each entry with rewritten child
    /// lists and shifted spans.
    #[allow(dead_code)]
    nodes: Vec<CopiedNode<'a>>,
}

/// One node of a copied block: the data needed to re-allocate it into
/// a new arena.
#[derive(Debug, Clone)]
struct CopiedNode<'a> {
    kind: Node<'a>,
    span: Span,
    /// The children as indices into the block's own node list
    /// (positions in `CheckpointBlock::nodes`), not arena ids.
    children: Vec<usize>,
    /// The index of this node within the block's list (its own
    /// position), so parent links resolve during the copy.
    position: usize,
    /// The extension-ownership mark, if the node was claimed.
    owned: Option<usize>,
}

/// The editor state: the options every update runs with.
pub struct Editor {
    options: Options,
}

impl Editor {
    /// Creates an editor that parses with `options`. The options value
    /// is captured: every update parses with the same extensions, the
    /// same highlighter setting, everything.
    #[must_use]
    pub fn new(options: Options) -> Self {
        Editor { options }
    }

    /// Produces the document for `new_source` given the previous
    /// document `previous`, reusing its unchanged blocks.
    ///
    /// The returned span list reports, in new-source coordinates, the
    /// byte ranges that were reparsed (not merely shifted), so a
    /// renderer can patch exactly those regions of its output.
    ///
    /// While the checkpoint machinery is being built out, every update
    /// is a full parse — byte-identical by construction, and the
    /// baseline the benchmark measures against.
    #[must_use]
    pub fn update<'doc>(
        &self,
        _previous: &Document<'doc>,
        new_source: &'doc str,
    ) -> (Document<'doc>, Vec<Span>) {
        (parse_with(new_source, &self.options), Vec::new())
    }
}

/// Computes a stable per-line hash for the line diff (FNV-1a). A hash
/// collision can only cost a reparse, never a wrong result: equality
/// is always confirmed by comparing the actual line bytes.
#[must_use]
pub fn line_hash(line: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in line {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Splits `src` into line-start offsets; each line includes its line
/// ending, and the last line may lack one.
#[must_use]
pub fn line_starts(src: &str) -> Vec<usize> {
    let bytes = src.as_bytes();
    let mut starts = vec![0usize];
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                i += 1;
                starts.push(i);
            }
            b'\r' => {
                i += 1;
                if bytes.get(i) == Some(&b'\n') {
                    i += 1;
                }
                starts.push(i);
            }
            _ => i += 1,
        }
    }
    starts
}

/// One edit region the line diff produced: `[old_start, old_end)` and
/// `[new_start, new_end)` are byte ranges in the old and new source,
/// each covering whole lines (the last line of a source may end
/// without a line ending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRegion {
    /// The region's first byte in the old source (a line start).
    pub old_start: usize,
    /// One past the region's last byte in the old source.
    pub old_end: usize,
    /// The corresponding first byte in the new source (a line start).
    pub new_start: usize,
    /// One past the corresponding last byte in the new source.
    pub new_end: usize,
}

/// Diffs the two sources at line granularity, producing the changed
/// regions in order. Common lines are matched by hash and confirmed by
/// byte comparison; the common prefix and suffix are trimmed first, so
/// a keystroke yields exactly one region.
#[must_use]
pub fn diff_regions(old: &str, new: &str) -> Vec<EditRegion> {
    let old_starts = line_starts(old);
    let new_starts = line_starts(new);
    let old_lines: Vec<&[u8]> = old_starts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = old_starts.get(i + 1).copied().unwrap_or(old.len());
            &old.as_bytes()[start..end]
        })
        .collect();
    let new_lines: Vec<&[u8]> = new_starts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = new_starts.get(i + 1).copied().unwrap_or(new.len());
            &new.as_bytes()[start..end]
        })
        .collect();
    let old_hashes: Vec<u64> = old_lines.iter().map(|l| line_hash(l)).collect();
    let new_hashes: Vec<u64> = new_lines.iter().map(|l| line_hash(l)).collect();

    // Trim the common prefix and suffix; what remains is the change.
    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_hashes[prefix] == new_hashes[prefix]
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < old_lines.len() - prefix
        && suffix < new_lines.len() - prefix
        && old_hashes[old_lines.len() - 1 - suffix] == new_hashes[new_lines.len() - 1 - suffix]
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let old_lo = prefix;
    let old_hi = old_lines.len() - suffix;
    let new_lo = prefix;
    let new_hi = new_lines.len() - suffix;
    if old_lo == old_hi && new_lo == new_hi {
        return Vec::new();
    }
    let old_region_start = old_starts[old_lo.min(old_starts.len() - 1)];
    let old_region_end = old_starts.get(old_hi).copied().unwrap_or(old.len());
    let new_region_start = new_starts[new_lo.min(new_starts.len() - 1)];
    let new_region_end = new_starts.get(new_hi).copied().unwrap_or(new.len());
    vec![EditRegion {
        old_start: old_region_start,
        old_end: old_region_end,
        new_start: new_region_start,
        new_end: new_region_end,
    }]
}

/// Shifts a span by `delta` bytes, clamped at zero.
#[must_use]
pub(crate) fn shift_span(span: Span, delta: isize) -> Span {
    Span::new(span.start.ishift_add(delta), span.end.ishift_add(delta))
}

trait IShift {
    fn ishift_add(self, delta: isize) -> usize;
}

impl IShift for usize {
    fn ishift_add(self, delta: isize) -> usize {
        let moved = self as isize + delta;
        if moved < 0 { 0 } else { moved as usize }
    }
}

/// The crate's internals, published for the test suite and for the
/// benchmark; not part of the semantic version's surface. Everything
/// here is sound to call with any input — the diff and the line
/// splitter are total functions — but the region semantics are
/// internal.
pub mod test_support {
    pub use super::diff_regions;
    pub use super::line_starts;
}
