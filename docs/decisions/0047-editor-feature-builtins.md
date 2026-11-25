# Decision 0047: how the editor feature built-ins are implemented

Date: 2026-11-25
Status: accepted

## Context

M12 (0.3.0, "editor-ready") adds four built-in extensions the editor
needs: inline math (`$…$` and `$$…$$`), wiki links
(`[[target|label]]`), highlight marks (`==…==`), and smart typography
(quotes, dashes and ellipsis). The repository already has a
declarative tier (`geist4-ext-loader`, format version 1) that compiles
`[[inline_span]]`, `[[wiki_link]]` and `[[replacement]]` declarations
from an `extension.toml` into an `Extension`, with the tier-1
conformance packages (`conformance/behavior/{inline-spans,wiki-links}`)
pinning their semantics. The four built-ins need exactly that
semantics, plus two things the tier does not do: multi-line spans for
`$$` display math, and context-dependent replacements for smart
typography (an apostrophe depends on whether it follows a letter).

## Decision

The four built-ins are handwritten `Extension` implementations in
`geist4-ext`, following the pattern of the existing built-ins
(`Strikethrough`, `Tables`, …): only the public extension API of the
core, no direct internals. They are not compiled from tier-1 manifest
text at startup. Two reasons:

1. The manifest path would drag `toml`+serde and the format validator
   into the wasm binary. Decision 0046 measured 434,075 bytes raw
   against the 524,288-byte budget at the time this work started; the
   manifest machinery is exactly the code the wasm build exists to
   avoid paying for at load.
2. The built-ins need hooks the tier does not expose (a block hook for
   display math, a post-process hook for smart typography), so manifest
   equivalence would be partial anyway.

The guarantee that the handwritten implementations follow the tier is
made by tests, not by shared code: dev-only tests in `geist4-ext` load
the tier-1 conformance packages with `geist4-ext-loader` and compare
their rendered output byte for byte against the built-ins' output on
the same markdown (highlight vs the `==`/`%`/`$` inline spans, wiki
links vs the wiki link rule). Those tests run with `cfg(test)` and the
`loader` dev-dependency, so the shipped `geist4-ext` crate does not
depend on the loader. Every behavior the tier pins (flanking rules,
math border guards, first-match ownership, escape and code-span
immunity, wiki percent-encoding and real-link precedence) is therefore
locked to the tier by construction, and the built-ins' own spec files
add cases the tier does not cover.

Smart typography cannot be declarative replacements: opening quotes
depend on surrounding context and replacements must not rewrite code
spans. It is a `post_process` hook that walks text nodes and rewrites
their content, skipping anything inside code spans, inline code and
extension subtrees (loaded extensions own their content semantics).

Display math is not a block form: `$$…$$` is an inline delimiter pair
whose content may span lines inside one paragraph, matching comrak and
pandoc's `tex_math_dollars` shape. The delimiter-stack match keeps
`$` and `$$` apart by construction — a run pushes the `math_inline`
kind when it holds one dollar and `math_display` when it holds two or
more, and matching requires the same byte and the same kind. `$$`
runs carry no border guards (comrak's scanner has none for two or
more dollars); flanking still applies, so `$$ $$` stays literal.
Inline `$…$` follows the tier's border guards exactly (`$ x$`, `$5$`,
`5$x$`, `$x $` stay literal). Rendering follows comrak: inline math
renders `<span data-math-style="inline">`, display math
`<span data-math-style="display">`, with the parsed inline content as
children.

## Consequences

- `geist4-ext` stays wasm-clean: the four built-ins add no new
  dependencies, and the size-budget check keeps enforcing the 512 KiB
  module.
- Equivalence is re-proven on every `cargo test` run in
  `geist4-ext`, not once: a drift between the tier and a built-in
  fails the build immediately.
- The tier-1 conformance packages remain the single normative
  description of the shared semantics; the built-ins' spec files
  (`spec/math.md`, `spec/wiki_links.md`, `spec/highlight.md`,
  `spec/smart_typography.md`) document their own extra behavior with
  examples checked into the repo like the other built-ins' specs.
- A future change to make the built-ins literally manifest-generated
  would require either a `no_std`-friendly manifest parser or keeping
  the wasm build on handwritten extensions; this decision picks the
  second.
