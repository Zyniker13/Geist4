# The Geist4 extension format, version 1

One standard way to describe an addition to Markdown, so any tool using
the engine can check, process and render it. An extension is a
directory (or an archive of one) holding three things:

    my-extension/
      extension.toml   the manifest: identity, kinds, behavior, rules
      spec.md          its examples in the CommonMark example format
      ...              tier 2 only: the WebAssembly component

The same `spec.md` is the extension's conformance test no matter which
tier implements its behavior, and `geist4 ext check <path>` validates
the whole package.

This document specifies format version 1 completely. A loader that
claims format version 1 implements everything here; a manifest that
asks for a format version above the loader's is rejected with the
version mismatch.

## The manifest: `extension.toml`

TOML, with this shape:

```toml
[extension]
name = "highlight"            # required
version = "1.0.0"             # required
format = "1.0"                # required; the format version
description = "Mark text"     # required
license = "MIT"               # required
homepage = "https://..."      # optional

# Which hooks the behavior uses. Informational and checked: the
# behavior tier must actually be able to use what it lists, and
# `ext check` warns about hooks named but unused.
hooks = ["inline_span"]

# Ordering and conflicts against other extensions by name.
# Extensions run in registration order; `after` puts this one after
# the named ones when both load, and `conflicts` rejects a session
# where both load.
after = ["strikethrough"]
conflicts = []

[[kinds]]                     # the node kinds this extension adds
kind = "highlight"            # required, unique across loaded extensions
container = false             # whether nodes hold children (blocks)
inline_text = true            # whether children render as inline content
attrs = []                    # attribute names the kind accepts, in order

[options.key]                 # user-configurable options
type = "string"               # string | bool | number
default = "..."
description = "..."

# --- The behavior, exactly one tier ----------------------------------

[[inline_span]]               # tier 1: declarative inline spans
kind = "highlight"
open = "=="
close = "=="
html = "<mark>{children}</mark>"

# [[block_container]]         # tier 1: declarative fenced containers
# [[replace]]                 # tier 1: declarative replacements
# [[attribute_list]]          # tier 1: declarative attribute blocks
# [wasm]                      # tier 2: path = "component.wasm",
#                             #   fuel = 10_000_000, memory_pages = 16,
#                             #   inline_triggers = ["%"]
# [host]                      # tier 3: the host supplies callbacks
```

Rules the loader enforces:

- `name` matches `[a-z][a-z0-9-]*`, 1–64 bytes; it is unique among
  loaded extensions, and the core built-ins' names are reserved
  (`tables`, `strikethrough`, `autolink`, `tasklist`, `tagfilter`,
  `footnotes`, `front_matter`, `heading_identifiers`,
  `description_lists`).
- `version` and `format` are semver `X.Y[.Z]`. The loader accepts a
  format whose major version matches its own and whose minor is not
  greater; a different major is rejected.
- `license` is an SPDX expression string; the loader checks it is
  present and well-formed, not which license it is.
- `hooks` names come from the fixed vocabulary in
  [Hooks](#hooks); unknown names are rejected.
- `kinds[].kind` matches `[a-z][a-z0-9-]*` and is unique among the
  manifest's kinds and across loaded extensions. `attrs` names are
  `[a-z][a-z0-9_-]*`.
- Every kind a behavior entry references must be declared in `kinds`;
  every kind must be referenced by exactly one behavior entry.
- `options` names are `[a-z][a-z0-9_-]*`, unique; at most 32 options;
  each has a `default` of its declared type. Values the user sets are
  type-checked against the declaration.
- The manifest is at most 64 KiB; a package holds at most 64 kinds and
  at most 256 behavior entries.

## Hooks

The vocabulary for `hooks`, and what each implies:

| hook name | trait hook | implied by |
| --- | --- | --- |
| `front_matter` | `front_matter_delimiter` | (tier 3 only in v1) |
| `inline_span` | `inline_triggers` + `inline_trigger` | `[[inline_span]]` |
| `inline_bracket` | `inline_bracket` | `[[wiki_link]]` |
| `block_container` | `try_start`/`try_continue`/`close`/`block_start_looks` | `[[block_container]]` |
| `post_process` | `post_process` | `[[replace]]`, `[[attribute_list]]` |
| `render` | `render` | every pattern with a template |
| `html_filter` | `filters_html`/`html_filter` | `[[html_filter]]` |

## Tier 1: the declarative definition

Declared entirely in the manifest, interpreted by the engine, so it
behaves identically in every binding — Rust, WebAssembly in a browser
or edge runtime, Swift, Python. The engine compiles each entry onto
the public extension API; nothing about the tier is special to the
parser.

### `[[inline_span]]` — delimited inline spans

Wraps the inline content between an opening and closing delimiter in
one of the extension's node kinds: `==highlight==`, `[[wiki links]]`,
`$x + 1$`.

```toml
[[inline_span]]
kind = "highlight"        # from [[kinds]]
open = "=="               # 1..16 bytes
close = "=="              # 1..16 bytes
html = "<mark>{children}</mark>"
can_open = true           # optional; default true
can_close = true          # optional; default true
multiline = true          # optional; default true: the span may cross soft breaks
math = false              # optional; default false
```

Semantics, exactly:

- The span is found during inline parsing at the open delimiter's
  first byte (the trigger), like the emphasis machinery: the engine
  registers the open delimiter's first byte as the extension's inline
  trigger.
- `can_open`/`can_close` are the CommonMark flanking rules applied to
  the delimiter: an opener cannot follow whitespace, a closer cannot
  precede whitespace, and a delimiter that is both punctuation-flanked
  acts per the rule of three. A one-byte delimiter behaves like `*`;
  a two-byte delimiter is a single unit — runs are not counted, so
  `====x====` opens with the first `==` and closes with the last.
- The delimiter pair is escaped by backslash like emphasis: `\==` is
  literal `==`. Escaped openers do not open; escaped closers do not
  close.
- With `multiline = false` the span must close on its line (a soft
  break does not sit inside it).
- With `math = true` the content is additionally checked like inline
  math: no leading or trailing space unless it is the only character,
  and a closing delimiter preceded by whitespace (or a digit preceded
  by `$` for the open) does not close. This is the CommonMark
  discussion-list math rule, matching the KaTeX-aware renderers.
- The first close after the open (respecting the rules above) wins;
  no nesting of the same pair. When nothing closes, the opener is
  literal text.
- Unmatched opens nest with themselves only through the standard
  emphasis machinery — there is none: the span is atomic. Two
  overlapping candidates resolve left to right.
- The node wraps the span's inline content as children
  (`inline_text` kinds), and renders as `html` with `{children}`
  replaced by the children's HTML and `{name}`-substitutions for each
  declared attribute the node carries. The template is at most 1 KiB
  and must be valid as the engine writes it (it is emitted literally
  except for the substitutions); no `{{`/`}}` escaping is provided.
- A template with no `{children}` on an `inline_text` kind is an
  error (the children would vanish); on a non-inline kind it is the
  empty-content case and allowed.

### `[[block_container]]` — fenced containers

A fence line opens a container that holds blocks until a matching
close fence or the document end: `:::note` … `:::`.

```toml
[[block_container]]
kind = "note_container"   # from [[kinds]]; must be a container kind
marker = ":::"            # 3..16 bytes, one char class repeated (like a fence)
name = "note"             # required: the opening word after the marker
info_html = ""            # optional: template for the info line's content
html = "<div class=\"note\">{children}</div>"
unclosed_ok = true        # optional; default true: EOF closes it
```

Semantics, exactly:

- A line whose content begins with the marker, optionally indented
  up to three spaces, then (after the marker run) optional spaces and
  the `name` word, opens the container. The rest of the line after
  the name is the info string, inline-parsed and rendered through
  `info_html` (empty by default, meaning the info is ignored).
- The close fence is a line whose content begins with the same
  marker run length, optionally indented up to three spaces, with
  nothing after it but spaces. A shorter or longer marker run does
  not close. The close fence is consumed.
- Between open and close (or EOF when `unclosed_ok`), ordinary block
  parsing continues inside the container: paragraphs, lists, other
  extensions' containers, including the same one (nested by a fresh
  match — the close fence closes the innermost open container of
  that name).
- The marker acts like a fenced code fence for interrupt purposes:
  it interrupts a paragraph. It does not look like anything inside a
  paragraph's lazy continuation: a lazy line joins the paragraph.
- The node renders as `html` with `{children}` replaced by the
  children's block HTML and `{info}` by the rendered info (empty when
  none).
- The `close` fence when missing at EOF with `unclosed_ok = false`
  is an error surfaced with a source span at the open fence.

### `[[replace]]` — literal replacements

A post-process pass replacing literal text with HTML or an entity.

```toml
[[replace]]
from = "<-"               # 1..32 bytes, UTF-8, no newline
to = "&larr;"             # 1..128 bytes of literal output
once = true               # optional; default true: at most once per text node
```

Semantics, exactly:

- Applied during post-processing to every text node's decoded
  contents, before autolinks' email pass if that extension is loaded
  after this one (registration order governs).
- The scan is a plain literal search over the text node's bytes; it
  never crosses a node boundary; it never matches inside raw HTML or
  code spans or code blocks (text nodes inside those are untouched —
  the engine's text nodes there are marked raw).
- Replacement output is emitted verbatim, entity-encoded or not, as
  the extension wrote it.
- At most 512 entries per extension; entries are applied in
  declaration order, each on the result of the previous.

### `[[attribute_list]]` — attribute blocks

Pandoc-style attribute lists attaching to the previous block or the
preceding inline span.

```toml
[[attribute_list]]
kind = "attr_target"      # unused kind is an error; see below
prefix = "{:"             # required; must start with "{"
suffix = "}"              # required
```

Semantics, exactly:

- A paragraph-final list on its own line — the line's content being
  exactly a list — attaches to the preceding sibling block: any
  core block or extension node. It sets the node's attributes by
  merging: `#id` sets `id`, `.class` appends to `class`,
  `key=value` sets `key` (value is an entity-decoded string), `key`
  alone sets `key=""`. A list that parses is consumed; one that does
  not (bad syntax, unknown key shape) stays literal text.
- The `kind` is the kind whose renderer receives the attributes when
  the target is an extension node; core nodes take the attributes
  into their own rendering (`id`/`class` on a heading or block). The
  loader maps the declared kind to the pattern's merge behavior.
- An inline list — `span{.class}` with the list immediately after an
  inline span's closing delimiter — attaches to that span.
- The list grammar: `{:` then a sequence of identifiers and
  `key=value` pairs separated by spaces, then `}`; identifiers and
  keys are `[A-Za-z:._-]+`; values may be quoted with `"` (no
  escapes). At most 32 entries per list.

### `[[wiki_link]]` — bracket links

A `[[...]]` (or other double-bracket) link producing a node, for
`[[Page name]]` → a link to another document.

```toml
[[wiki_link]]
kind = "wiki"
prefix = "[["
suffix = "]]"
html = "<a href=\"{target}\" class=\"wikilink\">{children}</a>"
```

Semantics, exactly:

- Found at the `[` trigger: when the bracket group's content starts
  with the prefix's remainder (`[`), the whole group becomes the
  node; `content` is split at the first `|` into target and children
  (`children` defaults to the target). The engine's existing
  `inline_bracket` hook does the claim; the tier merely declares the
  shapes.
- `html` substitutes `{target}` (percent-encoded with `#` and `?`
  kept as `%23`/`%3F` per the href rules) and `{children}` (the
  inline-parsed children).

### `[[html_filter]]`

```toml
[[html_filter]]
deny = ["script", "iframe", "style"]
```

The output-filtering hook with the same semantics as the built-in
`tagfilter`: each listed tag start renders as `&lt;`. At most 64
entries, each `[a-zA-Z][a-zA-Z0-9-]*`.

### What tier 1 cannot express

No hooks other than the patterns above: no custom block-state
machines, no inline span with a computed body, no tree surgery beyond
the attribute list, no conditionals beyond the flags given. Anything
beyond the tier's reach needs tier 2 or 3. This keeps the tier
small enough to implement identically everywhere — including
JavaScriptCore inside an app, where no sandbox can run.

## Tier 2: the WebAssembly component

For logic the declarative tier cannot express, where the host can run
a sandbox (native builds and the CLI). The package carries a
WebAssembly component built against the `geist4:extension` world:

```wit
package geist4:extension@1.0.0;

interface hooks {
  // The manifest's kinds, restated for the engine's cross-check.
  record kind-decl { kind: string; container: bool; inline-text: bool; attrs: list<string> }

  // Block start: given the line from the content position, whether it
  // is indented four columns, and the line's column, decide.
  enum start-decision { none, open, convert-paragraph }
  record block-start {
    decision: start-decision,
    kind: option<string>,       // the kind to open; none for convert
    attrs: list<tuple<string, string>>,
    consume: consume,           // how much of the line the block takes
  }
  enum consume { rest, columns(u32), whole-line }
  ...
}
```

The boundary is decision-shaped, not tree-shaped: hooks return what to
do, and the engine — which owns the arena — performs it. This keeps
the interface small, keeps the sandbox from ever touching the tree,
and lets tier 3 (host callbacks in JavaScript, Swift, Python) use the
same shapes. The full WIT file is checked in at
`crates/geist4-ext-loader/wit/geist4-extension.wit` and is normative;
this section describes its shape and the rules around it.

- The component exports `init(kinds, options) -> result<_, error>`,
  the block hooks (`block-start-look`, `block-start`,
  `block-continue`, `block-close`), the inline hooks
  (`inline-trigger`, `inline-bracket`), `post-process`, and `render`.
- Every call is made with a fuel limit and a memory limit, both set
  from the manifest (`[wasm] fuel = 10_000_000`, `memory_pages = 16`,
  with loader caps of 1e9 fuel and 256 pages). Running out of either
  is a defined error carrying the hook's name, surfaced as a parse
  problem with a source span — never a hang or an abort the host
  cannot report.
- The sandbox gets no imports beyond the interface: no file system,
  no network, no clock, no randomness. Extensions see only the bytes
  and state the hooks hand them.
- The engine learns the component's inline trigger bytes from the
  manifest's `[wasm] inline_triggers` — a list of one-byte strings,
  at most 32 of them, none of them a core trigger byte (`\`, `` ` ``,
  `&`, `<`, `*`, `_`, `[`, `!`, `]`), which the loader rejects. At
  each registered byte the engine calls `inline-trigger` in
  registration order; several extensions may share a byte.
- Hooks' state is the component's own memory, per parse; the engine
  holds an opaque store handle across a document's calls and drops it
  after.
- `render` returns the HTML for its node kinds given the children's
  rendered HTML (and per-child kind information), as the declarative
  templates do.
- The engine cross-checks the component's exported kinds against the
  manifest's `[[kinds]]` at load: a mismatch is a load error.

## Tier 3: host callbacks

The same manifest and `spec.md`, with behavior supplied as callbacks
from the binding's language through the shapes tier 2 uses. The
binding (JavaScript, Swift, Python) maps the WIT records onto its
language's objects; the engine calls it exactly where it would call
the component. Tier 3 exists for hosts that cannot run a sandbox
(the engine's own WebAssembly build) or need application data —
resolving a wiki link to a real document, say. A tier-3 extension
declares `[host]` and no behavior entries; the host registers the
callback object at load time. The hooks vocabulary is the same as
tier 2's, so one manifest describes both.

## `spec.md`

The extension's examples, in the CommonMark example format:

    ```````````````````````````````` example
    input
    .
    expected HTML
    ````````````````````````````````

- Run by `ext check` with the extension loaded and its declared
  options defaulted. Examples that show ordinary Markdown unchanged
  belong here too, so the format's rule "an extension never changes
  plain Markdown unless its pattern matched" is tested by the
  package itself.
- The file may carry HTML comments for notes; example sections
  separated by headings are fine (the harness splits on the fences).
- `ext check` reports each failing example with its index and the
  diff, and fails with a nonzero exit code.

## Validation order and messages

`ext check <path>` runs, in order, stopping at the first failure:

1. Read `extension.toml`; parse as TOML; validate the schema — every
   rule in [The manifest](#the-manifest-extensiontoml) above, with
   the offending key's name in the message.
2. Validate the behavior entries: referenced kinds exist, each kind
   is used exactly once, templates' substitutions name declared
   attributes (or `children`/`info`/`target` as the pattern allows),
   delimiters and markers are in range, filters are well-formed.
3. For tier 2: the component file exists, is a valid component
   binary, exports the world's required functions, and its kinds
   match the manifest.
4. Conflict check against the other extensions loaded in the same
   session (names, kinds, options).
5. Run `spec.md` through the engine with the extension loaded.

Messages are one line each: what failed, which key or example, and
what would satisfy the rule. Exit codes: 0 pass, 1 fail, 2 usage
error.

`ext manifest PATH` runs steps 1–3 only — the manifest and behavior
are validated and a tier-2 component is loaded, but the spec is not
read. It is the validation half of `ext check`, and the command the
conformance kit's manifest cases are checked with.

## The conformance kit

The corpus under `conformance/` in the engine's repository is the
format's conformance suite. Manifest cases state the outcome a
conforming loader must produce (`ok` or `error`, with a stable reason
token); declarative behavior packages pin each tier-1 feature's
semantics with `spec.md` examples that any implementation must render
byte for byte. Any implementation can run the corpus against its own
loader and engine; see `conformance/README.md`.

## Determinism and purity

Everything in the format is deterministic: no clock, no randomness,
no file or network access anywhere — including tier 2 (no imports)
and tier 3 (the host's own rules must supply only pure callbacks;
the loader cannot enforce it there, but the spec file's run may
differ if it does, and that is the host's bug to find).

## Limits

The loader enforces, at load time, with errors — the numbers are v1
normative:

| what | limit |
| --- | --- |
| manifest size | 64 KiB |
| kinds per extension | 64 |
| behavior entries per extension | 256 |
| replacements per extension | 512 |
| delimiters/marker bytes | 1–16 (span), 3–16 (container marker) |
| template size | 1 KiB |
| options | 32 |
| attribute lists per document | unbounded (per-list 32 entries) |
| wasm fuel | 1e9 (loader cap; the manifest may ask for less) |
| wasm memory | 256 pages (loader cap; the manifest may ask for less) |
| spec.md size | 4 MiB |
