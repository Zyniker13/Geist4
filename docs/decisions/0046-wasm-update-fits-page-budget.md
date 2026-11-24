# Decision 0046: the update surface fits the wasm page-load budget

Date: 2026-11-24
Status: accepted

## Context

M12 (0.3.0, "editor-ready") adds the incremental update surface —
`Editor::update` from M11 — to the C ABI (`geist4_update`,
`geist4_document_spans`) and to the JavaScript/wasm binding
(`Document.update`, `UpdatedDocument.changedSpans`). Decision 0029 set
the wasm page-load budget at 524,288 bytes raw (512 KiB) and 180,224
bytes gzipped, and decision 0044 already recorded that the module ships
without the `highlight` feature because the syntect pack alone is about
four times the budget. The question this decision answers: does the
update surface fit the same budget?

## Decision

Yes. Measured with the exact pipeline `scripts/check.sh` uses —
`cargo build -p geist4-wasm --target wasm32-unknown-unknown --release
--no-default-features`, then `wasm-bindgen --target web`, then
`wasm-opt -Oz --enable-bulk-memory --strip-debug` (bindgen first,
wasm-opt second, the same order wasm-pack uses; the `--strip-debug`
name section is dropped, so the shipped artifact is what every number
below measures):

- 0.2.0 baseline (before the update surface): 414,870 bytes raw,
  170,529 gzipped
- with the update surface: 434,075 bytes raw, 177,017 gzipped
- budget: 524,288 raw / 180,224 gzip

The update entry points cost 19,205 bytes raw and 6,488 bytes
gzipped: the `Document::update`/`UpdatedDocument` glue and TS types,
the span serialization into a flat `u32` array, the safe module's
`Updated::spans`/`into_document` accessors, and the code they pull in
behind the glue. Both numbers were measured with the exact pipeline
above — the baseline at the 0.2.0 release commit, the with-surface
number at the commit that added the update binding — so the deltas
compare like against like. The budget check in
`scripts/check.sh` (and its CI twin) fails the build if a later change
pushes the module past either limit; no code change is gated on this
decision — the surface ships and the existing budget continues to
apply.

Shape notes that keep the cost small:

- `changedSpans()` returns a flat `Vec<u32>` (`[start, end, start,
  end, …]`) rather than an array of objects or a `js_sys::Array`; the
  flat buffer serializes once and copies by view, and every source
  that fits a JS string fits `u32` offsets.
- The update path reuses the options registration the previous
  document already holds (`OwnedDocument::options`), so no options
  argument joins the ABI surface.
- The JS type is a separate `UpdatedDocument` class rather than a
  subclass of `Document`: wasm-bindgen inheritance would add a
  prototype chain for a two-method surface, and the TS types then read
  as two plain classes either way.

## Consequences

- The 512 KiB page-load budget survives M12 with about 17% headroom;
  CI enforces it on every push, so the number cannot silently regress.
- The Rust and C suites pin the update contract (value-identity, span
  containment and bounds, identical-sources-empty, failed-update
  leaves the previous document live); the Node harness now pins the
  same contract through the glue with an edit-sequence test, so all
  three bindings state one guarantee.
- Any future surface that would cross the budget must either displace
  something from the module or amend this decision and 0029 together;
  the highlighting opt-out (decision 0044) remains the established
  pattern for that.
