//! Fuzz target: the full pipeline, bytes to HTML. Every byte string
//! must parse and render without panicking, in time proportional to
//! its length (the pathological suite asserts the scaling property on
//! its families; the fuzzer explores everything else).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let document = geist4_core::parser::parse_document(&src);
    let _ = geist4_core::basic_html::render(&document);
});
