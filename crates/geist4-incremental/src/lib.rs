//! Incremental reparsing for editor use.
//!
//! An editor re-renders on every keystroke, but almost every keystroke
//! changes one block of a document whose other blocks are identical.
//! This crate turns a previous parse and the new source into a new
//! parse that reuses the unchanged blocks, per decision 0045: the edit
//! region (the top-level blocks a line diff marks as changed, plus
//! everything after them) is reparsed through the core's fragment
//! parser, unchanged blocks are copied structurally, and what
//! genuinely depends on the whole document — link reference
//! definitions, broken-reference problems — is re-derived from the
//! two halves.
//!
//! The full parse is the oracle: [`Editor::update`] falls back to it
//! whenever the region choice cannot be proven safe, and the
//! differential corpus in `tests/` compares every incremental result
//! against `parse_with` node for node.
//!
//! ## When the fast path runs
//!
//! Every guard below must hold, or the update is a full parse (which
//! is byte-identical by construction):
//!
//! * no extensions are registered — `begin_parse` state and
//!   document-wide `post_process` would have to be re-driven over the
//!   assembled tree, and a fragment starting at a front-matter
//!   delimiter could be mis-claimed;
//! * the checkpoint succeeded — the previous tree contains no
//!   extension nodes;
//! * the line diff finds at most one region — a multi-site edit is
//!   not split yet;
//! * the byte-identical prefix check passes — everything before the
//!   first changed block is the same in both sources;
//! * every link reference definition of the previous document lies
//!   inside the prefix, so no prefix reference can have resolved
//!   against a definition inside the reparsed region;
//! * the fragment defines no link references of its own, so a
//!   reference the previous parse left broken (literal text) cannot
//!   become resolved in the new document while the copy keeps it
//!   broken;
//! * the boundary is hard: the update starts at the beginning of the
//!   document, or the old source ends the prefix with a blank line
//!   (which closes paragraphs and HTML blocks) and the last copied
//!   block is neither a list (items continue across single blank
//!   lines) nor an indented code block (blank lines are interior to
//!   it). Without this, a fragment whose first line would continue
//!   the last copied block parses differently in context than alone.
//!
//! References in the fragment: the current guards refuse any
//! reparse whose fragment contains a `[` (a definition, a reference
//! link, an image or a footnote marker could all resolve differently
//! out of context), so the fragment cannot disagree with the copy it
//! replaces about resolution, and the prefix's definition map carries
//! over verbatim. A later stage can narrow this guard by seeding the
//! fragment parse with the prefix's definitions, the way a full parse
//! sees them in document order.

#![forbid(unsafe_code)]

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::{AttrValue, Options};
use geist4_core::node::{Node, SpannedDef};
use geist4_core::parser::{Document, normalize_label, parse_with};
use geist4_core::span::Span;
/// One node of a checkpointed block: the data needed to re-allocate it
/// into a new arena.
#[derive(Debug, Clone)]
struct CopiedNode<'a> {
    kind: Node<'a>,
    span: Span,
    /// The children as positions in the block's own node list, not
    /// arena ids.
    children: Vec<usize>,
    /// The attributes set on the node, in set order.
    attrs: Vec<(String, AttrValue)>,
    /// The extension-ownership mark, if the node was claimed.
    owned: Option<usize>,
}

/// One checkpointed top-level block: its source span and its full
/// subtree in pre-order (the block's own node first, then every
/// descendant, depth-first).
#[derive(Debug, Clone)]
struct CheckpointBlock<'a> {
    span: Span,
    nodes: Vec<CopiedNode<'a>>,
}

/// The editor's checkpoint: everything a cheap update needs from the
/// previous parse. It borrows the old source, which the previous
/// document borrows too, so both live as long as the caller's buffer.
#[derive(Debug, Clone)]
struct Checkpoint<'a> {
    /// The top-level blocks in document order.
    blocks: Vec<CheckpointBlock<'a>>,
    /// The source the checkpoint was taken from.
    old_source: &'a str,
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
    /// document `previous` and the source it was parsed from,
    /// reusing the unchanged blocks where the guards in the
    /// [module documentation](self) prove that safe, and falling back
    /// to a full parse whenever they do not.
    ///
    /// `old_source` must be the exact source `previous` was parsed
    /// from; the crate cannot recover it from the document, because
    /// the arena stores nodes, not text. Both sources must outlive the
    /// returned document: the copied prefix borrows `old_source` and
    /// the reparsed region borrows `new_source`.
    ///
    /// The returned span list reports, in new-source coordinates, the
    /// byte ranges that were reparsed rather than copied, so a
    /// renderer can patch exactly those regions of its output. A
    /// full-parse fallback reports the whole source; an update on an
    /// unchanged source reports nothing.
    #[must_use]
    pub fn update<'doc>(
        &self,
        previous: &Document<'doc>,
        old_source: &'doc str,
        new_source: &'doc str,
    ) -> (Document<'doc>, Vec<Span>) {
        // The fast path is plain CommonMark only, this stage: with
        // extensions registered, per-parse `begin_parse` state and
        // document-wide `post_process` would have to be re-driven over
        // the assembled tree.
        if !self.options.extensions().is_empty() {
            return (
                parse_with(new_source, &self.options),
                vec![whole(new_source)],
            );
        }
        let Some(checkpoint) = checkpoint_from(previous, old_source) else {
            return (
                parse_with(new_source, &self.options),
                vec![whole(new_source)],
            );
        };
        match diff_regions(old_source, new_source).as_slice() {
            // The sources are identical: copy the whole document at
            // shift zero. The result is value-identical to a full parse
            // and costs no parse.
            [] => (
                assemble(
                    previous,
                    &checkpoint,
                    new_source,
                    checkpoint.blocks.len(),
                    old_source.len(),
                    &empty_document(&self.options),
                ),
                Vec::new(),
            ),
            [region] => self.update_region(previous, &checkpoint, new_source, region),
            // A multi-site edit is not split into regions yet.
            _ => (
                parse_with(new_source, &self.options),
                vec![whole(new_source)],
            ),
        }
    }

    /// The one-region incremental path: copy the byte-identical prefix
    /// blocks, reparse from the first changed block to the end of the
    /// document, and re-derive the document-wide collections.
    fn update_region<'doc>(
        &self,
        previous: &Document<'doc>,
        checkpoint: &Checkpoint<'doc>,
        new_source: &'doc str,
        region: &EditRegion,
    ) -> (Document<'doc>, Vec<Span>) {
        let full = || {
            (
                parse_with(new_source, &self.options),
                vec![whole(new_source)],
            )
        };
        let Some(first) = region_blocks(checkpoint, region.old_start, region.old_end) else {
            return full();
        };
        // The prefix through the first changed block's start is
        // byte-identical in both sources: the diff trimmed the common
        // prefix, and any block starting before the region ends at or
        // before it.
        let prefix_end = checkpoint.blocks[first].span.start;
        if prefix_end > new_source.len()
            || old_source(checkpoint)[..prefix_end] != new_source[..prefix_end]
        {
            return full();
        }
        // Guard: every previous definition lies in the prefix, so no
        // prefix reference resolved against a definition the reparse
        // discards.
        if !previous.defs.iter().all(|def| def.span.end <= prefix_end) {
            return full();
        }
        // Guard: the boundary is hard (see the module documentation).
        if !hard_boundary(checkpoint, first, prefix_end) {
            return full();
        }
        let fragment_source = &new_source[prefix_end..];
        // Guard: the fragment holds no `[` at all, so it contains no
        // link reference definitions and no reference links or images.
        // The prefix's definitions therefore stay complete (the
        // previous guard proved they end before the boundary), the
        // fragment cannot resolve or break a reference differently
        // from the copy it replaces, and a definition it would add
        // cannot shadow the prefix's map. A fragment that mentions a
        // bracket takes the full parse; the corpus pins that the
        // result still matches.
        if fragment_source.contains('[') {
            return full();
        }
        let fragment = geist4_core::parser::parse_fragment(fragment_source, &self.options);
        let document = assemble(
            previous, checkpoint, new_source, first, prefix_end, &fragment,
        );
        let changed = vec![Span::new(prefix_end, new_source.len())];
        (document, changed)
    }
}

/// A span covering all of `source`.
fn whole(source: &str) -> Span {
    Span::new(0, source.len())
}

/// The checkpoint's old source.
fn old_source<'doc>(checkpoint: &Checkpoint<'doc>) -> &'doc str {
    checkpoint.old_source
}

/// Whether the update can start a reparse at `prefix_end`: either the
/// document begins there, or the old source ends the prefix with a
/// blank line and the last copied block neither continues across
/// blank lines (a list) nor contains them (an indented code block).
fn hard_boundary(checkpoint: &Checkpoint<'_>, first: usize, prefix_end: usize) -> bool {
    if prefix_end == 0 {
        return true;
    }
    let last = &checkpoint.blocks[first - 1];
    if matches!(last.nodes[0].kind, Node::List(_)) {
        return false;
    }
    if let Node::CodeBlock(data) = &last.nodes[0].kind {
        if !data.fenced {
            return false;
        }
    }
    ends_with_blank_line(&checkpoint.old_source[..prefix_end])
}

/// Whether `src`'s last line is empty after trimming spaces and tabs.
fn ends_with_blank_line(src: &str) -> bool {
    match src.lines().last() {
        Some(last) => last
            .trim_matches(|c: char| c == ' ' || c == '\t')
            .is_empty(),
        None => false,
    }
}

/// An empty fragment, for paths where the reparse contributes nothing:
/// the assembly needs a `Document` value with an empty arena, and a
/// fragment parse of the empty string provides one.
fn empty_document(options: &Options) -> Document<'static> {
    geist4_core::parser::parse_fragment("", options)
}

/// Which top-level block the byte range `[start, end)` first
/// intersects; the reparse runs from there to the end of the document.
///
/// A region that intersects no block sits in the gap between two
/// blocks (blank lines): the reparse starts at the last block before
/// the gap, because an inserted line there may belong to that block's
/// parse. With no block before the gap either, the update falls back.
fn region_blocks(checkpoint: &Checkpoint<'_>, start: usize, end: usize) -> Option<usize> {
    for (index, block) in checkpoint.blocks.iter().enumerate() {
        if block.span.start < end && block.span.end > start {
            return Some(index);
        }
    }
    checkpoint
        .blocks
        .iter()
        .rposition(|block| block.span.end <= start)
}

/// Builds the new document: the copied prefix blocks, then the
/// fragment's tree with spans shifted into document coordinates, and
/// the document-wide collections re-derived from the two halves.
///
/// With `first` equal to the block count and `prefix_end` equal to the
/// old source's length this copies the whole checkpoint — the
/// identical-sources path.
fn assemble<'doc>(
    previous: &Document<'doc>,
    checkpoint: &Checkpoint<'doc>,
    new_source: &'doc str,
    first: usize,
    prefix_end: usize,
    fragment: &Document<'doc>,
) -> Document<'doc> {
    let delta = prefix_end as isize;
    let mut arena = Arena::new();
    arena.reserve(
        checkpoint.blocks[..first]
            .iter()
            .map(|b| b.nodes.len())
            .sum::<usize>()
            + fragment.arena.len()
            + 1,
    );
    let root = arena.alloc(Node::Document, Span::new(0, new_source.len()));
    for block in &checkpoint.blocks[..first] {
        copy_checkpointed(&mut arena, &block.nodes, root, 0);
    }
    for top in fragment.arena.children(fragment.root).to_vec() {
        copy_fragment(&mut arena, fragment, top, root, delta);
    }

    // Definitions: the prefix's carry verbatim (its bytes are
    // identical), the fragment's shift into document coordinates. With
    // `first` covering everything, the fragment contributes nothing and
    // every definition carries.
    let mut defs: Vec<SpannedDef<'doc>> = previous
        .defs
        .iter()
        .filter(|def| def.span.end <= prefix_end)
        .cloned()
        .collect();
    for def in &fragment.defs {
        defs.push(SpannedDef {
            def: def.def.clone(),
            span: shift_span(def.span, delta),
            titled: def.titled,
        });
    }
    // The map is first-wins in document order: the prefix's
    // definitions precede the fragment's, exactly as in a full parse.
    // With the guards holding, `previous.link_defs` already is the
    // prefix's map.
    let mut link_defs = previous.link_defs.clone();
    for def in &defs {
        link_defs
            .entry(normalize_label(&def.def.label))
            .or_insert_with(|| def.def.clone());
    }
    // Problems follow the same partition as definitions.
    let mut problems: Vec<geist4_core::problems::Problem> = previous
        .problems
        .iter()
        .filter(|problem| problem.span.end <= prefix_end)
        .map(|problem| geist4_core::problems::Problem::new(problem.message.clone(), problem.span))
        .collect();
    for problem in &fragment.problems {
        problems.push(geist4_core::problems::Problem::new(
            problem.message.clone(),
            shift_span(problem.span, delta),
        ));
    }

    Document {
        arena,
        root,
        link_defs,
        defs,
        problems,
    }
}

/// Re-allocates a checkpointed subtree into `arena` under `parent`,
/// with every span shifted by `delta`. The list is pre-order with
/// children referencing parents by position, so two passes wire it:
/// allocate in list order, then attach children and reapply
/// attributes.
fn copy_checkpointed<'a>(
    arena: &mut Arena<'a>,
    nodes: &[CopiedNode<'a>],
    parent: NodeId,
    delta: isize,
) {
    if nodes.is_empty() {
        return;
    }
    let mut ids = Vec::with_capacity(nodes.len());
    for node in nodes {
        let id = arena.alloc(node.kind.clone(), shift_span(node.span, delta));
        for (name, value) in &node.attrs {
            arena.set_attr(id, name.clone(), value.clone());
        }
        if let Some(owner) = node.owned {
            arena.set_extension(id, owner);
        }
        ids.push(id);
    }
    for (index, node) in nodes.iter().enumerate() {
        for &child in &node.children {
            arena.push_child(ids[index], ids[child]);
        }
    }
    arena.push_child(parent, ids[0]);
}

/// Copies one fragment subtree into `arena` under `parent`, shifting
/// every span by `delta`.
fn copy_fragment<'doc>(
    arena: &mut Arena<'doc>,
    fragment: &Document<'doc>,
    source: NodeId,
    parent: NodeId,
    delta: isize,
) {
    let new_id = arena.alloc(
        fragment.arena.kind(source).clone(),
        shift_span(fragment.arena.span(source), delta),
    );
    for (name, value) in fragment.arena.attrs(source) {
        arena.set_attr(new_id, name.clone(), value.clone());
    }
    if let Some(owner) = fragment.arena.extension_of(source) {
        arena.set_extension(new_id, owner);
    }
    arena.push_child(parent, new_id);
    for child in fragment.arena.children(source).to_vec() {
        copy_fragment(arena, fragment, child, new_id, delta);
    }
}

/// Extracts a checkpoint from a finished document. Returns `None` when
/// the tree carries anything this stage cannot copy — an extension
/// node anywhere.
#[must_use]
fn checkpoint_from<'a>(document: &Document<'a>, source: &'a str) -> Option<Checkpoint<'a>> {
    let mut blocks = Vec::new();
    for &child in document.arena.children(document.root) {
        let mut nodes = Vec::new();
        collect_subtree(&document.arena, child, &mut nodes)?;
        blocks.push(CheckpointBlock {
            span: document.arena.span(child),
            nodes,
        });
    }
    Some(Checkpoint {
        blocks,
        old_source: source,
    })
}

/// Walks a subtree depth-first, pre-order, into `nodes`; children
/// reference parents by list position. Returns `None` when a node's
/// kind cannot be copied at this stage.
fn collect_subtree<'a>(
    arena: &Arena<'a>,
    id: NodeId,
    nodes: &mut Vec<CopiedNode<'a>>,
) -> Option<()> {
    if matches!(arena.kind(id), Node::Extension(_)) {
        return None;
    }
    let mine = nodes.len();
    nodes.push(CopiedNode {
        kind: arena.kind(id).clone(),
        span: arena.span(id),
        children: Vec::new(),
        attrs: arena.attrs(id).to_vec(),
        owned: arena.extension_of(id),
    });
    for child in arena.children(id) {
        let at = nodes.len();
        collect_subtree(arena, *child, nodes)?;
        nodes[mine].children.push(at);
    }
    Some(())
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
