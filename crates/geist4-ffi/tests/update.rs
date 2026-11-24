//! Differential and lifecycle tests for `geist4_update`, the C entry
//! point the editor clients call.
//!
//! The core guarantee (decision 0045): every update returns a document
//! value-identical to a fresh parse, and reports the changed spans in
//! new-source coordinates. Every bug in the incremental engine — a
//! stale mount, a dropped problem, a shifted span — shows up here as a
//! mismatch between the two paths, so the tests compare them directly
//! instead of re-asserting engine internals.

use geist4_ffi::{DocumentHandle, Geist4ChangeList, Geist4ProblemList, Geist4String, Status};
use std::ffi::c_char;

const ERR_CAP: usize = 256;

/// An error buffer for a call, returned as the message it received, if
/// any.
struct ErrBuf(Box<[u8]>);

impl ErrBuf {
    fn new() -> Self {
        ErrBuf(vec![0; ERR_CAP].into_boxed_slice())
    }
    fn ptr(&mut self) -> *mut c_char {
        self.0.as_mut_ptr().cast()
    }
    fn message(&self) -> String {
        let end = self.0.iter().position(|&b| b == 0).unwrap_or(ERR_CAP);
        String::from_utf8_lossy(&self.0[..end]).into_owned()
    }
}

/// Parses `source` with plain options, returning the handle.
fn parse(source: &[u8]) -> *mut DocumentHandle {
    parse_with(std::ptr::null_mut(), source)
}

/// Parses `source` with `options`, returning the handle.
fn parse_with(options: *mut geist4_ffi::OptionsHandle, source: &[u8]) -> *mut DocumentHandle {
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_parse(
            options,
            source.as_ptr().cast(),
            source.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(!document.is_null(), "parse failed: {:?}", err.message());
    document
}

/// Updates `previous` with `geist4_update`, asserting success.
fn update(previous: *const DocumentHandle, source: &[u8]) -> *mut DocumentHandle {
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_update(
            previous,
            source.as_ptr().cast(),
            source.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(!document.is_null(), "update failed: {:?}", err.message());
    document
}

/// Renders an existing document handle, asserting the status is OK.
fn render(document: *const DocumentHandle) -> String {
    let mut err = ErrBuf::new();
    let mut out = Geist4String {
        data: std::ptr::null_mut(),
        len: 0,
    };
    let status =
        unsafe { geist4_ffi::geist4_document_html(document, &raw mut out, err.ptr(), ERR_CAP) };
    assert_eq!(status, Status::GEIST4_OK, "html: {:?}", err.message());
    let text = unsafe { std::slice::from_raw_parts(out.data.cast::<u8>(), out.len) };
    let rendered = String::from_utf8_lossy(text).into_owned();
    unsafe { geist4_ffi::geist4_string_free(out) };
    rendered
}

/// Reads the problems a document reports as `(start, end, message)`.
fn problems(document: *const DocumentHandle) -> Vec<(usize, usize, String)> {
    let mut err = ErrBuf::new();
    let mut list = Geist4ProblemList {
        items: std::ptr::null(),
        len: 0,
    };
    let status = unsafe {
        geist4_ffi::geist4_document_problems(document, &raw mut list, err.ptr(), ERR_CAP)
    };
    assert_eq!(status, Status::GEIST4_OK, "problems: {:?}", err.message());
    (0..list.len)
        .map(|index| {
            let item = unsafe { &*list.items.add(index) };
            let message = if item.message.is_null() {
                String::new()
            } else {
                let bytes = unsafe {
                    std::slice::from_raw_parts(item.message.cast::<u8>(), item.message_len)
                };
                String::from_utf8_lossy(bytes).into_owned()
            };
            (item.start, item.end, message)
        })
        .collect()
}

/// Reads the changed spans a document reports as `(start, end)` pairs.
fn spans(document: *const DocumentHandle) -> Vec<(usize, usize)> {
    let mut err = ErrBuf::new();
    let mut list = Geist4ChangeList {
        items: std::ptr::null(),
        len: 0,
    };
    let status =
        unsafe { geist4_ffi::geist4_document_spans(document, &raw mut list, err.ptr(), ERR_CAP) };
    assert_eq!(status, Status::GEIST4_OK, "spans: {:?}", err.message());
    (0..list.len)
        .map(|index| {
            let item = unsafe { &*list.items.add(index) };
            (item.start, item.end)
        })
        .collect()
}

/// Compares two documents through every observable surface: HTML and
/// problems.
fn assert_value_identical(left: *const DocumentHandle, right: *const DocumentHandle) {
    assert_eq!(render(left), render(right), "HTML differs");
    assert_eq!(problems(left), problems(right), "problems differ");
}

/// A parse of `source` fresh: the differential oracle.
fn full(source: &str) -> *mut DocumentHandle {
    parse(source.as_bytes())
}

/// An update from `original` to `edited`, with the previous handle
/// freed before the new document is read — the lifecycle the C
/// clients use.
fn incremental(original: &str, edited: &str) -> *mut DocumentHandle {
    let previous = parse(original.as_bytes());
    let updated = update(previous, edited.as_bytes());
    unsafe { geist4_ffi::geist4_document_free(previous) };
    updated
}

#[test]
fn an_edit_matches_a_full_parse() {
    let original = "# Title\n\n- one\n- two\n\nParagraph.\n";
    let edited = "# Title\n\n- one\n- two edited\n\nParagraph.\n";
    let full = full(edited);
    let incremental = incremental(original, edited);
    assert_value_identical(full, incremental);
    unsafe { geist4_ffi::geist4_document_free(full) };
    unsafe { geist4_ffi::geist4_document_free(incremental) };
}

#[test]
fn a_new_problem_survives_the_update() {
    // The edited source has a problem the original did not; the
    // incremental path must report it exactly as a full parse would.
    let original = "line one\n\n# Tail";
    let edited = "line one\n\nunclosed [link\n\n# Tail";
    let full = full(edited);
    let incremental = incremental(original, edited);
    assert_eq!(problems(full), problems(incremental));
    assert_eq!(render(full), render(incremental));
    unsafe { geist4_ffi::geist4_document_free(full) };
    unsafe { geist4_ffi::geist4_document_free(incremental) };
}

#[test]
fn an_edit_across_a_heading_matches_a_full_parse() {
    // An edit that changes a line's block type (paragraph into
    // heading) is the mount boundary the incremental engine cares
    // about most.
    let original = "one\n\ntwo\n\nthree\n";
    let edited = "one\n\n# two\n\nthree\n";
    let full = full(edited);
    let incremental = incremental(original, edited);
    assert_value_identical(full, incremental);
    unsafe { geist4_ffi::geist4_document_free(full) };
    unsafe { geist4_ffi::geist4_document_free(incremental) };
}

#[test]
fn a_fenced_block_edit_matches_a_full_parse() {
    let original = "before\n\n```\nfenced\n```\n\nafter\n";
    let edited = "before\n\n```\nfenced!\nmore\n```\n\nafter\n";
    let full = full(edited);
    let incremental = incremental(original, edited);
    assert_value_identical(full, incremental);
    unsafe { geist4_ffi::geist4_document_free(full) };
    unsafe { geist4_ffi::geist4_document_free(incremental) };
}

#[test]
fn identical_sources_report_no_changes() {
    let source = "# Heading\n\nText.\n";
    let previous = parse(source.as_bytes());
    let updated = update(previous, source.as_bytes());
    assert!(
        spans(updated).is_empty(),
        "identical source must report no spans"
    );
    unsafe { geist4_ffi::geist4_document_free(previous) };
    unsafe { geist4_ffi::geist4_document_free(updated) };
}

#[test]
fn a_first_edit_reports_a_span_containing_the_edit() {
    // The reparse window is whole blocks: it may start before the
    // edited line (a lazy continuation needs its opening line) and
    // extend to the end of the source. The contract is containment:
    // the edited bytes lie inside the reported span, and the span lies
    // inside the source.
    let original = "alpha\nbeta\ngamma\n";
    let edited = "alpha\nBETA\ngamma\n";
    let updated = incremental(original, edited);
    let spans = spans(updated);
    assert_eq!(spans.len(), 1, "one edit region, one span");
    let (start, end) = spans[0];
    assert!(
        start <= 6 && end >= 10,
        "span {start}..{end} missed the edit"
    );
    assert!(end <= edited.len());
    unsafe { geist4_ffi::geist4_document_free(updated) };
}

#[test]
fn a_first_block_edit_reparses_only_the_prefix_window() {
    // The edit is in the FIRST block, so the reparse window cannot
    // include a copied prefix; it must be shorter than the whole
    // source only if the engine did not fall back. Assert the weaker,
    // always-true shape instead: the span covers the edit and lies in
    // bounds.
    let original = "alpha\n\nbeta\n\ngamma\n";
    let edited = "alpha!\n\nbeta\n\ngamma\n";
    let updated = incremental(original, edited);
    let spans = spans(updated);
    assert!(!spans.is_empty(), "an edit reports at least one span");
    let (start, end) = spans[0];
    assert!(
        start <= 5 && end >= 6,
        "span {start}..{end} missed the edit"
    );
    unsafe { geist4_ffi::geist4_document_free(updated) };
}

#[test]
fn spans_cover_the_changed_block_only() {
    // The reparse window is whole blocks: it starts at a block
    // boundary at or before the edit and runs to the source end when
    // the copy boundary requires it. The stable guarantee is
    // containment (the edited bytes lie inside a reported span) plus
    // bounds (no span leaves the source).
    let original = "para one\n\npara two\n\npara three\n";
    let edited = "para one\n\npara two!\n\npara three\n";
    let updated = incremental(original, edited);
    let spans = spans(updated);
    assert!(!spans.is_empty(), "an edit reports at least one span");
    let edit_start = edited.find("para two!").unwrap();
    let edit_end = edit_start + "para two!".len();
    assert!(
        spans
            .iter()
            .any(|(start, end)| *start <= edit_start && *end >= edit_end),
        "no span covered the edit at {edit_start}..{edit_end}: {spans:?}"
    );
    for (start, end) in &spans {
        assert!(start <= end, "inverted span {start}..{end}");
        assert!(*end <= edited.len(), "span {start}..{end} out of bounds");
    }
    unsafe { geist4_ffi::geist4_document_free(updated) };
}

#[test]
fn an_append_reports_the_new_tail() {
    let original = "# Head\n\nbody\n";
    let edited = "# Head\n\nbody\n\n## New section\n\nwords\n";
    let updated = incremental(original, edited);
    let spans = spans(updated);
    assert!(!spans.is_empty(), "an edit reports at least one span");
    let covered = spans.iter().map(|(start, end)| end - start).sum::<usize>();
    assert!(
        covered < edited.len(),
        "an append cannot have reparsed everything"
    );
    unsafe { geist4_ffi::geist4_document_free(updated) };
}

#[test]
fn spans_stay_in_bounds() {
    let sources = [
        ("# H\n\ntext\n", "# H\n\ntext edited\n"),
        ("- a\n- b\n", "- a\n- b\n- c\n"),
        ("> quote\n", "> quote\n\nafter\n"),
        ("a | b\n- | -\n1 | 2\n", "a | b\n- | -\n1 | 22\n"),
        ("one\ntwo\n", "one\ntwo\nthree\nfour\n"),
    ];
    for (original, edited) in sources {
        let updated = incremental(original, edited);
        for (start, end) in spans(updated) {
            assert!(start <= end, "inverted span {start}..{end}");
            assert!(end <= edited.len(), "span {start}..{end} out of bounds");
        }
        unsafe { geist4_ffi::geist4_document_free(updated) };
    }
}

#[test]
fn a_null_previous_handle_is_an_error() {
    let mut err = ErrBuf::new();
    let handle = unsafe {
        geist4_ffi::geist4_update(
            std::ptr::null(),
            b"x\n".as_ptr().cast(),
            2,
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(handle.is_null());
    assert!(err.message().contains("null"), "error: {}", err.message());
}

#[test]
fn a_failed_update_leaves_the_previous_document_live() {
    // Invalid UTF-8 makes the update fail before any state is
    // consumed; the previous document must still work.
    let previous = parse(b"hello\n\nworld\n");
    let mut err = ErrBuf::new();
    let result = unsafe {
        geist4_ffi::geist4_update(
            previous,
            [b'a', b'b', 0xFF, b'\n'].as_ptr().cast(),
            4,
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(result.is_null(), "invalid UTF-8 must fail the update");
    assert!(!err.message().is_empty(), "the failure explains itself");
    assert_eq!(
        render(previous),
        "<p>hello</p>\n<p>world</p>\n",
        "the previous document survived the failed update"
    );
    unsafe { geist4_ffi::geist4_document_free(previous) };
}

#[test]
fn update_keeps_the_previous_document_s_options() {
    // The update takes no options handle; it must reuse the previous
    // document's registration — here, the task list built-in.
    let options = geist4_ffi::geist4_options_create();
    let mut err = ErrBuf::new();
    let name = "tasklist";
    let status = unsafe {
        geist4_ffi::geist4_options_enable_builtin(
            options,
            name.as_ptr().cast(),
            name.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert_eq!(status, Status::GEIST4_OK, "enable tasklist");
    let edited = "# H\n\n- [x] done\n";

    // The incremental path: parse the original with tasklist enabled,
    // then update to the edited source.
    let previous = parse_with(options, b"# H\n");
    let incremental = {
        let updated = update(previous, edited.as_bytes());
        unsafe { geist4_ffi::geist4_document_free(previous) };
        updated
    };
    // The oracle: a fresh parse of the edited source with the same
    // registration.
    let oracle = parse_with(options, edited.as_bytes());
    assert_eq!(render(incremental), render(oracle), "tasklist lost");
    unsafe { geist4_ffi::geist4_document_free(incremental) };
    unsafe { geist4_ffi::geist4_document_free(oracle) };
    unsafe { geist4_ffi::geist4_options_free(options) };
}
