//! Fuzz target: the parser alone, bytes to a document tree. No
//! rendering, so parser crashes are distinguishable from renderer
//! ones. The document is dropped; a deep tree must also drop without
//! overflowing the stack (the engine's drop is iterative on purpose).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let _ = geist4_core::parser::parse_document(&src);
});
