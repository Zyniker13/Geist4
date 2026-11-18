//! Tier-3 host extensions: extensions a binding supplies as callbacks.
//!
//! The extension format's third tier has no WebAssembly component: the
//! same manifest and spec describe the extension, but its behavior
//! lives in the host process, supplied through the binding's language
//! (see `docs/extensions/format.md` and decision 0035). This module is
//! the engine side of that contract, and it carries two surfaces that
//! build the same object:
//!
//! * [`Geist4HostVTable`] — a `#[repr(C)]` table of function pointers and
//!   declared data. This is the surface Swift speaks, through the C
//!   ABI, and the shape the checked-in header exposes.
//! * [`HostHooks`] — boxed closures. This is the surface the other
//!   bindings use: Python builds one from a Python object, the wasm
//!   binding from JavaScript functions, and neither writes a line of
//!   `unsafe`.
//!
//! Both go through [`HostExtension::from_vtable`] and
//! [`HostExtension::from_hooks`], which validate the declared data the
//! same way the loader validates a tier-2 manifest — name and kind
//! patterns, attribute-name patterns, trigger bytes — and then hand
//! the resulting [`Extension`] to `Options::enable_host`.
//!
//! # The vocabulary
//!
//! The hooks are the tier-2 WIT vocabulary plus `render_override`,
//! which the WIT world does not expose but the trait does and M7
//! requires. Two shapes go beyond tier 2's WIT, both documented in
//! decision 0035: a `claim` consume on `block-start` — the task-list
//! pattern, where the hook marks a core node the parser already
//! opened and opens no block, which is the tier-3 route to
//! `render_override` — and typed attribute values, where a decision's
//! attributes carry a tag per value instead of tier-2's strings-only
//! form.
//!
//! # Call contract
//!
//! * Every byte buffer the engine passes a hook is borrowed for the
//!   duration of the call only; every buffer a hook hands back is read
//!   before the call returns; a callback never keeps engine memory.
//! * The host owns the context pointer: it must stay valid for every
//!   call, and [`Geist4HostVTable::free_ctx`] runs when the last engine-side
//!   reference to the extension drops.
//! * Hook return values: `1` means the out-argument was written or
//!   the answer is yes, `0` means decline or no, a negative value
//!   reports failure.
//! * A hook that fails — a negative return, a malformed decision, an
//!   undeclared kind, or a panic — latches the first such error: later
//!   failures do not overwrite it, every hook declines until the host
//!   takes the error with `Extension::take_error`, and a new parse
//!   begins. That is the format's error-surfacing rule, shared with
//!   tier 2.
//!
//! # Memory discipline
//!
//! The declared data — the name, the kinds and their attribute names,
//! the trigger bytes — is copied at construction, so the host may free
//! its registration arrays as soon as `from_vtable` returns. Only the
//! context pointer is kept. This crate is the one place `unsafe` is
//! allowed (decision 0028); every raw-pointer read lives here, and
//! the bindings build on the safe closure surface or the C ABI.

use std::borrow::Cow;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ContinueOutcome, CoreNode, ExtNode,
    Extension, InlineBracketCtx, InlineTriggerCtx, KindDecl, LookCtx, OverrideChildren,
    RenderedChild, StartContainer, TreeCtx,
};
use geist4_core::node::Node;

// ---------------------------------------------------------------------------
// The C surface: value types and the vtable.
// ---------------------------------------------------------------------------

/// A borrowed byte span. The engine fills spans it passes to a hook;
/// the host fills spans it hands back, and the engine reads them
/// before the call returns.
#[repr(C)]
pub struct Geist4Span {
    /// The bytes, borrowed for the call. Null with `len == 0` is an
    /// empty span.
    pub data: *const u8,
    /// The span's length in bytes.
    pub len: usize,
}

/// One attribute: a name and a typed value. The value's shape follows
/// `tag`: [`GEIST4_ATTR_STR`] reads `text`, [`GEIST4_ATTR_INT`] reads
/// `number`, [`GEIST4_ATTR_BOOL`] reads `number` as zero or non-zero.
/// Unused fields stay zero/null.
#[repr(C)]
pub struct Geist4Attr {
    /// The attribute's name.
    pub name: Geist4Span,
    /// Which field carries the value: [`GEIST4_ATTR_STR`],
    /// [`GEIST4_ATTR_INT`] or [`GEIST4_ATTR_BOOL`].
    pub tag: i32,
    /// The value for the integer and boolean tags.
    pub number: i64,
    /// The value for the string tag.
    pub text: Geist4Span,
}

/// One declared kind: a name, its two shape flags and the attribute
/// names the kind carries.
#[repr(C)]
pub struct Geist4KindDecl {
    /// The kind's name.
    pub kind: Geist4Span,
    /// Non-zero when the kind opens a block that holds blocks.
    pub container: i32,
    /// Non-zero when the kind may hold inline text.
    pub inline_text: i32,
    /// The kind's attribute names.
    pub attrs: *const Geist4Span,
    /// The number of attribute names.
    pub attrs_len: usize,
}

/// A `block-start` decision. The `tag` picks both the consume and how
/// the other fields read:
///
/// * [`GEIST4_START_WHOLE_LINE`] — open with the whole line consumed;
///   `kind` and the attributes apply.
/// * [`GEIST4_START_COLUMNS`] — seek `columns` columns past the
///   content and open, leaving the rest of the line as the block's
///   first paragraph; `kind`, `columns` and the attributes apply.
/// * [`GEIST4_START_REST`] — seek to the content and open, leaving
///   the whole rest of the line as the block's first paragraph; `kind`
///   and the attributes apply.
/// * [`GEIST4_START_CLAIM`] — the task-list pattern: consume `columns`
///   bytes off the line, claim the deepest plain container with the
///   attributes, and open nothing; `columns` and the attributes apply,
///   `kind` is ignored.
#[repr(C)]
pub struct Geist4StartDecision {
    /// Which consume the decision names.
    pub tag: i32,
    /// The extension kind to open; ignored for the claim tag.
    pub kind: Geist4Span,
    /// The attributes to set on the opened or claimed node.
    pub attrs: *const Geist4Attr,
    /// The number of attributes.
    pub attrs_len: usize,
    /// The columns to seek for [`GEIST4_START_COLUMNS`], or the bytes
    /// to consume for [`GEIST4_START_CLAIM`].
    pub columns: u32,
}

/// A `block-continue` decision: [`GEIST4_CONTINUE_KEEP`] consumes the
/// line's remaining columns as `columns` names (`u32::MAX` for every
/// remaining column, `0` for none) and keeps the block open;
/// [`GEIST4_CONTINUE_CLOSE_LINE`] closes the block and consumes the
/// line whole.
#[repr(C)]
pub struct Geist4ContinueDecision {
    /// Which continue the decision names.
    pub tag: i32,
    /// The columns to consume for the keep tag.
    pub columns: u32,
}

/// An inline decision. The `tag` picks the action:
///
/// * [`GEIST4_INLINE_LITERAL`] — leave the bytes as text.
/// * [`GEIST4_INLINE_DELIMITER`] — tag the run at the cursor with
///   `kind` (the `inline-trigger` hook only).
/// * [`GEIST4_INLINE_NODE`] — claim the bracket group's content as an
///   inline node of `kind`, keeping up to `end` bytes of it as the
///   node's children (the `inline-bracket` hook only; the trigger
///   hook has no claim API and treats this tag as literal, the way
///   tier 2 does).
#[repr(C)]
pub struct Geist4InlineDecision {
    /// Which action the decision names.
    pub tag: i32,
    /// The extension kind for the delimiter and node tags.
    pub kind: Geist4Span,
    /// The content length to keep for the node tag; the whole group
    /// when it reaches past the content.
    pub end: u32,
    /// The typed attributes for the node tag, read with
    /// [`read_attrs`]; null with zero length for none. Ignored for the
    /// other tags.
    pub attrs: *const Geist4Attr,
    /// The attribute count; zero when `attrs` is null.
    pub attrs_len: usize,
}

/// A rendering a hook supplies: [`GEIST4_OUT_HTML`] with `html` set,
/// or [`GEIST4_OUT_NONE`] to decline.
#[repr(C)]
pub struct Geist4Rendered {
    /// Which outcome the hook chose.
    pub tag: i32,
    /// The HTML, for the [`GEIST4_OUT_HTML`] tag.
    pub html: Geist4Span,
}

/// The engine-provided fetch a render-override hook pulls children
/// with. `index` counts from zero up to the child count the hook was
/// handed; the engine writes the child's HTML into `out`. Returns `0`
/// when the child was delivered, negative when `index` is past the
/// last child, which is how the host stops.
pub type Geist4ChildFetch =
    unsafe extern "C" fn(ctx: *mut c_void, index: u32, out: *mut Geist4Span) -> i32;

// The decision tags. Constants rather than enums: C callers switch on
// them, and the header exposes them as defines.

/// `block-start`: open with the whole line consumed.
pub const GEIST4_START_WHOLE_LINE: i32 = 0;
/// `block-start`: seek past the content, open, keep the rest of the
/// line after `columns` columns as the block's first paragraph.
pub const GEIST4_START_COLUMNS: i32 = 1;
/// `block-start`: seek to the content, open, keep the whole rest of
/// the line as the block's first paragraph.
pub const GEIST4_START_REST: i32 = 2;
/// `block-start`: the task-list pattern — consume `columns` bytes,
/// claim the deepest plain container, open nothing.
pub const GEIST4_START_CLAIM: i32 = 3;

/// `block-continue`: consume `columns` more columns and keep going.
pub const GEIST4_CONTINUE_KEEP: i32 = 0;
/// `block-continue`: close the block and consume the line whole.
pub const GEIST4_CONTINUE_CLOSE_LINE: i32 = 1;

/// Inline: leave the bytes as text.
///
/// A C-surface value only: the Rust side declines by returning `None`.
#[allow(dead_code)]
pub const GEIST4_INLINE_LITERAL: i32 = 0;
/// Inline: tag the scanned run with the decision's kind.
pub const GEIST4_INLINE_DELIMITER: i32 = 1;
/// Inline: claim the bracket group's content as a node.
pub const GEIST4_INLINE_NODE: i32 = 2;

/// A hook outcome: no HTML, no decision, decline.
///
/// A C-surface value only: the Rust side declines by returning `None`.
#[allow(dead_code)]
pub const GEIST4_OUT_NONE: i32 = 0;
/// A hook outcome: the HTML in `html` applies.
pub const GEIST4_OUT_HTML: i32 = 1;

/// Attribute tag: `text` carries a string value.
pub const GEIST4_ATTR_STR: i32 = 0;
/// Attribute tag: `number` carries an integer value.
pub const GEIST4_ATTR_INT: i32 = 1;
/// Attribute tag: `number` carries a boolean value (zero or non-zero).
pub const GEIST4_ATTR_BOOL: i32 = 2;

// The hook function-pointer shapes, as type aliases for the contracts'
// documentation. The vtable spells the signatures out instead of using
// these aliases: cbindgen renders `Option<alias>` fields as opaque
// forward-declared structs, which do not compile, while an inline
// `Option<unsafe extern "C" fn(...)>` field renders as the nullable
// function pointer a C host expects.

/// `init`: called once per parse before any other hook. Negative
/// return rejects the parse — the extension latches and declines.
pub type Geist4HookInit = unsafe extern "C" fn(ctx: *mut c_void) -> i32;

/// `block-start-look`: whether the extension would start a block on
/// this line. Returns `1` for yes, `0` for no, negative for failure.
pub type Geist4HookLook = unsafe extern "C" fn(
    ctx: *mut c_void,
    line: *const u8,
    line_len: usize,
    indented: i32,
    column: u32,
) -> i32;

/// `block-start`: writes a `Geist4StartDecision` into `out` and
/// returns `1`, returns `0` to decline, or returns negative on
/// failure. `container_kind` is `None` for the document and plain
/// containers, `"paragraph"` for an open paragraph, the open block's
/// kind for an extension block — the same values tier 2 passes.
pub type Geist4HookStart = unsafe extern "C" fn(
    ctx: *mut c_void,
    line: *const u8,
    line_len: usize,
    indented: i32,
    column: u32,
    container_kind: *const u8,
    container_kind_len: usize,
    out: *mut Geist4StartDecision,
) -> i32;

/// `block-continue`: writes a `Geist4ContinueDecision` into `out`
/// and returns `1`, returns `0` to close, or returns negative on
/// failure.
pub type Geist4HookContinue = unsafe extern "C" fn(
    ctx: *mut c_void,
    line: *const u8,
    line_len: usize,
    indented: i32,
    column: u32,
    blank: i32,
    out: *mut Geist4ContinueDecision,
) -> i32;

/// `block-close`: writes a `Geist4Rendered` into `out` and returns
/// `1`, returns `0` to keep the node with its children rendered as a
/// plain container renders them, or returns negative on failure.
pub type Geist4HookClose = unsafe extern "C" fn(ctx: *mut c_void, out: *mut Geist4Rendered) -> i32;

/// `inline-trigger`: called for the registered trigger bytes only.
/// Writes a `Geist4InlineDecision` into `out` and returns `1`,
/// returns `0` to leave the byte as text, or returns negative on
/// failure. `pos` is the prefix's length: the line is the prefix
/// before the cursor joined with the rest of the line.
pub type Geist4HookTrigger = unsafe extern "C" fn(
    ctx: *mut c_void,
    line: *const u8,
    line_len: usize,
    pos: u32,
    out: *mut Geist4InlineDecision,
) -> i32;

/// `inline-bracket`: called for `[`…`]` groups that matched no link,
/// with the group's content as text. Writes a
/// `Geist4InlineDecision` into `out` and returns `1`, returns `0`
/// to leave the group as text, or returns negative on failure.
pub type Geist4HookBracket = unsafe extern "C" fn(
    ctx: *mut c_void,
    content: *const u8,
    content_len: usize,
    out: *mut Geist4InlineDecision,
) -> i32;

/// `post-process`: rewrites one text node's contents. Writes the
/// replacement into `out` and returns `1`, returns `0` to leave the
/// text unchanged, or returns negative on failure.
pub type Geist4HookPostProcess = unsafe extern "C" fn(
    ctx: *mut c_void,
    text: *const u8,
    text_len: usize,
    out: *mut Geist4Span,
) -> i32;

/// `render`: renders one extension node whole. `attrs` arrive with
/// their typed tags; the children's rendered HTML arrives
/// concatenated. Writes a `Geist4Rendered` into `out` and returns
/// `1`, returns `0` for the neutral result — the node renders
/// nothing — or returns negative on failure.
pub type Geist4HookRender = unsafe extern "C" fn(
    ctx: *mut c_void,
    kind: *const u8,
    kind_len: usize,
    attrs: *const Geist4Attr,
    attrs_len: usize,
    children_html: *const u8,
    children_html_len: usize,
    out: *mut Geist4Rendered,
) -> i32;

/// `render_override`: renders one claimed core node. The children
/// were pre-rendered with the built-in conventions before the call;
/// the hook pulls child `i`'s HTML by calling `fetch` with `i` and
/// `fetch_ctx`, in order, as many times as it wants. Writes a
/// `Geist4Rendered` into `out` and returns `1`, returns `0` to
/// leave the node's output empty — a claimed node renders only
/// through its override, so a decline renders nothing — or returns
/// negative on failure.
pub type Geist4HookRenderOverride = unsafe extern "C" fn(
    ctx: *mut c_void,
    kind: *const u8,
    kind_len: usize,
    attrs: *const Geist4Attr,
    attrs_len: usize,
    n_children: u32,
    fetch: Geist4ChildFetch,
    fetch_ctx: *mut c_void,
    out: *mut Geist4Rendered,
) -> i32;

/// The tier-3 vtable: the declared data plus one function pointer per
/// hook, every hook optional. A `None` hook declines.
///
/// The declared data is read and copied during
/// [`HostExtension::from_vtable`]; the host may free it afterwards.
/// The context pointer and the function pointers must stay valid for
/// as long as the extension does.
#[repr(C)]
pub struct Geist4HostVTable {
    /// The host's context, the first argument of every hook.
    pub ctx: *mut c_void,
    /// Called with the context when the last engine-side reference to
    /// the extension drops. Optional.
    pub free_ctx: Option<unsafe extern "C" fn(ctx: *mut c_void)>,
    /// The extension's name.
    pub name: Geist4Span,
    /// The declared kinds.
    pub kinds: *const Geist4KindDecl,
    /// The number of declared kinds.
    pub kinds_len: usize,
    /// The inline trigger bytes to register.
    pub triggers: *const u8,
    /// The number of trigger bytes.
    pub triggers_len: usize,
    /// The `init` hook, or null. See [`Geist4HookInit`] for the
    /// contract.
    pub init: Option<unsafe extern "C" fn(ctx: *mut c_void) -> i32>,
    /// The `block-start-look` hook, or null. See [`Geist4HookLook`]
    /// for the contract.
    pub block_start_look: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            line: *const u8,
            line_len: usize,
            indented: i32,
            column: u32,
        ) -> i32,
    >,
    /// The `block-start` hook, or null. See [`Geist4HookStart`] for
    /// the contract.
    pub block_start: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            line: *const u8,
            line_len: usize,
            indented: i32,
            column: u32,
            container_kind: *const u8,
            container_kind_len: usize,
            out: *mut Geist4StartDecision,
        ) -> i32,
    >,
    /// The `block-continue` hook, or null. See [`Geist4HookContinue`]
    /// for the contract.
    pub block_continue: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            line: *const u8,
            line_len: usize,
            indented: i32,
            column: u32,
            blank: i32,
            out: *mut Geist4ContinueDecision,
        ) -> i32,
    >,
    /// The `block-close` hook, or null. See [`Geist4HookClose`] for
    /// the contract.
    pub block_close:
        Option<unsafe extern "C" fn(ctx: *mut c_void, out: *mut Geist4Rendered) -> i32>,
    /// The `inline-trigger` hook, or null. See [`Geist4HookTrigger`]
    /// for the contract.
    pub inline_trigger: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            line: *const u8,
            line_len: usize,
            pos: u32,
            out: *mut Geist4InlineDecision,
        ) -> i32,
    >,
    /// The `inline-bracket` hook, or null. See [`Geist4HookBracket`]
    /// for the contract.
    pub inline_bracket: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            content: *const u8,
            content_len: usize,
            out: *mut Geist4InlineDecision,
        ) -> i32,
    >,
    /// The `post-process` hook, or null. See [`Geist4HookPostProcess`]
    /// for the contract.
    pub post_process: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            text: *const u8,
            text_len: usize,
            out: *mut Geist4Span,
        ) -> i32,
    >,
    /// The `render` hook, or null. See [`Geist4HookRender`] for the
    /// contract.
    pub render: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            kind: *const u8,
            kind_len: usize,
            attrs: *const Geist4Attr,
            attrs_len: usize,
            children_html: *const u8,
            children_html_len: usize,
            out: *mut Geist4Rendered,
        ) -> i32,
    >,
    /// The `render_override` hook, or null. See
    /// [`Geist4HookRenderOverride`] for the contract.
    pub render_override: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            kind: *const u8,
            kind_len: usize,
            attrs: *const Geist4Attr,
            attrs_len: usize,
            n_children: u32,
            fetch: Geist4ChildFetch,
            fetch_ctx: *mut c_void,
            out: *mut Geist4Rendered,
        ) -> i32,
    >,
}

// The `Geist4Hook*` aliases above are the documented shapes of the
// vtable's hooks, spelled out inline in the fields because cbindgen
// cannot render an `Option<alias>` field. These conversions fail to
// compile if a field ever drifts from its alias.
const _: fn(&Geist4HostVTable) -> Option<Geist4HookInit> = |v| v.init;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookLook> = |v| v.block_start_look;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookStart> = |v| v.block_start;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookContinue> = |v| v.block_continue;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookClose> = |v| v.block_close;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookTrigger> = |v| v.inline_trigger;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookBracket> = |v| v.inline_bracket;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookPostProcess> = |v| v.post_process;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookRender> = |v| v.render;
const _: fn(&Geist4HostVTable) -> Option<Geist4HookRenderOverride> = |v| v.render_override;

// ---------------------------------------------------------------------------
// The safe surface: boxed closures.
// ---------------------------------------------------------------------------

/// A `block-start` decision in the closure surface.
#[derive(Debug, Clone, PartialEq)]
pub struct StartDecision {
    /// What the engine does with the line.
    pub consume: StartConsume,
    /// The extension kind to open; ignored for claims.
    pub kind: String,
    /// The attributes to set on the opened or claimed node.
    pub attrs: Vec<(String, AttrValue)>,
}

/// What a `block-start` decision does with the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartConsume {
    /// Open with the whole line consumed.
    WholeLine,
    /// Seek `n` columns past the content, open, keep the rest of the
    /// line as the block's first paragraph.
    Columns(u32),
    /// Seek to the content, open, keep the whole rest of the line as
    /// the block's first paragraph.
    Rest,
    /// The task-list pattern: consume `n` bytes, claim the deepest
    /// plain container, open nothing.
    Claim(u32),
}

/// A `block-continue` decision in the closure surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinueDecision {
    /// Consume the line as the consume names and keep the block open.
    Keep(ContinueConsume),
    /// Close the block and consume the line whole.
    CloseLine,
}

/// What a keep decision consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinueConsume {
    /// Every remaining column.
    WholeLine,
    /// `n` columns from the position the container prefixes left.
    Columns(u32),
    /// Nothing: the whole line becomes the block's content.
    Rest,
}

/// An `inline-bracket` claim in the closure surface: claim the
/// bracket group's content as an inline node of `kind`, keeping up to
/// `end` bytes of it as the node's children.
#[derive(Debug, Clone, PartialEq)]
pub struct BracketClaim {
    /// The extension kind to claim with.
    pub kind: String,
    /// The attributes to set on the claimed node.
    pub attrs: Vec<(String, AttrValue)>,
    /// The content length to keep; the whole group when it reaches
    /// past the content.
    pub end: u32,
}

/// The `init` hook: called once per parse before any other hook.
pub type InitHook = Box<dyn Fn() -> Result<(), String> + Send + Sync>;

/// The `block-start-look` hook: whether the extension would start a
/// block on this line.
pub type LookHook = Box<dyn Fn(&str, bool, u32) -> Result<bool, String> + Send + Sync>;

/// The `block-start` hook: a decision, or `None` to decline.
pub type StartHook = Box<
    dyn Fn(&str, bool, u32, Option<&str>) -> Result<Option<StartDecision>, String> + Send + Sync,
>;

/// The `block-continue` hook: a decision, or `None` to close.
pub type ContinueHook =
    Box<dyn Fn(&str, bool, u32, bool) -> Result<Option<ContinueDecision>, String> + Send + Sync>;

/// The `block-close` hook: the block's HTML, or `None` to keep the
/// node with its children rendered as a plain container renders them.
pub type CloseHook = Box<dyn Fn() -> Result<Option<String>, String> + Send + Sync>;

/// The `inline-trigger` hook: the kind to tag the run at the cursor
/// with, or `None` to leave the byte as text.
pub type TriggerHook = Box<dyn Fn(&str, u32) -> Result<Option<String>, String> + Send + Sync>;

/// The `inline-bracket` hook: a claim, or `None` to leave the group
/// as text.
pub type BracketHook = Box<dyn Fn(&str) -> Result<Option<BracketClaim>, String> + Send + Sync>;

/// The `post-process` hook: the replacement for one text node's
/// contents, or `None` to leave it unchanged.
pub type PostHook = Box<dyn Fn(&str) -> Result<Option<String>, String> + Send + Sync>;

/// The `render` hook: the extension node's whole output, or `None`
/// for the neutral result — the node renders nothing.
pub type RenderNodeHook =
    Box<dyn Fn(&str, &[(String, String)], &str) -> Result<Option<String>, String> + Send + Sync>;

/// The `render-override` hook: renders one claimed core node. The
/// children's HTML arrives pre-rendered, one string per child; the
/// callback writes the node's whole output and returns whether it
/// wrote — a `false` return leaves the node's output empty, and a
/// claimed node renders only through its override.
pub type OverrideHook =
    Box<dyn Fn(&CoreNode<'_>, &[String], &mut String) -> Result<bool, String> + Send + Sync>;

/// The closure surface: one boxed callback per hook, every hook
/// optional. A `None` hook declines.
///
/// The callbacks see the same values the C hooks see, in the shapes
/// the language bindings find natural. Callback failures travel
/// through `Result` where the C surface uses negative returns, and a
/// panic in any callback latches, like a failed call.
pub struct HostHooks {
    /// `init`: called once per parse before any other hook. An error
    /// latches and the extension declines for the parse.
    pub init: Option<InitHook>,
    /// `block-start-look`: whether the extension would start a block
    /// on this line.
    pub block_start_look: Option<LookHook>,
    /// `block-start`: a decision, or `None` to decline.
    pub block_start: Option<StartHook>,
    /// `block-continue`: a decision, or `None` to close.
    pub block_continue: Option<ContinueHook>,
    /// `block-close`: the block's HTML, or `None` to keep the node
    /// with its children rendered as a plain container renders them.
    pub block_close: Option<CloseHook>,
    /// `inline-trigger`: the kind to tag the run at the cursor with,
    /// or `None` to leave the byte as text.
    pub inline_trigger: Option<TriggerHook>,
    /// `inline-bracket`: a claim, or `None` to leave the group as
    /// text.
    pub inline_bracket: Option<BracketHook>,
    /// `post-process`: the replacement for one text node's contents,
    /// or `None` to leave it unchanged.
    pub post_process: Option<PostHook>,
    /// `render`: the extension node's whole output, or `None` for the
    /// neutral result — the node renders nothing.
    pub render: Option<RenderNodeHook>,
    /// `render_override`: renders one claimed core node.
    pub render_override: Option<OverrideHook>,
}

// ---------------------------------------------------------------------------
// Construction and validation.
// ---------------------------------------------------------------------------

impl Default for HostHooks {
    /// Every hook declined: an extension that does nothing.
    fn default() -> Self {
        Self {
            init: None,
            block_start_look: None,
            block_start: None,
            block_continue: None,
            block_close: None,
            inline_trigger: None,
            inline_bracket: None,
            post_process: None,
            render: None,
            render_override: None,
        }
    }
}

/// Why a vtable or a hook set was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The extension's name is not a valid extension name.
    Name {
        /// What was wrong.
        reason: String,
    },
    /// The extension's name is reserved for a built-in.
    Reserved {
        /// The rejected name.
        name: String,
    },
    /// A declared kind is not valid.
    Kind {
        /// The kind's name, when it was readable.
        name: String,
        /// What was wrong.
        reason: String,
    },
    /// A declared attribute name is not valid.
    Attr {
        /// The kind the attribute belongs to.
        kind: String,
        /// The rejected attribute name, when it was readable.
        name: String,
        /// What was wrong.
        reason: String,
    },
    /// The trigger bytes are not valid.
    Triggers {
        /// What was wrong.
        reason: String,
    },
    /// The extension's name is already registered.
    Duplicate {
        /// The rejected name.
        name: String,
    },
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::Name { reason } => write!(f, "host extension: {reason}"),
            HostError::Reserved { name } => {
                write!(
                    f,
                    "host extension: the name \"{name}\" is reserved for a built-in extension"
                )
            }
            HostError::Kind { name, reason } => {
                write!(f, "host extension kind \"{name}\": {reason}")
            }
            HostError::Attr { kind, name, reason } => {
                write!(
                    f,
                    "host extension kind \"{kind}\" attribute \"{name}\": {reason}"
                )
            }
            HostError::Triggers { reason } => write!(f, "host extension triggers: {reason}"),
            HostError::Duplicate { name } => write!(
                f,
                "host extension: the name \"{name}\" is already registered"
            ),
        }
    }
}

impl std::error::Error for HostError {}

/// The greatest name length the manifest rules allow, in bytes.
const MAX_NAME_BYTES: usize = 64;

/// The extension names reserved for the core built-ins, the same list
/// the loader's manifest rules reserve.
const RESERVED_NAMES: [&str; 9] = [
    "tables",
    "strikethrough",
    "autolink",
    "tasklist",
    "tagfilter",
    "footnotes",
    "front_matter",
    "heading_identifiers",
    "description_lists",
];

/// Whether `name` is a valid extension or kind name: 1 to 64 bytes,
/// matching `[a-z][a-z0-9-]*` — the manifest's rule, applied without
/// regular expressions.
fn is_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_NAME_BYTES
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// Whether `name` is a valid attribute name: matching
/// `[a-z][a-z0-9_-]*` — the manifest's rule.
fn is_attr_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_NAME_BYTES
        && bytes[0].is_ascii_lowercase()
        && bytes[1..].iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
        })
}

/// Whether `byte` is one of the core's own inline trigger bytes,
/// which an extension can never claim.
fn is_core_trigger_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'\\' | b'`' | b'&' | b'<' | b'*' | b'_' | b'[' | b'!' | b']'
    )
}

/// A tier-3 host extension: the declared data plus callbacks, behind
/// the core [`Extension`] trait.
///
/// Built with [`HostExtension::from_vtable`] (the C surface) or
/// [`HostExtension::from_hooks`] (the closure surface), then handed to
/// the safe module's `Options::enable_host` — or to the C ABI's
/// `geist4_options_enable_host`, which does both steps.
pub struct HostExtension {
    /// The extension's name, copied at construction.
    name: String,
    /// The declared kinds, copied at construction.
    kinds: Vec<KindDecl>,
    /// The inline trigger bytes, copied at construction.
    triggers: Vec<u8>,
    /// The callbacks.
    hooks: HostHooks,
    /// The context pointer and its free function, for the C surface's
    /// lifetime contract.
    cleanup: Option<(unsafe extern "C" fn(ctx: *mut c_void), *mut c_void)>,
    /// The latched error, if any: shared with the C-adapted callbacks
    /// so a negative return latches the same way a panic does.
    error: Arc<Mutex<Option<String>>>,
}

// The contract this type exists for: the host's context pointer is
// valid for every call and the host keeps its own state thread-safe,
// so the extension may travel behind the parser's `Arc`s.
unsafe impl Send for HostExtension {}
unsafe impl Sync for HostExtension {}

impl Drop for HostExtension {
    fn drop(&mut self) {
        if let Some((free_ctx, ctx)) = self.cleanup {
            // SAFETY: the host contract — the free function stays
            // valid for the extension's lifetime and accepts the
            // context it was registered with.
            unsafe { free_ctx(ctx) };
        }
    }
}

impl std::fmt::Debug for HostExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostExtension")
            .field("name", &self.name)
            .field("kinds", &self.kinds.len())
            .field("triggers", &self.triggers.len())
            .finish_non_exhaustive()
    }
}

/// A context pointer wrapped so a closure over it is `Send` and
/// `Sync`. The host contract guarantees both: the context is valid
/// for the duration of every call and the host keeps its own state
/// thread-safe.
/// The context pointer the hooks carry. A raw pointer is a value the
/// closures can copy freely; the vtable's owner keeps it valid.
#[derive(Clone, Copy)]
struct HostCtx(*mut c_void);

unsafe impl Send for HostCtx {}
unsafe impl Sync for HostCtx {}

impl HostCtx {
    /// The context pointer, read through the wrapper so a closure
    /// captures the whole `HostCtx` value — whose `Send`/`Sync`
    /// impls carry the host contract — instead of the bare pointer
    /// field, which the compiler's capture analysis would otherwise
    /// grab directly and reject.
    fn ptr(self) -> *mut c_void {
        self.0
    }
}

impl HostExtension {
    /// Builds a tier-3 extension from a `#[repr(C)]` vtable: the
    /// surface Swift reaches through the C ABI.
    ///
    /// The declared data is validated the same way the loader
    /// validates a tier-2 manifest and copied, so the host may free
    /// its registration arrays when this returns. The context pointer
    /// and the function pointers are kept; `Geist4HostVTable::free_ctx`
    /// runs when the extension drops.
    ///
    /// # Errors
    /// [`HostError`] when the declared data breaks the manifest's
    /// rules — a bad name, a reserved name, a bad kind or attribute
    /// name, or bad trigger bytes.
    ///
    /// # Safety
    /// The vtable's spans, kind array and trigger bytes must be valid
    /// reads for their lengths for the duration of this call; the
    /// function pointers and the context pointer must stay valid for
    /// the extension's lifetime. The out-arguments the hooks write
    /// must be valid for their types when the hook returns `1`.
    pub unsafe fn from_vtable(vtable: &Geist4HostVTable) -> Result<Self, HostError> {
        let name = unsafe { read_str(&vtable.name) }
            .map_err(|reason| HostError::Name { reason })?
            .to_owned();
        let mut kinds = Vec::new();
        if !vtable.kinds.is_null() && vtable.kinds_len > 0 {
            // SAFETY: the caller's contract — `kinds_len` entries of
            // `Geist4KindDecl` are valid reads.
            let entries = unsafe { std::slice::from_raw_parts(vtable.kinds, vtable.kinds_len) };
            for entry in entries {
                let kind = unsafe { read_str(&entry.kind) }
                    .map_err(|reason| HostError::Kind {
                        name: String::new(),
                        reason,
                    })?
                    .to_owned();
                if !is_name(&kind) {
                    return Err(HostError::Kind {
                        name: kind,
                        reason: "must match [a-z][a-z0-9-]*".to_string(),
                    });
                }
                if kinds
                    .iter()
                    .any(|declared: &KindDecl| declared.kind == kind)
                {
                    return Err(HostError::Kind {
                        name: kind.clone(),
                        reason: "declared twice".to_string(),
                    });
                }
                let mut attrs = Vec::new();
                if !entry.attrs.is_null() && entry.attrs_len > 0 {
                    // SAFETY: the caller's contract — `attrs_len`
                    // spans are valid reads.
                    let names = unsafe { std::slice::from_raw_parts(entry.attrs, entry.attrs_len) };
                    for span in names {
                        let attr = unsafe { read_str(span) }.map_err(|reason| HostError::Attr {
                            kind: kind.clone(),
                            name: String::new(),
                            reason,
                        })?;
                        if !is_attr_name(attr) {
                            return Err(HostError::Attr {
                                kind: kind.clone(),
                                name: attr.to_owned(),
                                reason: "must match [a-z][a-z0-9_-]*".to_string(),
                            });
                        }
                        if attrs.iter().any(|declared: &String| declared == attr) {
                            return Err(HostError::Attr {
                                kind: kind.clone(),
                                name: attr.to_owned(),
                                reason: "declared twice".to_string(),
                            });
                        }
                        attrs.push(attr.to_owned());
                    }
                }
                kinds.push(KindDecl {
                    kind,
                    container: entry.container != 0,
                    inline_text: entry.inline_text != 0,
                    attrs,
                });
            }
        }
        let mut triggers = Vec::new();
        if !vtable.triggers.is_null() && vtable.triggers_len > 0 {
            if vtable.triggers_len > 32 {
                return Err(HostError::Triggers {
                    reason: "at most 32 trigger bytes".to_string(),
                });
            }
            // SAFETY: the caller's contract — `triggers_len` bytes are
            // valid reads.
            let bytes = unsafe { std::slice::from_raw_parts(vtable.triggers, vtable.triggers_len) };
            for &byte in bytes {
                if !byte.is_ascii_graphic() {
                    return Err(HostError::Triggers {
                        reason: "each trigger must be one printable ASCII byte".to_string(),
                    });
                }
                if is_core_trigger_byte(byte) {
                    return Err(HostError::Triggers {
                        reason: "the core's own trigger bytes never reach the hook".to_string(),
                    });
                }
                if triggers.contains(&byte) {
                    return Err(HostError::Triggers {
                        reason: "each trigger byte is declared once".to_string(),
                    });
                }
                triggers.push(byte);
            }
        }
        let error = Arc::new(Mutex::new(None::<String>));
        // SAFETY: the caller's vtable contract — the spans, arrays and
        // function pointers are valid reads for the duration of this
        // call, and the pointers stay valid for the extension's
        // lifetime.
        let hooks = unsafe { hooks_from_vtable(vtable, &error) };
        let cleanup = vtable.free_ctx.map(|free_ctx| (free_ctx, vtable.ctx));
        Self::build(name, kinds, triggers, hooks, cleanup, error)
    }

    /// Builds a tier-3 extension from boxed callbacks: the surface the
    /// Python and wasm bindings use, with no `unsafe` on their side.
    ///
    /// The declared data is validated the same way
    /// [`HostExtension::from_vtable`] validates it.
    ///
    /// # Errors
    /// [`HostError`] when the declared data breaks the manifest's
    /// rules.
    pub fn from_hooks(
        name: String,
        kinds: Vec<KindDecl>,
        triggers: Vec<u8>,
        hooks: HostHooks,
    ) -> Result<Self, HostError> {
        Self::build(
            name,
            kinds,
            triggers,
            hooks,
            None,
            Arc::new(Mutex::new(None::<String>)),
        )
    }

    /// The shared constructor: validates the declared data and builds
    /// the extension.
    fn build(
        name: String,
        kinds: Vec<KindDecl>,
        triggers: Vec<u8>,
        hooks: HostHooks,
        cleanup: Option<(unsafe extern "C" fn(ctx: *mut c_void), *mut c_void)>,
        error: Arc<Mutex<Option<String>>>,
    ) -> Result<Self, HostError> {
        if !is_name(&name) {
            return Err(HostError::Name {
                reason: "the name must match [a-z][a-z0-9-]* and be 1-64 bytes".to_string(),
            });
        }
        if RESERVED_NAMES.contains(&name.as_str()) {
            return Err(HostError::Reserved { name });
        }
        for declared in &kinds {
            if !is_name(&declared.kind) {
                return Err(HostError::Kind {
                    name: declared.kind.clone(),
                    reason: "must match [a-z][a-z0-9-]*".to_string(),
                });
            }
            for attr in &declared.attrs {
                if !is_attr_name(attr) {
                    return Err(HostError::Attr {
                        kind: declared.kind.clone(),
                        name: attr.clone(),
                        reason: "must match [a-z][a-z0-9_-]*".to_string(),
                    });
                }
            }
        }
        if triggers.len() > 32 {
            return Err(HostError::Triggers {
                reason: "at most 32 trigger bytes".to_string(),
            });
        }
        for &byte in &triggers {
            if !byte.is_ascii_graphic() {
                return Err(HostError::Triggers {
                    reason: "each trigger must be one printable ASCII byte".to_string(),
                });
            }
            if is_core_trigger_byte(byte) {
                return Err(HostError::Triggers {
                    reason: "the core's own trigger bytes never reach the hook".to_string(),
                });
            }
        }
        let mut seen = Vec::new();
        for &byte in &triggers {
            if seen.contains(&byte) {
                return Err(HostError::Triggers {
                    reason: "each trigger byte is declared once".to_string(),
                });
            }
            seen.push(byte);
        }
        Ok(Self {
            name,
            kinds,
            triggers,
            hooks,
            cleanup,
            error,
        })
    }

    /// Whether an error is latched.
    fn latched(&self) -> bool {
        self.error
            .lock()
            .expect("the host runtime mutex stays sound")
            .is_some()
    }

    /// Latches the first error: later failures do not overwrite it.
    fn latch(&self, message: String) {
        let mut slot = self
            .error
            .lock()
            .expect("the host runtime mutex stays sound");
        slot.get_or_insert(message);
    }

    /// Calls a hook with panic handling: a panic latches with the
    /// panic's message and reports as declined. The `run` closure
    /// returns `Option<T>`: `None` for declined or failed, `Some` for
    /// an answer.
    fn call<T>(&self, run: impl FnOnce() -> Option<T>) -> Option<T> {
        if self.latched() {
            return None;
        }
        match catch_unwind(AssertUnwindSafe(run)) {
            Ok(answer) => answer,
            Err(panic) => {
                self.latch(super::panic_message(&panic));
                None
            }
        }
    }

    /// Calls a `Result`-returning hook the way the C surface calls a
    /// hook: an error is a failed call — it latches the message and
    /// the hook declines — and so does a panic.
    fn call_result<T>(&self, run: impl FnOnce() -> Result<Option<T>, String>) -> Option<T> {
        match self.call(|| Some(run())) {
            Some(Ok(value)) => value,
            Some(Err(message)) => {
                self.latch(message);
                None
            }
            None => None,
        }
    }

    /// Whether the vtable declared `kind`. A hook may only answer with
    /// kinds its extension declared, so a foreign name cannot route
    /// another extension's renderer against content the hook chose.
    fn owns_kind(&self, kind: &str) -> bool {
        self.kinds.iter().any(|declared| declared.kind == kind)
    }
}

/// Reads a vtable span as text: an empty span is an empty string, a
/// `len` past a null pointer or invalid UTF-8 is malformed.
///
/// # Safety
/// The span's bytes must be valid reads for `len` — the caller's
/// contract for every span it registers.
unsafe fn read_str<'a>(span: &Geist4Span) -> Result<&'a str, String> {
    if span.len == 0 {
        return Ok("");
    }
    if span.data.is_null() {
        return Err("a byte span with a length must carry a pointer".to_string());
    }
    // SAFETY: the caller's contract — `len` bytes are valid reads.
    let bytes = unsafe { std::slice::from_raw_parts(span.data, span.len) };
    std::str::from_utf8(bytes).map_err(|_| "a byte span must hold valid UTF-8".to_string())
}

/// Builds the closure surface from a vtable: each present hook becomes
/// a closure over the context pointer and the function pointer, and a
/// negative return latches through the shared error slot the closures
/// carry. The closures own a [`HostCtx`] so the raw pointer's
/// `Send`/`Sync` guarantee comes from the host contract, stated once
/// here rather than re-argued at every hook.
///
/// The function is one flat table on purpose: each closure mirrors one
/// vtable field in order, so the reader checks each hook against its C
/// contract without following helper indirection. Splitting it would
/// hide that correspondence.
#[allow(clippy::too_many_lines)]
unsafe fn hooks_from_vtable(
    vtable: &Geist4HostVTable,
    error: &Arc<Mutex<Option<String>>>,
) -> HostHooks {
    let ctx = HostCtx(vtable.ctx);
    let error = Arc::clone(error);
    HostHooks {
        init: vtable.init.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(move || {
                // SAFETY: the host contract — the hook is valid and
                // accepts the context it was registered with.
                let status = unsafe { hook(ctx.ptr()) };
                if status < 0 {
                    latch_shared(&error, format!("the init hook failed with status {status}"));
                    Err(format!("the init hook failed with status {status}"))
                } else {
                    Ok(())
                }
            }) as Box<dyn Fn() -> Result<(), String> + Send + Sync>
        }),
        block_start_look: vtable.block_start_look.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(move |line: &str, indented: bool, column: u32| {
                // SAFETY: the host contract, as above; the line's
                // bytes are borrowed for the call.
                let status = unsafe {
                    hook(
                        ctx.ptr(),
                        line.as_ptr(),
                        line.len(),
                        i32::from(indented),
                        column,
                    )
                };
                if status < 0 {
                    return Err(latch_shared(
                        &error,
                        format!("the block-start-look hook failed with status {status}"),
                    ));
                }
                Ok(status > 0)
            }) as Box<dyn Fn(&str, bool, u32) -> Result<bool, String> + Send + Sync>
        }),
        block_start: vtable.block_start.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(
                move |line: &str,
                      indented: bool,
                      column: u32,
                      container: Option<&str>|
                      -> Result<Option<StartDecision>, String> {
                    let container = container.unwrap_or("");
                    let mut slot = std::mem::MaybeUninit::<Geist4StartDecision>::uninit();
                    // SAFETY: the host contract; when the hook returns
                    // 1 it has written a valid decision.
                    let status = unsafe {
                        hook(
                            ctx.ptr(),
                            line.as_ptr(),
                            line.len(),
                            i32::from(indented),
                            column,
                            container.as_ptr(),
                            container.len(),
                            slot.as_mut_ptr(),
                        )
                    };
                    if status < 0 {
                        return Err(latch_shared(
                            &error,
                            format!("the block-start hook failed with status {status}"),
                        ));
                    }
                    if status == 0 {
                        return Ok(None);
                    }
                    // SAFETY: the contract — the hook returned 1, so
                    // it wrote the decision.
                    let decision = unsafe { slot.assume_init() };
                    match unsafe { start_from_c(&decision, &error) } {
                        Some(started) => Ok(Some(started)),
                        None => {
                            Err("the block-start hook returned a malformed decision".to_string())
                        }
                    }
                },
            )
                as Box<
                    dyn Fn(&str, bool, u32, Option<&str>) -> Result<Option<StartDecision>, String>
                        + Send
                        + Sync,
                >
        }),
        block_continue: vtable.block_continue.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(
                move |line: &str,
                      indented: bool,
                      column: u32,
                      blank: bool|
                      -> Result<Option<ContinueDecision>, String> {
                    let mut slot = std::mem::MaybeUninit::<Geist4ContinueDecision>::uninit();
                    // SAFETY: the host contract; a return of 1 means
                    // the hook wrote the decision.
                    let status = unsafe {
                        hook(
                            ctx.ptr(),
                            line.as_ptr(),
                            line.len(),
                            i32::from(indented),
                            column,
                            i32::from(blank),
                            slot.as_mut_ptr(),
                        )
                    };
                    if status < 0 {
                        return Err(latch_shared(
                            &error,
                            format!("the block-continue hook failed with status {status}"),
                        ));
                    }
                    if status == 0 {
                        return Ok(None);
                    }
                    // SAFETY: the contract — the hook returned 1, so
                    // it wrote the decision.
                    let decision = unsafe { slot.assume_init() };
                    match unsafe { continue_from_c(&decision, &error) } {
                        Some(continued) => Ok(Some(continued)),
                        None => {
                            Err("the block-continue hook returned a malformed decision".to_string())
                        }
                    }
                },
            )
                as Box<
                    dyn Fn(&str, bool, u32, bool) -> Result<Option<ContinueDecision>, String>
                        + Send
                        + Sync,
                >
        }),
        block_close: vtable.block_close.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(move || -> Result<Option<String>, String> {
                let mut slot = std::mem::MaybeUninit::<Geist4Rendered>::uninit();
                // SAFETY: the host contract; a return of 1 means the
                // hook wrote the rendering.
                let status = unsafe { hook(ctx.ptr(), slot.as_mut_ptr()) };
                if status < 0 {
                    return Err(latch_shared(
                        &error,
                        format!("the block-close hook failed with status {status}"),
                    ));
                }
                if status == 0 {
                    return Ok(None);
                }
                // SAFETY: the contract — the hook returned 1, so it
                // wrote the rendering.
                match unsafe { rendered_from_c(&slot.assume_init(), &error) } {
                    Some(html) => Ok(Some(html)),
                    None => Err("the block-close hook returned a malformed rendering".to_string()),
                }
            }) as Box<dyn Fn() -> Result<Option<String>, String> + Send + Sync>
        }),
        inline_trigger: vtable.inline_trigger.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(
                move |line: &str, pos: u32| -> Result<Option<String>, String> {
                    let mut slot = std::mem::MaybeUninit::<Geist4InlineDecision>::uninit();
                    // SAFETY: the host contract; a return of 1 means
                    // the hook wrote the decision.
                    let status = unsafe {
                        hook(ctx.ptr(), line.as_ptr(), line.len(), pos, slot.as_mut_ptr())
                    };
                    if status < 0 {
                        return Err(latch_shared(
                            &error,
                            format!("the inline-trigger hook failed with status {status}"),
                        ));
                    }
                    if status == 0 {
                        return Ok(None);
                    }
                    // SAFETY: the contract — the hook returned 1, so
                    // it wrote the decision.
                    let decision = unsafe { slot.assume_init() };
                    match decision.tag {
                        GEIST4_INLINE_DELIMITER => match unsafe { read_str(&decision.kind) } {
                            Ok(kind) => Ok(Some(kind.to_owned())),
                            Err(reason) => {
                                Err(latch_shared(&error, format!("inline-trigger: {reason}")))
                            }
                        },
                        // The trigger hook has no claim API; a node
                        // decision stays literal, the way tier 2
                        // declines it.
                        _ => Ok(None),
                    }
                },
            ) as Box<dyn Fn(&str, u32) -> Result<Option<String>, String> + Send + Sync>
        }),
        inline_bracket: vtable.inline_bracket.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(
                move |content: &str| -> Result<Option<BracketClaim>, String> {
                    let mut slot = std::mem::MaybeUninit::<Geist4InlineDecision>::uninit();
                    // SAFETY: the host contract; a return of 1 means the
                    // hook wrote the decision.
                    let status = unsafe {
                        hook(
                            ctx.ptr(),
                            content.as_ptr(),
                            content.len(),
                            slot.as_mut_ptr(),
                        )
                    };
                    if status < 0 {
                        return Err(latch_shared(
                            &error,
                            format!("the inline-bracket hook failed with status {status}"),
                        ));
                    }
                    if status == 0 {
                        return Ok(None);
                    }
                    // SAFETY: the contract — the hook returned 1, so it
                    // wrote the decision.
                    let decision = unsafe { slot.assume_init() };
                    // A None from the conversion is either a legitimate
                    // literal decline or a malformed decision — the
                    // conversion latched the malformed case, so both
                    // surfaces decline the same way.
                    Ok(unsafe { bracket_from_c(&decision, &error) })
                },
            ) as Box<dyn Fn(&str) -> Result<Option<BracketClaim>, String> + Send + Sync>
        }),
        post_process: vtable.post_process.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(move |text: &str| -> Result<Option<String>, String> {
                let mut slot = std::mem::MaybeUninit::<Geist4Span>::uninit();
                // SAFETY: the host contract; a return of 1 means the
                // hook wrote the replacement.
                let status =
                    unsafe { hook(ctx.ptr(), text.as_ptr(), text.len(), slot.as_mut_ptr()) };
                if status < 0 {
                    return Err(latch_shared(
                        &error,
                        format!("the post-process hook failed with status {status}"),
                    ));
                }
                if status == 0 {
                    return Ok(None);
                }
                // SAFETY: the contract — the hook returned 1, so it
                // wrote the span, whose bytes are read before this
                // call returns.
                let span = unsafe { slot.assume_init() };
                match unsafe { read_str(&span) } {
                    Ok(rewritten) => Ok(Some(rewritten.to_owned())),
                    Err(reason) => Err(latch_shared(&error, format!("post-process: {reason}"))),
                }
            }) as Box<dyn Fn(&str) -> Result<Option<String>, String> + Send + Sync>
        }),
        render: vtable.render.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(
                move |kind: &str,
                      attrs: &[(String, String)],
                      children: &str|
                      -> Result<Option<String>, String> {
                    let c_attrs = attrs_to_c(attrs);
                    let mut slot = std::mem::MaybeUninit::<Geist4Rendered>::uninit();
                    // SAFETY: the host contract; a return of 1 means
                    // the hook wrote the rendering.
                    let status = unsafe {
                        hook(
                            ctx.ptr(),
                            kind.as_ptr(),
                            kind.len(),
                            c_attrs.as_ptr(),
                            c_attrs.len(),
                            children.as_ptr(),
                            children.len(),
                            slot.as_mut_ptr(),
                        )
                    };
                    if status < 0 {
                        return Err(latch_shared(
                            &error,
                            format!("the render hook failed with status {status}"),
                        ));
                    }
                    if status == 0 {
                        return Ok(None);
                    }
                    // SAFETY: the contract — the hook returned 1, so
                    // it wrote the rendering.
                    match unsafe { rendered_from_c(&slot.assume_init(), &error) } {
                        Some(html) => Ok(Some(html)),
                        None => Err("the render hook returned a malformed rendering".to_string()),
                    }
                },
            )
                as Box<
                    dyn Fn(&str, &[(String, String)], &str) -> Result<Option<String>, String>
                        + Send
                        + Sync,
                >
        }),
        render_override: vtable.render_override.map(|hook| {
            let error = Arc::clone(&error);
            Box::new(
                move |node: &CoreNode,
                      children: &[String],
                      out: &mut String|
                      -> Result<bool, String> {
                    let c_attrs = typed_attrs_to_c(node.attrs);
                    let mut fetch_ctx = children.to_vec();
                    let mut slot = std::mem::MaybeUninit::<Geist4Rendered>::uninit();
                    // SAFETY: the fetch closure reads the vector the
                    // box owns, which stays alive for the call.
                    let status = unsafe {
                        hook(
                            ctx.ptr(),
                            node.kind.as_ptr(),
                            node.kind.len(),
                            c_attrs.as_ptr(),
                            c_attrs.len(),
                            u32::try_from(fetch_ctx.len()).unwrap_or(u32::MAX),
                            fetch_children,
                            (&raw mut fetch_ctx).cast::<c_void>(),
                            slot.as_mut_ptr(),
                        )
                    };
                    if status < 0 {
                        return Err(latch_shared(
                            &error,
                            format!("the render-override hook failed with status {status}"),
                        ));
                    }
                    if status == 0 {
                        // A decline renders nothing: a claimed node
                        // renders only through its override.
                        return Ok(false);
                    }
                    // SAFETY: the contract — the hook returned 1, so
                    // it wrote the rendering.
                    match unsafe { rendered_from_c(&slot.assume_init(), &error) } {
                        Some(html) => {
                            out.push_str(&html);
                            Ok(true)
                        }
                        None => {
                            Err("the render-override hook returned a malformed rendering"
                                .to_string())
                        }
                    }
                },
            )
                as Box<
                    dyn Fn(&CoreNode, &[String], &mut String) -> Result<bool, String> + Send + Sync,
                >
        }),
    }
}

// ---------------------------------------------------------------------------
// The trait implementation.
// ---------------------------------------------------------------------------

impl Extension for HostExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn inline_triggers(&self) -> &[u8] {
        &self.triggers
    }

    fn kinds(&self) -> Vec<KindDecl> {
        self.kinds.clone()
    }

    /// Extension blocks hold blocks, the way tier 2's do: the
    /// vocabulary's consume decisions speak of "the rest of the line
    /// processed inside the block as usual", which only a
    /// block-holding block can do. There is no per-kind flag to make
    /// this depend on, so it is a constant.
    fn contains_blocks(&self) -> bool {
        true
    }

    /// Starts a parse. While an error stays latched — from an earlier
    /// parse the host has not drained with `take_error` — every hook
    /// declines, so the latch survives `begin_parse` until it is
    /// taken, exactly tier 2's rule.
    fn begin_parse(&self) {
        if self.latched() {
            return;
        }
        if let Some(init) = &self.hooks.init {
            let _: Option<()> = self.call_result(|| init().map(|()| None));
        }
    }

    /// The latched error, if one has occurred: returned and taken.
    fn take_error(&self) -> Option<String> {
        let mut slot = self
            .error
            .lock()
            .expect("the host runtime mutex stays sound");
        slot.take()
    }

    /// The pure `block-start-look` call: whether the extension would
    /// start a block on this line. A failure latches and declines
    /// (`false`).
    fn block_start_looks(&self, ctx: &LookCtx<'_, '_>) -> bool {
        let Some(hook) = &self.hooks.block_start_look else {
            return false;
        };
        let line = lossy(ctx.line);
        let column = u32::try_from(ctx.column).unwrap_or(u32::MAX);
        self.call_result(|| hook(&line, ctx.indented, column).map(Some))
            .unwrap_or(false)
    }

    /// The `block-start` call, with the decision mapped onto the
    /// context's opening APIs, the same mappings tier 2 uses:
    ///
    /// * `whole-line` opens with `open_extension`, which consumes the
    ///   whole line.
    /// * `columns(n)` and `rest` move the cursor with `seek_content`
    ///   and open with `open_extension_keep_rest`, leaving the rest of
    ///   the line for the parser to add as the block's first
    ///   paragraph.
    /// * `claim(n)` — the task-list pattern — marks the deepest plain
    ///   container with `set_extension` and the decision's attributes,
    ///   consumes `n` bytes off the line, and opens nothing.
    ///
    /// When the deepest open block is a paragraph, the extension opens
    /// rather than converts, as tier 2 does: the paragraph's text
    /// survives as its own paragraph node in front of the new block.
    /// A decision naming an undeclared kind, or a claim with no plain
    /// container open, is a malformed decision and latches.
    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        let Some(hook) = &self.hooks.block_start else {
            return;
        };
        let container = ctx.container();
        let container_kind = match &container {
            StartContainer::Container => None,
            StartContainer::Paragraph => Some(String::from("paragraph")),
            StartContainer::Extension(_) => {
                let open = ctx.open_node();
                open.and_then(|node| match ctx.arena().kind(node) {
                    Node::Extension(ext) => Some(ext.kind.clone()),
                    _ => None,
                })
            }
        };
        let line = lossy(ctx.line());
        let column = u32::try_from(ctx.column()).unwrap_or(u32::MAX);
        let decision =
            self.call_result(|| hook(&line, ctx.indented(), column, container_kind.as_deref()));
        let Some(decision) = decision else {
            return;
        };
        match decision.consume {
            StartConsume::WholeLine => {
                if !self.owns_kind(&decision.kind) {
                    self.latch(format!(
                        "extension `{}` answered block-start with the undeclared kind `{}`",
                        self.name, decision.kind
                    ));
                    return;
                }
                let state: Box<dyn std::any::Any + Send> = Box::new(());
                let node = ctx.open_extension(decision.kind, state);
                set_typed_attrs(ctx.arena(), node, decision.attrs);
            }
            StartConsume::Columns(n) => {
                if !self.owns_kind(&decision.kind) {
                    self.latch(format!(
                        "extension `{}` answered block-start with the undeclared kind `{}`",
                        self.name, decision.kind
                    ));
                    return;
                }
                ctx.seek_content(usize::try_from(n).unwrap_or(usize::MAX));
                let state: Box<dyn std::any::Any + Send> = Box::new(());
                let node = ctx.open_extension_keep_rest(decision.kind, state);
                set_typed_attrs(ctx.arena(), node, decision.attrs);
            }
            StartConsume::Rest => {
                if !self.owns_kind(&decision.kind) {
                    self.latch(format!(
                        "extension `{}` answered block-start with the undeclared kind `{}`",
                        self.name, decision.kind
                    ));
                    return;
                }
                ctx.seek_content(0);
                let state: Box<dyn std::any::Any + Send> = Box::new(());
                let node = ctx.open_extension_keep_rest(decision.kind, state);
                set_typed_attrs(ctx.arena(), node, decision.attrs);
            }
            StartConsume::Claim(bytes) => {
                let Some(item) = ctx.container_node() else {
                    self.latch(format!(
                        "extension `{}` answered block-start with a claim but no plain container is open",
                        self.name
                    ));
                    return;
                };
                let index = ctx.extension();
                let arena = ctx.arena();
                arena.set_extension(item, index);
                set_attrs_on(arena, item, decision.attrs);
                ctx.consume(usize::try_from(bytes).unwrap_or(usize::MAX));
            }
        }
    }

    /// The `block-continue` call, with the decision mapped onto the
    /// cursor the way tier 2 maps it: `keep` consumes the line as its
    /// consume names, `close-line` closes the block and consumes the
    /// line whole. `blank` is whether nothing but spaces remains after
    /// the container prefixes, which is when the context's line is
    /// empty.
    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        let Some(hook) = &self.hooks.block_continue else {
            return ContinueOutcome::Close;
        };
        let blank = ctx.line().is_empty();
        let indented = ctx.indent() >= 4;
        let line = lossy(ctx.line());
        let column = u32::try_from(ctx.column()).unwrap_or(u32::MAX);
        let decision = self.call_result(|| hook(&line, indented, column, blank));
        let Some(decision) = decision else {
            return ContinueOutcome::Close;
        };
        match decision {
            ContinueDecision::Keep(consume) => {
                match consume {
                    ContinueConsume::WholeLine => ctx.consume_cols(usize::MAX),
                    ContinueConsume::Columns(n) => {
                        ctx.consume_cols(usize::try_from(n).unwrap_or(usize::MAX));
                    }
                    ContinueConsume::Rest => {}
                }
                ContinueOutcome::Continue
            }
            ContinueDecision::CloseLine => ContinueOutcome::CloseAndConsume,
        }
    }

    /// The `block-close` call. When the hook returns HTML, the node's
    /// whole rendering is that HTML — the extension owns the block's
    /// output and the engine's children render nowhere — realized by
    /// converting the node to a raw HTML block holding the returned
    /// HTML, newline-terminated, the way tier 2 realizes it. A decline
    /// keeps the node, and its children render through the usual
    /// renderer, the way a plain container does.
    fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        // The state box marks the frame as this extension's, the way
        // tier 2's does, so a frame another extension opened is left
        // alone.
        if state.downcast_ref::<()>().is_none() {
            return;
        }
        let Some(hook) = &self.hooks.block_close else {
            return;
        };
        let Some(html) = self.call_result(|| hook.as_ref()()) else {
            return;
        };
        let mut literal = html;
        if !literal.ends_with('\n') {
            literal.push('\n');
        }
        ctx.arena.set_kind(
            ctx.node,
            Node::HtmlBlock(geist4_core::node::HtmlBlockData {
                kind: 7,
                literal: Cow::Owned(literal),
            }),
        );
    }

    /// The `inline-trigger` call. The engine only reaches it for the
    /// bytes `inline_triggers` registers. The line is the prefix
    /// before the cursor joined with the rest of the line, and `pos`
    /// is the prefix's length. A kind answer tags the run at the
    /// cursor; anything else leaves the byte as text, the way tier 2
    /// treats literal and node decisions.
    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let Some(hook) = &self.hooks.inline_trigger else {
            return;
        };
        let pos = u32::try_from(ctx.line_prefix().len()).unwrap_or(u32::MAX);
        let prefix = String::from_utf8_lossy(ctx.line_prefix());
        let rest = String::from_utf8_lossy(ctx.rest_of_line());
        let line = format!("{prefix}{rest}");
        let Some(kind) = self.call_result(|| hook(&line, pos)) else {
            return;
        };
        if !self.owns_kind(&kind) {
            self.latch(format!(
                "extension `{}` answered inline-trigger with the undeclared kind `{kind}`",
                self.name
            ));
            return;
        }
        let run = ctx.scan_run();
        ctx.push_delim(&run, &kind);
    }

    /// The `inline-bracket` call for `[`…`]` groups that matched no
    /// link. Only a group whose first node is plain text is decided
    /// on — that is when the content exists — and the check reads only
    /// the group's first node, so bracket-heavy input stays cheap. A
    /// claim takes the whole group, keeping up to `end` bytes of the
    /// content as the node's children.
    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        let Some(hook) = &self.hooks.inline_bracket else {
            return;
        };
        if !ctx.content_starts_with(b"") {
            return;
        }
        let Some(content) = ctx.content() else {
            return;
        };
        let content = content.to_owned();
        let Some(claim) = self.call_result(|| hook(&content)) else {
            return;
        };
        if !self.owns_kind(&claim.kind) {
            self.latch(format!(
                "extension `{}` answered inline-bracket with the undeclared kind `{}`",
                self.name, claim.kind
            ));
            return;
        }
        ctx.claim_with_content(claim.kind, claim.attrs, claim.end as usize);
    }

    /// The `post-process` pass: the extension rewrites one text node's
    /// contents, the way a tier-1 replacement does. The walk covers
    /// every `Text` node — code blocks and raw HTML hold their text in
    /// node data rather than text children, and code spans do not
    /// exist yet at this pass — so text inside them is never offered.
    /// A failure latches and ends the walk.
    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        let Some(hook) = &self.hooks.post_process else {
            return;
        };
        let mut stack: Vec<NodeId> = vec![ctx.root];
        while let Some(id) = stack.pop() {
            let text = if let Node::Text(text) = ctx.arena.kind(id) {
                text.to_string()
            } else {
                stack.extend(ctx.arena.children(id).iter().copied());
                continue;
            };
            let Some(rewritten) = self.call_result(|| hook(&text)) else {
                return;
            };
            if rewritten != text {
                ctx.arena.set_kind(id, Node::Text(Cow::Owned(rewritten)));
            }
            stack.extend(ctx.arena.children(id).iter().copied());
        }
    }

    /// The `render` call: the extension renders the node's whole
    /// output from the kind, the attributes in their display form and
    /// the children's rendered HTML. A failure latches and renders
    /// nothing, the neutral result.
    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        let Some(hook) = &self.hooks.render else {
            return;
        };
        let attrs: Vec<(String, String)> = node
            .attrs
            .iter()
            .map(|(name, value)| (name.clone(), attr_string(value)))
            .collect();
        let children_html: String = children.iter().map(|child| child.html.as_str()).collect();
        if let Some(html) = self.call_result(|| hook(&node.kind, &attrs, &children_html)) {
            out.push_str(&html);
        }
    }

    /// The `render_override` call for a claimed core node. The
    /// children are pre-rendered with the built-in conventions —
    /// pulling them one by one and concatenating is the same output,
    /// because the built-in's per-child conventions do not depend on
    /// the output so far — so the callback receives one HTML string
    /// per child and assembles its output freely.
    fn render_override(
        &self,
        node: &CoreNode,
        children: &mut OverrideChildren<'_, '_>,
        out: &mut String,
    ) {
        let Some(hook) = &self.hooks.render_override else {
            return;
        };
        let mut rendered: Vec<String> = Vec::new();
        let mut scratch = String::new();
        while children.render_next(&mut scratch) {
            rendered.push(std::mem::take(&mut scratch));
        }
        let owned = CoreNode {
            kind: node.kind,
            attrs: node.attrs,
        };
        let _ = self.call_result(|| hook(&owned, &rendered, out).map(Some));
    }
}

// ---------------------------------------------------------------------------
// Decision conversion and small helpers.
// ---------------------------------------------------------------------------

/// Reads a `block-start` decision the C surface wrote, validating its
/// tags and spans; a malformed decision latches and declines.
unsafe fn start_from_c(
    decision: &Geist4StartDecision,
    error: &Mutex<Option<String>>,
) -> Option<StartDecision> {
    let read_kind = |decision: &Geist4StartDecision| -> Option<String> {
        // SAFETY: the caller's contract — the decision's spans are
        // valid reads.
        match unsafe { read_str(&decision.kind) } {
            Ok(kind) => Some(kind.to_owned()),
            Err(reason) => {
                latch_shared(error, format!("block-start: {reason}"));
                None
            }
        }
    };
    let read = unsafe { read_attrs(decision.attrs, decision.attrs_len) };
    let Some(attrs) = read else {
        latch_shared(error, "block-start: a malformed attribute list".to_string());
        return None;
    };
    match decision.tag {
        GEIST4_START_WHOLE_LINE => read_kind(decision).map(|kind| StartDecision {
            consume: StartConsume::WholeLine,
            kind,
            attrs,
        }),
        GEIST4_START_COLUMNS => read_kind(decision).map(|kind| StartDecision {
            consume: StartConsume::Columns(decision.columns),
            kind,
            attrs,
        }),
        GEIST4_START_REST => read_kind(decision).map(|kind| StartDecision {
            consume: StartConsume::Rest,
            kind,
            attrs,
        }),
        GEIST4_START_CLAIM => Some(StartDecision {
            consume: StartConsume::Claim(decision.columns),
            kind: String::new(),
            attrs,
        }),
        tag => {
            latch_shared(error, format!("block-start: the unknown consume tag {tag}"));
            None
        }
    }
}

/// Reads a `block-continue` decision the C surface wrote.
unsafe fn continue_from_c(
    decision: &Geist4ContinueDecision,
    error: &Mutex<Option<String>>,
) -> Option<ContinueDecision> {
    match decision.tag {
        GEIST4_CONTINUE_KEEP => Some(ContinueDecision::Keep(match decision.columns {
            u32::MAX => ContinueConsume::WholeLine,
            columns => ContinueConsume::Columns(columns),
        })),
        GEIST4_CONTINUE_CLOSE_LINE => Some(ContinueDecision::CloseLine),
        tag => {
            latch_shared(error, format!("block-continue: the unknown tag {tag}"));
            None
        }
    }
}

/// Reads an `inline-bracket` decision the C surface wrote.
unsafe fn bracket_from_c(
    decision: &Geist4InlineDecision,
    error: &Mutex<Option<String>>,
) -> Option<BracketClaim> {
    match decision.tag {
        GEIST4_INLINE_NODE => {
            let kind = match unsafe { read_str(&decision.kind) } {
                Ok(kind) => kind.to_owned(),
                Err(reason) => {
                    latch_shared(error, format!("inline-bracket: {reason}"));
                    return None;
                }
            };
            // The bracket claim carries typed attributes the same way
            // a block start does: the C surface fills the decision's
            // attribute array, and a malformed array latches and
            // declines.
            let read = unsafe { read_attrs(decision.attrs, decision.attrs_len) };
            let Some(attrs) = read else {
                latch_shared(
                    error,
                    "inline-bracket: a malformed attribute list".to_string(),
                );
                return None;
            };
            Some(BracketClaim {
                kind,
                attrs,
                end: decision.end,
            })
        }
        // `literal` and `delimiter` leave the group as text, the way
        // tier 2 declines them.
        _ => None,
    }
}

/// Reads a `Geist4Rendered` the C surface wrote.
unsafe fn rendered_from_c(
    rendered: &Geist4Rendered,
    error: &Mutex<Option<String>>,
) -> Option<String> {
    match rendered.tag {
        GEIST4_OUT_HTML => match unsafe { read_str(&rendered.html) } {
            Ok(html) => Some(html.to_owned()),
            Err(reason) => {
                latch_shared(error, format!("render: {reason}"));
                None
            }
        },
        _ => None,
    }
}

/// Reads an attribute array the C surface wrote, as typed values.
///
/// # Safety
/// `attrs` must be a valid read of `len` entries — the caller's
/// contract — and each entry's spans must be valid reads.
unsafe fn read_attrs(attrs: *const Geist4Attr, len: usize) -> Option<Vec<(String, AttrValue)>> {
    if attrs.is_null() || len == 0 {
        return Some(Vec::new());
    }
    // SAFETY: the caller's contract — `len` entries are valid reads.
    let entries = unsafe { std::slice::from_raw_parts(attrs, len) };
    let mut out = Vec::with_capacity(len);
    for entry in entries {
        let name = match unsafe { read_str(&entry.name) } {
            Ok(name) => name.to_owned(),
            Err(_) => return None,
        };
        let value = match entry.tag {
            GEIST4_ATTR_STR => match unsafe { read_str(&entry.text) } {
                Ok(text) => AttrValue::Str(text.to_owned()),
                Err(_) => return None,
            },
            GEIST4_ATTR_INT => AttrValue::Int(entry.number),
            GEIST4_ATTR_BOOL => AttrValue::Bool(entry.number != 0),
            _ => return None,
        };
        out.push((name, value));
    }
    Some(out)
}

/// Builds the C surface's attribute array from display-form pairs: a
/// string value keeps its bytes, an integer or boolean value carries
/// its number. The `text` span is left empty for the typed tags.
fn attrs_to_c(attrs: &[(String, String)]) -> Vec<Geist4Attr> {
    attrs
        .iter()
        .map(|(name, value)| Geist4Attr {
            name: span_of(name.as_bytes()),
            tag: GEIST4_ATTR_STR,
            number: 0,
            text: span_of(value.as_bytes()),
        })
        .collect()
}

/// Builds the C surface's attribute array from typed pairs.
fn typed_attrs_to_c(attrs: &[(String, AttrValue)]) -> Vec<Geist4Attr> {
    attrs
        .iter()
        .map(|(name, value)| match value {
            AttrValue::Str(text) => Geist4Attr {
                name: span_of(name.as_bytes()),
                tag: GEIST4_ATTR_STR,
                number: 0,
                text: span_of(text.as_bytes()),
            },
            AttrValue::Int(number) => Geist4Attr {
                name: span_of(name.as_bytes()),
                tag: GEIST4_ATTR_INT,
                number: *number,
                text: Geist4Span {
                    data: std::ptr::null(),
                    len: 0,
                },
            },
            AttrValue::Bool(flag) => Geist4Attr {
                name: span_of(name.as_bytes()),
                tag: GEIST4_ATTR_BOOL,
                number: i64::from(*flag),
                text: Geist4Span {
                    data: std::ptr::null(),
                    len: 0,
                },
            },
        })
        .collect()
}

/// A borrowed span over `bytes`, valid for the call it is passed in.
fn span_of(bytes: &[u8]) -> Geist4Span {
    Geist4Span {
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}

/// The engine-provided fetch a render-override hook pulls children
/// with: child `i`'s pre-rendered HTML, copied into `out`. A negative
/// return means the index is past the last child, which is how the
/// host stops.
unsafe extern "C" fn fetch_children(ctx: *mut c_void, index: u32, out: *mut Geist4Span) -> i32 {
    // SAFETY: the box owns the vector for the duration of the hook
    // call, which is the only time the host can reach it.
    let children = unsafe { &*(ctx as *const Vec<String>) };
    match children.get(index as usize) {
        Some(html) => {
            // SAFETY: the caller's contract — `out` is a valid write.
            unsafe {
                (*out).data = html.as_ptr();
                (*out).len = html.len();
            }
            0
        }
        None => -1,
    }
}

/// A node attribute's value in display form: a string value passes
/// through; the typed values render as their Rust display, the way the
/// declarative tier's templates interpolate an `{attr}`.
fn attr_string(value: &AttrValue) -> String {
    match value {
        AttrValue::Str(text) => text.clone(),
        AttrValue::Int(number) => number.to_string(),
        AttrValue::Bool(flag) => flag.to_string(),
    }
}

/// The line's bytes as text. The parser cuts `line` at ASCII bytes —
/// the scanner positions — so the slice is valid UTF-8 and the lossy
/// conversion borrows; a hook never sees invalid text because of the
/// engine.
fn lossy(line: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(line)
}

/// Sets a start decision's typed attributes on a freshly opened
/// extension node. The node was created with empty attributes, so the
/// decision's list becomes the node's list, in the decision's order.
fn set_typed_attrs(arena: &mut Arena<'_>, node: NodeId, attrs: Vec<(String, AttrValue)>) {
    if attrs.is_empty() {
        return;
    }
    let mut ext = match arena.kind(node) {
        Node::Extension(ext) => ext.clone(),
        _ => return,
    };
    ext.attrs.extend(attrs);
    arena.set_kind(node, Node::Extension(ext));
}

/// Sets typed attributes on any node, in order — the claim path, where
/// the node already carries its own data.
fn set_attrs_on(arena: &mut Arena<'_>, node: NodeId, attrs: Vec<(String, AttrValue)>) {
    for (name, value) in attrs {
        arena.set_attr(node, name, value);
    }
}

/// Latches through the shared error slot the C-adapted closures carry:
/// the same first-error-wins rule the extension's own latch applies.
fn latch_shared(error: &Mutex<Option<String>>, message: String) -> String {
    let mut slot = error.lock().expect("the host runtime mutex stays sound");
    slot.get_or_insert(message.clone());
    message
}
