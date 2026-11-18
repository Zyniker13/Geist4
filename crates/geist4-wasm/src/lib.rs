//! The WebAssembly binding: JavaScript — and anything with a JS engine
//! and a WebAssembly runtime — speaks to the engine through the same
//! boundary the C ABI defines (decision 0028), via `geist4-ffi`'s safe
//! module. This crate contains no `unsafe`: it maps the safe Rust
//! contract onto JavaScript idioms, where an error becomes a thrown
//! `Error` and the input is always a JS string.
//!
//! The generated glue and TypeScript types live in `pkg/`, checked in
//! like the C header so callers never need the Rust toolchain to consume
//! them; `scripts/check.sh` verifies they are current (decision 0029).

use geist4_ffi::safe::{Options as SafeOptions, OwnedDocument};
use wasm_bindgen::prelude::*;

/// The engine version, as in the C header's `geist4_version`.
#[wasm_bindgen]
pub fn version() -> String {
    geist4_ffi::safe::version().to_string()
}

/// The parse options: which built-in extensions are registered.
///
/// `new` is plain CommonMark; `enable_builtin` registers built-ins by
/// name — `tables`, `strikethrough`, `autolink`, `tasklist`,
/// `tagfilter`, `footnotes`, `front_matter`, `heading_identifiers`,
/// `description_lists`, or the bundles `gfm` and `all`. An unknown name
/// throws with the valid names listed; a name registered twice does
/// nothing, so bundles and single names compose freely.
#[wasm_bindgen]
pub struct Options {
    inner: SafeOptions,
}

#[wasm_bindgen]
impl Options {
    /// Plain CommonMark, no extensions registered.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Options {
        Options {
            inner: SafeOptions::new(),
        }
    }

    /// Registers a built-in extension by name.
    ///
    /// # Errors
    /// An unknown name, with the valid names in the message.
    pub fn enable_builtin(&mut self, name: &str) -> Result<(), JsError> {
        self.inner
            .enable_builtin(name)
            .map_err(|error| JsError::new(&error.to_string()))
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

/// One problem the parse noticed, with its span into the source.
///
/// Spans are byte offsets into the UTF-8 encoding of the source, the
/// same convention the core uses; JavaScript callers indexing the
/// source as UTF-16 should convert with `TextEncoder`.
#[wasm_bindgen]
pub struct Problem {
    message: String,
    start: usize,
    end: usize,
}

#[wasm_bindgen]
impl Problem {
    /// What the problem is, as one line.
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// The span's start, as a byte offset into the source.
    #[wasm_bindgen(getter)]
    pub fn start(&self) -> usize {
        self.start
    }

    /// The span's end, exclusive, as a byte offset into the source.
    #[wasm_bindgen(getter)]
    pub fn end(&self) -> usize {
        self.end
    }
}

/// A parsed document: parse once, render or inspect many times.
///
/// The document owns everything it borrows; dropping it (letting the
/// JavaScript reference go) frees the parse. The options used at parse
/// time are remembered, so rendering needs no other argument and the
/// options object may be dropped first.
#[wasm_bindgen]
pub struct Document {
    inner: OwnedDocument,
}

#[wasm_bindgen]
impl Document {
    /// Parses `source` as plain CommonMark. The `parse_with` form takes
    /// an options object with built-ins enabled; the two produce
    /// identical documents when no extension is registered.
    ///
    /// # Errors
    /// Only an engine bug would throw here; valid input parses.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(source: &str) -> Result<Document, JsError> {
        Self::build(OwnedDocument::parse(source, None))
    }

    /// Parses `source` with the given options.
    ///
    /// # Errors
    /// Only an engine bug would throw here; valid input parses.
    #[wasm_bindgen(js_name = parseWith)]
    pub fn parse_with(source: &str, options: &Options) -> Result<Document, JsError> {
        Self::build(OwnedDocument::parse(source, Some(&options.inner)))
    }

    fn build(
        inner: Result<OwnedDocument, geist4_ffi::safe::PanicReport>,
    ) -> Result<Document, JsError> {
        inner
            .map(|inner| Document { inner })
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// The document's HTML, byte for byte as the Rust core renders it.
    ///
    /// # Errors
    /// Only an engine bug would throw here.
    pub fn html(&self) -> Result<String, JsError> {
        self.inner
            .render_html()
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// The problems the parse noticed: broken references and similar
    /// issues the tree absorbed without changing the output, in
    /// document order.
    pub fn problems(&self) -> Vec<Problem> {
        self.inner
            .problems()
            .into_iter()
            .map(|problem| Problem {
                message: problem.message,
                start: problem.start,
                end: problem.end,
            })
            .collect()
    }
}

/// Parses and renders in one call, as plain CommonMark.
///
/// The convenient path for callers who only want HTML. The `renderHtml`
/// form of the same call takes an options object.
///
/// # Errors
/// Only an engine bug would throw here.
#[wasm_bindgen(js_name = renderHtml)]
pub fn render_html(source: &str) -> Result<String, JsError> {
    geist4_ffi::safe::render_html(source, None).map_err(|error| JsError::new(&error.to_string()))
}

/// Parses and renders in one call with the given options.
///
/// # Errors
/// Only an engine bug would throw here.
#[wasm_bindgen(js_name = renderHtmlWith)]
pub fn render_html_with(source: &str, options: &Options) -> Result<String, JsError> {
    geist4_ffi::safe::render_html(source, Some(&options.inner))
        .map_err(|error| JsError::new(&error.to_string()))
}

// There are no host-side unit tests here: wasm-bindgen's generated
// shims for exported types cannot run off-WebAssembly ("cannot call
// wasm-bindgen imported functions on non-wasm targets"). The contract
// is pinned where it actually runs — `node-tests/run.mjs` asserts the
// byte-exact HTML, problems, and errors through the checked-in glue
// under Node, and `geist4-ffi`'s tests pin the safe module beneath it.
