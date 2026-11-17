//! The C ABI's contract, exercised from Rust: argument validation,
//! status codes, ownership discipline, and byte-exact rendering through
//! the exported entry points. The C program under `c-tests/` runs the
//! same contract from the other side of the boundary, against the
//! generated header.

use geist4_ffi::{DocumentHandle, Geist4ProblemList, Geist4String, Status};
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
    fn message(&self) -> Option<String> {
        let end = self.0.iter().position(|&b| b == 0).unwrap_or(ERR_CAP);
        if end == 0 {
            None
        } else {
            Some(String::from_utf8_lossy(&self.0[..end]).into_owned())
        }
    }
}

/// Parses `source` with one optional built-in enabled, rendering HTML.
fn render(source: &str, builtin: Option<&str>) -> String {
    let options = geist4_ffi::geist4_options_create();
    if let Some(name) = builtin {
        let mut err = ErrBuf::new();
        let status = unsafe {
            geist4_ffi::geist4_options_enable_builtin(
                options,
                name.as_ptr().cast(),
                name.len(),
                err.ptr(),
                ERR_CAP,
            )
        };
        assert_eq!(status, Status::GEIST4_OK, "builtin {name}");
    }
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
    let out = render_document(document);
    unsafe { geist4_ffi::geist4_document_free(document) };
    unsafe { geist4_ffi::geist4_options_free(options) };
    out
}

/// Renders an existing document handle, asserting the status is OK.
fn render_document(document: *mut DocumentHandle) -> String {
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

#[test]
fn version_is_a_nonempty_string() {
    let version = unsafe { geist4_ffi::geist4_version() };
    assert!(!version.is_null());
    let text = unsafe { std::ffi::CStr::from_ptr(version) };
    assert_eq!(text.to_bytes(), env!("CARGO_PKG_VERSION").as_bytes());
}

#[test]
fn plain_parse_renders_the_spec_shape() {
    assert_eq!(render("# hi\n", None), "<h1>hi</h1>\n");
    assert_eq!(render("", None), "");
    assert_eq!(render("a *b* c\n", None), "<p>a <em>b</em> c</p>\n");
}

#[test]
fn a_nul_byte_is_ordinary_content() {
    // The engine replaces NUL with U+FFFD, and the (ptr, len) input
    // convention exists precisely so the byte survives the boundary.
    assert_eq!(render("a\0b\n", None), "<p>a\u{FFFD}b</p>\n");
}

#[test]
fn null_arguments_are_rejected() {
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_parse(std::ptr::null(), std::ptr::null(), 5, err.ptr(), ERR_CAP)
    };
    assert!(document.is_null(), "null input parses nothing");
    assert!(err.message().unwrap().contains("null"));

    let mut err = ErrBuf::new();
    let mut out = Geist4String {
        data: std::ptr::null_mut(),
        len: 0,
    };
    // The render call writes through a raw pointer, as C would.
    let status = unsafe {
        geist4_ffi::geist4_render_html(
            std::ptr::null(),
            std::ptr::null(),
            5,
            &raw mut out,
            err.ptr(),
            ERR_CAP,
        )
    };
    assert_eq!(status, Status::GEIST4_ERR_NULL);
    assert!(out.data.is_null(), "out untouched on failure");
}

#[test]
fn empty_input_parses_with_a_non_null_pointer() {
    let empty: &[u8] = b"";
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_parse(
            std::ptr::null(),
            empty.as_ptr().cast(),
            0,
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(!document.is_null());
    assert_eq!(render_document(document), "");
    unsafe { geist4_ffi::geist4_document_free(document) };

    // A null pointer with a zero length is the same empty document; the
    // contract allows it so C callers without a buffer can parse "".
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_parse(std::ptr::null(), std::ptr::null(), 0, err.ptr(), ERR_CAP)
    };
    assert!(!document.is_null());
    assert_eq!(render_document(document), "");
    unsafe { geist4_ffi::geist4_document_free(document) };
}

#[test]
fn invalid_utf8_is_rejected_with_the_offset() {
    let bad: &[u8] = b"a\xffb";
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_parse(
            std::ptr::null(),
            bad.as_ptr().cast(),
            bad.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(document.is_null());
    assert_eq!(document, std::ptr::null_mut());
    let message = err.message().unwrap();
    assert!(message.contains("byte 1"), "{message}");
}

#[test]
fn unknown_extension_names_are_rejected() {
    let options = geist4_ffi::geist4_options_create();
    let mut err = ErrBuf::new();
    let name = "no_such_extension";
    let status = unsafe {
        geist4_ffi::geist4_options_enable_builtin(
            options,
            name.as_ptr().cast(),
            name.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert_eq!(status, Status::GEIST4_ERR_UNKNOWN_EXT);
    assert!(err.message().unwrap().contains("valid names"));
    unsafe { geist4_ffi::geist4_options_free(options) };
}

#[test]
fn gfm_bundle_renders_strikethrough_and_tables() {
    assert_eq!(
        render("~~gone~~\n", Some("gfm")),
        "<p><del>gone</del></p>\n"
    );
    assert_eq!(
        render("| a | b |\n| --- | --- |\n| 1 | 2 |\n", Some("gfm")),
        "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\
         \n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n</tbody>\n</table>\n"
    );
}

#[test]
fn every_bundle_name_enables_something() {
    for name in [
        "tables",
        "strikethrough",
        "autolink",
        "tasklist",
        "tagfilter",
        "footnotes",
        "front_matter",
        "heading_identifiers",
        "description_lists",
        "gfm",
        "all",
    ] {
        let options = geist4_ffi::geist4_options_create();
        let mut err = ErrBuf::new();
        let status = unsafe {
            geist4_ffi::geist4_options_enable_builtin(
                options,
                name.as_ptr().cast(),
                name.len(),
                err.ptr(),
                ERR_CAP,
            )
        };
        assert_eq!(status, Status::GEIST4_OK, "{name}");
        unsafe { geist4_ffi::geist4_options_free(options) };
    }
}

#[test]
fn enabling_twice_is_idempotent() {
    // Twice-registered hooks would run twice; the option is idempotent,
    // so "gfm" then "tables" registers tables once.
    let options = geist4_ffi::geist4_options_create();
    for name in ["gfm", "tables", "strikethrough"] {
        let mut err = ErrBuf::new();
        let status = unsafe {
            geist4_ffi::geist4_options_enable_builtin(
                options,
                name.as_ptr().cast(),
                name.len(),
                err.ptr(),
                ERR_CAP,
            )
        };
        assert_eq!(status, Status::GEIST4_OK, "{name}");
    }
    let mut err = ErrBuf::new();
    let document = unsafe {
        geist4_ffi::geist4_parse(options, b"~~x~~\n".as_ptr().cast(), 6, err.ptr(), ERR_CAP)
    };
    assert!(!document.is_null());
    assert_eq!(render_document(document), "<p><del>x</del></p>\n");
    unsafe { geist4_ffi::geist4_document_free(document) };
    unsafe { geist4_ffi::geist4_options_free(options) };
}

#[test]
fn broken_references_become_problems_with_spans() {
    let source = "[text][missing]\n\n[text]: /here\n";
    let options = geist4_ffi::geist4_options_create();
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
    assert!(!document.is_null());
    let mut list = Geist4ProblemList {
        items: std::ptr::null(),
        len: 0,
    };
    let status = unsafe {
        geist4_ffi::geist4_document_problems(document, &raw mut list, err.ptr(), ERR_CAP)
    };
    assert_eq!(status, Status::GEIST4_OK);
    assert!(list.len >= 1, "expected a problem about `missing`");
    assert!(!list.items.is_null());
    let first = unsafe { &*list.items };
    let message =
        unsafe { std::slice::from_raw_parts(first.message.cast::<u8>(), first.message_len) };
    let message = String::from_utf8_lossy(message);
    assert!(message.contains("missing"), "{message}");
    assert!(first.start <= source.len());
    assert!(first.end <= source.len());
    assert!(first.start < first.end);
    unsafe { geist4_ffi::geist4_document_free(document) };
    unsafe { geist4_ffi::geist4_options_free(options) };
}

#[test]
fn one_shot_render_matches_the_handle_path() {
    let source = "# one\n\n* two\n";
    let options = geist4_ffi::geist4_options_create();
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
    let through_handle = render_document(document);
    unsafe { geist4_ffi::geist4_document_free(document) };

    let mut out = Geist4String {
        data: std::ptr::null_mut(),
        len: 0,
    };
    let status = unsafe {
        geist4_ffi::geist4_render_html(
            options,
            source.as_ptr().cast(),
            source.len(),
            &raw mut out,
            err.ptr(),
            ERR_CAP,
        )
    };
    assert_eq!(status, Status::GEIST4_OK, "one-shot: {:?}", err.message());
    let text = unsafe { std::slice::from_raw_parts(out.data.cast::<u8>(), out.len) };
    let one_shot = String::from_utf8_lossy(text).into_owned();
    unsafe { geist4_ffi::geist4_string_free(out) };
    unsafe { geist4_ffi::geist4_options_free(options) };

    assert_eq!(one_shot, through_handle);
    assert_eq!(one_shot, "<h1>one</h1>\n<ul>\n<li>two</li>\n</ul>\n");
}

#[test]
fn the_options_handle_may_be_freed_before_rendering() {
    // The document captures its own copy of the registration, so
    // freeing the options early is safe.
    let source = "~~x~~\n";
    let options = geist4_ffi::geist4_options_create();
    let name = "strikethrough";
    let mut err = ErrBuf::new();
    let status = unsafe {
        geist4_ffi::geist4_options_enable_builtin(
            options,
            name.as_ptr().cast(),
            name.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert_eq!(status, Status::GEIST4_OK);
    let document = unsafe {
        geist4_ffi::geist4_parse(
            options,
            source.as_ptr().cast(),
            source.len(),
            err.ptr(),
            ERR_CAP,
        )
    };
    assert!(!document.is_null());
    unsafe { geist4_ffi::geist4_options_free(options) };
    assert_eq!(render_document(document), "<p><del>x</del></p>\n");
    unsafe { geist4_ffi::geist4_document_free(document) };
}

#[test]
fn error_buffers_truncate_and_stay_terminated() {
    let options = geist4_ffi::geist4_options_create();
    let mut tiny = [7u8; 8];
    let name = "nope";
    let status = unsafe {
        geist4_ffi::geist4_options_enable_builtin(
            options,
            name.as_ptr().cast(),
            name.len(),
            tiny.as_mut_ptr().cast(),
            tiny.len(),
        )
    };
    assert_eq!(status, Status::GEIST4_ERR_UNKNOWN_EXT);
    assert_eq!(tiny[7], 0, "always terminated");
    let message = std::ffi::CStr::from_bytes_until_nul(&tiny).unwrap();
    assert_eq!(message.to_bytes().len(), 7, "truncated to cap - 1");

    // A null buffer with zero capacity is legal: the status still says
    // what happened.
    let status = unsafe {
        geist4_ffi::geist4_options_enable_builtin(
            options,
            name.as_ptr().cast(),
            name.len(),
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(status, Status::GEIST4_ERR_UNKNOWN_EXT);
    unsafe { geist4_ffi::geist4_options_free(options) };
}

#[test]
fn freeing_null_handles_is_a_no_op() {
    unsafe {
        geist4_ffi::geist4_options_free(std::ptr::null_mut());
        geist4_ffi::geist4_document_free(std::ptr::null_mut());
        geist4_ffi::geist4_string_free(Geist4String {
            data: std::ptr::null_mut(),
            len: 0,
        });
    }
}

#[test]
fn documents_keep_working_across_many_renders() {
    let source = "once\n";
    let options = geist4_ffi::geist4_options_create();
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
    assert!(!document.is_null());
    for _ in 0..3 {
        assert_eq!(render_document(document), "<p>once</p>\n");
    }
    unsafe { geist4_ffi::geist4_document_free(document) };
    unsafe { geist4_ffi::geist4_options_free(options) };
}
