# 0034 — The Python binding: PyO3 over the safe module, wheels by maturin

Date: 2026-11-17

## Context

M7 ("Bindings Everywhere") calls for Python bindings (PyO3 with
`maturin`), tested in CI. The bindings built so far — the C ABI
(decision 0028), the WebAssembly binding (decision 0029), and the
Swift binding (decision 0033) — all rest on one boundary:
`geist4-ffi`'s `safe` module, so no binding needs `unsafe` of its own
and all the surfaces stay method-for-method identical.

## Decision

`crates/geist4-python` is a PyO3 extension module over the `safe`
module, packaged as a wheel named `geist4` with maturin:

- The module imports as `geist4._geist4`; a thin `geist4/__init__.py`
  re-exports the public names, so users `import geist4` and never see
  the underscore module.
- The surface mirrors the JavaScript binding one for one: `version()`,
  `render_html(source, options=None)`, `Options` with
  `enable_builtin`, `Document(source, options=None)` with `html()` and
  `problems()`, and `Problem` with `message`, `start`, `end`. One
  exception type, `Geist4Error`, for unknown extension names and
  engine panics alike.
- `pyo3` 0.30.1 is pinned; the `abi3-py39` feature builds against the
  stable Python ABI (pyo3 0.30 dropped 3.8, EOL since 2024-10), so one
  wheel serves every Python 3.9 and newer, with no per-version wheels
  to build or test. maturin builds the wheel from `pyproject.toml`,
  which sets `module-name = "geist4._geist4"` and
  `python-source = "python"`.
- The crate is a workspace member with the standard lint set;
  `pyo3/extension-module` is enabled only by maturin at wheel-build
  time (via `pyproject.toml`), so `cargo test` and `cargo clippy` on
  the host link normally against libpython.
- The gate (`scripts/check.sh`) builds the wheel with maturin,
  installs it into a throwaway venv under `target/`, and runs the
  contract suite (`tests/test_contract.py`, stdlib `unittest`) against
  the installed package — exactly what a pip user gets, not a
  source-tree import. CI runs the same gate, so the wheel is built and
  tested on every push, per the roadmap.
- The contract suite mirrors the C contract test: byte-for-byte
  renders (heading, empty document, NUL as ordinary content, GFM
  strikethrough and tables), the unknown-name error listing the valid
  names, bundle/single composition and idempotence, `Document`
  rendering twice identically while owning its options copy, and
  problem spans within the source. A Python string is always valid
  Unicode, so the C ABI's null and invalid-UTF-8 rejections are
  unreachable here, as in Swift.

## Consequences

- Every Python user gets the same byte-for-byte output as every other
  binding, pinned by a contract suite that runs in the same gate as
  everything else.
- `cargo install maturin` (crates.io, `--locked`) is the only new
  tool; the venv and `unittest` are stdlib, so nothing installs from
  PyPI in the gate or CI.
- Recorded as security-relevant dependencies per the house rules:
  `pyo3` 0.30.1 (the FFI boundary to the CPython stable ABI) and
  maturin (the wheel builder) — both are the standard, widely
  maintained tools for Rust-to-Python bindings; `cargo deny check`
  continues to cover the Rust side.
- The wheel is a build artifact, like the XCFramework: never checked
  in, rebuilt by the gate, and its `dist/` and `venv/` outputs are
  ignored.
- Wheel size is not budgeted here (the WebAssembly binding has a
  budget because it ships to browsers); the Python wheel ships where
  wheel size matters little. If it ever matters, `strip` and
  `opt-level` tuning come first.
