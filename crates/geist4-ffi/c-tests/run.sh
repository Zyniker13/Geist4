#!/bin/sh
# Builds the shared library, compiles the C contract test against the
# generated header, and runs it. The C compiler and the shared-library
# naming are the only platform bits here; Windows is skipped until a
# binding needs it there (tests/ffi.rs pins the same contract from
# Rust on every platform).
set -eu
cd "$(dirname "$0")/../../.."

if ! command -v cc >/dev/null 2>&1; then
    echo 'no C compiler on PATH; skipping the C contract tests'
    exit 0
fi
case "$(uname -s)" in
    *MINGW*|*MSYS*|*CYGWIN*)
        echo 'skipping the C contract tests on Windows (tests/ffi.rs pins the same contract from Rust)'
        exit 0
        ;;
esac

echo '== building geist4-ffi (release) =='
cargo build -p geist4-ffi --release

cd crates/geist4-ffi
case "$(uname -s)" in
    Darwin)
        lib=../../target/release/libgeist4_ffi.dylib
        rpath="-Wl,-rpath,$PWD/../../target/release"
        ;;
    *)
        lib=../../target/release/libgeist4_ffi.so
        rpath="-Wl,-rpath,$PWD/../../target/release"
        ;;
esac

echo '== compiling test_ffi.c =='
cc -std=c11 -Wall -Wextra -Werror -Iinclude c-tests/test_ffi.c "$lib" \
    $rpath -o c-tests/test_ffi

echo '== running the C contract tests =='
./c-tests/test_ffi
