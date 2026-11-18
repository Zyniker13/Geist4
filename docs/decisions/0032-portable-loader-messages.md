# 0032 — Portable loader messages

Date: 2026-11-17

## Context

The conformance kit pins the loader's error messages, and CI runs the
kit on every supported operating system. Case 15 (a missing component
file) passed on Linux and macOS but failed on Windows: the loader
formatted the raw `std::io::Error`, and its text differs by platform
("No such file or directory (os error 2)" against "The system cannot
find the file specified. (os error 2)"). The engine's message strings
are a cross-platform contract — bindings and the conformance kit rely
on them — so they must not leak the operating system's own wording.

## Decision

`geist4-ext-loader` maps the common `std::io::ErrorKind` values to
fixed lowercase messages (`NotFound` → "no such file or directory",
`PermissionDenied` → "permission denied") through `error::io_message`,
and formats both file-reading failures — the manifest read in
`package::load` and the component read in `SandboxExtension::load` —
through it. Rarer kinds keep the operating system's own text, which
nothing pins. The os-error code suffix is dropped: the code for the
same kind differs by platform (access denied is 13 on Unix, 5 on
Windows), so quoting a code would not be portable either.

## Consequences

Case 15 passes on every platform with one expected line instead of a
per-OS expectation, and every binding surfaces the same message for
the same failure. The wording for a missing or unreadable file is now
part of the loader's contract; changing it is a conformance-kit
change, never a drive-by edit. Unit tests pin both the kind mapping
and the real missing-file path; detailed OS diagnostics for exotic
I/O kinds remain visible through the unnormalized fallback.
