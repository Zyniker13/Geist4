#!/bin/sh
# The checks CI runs (see .github/workflows/ci.yml). Run this before every
# push; CI runs the same script, so local and CI can never drift apart.
set -eu
cd "$(dirname "$0")/.."

echo '== cargo fmt =='
cargo fmt --all --check

echo '== cargo clippy =='
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo '== cargo test =='
cargo test --workspace --all-features

echo '== C header is current =='
command -v cbindgen >/dev/null 2>&1 || {
    echo 'cbindgen not found; install it with: cargo install --locked cbindgen@0.29.0' >&2
    exit 1
}
header_tmp=$(mktemp "${TMPDIR:-/tmp}/geist4-header-XXXXXX.h")
trap 'rm -f "$header_tmp"' EXIT
cbindgen --config crates/geist4-ffi/cbindgen.toml --crate geist4-ffi \
    --output "$header_tmp" >/dev/null 2>&1
cmp crates/geist4-ffi/include/geist4.h "$header_tmp" || {
    echo 'crates/geist4-ffi/include/geist4.h is stale; regenerate it with:' >&2
    echo '  cbindgen --config crates/geist4-ffi/cbindgen.toml --crate geist4-ffi --output crates/geist4-ffi/include/geist4.h' >&2
    exit 1
}

echo '== C ABI tests =='
sh crates/geist4-ffi/c-tests/run.sh

echo '== Swift binding =='
case "$(uname -s)" in
Darwin)
    command -v swift >/dev/null 2>&1 || {
        echo 'swift not found; the Swift binding gate needs the Swift toolchain' >&2
        exit 1
    }
    # Builds the full XCFramework — macOS, iOS device and both iOS
    # simulator triples — assembles it, and runs the Swift contract
    # suite against it. A staticlib never links, so the iOS slices
    # need no Apple SDK: the whole build runs under the Command Line
    # Tools on every macOS machine the gate runs on.
    sh bindings/swift/make-xcframework.sh aarch64-apple-darwin \
        aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
    (cd bindings/swift && swift run Geist4Contract)
    ;;
*)
    echo 'skipped: the Swift binding needs macOS'
    ;;
esac

echo '== wasm binding =='
command -v wasm-bindgen >/dev/null 2>&1 || {
    echo 'wasm-bindgen not found; install it with: cargo install --locked wasm-bindgen-cli@0.2.130' >&2
    exit 1
}
command -v node >/dev/null 2>&1 || {
    echo 'node not found; the wasm binding tests need Node' >&2
    exit 1
}
rustup target list --installed | grep -q wasm32-unknown-unknown || {
    echo 'the wasm32-unknown-unknown target is missing; add it with: rustup target add wasm32-unknown-unknown' >&2
    exit 1
}
# Built without the `highlight` feature: the syntect pack is ~1.5 MiB of
# tables and would blow decision 0029's page-load budget fourfold
# (decision 0044); the wasm module ships without the engine and a page
# wanting highlighting registers a JS host hook instead. The rlib
# host-side tests still run with default features, where the engine is.
cargo build -p geist4-wasm --target wasm32-unknown-unknown --release --no-default-features
command -v wasm-opt >/dev/null 2>&1 || {
    echo "wasm-opt not found; install binaryen (brew install binaryen on macOS," >&2
    echo "apt-get install binaryen on Debian/Ubuntu, or the release tarball from" >&2
    echo "https://github.com/WebAssembly/binaryen/releases on Windows) — decision 0029" >&2
    exit 1
}
pkg_tmp=$(mktemp -d "${TMPDIR:-/tmp}/geist4-pkg-XXXXXX")
trap 'rm -rf "$pkg_tmp" "$header_tmp"' EXIT
wasm-bindgen --target web --typescript \
    --out-dir "$pkg_tmp" --out-name geist4 \
    target/wasm32-unknown-unknown/release/geist4_wasm.wasm >/dev/null 2>&1
# Post-process with binaryen: -Oz optimizes, --strip-debug drops the wasm
# name section (97 KiB of function names that only aid stack traces). This
# is the same order wasm-pack uses: bindgen first, wasm-opt second, so the
# shipped artifact — and the one every size number below measures — is the
# optimized module. Regenerate the checked-in pkg/ the same way.
wasm-opt -Oz --enable-bulk-memory --strip-debug \
    -o "$pkg_tmp/geist4_bg.opt.wasm" "$pkg_tmp/geist4_bg.wasm"
mv "$pkg_tmp/geist4_bg.opt.wasm" "$pkg_tmp/geist4_bg.wasm"
# The JS glue and TypeScript types are text and identical on every machine;
# the .wasm itself is not (rustc embeds platform-specific paths in panic
# locations), so its behavior is pinned by running the harness against it
# instead of comparing bytes. Both runs matter: the fresh glue proves the
# current source behaves, the checked-in glue proves the committed artifact
# does — a stale pkg/ fails its own run the moment a fix changes behavior.
pkg_stale=0
for glue_file in geist4.js geist4.d.ts geist4_bg.wasm.d.ts; do
    cmp "crates/geist4-wasm/pkg/$glue_file" "$pkg_tmp/$glue_file" || pkg_stale=1
done
[ "$pkg_stale" -eq 0 ] || {
    echo 'crates/geist4-wasm/pkg/ is stale; regenerate it with:' >&2
    echo '  cargo build -p geist4-wasm --target wasm32-unknown-unknown --release' >&2
    echo '  wasm-bindgen --target web --typescript --out-dir crates/geist4-wasm/pkg --out-name geist4 target/wasm32-unknown-unknown/release/geist4_wasm.wasm' >&2
    exit 1
}
# The size budget from decision 0029: the .wasm stays under 512 KiB raw
# and 176 KiB gzipped (amended in m7-5 when the host-extension callback
# surface joined the module; see decision 0029), the transfer size an
# editor's page load pays.
wasm_bytes=$(wc -c < "$pkg_tmp/geist4_bg.wasm" | tr -d ' ')
gzip_bytes=$(gzip -9 -c "$pkg_tmp/geist4_bg.wasm" | wc -c | tr -d ' ')
[ "$wasm_bytes" -le 524288 ] || {
    echo "the wasm is $wasm_bytes bytes; decision 0029's budget is 524288 raw" >&2
    exit 1
}
[ "$gzip_bytes" -le 180224 ] || {
    echo "the gzipped wasm is $gzip_bytes bytes; decision 0029's budget is 180224" >&2
    exit 1
}
GEIST4_PKG_DIR="$pkg_tmp" node crates/geist4-wasm/node-tests/run.mjs
node crates/geist4-wasm/node-tests/run.mjs

echo '== Python binding =='
command -v maturin >/dev/null 2>&1 || {
    echo 'maturin not found; install it with: cargo install --locked maturin@1.16.0' >&2
    exit 1
}
# Prefer python3, fall back to python (Windows names it the latter).
if [ -z "${PYTHON:-}" ]; then
    if command -v python3 >/dev/null 2>&1; then
        PYTHON=python3
    elif command -v python >/dev/null 2>&1; then
        PYTHON=python
    else
        echo 'python not found; the Python binding gate needs Python 3.9 or newer' >&2
        exit 1
    fi
fi
# Build the wheel exactly as pip users receive it, install it into a
# throwaway venv, and run the contract suite against the installed
# package — not a source-tree import (decision 0034).
rm -rf target/wheels target/py-venv
(cd crates/geist4-python && maturin build --release --out ../../target/wheels) >/dev/null
"$PYTHON" -m venv target/py-venv
if [ -x target/py-venv/Scripts/python.exe ]; then
    venv_python=target/py-venv/Scripts/python.exe
else
    venv_python=target/py-venv/bin/python
fi
"$venv_python" -m pip install --quiet --no-index --find-links target/wheels geist4
"$venv_python" -m unittest discover -s crates/geist4-python/tests

echo '== cross-binding =='
# Every binding renders the checked-in case list (the spec's 652
# examples and every built-in extension's spec, `tools/gen_cross_cases.py`)
# through its own surface, and the outputs must match the core's byte
# for byte (TESTING.md, "Cross-binding").
"$PYTHON" tools/gen_cross_cases.py --check
cargo build --release -p geist4-ffi --bin cross_reference
ref_bin=target/release/cross_reference
[ -f "$ref_bin" ] || ref_bin=target/release/cross_reference.exe
cross_args="--reference $ref_bin --node node --python $venv_python"
case "$(uname -s)" in
Darwin)
    # The Swift gate above built the contract runner; the cross runner
    # is a separate executable target, so build it here. A fresh runner
    # has nothing in .build yet — `swift run Geist4Contract` builds
    # only the product it runs.
    (cd bindings/swift && swift build --product Geist4Cross)
    cross_args="$cross_args --swift bindings/swift/.build/debug/Geist4Cross"
    ;;
esac
# shellcheck disable=SC2086
"$PYTHON" tools/cross_binding.py $cross_args

echo '== conformance kit =='
sh conformance/run.sh

echo '== cargo doc =='
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps

echo '== cargo deny =='
cargo deny check

echo 'all checks passed'
