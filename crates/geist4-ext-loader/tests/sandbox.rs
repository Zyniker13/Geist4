//! Tests for the WebAssembly sandbox in `geist4_ext_loader::sandbox`.
//!
//! Two kinds of tests run here: error-path tests that need no
//! component, and round-trip tests against `fixtures/minimal.wasm`, a
//! real tier-2 component whose hooks are deterministic probes (see the
//! guest source note in `docs/extensions/format.md`'s tier-2 section
//! and the module docs of `src/sandbox.rs`). The fixture is a component
//! built from a guest crate outside this repository: `init` accepts
//! exactly the kind `note`, `block-start-look` answers for `$$:` lines
//! and loops forever on `BURN` (the fuel probe), `inline-trigger`
//! returns a delimiter run for `*` and grows memory on `!` (the memory
//! probe), `inline-bracket` claims the group `hello`, `post-process`
//! rewrites `foo` to `bar`, and `render` wraps children in `<b>`.
//!
//! `fixtures/nohooks.wasm` is a valid component that exports nothing,
//! so loading it fails for lack of the hooks interface.
//! `fixtures/pollution.wasm` is a component that exports the hooks but
//! also imports a function, so instantiating it under the sandbox's
//! empty linker fails.
#![cfg(feature = "sandbox")]

use std::path::{Path, PathBuf};

use geist4_ext_loader::sandbox::{
    BlockStart, Consume, ContinueDecision, InlineAction, RenderedBlock, SandboxExtension,
};
use geist4_ext_loader::types::{KindEntry, OptionValue};

/// The loader's fuel cap: the most the sandbox ever grants per call.
const FUEL_CAP: u64 = 1_000_000_000;

/// The loader's memory cap, in WebAssembly pages.
const MEMORY_PAGES_CAP: u32 = 256;

/// The fixture component: a real tier-2 extension with probe hooks.
const MINIMAL: &[u8] = include_bytes!("fixtures/minimal.wasm");

/// A valid component that exports nothing.
const NO_HOOKS: &[u8] = include_bytes!("fixtures/nohooks.wasm");

/// A component that exports the hooks but also imports a function; the
/// sandbox's empty linker must refuse it.
const POLLUTED: &[u8] = include_bytes!("fixtures/pollution.wasm");

/// Writes `bytes` to a fresh file under the system temp directory and
/// returns its path; the caller removes it. The name is unique per
/// call, so tests running in parallel do not overwrite one another.
fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "geist4-sbx-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes).expect("write temp file");
    path
}

/// Removes a temp file made by `temp_file`, ignoring absence.
fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Loads `bytes` with the given limits from a temp file, standing in
/// for a component on disk.
fn load_fixture(bytes: &[u8], fuel: Option<u64>, memory_pages: Option<u32>) -> SandboxExtension {
    let name = if bytes.len() == MINIMAL.len() {
        "minimal.wasm"
    } else if bytes.len() == NO_HOOKS.len() {
        "nohooks.wasm"
    } else {
        "pollution.wasm"
    };
    let path = temp_file(name, bytes);
    let result = SandboxExtension::load(&path, fuel, memory_pages);
    remove(&path);
    result.expect("fixture component loads")
}

/// The `note` kind the fixture component's `init` accepts.
fn note_kind() -> KindEntry {
    KindEntry {
        kind: "note".to_owned(),
        container: true,
        inline_text: false,
        attrs: vec!["class".to_owned()],
    }
}

/// One kind entry with a name the fixture component rejects.
fn other_kind() -> KindEntry {
    KindEntry {
        kind: "other".to_owned(),
        container: false,
        inline_text: false,
        attrs: vec![],
    }
}

/// Options exercising every `option-value` variant.
fn all_options() -> Vec<(String, OptionValue)> {
    vec![
        ("label".to_owned(), OptionValue::String("hi".to_owned())),
        ("strict".to_owned(), OptionValue::Bool(true)),
        ("ratio".to_owned(), OptionValue::Number(0.5)),
    ]
}

#[test]
fn load_rejects_a_missing_file() {
    let missing = std::env::temp_dir().join(format!(
        "geist4-sbx-{}-no-such-component.wasm",
        std::process::id()
    ));
    let err = match SandboxExtension::load(&missing, None, None) {
        Err(err) => err,
        Ok(_) => panic!("a missing file must not load"),
    };
    assert_eq!(err.hook, "load");
    assert!(err.message.contains("cannot read"), "{err}");
}

#[test]
fn load_rejects_a_non_wasm_file() {
    let path = temp_file("not-wasm.wasm", b"this is not WebAssembly at all");
    let err = match SandboxExtension::load(&path, None, None) {
        Err(err) => err,
        Ok(_) => panic!("a non-wasm file must not load"),
    };
    remove(&path);
    assert_eq!(err.hook, "load");
    assert!(err.message.contains("invalid component"), "{err}");
}

#[test]
fn load_rejects_a_core_module() {
    // A valid core WebAssembly module (an empty one): the sandbox
    // accepts only components, so this is an invalid-component error,
    // not a successful load.
    let path = temp_file("core-module.wasm", b"\0asm\x01\0\0\0");
    let err = match SandboxExtension::load(&path, None, None) {
        Err(err) => err,
        Ok(_) => panic!("a core module must not load"),
    };
    remove(&path);
    assert_eq!(err.hook, "load");
    assert!(err.message.contains("invalid component"), "{err}");
}

#[test]
fn fixture_loads_and_checks() {
    let extension = load_fixture(MINIMAL, None, None);
    extension.check().expect("check passes");
    assert_eq!(extension.fuel(), FUEL_CAP);
    assert_eq!(extension.memory_pages(), MEMORY_PAGES_CAP);
}

#[test]
fn limits_are_clamped_to_the_loader_caps() {
    let extension = load_fixture(MINIMAL, Some(u64::MAX), Some(u32::MAX));
    assert_eq!(extension.fuel(), FUEL_CAP);
    assert_eq!(extension.memory_pages(), MEMORY_PAGES_CAP);
}

#[test]
fn init_receives_kinds_and_options() {
    let extension = load_fixture(MINIMAL, None, None);
    let mut instance = extension.instantiate().expect("instantiate");
    instance
        .init(&[note_kind()], &all_options())
        .expect("init accepts the declared kind");
    let err = instance.init(&[other_kind()], &[]).unwrap_err();
    assert_eq!(err.hook, "init");
    assert_eq!(err.message, "unexpected kind: other");
}

#[test]
fn block_hooks_round_trip() {
    let extension = load_fixture(MINIMAL, None, None);
    let mut instance = extension.instantiate().expect("instantiate");
    instance.init(&[note_kind()], &all_options()).expect("init");

    assert!(
        instance
            .block_start_look("$$: note", false, 0)
            .expect("look")
    );
    assert!(
        !instance
            .block_start_look("plain text", false, 0)
            .expect("look")
    );

    let start = instance
        .block_start("$$: note", false, 4, None)
        .expect("block-start call");
    assert_eq!(
        start,
        Some(BlockStart {
            kind: "note".to_owned(),
            attrs: vec![("class".to_owned(), "note".to_owned())],
            consume: Consume::Rest,
        })
    );
    assert_eq!(
        instance
            .block_start("plain text", false, 0, None)
            .expect("call"),
        None
    );

    let decision = instance
        .block_continue("more text", false, 0, false)
        .expect("block-continue call");
    assert_eq!(decision, Some(ContinueDecision::Keep(Consume::Rest)));
    assert_eq!(
        instance.block_continue("", false, 0, true).expect("call"),
        None
    );

    assert_eq!(instance.block_close().expect("call"), None::<RenderedBlock>);

    let html = instance
        .render("badge", &[], "<em>hi</em>")
        .expect("render call");
    assert_eq!(html, "<b><em>hi</em></b>");
    let err = instance.render("nope", &[], "").unwrap_err();
    assert_eq!(err.hook, "render");
    assert_eq!(err.message, "unknown kind");
}

#[test]
fn inline_hooks_round_trip() {
    let extension = load_fixture(MINIMAL, None, None);
    let mut instance = extension.instantiate().expect("instantiate");
    instance.init(&[note_kind()], &all_options()).expect("init");

    let action = instance.inline_trigger("*x", 0).expect("call");
    match action {
        Some(InlineAction::Delimiter { kind, end }) => {
            assert_eq!(kind, "em");
            assert_eq!(end, 1);
        }
        other => panic!("expected a delimiter decision, got {other:?}"),
    }
    assert_eq!(
        instance.inline_trigger("plain", 0).expect("call"),
        None::<InlineAction>
    );

    let action = instance.inline_bracket("hello").expect("call");
    match action {
        Some(InlineAction::Node { kind, attrs, end }) => {
            assert_eq!(kind, "badge");
            assert!(attrs.is_empty());
            assert_eq!(end, 5);
        }
        other => panic!("expected a node decision, got {other:?}"),
    }
    assert_eq!(
        instance.inline_bracket("bye").expect("call"),
        None::<InlineAction>
    );

    assert_eq!(
        instance.post_process("foo and foo").expect("call"),
        "bar and bar"
    );
}

#[test]
fn fuel_exhaustion_is_a_defined_error() {
    // Component-boundary calls cost thousands of fuel units through the
    // canonical ABI even for trivial work, so the budget that `init`
    // completes under is generous; the `BURN` probe loops forever and
    // exhausts any budget.
    let extension = load_fixture(MINIMAL, Some(100_000), None);
    let mut instance = extension.instantiate().expect("instantiate");
    instance
        .init(&[note_kind()], &all_options())
        .expect("init fits in the fuel");
    let err = instance.block_start_look("BURN", false, 0).unwrap_err();
    assert_eq!(err.hook, "block-start-look");
    assert_eq!(err.message, "fuel exhausted");
    // An error spends the instance: wasmtime refuses to re-enter a
    // component instance after a trap, so the engine abandons the parse
    // and drops the instance, and the next document starts from a
    // fresh instance of the same extension.
    drop(instance);
    let mut fresh = extension.instantiate().expect("instantiate");
    fresh.init(&[note_kind()], &all_options()).expect("init");
    assert!(!fresh.block_start_look("plain", false, 0).expect("call"));
}

#[test]
fn memory_limit_stops_growth() {
    // The fixture's initial memory is 17 pages, so 20 grants it; the
    // `!` trigger grows by the current size plus 64 pages, which is
    // past every limit at least as large as the initial memory.
    let extension = load_fixture(MINIMAL, None, Some(20));
    let mut instance = extension.instantiate().expect("instantiate");
    instance.init(&[note_kind()], &all_options()).expect("init");
    let err = instance.inline_trigger("!x", 0).unwrap_err();
    assert_eq!(err.hook, "inline-trigger");
    assert!(err.message.contains("memory limit exceeded"), "{err}");
}

#[test]
fn a_component_without_the_hooks_interface_fails_to_load() {
    let err = match SandboxExtension::from_bytes(NO_HOOKS, None, None) {
        Err(err) => err,
        Ok(_) => panic!("a component without the hooks must not load"),
    };
    assert_eq!(err.hook, "load");
    assert!(
        err.message
            .contains("does not export the `geist4:extension/hooks@1.0.0` interface"),
        "{err}"
    );
}

#[test]
fn a_component_with_imports_fails_to_instantiate() {
    let extension = match SandboxExtension::from_bytes(POLLUTED, None, None) {
        Ok(extension) => extension,
        Err(err) => panic!("the polluted component loads, but must not instantiate: {err}"),
    };
    let err = extension.check().unwrap_err();
    assert_eq!(err.hook, "instantiate");
}
