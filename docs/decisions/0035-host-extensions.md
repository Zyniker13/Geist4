# 0035 — Host extensions (tier 3) and render overrides across the bindings

## Context

M7 requires host extensions (the format's tier 3) and render overrides
working from JavaScript, Swift and Python, each with an example. The
format (`docs/extensions/format.md`, "Tier 3: host callbacks") fixes the
contract: the same manifest and `spec.md` as the other tiers, behavior
supplied as callbacks from the binding's language through the shapes
tier 2 uses; the engine calls the host exactly where it would call a
tier-2 component. The loader already validates the `[host]` tier marker
and the shared tables (`[[kinds]]`, `[options.*]`) every tier uses.

The core's `Extension` trait is complete, including `render_override`
for core nodes claimed during parsing via `Arena::set_extension`. The
tier-2 adapter (`geist4-ext-loader`) maps the WIT world's hooks onto
that trait; its decision mappings, latch semantics and declines are the
tested reference for what tier 3 must do. Decision 0028 makes
`geist4-ffi` the only crate where `unsafe` is allowed, and the safe
module is the surface every binding builds on. Swift speaks only the C
ABI; Python (PyO3) and JavaScript (wasm-bindgen) each compile Rust code
of their own inside the wheel or the wasm module.

## Decision

**One vocabulary, three encodings.** Tier 3 supports exactly the tier-2
WIT hook set — `block-start-look`, `block-start`, `block-continue`,
`block-close`, `inline-trigger`, `inline-bracket`, `post-process`,
`render` — plus `render_override`, which the WIT world does not expose
but the trait does and M7 requires. The declared data (kinds and their
attrs, the extension's name, the inline trigger bytes) is static
registration data, not callbacks, mirroring how the manifest carries it
for the other tiers. The trait hooks the vocabulary does not cover
(`front_matter_delimiter`, `begin_render`, `document_suffix`,
`html_filter`) keep their neutral defaults in tier 3 for v1; widening
the surface is a later, separate decision.

Two vocabulary extensions beyond tier 2's WIT, both small and both
needed to reach M7's bar:

- **A `claim` consume on `block-start`.** The task-list pattern claims
  a core node the parser already opened: the built-in consumes three
  bytes, marks the list item with `Arena::set_extension` and
  `set_attr`, and opens no block; the render override then renders the
  item. The tier-2 decision has no such action — its three consumes
  all open extension blocks. Tier 3's `block-start` decision gains a
  fourth consume, `claim(bytes)`: the engine consumes the bytes off
  the line, claims the deepest plain container (`container_node`),
  sets the decision's attributes on it, and opens nothing. A claim
  with no plain container open is a malformed decision and latches,
  like an undeclared kind. This is the tier-3 route to
  `render_override`.
- **Typed attribute values.** Tier-2 decisions carry every attribute
  value as a string. Tier 3's decisions and render hooks carry a tag
  per value — string, integer, boolean — because a claim that sets a
  boolean (the task list's `checked`) should not have to round-trip
  through text. The render hook still sees the display form, as
  tier 2 does; only decisions set typed values.

Option declarations are accepted in a tier-3 manifest's shared tables
but the bindings' programmatic registration in v1 declares no options
and sets no values; option plumbing arrives with package loading in
the bindings, which is future work.

**Registration through the safe module.**
`safe::Options::enable_host(Arc<dyn Extension>)` is the one
registration point for every binding, with the loader's conflict rules:
the name must match `[a-z][a-z0-9-]*`, must be unique among the
registered extensions, and may not collide with a built-in's reserved
name. Kinds are validated at registration (name pattern, non-empty
attr names); trigger bytes must be outside the core trigger set. A
decision naming an undeclared kind latches exactly as tier 2 latches.

**Swift and C get a vtable in `geist4-ffi::host`.** The host fills a
`#[repr(C)]` table of `extern "C"` function pointers with a context
pointer; `HostExtension` implements the `Extension` trait by calling
the table. All `unsafe` stays inside `geist4-ffi` (decision 0028).
Memory follows the C ABI's conventions: the engine's inputs are
borrowed for the duration of the call; the host's outputs are copied by
the engine before the call returns, so the host may hand back pointers
into its own short-lived storage. The children of a claimed core node
are pulled through an engine-provided fetch callback with
`OverrideChildren::render_next` semantics, one child per call, in
order.

**Python and JavaScript bind natively.** Each of those crates
implements the `Extension` trait directly over its own callables —
PyO3 Python objects, wasm-bindgen JS closures — with no C marshalling
in between. The semantics are the same vtable-for-vtable; only the
encoding differs, exactly as each binding already mirrors the safe
module's API in its own language.

**Semantics mirror tier 2 exactly.** The decision mappings are the
tier-2 adapter's, applied hook for hook: `block-start` opens with
`open_extension` (`whole-line`) or `open_extension_keep_rest` after
`seek_content` (`columns(n)`, `rest`), interrupting rather than
converting a paragraph; `block-continue` maps `keep` to the consume and
`close-line` to close-and-consume, with a declined or failed call
closing the block; `block-close` HTML replaces the node's rendering as
a newline-terminated raw HTML block; `inline-trigger` may only hand
back a delimiter run (the trait has no whole-span claim there);
`inline-bracket` claims the group with the decision's `end`;
`post-process` walks text nodes outside code and raw HTML; `render`
sees attribute values as strings and the children's concatenated HTML.
The first hook failure — an error status, a malformed decision, or a
panic in the callback — latches: every later hook declines, the
`take_error` message names the hook, and the error surfaces as a parse
problem, exactly as tier 2's traps do.

**No fuel, no memory limit.** Tier-3 callbacks run in the host's own
language, outside any sandbox; there is nothing to meter. The format's
purity rule applies: the host supplies only pure callbacks, and a host
that hangs hangs the parse — the same contract as any callback API, and
the format assigns that risk to the host.

**Examples share one `spec.md`.** The tier-3 example package carries an
`extension.toml` with `[host]` and the shared tables, and a `spec.md`
in the CommonMark example format. Each binding's test registers the
same declared data and callbacks programmatically and runs that one
`spec.md` byte for byte, so the vocabulary cannot drift between
bindings without a test failing. Loading extension packages from the
bindings themselves (manifest parsing outside Rust) is future work;
the Rust loader already validates the manifest.

## Consequences

- Every binding speaks the same hook vocabulary, so one `spec.md` and
  one example pin the behavior across all three languages and the
  cross-binding suite (M7) extends to tier 3 for free.
- `unsafe` stays confined to `geist4-ffi`; the Python and wasm
  adapters, like the bindings themselves, compile under the workspace
  `forbid(unsafe_code)`.
- Three encodings of one vocabulary can drift; the shared `spec.md`
  run through every binding is the guard, and the m7-6 cross-binding
  suite repeats it for the built-ins.
- Tier-3 callbacks are outside the sandbox by design: the format gives
  the host the responsibility for purity and termination, and the
  engine's own invariants hold because a callback can only answer
  decisions — it never touches the tree.
- The vtable gives any C-ABI consumer (not only Swift) the full tier-3
  surface, which keeps the C ABI the universal floor.
- The C header spells each hook's signature out in the vtable fields
  (`Option<unsafe extern "C" fn(...) -> i32>`, which cbindgen renders
  as a nullable function pointer) instead of referring to the
  `Geist4Hook*` type aliases: cbindgen renders an
  `Option<alias-to-fn-pointer>` field as a forward-declared opaque
  struct, and a header full of undefined `Option_Geist4Hook*` types
  does not compile. The aliases stay as the documented shapes,
  re-exported from `safe` as the C ABI's named function-pointer types
  for any Rust host speaking the C ABI.
