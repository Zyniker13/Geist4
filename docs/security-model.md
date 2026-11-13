# Security model

Geist4 turns untrusted Markdown text into HTML. This document states the
threat model, every security-relevant default and why it is set, and the
residual work that stays with the caller. It is part of milestone M3's
contract: nothing here may change silently — a change to any default
below requires a new decision record and an update to this file.

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

Geist4 is a *library*: it parses and renders, it does not sanitize to a
specific application policy and it does not serve the output. The
boundary this document draws is between what the engine guarantees and
what the host must still do.

## What the engine guarantees

- **Memory safety.** The crate tree is compiled with `unsafe_code =
  "forbid"` at the workspace level; there is no `unsafe` anywhere in
  Geist4.
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
  the parser and renderer and assert no panic and deterministic output.
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
ecosystem's behavior; hosts that need user-visible stripping get it by
pre-checking destinations through the same rule, which will be exposed
as a public API function in the binding milestones.

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
and omit (drop it silently) policies are planned as renderer options in
the extension/API milestones; the *default stays* `allow` so that
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
with a fixed fuel budget — should size-limit the input string and pass
an explicit depth/size option when the options plumbing lands; the
decision to keep the default unlimited is recorded in decision 0010.

## What a caller must still do

- Sanitize raw HTML in the output (or filter the input) when the source
  is untrusted. The engine's job is conformance plus a safe URL policy;
  full sanitization to a host-specific policy is not.
- Apply a Content-Security-Policy to the pages that show the output.
  CSP is the structural backstop for script execution; URL filtering
  alone is not.
- Size-limit inputs before handing them to the engine if the host needs
  a cap (a chat message is not a book).
- Keep context in mind: the HTML renderer produces fragment HTML, not a
  document; the host supplies the surrounding page and headers.

## Testing that backs this document

- `tests/pathological.rs` — linear scaling on 14 adversarial families,
  10,000-deep nesting, fixed pathological cases.
- `tests/properties.rs` — no panics, deterministic output, balanced
  structural tags, span bounds, over 512 generated cases each.
- `tests/differential.rs` — 400 generated documents byte-identical to
  the reference implementation.
- `tests/regressions/` — one file per bug found, with the reproducing
  input and the exact expected output.

Any change that weakens a guarantee above must come with a failing test
first — the suite is the enforcement mechanism for this document.
