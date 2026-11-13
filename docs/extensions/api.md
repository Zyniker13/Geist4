# The extension API

This guide describes how to extend Geist4 from Rust. Loaded
extensions in the standard format (see `docs/extensions/format.md`,
milestone M6) compile down to exactly these hooks; the built-in
extensions in the `geist4-ext` crate use only this public API, so what
works here works everywhere.

Parse and render with extensions through `Options`:

```rust
use geist4_core::{parse_with, render, Options};

let mut options = Options::new();
options.extension(geist4_ext::gfm());
let doc = parse_with(src, &options);
let html = render_with(&doc, &options);
```

With no extensions registered, parsing and rendering are exactly the
plain CommonMark behavior; nothing in this guide can change that.

## What an extension is

An extension is any value implementing `geist4_core::extension::Extension`.
All hooks have default implementations, so an extension enables only
what it needs:

| hook             | runs                                                     | used by                     |
| ---------------- | -------------------------------------------------------- | --------------------------- |
| `declare`        | at registration: names the node kinds and options it adds | every extension             |
| `try_start`      | per line, after the built-in block scanners fail          | tables, front matter, definition lists |
| `try_continue`   | per line, for its open block frame                        | front matter, definition lists |
| `close`          | when its open frame closes                                | most block extensions        |
| `inline_trigger` | per inline subject, at registered trigger bytes           | strikethrough               |
| `post_process`   | once, after the tree is complete, with parent context     | heading IDs, task lists, the raw-HTML filter, footnotes |
| `render`         | when the renderer meets one of its node kinds             | every extension with output |

## Node kinds are data

An extension node is `Node::Extension(ExtNode)`: the kind name, an
ordered list of `(name, value)` attributes, and children. Kinds are
declared up front (name, container-or-leaf, attribute schema), which
is what lets the JSON export, `mdast` export and the declarative tier
(M6) treat extension nodes uniformly. Unknown kinds are not a parse
error — they render nothing and are reported by `check` — because an
extension that adds kinds may not be loaded where a document using
them is read.

## Purity rules

Hooks get no I/O, no clock, no randomness and no global state. The
same source and options always produce the same tree. That is what
makes a declarative definition and a sandboxed WebAssembly extension
interchangeable, and it keeps the core free of global state.

## Limits

Built-in extensions enforce the reference implementations' limits
(table column count, footnote nesting) the way the core does; loaded
extensions inherit the format's limits (M6). A hook that returns an
error aborts the parse with a span-carrying problem, not a panic.

## Where to look

- `geist4-core/src/extension.rs` — the traits and context types.
- `crates/geist4-ext/src/*` — the built-ins as worked examples.
- `crates/geist4-ext/spec/*.md` — each extension's conformance
  examples in the CommonMark example format, including examples that
  show ordinary Markdown is unchanged when the extension is on.
