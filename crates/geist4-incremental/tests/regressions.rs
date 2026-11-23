//! Regression tests for the two arithmetic bugs the property battery
//! found on its first run (`tests/properties.rs`, seed 2026_1123):
//! both crashed inside `update_region` before the oracle could judge
//! the update, which is exactly the class of failure the corpus's
//! hand-written cases missed.

use geist4_core::parser::parse_document;
use geist4_incremental::Editor;

/// `hard_boundary` indexed `blocks[first - 1]` after only checking
/// `prefix_end == 0`: with leading blank lines the first block starts
/// after byte 0 while `first` is still 0, and the index underflowed
/// (subtraction overflow) instead of falling back. The update now
/// never indexes before the checkpoint and treats a first block
/// preceded by blank lines as an acceptable start.
#[test]
fn leading_blank_lines_before_the_first_block_do_not_panic() {
    let old = "\n\npara\n";
    let new = "\n\nparagraph\n";
    let editor = Editor::new(Default::default());
    let previous = parse_document(old);
    let (document, spans) = editor.update(&previous, old, new);
    assert_eq!(
        geist4_core::debug::print(&document.arena, document.root),
        geist4_core::debug::print(&parse_document(new).arena, document.root),
        "tree must match a full parse"
    );
    let _ = spans;
}

/// The prefix check sliced `old_source[..prefix_end]` on the claim
/// that the first changed block's start lies inside both sources.
/// Deleting the first paragraph of a two-paragraph document violates
/// the claim for the new source (its first block starts at 0), and
/// the slice panicked with an out-of-bounds index. The claim is now
/// checked, not assumed, and a violated claim takes the full parse.
#[test]
fn deleting_the_first_block_of_a_longer_document_does_not_panic() {
    let old = "first\n\nsecond\n";
    let new = "second\n";
    let editor = Editor::new(Default::default());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_eq!(
        geist4_core::debug::print(&document.arena, document.root),
        geist4_core::debug::print(&parse_document(new).arena, document.root),
        "tree must match a full parse"
    );
}

/// The minimal failing input of the slice panic (`blocks = ["", "*"]`,
/// delete kind): an empty first paragraph. Kept verbatim so the case
/// the shrinker found is pinned regardless of the generator's future
/// shape.
#[test]
fn deleting_an_empty_first_paragraph_does_not_panic() {
    let old = "\n\n*\n";
    let new = "*\n";
    let editor = Editor::new(Default::default());
    let previous = parse_document(old);
    let (document, _) = editor.update(&previous, old, new);
    assert_eq!(
        geist4_core::debug::print(&document.arena, document.root),
        geist4_core::debug::print(&parse_document(new).arena, document.root),
        "tree must match a full parse"
    );
}
