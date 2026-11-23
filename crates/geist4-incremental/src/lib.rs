//! Incremental reparsing for editor use.
//!
//! An editor re-renders on every keystroke, but almost every keystroke
//! changes one block of a document whose other blocks are identical.
//! This crate turns a previous parse and the new source into a new
//! parse that reuses the unchanged blocks, per decision 0045: the
//! blocks before and after the edit are mounted structurally from the
//! previous arena (the tail once a hard block boundary follows the
//! edit), the edit window between them is reparsed through the core's
//! fragment parser, and what genuinely depends on the whole document —
//! link reference definitions, broken-reference problems — is
//! re-derived from the parts.
//!
//! The full parse is the oracle: [`Editor::update`] falls back to it
//! whenever the region choice cannot be proven safe, and the
//! differential corpus in `tests/` compares every incremental result
//! against `parse_with` node for node.
//!
//! ## When the fast path runs
//!
//! Every guard must hold, or the update is a full parse (which is
//! value-identical by construction):
//!
//! * no extensions are registered — `begin_parse` state and
//!   document-wide `post_process` would have to be re-driven over the
//!   assembled tree, and a fragment starting at a front-matter
//!   delimiter could be mis-claimed;
//! * the line diff finds at most one region — a multi-site edit is
//!   not split into regions yet;
//! * the byte-identical prefix check passes — everything before the
//!   first changed block is the same in both sources;
//! * every link reference definition of the previous document lies
//!   inside the prefix, so no prefix reference can have resolved
//!   against a definition inside the reparsed region, and the copied
//!   tail cannot carry definitions either;
//! * the bytes before the first reparsed block, when the reparse
//!   starts at the document's first block, are whole blank lines —
//!   anything else there is outside every block and takes the full
//!   parse;
//! * the boundary is hard: the update starts at the beginning of the
//!   document, or the old source ends the prefix with a blank line
//!   (which closes paragraphs and HTML blocks) and the last copied
//!   block is neither a list (items continue across single blank
//!   lines) nor an indented code block (blank lines are interior to
//!   it). Without this, a fragment whose first line would continue
//!   the last copied block parses differently in context than alone;
//! * the reparse window holds no `[` at all, so it contains no link
//!   reference definitions and no reference links or images: the
//!   prefix's definition map carries over verbatim and the window
//!   cannot disagree with the copy it replaces about resolution. A
//!   later stage can narrow this guard by seeding the fragment parse
//!   with the prefix's definitions, the way a full parse sees them in
//!   document order.
//!
//! ## The reparse window and the copied tail
//!
//! The window starts at the first changed top-level block. Its end is
//! chosen among the block boundaries after the region: the first one
//! where both sources end the boundary with a blank line and the
//! blocks on both sides of it tolerate a following blank line (no
//! list, no unfenced code block). A window that parses to such a
//! boundary lets everything from the boundary on mount from the
//! previous arena with every span shifted by one delta — the common
//! suffix of the line diff is byte-identical, so the mounted values
//! are the ones a full parse would produce. When no boundary
//! qualifies, the window runs to end-of-document and the tail is
//! empty; when the window would have to contain a `[`, the update
//! falls back to a full parse.

#![forbid(unsafe_code)]

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::Options;
use geist4_core::node::{Node, SpannedDef};
use geist4_core::parser::{Document, parse_with};
use geist4_core::span::Span;

/// Metadata for one top-level block, kept between updates.
#[derive(Debug, Clone, Copy)]
struct BlockMeta {
    /// The block's span in the source its document was parsed from.
    span: Span,
    /// The block's root node in that document's arena. Valid as long
    /// as the caller keeps the document alive — the update API takes
    /// the previous document by reference, so the borrow is checked.
    root: NodeId,
    /// Whether the block kind continues across a blank line (a list)
    /// or holds blank lines (an unfenced code block): a reparse must
    /// not start or end right after such a block.
    tail_continues: bool,
}

/// The checkpoint: per-top-level-block metadata for the next update.
/// Cheap to build (one walk of the root's children) and cheap to
/// carry (plain data, no node copies).
#[derive(Debug, Clone)]
struct Checkpoint {
    /// The top-level blocks in document order.
    blocks: Vec<BlockMeta>,
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
    /// returned document: the mounted prefix and tail borrow
    /// `old_source`, the reparsed window borrows `new_source`.
    ///
    /// The returned span list reports, in new-source coordinates, the
    /// byte ranges that were reparsed or whose spans moved, so a
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
        let full = || {
            (
                parse_with(new_source, &self.options),
                vec![whole(new_source)],
            )
        };
        // The fast path is plain CommonMark only, this stage: with
        // extensions registered, per-parse `begin_parse` state and
        // document-wide `post_process` would have to be re-driven over
        // the assembled tree.
        if !self.options.extensions().is_empty() {
            return full();
        }
        let Some(checkpoint) = checkpoint_from(previous) else {
            return full();
        };
        match diff_regions(old_source, new_source).as_slice() {
            // The sources are identical: mount the whole document at
            // shift zero. The result is value-identical to a full
            // parse and costs no parse.
            [] => (
                assemble(
                    previous,
                    &checkpoint,
                    new_source,
                    checkpoint.blocks.len(),
                    old_source.len(),
                    &empty_document(&self.options),
                    None,
                ),
                Vec::new(),
            ),
            [region] => self.update_region(previous, checkpoint, old_source, new_source, region),
            // A multi-site edit is not split into regions yet.
            _ => full(),
        }
    }

    /// The one-region incremental path: mount the byte-identical
    /// prefix blocks, reparse the chosen window, mount the unchanged
    /// tail with shifted spans, and re-derive the document-wide
    /// collections. The new checkpoint's metadata is built during
    /// assembly: mounted blocks report their fresh root ids as they
    /// are allocated.
    fn update_region<'doc>(
        &self,
        previous: &Document<'doc>,
        checkpoint: Checkpoint,
        old_source: &'doc str,
        new_source: &'doc str,
        region: &EditRegion,
    ) -> (Document<'doc>, Vec<Span>) {
        let full = || {
            (
                parse_with(new_source, &self.options),
                vec![whole(new_source)],
            )
        };
        let Some(first) = region_blocks(&checkpoint, region.old_start, region.old_end) else {
            return full();
        };
        // The prefix through the first changed block's start is
        // byte-identical in both sources: the diff trimmed the common
        // prefix, and any block starting before the region ends at or
        // before it. The comment is a claim, not a mechanism: verify
        // it before slicing, and take the full parse otherwise.
        let prefix_end = checkpoint.blocks[first].span.start;
        if old_source.len() < prefix_end
            || new_source.len() < prefix_end
            || old_source[..prefix_end] != new_source[..prefix_end]
        {
            return full();
        }
        // Guard: bytes before the first reparsed block belong to no
        // block. They are droppable only as whole blank lines — a full
        // parse gives blank lines no nodes, definitions or problems —
        // and only then does the fragment start at a line start. Any
        // other leading bytes (a block span starting inside its first
        // line, say) take the full parse.
        if first == 0 && prefix_end > 0 && !blank_prefix(&old_source[..prefix_end]) {
            return full();
        }
        // Guard: every previous definition lies in the prefix, so no
        // prefix reference resolved against a definition the reparse
        // discards, and the copied tail cannot carry definitions
        // either (they would have been found here).
        if !previous.defs.iter().all(|def| def.span.end <= prefix_end) {
            return full();
        }
        // Guard: the prefix boundary is hard (see the module
        // documentation).
        if !hard_boundary(&checkpoint, first, prefix_end, old_source) {
            return full();
        }
        let delta = new_source.len() as isize - old_source.len() as isize;

        // Choose the window's end: the first block boundary at or
        // after the changed region where both sources end the boundary
        // with a blank line, the block before it tolerates a following
        // blank line, and the window's own last block does not
        // continue across one. A candidate whose window contains a `[`
        // ends the search — the bracket guard is monotone in the
        // window size — and the end-of-document window decides it
        // then.
        let start = checkpoint
            .blocks
            .partition_point(|block| block.span.start < region.old_end);
        let mut tail_from = checkpoint.blocks.len();
        let mut window_end_old = old_source.len();
        let mut window_end_new = new_source.len();
        let mut fragment: Option<Document<'doc>> = None;
        let mut attempts = 0usize;
        for index in start..checkpoint.blocks.len() {
            let candidate_old = checkpoint.blocks[index].span.start;
            let candidate_new = (candidate_old as isize + delta) as usize;
            if candidate_new > new_source.len() {
                break;
            }
            // Index 0 has no block before it, and its boundary (the
            // document start) has no blank line before it either, so
            // the checks below would reject it — skip it explicitly to
            // avoid the underflow.
            if index == 0
                || checkpoint.blocks[index - 1].tail_continues
                || !ends_with_blank_line(&old_source[..candidate_old])
                || !ends_with_blank_line(&new_source[..candidate_new])
            {
                continue;
            }
            let candidate_fragment = &new_source[prefix_end..candidate_new];
            if candidate_fragment.contains('[') {
                break;
            }
            // A cap on window parses: a document of many paragraphs
            // offers a candidate boundary after every block, and
            // parsing each would be quadratic. Eight attempts cover
            // every realistic doc; beyond that the end-of-document
            // window below is correct, just larger.
            attempts += 1;
            if attempts > 8 {
                break;
            }
            let parsed = geist4_core::parser::parse_fragment(candidate_fragment, &self.options);
            let last_continues = parsed
                .arena
                .children(parsed.root)
                .last()
                .map(|&id| tail_continues_kind(parsed.arena.kind(id)))
                .unwrap_or(false);
            if last_continues {
                continue;
            }
            tail_from = index;
            window_end_old = candidate_old;
            window_end_new = candidate_new;
            fragment = Some(parsed);
            break;
        }
        if fragment.is_none() {
            // No qualifying window: the end-of-document window is
            // always correct about the tail (there is none), subject
            // only to the bracket guard.
            let rest = &new_source[prefix_end..];
            if rest.contains('[') {
                return full();
            }
            tail_from = checkpoint.blocks.len();
            window_end_old = old_source.len();
            window_end_new = new_source.len();
            fragment = Some(geist4_core::parser::parse_fragment(rest, &self.options));
        }
        let fragment = fragment.expect("set in both branches above");
        // Guard: no previous problem straddles a partition boundary —
        // the prefix boundary or the tail boundary. A problem spanning
        // one would be neither carried nor regenerated whole.
        let partition = [prefix_end, window_end_old];
        if previous.problems.iter().any(|problem| {
            partition
                .iter()
                .any(|&at| problem.span.start < at && at < problem.span.end)
        }) {
            return full();
        }
        let tail_delta = window_end_new as isize - window_end_old as isize;
        let document = assemble(
            previous,
            &checkpoint,
            new_source,
            first,
            prefix_end,
            &fragment,
            Some((tail_from, window_end_old, tail_delta)),
        );
        let changed = vec![Span::new(prefix_end, new_source.len())];
        (document, changed)
    }
}

/// A span covering all of `source`.
fn whole(source: &str) -> Span {
    Span::new(0, source.len())
}

/// Whether a node kind continues across a blank line (a list) or
/// holds blank lines (an unfenced code block).
fn tail_continues_kind(kind: &Node<'_>) -> bool {
    match kind {
        Node::List(_) => true,
        Node::CodeBlock(data) => !data.fenced,
        _ => false,
    }
}

/// Whether the update can start a reparse at `prefix_end`: either the
/// document begins there, or the old source ends the prefix with a
/// blank line and the last copied block neither continues across
/// blank lines (a list) nor contains them (an indented code block).
fn hard_boundary(
    checkpoint: &Checkpoint,
    first: usize,
    prefix_end: usize,
    old_source: &str,
) -> bool {
    if first == 0 {
        // No copied blocks precede the reparse, so nothing can
        // continue into it; whether the reparse may start at the first
        // block's start is decided by the blank-prefix gate at the
        // call site.
        return true;
    }
    if checkpoint.blocks[first - 1].tail_continues {
        return false;
    }
    ends_with_blank_line(&old_source[..prefix_end])
}

/// Whether `src` is a run of whole blank lines: every line blank
/// after trimming spaces and tabs, and the text ending with a line
/// ending, so the byte after `src` starts a line. Such a prefix
/// produces no blocks, no definitions and no problems in a full
/// parse, so dropping it from a reparse changes nothing.
fn blank_prefix(src: &str) -> bool {
    (src.ends_with('\n') || src.ends_with('\r'))
        && src.lines().all(|line| {
            line.trim_matches(|c: char| c == ' ' || c == '\t')
                .is_empty()
        })
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
/// intersects; the reparse runs from there.
///
/// A region that intersects no block sits in the gap between two
/// blocks (blank lines): the reparse starts at the last block before
/// the gap, because an inserted line there may belong to that block's
/// parse. With no block before the gap either, the update falls back.
fn region_blocks(checkpoint: &Checkpoint, start: usize, end: usize) -> Option<usize> {
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

/// Builds the new document: the mounted prefix blocks, the fragment's
/// tree with spans shifted into document coordinates, and — when a
/// tail qualifies — the mounted tail blocks with spans shifted by the
/// tail's delta. The document-wide collections are re-derived from the
/// parts.
///
/// With `first` equal to the block count, `prefix_end` equal to the
/// old source's length and no tail this mounts the whole checkpoint —
/// the identical-sources path.
fn assemble<'doc>(
    previous: &Document<'doc>,
    checkpoint: &Checkpoint,
    new_source: &'doc str,
    first: usize,
    prefix_end: usize,
    fragment: &Document<'doc>,
    tail: Option<(usize, usize, isize)>,
) -> Document<'doc> {
    let delta = prefix_end as isize;
    let mut arena = Arena::new();
    arena.reserve(checkpoint.blocks.len() + fragment.arena.len() + 1);
    let root = arena.alloc(Node::Document, Span::new(0, new_source.len()));
    for meta in &checkpoint.blocks[..first] {
        mount_subtree(&mut arena, &previous.arena, meta.root, root, 0);
    }
    for top in fragment.arena.children(fragment.root).to_vec() {
        mount_subtree(&mut arena, &fragment.arena, top, root, delta);
    }
    if let Some((from, window_end_old, tail_delta)) = tail {
        for meta in &checkpoint.blocks[from..] {
            mount_subtree(&mut arena, &previous.arena, meta.root, root, tail_delta);
        }
        let _ = window_end_old;
    }

    // Definitions: the prefix's carry verbatim (its bytes are
    // identical and its spans already sit at the same offsets in both
    // sources). The bracket guard proves the window defines none, and
    // the definitions guard proves the discards held none — so the
    // map is the prefix's map. With `first` covering everything, this
    // is every definition of the previous document.
    let defs: Vec<SpannedDef<'doc>> = previous
        .defs
        .iter()
        .filter(|def| def.span.end <= prefix_end)
        .cloned()
        .collect();
    // The map is the prefix's map, verbatim: the guards prove the
    // reparsed parts hold no definitions of their own, so nothing is
    // inserted and nothing shadows.
    let link_defs = previous.link_defs.clone();
    // Problems follow the same partition: the prefix's carry
    // verbatim, the fragment's shift into document coordinates, the
    // tail's shift by the tail's delta. The straddle guard proves
    // every problem lands in exactly one part.
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
    if let Some((_, window_end_old, tail_delta)) = tail {
        for problem in &previous.problems {
            if problem.span.start >= window_end_old {
                problems.push(geist4_core::problems::Problem::new(
                    problem.message.clone(),
                    shift_span(problem.span, tail_delta),
                ));
            }
        }
    }

    Document {
        arena,
        root,
        link_defs,
        defs,
        problems,
    }
}

/// Mounts `source`'s subtree from `src` into `arena` under `parent`,
/// shifting every span by `delta`. Returns the new root's id.
fn mount_subtree<'doc>(
    arena: &mut Arena<'doc>,
    src: &Arena<'doc>,
    source: NodeId,
    parent: NodeId,
    delta: isize,
) -> NodeId {
    let new_id = arena.alloc(
        src.kind(source).clone(),
        shift_span(src.span(source), delta),
    );
    for (name, value) in src.attrs(source) {
        arena.set_attr(new_id, name.clone(), value.clone());
    }
    if let Some(owner) = src.extension_of(source) {
        arena.set_extension(new_id, owner);
    }
    arena.push_child(parent, new_id);
    for child in src.children(source).to_vec() {
        mount_subtree(arena, src, child, new_id, delta);
    }
    new_id
}

/// Extracts a checkpoint from a finished document: per-top-level-block
/// metadata only. Always succeeds for a plain parse — with no
/// extensions registered, no extension nodes can exist in the tree
/// (only extension hooks create them), so there is nothing else to
/// refuse.
#[must_use]
fn checkpoint_from(document: &Document<'_>) -> Option<Checkpoint> {
    let blocks = document
        .arena
        .children(document.root)
        .iter()
        .map(|&child| BlockMeta {
            span: document.arena.span(child),
            root: child,
            tail_continues: tail_continues_kind(document.arena.kind(child)),
        })
        .collect();
    Some(Checkpoint { blocks })
}

/// The end offset of the line starting at `start`: one past its
/// ending — `\n`, `\r\n`, or a lone `\r` — or the source length for a
/// final line without an ending. Matches [`line_starts`]' splitting.
fn line_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => return i + 1,
            b'\r' => {
                if bytes.get(i + 1) == Some(&b'\n') {
                    return i + 2;
                }
                return i + 1;
            }
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The start offset of the line ending at `end` (one past its ending,
/// or the source length for a final line without one): the previous
/// line's end. Matches [`line_starts`]' splitting.
fn line_start(bytes: &[u8], end: usize) -> usize {
    if end == 0 {
        return 0;
    }
    let mut j = end - 1;
    // Skip this line's own ending, if it has one: a final line may
    // lack an ending, in which case j stays on its last content byte.
    if bytes[j] == b'\n' {
        if j == 0 {
            return 0;
        }
        j -= 1;
        if bytes[j] == b'\r' {
            if j == 0 {
                return 0;
            }
            j -= 1;
        }
    } else if bytes[j] == b'\r' {
        if j == 0 {
            return 0;
        }
        j -= 1;
    }
    // Scan for the previous line's ending: the largest position at or
    // before j whose byte ends a line. A line's content never holds a
    // line ending (the splitter would have split there), so the first
    // ending byte found belongs to the previous line.
    loop {
        if bytes[j] == b'\n' {
            return j + 1;
        }
        if bytes[j] == b'\r' && bytes.get(j + 1) != Some(&b'\n') {
            return j + 1;
        }
        if j == 0 {
            return 0;
        }
        j -= 1;
    }
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
/// without a line ending). A side with no changed lines is empty, its
/// two offsets equal.
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
/// regions in order. Common lines are confirmed by direct byte
/// comparison — no hashing, so equality is exact by construction; the
/// common prefix and suffix of lines are trimmed, so a keystroke
/// yields exactly one region.
#[must_use]
pub fn diff_regions(old: &str, new: &str) -> Vec<EditRegion> {
    let ob = old.as_bytes();
    let nb = new.as_bytes();
    // Forward: walk the common prefix line by line; the first unequal
    // line bounds it. Equal lines have equal endings, so the walk
    // stays aligned.
    let mut o = 0usize;
    let mut n = 0usize;
    let p_old;
    let p_new;
    loop {
        let oe = line_end(ob, o);
        let ne = line_end(nb, n);
        if ob[o..oe] != nb[n..ne] {
            p_old = o;
            p_new = n;
            break;
        }
        o = oe;
        n = ne;
        if o >= ob.len() || n >= nb.len() {
            p_old = o;
            p_new = n;
            break;
        }
    }
    if p_old >= ob.len() && p_new >= nb.len() {
        // Every line matched: the sources are identical.
        return Vec::new();
    }
    // Backward: walk the common suffix line by line, from the ends up,
    // never crossing the forward boundary.
    let mut o_stop = ob.len();
    let mut n_stop = nb.len();
    let mut o_edge = ob.len();
    let mut n_edge = nb.len();
    while o_stop > p_old && n_stop > p_new {
        let os = line_start(ob, o_edge);
        let ns = line_start(nb, n_edge);
        if ob[os..o_edge] != nb[ns..n_edge] {
            break;
        }
        o_stop = os;
        n_stop = ns;
        if os == 0 || ns == 0 {
            break;
        }
        o_edge = os;
        n_edge = ns;
    }
    vec![EditRegion {
        old_start: p_old,
        old_end: o_stop,
        new_start: p_new,
        new_end: n_stop,
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
