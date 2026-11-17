# 0028 — The C ABI: one unsafe boundary, owned inputs, NUL-terminated outputs

## Context

M7's native bindings (Swift now; other C-hinged languages later) need a
stable C-compatible API. Two facts shape it. First, the core keeps
`#![forbid(unsafe_code)]`, but a C ABI must, by definition, hand out raw
pointers and cross an FFI boundary where panics are undefined behavior.
Second, the core's `Document<'a>` borrows its source text: node text is
slices of the input, so a document cannot outlive the buffer it was
parsed from — while a C caller's buffer can be freed at any moment
after the call returns.

## Decision

`geist4-ffi` is the one crate in the workspace where `unsafe` is
allowed: its `[lints]` override the workspace's `unsafe_code = "forbid"`
with `allow` plus `unsafe_op_in_unsafe_fn = "deny"`, so every unsafe
operation is an explicitly marked block a reviewer can enumerate. The
crate keeps the workspace's clippy set.

The boundary rules:

- Every entry point is an `unsafe extern "C" fn` that validates its
  arguments (null pointers are rejected with `GEIST4_ERR_NULL`), wraps
  its body in `catch_unwind`, and returns a `geist4_status` code. A
  panic becomes `GEIST4_ERR_PANIC`, never a cross-boundary unwind.
- Inputs are `(const char *data, size_t len)` pairs, not C strings, so
  documents may contain NUL bytes like any Markdown source. The bytes
  must be valid UTF-8; invalid input is rejected with
  `GEIST4_ERR_UTF8` and the offending offset in the error buffer.
- A `geist4_document` handle owns its input: the FFI copies the bytes
  into a `Box<str>` and hands the parse a `Box::leak`ed `&'static str`,
  so the document's borrowed text points into an allocation the handle
  also owns and frees last (document first, then input). The erase to
  `'static` is the crate's central unsafe invariant; it is confined to
  the handle constructor and destructor.
- Outputs are `geist4_string { char *data; size_t len; }`, owned by the
  caller and released with `geist4_string_free`. `data` is always
  non-null, NUL-terminated, and `len` counts the bytes without the
  terminator, so a C caller may use `strlen` or the length. The engine
  replaces NUL with U+FFFD at input, so renderer output cannot contain
  an interior NUL; if one ever appears the entry point fails with
  `GEIST4_ERR_OUTPUT` instead of building a lying C string.
- Errors that are not argument errors report one line into a caller
  provided `(char *err, size_t err_cap)` buffer, NUL-terminated and
  truncated to fit; the buffer is untouched on success.
- The options handle stores the registered built-ins; a document
  handle captures a clone of the options it was parsed with, so
  rendering after the options handle is freed keeps every extension
  alive by `Arc`.

## Consequences

Swift (and any other native binding) builds on one audited surface, and
the core's `forbid(unsafe)` stands everywhere else. The header is
generated with `cbindgen` and checked in; CI regenerates it and fails
on drift, and a small C test program compiles against the generated
header and runs against the shared library, pinning the contract: null
rejection, UTF-8 rejection, ownership discipline, and byte-exact HTML
for a few fixed documents. The `'static` erasure is the price of a
borrowing engine behind a C ABI; it is safe only because the handle
type is private and its free function is the sole path to the
allocation. A caller who frees a document twice or uses a handle after
`geist4_document_free` is outside the contract, as with any C library.
