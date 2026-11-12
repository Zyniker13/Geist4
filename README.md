# Geist4

A Markdown engine in Rust: a parser, renderer and checker that conforms exactly to [CommonMark](https://spec.commonmark.org/), with a standard, loadable format for extensions to the language.

**Status:** early development. Nothing here is ready to use yet. Progress is tracked in [STATUS.md](STATUS.md) and the plan is in [ROADMAP.md](ROADMAP.md).

## Goals

1. **Correct.** Every example in the CommonMark specification passes, and so does every example of every extension. Behavior is defined by tests, not by accident.
2. **Reliable.** It never panics, never hangs, and runs in linear time on any input, including hostile input. It is tested by spec suites, unit tests, property tests, fuzzing, differential tests against the reference implementation, pathological-input tests and mutation testing. See [TESTING.md](TESTING.md).
3. **Secure.** Safe by default: no `unsafe` code in the core, dangerous URLs and raw HTML handled by explicit policy, limits on nesting and size, and extensions loaded in a sandbox. See [SECURITY.md](SECURITY.md).
4. **Fast.** Single pass per phase, no regular expressions, no needless copies. Benchmarked against the leading engines on every change.
5. **Extensible.** Additions to Markdown are described in one standard, versioned extension format, so the engine can parse, check, and render them like the core language, and each extension ships its own spec tests.

## Layout

| Path | What it is |
| --- | --- |
| `ROADMAP.md` | Milestones to 0.1.0, each with a "Done when" line |
| `STATUS.md` | Where the work stands now |
| `TESTING.md` | How the engine is tested, and how to run every suite |
| `docs/decisions/` | Decision records, one per decision |
| `docs/extensions/` | The extension format specification (from milestone M5) |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. The CommonMark specification and its tests, vendored under `spec/`, keep their own license (CC BY-SA 4.0).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
