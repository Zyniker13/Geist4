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
cargo build -p geist4-wasm --target wasm32-unknown-unknown --release
pkg_tmp=$(mktemp -d "${TMPDIR:-/tmp}/geist4-pkg-XXXXXX")
trap 'rm -rf "$pkg_tmp" "$header_tmp"' EXIT
wasm-bindgen --target web --typescript \
    --out-dir "$pkg_tmp" --out-name geist4 \
    target/wasm32-unknown-unknown/release/geist4_wasm.wasm >/dev/null 2>&1
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
# and 160 KiB gzipped, the transfer size an editor's page load pays.
wasm_bytes=$(wc -c < "$pkg_tmp/geist4_bg.wasm" | tr -d ' ')
gzip_bytes=$(gzip -9 -c "$pkg_tmp/geist4_bg.wasm" | wc -c | tr -d ' ')
[ "$wasm_bytes" -le 524288 ] || {
    echo "the wasm is $wasm_bytes bytes; decision 0029's budget is 524288 raw" >&2
    exit 1
}
[ "$gzip_bytes" -le 163840 ] || {
    echo "the gzipped wasm is $gzip_bytes bytes; decision 0029's budget is 163840" >&2
    exit 1
}
GEIST4_PKG_DIR="$pkg_tmp" node crates/geist4-wasm/node-tests/run.mjs
node crates/geist4-wasm/node-tests/run.mjs

echo '== conformance kit =='
sh conformance/run.sh

echo '== cargo doc =='
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps

echo '== cargo deny =='
cargo deny check

echo 'all checks passed'
