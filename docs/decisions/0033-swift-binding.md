# 0033 — The Swift binding: one static library, a plain contract executable

## Context

M7's third binding (m7-3) is Swift, for the macOS and iOS apps that use the
engine. The C ABI (m7-1, decision 0028) is the boundary every binding
builds on; the question was how a Swift package consumes it, what the
contract suite looks like, and what the gate can and cannot check on a
machine that ships only the Command Line Tools.

## Decision

The Swift package (`bindings/swift/`) links the C ABI's static library,
never Rust: `geist4-ffi` builds as a `staticlib` per target triple, and the
package declares it as a SwiftPM binary target. The wrapper
(`Sources/Geist4/Geist4.swift`) is plain safe Swift over the C API —
opaque handles become classes that free themselves, pointer-plus-length
inputs go through `String.withUTF8`, and the owned NUL-terminated outputs
decode to `String`s. Errors are thrown `Geist4Error`s carrying the C
status and message. No binding needs its own `unsafe`; the one boundary
stays in `geist4-ffi`.

The XCFramework is a build artifact, not a checked-in one, unlike the C
header and the wasm `pkg/`. A static library is large and
toolchain-specific, so it is never committed; `make-xcframework.sh`
rebuilds it deterministically. The script assembles the XCFramework layout
by hand — slice directories, one XML `Info.plist` per slice, a top-level
`Info.plist` listing the slices — with no `xcodebuild`, so it runs under
the Command Line Tools alone and behaves identically on CI. With no
arguments it builds the host triple only; passed iOS triples it builds the
device and simulator slices (the two simulator architectures merge into
one slice with `lipo`). iOS triples need the Apple SDKs, which only full
Xcode ships, so the script fails with a clear message when they are
missing.

The contract suite is a plain executable (`Geist4Contract`), not XCTest:
the Command Line Tools do not ship `XCTest.framework`, and the C and Node
contract suites are plain executables too. It mirrors the C contract
test's byte-exact expectations — the same HTML, the same problem
reporting, the same error behavior — so all four bindings can never
quietly diverge. The C-only cases (null input, invalid UTF-8) are
unreachable from Swift, where a `String` is always valid UTF-8 and never
null.

The gate: `scripts/check.sh` builds the host-triple slice and runs the
contract executable on macOS, and skips with a note elsewhere. CI's macOS
runner additionally builds the full framework — macOS plus the iOS device
and simulator slices — so the iOS paths are proven on every push even
though no local machine here has full Xcode.

## Consequences

Swift callers add the package, run `make-xcframework.sh`, and link; a
Rust toolchain is needed only to rebuild the library, and the gate rebuilds
it on every run, so staleness cannot drift. The Swift contract is pinned by
`Geist4Contract` against the same byte-exact expectations the Rust, C, and
JavaScript suites assert. The wrapper stays fully safe Swift; the C ABI's
error model maps onto thrown errors, so a bad option name or a parse
failure is a catchable `Geist4Error`, never a crash.
