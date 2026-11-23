//! The Python binding: the [`geist4_ffi::safe`] API as a native
//! extension module, packaged as wheels with maturin.
//!
//! The surface mirrors the JavaScript and Swift bindings one for one:
//! `version`, `render_html`, `Options` with `enable_builtin`,
//! `Document` with `html` and `problems`, and one exception type,
//! [`Geist4Error`]. Host extensions (tier 3) supply the same hook
//! shapes tier 2 manifests describe as Python callbacks, registered
//! through `Options.enable_host`. The binding itself is plain safe Rust — the
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
    self, AttrValue, BracketClaim, BracketHook, CloseHook, ContinueConsume, ContinueDecision,
    ContinueHook, CoreNode, Extension, HostError, HostExtension as SafeHostExtension, HostHooks,
    InitHook, KindDecl, LookHook, Options as SafeOptions, OverrideHook, OwnedDocument, PanicReport,
    PostHook, ProblemReport, RenderNodeHook, StartConsume, StartDecision, StartHook, TriggerHook,
    UnknownBuiltin,
};
use pyo3::IntoPyObjectExt;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList};
use std::sync::Arc;

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

    /// Registers a host extension: Python callbacks at the engine's
    /// extension points. The extension's declared data is validated
    /// here — as a manifest would be — and a duplicate name raises
    /// `Geist4Error`.
    fn enable_host(&mut self, extension: &HostExtension) -> PyResult<()> {
        let host = extension.to_extension().map_err(host_error)?;
        self.inner.enable_host(host).map_err(host_error)?;
        Ok(())
    }

    /// Enables built-in syntax highlighting for fenced code blocks in
    /// HTML output. Off by default.
    fn enable_highlighting(&mut self) {
        self.inner.enable_highlighting();
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

// ---------------------------------------------------------------------------
// Host extensions (tier 3): Python callbacks at the engine's hook points.
// ---------------------------------------------------------------------------

/// The message a raised Python exception becomes: the hook's failure
/// message, which latches like a C hook's negative return.
fn py_message(error: PyErr) -> String {
    error.to_string()
}

/// The error for a decision mapping that lacks a required field.
fn missing_field(name: &str) -> String {
    format!("the decision has no `{name}` field")
}

/// Reads one required field from a decision mapping.
fn require_field<'py>(answer: &Bound<'py, PyAny>, name: &str) -> Result<Bound<'py, PyAny>, String> {
    match answer.get_item(name) {
        Ok(item) => Ok(item),
        Err(_) => Err(missing_field(name)),
    }
}

/// Reads one optional field from a decision mapping.
fn optional_field<'py>(
    answer: &Bound<'py, PyAny>,
    name: &str,
) -> Result<Option<Bound<'py, PyAny>>, String> {
    match answer.get_item(name) {
        Ok(item) if item.is_none() => Ok(None),
        Ok(item) => Ok(Some(item)),
        Err(_) => Ok(None),
    }
}

/// Converts one attribute value: bool first — a Python bool is an int
/// too — then int, then string.
fn attr_from_py(value: &Bound<'_, PyAny>) -> Result<AttrValue, String> {
    if value.is_instance_of::<PyBool>() {
        return value
            .extract::<bool>()
            .map(AttrValue::Bool)
            .map_err(py_message);
    }
    if let Ok(int) = value.extract::<i64>() {
        return Ok(AttrValue::Int(int));
    }
    value
        .extract::<String>()
        .map(AttrValue::Str)
        .map_err(py_message)
}

/// Converts a Python mapping of attributes into the engine's list, in
/// the mapping's order. Accepts a mapping from attribute names to
/// values, or a sequence of `(name, value)` pairs.
fn attrs_from_py(value: &Bound<'_, PyAny>) -> Result<Vec<(String, AttrValue)>, String> {
    if let Ok(pairs) = value.extract::<Vec<(String, Bound<'_, PyAny>)>>() {
        return pairs
            .into_iter()
            .map(|(name, value)| attr_from_py(&value).map(|value| (name, value)))
            .collect();
    }
    let pairs = entry_pairs(Some(value), "attrs").map_err(py_message)?;
    pairs
        .into_iter()
        .map(|(name, value)| attr_from_py(&value).map(|value| (name, value)))
        .collect()
}

/// Reads a block-start consume: `whole_line` or `rest` on its own,
/// or `("columns", n)` / `("claim", n)` as a pair.
fn start_consume_from_py(value: &Bound<'_, PyAny>) -> Result<StartConsume, String> {
    if let Ok(name) = value.extract::<String>() {
        return match name.as_str() {
            "whole_line" => Ok(StartConsume::WholeLine),
            "rest" => Ok(StartConsume::Rest),
            other => Err(format!("unknown block-start consume {other:?}")),
        };
    }
    let (name, columns) = value.extract::<(String, u32)>().map_err(py_message)?;
    match name.as_str() {
        "columns" => Ok(StartConsume::Columns(columns)),
        "claim" => Ok(StartConsume::Claim(columns)),
        other => Err(format!("unknown block-start consume {other:?}")),
    }
}

/// Reads a block-start decision from the mapping Python returned.
fn start_from_py(answer: &Bound<'_, PyAny>) -> Result<StartDecision, String> {
    Ok(StartDecision {
        consume: start_consume_from_py(&require_field(answer, "consume")?)?,
        kind: require_field(answer, "kind")?
            .extract::<String>()
            .map_err(py_message)?,
        attrs: match optional_field(answer, "attrs")? {
            Some(value) => attrs_from_py(&value)?,
            None => Vec::new(),
        },
    })
}

/// Reads a block-continue consume: `whole_line` or `rest` on its own,
/// or `("columns", n)` as a pair.
fn continue_consume_from_py(value: &Bound<'_, PyAny>) -> Result<ContinueConsume, String> {
    if let Ok(name) = value.extract::<String>() {
        return match name.as_str() {
            "whole_line" => Ok(ContinueConsume::WholeLine),
            "rest" => Ok(ContinueConsume::Rest),
            other => Err(format!("unknown block-continue consume {other:?}")),
        };
    }
    let (name, columns) = value.extract::<(String, u32)>().map_err(py_message)?;
    match name.as_str() {
        "columns" => Ok(ContinueConsume::Columns(columns)),
        other => Err(format!("unknown block-continue consume {other:?}")),
    }
}

/// Reads a block-continue decision: `{"consume": "close_line"}` closes
/// the block and consumes the line; other `{"consume": ...}` shapes
/// keep the block open. Python's `None` declines and never reaches
/// here.
fn continue_from_py(answer: &Bound<'_, PyAny>) -> Result<ContinueDecision, String> {
    let consume = require_field(answer, "consume")?;
    if consume.extract::<String>().ok().as_deref() == Some("close_line") {
        return Ok(ContinueDecision::CloseLine);
    }
    Ok(ContinueDecision::Keep(continue_consume_from_py(&consume)?))
}

/// Reads a bracket claim from the mapping Python returned.
fn bracket_from_py(answer: &Bound<'_, PyAny>) -> Result<BracketClaim, String> {
    Ok(BracketClaim {
        kind: require_field(answer, "kind")?
            .extract::<String>()
            .map_err(py_message)?,
        attrs: match optional_field(answer, "attrs")? {
            Some(value) => attrs_from_py(&value)?,
            None => Vec::new(),
        },
        end: require_field(answer, "end")?
            .extract::<u32>()
            .map_err(py_message)?,
    })
}

/// Converts one engine attribute value to Python.
fn value_to_py(py: Python<'_>, value: &AttrValue) -> Py<PyAny> {
    match value {
        AttrValue::Bool(flag) => (*flag)
            .into_bound_py_any(py)
            .expect("a bool converts")
            .unbind(),
        AttrValue::Int(int) => (*int)
            .into_bound_py_any(py)
            .expect("an int converts")
            .unbind(),
        AttrValue::Str(text) => text
            .as_str()
            .into_bound_py_any(py)
            .expect("a str converts")
            .unbind(),
    }
}

/// Checks a callback argument is callable, for a clear error at
/// construction instead of a failure at the hook.
fn callable(
    py: Python<'_>,
    extension: &str,
    name: &str,
    hook: Option<Py<PyAny>>,
) -> PyResult<Option<Py<PyAny>>> {
    match hook {
        Some(hook) if hook.bind(py).is_callable() => Ok(Some(hook)),
        Some(_) => Err(Geist4Error::new_err(format!(
            "extension {extension:?}: the {name} callback is not callable"
        ))),
        None => Ok(None),
    }
}

/// Maps a binding-side message — bad declared data, a bad decision
/// shape — onto the binding's one exception type.
fn binding_error(message: String) -> PyErr {
    Geist4Error::new_err(message)
}

/// Maps a [`HostError`] — invalid declared data, or a duplicate name
/// — onto the binding's one exception type.
fn host_error(error: HostError) -> PyErr {
    Geist4Error::new_err(error.to_string())
}

/// The optional Python callbacks, stored by hook.
#[derive(Default)]
struct PyHooks {
    init: Option<Py<PyAny>>,
    block_start_look: Option<Py<PyAny>>,
    block_start: Option<Py<PyAny>>,
    block_continue: Option<Py<PyAny>>,
    block_close: Option<Py<PyAny>>,
    inline_trigger: Option<Py<PyAny>>,
    inline_bracket: Option<Py<PyAny>>,
    post_process: Option<Py<PyAny>>,
    render: Option<Py<PyAny>>,
    render_override: Option<Py<PyAny>>,
}

/// Reads `(name, value)` entries that may arrive as a list of pairs or
/// as a mapping from names to values; `what` names the collection in
/// the error message.
fn entry_pairs<'py>(
    entries: Option<&Bound<'py, PyAny>>,
    what: &str,
) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    let Some(entries) = entries else {
        return Ok(Vec::new());
    };
    if entries.is_instance_of::<PyDict>() {
        let dict = entries.cast::<PyDict>()?;
        let mut out = Vec::new();
        for (name, value) in dict.iter() {
            let name: String = name
                .extract()
                .map_err(|_| binding_error(format!("every {what} name must be a string")))?;
            out.push((name, value));
        }
        return Ok(out);
    }
    entries
        .extract::<Vec<(String, Bound<'py, PyAny>)>>()
        .map_err(|_| {
            binding_error(format!(
                "the {what} must be a mapping or a list of (name, value) pairs"
            ))
        })
}

/// Reads the declared kinds. Each entry maps a kind name either to a
/// mapping with the optional `attrs`, `container` and `inline_text`
/// fields, or to a list of attribute names (an inline kind). The
/// collection of entries itself may be a list of `(name, shape)`
/// pairs or a mapping from kind names to shapes.
fn kinds_from_py(kinds: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<KindDecl>> {
    let mut out = Vec::new();
    for (name, shape) in entry_pairs(kinds, "kinds")? {
        let (container, inline_text, attrs) = if shape.is_instance_of::<PyList>() {
            (false, true, shape.extract::<Vec<String>>()?)
        } else if shape.is_instance_of::<PyDict>() {
            (
                optional_flag(&shape, "container")?,
                optional_flag(&shape, "inline_text")?,
                optional_names(&shape)?,
            )
        } else {
            return Err(Geist4Error::new_err(format!(
                "the kind {name:?} must be a list of attribute names or a mapping"
            )));
        };
        out.push(KindDecl {
            kind: name,
            container,
            inline_text,
            attrs,
        });
    }
    Ok(out)
}

/// Reads an optional bool field from a kind mapping.
fn optional_flag(shape: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    match optional_field(shape, name).map_err(binding_error)? {
        Some(value) => value.extract::<bool>(),
        None => Ok(false),
    }
}

/// Reads the optional `attrs` field from a kind mapping.
fn optional_names(shape: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    match optional_field(shape, "attrs").map_err(binding_error)? {
        Some(value) => value.extract::<Vec<String>>(),
        None => Ok(Vec::new()),
    }
}

/// Reads the declared inline triggers: each one byte long.
fn triggers_from_py(triggers: Option<Vec<String>>) -> PyResult<Vec<u8>> {
    let mut out = Vec::new();
    for trigger in triggers.unwrap_or_default() {
        let bytes = trigger.as_bytes();
        if bytes.len() != 1 {
            return Err(Geist4Error::new_err(format!(
                "an inline trigger must be one byte, got {trigger:?}"
            )));
        }
        out.push(bytes[0]);
    }
    Ok(out)
}

/// A host extension: Python callbacks the engine calls at its
/// extension points, the tier-3 counterpart of a built-in. Register
/// it on `Options` with `enable_host`.
///
/// Declare the node kinds the extension can open or claim — `kinds`
/// maps a kind name to a mapping with optional `attrs` (attribute
/// names), `container` and `inline_text` flags, or to just a list of
/// attribute names (an inline kind) — and the inline trigger bytes
/// (`triggers`, each one byte). Both are validated when the extension
/// is enabled, as a manifest would be.
///
/// Every callback is optional; an absent callback declines:
///
/// - `init()` — called once per parse before any other hook.
/// - `block_start_look(line, indented, column)` — return whether the
///   extension might start a block on this line.
/// - `block_start(line, indented, column, container)` — return a
///   decision or `None` to decline. The decision is a mapping with
///   `consume` (`"whole_line"`, `("columns", n)`, `"rest"` or
///   `("claim", n)` — a claim marks the deepest plain container
///   instead of opening a block), `kind` and optional `attrs`.
/// - `block_continue(line, indented, column, blank)` — return
///   `{"consume": "whole_line" | ("columns", n) | "rest"}` to keep the
///   block open, `{"consume": "close_line"}` to close it and consume
///   the line (a closing fence), or `None` to decline: the line then
///   goes to the built-in parser as the block's child content.
/// - `block_close()` — return the block's whole HTML, or `None` to
///   render the children as a plain container's children.
/// - `inline_trigger(text, column)` — return the kind to tag the run
///   at the cursor with, or `None` to leave the byte as text.
/// - `inline_bracket(text)` — return a claim `{"kind": ..., "end":
///   n, "attrs": {...}}` — the content length to keep as the node's
///   children — or `None` to leave the group as text.
/// - `post_process(text)` — return the replacement for one text
///   node's contents, or `None` to leave it unchanged.
/// - `render(kind, attrs, children)` — return the extension node's
///   whole HTML, or `None` to render nothing.
/// - `render_override(kind, attrs, children)` — return a claimed core
///   node's whole HTML, or `None` to fall back to the built-in; the
///   children arrive pre-rendered and the returned HTML is appended.
///
/// Attributes are strings, ints or bools in, strings, ints or bools
/// out. A callback that raises fails the extension: it declines for
/// the rest of the parse and its message surfaces as a document
/// problem, like a C hook's negative return.
#[pyclass]
struct HostExtension {
    /// The extension's name: how it registers, and the prefix its
    /// problems carry.
    #[pyo3(get)]
    name: String,
    /// The declared kinds.
    kinds: Vec<KindDecl>,
    /// The declared inline trigger bytes.
    triggers: Vec<u8>,
    /// The optional Python callbacks, by hook.
    hooks: PyHooks,
}

impl HostExtension {
    /// Builds the closure surface from the callables and hands the
    /// extension to the safe module, which validates the declared
    /// data the same way it validates a tier-2 manifest.
    fn to_extension(&self) -> Result<Arc<dyn Extension>, HostError> {
        let hooks = &self.hooks;
        SafeHostExtension::from_hooks(
            self.name.clone(),
            self.kinds.clone(),
            self.triggers.clone(),
            HostHooks {
                init: hooks.init.clone().map(|hook| -> InitHook {
                    Box::new(move || {
                        Python::attach(|py| hook.bind(py).call0().map(|_| ()).map_err(py_message))
                    })
                }),
                block_start_look: hooks.block_start_look.clone().map(|hook| -> LookHook {
                    Box::new(move |line: &str, indented: bool, column: u32| {
                        Python::attach(|py| {
                            hook.bind(py)
                                .call1((line, indented, column))
                                .map_err(py_message)?
                                .extract::<bool>()
                                .map_err(py_message)
                        })
                    })
                }),
                block_start: hooks.block_start.clone().map(|hook| -> StartHook {
                    Box::new(
                        move |line: &str, indented: bool, column: u32, container: Option<&str>| {
                            Python::attach(|py| {
                                let answer = hook
                                    .bind(py)
                                    .call1((line, indented, column, container))
                                    .map_err(py_message)?;
                                if answer.is_none() {
                                    return Ok(None);
                                }
                                start_from_py(&answer)
                                    .map(Some)
                                    .map_err(|message| format!("block-start: {message}"))
                            })
                        },
                    )
                }),
                block_continue: hooks.block_continue.clone().map(|hook| -> ContinueHook {
                    Box::new(
                        move |line: &str, indented: bool, column: u32, blank: bool| {
                            Python::attach(|py| {
                                let answer = hook
                                    .bind(py)
                                    .call1((line, indented, column, blank))
                                    .map_err(py_message)?;
                                if answer.is_none() {
                                    return Ok(None);
                                }
                                continue_from_py(&answer)
                                    .map(Some)
                                    .map_err(|message| format!("block-continue: {message}"))
                            })
                        },
                    )
                }),
                block_close: hooks.block_close.clone().map(|hook| -> CloseHook {
                    Box::new(move || {
                        Python::attach(|py| {
                            hook.bind(py)
                                .call0()
                                .map_err(py_message)?
                                .extract::<Option<String>>()
                                .map_err(py_message)
                        })
                    })
                }),
                inline_trigger: hooks.inline_trigger.clone().map(|hook| -> TriggerHook {
                    Box::new(move |text: &str, column: u32| {
                        Python::attach(|py| {
                            hook.bind(py)
                                .call1((text, column))
                                .map_err(py_message)?
                                .extract::<Option<String>>()
                                .map_err(py_message)
                        })
                    })
                }),
                inline_bracket: hooks.inline_bracket.clone().map(|hook| -> BracketHook {
                    Box::new(move |text: &str| {
                        Python::attach(|py| {
                            let answer = hook.bind(py).call1((text,)).map_err(py_message)?;
                            if answer.is_none() {
                                return Ok(None);
                            }
                            bracket_from_py(&answer)
                                .map(Some)
                                .map_err(|message| format!("inline-bracket: {message}"))
                        })
                    })
                }),
                post_process: hooks.post_process.clone().map(|hook| -> PostHook {
                    Box::new(move |text: &str| {
                        Python::attach(|py| {
                            hook.bind(py)
                                .call1((text,))
                                .map_err(py_message)?
                                .extract::<Option<String>>()
                                .map_err(py_message)
                        })
                    })
                }),
                render: hooks.render.clone().map(|hook| -> RenderNodeHook {
                    Box::new(
                        move |kind: &str, attrs: &[(String, String)], children: &str| {
                            Python::attach(|py| {
                                let mapping = PyDict::new(py);
                                for (name, value) in attrs {
                                    mapping.set_item(name, value).map_err(py_message)?;
                                }
                                hook.bind(py)
                                    .call1((kind, mapping, children))
                                    .map_err(py_message)?
                                    .extract::<Option<String>>()
                                    .map_err(py_message)
                            })
                        },
                    )
                }),
                render_override: hooks.render_override.clone().map(|hook| -> OverrideHook {
                    Box::new(
                        move |node: &CoreNode<'_>, children: &[String], out: &mut String| {
                            Python::attach(|py| {
                                let mapping = PyDict::new(py);
                                for (name, value) in node.attrs {
                                    mapping
                                        .set_item(name, value_to_py(py, value))
                                        .map_err(py_message)?;
                                }
                                let answer = hook
                                    .bind(py)
                                    .call1((node.kind, mapping, children))
                                    .map_err(py_message)?;
                                let html =
                                    answer.extract::<Option<String>>().map_err(py_message)?;
                                if let Some(html) = html {
                                    out.push_str(&html);
                                    return Ok(true);
                                }
                                Ok(false)
                            })
                        },
                    )
                }),
            },
        )
        .map(|extension| Arc::new(extension) as Arc<dyn Extension>)
    }
}

#[pymethods]
impl HostExtension {
    /// Declares and collects the extension; validated at
    /// `Options.enable_host`. The class docstring documents each
    /// callback's contract.
    #[new]
    #[pyo3(signature = (
        name,
        kinds = None,
        triggers = None,
        *,
        init = None,
        block_start_look = None,
        block_start = None,
        block_continue = None,
        block_close = None,
        inline_trigger = None,
        inline_bracket = None,
        post_process = None,
        render = None,
        render_override = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        name: String,
        kinds: Option<&Bound<'_, PyAny>>,
        triggers: Option<Vec<String>>,
        init: Option<Py<PyAny>>,
        block_start_look: Option<Py<PyAny>>,
        block_start: Option<Py<PyAny>>,
        block_continue: Option<Py<PyAny>>,
        block_close: Option<Py<PyAny>>,
        inline_trigger: Option<Py<PyAny>>,
        inline_bracket: Option<Py<PyAny>>,
        post_process: Option<Py<PyAny>>,
        render: Option<Py<PyAny>>,
        render_override: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let hooks = PyHooks {
            init: callable(py, &name, "init", init)?,
            block_start_look: callable(py, &name, "block_start_look", block_start_look)?,
            block_start: callable(py, &name, "block_start", block_start)?,
            block_continue: callable(py, &name, "block_continue", block_continue)?,
            block_close: callable(py, &name, "block_close", block_close)?,
            inline_trigger: callable(py, &name, "inline_trigger", inline_trigger)?,
            inline_bracket: callable(py, &name, "inline_bracket", inline_bracket)?,
            post_process: callable(py, &name, "post_process", post_process)?,
            render: callable(py, &name, "render", render)?,
            render_override: callable(py, &name, "render_override", render_override)?,
        };
        Ok(Self {
            name,
            kinds: kinds_from_py(kinds)?,
            triggers: triggers_from_py(triggers)?,
            hooks,
        })
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
    module.add_class::<HostExtension>()?;
    module.add("Geist4Error", module.py().get_type::<Geist4Error>())?;
    Ok(())
}
