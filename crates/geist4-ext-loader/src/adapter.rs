//! The tier-2 adapter: a loaded WebAssembly component behind the core
//! `Extension` trait.
//!
//! [`WasmExtension`] wraps a loaded [`SandboxExtension`] — the compiled
//! component plus the fuel and memory limits the manifest's `[wasm]`
//! table chose — in the object the parser and renderer drive. Every
//! hook the WIT world (`geist4:extension@1.0.0/hooks`) exposes is
//! routed to a component call: `init` in `begin_parse`,
//! `block-start-look` in `block_start_looks`, `block-start` in
//! `try_start`, `block-continue` in `try_continue`, `block-close` in
//! `close`, `inline-trigger` and `inline-bracket` in their hooks,
//! `post-process` in `post_process`, and `render` in `render`. The
//! trait's hooks the world does not expose — `front_matter_delimiter`,
//! `inline_triggers`, `begin_render`, `document_suffix`,
//! `render_override`, `filters_html`, `html_filter` — keep their
//! neutral defaults, so they decline.
//!
//! # Per-parse state
//!
//! The component's state is its own instance memory: per the sandbox's
//! rule, one instance lives across a document's hook calls and is
//! dropped after. `begin_parse` creates it — instantiation and `init`
//! run there, outside any hook — and the next parse drops it and
//! starts fresh, so hooks never see state from a previous document.
//!
//! # Latched errors
//!
//! A hook call that fails — a trap, fuel exhaustion, the memory limit,
//! a malformed reply, or a component that declines `init` or `render` —
//! latches the first such error: later failures do not overwrite it,
//! the instance is dropped, and every hook declines (returns the
//! neutral result) until the host takes the error with `take_error`
//! and a new parse begins. That is the format's error-surfacing rule
//! for tier 2 in `docs/extensions/format.md`: the error surfaces as a
//! parse problem the host reports, the document still parses, and the
//! extension's effects stop at the failure.
//!
//! The latch and the live instance share one mutex, so the trait's
//! `&self` hook signatures see a consistent view. This is the
//! adapter's own per-extension state, which the format allows — the
//! core stays free of global state.
//!
//! # Decision mappings the trait cannot express
//!
//! Three decisions the world allows have no counterpart in the trait's
//! hooks, and the adapter declines them rather than guessing:
//!
//! * `inline-trigger`'s `node` decision (claim a span at the trigger
//!   byte whole): `InlineTriggerCtx` only scans runs and pushes
//!   delimiters — it has no claim API — so a node decision declines
//!   and the trigger byte stays literal text.
//! * `inline-bracket`'s `delimiter` decision: `InlineBracketCtx`
//!   claims whole groups or nothing; a delimiter decision declines and
//!   the group stays literal text.
//! * A decision's `end` offset: the trait's APIs claim the whole
//!   bracket group or the whole scanned run, so a partial claim is not
//!   representable and the offset is ignored.
//!
//! Two more mappings are shaped by the trait where the world leaves
//! the engine's behavior open; both are documented on their hooks:
//! how a start decision opens when the deepest open block is a
//! paragraph (see `try_start`), and how a close decision's HTML
//! becomes the node's rendering (see `close`).

use std::borrow::Cow;
use std::sync::Mutex;

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ExtNode, InlineBracketCtx,
    InlineTriggerCtx, KindDecl, LookCtx, RenderedChild, Run, StartContainer, TreeCtx,
};
use geist4_core::node::Node;

use crate::sandbox::{Consume, InlineAction, SandboxError, SandboxExtension, SandboxInstance};

/// The per-block state a `WasmExtension` frame carries.
///
/// The component keeps the block's state in its own instance memory,
/// so the box only marks the frame as this adapter's: the parser hands
/// it back through `try_continue` and `close`, whose downcasts must
/// succeed for every frame this adapter opened.
struct BlockState;

/// The adapter's per-parse runtime: the live instance of the current
/// parse and the latched error, if any.
struct Runtime {
    instance: Option<SandboxInstance>,
    error: Option<String>,
}

/// A tier-2 extension: a WebAssembly component behind the
/// [`geist4_core::extension::Extension`] trait.
///
/// Built from a validated tier-2 manifest and the component binary it
/// names; `crate::package::load` is the normal constructor path.
pub struct WasmExtension {
    /// The validated manifest: the name, the declared kinds (none, for
    /// a valid tier-2 manifest — every declared kind must be used by a
    /// tier-1 behavior entry, and a tier-2 manifest declares no
    /// entries), and the options with their defaults.
    manifest: crate::types::Manifest,
    /// The loaded component and its limits.
    extension: SandboxExtension,
    /// The per-parse runtime, behind a mutex for the `&self` hooks.
    runtime: Mutex<Runtime>,
}

impl WasmExtension {
    /// Builds the adapter for a validated tier-2 manifest and its
    /// loaded component. The caller — `package::load`, or a test
    /// building the pieces directly — has checked the manifest with
    /// `crate::manifest::parse` and the component with
    /// [`SandboxExtension::check`]; the checks are not repeated here.
    #[must_use]
    pub fn new(manifest: crate::types::Manifest, extension: SandboxExtension) -> Self {
        Self {
            manifest,
            extension,
            runtime: Mutex::new(Runtime {
                instance: None,
                error: None,
            }),
        }
    }

    /// Locks the runtime. The lock guards only this extension's own
    /// per-parse state. It cannot be poisoned in practice: every
    /// critical section only moves plain data, and a component call
    /// reports failures as returned errors, never as unwinds.
    fn lock(&self) -> std::sync::MutexGuard<'_, Runtime> {
        self.runtime.lock().expect("the runtime mutex stays sound")
    }

    /// Runs one component call against the current parse's instance.
    ///
    /// Returns `None` when there is no instance to call — `begin_parse`
    /// has not run, an earlier failure is latched, or the call itself
    /// just failed. A failure latches its message (the first one wins)
    /// and drops the instance, so every later hook declines until the
    /// host takes the error and a new parse begins.
    fn call<T>(
        &self,
        run: impl FnOnce(&mut SandboxInstance) -> Result<T, SandboxError>,
    ) -> Option<T> {
        let mut runtime = self.lock();
        let instance = runtime.instance.as_mut()?;
        match run(instance) {
            Ok(value) => Some(value),
            Err(error) => {
                runtime.error.get_or_insert(error.to_string());
                runtime.instance = None;
                None
            }
        }
    }

    /// Begins a parse when no error is latched: builds a fresh
    /// instance, initializes it with the manifest's kinds and option
    /// defaults, and keeps it for the hooks. Any failure —
    /// instantiation, `init`, a limit hit during either — latches.
    fn start_instance(&self) {
        let kinds: Vec<crate::types::KindEntry> = self.manifest.kinds.clone();
        let options: Vec<(String, crate::types::OptionValue)> = self
            .manifest
            .options
            .iter()
            .map(|(name, decl)| (name.clone(), decl.default.clone()))
            .collect();
        match self.extension.instantiate().and_then(|mut instance| {
            instance.init(&kinds, &options)?;
            Ok(instance)
        }) {
            Ok(instance) => self.lock().instance = Some(instance),
            Err(error) => self.lock().error = Some(error.to_string()),
        }
    }
}

impl geist4_core::extension::Extension for WasmExtension {
    fn name(&self) -> &str {
        &self.manifest.extension.name
    }

    fn kinds(&self) -> Vec<KindDecl> {
        self.manifest
            .kinds
            .iter()
            .map(|entry| KindDecl {
                kind: entry.kind.clone(),
                container: entry.container,
                inline_text: entry.inline_text,
                attrs: entry.attrs.clone(),
            })
            .collect()
    }

    /// Extension blocks hold blocks: the world's `consume` decisions
    /// speak of "the rest of the line … processed inside the block as
    /// usual, typically as its first paragraph", which only a
    /// block-holding block can do. There is no per-kind flag to make
    /// this depend on, so it is a constant.
    fn contains_blocks(&self) -> bool {
        true
    }

    /// Starts a parse: a fresh instance under this extension's limits,
    /// initialized with the manifest's kinds and option defaults,
    /// exactly the state the sandbox's per-parse rule describes.
    ///
    /// While an error stays latched (from an earlier parse the host
    /// has not drained with `take_error`), every hook declines; the
    /// latch survives `begin_parse` until it is taken.
    fn begin_parse(&self) {
        let mut runtime = self.lock();
        runtime.instance = None;
        if runtime.error.is_some() {
            // Still latched from an earlier parse: hooks decline until
            // the host takes the error and a new parse begins.
            return;
        }
        drop(runtime);
        self.start_instance();
    }

    /// The latched error, if one has occurred: returned and taken. The
    /// next call returns `None` until a new error is latched, and the
    /// next `begin_parse` builds a fresh instance again.
    fn take_error(&self) -> Option<String> {
        self.lock().error.take()
    }

    /// The pure `block-start-look` call: whether the component would
    /// start a block on this line. An error latches and declines
    /// (`false`).
    fn block_start_looks(&self, ctx: &LookCtx<'_, '_>) -> bool {
        let line = lossy(ctx.line);
        let column = u32::try_from(ctx.column).unwrap_or(u32::MAX);
        self.call(|instance| instance.block_start_look(&line, ctx.indented, column))
            .is_some_and(|looked| looked)
    }

    /// The `block-start` call, with the decision mapped onto the
    /// context's opening APIs:
    ///
    /// * `whole-line` opens with `open_extension`, which consumes the
    ///   whole line.
    /// * `columns(n)` and `rest` move the cursor with `seek_content` —
    ///   to the content, then `n` bytes past it — and open with
    ///   `open_extension_keep_rest`, leaving the rest of the line for
    ///   the parser to add as the block's first paragraph. The world
    ///   counts `columns` from the same position the context's
    ///   `column` names; for the ASCII markers the format's block
    ///   starts are, columns and bytes coincide.
    ///
    /// The world's `container-kind` is passed through: `none` for the
    /// document and plain containers, `"paragraph"` for an open
    /// paragraph, the open block's kind for an extension block. When
    /// the deepest open block is a paragraph, the adapter opens rather
    /// than converts — `open_extension` interrupts the paragraph, and
    /// its text survives as its own paragraph node in front of the new
    /// block, which preserves the text without adopting the
    /// table-specific pre-paragraph rules `convert_paragraph` exists
    /// for. The decision's attributes are set on the opened node, in
    /// the decision's order.
    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
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
        let indented = ctx.indented();
        let column = u32::try_from(ctx.column()).unwrap_or(u32::MAX);
        let Some(Some(start)) = self.call(|instance| {
            instance.block_start(&line, indented, column, container_kind.as_deref())
        }) else {
            return;
        };
        let state: Box<dyn std::any::Any + Send> = Box::new(BlockState);
        let node = match start.consume {
            Consume::WholeLine => ctx.open_extension(start.kind, state),
            Consume::Columns(n) => {
                ctx.seek_content(usize::try_from(n).unwrap_or(usize::MAX));
                ctx.open_extension_keep_rest(start.kind, state)
            }
            Consume::Rest => {
                ctx.seek_content(0);
                ctx.open_extension_keep_rest(start.kind, state)
            }
        };
        set_node_attrs(ctx.arena(), node, start.attrs);
    }

    /// The `block-continue` call, with the decision mapped onto the
    /// cursor: `whole-line` consumes every remaining column (the way a
    /// closing fence consumes its line), `columns(n)` consumes `n`
    /// columns from the position the container prefixes left — the
    /// same position the world counts from — and `rest` consumes
    /// nothing, so the whole line becomes the block's content.
    ///
    /// `blank` is the world's "nothing but spaces remains after the
    /// container prefixes": the context's `line` already starts at the
    /// first non-space byte, so it is empty exactly then. `indented`
    /// is the context's indent reaching four columns.
    ///
    /// The state box only marks the frame — the component keeps the
    /// block's state in its own instance memory — so it is not read.
    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> bool {
        let blank = ctx.line().is_empty();
        let indented = ctx.indent() >= 4;
        let line = lossy(ctx.line());
        let column = u32::try_from(ctx.column()).unwrap_or(u32::MAX);
        let Some(Some(decision)) =
            self.call(|instance| instance.block_continue(&line, indented, column, blank))
        else {
            return false;
        };
        match decision.consume {
            Consume::WholeLine => ctx.consume_cols(usize::MAX),
            Consume::Columns(n) => ctx.consume_cols(usize::try_from(n).unwrap_or(usize::MAX)),
            Consume::Rest => {}
        }
        true
    }

    /// The `block-close` call. When the component returns HTML, the
    /// world says the engine uses it as the node's whole rendering —
    /// the component owns the block's output and the engine's children
    /// render nowhere. The adapter realizes that by converting the
    /// node to a raw HTML block holding the returned HTML (newline-
    /// terminated, like every HTML block literal), dropping the
    /// children from the rendering. `none` keeps the node, and its
    /// children render through the usual renderer, the way a plain
    /// container does.
    fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        // The state box only marks the frame as this adapter's; the
        // component's own state lived in the instance, which the
        // parser has just finished with.
        if state.downcast_mut::<BlockState>().is_none() {
            return;
        }
        let Some(Some(rendered)) = self.call(|instance| instance.block_close()) else {
            return;
        };
        let mut literal = rendered.html;
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
    /// bytes `inline_triggers` registers, and this adapter registers
    /// none — the world has the engine learn trigger bytes from what
    /// the component scans in `init`, which carries no such data — so
    /// in practice the hook is dormant. The mapping is implemented
    /// regardless, because the world exposes the hook: the line is the
    /// prefix before the cursor joined with the rest of the line, and
    /// `pos` is the prefix's length. A `delimiter` decision scans the
    /// run at the cursor and tags it with the decision's kind, the way
    /// a tier-1 span's delimiter works; `literal` leaves the byte as
    /// text; `node` declines, as the trait has no claim API here.
    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let pos = u32::try_from(ctx.line_prefix().len()).unwrap_or(u32::MAX);
        let prefix = String::from_utf8_lossy(ctx.line_prefix());
        let rest = String::from_utf8_lossy(ctx.rest_of_line());
        let line = format!("{prefix}{rest}");
        let Some(Some(InlineAction::Delimiter { kind, .. })) =
            self.call(|instance| instance.inline_trigger(&line, pos))
        else {
            // `literal`, `node`, a declined call, or an error: the
            // trigger byte stays literal text.
            return;
        };
        let run: Run = ctx.scan_run();
        ctx.push_delim(&run, &kind);
    }

    /// The `inline-bracket` call for `[`…`]` groups that matched no
    /// link. Only a group whose first node is plain text is decided
    /// on — that is when the world's `content` exists — and the check
    /// reads only the group's first node, so bracket-heavy input stays
    /// cheap. A `node` decision claims the whole group (the trait's
    /// claim has no `end` to stop early at); `literal` and
    /// `delimiter` leave the group as text.
    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        let action = match ctx.content_starts_with(b"") {
            true => ctx
                .content()
                .and_then(|content| self.call(|instance| instance.inline_bracket(content))),
            false => None,
        };
        let Some(Some(InlineAction::Node { kind, attrs, end })) = action else {
            return;
        };
        let attrs = attrs
            .into_iter()
            .map(|(name, value)| (name, AttrValue::Str(value)))
            .collect();
        // The world's node decision keeps the group's content as the
        // node's inline children, up to the decision's `end`.
        ctx.claim_with_content(kind, attrs, end as usize);
    }

    /// The `post-process` pass: the component rewrites one text node's
    /// contents, the way a tier-1 replacement does. The walk covers
    /// every `Text` node — code blocks and raw HTML hold their text in
    /// node data rather than text children, and code spans do not
    /// exist yet at this pass — so text inside them is never offered,
    /// as the world asks. An error latches and ends the walk; every
    /// later call declines anyway.
    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        let mut stack: Vec<NodeId> = vec![ctx.root];
        while let Some(id) = stack.pop() {
            let text = match ctx.arena.kind(id) {
                Node::Text(text) => text.to_string(),
                _ => {
                    stack.extend(ctx.arena.children(id).iter().copied());
                    continue;
                }
            };
            let Some(rewritten) = self.call(|instance| instance.post_process(&text)) else {
                return;
            };
            if rewritten != text {
                ctx.arena.set_kind(id, Node::Text(Cow::Owned(rewritten)));
            }
            stack.extend(ctx.arena.children(id).iter().copied());
        }
    }

    /// The `render` call: the component renders the node's whole
    /// output from the kind, the attributes (their values in the
    /// world's string form, as `attr_string` describes) and the
    /// children's rendered HTML. An error latches and renders nothing,
    /// the neutral result.
    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        let attrs: Vec<(String, String)> = node
            .attrs
            .iter()
            .map(|(name, value)| (name.clone(), attr_string(value)))
            .collect();
        let children_html: String = children.iter().map(|child| child.html.as_str()).collect();
        if let Some(html) =
            self.call(|instance| instance.render(&node.kind, &attrs, &children_html))
        {
            out.push_str(&html);
        }
    }
}

/// A node attribute's value in the world's string form: a string value
/// passes through; the typed values render as their Rust display, the
/// way the declarative tier's templates interpolate an `{attr}`.
fn attr_string(value: &AttrValue) -> String {
    match value {
        AttrValue::Str(text) => text.clone(),
        AttrValue::Int(number) => number.to_string(),
        AttrValue::Bool(flag) => flag.to_string(),
    }
}

/// The line's bytes as the string the component sees. The parser cuts
/// `line` at ASCII bytes — the scanner positions — so the slice is
/// valid UTF-8 and the lossy conversion borrows; a component never
/// sees invalid text because of the engine.
fn lossy(line: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(line)
}

/// Sets a start decision's attributes on the freshly opened extension
/// node. The node was created with empty attributes, so the decision's
/// list becomes the node's list, in the decision's order.
fn set_node_attrs(arena: &mut Arena<'_>, node: NodeId, attrs: Vec<(String, String)>) {
    if attrs.is_empty() {
        return;
    }
    let mut ext = match arena.kind(node) {
        Node::Extension(ext) => ext.clone(),
        _ => return,
    };
    ext.attrs = attrs
        .into_iter()
        .map(|(name, value)| (name, AttrValue::Str(value)))
        .collect();
    arena.set_kind(node, Node::Extension(ext));
}
