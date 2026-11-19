# Testing

The engine is meant to be absolutely reliable, so testing is a first-class part of the design, not a step at the end. This file describes every test suite, what it proves, and how to run it. Keep it true: when a suite is added, changed, or moved, update this file in the same commit.

## The Rules

1. **Every behavior has a test.** A rule from a spec has a spec example; a rule we decide ourselves has a unit test and a decision record.
2. **Every bug gets a regression test** in `tests/regressions/` before it is fixed, with a comment saying what it caught and which commit fixed it.
3. **Counts never go down.** The CommonMark pass count, each extension's pass count, and coverage are checked in and enforced by CI.
4. **Local and CI are the same.** `scripts/check.sh` runs exactly what CI runs. A change is pushed only after it passes there.
5. **No test is skipped to get green.** A test that fails is fixed, or the behavior it checks is changed by a decision record. Ignored tests need a reason in a comment and an issue.

## Suites

Each suite below says where it lives and how it runs. Paths and commands are the plan; fill in the real ones as each suite is built.

| Suite | Proves | Where | Runs |
| --- | --- | --- | --- |
| CommonMark spec | Exact conformance: all 652 examples | `spec/`, harness in `crates/spec-harness` | every CI run |
| Extension specs | Each extension's own examples, plus examples showing plain Markdown is unchanged | `crates/geist4-ext/spec/*.md`, run by `crates/geist4-ext/tests/spec.rs` | every CI run |
| GFM spec | GFM extensions conform to the GFM spec's examples | `spec/gfm/` | every CI run |
| Unit tests | Each scanner, rule and edge case on its own | next to the code | every CI run |
| Property tests | No panic on any input; well-formed HTML; deterministic output; valid spans | `tests/properties/` (`proptest`) | every CI run, more cases locally |
| Pathological inputs | Linear time on hostile input; limits behave as documented | `tests/pathological/` | every CI run |
| Differential tests | Same output as the reference C implementation on generated input, except documented differences | `tests/differential/` | every CI run |
| Fuzzing | No crash, hang or limit breach on mutated input | `fuzz/` (`cargo-fuzz`) | a few minutes per target in CI; an hour per target locally per milestone |
| Cross-binding | JavaScript, Swift, Python and C give byte-identical output to the core on every spec | `tests/bindings/` | every CI run |
| C ABI | The generated header matches the Rust ABI, and a C program linking the shared library gets the same contract (nulls, UTF-8, ownership, byte-exact HTML) as `tests/ffi.rs` | `crates/geist4-ffi/c-tests/` | every CI run |
| Wasm binding | The checked-in JS glue and TypeScript types are current with the Rust source, the .wasm stays inside the size budget, and the real WebAssembly build gives JavaScript the same byte-exact contract as the C ABI | `crates/geist4-wasm/node-tests/` | every CI run |
| Swift binding | The Swift wrapper over the C ABI gives the same byte-exact contract (HTML, problem reporting, error behavior) as the C test, and the XCFramework assembles for macOS and iOS | `bindings/swift/` | every CI run (macOS runner) |
| Python binding | The wheel `pip` installs gives the same byte-exact contract (HTML, problem reporting, error behavior) as the C test | `crates/geist4-python/tests/` | every CI run |
| mdast export | The mdast tree equals the reference JavaScript implementation's tree byte for byte on every spec example and boundary probe | `crates/geist4-mdast/tests/` | every CI run |
| Host extensions from the bindings | The Rust battery, and its mirrors in Python, JavaScript and Swift, exercise the same host-extension contract (fenced blocks, claim+override, inline triggers, latched errors, validation) through each binding's callback surface | `crates/geist4-ffi/tests/host_extension.rs`, `crates/geist4-python/tests/test_host_extensions.py`, `crates/geist4-wasm/node-tests/run.mjs`, `bindings/swift` (`Geist4Contract`) | every CI run |
| Extension tiers | An extension's `spec.md` passes whichever tier implements it | `tests/ext-conformance/` | every CI run |
| Round trip | `format` is idempotent and keeps the rendered HTML the same | `tests/roundtrip/` | every CI run |
| JSON schema | Every JSON tree validates against the published schema | `tests/json/` | every CI run |
| Extension sandbox | Hostile extensions are stopped by fuel and memory limits | `tests/sandbox/` | every CI run |
| Benchmarks | Speed against the corpus and other engines | `benches/` (`criterion`), instruction counts in CI | instruction counts every CI run; wall clock locally |
| Mutation testing | The tests notice when the code is changed | `cargo-mutants` | before each release, results below |
| Coverage | Which lines no test reaches | `cargo-llvm-cov` | every CI run, threshold enforced |

## Running Everything

```sh
scripts/check.sh            # exactly what CI runs
```

### Fuzzing

The `fuzz/` crate holds the targets; it builds with nightly (a scoped
`fuzz/rust-toolchain.toml`) and is excluded from the workspace. Run
from inside `fuzz/`:

```sh
cd fuzz
cargo fuzz run to_html                 -- -max_total_time=3600  # the full pipeline
cargo fuzz run parse_document          -- -max_total_time=3600  # parser only
cargo fuzz run parse_with_extension    -- -max_total_time=3600  # extension hooks active
cargo fuzz cmin to_html                                         # minimize the corpus
```

A three-minute run per target is what CI (`.github/workflows/fuzz.yml`)
executes on every push; the hour-long runs are for after each milestone,
with results recorded under Results below. The extension target
(`parse_with_extension`) parses and renders with every built-in
registered — tables, strikethrough, autolink, tasklist, tagfilter and
footnotes — so the inline trigger dispatch, the extension delimiter
machinery, the bracket claim, the output filter and the post-process
tree surgery all get mutated input.

The minimized corpus is not committed in full (it is tens of megabytes
of near-duplicate files); instead a deterministic subset is checked in
under `fuzz/seeds/`, produced by selecting one file per coverage edge
from the minimized corpus, so every local run starts from the fuzzer's
learned edges. `tests/differential.rs` runs the entire seed set against
cmark 0.31.2 on every `cargo test`, byte-identical output required, so
fuzzer-found inputs become a permanent differential fixture.

Document each other command here as it exists (benchmarks, mutation
testing), with the time it takes.

### The C ABI header and tests

`crates/geist4-ffi` is the one place Rust meets C. Its header
`crates/geist4-ffi/include/geist4.h` is generated from the Rust source
by `cbindgen` and checked in, and CI verifies it is current: regenerate
and `diff` after any change to `geist4-ffi`'s public ABI.

```sh
cargo install --locked cbindgen@0.29.0
cbindgen --config crates/geist4-ffi/cbindgen.toml --crate geist4-ffi \
    --output crates/geist4-ffi/include/geist4.h
sh crates/geist4-ffi/c-tests/run.sh   # builds the lib, compiles and runs the C test
```

The C test compiles `c-tests/test_ffi.c` against the generated header,
links the shared library the way a native binding would, and runs the
same contract `crates/geist4-ffi/tests/ffi.rs` pins from Rust: null
rejection, UTF-8 rejection with byte offsets, ownership and free
discipline, and byte-exact HTML. It needs a C compiler; on platforms
without one the script skips the C program and the Rust tests still
pin the contract.

### The wasm binding glue and Node tests

`crates/geist4-wasm` builds the engine for WebAssembly. Its glue and
TypeScript types in `crates/geist4-wasm/pkg/` are generated by
`wasm-bindgen` and checked in, and CI verifies the text parts are
current — same rule as the C header. The `.wasm` itself cannot be
compared byte for byte across machines (rustc embeds platform-specific
paths in panic locations), so the harness runs twice per `check.sh`:
once against the freshly built glue (`GEIST4_PKG_DIR` points it at the
scratch directory) and once against the checked-in `pkg/` — the fresh
run proves the source behaves, the checked-in run proves the committed
artifact does. The CLI and the `wasm-bindgen` crate must stay
version-paired (decision 0029).

```sh
cargo install --locked wasm-bindgen-cli@0.2.130
rustup target add wasm32-unknown-unknown
cargo build -p geist4-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --typescript --out-dir crates/geist4-wasm/pkg \
    --out-name geist4 target/wasm32-unknown-unknown/release/geist4_wasm.wasm
node crates/geist4-wasm/node-tests/run.mjs
```

The Node harness imports the checked-in glue, initializes the real
WebAssembly, and asserts the same contract the C ABI pins: byte-exact
HTML for plain and GFM rendering, one problem per author mistake,
unknown-extension rejection listing the valid names, NUL bytes as
replacement characters, and empty documents. `scripts/check.sh` also
enforces the size budget from decision 0029: the `.wasm` stays under
512 KiB raw and 160 KiB gzipped. There are no Rust unit tests inside
`geist4-wasm`: wasm-bindgen's shims for exported types cannot run off
WebAssembly, so the contract is pinned where it actually runs.

### The Swift binding and contract executable

`bindings/swift` is a Swift package that links the C ABI's static
library (decision 0033). The XCFramework is a build artifact, never
checked in: `make-xcframework.sh` rebuilds it deterministically and
assembles the layout by hand — slice directories, per-slice and
top-level `Info.plist` files — with no `xcodebuild`, so it runs under
the Command Line Tools alone. With no arguments it builds the host
triple only; passed iOS triples it builds the device and simulator
slices (the simulator architectures merge into one slice with `lipo`).
The libraries are built with `cargo rustc --crate-type staticlib`: a
static library is archived, never linked, so no Apple SDK is needed —
not even for the iOS triples.

```sh
sh bindings/swift/make-xcframework.sh aarch64-apple-darwin \
    aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios  # full framework
cd bindings/swift && swift run Geist4Contract    # the contract executable
```

The contract executable mirrors the C contract test's byte-exact
expectations: the same HTML for plain and GFM rendering, one problem
per author mistake, unknown-extension rejection listing the valid
names, and documents that outlive their options. The C-only cases
(null input, invalid UTF-8) are unreachable from Swift, where a
`String` is always valid UTF-8 and never null. `scripts/check.sh`
builds the full framework — macOS plus the iOS device and simulator
slices — and runs the contract on macOS, and skips elsewhere; CI runs
the same script, so the iOS paths are proven identically everywhere.

### The Python binding and its contract suite

`crates/geist4-python` is a PyO3 extension module over the same safe
module as the other bindings (decision 0034), packaged as a wheel
named `geist4` with maturin and installed as `geist4._geist4` behind a
thin `geist4` package. The wheel is a build artifact and never checked
in.

```sh
cargo install --locked maturin@1.16.0
cd crates/geist4-python && maturin build --release --out ../../target/wheels
python3 -m venv target/py-venv
target/py-venv/bin/pip install --no-index --find-links target/wheels geist4
target/py-venv/bin/python -m unittest discover -s crates/geist4-python/tests
```

The contract suite mirrors the C contract test: byte-exact HTML for
plain and GFM rendering, a NUL byte rendering as the replacement
character, empty documents, unknown-extension rejection listing the
valid names, bundle/single composition and idempotence, documents that
outlive their options, and problem spans as byte offsets. It runs
against the wheel pip installed, so it tests exactly what a pip user
gets, not a source-tree import. A Python string is always valid
Unicode, so the C-only cases (null input, invalid UTF-8) are
unreachable from Python, as from Swift. `scripts/check.sh` builds the
wheel, installs it into a throwaway venv, and runs the suite on every
platform CI runs the gate on.

### The mdast export fixtures

`crates/geist4-mdast` exports the Geist tree as an mdast (unist) tree
(decision 0037). Its test parses every fixture's input, exports,
serializes with `serde_json`, and requires the bytes to equal the
reference implementation's tree:

```sh
cargo test -p geist4-mdast
```

The fixtures under `tests/fixtures/` cover every CommonMark 0.31.2
spec example plus boundary probes (667 files), and their expected
trees were generated from `mdast-util-from-markdown@2.1.0` by the
checked-in generator:

```sh
cd crates/geist4-mdast/tools && npm ci && node gen-mdast-fixtures.mjs
```

The generator runs the reference over the spec examples and over the
probe inputs in `tools/probes/`, writing one JSON file per case; the
reference version is pinned in `tools/package.json` and bumped
together with the fixtures. A legitimate, understood difference must
be recorded in `tests/differences.json` with a reason — the test
asserts both directions (a differing fixture must be listed there, and
every listed fixture must actually differ), so the list can neither
hide drift nor survive drift that is gone. The suite is empty today:
the export matches the reference byte for byte on every fixture.

## Host extensions from each binding (m7-5)

The host-extension contract — declaring kinds and attrs, claiming
fenced blocks with `whole_line`/`columns` consumes and closing with
`close_line`, inline triggers, bracket triggers, render overrides and
post-processing, latched errors surfacing through `problems()` — is
pinned from Rust first
(`crates/geist4-ffi/tests/host_extension.rs`, nine tests), then
mirrored through each binding so the callback surfaces stay in step:

```sh
cargo test -p geist4-ffi --test host_extension          # Rust battery (9)
target/py-venv/bin/python -m unittest crates.geist4-python.tests.test_host_extensions  # Python (28)
node crates/geist4-wasm/node-tests/run.mjs              # JavaScript: 13 host checks of 28
cd bindings/swift && swift run Geist4Contract           # Swift: 9 host checks of 19
```

Python, JavaScript and Swift each expose a `HostExtension` plus
`Options.enable_host`; a hook that raises (Python exception, thrown
JavaScript exception, returned problem in Swift) latches and surfaces
through `problems()` exactly like every other hook failure.

Runnable examples render the same document — a `:::note` fenced block
plus a claimed `- TODO:` item — through each binding's surface:

```sh
python3 examples/bindings/notes.py       # needs the wheel in the venv
node examples/bindings/notes.mjs         # reads crates/geist4-wasm/pkg
cd bindings/swift && swift run Geist4HostNotes
```

All three print byte-identical HTML; that is the m7-5 acceptance
demonstration.

## Known Differences

List here every case where the output deliberately differs from the reference implementation in the differential suite, with the reason and a link to a decision record. The list starts empty and should stay short.

## Results

Latest long-run results (2026-11-13):

- `to_html` (full pipeline): 1 hour, 8.27M executions, 0 crashes, 0
  hangs, 0 OOMs. One real panic found in an earlier run (title
  post-processing slicing past a one-byte quoted title) fixed and made a
  regression test.
- `parse_document` (parser only): 1 hour, 11.67M executions, 0 crashes,
  0 hangs, 0 OOMs.
- Corpora after `cargo fuzz cmin`: 9,334 files (`to_html`), 8,919 files
  (`parse_document`), from ~35k raw files per target.
- Seed subset committed: 490 files (`to_html`), 472 files
  (`parse_document`), one per coverage edge; all 962 pass the
  differential test.

Mutation testing, coverage numbers and benchmark numbers are recorded
here as those workstreams come online.
