#!/bin/sh
# The Geist4 extension-format conformance kit runner.
#
# Runs every case under conformance/:
#
#   manifest/NN-name/  — a package whose manifest is either valid or
#       rejected; expected.txt states the outcome (line 1: `ok` or
#       `error`), and for errors the stable reason token (line 2) and
#       this implementation's exact message (line 3, informational).
#   behavior/<name>/   — a full declarative package whose spec.md
#       examples must render identically in any implementation.
#
# The harness is this repository's CLI (`geist4 ext manifest`,
# `geist4 ext check`). Another implementation of the format replaces
# it with its own commands and compares the same expected.txt files;
# see conformance/README.md.
#
# Env: GEIST4 overrides the CLI path. The default is
# target/debug/geist4 (plus .exe where Windows names its binaries
# that way), built with the sandbox feature if missing.
set -eu
cd "$(dirname "$0")/.."

exe=
case "$(uname -s)" in
    *MINGW*|*MSYS*|*CYGWIN*) exe=.exe ;;
esac

if [ -n "${GEIST4:-}" ]; then
    GEIST="$GEIST4"
else
    if [ ! -x "target/debug/geist4$exe" ]; then
        cargo build -q -p geist4-cli --features geist4-ext-loader/sandbox
    fi
    GEIST="target/debug/geist4$exe"
fi

fails=0
total=0
stderr="${TMPDIR:-/tmp}/conformance-stderr.$$"
trap 'rm -f "$stderr"' EXIT

run_case() { # $1 = directory, $2 = ext subcommand, $3 = expected outcome
    total=$((total + 1))
    if "$GEIST" ext "$2" "$1" >/dev/null 2>"$stderr"; then
        actual=ok
    else
        actual=error
    fi
    if [ "$actual" != "$3" ]; then
        echo "FAIL $1: expected $3, got $actual"
        sed 's/^/    /' "$stderr"
        fails=$((fails + 1))
    elif [ "$actual" = error ] && [ "$2" = "manifest" ]; then
        # Manifest cases: the third line is this implementation's
        # exact message at the time the case was captured; it must
        # still be produced verbatim.
        expected_message=$(sed -n '3p' "$1/expected.txt")
        if [ -n "$expected_message" ] && ! grep -qF "$expected_message" "$stderr"; then
            echo "FAIL $1: the loader's message changed"
            echo "    expected: $expected_message"
            sed 's/^/    actual:   /' "$stderr"
            fails=$((fails + 1))
        fi
    fi
}

for dir in conformance/manifest/*/; do
    run_case "${dir%/}" "manifest" "$(sed -n '1p' "${dir}expected.txt")"
done
for dir in conformance/behavior/*/; do
    run_case "${dir%/}" "check" ok
done

echo "conformance: $((total - fails))/$total cases pass"
[ "$fails" -eq 0 ]
