//! The Python binding: the [`geist4_ffi::safe`] API as a native
//! extension module, packaged as wheels with maturin.
//!
//! The surface mirrors the JavaScript and Swift bindings one for one:
//! `version`, `render_html`, `Options` with `enable_builtin`,
//! `Document` with `html` and `problems`, and one exception type,
//! [`Geist4Error`]. The binding itself is plain safe Rust — the
//! engine's `unsafe` lives only behind `geist4-ffi` (decision 0028),
//! so this crate compiles under the workspace's `forbid(unsafe_code)`.
//!
//! A Python string is always valid Unicode, so the C ABI's null and
//! invalid-UTF-8 rejections are unreachable here, as in the Swift
//! binding; the NUL byte reaches the engine as ordinary content.
//!
//! Errors are one type: an unknown extension name raises
//! [`Geist4Error`] with the valid names in the message, and an engine
//! panic — always a bug — raises it too.

use geist4_ffi::safe::{
    self, Options as SafeOptions, OwnedDocument, PanicReport, ProblemReport, UnknownBuiltin,
};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(
    geist4,
    Geist4Error,
    PyException,
    "An error from the Geist4 engine: an unknown extension name, or an\nengine panic (always a bug)."
);

/// Maps an [`UnknownBuiltin`] onto the binding's one exception type,
/// with the valid names in the message.
fn unknown_error(error: UnknownBuiltin) -> PyErr {
    Geist4Error::new_err(error.to_string())
}

/// Maps a [`PanicReport`] onto the binding's one exception type. The
/// engine must never panic, so this is always a bug.
fn panic_error(error: PanicReport) -> PyErr {
    Geist4Error::new_err(error.to_string())
}

/// The engine version, as in the C header's `geist4_version`.
#[pyfunction]
fn version() -> &'static str {
    safe::version()
}

/// The parse options: which built-in extensions are registered.
///
/// `Options()` is plain CommonMark; `enable_builtin` registers
/// built-ins by name — `tables`, `strikethrough`, `autolink`,
/// `tasklist`, `tagfilter`, `footnotes`, `front_matter`,
/// `heading_identifiers`, `description_lists`, or the bundles `gfm`
/// and `all`.
#[pyclass]
struct Options {
    inner: SafeOptions,
}

#[pymethods]
impl Options {
    /// Plain CommonMark, no extensions registered.
    #[new]
    fn new() -> Self {
        Self {
            inner: SafeOptions::new(),
        }
    }

    /// Registers a built-in extension by name. Idempotent: a name
    /// already registered does nothing, so bundles and single names
    /// compose freely.
    ///
    /// Raises `Geist4Error` on an unknown name, with the valid names
    /// in the message.
    fn enable_builtin(&mut self, name: &str) -> PyResult<()> {
        self.inner.enable_builtin(name).map_err(unknown_error)
    }
}

/// A parsed document: parse once, render or inspect many times.
///
/// The document owns everything it borrows; dropping the last Python
/// reference frees the parse. The options used at parse time are
/// remembered, so rendering needs no other argument and the options
/// object may be dropped first.
#[pyclass]
struct Document {
    inner: OwnedDocument,
}

#[pymethods]
impl Document {
    /// Parses `source` with the given options, or as plain CommonMark
    /// when they are omitted.
    #[new]
    #[pyo3(signature = (source, options=None))]
    fn parse(source: &str, options: Option<&Options>) -> PyResult<Self> {
        let options = options.map(|options| &options.inner);
        OwnedDocument::parse(source, options)
            .map(|inner| Self { inner })
            .map_err(panic_error)
    }

    /// The document's HTML, byte for byte as the Rust core renders it.
    ///
    /// Only an engine bug would raise here.
    fn html(&self) -> PyResult<String> {
        self.inner.render_html().map_err(panic_error)
    }

    /// The problems the parse noticed: broken references and similar
    /// issues the tree absorbed without changing the output, in
    /// document order. Spans are byte offsets into the UTF-8 encoding
    /// of the source, the same convention the core uses.
    fn problems(&self) -> Vec<Problem> {
        self.inner
            .problems()
            .into_iter()
            .map(Problem::from)
            .collect()
    }
}

/// One problem the parse noticed, with its span into the source.
///
/// Spans are byte offsets into the UTF-8 encoding of the source, the
/// same convention the core uses.
#[pyclass]
struct Problem {
    inner: ProblemReport,
}

impl From<ProblemReport> for Problem {
    fn from(inner: ProblemReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl Problem {
    /// What the problem is, as one line.
    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    /// The span's start, as a byte offset into the source.
    #[getter]
    fn start(&self) -> usize {
        self.inner.start
    }

    /// The span's end, exclusive, as a byte offset into the source.
    #[getter]
    fn end(&self) -> usize {
        self.inner.end
    }

    fn __repr__(&self) -> String {
        format!(
            "Problem(message={:?}, start={}, end={})",
            self.inner.message, self.inner.start, self.inner.end
        )
    }
}

/// Parses and renders in one call, as plain CommonMark. The
/// convenient path for callers who only want HTML; the `options` form
/// of the same call enables built-ins.
#[pyfunction]
#[pyo3(signature = (source, options=None))]
fn render_html(source: &str, options: Option<&Options>) -> PyResult<String> {
    let options = options.map(|options| &options.inner);
    safe::render_html(source, options).map_err(panic_error)
}

/// The extension module Python imports as `geist4._geist4`.
#[pymodule]
fn _geist4(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    module.add_function(wrap_pyfunction!(render_html, module)?)?;
    module.add_class::<Options>()?;
    module.add_class::<Document>()?;
    module.add_class::<Problem>()?;
    module.add("Geist4Error", module.py().get_type::<Geist4Error>())?;
    Ok(())
}
