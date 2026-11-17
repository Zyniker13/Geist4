//! The WebAssembly sandbox for tier-2 extensions.
//!
//! A tier-2 extension (see `docs/extensions/format.md`) is a WebAssembly
//! component built against the `geist4:extension@1.0.0/geist4-extension`
//! world, whose WIT file lives at `wit/geist4-extension.wit` and is
//! normative. This module loads such a component, instantiates it under
//! the format's limits, and calls its hooks in the decision shape the
//! world defines: every hook receives the line or context bytes it
//! decides from and returns what to do, while the engine — which owns
//! the document tree — performs the decision. The component never
//! touches the tree.
//!
//! The guarantees the sandbox enforces, all normative in the format:
//!
//! - No imports. The component is instantiated against an empty linker,
//!   so any import — file system, network, clock, randomness — fails
//!   instantiation with a load error. Extensions see only the bytes and
//!   state the hooks hand them.
//! - A fuel limit per call, from the manifest's `[wasm] fuel` with a
//!   loader cap of 1e9, so a hostile extension is stopped as a defined
//!   error carrying the hook's name — surfaced as a parse problem with
//!   a source span — never a hang.
//! - A memory limit, from the manifest's `[wasm] memory_pages` with a
//!   loader cap of 256 pages (16 MiB), enforced on every allocation the
//!   component makes, including at instantiation.
//! - Per-parse state. The engine holds one [`SandboxInstance`] across a
//!   document's hook calls and drops it after; the component's state
//!   lives in its own instance memory and outlives nothing.
//!
//! An error from a hook — fuel exhaustion, the memory limit, a trap —
//! spends that instance: wasmtime refuses to re-enter a component
//! instance after a trap, so the caller drops the instance and fails
//! the parse with the error. A fresh instance of the same extension
//! parses the next document; the extension itself is not poisoned.
//!
//! Loading verifies the component is a valid component binary exporting
//! the world's functions; [`SandboxExtension::check`] additionally
//! instantiates it once and calls `init` with empty lists. Bindings are
//! made through wasmtime's dynamic `Val` interface rather than
//! generated code, so this crate holds no `unsafe` of its own.

use std::fmt;
use std::path::Path;

use wasmtime::component::types::ComponentItem;
use wasmtime::component::{Component, Func, Instance, Linker, Val};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap};

use crate::types::{KindEntry, OptionValue};

/// The loader cap on per-call fuel: the most the manifest may ask for,
/// and what a manifest without `[wasm] fuel` gets. Normative in the
/// format's limits table.
const FUEL_CAP: u64 = 1_000_000_000;

/// The loader cap on the component's memory, in WebAssembly pages: the
/// most the manifest may ask for, and what a manifest without
/// `[wasm] memory_pages` gets. 256 pages are 16 MiB.
const MEMORY_PAGES_CAP: u32 = 256;

/// A WebAssembly page's size in bytes.
const PAGE_SIZE: u64 = 65_536;

/// The name the world exports the hooks interface under. A component
/// built against the world names its export with the interface's id.
const HOOKS_INTERFACE: &str = "geist4:extension/hooks@1.0.0";

/// An error from loading, checking, or calling a sandboxed extension:
/// which hook it came from and what went wrong. The loader surfaces it
/// as a parse problem with a source span, so the message is one line of
/// prose, not a chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxError {
    /// The hook the error came from: `load` for loading and compiling
    /// the component, `instantiate` for creating an instance, or one of
    /// the world's hook names for a call — including when the call ran
    /// out of fuel or memory.
    pub hook: String,
    /// What went wrong, one line.
    pub message: String,
}

impl SandboxError {
    fn new(hook: &str, message: impl Into<String>) -> Self {
        SandboxError {
            hook: hook.to_owned(),
            message: message.into(),
        }
    }

    /// Maps a wasmtime error from a hook call onto the sandbox's error
    /// shape, naming fuel and memory exhaustion specifically: those are
    /// the two defined limit errors, everything else is the trap or
    /// failure wasmtime reports.
    fn from_call(hook: &str, err: wasmtime::Error) -> Self {
        if let Some(trap) = err.downcast_ref::<Trap>() {
            if matches!(trap, Trap::OutOfFuel) {
                return SandboxError::new(hook, "fuel exhausted");
            }
            return SandboxError::new(hook, trap.to_string());
        }
        // The store limiter rejects growth with its own error, not a
        // trap, when a component allocates past its memory limit; the
        // reason sits deeper in the error chain than the top-level
        // backtrace text, so the chain is searched for it.
        let top = err.to_string();
        let mut cause: Option<&(dyn std::error::Error + 'static)> = err.source();
        while let Some(current) = cause {
            let message = current.to_string();
            if message.contains("growing memory") || message.contains("memory limits") {
                return SandboxError::new(hook, format!("memory limit exceeded: {message}"));
            }
            cause = current.source();
        }
        SandboxError::new(hook, top)
    }
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sandbox extension hook `{}`: {}",
            self.hook, self.message
        )
    }
}

impl std::error::Error for SandboxError {}

/// How much of a line a block takes when it opens or continues,
/// counted from the engine's cursor: the position the container
/// prefixes left, where the hook's `line` starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consume {
    /// The whole line is the block's own — a fence line, say. Nothing
    /// on it becomes the block's content.
    WholeLine,
    /// `n` columns are the block's own (its marker, its prefix); the
    /// rest of the line is processed inside the block as usual,
    /// typically as its first paragraph.
    Columns(u32),
    /// Everything from the cursor on is kept as the block's content —
    /// the block takes no prefix of its own.
    Rest,
}

/// A decision to open one of the extension's blocks on the current
/// line: [`SandboxInstance::block_start`] returns it when the hook
/// opens, `None` when it declines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStart {
    /// The kind of the block to open: one of the kinds `init`
    /// received, and a container kind when the block holds blocks.
    pub kind: String,
    /// The attributes to set on the new node, in order.
    pub attrs: Vec<(String, String)>,
    /// How much of the opening line the block takes.
    pub consume: Consume,
}

/// A decision about the open block and the current line:
/// [`SandboxInstance::block_continue`] returns it when the hook
/// decides, `None` when it closes the block and leaves the line free
/// to start other blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinueDecision {
    /// The block continues, taking as much of the line as `consume`
    /// names.
    Keep(Consume),
    /// The block closes and the line is consumed whole — a closing
    /// fence.
    CloseLine,
}

/// The HTML of a closed block, when the component renders the block's
/// whole output itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedBlock {
    /// The block's HTML, emitted as the node's whole rendering.
    pub html: String,
}

/// What an inline hook decides at a candidate span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineAction {
    /// The candidate is plain text: for
    /// [`SandboxInstance::inline_trigger`] the trigger byte itself is
    /// taken as literal text and scanning continues after it; for
    /// [`SandboxInstance::inline_bracket`] the whole group stays
    /// literal text.
    Literal,
    /// The run from the trigger through `end` is one delimiter run of
    /// `kind`: the engine records it the way it records a tier-1
    /// span's delimiter run and pairs matching runs itself, wrapping
    /// the content between the two runs as the node's inline children.
    Delimiter {
        /// The kind of the nodes built between a matching pair of runs.
        kind: String,
        /// One past the run's last byte on the line.
        end: u32,
    },
    /// The span from the trigger through `end` is claimed whole as one
    /// node of `kind` with `attrs`. The engine parses the claimed bytes
    /// as the node's inline children and continues scanning after
    /// `end`; the delimiters, if any, belong to the claim, so a hook
    /// that must keep them out of its children uses
    /// [`InlineAction::Delimiter`] instead.
    Node {
        /// The kind of the node to build.
        kind: String,
        /// The attributes to set on the node, in order.
        attrs: Vec<(String, String)>,
        /// One past the node's last byte: on the line for
        /// `inline-trigger`, within the bracket content for
        /// `inline-bracket`.
        end: u32,
    },
}

/// A loaded tier-2 extension: the compiled component and the limits
/// every instance of it runs under. Loading compiles the component and
/// binds its exports, so [`SandboxExtension::load`] fails on a file
/// that is not a component and on one that lacks a hook; each parse
/// then runs in its own [`SandboxInstance`].
#[derive(Clone)]
pub struct SandboxExtension {
    engine: Engine,
    component: Component,
    hooks: HookIndices,
    fuel: u64,
    memory_pages: u32,
}

/// The exports' indices, bound once at load and resolved per instance.
#[derive(Clone)]
struct HookIndices {
    init: wasmtime::component::ComponentExportIndex,
    block_start_look: wasmtime::component::ComponentExportIndex,
    block_start: wasmtime::component::ComponentExportIndex,
    block_continue: wasmtime::component::ComponentExportIndex,
    block_close: wasmtime::component::ComponentExportIndex,
    inline_trigger: wasmtime::component::ComponentExportIndex,
    inline_bracket: wasmtime::component::ComponentExportIndex,
    post_process: wasmtime::component::ComponentExportIndex,
    render: wasmtime::component::ComponentExportIndex,
}

/// A store's data: the resource limiter the store reports allocations
/// to. Set before instantiation, so the memory limit bounds even the
/// component's initial memory.
struct State {
    limits: StoreLimits,
}

impl SandboxExtension {
    /// Loads a tier-2 extension from a component binary at `path`,
    /// failing when the file cannot be read, is not a component binary,
    /// or lacks one of the world's hooks.
    ///
    /// `fuel` and `memory_pages` are the manifest's `[wasm]` values; a
    /// `None` takes the loader's cap, and a value above the cap is cut
    /// to it. The loader may not grant more than the cap, so an
    /// extension that asks for more fuel than the cap runs under it.
    pub fn load(
        path: &Path,
        fuel: Option<u64>,
        memory_pages: Option<u32>,
    ) -> Result<Self, SandboxError> {
        let bytes = std::fs::read(path)
            .map_err(|err| SandboxError::new("load", format!("cannot read component: {err}")))?;
        Self::from_bytes(&bytes, fuel, memory_pages)
    }

    /// Loads a tier-2 extension from the component binary's bytes.
    ///
    /// The component must be in WebAssembly binary form. Only a
    /// component — not a core module — is accepted, so a plain `.wasm`
    /// core module is an invalid-component load error.
    pub fn from_bytes(
        bytes: &[u8],
        fuel: Option<u64>,
        memory_pages: Option<u32>,
    ) -> Result<Self, SandboxError> {
        let engine = Self::engine()?;
        let component = Component::from_binary(&engine, bytes)
            .map_err(|err| SandboxError::new("load", format!("invalid component: {err}")))?;
        let hooks = HookIndices::bind(&component)?;
        Ok(SandboxExtension {
            engine,
            component,
            hooks,
            fuel: effective_fuel(fuel),
            memory_pages: effective_memory_pages(memory_pages),
        })
    }

    /// The engine every instance of this extension runs on: fuel
    /// metering on, component model on, nothing else. No epoch
    /// interruption, no async: a sandboxed hook call is synchronous
    /// work bounded by fuel.
    fn engine() -> Result<Engine, SandboxError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.wasm_component_model(true);
        Engine::new(&config).map_err(|err| SandboxError::new("load", err.to_string()))
    }

    /// Verifies the component is loadable end to end: instantiates it
    /// once under the limits and calls `init` with empty lists. A
    /// missing export fails in `instantiate`; a component whose `init`
    /// rejects the empty manifest fails with its reason. The instance
    /// is dropped after the call.
    pub fn check(&self) -> Result<(), SandboxError> {
        let mut instance = self.instantiate()?;
        instance.init(&[], &[])
    }

    /// Starts a parse: a fresh instance of the component, with its own
    /// memory and state, under this extension's limits. The caller
    /// holds the instance across a document's hook calls and drops it
    /// after, per the format's per-parse state rule.
    ///
    /// Instantiation itself runs under the loader's full fuel budget,
    /// not the per-call limit: a component's setup cost is fixed at
    /// load time, while the manifest's fuel bounds each hook call.
    pub fn instantiate(&self) -> Result<SandboxInstance, SandboxError> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_bytes())
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&self.engine, State { limits });
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(FUEL_CAP)
            .map_err(|err| SandboxError::new("instantiate", err.to_string()))?;
        let linker: Linker<State> = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &self.component)
            .map_err(|err| SandboxError::from_call("instantiate", err))?;
        let funcs = self.hooks.resolve(&instance, &mut store)?;
        Ok(SandboxInstance {
            store,
            funcs,
            fuel: self.fuel,
        })
    }

    /// The memory the component's instances may hold, in bytes: the
    /// manifest's pages, cut to the loader's cap.
    fn memory_bytes(&self) -> usize {
        usize::try_from(u64::from(self.memory_pages) * PAGE_SIZE).unwrap_or(usize::MAX)
    }

    /// The per-call fuel this extension runs under, after the cap.
    #[must_use]
    pub fn fuel(&self) -> u64 {
        self.fuel
    }

    /// The memory this extension's instances may hold, in pages, after
    /// the cap.
    #[must_use]
    pub fn memory_pages(&self) -> u32 {
        self.memory_pages
    }
}

/// The manifest's fuel request cut to the loader's cap; `None` takes
/// the cap.
fn effective_fuel(fuel: Option<u64>) -> u64 {
    fuel.unwrap_or(FUEL_CAP).min(FUEL_CAP)
}

/// The manifest's memory request, in pages, cut to the loader's cap;
/// `None` takes the cap.
fn effective_memory_pages(memory_pages: Option<u32>) -> u32 {
    memory_pages
        .unwrap_or(MEMORY_PAGES_CAP)
        .min(MEMORY_PAGES_CAP)
}

impl HookIndices {
    /// Binds the world's exports against a compiled component, so a
    /// component that lacks the hooks interface or one of its functions
    /// fails at load with the name of what is missing. The component
    /// exports the hooks as one interface instance named with the
    /// interface's id; the functions live inside it.
    fn bind(component: &Component) -> Result<Self, SandboxError> {
        let (item, hooks_index) = component.get_export(None, HOOKS_INTERFACE).ok_or_else(|| {
            SandboxError::new(
                "load",
                format!("component does not export the `{HOOKS_INTERFACE}` interface"),
            )
        })?;
        if !matches!(item, ComponentItem::ComponentInstance(_)) {
            return Err(SandboxError::new(
                "load",
                format!("component export `{HOOKS_INTERFACE}` is not an interface instance"),
            ));
        }
        let bind = |name: &str| -> Result<wasmtime::component::ComponentExportIndex, SandboxError> {
            let (item, index) =
                component
                    .get_export(Some(&hooks_index), name)
                    .ok_or_else(|| {
                        SandboxError::new(
                            "load",
                            format!("component does not export `{HOOKS_INTERFACE}#{name}`"),
                        )
                    })?;
            match item {
                ComponentItem::ComponentFunc(_) => Ok(index),
                _ => Err(SandboxError::new(
                    "load",
                    format!("component export `{HOOKS_INTERFACE}#{name}` is not a function"),
                )),
            }
        };
        let init = bind("init")?;
        let block_start_look = bind("block-start-look")?;
        let block_start = bind("block-start")?;
        let block_continue = bind("block-continue")?;
        let block_close = bind("block-close")?;
        let inline_trigger = bind("inline-trigger")?;
        let inline_bracket = bind("inline-bracket")?;
        let post_process = bind("post-process")?;
        let render = bind("render")?;
        Ok(HookIndices {
            init,
            block_start_look,
            block_start,
            block_continue,
            block_close,
            inline_trigger,
            inline_bracket,
            post_process,
            render,
        })
    }

    /// Resolves the exports onto one instance, for the hook calls of a
    /// single parse.
    fn resolve(
        &self,
        instance: &Instance,
        store: &mut Store<State>,
    ) -> Result<Hooks, SandboxError> {
        let mut resolve = |index: &wasmtime::component::ComponentExportIndex,
                           name: &str|
         -> Result<Hook, SandboxError> {
            let func = instance
                .get_func(&mut *store, *index)
                .ok_or_else(|| SandboxError::new("load", format!("missing export `{name}`")))?;
            let results = func.ty(&*store).results().len();
            Ok(Hook { func, results })
        };
        let init = resolve(&self.init, "init")?;
        let block_start_look = resolve(&self.block_start_look, "block-start-look")?;
        let block_start = resolve(&self.block_start, "block-start")?;
        let block_continue = resolve(&self.block_continue, "block-continue")?;
        let block_close = resolve(&self.block_close, "block-close")?;
        let inline_trigger = resolve(&self.inline_trigger, "inline-trigger")?;
        let inline_bracket = resolve(&self.inline_bracket, "inline-bracket")?;
        let post_process = resolve(&self.post_process, "post-process")?;
        let render = resolve(&self.render, "render")?;
        Ok(Hooks {
            init,
            block_start_look,
            block_start,
            block_continue,
            block_close,
            inline_trigger,
            inline_bracket,
            post_process,
            render,
        })
    }
}

/// One bound export: the function and how many results it returns, so
/// a call can size the results' buffer.
struct Hook {
    func: Func,
    results: usize,
}

/// The nine bound hooks of one instance.
struct Hooks {
    init: Hook,
    block_start_look: Hook,
    block_start: Hook,
    block_continue: Hook,
    block_close: Hook,
    inline_trigger: Hook,
    inline_bracket: Hook,
    post_process: Hook,
    render: Hook,
}

/// One parse's instance of a tier-2 extension: the store, the instance
/// and the bound hooks, held across a document's calls and dropped
/// after. Every hook call runs under the extension's fuel limit, armed
/// anew each call, and the memory limit covers the instance's whole
/// life.
pub struct SandboxInstance {
    store: Store<State>,
    funcs: Hooks,
    fuel: u64,
}

impl SandboxInstance {
    /// Calls one hook with `params`, arming the fuel limit first and
    /// mapping traps onto [`SandboxError`]. Returns the results, whose
    /// shapes each hook's method then reads.
    fn call(
        store: &mut Store<State>,
        fuel: u64,
        hook: &str,
        func: &Func,
        results: usize,
        params: &[Val],
    ) -> Result<Vec<Val>, SandboxError> {
        store
            .set_fuel(fuel)
            .map_err(|err| SandboxError::new(hook, err.to_string()))?;
        let mut out = vec![Val::Bool(false); results];
        func.call(store, params, &mut out)
            .map_err(|err| SandboxError::from_call(hook, err))?;
        Ok(out)
    }

    /// Calls one hook and keeps only its first result, for the hooks
    /// that return one value.
    fn call_one(
        store: &mut Store<State>,
        fuel: u64,
        hook: &str,
        func: &Hook,
        params: &[Val],
    ) -> Result<Val, SandboxError> {
        Ok(Self::call(store, fuel, hook, &func.func, func.results, params)?.remove(0))
    }

    /// Hands the component the manifest's kinds and the user-set
    /// options, once per parse, before any other hook. Fails with the
    /// component's reason when it rejects the manifest — the engine's
    /// own cross-check of kinds against the manifest runs on its side.
    pub fn init(
        &mut self,
        kinds: &[KindEntry],
        options: &[(String, OptionValue)],
    ) -> Result<(), SandboxError> {
        let kinds = Val::List(
            kinds
                .iter()
                .map(|kind| {
                    Val::Record(vec![
                        ("kind".to_owned(), Val::String(kind.kind.clone())),
                        ("container".to_owned(), Val::Bool(kind.container)),
                        ("inline-text".to_owned(), Val::Bool(kind.inline_text)),
                        (
                            "attrs".to_owned(),
                            Val::List(
                                kind.attrs
                                    .iter()
                                    .map(|name| Val::String(name.clone()))
                                    .collect(),
                            ),
                        ),
                    ])
                })
                .collect(),
        );
        let options = Val::List(
            options
                .iter()
                .map(|(name, value)| {
                    Val::Tuple(vec![Val::String(name.clone()), option_value(value)])
                })
                .collect(),
        );
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "init",
            &self.funcs.init,
            &[kinds, options],
        )?;
        match result_result_unit(result, "init")? {
            Ok(()) => Ok(()),
            Err(reason) => Err(SandboxError::new("init", reason)),
        }
    }

    /// A pure look at a line, answering whether this extension would
    /// start a block on it — the engine's lazy-continuation check. The
    /// component must treat this as stateless; the engine may call it
    /// several times for the same line.
    pub fn block_start_look(
        &mut self,
        line: &str,
        indented: bool,
        column: u32,
    ) -> Result<bool, SandboxError> {
        let hook = &self.funcs.block_start_look;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "block-start-look",
            hook,
            &[
                Val::String(line.to_owned()),
                Val::Bool(indented),
                Val::U32(column),
            ],
        )?;
        result_bool(result, "block-start-look")
    }

    /// Decides whether to open one of the extension's blocks on the
    /// current line, after the built-in block starts have all declined.
    /// `container_kind` is the kind of the deepest open block when that
    /// block is a paragraph or another extension block, `None` for the
    /// document and plain core containers. The engine decides open
    /// versus convert from its own tree; the decision's `consume` tells
    /// it how much of the line the block takes.
    pub fn block_start(
        &mut self,
        line: &str,
        indented: bool,
        column: u32,
        container_kind: Option<&str>,
    ) -> Result<Option<BlockStart>, SandboxError> {
        let hook = &self.funcs.block_start;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "block-start",
            hook,
            &[
                Val::String(line.to_owned()),
                Val::Bool(indented),
                Val::U32(column),
                Val::Option(container_kind.map(|kind| Box::new(Val::String(kind.to_owned())))),
            ],
        )?;
        let option = result_option(result, "block-start")?;
        option
            .map(|start| {
                let fields = result_record(start, "block-start")?;
                Ok(BlockStart {
                    kind: record_string(&fields, "kind", "block-start")?,
                    attrs: record_attrs(&fields, "block-start")?,
                    consume: record_consume(&fields, "block-start")?,
                })
            })
            .transpose()
    }

    /// Decides what the current line does to the open block:
    /// `Some(Keep)` continues it, consuming the line as the decision
    /// names; `Some(CloseLine)` closes it and consumes the line whole;
    /// `None` closes it, leaving the line free to start other blocks.
    pub fn block_continue(
        &mut self,
        line: &str,
        indented: bool,
        column: u32,
        blank: bool,
    ) -> Result<Option<ContinueDecision>, SandboxError> {
        let hook = &self.funcs.block_continue;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "block-continue",
            hook,
            &[
                Val::String(line.to_owned()),
                Val::Bool(indented),
                Val::U32(column),
                Val::Bool(blank),
            ],
        )?;
        let option = result_option(result, "block-continue")?;
        option
            .map(|decision| {
                let (case, payload) = match decision {
                    Val::Variant(case, payload) => (case, payload),
                    _ => return Err(unexpected("block-continue", "a continue-decision")),
                };
                match (case.as_str(), payload) {
                    ("keep", Some(payload)) => Ok(ContinueDecision::Keep(consume_val(
                        *payload,
                        "block-continue",
                    )?)),
                    ("close-line", _) => Ok(ContinueDecision::CloseLine),
                    _ => Err(unexpected("block-continue", "a continue-decision case")),
                }
            })
            .transpose()
    }

    /// Runs once when the open block closes: `Some` is the block's
    /// whole rendered HTML, `None` lets the engine render the node's
    /// children through the usual renderer, the way a plain container
    /// renders.
    pub fn block_close(&mut self) -> Result<Option<RenderedBlock>, SandboxError> {
        let hook = &self.funcs.block_close;
        let result = Self::call_one(&mut self.store, self.fuel, "block-close", hook, &[])?;
        let option = result_option(result, "block-close")?;
        option
            .map(|block| {
                let fields = result_record(block, "block-close")?;
                Ok(RenderedBlock {
                    html: record_string(&fields, "html", "block-close")?,
                })
            })
            .transpose()
    }

    /// Runs when the inline parser reaches one of the extension's
    /// trigger bytes: `line` is the line's bytes and `pos` the trigger
    /// byte's offset within it. The decision's [`InlineAction`] says
    /// what the engine does at the candidate.
    pub fn inline_trigger(
        &mut self,
        line: &str,
        pos: u32,
    ) -> Result<Option<InlineAction>, SandboxError> {
        let hook = &self.funcs.inline_trigger;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "inline-trigger",
            hook,
            &[Val::String(line.to_owned()), Val::U32(pos)],
        )?;
        result_inline_action(result, "inline-trigger")
    }

    /// Runs when a `[`…`]` group matched no link: `content` is the
    /// group's plain-text content between the brackets. A `node`
    /// decision claims the whole group, with the content's bytes up to
    /// `end` as the node's inline children; `None` leaves the group
    /// literal text, exactly as without the hook.
    pub fn inline_bracket(&mut self, content: &str) -> Result<Option<InlineAction>, SandboxError> {
        let hook = &self.funcs.inline_bracket;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "inline-bracket",
            hook,
            &[Val::String(content.to_owned())],
        )?;
        result_inline_action(result, "inline-bracket")
    }

    /// Rewrites one text node's decoded contents, the way a tier-1
    /// replacement does: the engine replaces the node's text with what
    /// this returns.
    pub fn post_process(&mut self, text: &str) -> Result<String, SandboxError> {
        let hook = &self.funcs.post_process;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "post-process",
            hook,
            &[Val::String(text.to_owned())],
        )?;
        result_string(result, "post-process")
    }

    /// Renders one of the component's node kinds, given the attributes
    /// the node carries and the children's rendered HTML. Fails with
    /// the component's reason when it refuses a kind.
    pub fn render(
        &mut self,
        kind: &str,
        attrs: &[(String, String)],
        children: &str,
    ) -> Result<String, SandboxError> {
        let hook = &self.funcs.render;
        let result = Self::call_one(
            &mut self.store,
            self.fuel,
            "render",
            hook,
            &[
                Val::String(kind.to_owned()),
                attrs_val(attrs),
                Val::String(children.to_owned()),
            ],
        )?;
        match result_result_string(result, "render")? {
            Ok(html) => Ok(html),
            Err(reason) => Err(SandboxError::new("render", reason)),
        }
    }
}

/// Lowers one option value onto the wire.
fn option_value(value: &OptionValue) -> Val {
    match value {
        OptionValue::String(text) => Val::Variant(
            "string".to_owned(),
            Some(Box::new(Val::String(text.clone()))),
        ),
        OptionValue::Bool(flag) => {
            Val::Variant("bool".to_owned(), Some(Box::new(Val::Bool(*flag))))
        }
        OptionValue::Number(number) => {
            Val::Variant("f64".to_owned(), Some(Box::new(Val::Float64(*number))))
        }
    }
}

/// Lowers a list of attributes onto the wire.
fn attrs_val(attrs: &[(String, String)]) -> Val {
    Val::List(
        attrs
            .iter()
            .map(|(name, value)| {
                Val::Tuple(vec![Val::String(name.clone()), Val::String(value.clone())])
            })
            .collect(),
    )
}

/// Reads a result whose shape must be a WIT `bool`.
fn result_bool(value: Val, hook: &str) -> Result<bool, SandboxError> {
    match value {
        Val::Bool(flag) => Ok(flag),
        _ => Err(unexpected(hook, "a bool")),
    }
}

/// Reads a result whose shape must be a WIT `string`.
fn result_string(value: Val, hook: &str) -> Result<String, SandboxError> {
    match value {
        Val::String(text) => Ok(text),
        _ => Err(unexpected(hook, "a string")),
    }
}

/// Reads a result whose shape must be a WIT `result<_, string>` — no
/// ok payload, an error string on failure: the component's own error
/// comes back as the returned error's message.
fn result_result_unit(value: Val, hook: &str) -> Result<Result<(), String>, SandboxError> {
    match value {
        Val::Result(Ok(None)) => Ok(Ok(())),
        Val::Result(Ok(Some(_))) => Err(unexpected(hook, "a result with no ok payload")),
        Val::Result(Err(Some(payload))) => Ok(Err(result_string(*payload, hook)?)),
        Val::Result(Err(None)) => Ok(Err(String::from("failed"))),
        _ => Err(unexpected(hook, "a result")),
    }
}

/// Reads a result whose shape must be a WIT `result<string, string>`.
fn result_result_string(value: Val, hook: &str) -> Result<Result<String, String>, SandboxError> {
    match value {
        Val::Result(Ok(None)) => Ok(Ok(String::new())),
        Val::Result(Ok(Some(payload))) => Ok(Ok(result_string(*payload, hook)?)),
        Val::Result(Err(Some(payload))) => Ok(Err(result_string(*payload, hook)?)),
        Val::Result(Err(None)) => Ok(Err(String::from("failed"))),
        _ => Err(unexpected(hook, "a result")),
    }
}

/// Reads a result whose shape must be a WIT `option`.
fn result_option(value: Val, hook: &str) -> Result<Option<Val>, SandboxError> {
    match value {
        Val::Option(inner) => Ok(inner.map(|boxed| *boxed)),
        _ => Err(unexpected(hook, "an option")),
    }
}

/// Reads a result whose shape must be a WIT `record`.
fn result_record(value: Val, hook: &str) -> Result<Vec<(String, Val)>, SandboxError> {
    match value {
        Val::Record(fields) => Ok(fields),
        _ => Err(unexpected(hook, "a record")),
    }
}

/// Reads a result whose shape must be the `inline-action` variant.
fn result_inline_action(value: Val, hook: &str) -> Result<Option<InlineAction>, SandboxError> {
    let option = result_option(value, hook)?;
    option
        .map(|action| {
            let (case, payload) = match action {
                Val::Variant(case, payload) => (case, payload),
                _ => return Err(unexpected(hook, "an inline-action")),
            };
            match (case.as_str(), payload) {
                ("literal", _) => Ok(InlineAction::Literal),
                ("delimiter", Some(payload)) => {
                    let fields = result_record(*payload, hook)?;
                    Ok(InlineAction::Delimiter {
                        kind: record_string(&fields, "kind", hook)?,
                        end: record_u32(&fields, "end", hook)?,
                    })
                }
                ("node", Some(payload)) => {
                    let fields = result_record(*payload, hook)?;
                    Ok(InlineAction::Node {
                        kind: record_string(&fields, "kind", hook)?,
                        attrs: record_attrs(&fields, hook)?,
                        end: record_u32(&fields, "end", hook)?,
                    })
                }
                _ => Err(unexpected(hook, "an inline-action case")),
            }
        })
        .transpose()
}

/// Reads a `consume` variant value: how much of a line a block takes.
fn consume_val(value: Val, hook: &str) -> Result<Consume, SandboxError> {
    let (case, payload) = match value {
        Val::Variant(case, payload) => (case, payload),
        _ => return Err(unexpected(hook, "a consume")),
    };
    match (case.as_str(), payload) {
        ("whole-line", _) => Ok(Consume::WholeLine),
        ("rest", _) => Ok(Consume::Rest),
        ("columns", Some(payload)) => match *payload {
            Val::U32(columns) => Ok(Consume::Columns(columns)),
            _ => Err(unexpected(hook, "a consume column count")),
        },
        _ => Err(unexpected(hook, "a consume case")),
    }
}

/// Reads a record's `consume` field: the one place a decision carries
/// how much of a line a block takes.
fn record_consume(fields: &[(String, Val)], hook: &str) -> Result<Consume, SandboxError> {
    consume_val(record_field(fields, "consume", hook)?, hook)
}

/// Reads a record's string field.
fn record_string(fields: &[(String, Val)], name: &str, hook: &str) -> Result<String, SandboxError> {
    match record_field(fields, name, hook)? {
        Val::String(text) => Ok(text),
        _ => Err(unexpected(hook, "a string field")),
    }
}

/// Reads a record's u32 field.
fn record_u32(fields: &[(String, Val)], name: &str, hook: &str) -> Result<u32, SandboxError> {
    match record_field(fields, name, hook)? {
        Val::U32(number) => Ok(number),
        _ => Err(unexpected(hook, "a u32 field")),
    }
}

/// Reads a record's attribute list field.
fn record_attrs(
    fields: &[(String, Val)],
    hook: &str,
) -> Result<Vec<(String, String)>, SandboxError> {
    let list = match record_field(fields, "attrs", hook)? {
        Val::List(items) => items,
        _ => return Err(unexpected(hook, "an attribute list")),
    };
    let mut attrs = Vec::with_capacity(list.len());
    for item in list {
        let mut pair = match item {
            Val::Tuple(fields) => fields,
            _ => return Err(unexpected(hook, "an attribute pair")),
        };
        if pair.len() != 2 {
            return Err(unexpected(hook, "an attribute pair"));
        }
        let value = result_string(pair.pop().unwrap_or(Val::Bool(false)), hook)?;
        let name = result_string(pair.pop().unwrap_or(Val::Bool(false)), hook)?;
        attrs.push((name, value));
    }
    Ok(attrs)
}

/// Reads one record field by name.
fn record_field(fields: &[(String, Val)], name: &str, hook: &str) -> Result<Val, SandboxError> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.clone())
        .ok_or_else(|| unexpected(hook, &format!("a `{name}` field")))
}

/// The error for a result whose shape does not match the world: only a
/// component built against a different world's types can produce it.
fn unexpected(hook: &str, what: &str) -> SandboxError {
    SandboxError::new(
        hook,
        format!("the component returned an unexpected result: expected {what}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_defaults_to_and_clamps_to_the_cap() {
        assert_eq!(effective_fuel(None), FUEL_CAP);
        assert_eq!(effective_fuel(Some(1_000)), 1_000);
        assert_eq!(effective_fuel(Some(FUEL_CAP)), FUEL_CAP);
        assert_eq!(effective_fuel(Some(FUEL_CAP + 1)), FUEL_CAP);
        assert_eq!(effective_fuel(Some(u64::MAX)), FUEL_CAP);
    }

    #[test]
    fn memory_pages_defaults_to_and_clamps_to_the_cap() {
        assert_eq!(effective_memory_pages(None), MEMORY_PAGES_CAP);
        assert_eq!(effective_memory_pages(Some(20)), 20);
        assert_eq!(
            effective_memory_pages(Some(MEMORY_PAGES_CAP)),
            MEMORY_PAGES_CAP
        );
        assert_eq!(
            effective_memory_pages(Some(MEMORY_PAGES_CAP + 1)),
            MEMORY_PAGES_CAP
        );
        assert_eq!(effective_memory_pages(Some(u32::MAX)), MEMORY_PAGES_CAP);
    }

    #[test]
    fn sandbox_error_display_names_the_hook() {
        let err = SandboxError::new("post-process", "fuel exhausted");
        assert_eq!(
            err.to_string(),
            "sandbox extension hook `post-process`: fuel exhausted"
        );
        assert_eq!(err.hook, "post-process");
        assert_eq!(err.message, "fuel exhausted");
    }
}
