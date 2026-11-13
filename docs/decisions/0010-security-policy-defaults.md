# 0010 — Security policy defaults: URL filter, raw HTML, no built-in limits

## Context

Milestone M3 requires a written safety policy: a default URL policy, a
raw-HTML policy, and a position on nesting/size limits, documented in
`docs/security-model.md`. The constraints pulling in opposite
directions: CommonMark conformance is byte-for-byte tested against the
652-example specification suite (which forces raw-HTML passthrough and
forbids any output-changing truncation), while the security goal wants
untrusted input to render without script execution.

## Decision

1. **URL filter stays as implemented** (from the M1 renderer): link and
   image destinations that literally begin with `javascript:`,
   `vbscript:`, `file:`, or `data:` are dropped, with `data:` allowed
   only for the four raster image types `png`, `gif`, `jpeg`, `webp`.
   Matching is byte-wise ASCII prefix, case-sensitive, no
   normalization — deliberately conservative: unknown schemes are not
   dangerous URLs and render as relative destinations.
2. **Raw HTML default is `allow`**, byte-for-byte passthrough, because
   the conformance suite requires it. `escape` and `omit` policies are
   planned as renderer options in the API milestones; the default does
   not change.
3. **No engine-imposed nesting/size limits.** Limits that truncate
   change output; instead the engine guarantees linearity and no
   recursion, making memory use proportional to input and immune to
   stack overflow at any depth. Hosts needing hard caps size-limit the
   input; an optional limit knob may be added later without changing
   the default.
4. Hosts rendering untrusted Markdown sanitize the output (or filter
   input) and apply a CSP; `docs/security-model.md` states this as the
   caller's residual responsibility.

## Consequences

- The 652-example suite and the differential suite remain the source of
  truth for output; no security mechanism may special-case an example.
- The safe URL filter is embedded in the current renderer; when the
  policy API lands it becomes a named policy with `safe` as the default
  value, and the byte-matching rule is kept so the filter is
  reasoning-friendly (no normalization surprises).
- The known trade-off: with raw HTML allowed and no sanitizer, the
  engine alone does not make untrusted input safe to display. The
  documentation says so plainly rather than implying otherwise.
- The no-limits decision is revisitable if a host binding requires a
  fuel budget (the WASM milestone already plans fuel and memory limits
  at the *binding* layer, which is the right place for caps that must
  not alter conformant output on other platforms).
