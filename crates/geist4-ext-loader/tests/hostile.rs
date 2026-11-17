//! Tests for the hostile fixture: a real tier-2 component written to
//! misbehave — an undeclared kind, an oversized claim, a memory hog,
//! and an endless loop. Each probe must end as a defined error or a
//! clean decline, never a host panic or a host hang.
#![cfg(feature = "sandbox")]

use std::collections::BTreeMap;
use std::sync::Arc;

use geist4_core::basic_html;
use geist4_core::extension::{Extension, Options};
use geist4_core::parser::parse_with;

use geist4_ext_loader::adapter::WasmExtension;
use geist4_ext_loader::sandbox::SandboxExtension;
use geist4_ext_loader::types::{Extension as ManifestExtension, KindEntry, Manifest, Wasm};

/// The hostile component.
const HOSTILE: &[u8] = include_bytes!("fixtures/hostile.wasm");

/// A tier-2 manifest for the hostile component. The one declared kind
/// is `hug`; the probes answer with `bogus` (undeclared), oversized
/// claim ends, a huge render, and an endless look.
fn hostile_manifest() -> Manifest {
    Manifest {
        extension: ManifestExtension {
            name: "hostile".to_owned(),
            version: "1.0.0".to_owned(),
            format: "1.0".to_owned(),
            description: "the hostile fuzz fixture".to_owned(),
            license: "MIT".to_owned(),
            homepage: None,
            hooks: vec![
                "block-start-look".to_owned(),
                "block-start".to_owned(),
                "inline-bracket".to_owned(),
                "render".to_owned(),
            ],
            after: vec![],
            conflicts: vec![],
        },
        kinds: vec![KindEntry {
            kind: "hug".to_owned(),
            container: false,
            inline_text: true,
            attrs: vec!["probe".to_owned()],
        }],
        options: BTreeMap::new(),
        behavior: vec![],
        wasm: Some(Wasm {
            path: "component.wasm".to_owned(),
            fuel: None,
            memory_pages: None,
            inline_triggers: vec![],
        }),
        host: false,
    }
}

/// The hostile adapter at default limits, plus its handle for taking
/// the latched error.
fn hostile_extension() -> (Arc<WasmExtension>, Options) {
    let sandbox =
        SandboxExtension::from_bytes(HOSTILE, None, None).expect("the hostile component loads");
    let extension = Arc::new(WasmExtension::new(hostile_manifest(), sandbox));
    let mut options = Options::new();
    options.extension(extension.clone());
    (extension, options)
}

#[test]
fn undeclared_block_kind_is_latched() {
    // The component answers block-start with `bogus`, which the
    // manifest never declared. The adapter latches a protocol
    // violation and opens nothing; the line stays plain content.
    let (extension, options) = hostile_extension();
    let document = parse_with("%%bad:\nhello\n", &options);
    let html = basic_html::render_with(&document, &options);
    assert!(
        !html.contains("bogus"),
        "the undeclared kind reaches no markup: {html}"
    );
    assert!(
        html.contains("hello"),
        "the document still renders its text: {html}"
    );
    let error = extension
        .take_error()
        .expect("the undeclared kind latches an error");
    assert!(
        error.contains("bogus") && error.contains("undeclared"),
        "the error names the violation: {error}"
    );
}

#[test]
fn oversized_claim_keeps_whole_content() {
    // The component claims a node with `end` = 999,999 over six bytes
    // of content. The consumer keeps the whole content and renders it
    // through the component; nothing panics and nothing is latched.
    let (extension, options) = hostile_extension();
    let document = parse_with("[[secret]]\n", &options);
    let html = basic_html::render_with(&document, &options);
    assert!(
        html.contains("<hug>secret</hug>"),
        "the oversized claim keeps the whole content: {html}"
    );
    assert!(
        extension.take_error().is_none(),
        "an oversized claim is a defined clamp, not an error"
    );
}

#[test]
fn render_memory_hog_is_stopped() {
    // The component's render builds twenty million bytes, past the
    // sandbox's memory cap. The call ends as a defined error; the
    // host keeps its memory and the node renders nothing.
    let (extension, options) = hostile_extension();
    let document = parse_with("[[big]]\n", &options);
    let html = basic_html::render_with(&document, &options);
    assert!(
        !html.contains("xxxxx"),
        "the memory hog's output reaches no HTML: {html}"
    );
    let error = extension
        .take_error()
        .expect("the memory hog ends as a defined error");
    assert!(
        error.to_lowercase().contains("memory"),
        "the error names the memory cap: {error}"
    );
}

#[test]
fn endless_look_is_stopped_by_fuel() {
    // The component's block-start-look runs forever on `%%spin`
    // lines. The sandbox's fuel ends the call as a defined error, the
    // adapter declines the look, and the parse completes.
    let (extension, options) = hostile_extension();
    let document = parse_with("%%spin\nhello\n", &options);
    let html = basic_html::render_with(&document, &options);
    assert!(
        html.contains("hello"),
        "the document still renders its text: {html}"
    );
    let error = extension
        .take_error()
        .expect("the endless look ends as a defined error");
    assert!(
        error.to_lowercase().contains("fuel"),
        "the error names the fuel cap: {error}"
    );
}
