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

echo '== cargo doc =='
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps

echo '== cargo deny =='
cargo deny check

echo 'all checks passed'
