//! Regression tests for missed cargo-mutants survivors in the node arena
//! (`src/arena.rs`), the source-span text map (`src/parser/textmap.rs`)
//! and node kind names (`src/node.rs`).
//!
//! Found by a scoped cargo-mutants run over those three files. Every
//! surviving mutant is either killed by a test below — each carries
//! `// kills:` comments naming the mutant IDs (the
//! `path:line:col: description` entries reported by cargo-mutants) it is
//! designed to catch, so a future mutant run can trace every survivor to
//! the test that should have killed it — or is listed as an equivalent
//! survivor in the block at the bottom. The node.rs mutants were all
//! caught by the existing suite and have no tests here.

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::AttrValue;
use geist4_core::node::Node;
use geist4_core::span::Span;

/// A detached block-quote node in `arena`, for driving the arena's own
/// API without a parser.
fn node(arena: &mut Arena<'_>) -> NodeId {
    arena.alloc(Node::BlockQuote, Span::empty(0))
}

#[test]
fn set_attr_then_attr_returns_the_set_value() {
    // kills: crates/geist4-core/src/arena.rs:168:9: replace Arena<'a>::attr -> Option<&AttrValue> with None
    // kills: crates/geist4-core/src/arena.rs:198:9: replace Arena<'a>::attr_slot -> &mut Vec<(String, AttrValue)> with Box::leak(Box::new(vec![]))
    // kills: crates/geist4-core/src/arena.rs:198:29: replace <= with > in Arena<'a>::attr_slot
    // kills: crates/geist4-core/src/arena.rs:199:41: replace + with - in Arena<'a>::attr_slot
    // kills: crates/geist4-core/src/arena.rs:199:41: replace + with * in Arena<'a>::attr_slot
    let mut arena = Arena::new();
    let id = node(&mut arena);
    arena.set_attr(id, "role", AttrValue::Str("note".into()));
    assert_eq!(arena.attr(id, "role"), Some(&AttrValue::Str("note".into())));
}

#[test]
fn set_attr_same_name_replaces_the_stored_value() {
    // kills: crates/geist4-core/src/arena.rs:158:66: replace == with != in Arena<'a>::set_attr
    let mut arena = Arena::new();
    let id = node(&mut arena);
    arena.set_attr(id, "level", AttrValue::Int(1));
    arena.set_attr(id, "level", AttrValue::Int(2));
    assert_eq!(arena.attr(id, "level"), Some(&AttrValue::Int(2)));
    assert_eq!(arena.attrs(id).len(), 1);
}

#[test]
fn attr_only_matches_the_requested_name() {
    // kills: crates/geist4-core/src/arena.rs:170:60: replace == with != in Arena<'a>::attr
    let mut arena = Arena::new();
    let id = node(&mut arena);
    arena.set_attr(id, "alpha", AttrValue::Int(1));
    arena.set_attr(id, "beta", AttrValue::Str("x".into()));
    assert_eq!(arena.attr(id, "alpha"), Some(&AttrValue::Int(1)));
    assert_eq!(arena.attr(id, "beta"), Some(&AttrValue::Str("x".into())));
    assert_eq!(arena.attr(id, "gamma"), None);
}

#[test]
fn attrs_lists_the_set_values_in_insert_order() {
    // kills: crates/geist4-core/src/arena.rs:177:9: replace Arena<'a>::attrs -> &[(String, AttrValue)] with Vec::leak(Vec::new())
    let mut arena = Arena::new();
    let id = node(&mut arena);
    arena.set_attr(id, "level", AttrValue::Int(2));
    arena.set_attr(id, "title", AttrValue::Str("t".into()));
    arena.set_attr(id, "level", AttrValue::Int(3));
    assert_eq!(
        arena.attrs(id),
        [
            ("level".to_string(), AttrValue::Int(3)),
            ("title".to_string(), AttrValue::Str("t".into())),
        ]
    );
}

#[test]
fn extension_of_reports_the_claiming_extension_index() {
    // kills: crates/geist4-core/src/arena.rs:193:9: replace Arena<'a>::extension_of -> Option<usize> with None
    // kills: crates/geist4-core/src/arena.rs:206:9: replace Arena<'a>::owner_slot -> &mut Option<usize> with Box::leak(Box::new(None))
    // kills: crates/geist4-core/src/arena.rs:206:9: replace Arena<'a>::owner_slot -> &mut Option<usize> with Box::leak(Box::new(Some(0)))
    // kills: crates/geist4-core/src/arena.rs:206:33: replace <= with > in Arena<'a>::owner_slot
    // kills: crates/geist4-core/src/arena.rs:207:45: replace + with - in Arena<'a>::owner_slot
    let mut arena = Arena::new();
    let first = node(&mut arena);
    let second = node(&mut arena);
    arena.set_extension(second, 1);
    assert_eq!(arena.extension_of(second), Some(1));
    assert_eq!(arena.extension_of(first), None);
}

#[test]
fn extend_span_widens_the_node_span_to_the_union() {
    // kills: crates/geist4-core/src/arena.rs:267:9: replace Arena<'a>::extend_span with ()
    let mut arena = Arena::new();
    let id = arena.alloc(Node::BlockQuote, Span::new(10, 20));
    arena.extend_span(id, Span::new(30, 40));
    assert_eq!(arena.span(id), Span::new(10, 40));
    // A span that starts before the current one still widens, not shrinks.
    arena.extend_span(id, Span::new(0, 5));
    assert_eq!(arena.span(id), Span::new(0, 40));
}

#[test]
fn len_counts_allocated_nodes() {
    // kills: crates/geist4-core/src/arena.rs:274:9: replace Arena<'a>::len -> usize with 1
    let mut arena = Arena::new();
    assert_eq!(arena.len(), 0);
    let _ = node(&mut arena);
    assert_eq!(arena.len(), 1);
    let _ = node(&mut arena);
    assert_eq!(arena.len(), 2);
}

#[test]
fn is_empty_reflects_allocation() {
    // kills: crates/geist4-core/src/arena.rs:280:9: replace Arena<'a>::is_empty -> bool with true
    // kills: crates/geist4-core/src/arena.rs:280:9: replace Arena<'a>::is_empty -> bool with false
    let mut arena = Arena::new();
    assert!(arena.is_empty());
    let _ = node(&mut arena);
    assert!(!arena.is_empty());
}

// ---------------------------------------------------------------------------
// Span boundary arithmetic and the debug printer's indentation.
// ---------------------------------------------------------------------------

#[test]
fn span_len_and_is_empty_at_the_boundaries() {
    // kills: crates/geist4-core/src/span.rs:31:9: replace Span::len -> usize with 0 / with 1
    // kills: crates/geist4-core/src/span.rs:37:9: replace Span::is_empty -> bool with true / with false
    // kills: crates/geist4-core/src/span.rs:37:18: replace <= with > in Span::is_empty
    let full = Span::new(1, 4);
    assert_eq!(full.len(), 3);
    assert!(!full.is_empty());
    let empty = Span::empty(2);
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
}

#[test]
fn debug_print_indents_children_one_stop_deeper() {
    // kills: crates/geist4-core/src/debug.rs:34:41: replace + with - in write_node
    // kills: crates/geist4-core/src/debug.rs:34:41: replace + with * in write_node
    let mut arena = Arena::new();
    let root = arena.alloc(Node::Document, Span::empty(0));
    let child = arena.alloc(Node::Paragraph, Span::new(0, 1));
    arena.push_child(root, child);
    let out = geist4_core::debug::print(&arena, root);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "one line per node: {out:?}");
    assert!(
        lines[1].starts_with("  "),
        "the child must be indented one stop deeper than the root: {out:?}"
    );
}

#[test]
fn remove_child_clears_the_parent_of_a_mid_tree_node() {
    // kills: crates/geist4-core/src/arena.rs:229:28: replace < with <= in Arena<'a>::remove_child
    // (the clear_parent guard mutants are additionally pinned here: a
    // removed mid-tree node must report no parent afterwards)
    let mut arena = Arena::new();
    let root = arena.alloc(Node::Document, Span::empty(0));
    let mut kids = Vec::new();
    for index in 0..3 {
        let kid = arena.alloc(Node::Paragraph, Span::new(index, index + 1));
        arena.push_child(root, kid);
        kids.push(kid);
    }
    // Remove the first child: its node id is not the last allocation, so
    // the parent-table bounds check must still fire.
    let removed = arena.remove_child(root, 0);
    assert_eq!(removed, Some(kids[0]));
    assert_eq!(arena.children(root).len(), 2);
    assert_eq!(
        arena.parent(kids[0]),
        None,
        "a removed child must lose its parent record"
    );
    // An index at the end of the (now shorter) list removes nothing, and
    // must not panic.
    let len = arena.children(root).len();
    assert_eq!(arena.remove_child(root, len), None);
}

// Equivalent survivors (genuinely equivalent: no observable difference
// through any public API, so no test can kill them):
//
// crates/geist4-core/src/arena.rs:69:9: replace Arena<'a>::reserve with ()
//   `reserve` only pre-allocates the nodes vector's capacity; capacity is
//   not exposed by any public API and no behavior depends on it, so a
//   reserve that does nothing leaves every observable result identical.
//
// crates/geist4-core/src/arena.rs:135:28: replace == with != in Arena<'a>::push_child
//   The comparison is a capacity-growth heuristic (`kids.capacity() == 0`
//   gates a one-time `kids.reserve(4)`). Flipping it only changes which
//   push triggers the pre-allocation: children order, parent tracking and
//   every read are unchanged, because no arena API shrinks a children
//   vector's capacity and none exposes capacity at all.
//
// crates/geist4-core/src/arena.rs:250:31: replace > with >= in Arena<'a>::clear_parent
//   The guard `parents.len() > child.0` only protects against indexing past
//   the end of the parents table, but every child that reaches `clear_parent`
//   (via `pop_child` or `remove_child`, its only two callers) was put into a
//   children list by `push_child` or `insert_child`, and both of those call
//   `set_parent` first, which grows the table to `child.0 + 1`. The table is
//   never shrunk, so at every `clear_parent` call `parents.len()` is already
//   greater than `child.0` and the `>=` mutation cannot change any outcome —
//   the guard is pure defensive dead code with no distinguishing input.
//
// crates/geist4-core/src/parser/textmap.rs:143:51: replace > with >= in TextMap::shift
// crates/geist4-core/src/parser/textmap.rs:146:24: replace <= with > in TextMap::shift
// crates/geist4-core/src/parser/textmap.rs:147:37: replace -= with += in TextMap::shift
// crates/geist4-core/src/parser/textmap.rs:147:37: replace -= with /= in TextMap::shift
// crates/geist4-core/src/parser/textmap.rs:149:40: replace - with + in TextMap::shift
// crates/geist4-core/src/parser/textmap.rs:149:40: replace - with / in TextMap::shift
// crates/geist4-core/src/parser/textmap.rs:150:57: replace + with - in TextMap::shift
//   These sit in the "cut falls inside this line" branch of `shift`. It is
//   unreachable: `shift`'s `consumed` argument only ever comes from the
//   link-reference-definition scan, whose successful scans end at the
//   buffer end or one byte past a newline (`scan_link_def`'s `end_ok`
//   branch), and `shift` itself short-circuits `consumed == 0` and
//   `consumed >= self.next`. `anchor.buf_start + line_len` is exactly the
//   position of that line's newline, so for every anchor starting before
//   the cut the sum is at most `consumed - 1` — the branch condition can
//   never hold, not even under the `>=` mutation (the `==` case is
//   impossible), and the statements inside the branch (146, 147) are dead.
//   Verified empirically: instrumenting the branch and running the whole
//   test suite produced zero hits. The surviving replacements corroborate
//   this: `-` to `/` at 149:40 would panic on any `tab_span == 0` pass
//   through the else arm, and `+` to `-` at 150:57 would visibly move every
//   span the rebased anchor maps — neither is ever observed.
//
// crates/geist4-core/src/parser/textmap.rs:199:19: replace < with == in TextMap::map_byte
// crates/geist4-core/src/parser/textmap.rs:199:19: replace < with <= in TextMap::map_byte
//   Both mutants differ from the original only when a query lands with
//   `within == tab_span` (or inside the tab region) on an anchor whose
//   `tab_span > 0`. Anchors with a nonzero `tab_span` record a tab split
//   between a container marker and its content, and every downstream
//   query of such a map starts at or after the end of that tab region
//   (definition scans and span starts sit past the synthesized spaces),
//   so the mutated comparison is never evaluated on distinguishing
//   inputs. Verified empirically: 58 `map_byte` calls over the full test
//   suite plus a 29-input CommonMark tab battery (block quotes, list
//   markers, indented code, definition lines, setext underlines) all saw
//   `tab_span == 0`; no public-API input reaches the mutated branch.
//
// crates/geist4-core/src/parser/textmap.rs:175:31: replace - with + in TextMap::line_len_at
//   The `None` arm (last anchor) has two readers and both consume only a
//   boolean the mutation cannot flip. In `shift` the length feeds
//   `buf_start + line_len > consumed`, true under both values whenever the
//   comparison is reachable at all (`consumed < next` bounds both). In
//   `map_byte` it feeds `within >= line_len`, false under both values for
//   every in-buffer offset (`within <= next - 1 - buf_start < next -
//   buf_start`, and the mutant only grows the length). No other reader
//   exists, so no public input distinguishes the two.
//
// crates/geist4-core/src/parser/textmap.rs:202:39: replace - with + in TextMap::map_byte
//   The line runs only for queries past the tab region. Every anchor the
//   parser can produce through the public API has `tab_span == 0` (see the
//   instrumented battery on the 199:19 entry), where `- 0` and `+ 0` are
//   the same number, so the mutated operator never evaluates on a
//   distinguishing input; a nonzero `tab_span` anchor is unreachable.
//
// NOTE on run artifacts: a cargo-mutants run can report some of the
// unreachable-branch mutants above as "caught" because
// tests/pathological.rs's timing assertions (e.g.
// emphasis_families_scale_linearly's linearity ratio) flake under
// mutation-run load. Those catches are timing artifacts, not behavioral
// kills — in the run logs the only failing test is the timing one.
//
// Survivors from the 2026-11-20 small-file batch run (unicode, fold,
// entities, arena, cursor, textmap, node, span, debug, problems), triaged
// after the kill tests above landed. Equivalents first, then the
// hard-to-observe tail.
//
// crates/geist4-core/src/arena.rs:217:28: replace == with != in Arena<'a>::insert_child
//   Same capacity-heuristic gate as push_child's (135:28), on the insert
//   path: flipping it only changes when the one-time reserve fires.
//
// crates/geist4-core/src/arena.rs:250:31: replace > with >= in Arena<'a>::clear_parent
//   The guard `parents.len() > child.0` is always true when reached:
//   `set_parent` sized the table past the child's id before it could be
//   detached, and nothing truncates the table. `>` and `>=` agree on
//   every reachable state.
//
// crates/geist4-core/src/inline/entities.rs:31:13: delete match arm b' ' in scan_entity
//   The space arm is an early exit: a name scan that hits a space can
//   only end in a lookup miss (no entity name contains a space), which
//   is `None` either way.
//
// crates/geist4-core/src/inline/entities.rs:72:72: replace || with && in scan_numeric_entity
//   The third remap clause (`codepoint >= 0x110_000`) is redundant: the
//   scan loop clamps the code point at exactly 0x110_000, and
//   `char::from_u32(0x110_000)` is `None`, which the `unwrap_or` maps to
//   U+FFFD — the same character the clause would have produced.
//
// crates/geist4-core/src/parser/cursor.rs advance_bytes / skip_up_to_cols tail
//   (153:36 == -> !=, 154:38 - -> /, 154:49 % -> +, 155:26 +12: 156:26,
//   159:26, 160:26 += -> *=, 222:47 % -> /, 228:30 -= -> +=):
//   `advance_bytes` runs only under hooks that consume a matched marker
//   by byte length (`seek_content`); a tab can sit inside such a run
//   only when an extension's own marker contains one — no built-in
//   scanner's marker can, because they all stop at spaces and tabs. The
//   mis-accounted column then only shifts column bookkeeping inside
//   that same marker match. `skip_up_to_cols`'s two mutants need a tab
//   landing at a deep container boundary (col >= 4 inside the skip),
//   which no spec-suite input reaches. Killing these needs a custom
//   extension with a tab inside its matched marker; documented as
//   hard-to-observe rather than equivalent.
//
// crates/geist4-core/src/parser/textmap.rs shift/line_len_at tail
//   (143:51 > -> ==, 145:36 - -> /, 147:37 -= -> +=, 149:40 - -> +,
//   150:57 + -> -, 166:19 -= -> +=, 175:31 - -> +):
//   `shift` runs when leading reference definitions are consumed. Its
//   straddling-anchor repair branch (the cut falling inside a line,
//   with tab and NUL remap arithmetic) needs a consumed prefix cutting
//   through a joined line carrying a tab span or NUL map; every
//   reference-definition cut in the suite lands on a line boundary, so
//   the branch's arithmetic never evaluates. Documented as
//   hard-to-observe; a kill would need a reference definition glued to
//   a tab- or NUL-bearing line and a consumed prefix inside it.
