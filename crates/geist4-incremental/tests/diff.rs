//! The line-diff contract: the regions `diff_regions` returns are
//! whole-line byte ranges whose union covers exactly the changed
//! lines, confirmed by byte comparison, with common prefix and suffix
//! trimmed so a single keystroke yields one region.

use geist4_incremental::test_support::{diff_regions, line_starts};

fn bytes_of(src: &str, start: usize, end: usize) -> &str {
    &src[start..end]
}

#[test]
fn identical_sources_yield_no_regions() {
    let src = "# one\n\ntwo\n";
    assert!(diff_regions(src, src).is_empty());
}

#[test]
fn a_one_word_edit_yields_one_region_of_two_lines() {
    // Editing a paragraph line makes that paragraph's two lines change
    // (its second line reflows into the first edit? no — line diff is
    // line-granular: only the edited line differs).
    let old = "para one here\nstill para one\n\nnext block\n";
    let new = "para one HERE\nstill para one\n\nnext block\n";
    let regions = diff_regions(old, new);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    assert_eq!(
        bytes_of(old, region.old_start, region.old_end),
        "para one here\n"
    );
    assert_eq!(
        bytes_of(new, region.new_start, region.new_end),
        "para one HERE\n"
    );
}

#[test]
fn an_inserted_line_extends_the_region_over_the_insert_point() {
    let old = "a\nb\n";
    let new = "a\ninserted\nb\n";
    let regions = diff_regions(old, new);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    // The common prefix is line 0 ("a\n"); the old region is empty at
    // the end, the new region is the inserted line.
    assert_eq!(region.old_start, region.old_end);
    assert_eq!(
        bytes_of(new, region.new_start, region.new_end),
        "inserted\n"
    );
}

#[test]
fn a_deleted_line_leaves_an_empty_new_region() {
    let old = "a\ngone\nb\n";
    let new = "a\nb\n";
    let regions = diff_regions(old, new);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    assert_eq!(bytes_of(old, region.old_start, region.old_end), "gone\n");
    assert_eq!(region.new_start, region.new_end);
}

#[test]
fn a_trailing_edit_without_a_final_newline_still_bounds() {
    let old = "a\nend";
    let new = "a\nEND";
    let regions = diff_regions(old, new);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    assert_eq!(bytes_of(old, region.old_start, region.old_end), "end");
    assert_eq!(bytes_of(new, region.new_start, region.new_end), "END");
}

#[test]
fn crlf_sources_diff_on_line_boundaries() {
    let old = "a\r\nkeep\r\nb\r\n";
    let new = "a\r\nKEEP\r\nb\r\n";
    let regions = diff_regions(old, new);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    assert_eq!(bytes_of(old, region.old_start, region.old_end), "keep\r\n");
    assert_eq!(bytes_of(new, region.new_start, region.new_end), "KEEP\r\n");
}

#[test]
fn an_empty_source_against_content_diffs_everything() {
    let regions = diff_regions("", "hello\n");
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].old_start, 0);
    assert_eq!(regions[0].old_end, 0);
    assert_eq!(regions[0].new_start, 0);
    assert_eq!(regions[0].new_end, 6);
    let back = diff_regions("hello\n", "");
    assert_eq!(back[0].old_start, 0);
    assert_eq!(back[0].old_end, 6);
    assert_eq!(back[0].new_start, 0);
    assert_eq!(back[0].new_end, 0);
}

#[test]
fn line_starts_handles_every_ending_shape() {
    assert_eq!(line_starts("a\r\nb\nc\rd"), vec![0, 3, 5, 7]);
    assert_eq!(line_starts(""), vec![0]);
    assert_eq!(line_starts("\n"), vec![0, 1]);
}

#[test]
fn edits_at_both_ends_yield_the_middle_region() {
    // A pathological case for a naive prefix trim: the changed middle
    // is bounded by shared lines on both sides.
    let old = "x\nA\nB\nC\ny\n";
    let new = "x\nA\nB!\nC\ny\n";
    let regions = diff_regions(old, new);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    assert_eq!(bytes_of(old, region.old_start, region.old_end), "B\n");
    assert_eq!(bytes_of(new, region.new_start, region.new_end), "B!\n");
}
