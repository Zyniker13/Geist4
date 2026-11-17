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

echo '== conformance kit =='
sh conformance/run.sh

echo '== cargo doc =='
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps

echo '== cargo deny =='
cargo deny check

echo 'all checks passed'
