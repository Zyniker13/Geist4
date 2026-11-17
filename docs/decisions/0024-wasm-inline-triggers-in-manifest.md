# Decision 0024: tier-2 inline trigger bytes are declared in the manifest

## Context

The WIT world's `inline-trigger` hook is only useful when the engine
knows at which bytes to call it. The first cut of the format said the
engine "registers the extension's trigger bytes at load, from what the
component scans in `init`" — but `init` returns only
`result<_, string>`, so there is no channel carrying trigger bytes
from the component to the engine, and the adapter's hook was dormant.
A tier-2 extension could not offer an inline span at all, which makes
the format's own inline-trigger hook dead weight and misrepresents
what tier 2 can do.

## Decision

The engine learns a tier-2 extension's inline trigger bytes from the
manifest: the `[wasm]` table gains

```toml
inline_triggers = ["%", "^"]     # optional; at most 32 one-byte strings
```

The loader validates each entry as exactly one printable ASCII byte,
unique in the list, and not one of the core's own trigger bytes
(`\`, `` ` ``, `&`, `<`, `*`, `_`, `[`, `!`, `]`), which the core never
hands to extension hooks. The adapter returns the bytes from
`Extension::inline_triggers`; the core's existing dispatch (registration
order, several extensions may share a byte) is unchanged.

The WIT world is unchanged: `init` still receives only kinds and
options, and the inline-trigger hook's documentation now names the
manifest as the registration source. `docs/extensions/format.md`
documents the field in the `[wasm]` sketch and the tier-2 rules.

## Consequences

- A tier-2 inline span works end to end: manifest registers a byte,
  the component's `inline-trigger` returns a `delimiter` decision, the
  engine pairs the runs with its normal machinery, and `render`
  supplies the HTML. The adapter test suite pins the path with the
  rebuilt guest fixture (an `em` kind over `%` runs).
- The `node` decision of `inline-trigger` stays declined by the
  adapter (the core trait has no claim API on the trigger context); a
  component that wants to claim a span whole uses `inline-bracket`,
  whose claims carry `end`. Documented in the adapter.
- The manifest is also the right place for a tool: a host reading
  `extension.toml` can show which bytes an extension hooks without
  running the component.
- Fixtures and examples rebuilt on this shape: the guest fixture's
  `%` arm, and the tier-2 examples under `examples/extensions/`.
