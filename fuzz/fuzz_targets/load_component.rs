//! Fuzzes the WebAssembly boundary's load path: arbitrary bytes are
//! offered to the component loader, which must answer with a working
//! sandbox or a one-line error — never a panic, never an import the
//! sandbox did not declare.
//!
//! The target keeps the loader's caps (the manifest may only lower
//! them), so a hostile component that happens to load still cannot
//! run unbounded here; running hooks is not this target's job (the
//! fuel and memory probes are covered by the sandbox test suite and
//! the hostile fixtures).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match geist4_ext_loader::sandbox::SandboxExtension::from_bytes(data, None, None) {
        Ok(extension) => {
            // A component that binds must have the caps the loader
            // grants, whatever the bytes claimed.
            assert_eq!(extension.fuel(), 1_000_000_000, "fuel cap not applied");
            assert_eq!(extension.memory_pages(), 256, "memory cap not applied");
        }
        Err(error) => {
            assert!(
                !error.to_string().contains('\n'),
                "load error is not one line: {error}"
            );
        }
    }
});
