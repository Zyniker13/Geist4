//! Fuzz target: parse and render with the extension hooks active. The
//! strikethrough built-in drives the inline trigger dispatch, the
//! extension delimiter machinery and the inline renderer; the tables
//! built-in drives the block-level hooks, the paragraph conversion and
//! the block renderer; the autolink built-in drives the trigger match,
//! the unwrite-and-splice link emission and the post-process email
//! pass over consolidated text nodes; the tasklist, tagfilter and
//! footnotes built-ins drive the bracket claim, the render-time
//! rewrite and the post-process tree surgery. Arbitrary bytes must not
//! panic and must drop cleanly.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let mut options = geist4_core::extension::Options::new();
    options.extension(geist4_ext::tables());
    options.extension(geist4_ext::strikethrough());
    options.extension(geist4_ext::autolink());
    options.extension(geist4_ext::tasklist());
    options.extension(geist4_ext::tagfilter());
    options.extension(geist4_ext::footnotes());
    let doc = geist4_core::parser::parse_with(&src, &options);
    let _ = geist4_core::basic_html::render_with(&doc, &options);
});
