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
//!
//! Host extensions register JavaScript callbacks at the engine's hook
//! points through `HostExtension` and `Options.enableHost`, the same
//! vocabulary the tier-2 adapter exposes and the other bindings share.

use geist4_ffi::safe::{
    AttrValue, BracketClaim, BracketHook, CloseHook, ContinueConsume, ContinueDecision,
    ContinueHook, CoreNode, HostExtension as SafeHostExtension, HostHooks, InitHook, KindDecl,
    LookHook, Options as SafeOptions, OverrideHook, OwnedDocument, PostHook, RenderNodeHook,
    StartConsume, StartDecision, StartHook, TriggerHook,
};
use js_sys::{Array, Function, Object, Reflect};
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

    /// Registers a host extension: JavaScript callbacks at the engine's
    /// hook points, the same vocabulary the tier-2 adapter exposes.
    ///
    /// The extension's declared data is validated exactly like a
    /// tier-2 manifest's, and its name is checked against the
    /// built-ins' and every earlier extension's.
    ///
    /// # Errors
    /// Invalid declared data — a reserved name, a malformed kind, a
    /// trigger that is not one byte — or a name already registered.
    #[wasm_bindgen(js_name = enableHost)]
    pub fn enable_host(&mut self, extension: &HostExtension) -> Result<(), JsError> {
        let host = extension
            .to_extension()
            .map_err(|error| JsError::new(&error.to_string()))?;
        self.inner
            .enable_host(host)
            .map(|_| ())
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Enables built-in syntax highlighting for fenced code blocks in
    /// HTML output. Off by default.
    #[wasm_bindgen(js_name = enableHighlighting)]
    pub fn enable_highlighting(&mut self) {
        self.inner.enable_highlighting();
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

// ---------------------------------------------------------------------------
// Host extensions: JavaScript callbacks at the engine's hook points, the
// same vocabulary the tier-2 adapter exposes and the other bindings share.
// ---------------------------------------------------------------------------

use std::cell::RefCell;
use std::sync::Arc;

thread_local! {
    /// The JavaScript callbacks host extensions registered, indexed by
    /// table position. The hook closures capture only their indices,
    /// which keeps them `Send` and `Sync` the way the shared hooks
    /// shape requires; on WebAssembly the runtime is single-threaded,
    /// so the per-thread table is the one table.
    static JS_HOOKS: RefCell<Vec<Function>> = RefCell::new(Vec::new());
}

/// Registers one callback in the table and returns its index.
fn table_index(function: &Function) -> usize {
    JS_HOOKS.with(|table| {
        let mut table = table.borrow_mut();
        table.push(function.clone());
        table.len() - 1
    })
}

/// Fetches the callback registered at `index`.
fn table_call(index: usize) -> Function {
    JS_HOOKS.with(|table| table.borrow()[index].clone())
}

/// Whether a hook declined: `null` or `undefined`.
fn declined(answer: &JsValue) -> bool {
    answer.is_null() || answer.is_undefined()
}

/// Reads one field from a plain object; absent, `null`, or an
/// unreachable read (plain objects only) is `None`.
fn field(object: &Object, name: &str) -> Option<JsValue> {
    let value = Reflect::get(object, &JsValue::from(name)).ok()?;
    if declined(&value) { None } else { Some(value) }
}

/// Reads one required field from a plain object, naming the owner in
/// the error.
fn require_field(object: &Object, name: &str, owner: &str) -> Result<JsValue, String> {
    match field(object, name) {
        Some(value) => Ok(value),
        None => Err(format!("{owner} needs a `{name}`")),
    }
}

/// Reads one attribute value: a boolean, an integral number, or a
/// string.
fn attr_from_js(value: &JsValue) -> Result<AttrValue, String> {
    if let Some(flag) = value.as_bool() {
        return Ok(AttrValue::Bool(flag));
    }
    if let Some(number) = value.as_f64() {
        if number.is_finite() && number.fract() == 0.0 {
            // Checked above: integral and finite, so the cast only
            // loses precision past 2^53, where a JS number cannot be
            // exact anyway.
            #[allow(clippy::cast_possible_truncation)]
            return Ok(AttrValue::Int(number as i64));
        }
        return Err("must be an integer".to_string());
    }
    if let Some(text) = value.as_string() {
        return Ok(AttrValue::Str(text));
    }
    Err("must be a string, number or boolean".to_string())
}

/// Reads a mapping of attribute names to values, in the mapping's
/// order.
fn attrs_from_js(value: &JsValue) -> Result<Vec<(String, AttrValue)>, String> {
    let mapping = Object::try_from(value)
        .ok_or_else(|| "the attributes must be a mapping from names to values".to_string())?;
    let mut attrs = Vec::new();
    for entry in Object::entries(mapping).iter() {
        let pair = entry
            .dyn_into::<Array>()
            .map_err(|_| "the attributes must be a mapping from names to values".to_string())?;
        let name = pair
            .get(0)
            .as_string()
            .ok_or_else(|| "every attribute name must be a string".to_string())?;
        let value = attr_from_js(&pair.get(1))
            .map_err(|message| format!("attribute {name:?}: {message}"))?;
        attrs.push((name, value));
    }
    Ok(attrs)
}

/// Reads one column count: non-integers, negatives, and values past
/// `u32::MAX` are errors, the other bindings' strictness.
fn column_count(value: &JsValue, what: &str) -> Result<u32, String> {
    let number = value
        .as_f64()
        .ok_or_else(|| format!("the {what} count must be a number"))?;
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > f64::from(u32::MAX)
    {
        return Err(format!("the {what} count must be a non-negative integer"));
    }
    // Checked above: integral, non-negative, in range.
    #[allow(clippy::cast_possible_truncation)]
    Ok(number as u32)
}

/// Reads a start consume: `"whole_line"` or `"rest"` on its own, or a
/// `["columns", n]` / `["claim", n]` pair.
fn start_consume_from_js(value: &JsValue) -> Result<StartConsume, String> {
    if let Some(text) = value.as_string() {
        return match text.as_str() {
            "whole_line" => Ok(StartConsume::WholeLine),
            "rest" => Ok(StartConsume::Rest),
            _ => Err(format!("unknown start consume `{text}`")),
        };
    }
    let array = value
        .clone()
        .dyn_into::<Array>()
        .map_err(|_| "the start consume must be a string or a [name, n] pair".to_string())?;
    let name = array
        .get(0)
        .as_string()
        .ok_or_else(|| "the start consume's first item must be `columns` or `claim`".to_string())?;
    let count = column_count(&array.get(1), &name)?;
    match name.as_str() {
        "columns" => Ok(StartConsume::Columns(count)),
        "claim" => Ok(StartConsume::Claim(count)),
        _ => Err(format!("unknown start consume `{name}`")),
    }
}

/// Reads a start decision: `{ consume, kind, attrs? }`.
fn start_from_js(answer: &JsValue) -> Result<StartDecision, String> {
    let answer = Object::try_from(answer)
        .ok_or_else(|| "the start decision must be an object".to_string())?;
    let consume = require_field(answer, "consume", "the start decision")?;
    let kind = require_field(answer, "kind", "the start decision")?
        .as_string()
        .ok_or_else(|| "the start decision's `kind` must be a string".to_string())?;
    let attrs = match field(answer, "attrs") {
        Some(value) => attrs_from_js(&value)?,
        None => Vec::new(),
    };
    Ok(StartDecision {
        consume: start_consume_from_js(&consume)?,
        kind,
        attrs,
    })
}

/// Reads a continue consume: `"whole_line"`, `"rest"`, or a
/// `["columns", n]` pair.
fn continue_consume_from_js(value: &JsValue) -> Result<ContinueConsume, String> {
    if let Some(text) = value.as_string() {
        return match text.as_str() {
            "whole_line" => Ok(ContinueConsume::WholeLine),
            "rest" => Ok(ContinueConsume::Rest),
            _ => Err(format!("unknown continue consume `{text}`")),
        };
    }
    let array = value
        .clone()
        .dyn_into::<Array>()
        .map_err(|_| "the continue consume must be a string or a [name, n] pair".to_string())?;
    let name = array
        .get(0)
        .as_string()
        .ok_or_else(|| "the continue consume's first item must be `columns`".to_string())?;
    if name != "columns" {
        return Err(format!("unknown continue consume `{name}`"));
    }
    Ok(ContinueConsume::Columns(column_count(
        &array.get(1),
        &name,
    )?))
}

/// Reads a continue decision: `{ consume }`, where the consume may be
/// `"close_line"`.
fn continue_from_js(answer: &JsValue) -> Result<ContinueDecision, String> {
    let answer = Object::try_from(answer)
        .ok_or_else(|| "the continue decision must be an object".to_string())?;
    let consume = require_field(answer, "consume", "the continue decision")?;
    if consume.as_string().as_deref() == Some("close_line") {
        return Ok(ContinueDecision::CloseLine);
    }
    Ok(ContinueDecision::Keep(continue_consume_from_js(&consume)?))
}

/// Reads a bracket claim: `{ kind, end, attrs? }`.
fn bracket_from_js(answer: &JsValue) -> Result<BracketClaim, String> {
    let answer = Object::try_from(answer)
        .ok_or_else(|| "the bracket claim must be an object".to_string())?;
    let kind = require_field(answer, "kind", "the bracket claim")?
        .as_string()
        .ok_or_else(|| "the bracket claim's `kind` must be a string".to_string())?;
    let end = require_field(answer, "end", "the bracket claim")?;
    let end = column_count(&end, "end")?;
    let attrs = match field(answer, "attrs") {
        Some(value) => attrs_from_js(&value)?,
        None => Vec::new(),
    };
    Ok(BracketClaim { kind, end, attrs })
}

/// The message a thrown JS value becomes: an `Error`'s own `toString`
/// (`Error: host callback exploded`), a thrown string as-is, and
/// anything else as its debug shape.
fn js_message(error: JsValue) -> String {
    match error.dyn_into::<js_sys::Error>() {
        Ok(error) => error.to_string().into(),
        Err(value) => value.as_string().unwrap_or_else(|| format!("{value:?}")),
    }
}

/// Reads a hook's boolean answer: anything but `true` or `false` is an
/// error.
fn expect_bool(answer: JsValue, hook: &str) -> Result<bool, String> {
    answer
        .as_bool()
        .ok_or_else(|| format!("the {hook} hook must return true or false"))
}

/// Reads a hook's optional string answer: `null` or `undefined` is
/// `None`.
fn optional_string(answer: JsValue, hook: &str) -> Result<Option<String>, String> {
    if declined(&answer) {
        return Ok(None);
    }
    answer
        .as_string()
        .map(Some)
        .ok_or_else(|| format!("the {hook} hook must return a string or null"))
}

/// Builds a plain JS object from attribute name/value pairs, in order.
fn attrs_object<'a>(pairs: impl Iterator<Item = (&'a str, JsValue)>) -> JsValue {
    let mapping = Object::new();
    for (name, value) in pairs {
        // A plain object cannot fail to take a property.
        let _taken = Reflect::set(&mapping, &JsValue::from(name), &value);
    }
    mapping.into()
}

/// The value a declared attribute renders as: booleans as booleans,
/// integers as numbers, strings as strings.
fn value_to_js(value: &AttrValue) -> JsValue {
    match value {
        AttrValue::Bool(flag) => JsValue::from(*flag),
        AttrValue::Int(number) => {
            // JS numbers are f64; beyond 2^53 a JS number cannot be
            // exact anyway, so the cast loses nothing representable.
            #[allow(clippy::cast_precision_loss)]
            let number = *number as f64;
            JsValue::from(number)
        }
        AttrValue::Str(text) => JsValue::from(text.as_str()),
    }
}

/// Reads the attribute names a kind accepts: from a list shape, the
/// list itself; from a mapping shape, its `attrs` field.
fn kind_attrs(shape: &Object) -> Result<Vec<String>, String> {
    if shape.is_instance_of::<Array>() {
        return attr_names(&shape.clone().into());
    }
    match field(shape, "attrs") {
        Some(value) => attr_names(&value),
        None => Ok(Vec::new()),
    }
}

/// Reads a list of attribute names.
fn attr_names(value: &JsValue) -> Result<Vec<String>, String> {
    let list = value
        .clone()
        .dyn_into::<Array>()
        .map_err(|_| "the attribute names must be a list of strings".to_string())?;
    let mut names = Vec::new();
    for item in list.iter() {
        let name = item
            .as_string()
            .ok_or_else(|| "every attribute name must be a string".to_string())?;
        names.push(name);
    }
    Ok(names)
}

/// Reads one of a kind's optional boolean flags.
fn optional_flag(shape: &Object, name: &str) -> Result<bool, String> {
    match field(shape, name) {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| format!("the kind's `{name}` flag must be a boolean")),
        None => Ok(false),
    }
}

/// One kind's declared shape: a mapping with optional `attrs`
/// (attribute names), `container` and `inline_text` flags, or just a
/// list of attribute names (an inline kind).
fn kind_decl(name: &str, shape: &Object) -> Result<KindDecl, String> {
    let (container, inline_text, attrs) = if shape.is_instance_of::<Array>() {
        (false, true, kind_attrs(shape)?)
    } else {
        (
            optional_flag(shape, "container")?,
            optional_flag(shape, "inline_text")?,
            kind_attrs(shape)?,
        )
    };
    Ok(KindDecl {
        kind: name.to_string(),
        container,
        inline_text,
        attrs,
    })
}

/// Reads the declared kinds: a mapping from kind names to shapes, or a
/// list of `[name, shape]` pairs.
fn kinds_from_js(value: &JsValue) -> Result<Vec<KindDecl>, String> {
    let mut out = Vec::new();
    if let Ok(list) = value.clone().dyn_into::<Array>() {
        for entry in list.iter() {
            let pair = entry
                .dyn_into::<Array>()
                .map_err(|_| "a kind entry must be a [name, shape] pair".to_string())?;
            let name = pair
                .get(0)
                .as_string()
                .ok_or_else(|| "every kind name must be a string".to_string())?;
            let shape_value = pair.get(1);
            let shape = Object::try_from(&shape_value)
                .ok_or_else(|| "a kind's shape must be an object".to_string())?;
            out.push(kind_decl(&name, shape)?);
        }
        return Ok(out);
    }
    let mapping = Object::try_from(value)
        .ok_or_else(|| "the kinds must be a mapping from names to shapes".to_string())?;
    for entry in Object::entries(mapping).iter() {
        let pair = entry
            .dyn_into::<Array>()
            .map_err(|_| "the kinds must be a mapping from names to shapes".to_string())?;
        let name = pair
            .get(0)
            .as_string()
            .ok_or_else(|| "every kind name must be a string".to_string())?;
        let shape_value = pair.get(1);
        let shape = Object::try_from(&shape_value)
            .ok_or_else(|| "a kind's shape must be an object".to_string())?;
        out.push(kind_decl(&name, shape)?);
    }
    Ok(out)
}

/// Reads the declared inline triggers: each one byte long.
fn triggers_from_js(value: &JsValue) -> Result<Vec<u8>, String> {
    let list = value
        .clone()
        .dyn_into::<Array>()
        .map_err(|_| "the triggers must be a list of one-character strings".to_string())?;
    let mut out = Vec::new();
    for trigger in list.iter() {
        let text = trigger
            .as_string()
            .ok_or_else(|| "every trigger must be a string".to_string())?;
        if text.len() != 1 {
            return Err(format!("an inline trigger must be one byte, got {text:?}"));
        }
        out.push(text.into_bytes()[0]);
    }
    Ok(out)
}

/// The optional JavaScript callbacks, by hook.
#[derive(Default)]
struct JsHooks {
    init: Option<Function>,
    block_start_look: Option<Function>,
    block_start: Option<Function>,
    block_continue: Option<Function>,
    block_close: Option<Function>,
    inline_trigger: Option<Function>,
    inline_bracket: Option<Function>,
    post_process: Option<Function>,
    render: Option<Function>,
    render_override: Option<Function>,
}

impl JsHooks {
    /// Reads the callbacks from the config's `hooks` object; an absent
    /// callback declines.
    fn from_config(hooks: &JsValue, extension: &str) -> Result<Self, String> {
        let hooks = Object::try_from(hooks)
            .ok_or_else(|| format!("{extension} hooks must be an object"))?;
        let read = |js_name: &str, rust_name: &str| -> Result<Option<Function>, String> {
            match field(hooks, js_name) {
                Some(value) => value
                    .dyn_into::<Function>()
                    .map(Some)
                    .map_err(|_| format!("the {rust_name} hook must be a function")),
                None => Ok(None),
            }
        };
        Ok(Self {
            init: read("init", "init")?,
            block_start_look: read("blockStartLook", "block-start-look")?,
            block_start: read("blockStart", "block-start")?,
            block_continue: read("blockContinue", "block-continue")?,
            block_close: read("blockClose", "block-close")?,
            inline_trigger: read("inlineTrigger", "inline-trigger")?,
            inline_bracket: read("inlineBracket", "inline-bracket")?,
            post_process: read("postProcess", "post-process")?,
            render: read("render", "render")?,
            render_override: read("renderOverride", "render-override")?,
        })
    }

    /// Maps the callbacks onto the shared hooks shape, the same shape
    /// the Python binding fills. Every missing hook stays absent,
    /// which declines exactly like every other binding.
    ///
    /// The callbacks live in a per-thread table; the closures capture
    /// only their table indices, which keeps them `Send` and `Sync`
    /// the way the shared hooks shape requires. A hook must not
    /// re-enter the engine (for example by calling `parse` from
    /// inside a hook): the table is borrowed for the duration of each
    /// call.
    fn to_hooks(&self) -> HostHooks {
        let init = self.init.as_ref().map(table_index);
        let block_start_look = self.block_start_look.as_ref().map(table_index);
        let block_start = self.block_start.as_ref().map(table_index);
        let block_continue = self.block_continue.as_ref().map(table_index);
        let block_close = self.block_close.as_ref().map(table_index);
        let inline_trigger = self.inline_trigger.as_ref().map(table_index);
        let inline_bracket = self.inline_bracket.as_ref().map(table_index);
        let post_process = self.post_process.as_ref().map(table_index);
        let render = self.render.as_ref().map(table_index);
        let render_override = self.render_override.as_ref().map(table_index);

        HostHooks {
            init: init.map(|index| -> InitHook {
                Box::new(move || {
                    table_call(index)
                        .call0(&JsValue::UNDEFINED)
                        .map(|_| ())
                        .map_err(js_message)
                })
            }),
            block_start_look: block_start_look.map(|index| -> LookHook {
                Box::new(move |line: &str, indented: bool, column: u32| {
                    let hook = table_call(index);
                    expect_bool(
                        hook.call3(
                            &JsValue::UNDEFINED,
                            &JsValue::from(line),
                            &JsValue::from(indented),
                            &JsValue::from(column),
                        )
                        .map_err(js_message)?,
                        "block-start-look",
                    )
                })
            }),
            block_start: block_start.map(|index| -> StartHook {
                Box::new(
                    move |line: &str, indented: bool, column: u32, container: Option<&str>| {
                        let hook = table_call(index);
                        let answer = hook
                            .call4(
                                &JsValue::UNDEFINED,
                                &JsValue::from(line),
                                &JsValue::from(indented),
                                &JsValue::from(column),
                                &container.map(JsValue::from).unwrap_or(JsValue::UNDEFINED),
                            )
                            .map_err(js_message)?;
                        if declined(&answer) {
                            return Ok(None);
                        }
                        start_from_js(&answer)
                            .map(Some)
                            .map_err(|message| format!("block-start: {message}"))
                    },
                )
            }),
            block_continue: block_continue.map(|index| -> ContinueHook {
                Box::new(
                    move |line: &str, indented: bool, column: u32, blank: bool| {
                        let hook = table_call(index);
                        let answer = hook
                            .call4(
                                &JsValue::UNDEFINED,
                                &JsValue::from(line),
                                &JsValue::from(indented),
                                &JsValue::from(column),
                                &JsValue::from(blank),
                            )
                            .map_err(js_message)?;
                        if declined(&answer) {
                            return Ok(None);
                        }
                        continue_from_js(&answer)
                            .map(Some)
                            .map_err(|message| format!("block-continue: {message}"))
                    },
                )
            }),
            block_close: block_close.map(|index| -> CloseHook {
                Box::new(move || {
                    let hook = table_call(index);
                    optional_string(
                        hook.call0(&JsValue::UNDEFINED).map_err(js_message)?,
                        "block-close",
                    )
                })
            }),
            inline_trigger: inline_trigger.map(|index| -> TriggerHook {
                Box::new(move |text: &str, column: u32| {
                    let hook = table_call(index);
                    optional_string(
                        hook.call2(
                            &JsValue::UNDEFINED,
                            &JsValue::from(text),
                            &JsValue::from(column),
                        )
                        .map_err(js_message)?,
                        "inline-trigger",
                    )
                })
            }),
            inline_bracket: inline_bracket.map(|index| -> BracketHook {
                Box::new(move |text: &str| {
                    let hook = table_call(index);
                    let answer = hook
                        .call1(&JsValue::UNDEFINED, &JsValue::from(text))
                        .map_err(js_message)?;
                    if declined(&answer) {
                        return Ok(None);
                    }
                    bracket_from_js(&answer)
                        .map(Some)
                        .map_err(|message| format!("inline-bracket: {message}"))
                })
            }),
            post_process: post_process.map(|index| -> PostHook {
                Box::new(move |text: &str| {
                    let hook = table_call(index);
                    optional_string(
                        hook.call1(&JsValue::UNDEFINED, &JsValue::from(text))
                            .map_err(js_message)?,
                        "post-process",
                    )
                })
            }),
            render: render.map(|index| -> RenderNodeHook {
                Box::new(
                    move |kind: &str, attrs: &[(String, String)], children: &str| {
                        let hook = table_call(index);
                        let mapping = attrs
                            .iter()
                            .map(|(name, value)| (name.as_str(), JsValue::from(value.as_str())))
                            .collect::<Vec<_>>();
                        let answer = hook
                            .call3(
                                &JsValue::UNDEFINED,
                                &JsValue::from(kind),
                                &attrs_object(mapping.into_iter()),
                                &JsValue::from(children),
                            )
                            .map_err(js_message)?;
                        if declined(&answer) {
                            return Ok(None);
                        }
                        answer
                            .as_string()
                            .map(Some)
                            .ok_or_else(|| {
                                "the render hook must return a string or null".to_string()
                            })
                            .map_err(|message| format!("render: {message}"))
                    },
                )
            }),
            render_override: render_override.map(|index| -> OverrideHook {
                Box::new(
                    move |node: &CoreNode<'_>, children: &[String], out: &mut String| {
                        let hook = table_call(index);
                        let attrs = node
                            .attrs
                            .iter()
                            .map(|(name, value)| (name.as_str(), value_to_js(value)))
                            .collect::<Vec<_>>();
                        let list = children
                            .iter()
                            .map(|text| JsValue::from(text.as_str()))
                            .collect::<Array>();
                        let answer = hook
                            .call3(
                                &JsValue::UNDEFINED,
                                &JsValue::from(node.kind),
                                &attrs_object(attrs.into_iter()),
                                &list.into(),
                            )
                            .map_err(js_message)?;
                        if declined(&answer) {
                            return Ok(false);
                        }
                        let text = answer.as_string().ok_or_else(|| {
                            "the render-override hook must return a string or null".to_string()
                        })?;
                        out.push_str(&text);
                        Ok(true)
                    },
                )
            }),
        }
    }
}

/// A host extension: JavaScript callbacks the engine calls at its
/// extension points, the tier-3 counterpart of a built-in. Register
/// it on `Options` with `enableHost`.
///
/// The config is a plain object: `{ name, kinds?, triggers?, hooks? }`.
/// `kinds` maps a kind name to a shape — a mapping with optional
/// `attrs` (attribute names), `container` and `inline_text` flags, or
/// just a list of attribute names (an inline kind) — and `triggers`
/// is a list of one-character strings. Both are validated when the
/// extension is enabled, as a manifest would be.
///
/// `hooks` is an object of optional functions: `init()`,
/// `blockStartLook(line, indented, column)`, `blockStart(line,
/// indented, column, container)`, `blockContinue(line, indented,
/// column, blank)`, `blockClose()`, `inlineTrigger(text, column)`,
/// `inlineBracket(text)`, `postProcess(text)`, `render(kind, attrs,
/// children)` and `renderOverride(kind, attrs, children)`. An absent
/// hook declines; a hook that throws surfaces as a parse problem.
#[wasm_bindgen]
pub struct HostExtension {
    name: String,
    kinds: Vec<KindDecl>,
    triggers: Vec<u8>,
    hooks: JsHooks,
}

#[wasm_bindgen]
impl HostExtension {
    /// Builds the extension from its config object.
    #[wasm_bindgen(constructor)]
    pub fn new(config: &JsValue) -> Result<Self, JsValue> {
        let config = Object::try_from(config)
            .ok_or_else(|| JsValue::from_str("the extension config must be an object"))?;
        let name = require_field(config, "name", "the extension")
            .and_then(|value| {
                value
                    .as_string()
                    .ok_or_else(|| "the extension's `name` must be a string".to_string())
            })
            .map_err(|message| JsValue::from_str(&message))?;
        let kinds = match field(config, "kinds") {
            Some(value) => kinds_from_js(&value).map_err(|message| JsValue::from_str(&message))?,
            None => Vec::new(),
        };
        let triggers = match field(config, "triggers") {
            Some(value) => {
                triggers_from_js(&value).map_err(|message| JsValue::from_str(&message))?
            }
            None => Vec::new(),
        };
        let hooks = match field(config, "hooks") {
            Some(value) => JsHooks::from_config(&value, &name)
                .map_err(|message| JsValue::from_str(&message))?,
            None => JsHooks::default(),
        };
        Ok(Self {
            name,
            kinds,
            triggers,
            hooks,
        })
    }

    /// The extension's name.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Builds the shared extension the engine loads.
    fn to_extension(&self) -> Result<Arc<SafeHostExtension>, String> {
        let hooks = self.hooks.to_hooks();
        SafeHostExtension::from_hooks(
            self.name.clone(),
            self.kinds.clone(),
            self.triggers.clone(),
            hooks,
        )
        .map(Arc::new)
        .map_err(|error| error.to_string())
    }
}
