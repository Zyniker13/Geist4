//! Fuzz target: parse and render with an extension's hooks active. The
//! strikethrough built-in drives the inline trigger dispatch, the
//! extension delimiter machinery and the extension renderer; arbitrary
//! bytes must not panic and must drop cleanly.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let mut options = geist4_core::extension::Options::new();
    options.extension(geist4_ext::strikethrough());
    let doc = geist4_core::parser::parse_with(&src, &options);
    let _ = geist4_core::basic_html::render_with(&doc, &options);
});
