//! Fuzzes the extension manifest parser and validator.
//!
//! The input is a candidate `extension.toml`. The invariant is the
//! one the loader promises: parsing either yields a manifest or a
//! one-line error with a byte span — never a panic, never a message
//! that names a position outside the input. Nothing here touches the
//! filesystem or the sandbox: `parse` is a pure function.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::ops::Range;

fuzz_target!(|data: &[u8]| {
    // The manifest is text; skip inputs that are not UTF-8, which the
    // TOML reader would reject anyway but which muddy the span
    // invariant with non-character byte offsets.
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    match geist4_ext_loader::manifest::parse(src) {
        Ok(manifest) => {
            // A manifest that parses must respect the limits the
            // validator promises, whatever path reached acceptance.
            assert!(
                manifest.kinds.len() <= 64,
                "more kinds than the limit allows"
            );
            assert!(
                manifest.behavior.len() <= 256,
                "more behavior entries than the limit allows"
            );
            assert!(
                manifest.wasm.is_none() || (manifest.behavior.is_empty() && !manifest.host),
                "a tier-2 manifest carries no other behavior"
            );
        }
        Err(error) => {
            let Range { start, end } = error.span.clone().unwrap_or(0..0);
            assert!(start <= end, "error span runs backwards: {error:?}");
            assert!(end <= src.len(), "error span {start}..{end} exceeds the input");
            assert!(
                !error.message.contains('\n'),
                "error message is not one line"
            );
        }
    }
});
