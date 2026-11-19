# Security model

Geist4 turns untrusted Markdown text into HTML. This document states the
threat model, every security-relevant default and why it is set, the
boundaries the engine draws — reviewed again entry point by entry point
at the 0.1.0 gate — and the residual work that stays with the caller.
Nothing here may change silently: a change to any default below requires
a new decision record and an update to this file.

## Threat model

An embedding application renders text from untrusted authors (chat
messages, issue bodies, comments) and shows the result in a browser or
web view. The attacker controls the Markdown source and wants, in
increasing order of value:

1. **Broken pages** — output that corrupts the host page's markup
   (unbalanced tags that swallow following content).
2. **Content injection** — arbitrary HTML or attribute values in the
   output that the host application did not intend to allow.
3. **Script execution** — active content (`javascript:` URLs, event
   handlers, raw `<script>`) that runs in the page's origin.
4. **Resource exhaustion** — inputs whose parse time or memory use
   explodes, denying service to the embedding process.

A second attacker appears with loadable extensions: whoever authored a
package a host loads. That code is *not* attacker-controlled source
text; it runs inside the engine's extension sandbox, and the threat is
that it escapes the limits the sandbox promises (see "The Wasm
sandbox").

Geist4 is a *library*: it parses and renders, it does not sanitize to a
specific application policy and it does not serve the output. The
boundary this document draws is between what the engine guarantees and
what the host must still do.

## What the engine guarantees

- **Memory safety.** The workspace compiles with `unsafe_code = "forbid"`
  — with one carve-out: the C ABI crate `geist4-ffi` (decision 0028),
  where `unsafe` is confined to the `extern "C"` boundaries. Every one
  of those entry points validates its raw arguments before use (null
  pointers → `GEIST4_ERR_NULL`, lengths checked, UTF-8 validated) and
  wraps its body in `catch_unwind`, so no Rust panic and no invalid
  pointer dereference can cross the ABI. No other crate in the tree
  contains `unsafe`.
- **Bounded work.** The parser and renderer contain no recursion, no
  regular expressions, no global state and no quadratic-or-worse
  algorithms on adversarial input. The pathological suite
  (`crates/geist4-core/tests/pathological.rs`) ports the reference
  implementation's pathological inputs — deep nesting, delimiter runs,
  unclosed brackets, huge reference tables — and requires linear
  scaling, measured as time at n versus 10n. Deep-nesting inputs that
  overflow the call stack in naive implementations (the same suite runs
  nesting 10,000 deep) complete normally.
- **No panics on arbitrary input.** Property tests
  (`crates/geist4-core/tests/properties.rs`) feed generated inputs to
  the parser and renderer and assert no panic and deterministic output;
  the FFI and wasm-bindgen surfaces catch any panic that slips through
  and report it as an error value instead of unwinding into the host.
- **Spec-conformant escaping.** Text and attribute values are escaped
  per the CommonMark HTML rendering rules; NUL bytes are replaced with
  U+FFFD before inline processing, as in the reference parser.

## Defaults and why

### URL policy (safe destinations)

Link and image destinations are filtered by the renderer
(`basic_html::is_dangerous_url`). The default drops:

- `javascript:` and `vbscript:` — script URLs;
- `file:` — local resource access;
- `data:` — except `data:image/png`, `data:image/gif`,
  `data:image/jpeg` and `data:image/webp`, which are kept because
  embedded raster images are the one `data:` use with legitimate
  Markdown-document value and no script capability.

Matching is byte-wise ASCII prefix comparison with no Unicode
normalization and no case folding: a destination is dangerous only when
it *literally* begins with one of the schemes above, so look-alike
Unicode confusables, percent-encodings and case variants are not
smugged past the filter — they are simply not matched, and are treated
as relative URLs. This is deliberately more conservative than scheme
normalization: anything that is not a known-safe absolute scheme
(including `https:` and `http:`, which are unconditionally allowed)
renders as a relative link, and a host that wants scheme allow-listing
can add it on top.

Relative URLs and fragments are allowed. URLs are percent-escaped for
attribute context; `&` and `'` are entity-escaped, so attribute
injection through a crafted URL is not possible.

Dropping is silent (the URL renders empty), matching the reference
ecosystem's behavior. The rule itself is not public API in 0.1.0: a
host that needs user-visible stripping replicates the check on source
destinations, or waits for the planned policy options.

### Raw HTML

The HTML renderer's default is **allow**: raw HTML in the source — HTML
blocks and inline tags — passes through to the output byte for byte.
This is required for CommonMark conformance (the specification's HTML
examples are byte-for-byte test cases; a default that escaped or
dropped raw HTML could not pass the 652-example suite) and it is what
hosts that want raw HTML expect.

Because raw HTML is the classic script-execution vector, hosts
rendering untrusted text must pair the engine with a sanitizer, or use
a non-allow raw-HTML policy. Escape (render the markup as visible text)
and omit (drop it silently) policies are planned as renderer options
after 0.1.0; the *default stays* `allow` so that
conformant output is what you get with no configuration. Until the
policy options exist, the only shipped renderer is the conformant one —
the safe-path instruction for untrusted input is: sanitize the output
(or run a host-side filter) exactly as you would for any other
CommonMark renderer.

### Limits

The engine imposes no nesting-depth or document-size limits of its own:
limits that truncate structure change rendering output, and the project
never trades a spec example for a limit. The guarantees instead are
linearity (above) and the absence of recursion, so memory use is
O(input) up to a constant factor and no input depth overflows the
stack. Embeddings that need hard caps — for example a browser binding
with a fixed fuel budget — should size-limit the input string
themselves; the options surface carries extension selection only, with
no depth or size option in 0.1.0. The
decision to keep the default unlimited is recorded in decision 0010.

Two places do carry hard caps because they bound *engine-internal
work*, not the host's document:

- the inline machinery: a link label is at most 1000 bytes, a backtick
  run at most 1000 characters, an entity name at most 32 characters
  (`crates/geist4-core/src/inline/`);
- the extension hook surface: a line opens at most 16 extension blocks
  (`EXTENSION_OPENS_PER_LINE` in the block parser), so a hostile
  extension cannot spin the parse by re-opening itself endlessly.

## The boundaries, re-checked at the 0.1.0 gate

Every public entry point was read again with this threat model in hand.
What each boundary guarantees, and the tests that hold it:

### The Rust core (`geist4-core`)

The parse/render surface is pure computation: no I/O, no environment
access, no threads, no globals. Everything a render outputs derives
from the input text plus the registered extensions. The M7 span work
(`CloseCtx::take_text_map`/`set_text_map`, `Arena::iter`) added no new
inbound data: it moves engine-internal bookkeeping between nodes, and
the corpus-wide span-invariant test (validating every exported tree in
the 1158-case cross-binding corpus) fails the build if any span leaves
the source range.

### The declarative extension tier (the package manifest)

A third-party extension declares itself in a TOML manifest, and the
loader validates the manifest before anything loads: the name must
match `[a-z][a-z0-9-]*` (1–64 bytes); the nine built-in extension
names are reserved and rejected; kinds must be unique within the
manifest and each kind must be served by exactly one behavior entry;
trigger and attribute declarations are shape- and size-checked
(a `[[wiki_link]]` prefix/suffix is 1–16 bytes, and similar caps for
the other declarations); `[wasm] fuel` and `[wasm] memory_pages` must
be positive numbers. A rejected manifest is a load error naming the
offending field — never a silent default. On top of the per-package
checks, the CLI's package loader rejects duplicate extension names
across the loaded set and enforces each package's declared
`conflicts` before any parse runs.

### The Wasm sandbox (`geist4-ext-loader`)

Third-party extension packages are the untrusted-code boundary. The
loader:

- accepts only WebAssembly components: the wasmtime linker used for
  instantiation defines **no host functions at all**, so a component
  that imports anything fails to instantiate — the guest cannot call
  out: no filesystem, no network, no clock, no environment;
- runs every hook under a **fuel limit per call** (the manifest's
  `[wasm] fuel`, capped by the loader at 1,000,000,000 units) — a
  spinning or looping guest is stopped, not hung;
- enforces a **memory cap** (the manifest's `[wasm] memory_pages`,
  capped at 256 pages, 16 MiB) — an allocating guest traps instead of
  exhausting the host process;
- validates the manifest before anything loads: format version (same
  major, no greater minor), positive fuel/pages, kind and attribute
  shapes, reserved names;
- reports a guest trap as an extension error the document records —
  the host process never crashes on misbehaving guest code.

The hostile-fixture battery pins this: `hostile.wasm` (burns fuel),
`nohooks.wasm` (exports nothing the ABI wants), `pollution.wasm`
(pads memory), `minimal.wasm` (well-behaved control) in
`crates/geist4-ext-loader/tests/`, plus the manifest-rejection tests
and the `parse_manifest`/`load_component` fuzz targets in the fuzz
crate. A host that loads packages should still load only packages it
chose to trust: the sandbox bounds what a guest can *do*, not whether
its output is *good*.

### The C ABI (`geist4-ffi`)

Eleven C entry points: ten `unsafe extern "C"` functions
(`geist4_version`, `geist4_options_free`,
`geist4_options_enable_builtin`, `geist4_options_enable_host`,
`geist4_parse`, `geist4_document_html/problems/free`,
`geist4_render_html`, `geist4_string_free`) plus
`geist4_options_create`, which takes no pointers and is a plain
`extern "C"`. Each one:

- treats a null pointer as an error value (`GEIST4_ERR_NULL`), never a
  dereference;
- validates UTF-8 on every input string before handing it to the core;
- delegates to the `safe` module's internals, which run inside
  `catch_unwind` — the workspace keeps `panic = "unwind"` for exactly
  this — mapping a panic to `GEIST4_ERR_PANIC` and a message buffer,
  so nothing unwinds into a C, Swift, Python or JavaScript caller;
- returns owned allocations (`Geist4String`, document handles) that
  the host releases with the matching `*_free`, each of which accepts
  null and tolerates a double free.

The `safe` module exposes the same behavior over plain Rust types for
the wasm-bindgen layer and tests, so every binding shares one reviewed
boundary instead of each building its own.

### The bindings (Swift, JavaScript, Python, C)

Each binding is a thin translation layer over the FFI or the safe
module: it converts types, never parses, and adds no `unsafe` of its
own. Every input crosses as a JavaScript string, and wasm-bindgen
copies the string's bytes into WebAssembly memory at the call
boundary — no shared buffer or typed-array view can be detached or
mutated while the engine runs.

### The tree exporters (`geist4-mdast`, `geist4-json`, `geist4-writer`)

All three are pure functions of the parsed tree and source. The JSON
exports go through `serde_json`'s escaping; mdast positions are plain
integers. The writer emits Markdown *source* — text a later `parse`
re-reads; it carries no execution semantics of its own, and its
escaping is verified by the 652-example round-trip and idempotence
gates. No exporter executes anything, reads anything, or touches the
network.

### The CLI (`geist4-cli`)

Reads the one file named on the command line, writes the result to
stdout, and loads extension packages only when `--ext` names them —
through the full sandbox above. No network access, no shell-outs, no
writes outside stdout.

## What a caller must still do

- Sanitize raw HTML in the output (or filter the input) when the source
  is untrusted. The engine's job is conformance plus a safe URL policy;
  full sanitization to a host-specific policy is not.
- Apply a Content-Security-Policy to the pages that show the output.
  CSP is the structural backstop for script execution; URL filtering
  alone is not.
- Size-limit inputs before handing them to the engine if the host needs
  a cap (a chat message is not a book).
- Only load extension packages you chose to trust. The sandbox bounds
  compute and memory and removes all ambient authority, but it cannot
  judge whether a package's *output* is appropriate for your page.
- Keep context in mind: the HTML renderer produces fragment HTML, not a
  document; the host supplies the surrounding page and headers.

## Testing that backs this document

- `crates/geist4-core/tests/pathological.rs` — linear scaling on 14
  adversarial families, 10,000-deep nesting, fixed pathological cases.
- `crates/geist4-core/tests/properties.rs` — no panics, deterministic
  output, balanced structural tags, span bounds — 512 generated cases
  per property.
- `crates/geist4-core/tests/differential.rs` — 400 generated documents
  byte-identical to the reference implementation.
- `crates/geist4-core/tests/regressions/` — one file per bug found,
  with the reproducing input and the exact expected output.
- `crates/geist4-ext-loader/tests/` — the hostile-fixture battery,
  manifest-rejection tests, adapter failure paths.
- `crates/geist4-ffi` contract tests — null-pointer, invalid-UTF-8 and
  double-free paths on every entry point; the fuzz crate covers the
  manifest parsers and FFI input decoders.
- `crates/geist4-json/tests/schema.rs` — every span in every exported
  tree stays inside the source, across the whole 1158-case corpus.

Any change that weakens a guarantee above must come with a failing test
first — the suite is the enforcement mechanism for this document.
