/* Generated with cbindgen:0.29.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * The outcome of an FFI call.
 *
 * Every fallible entry point returns this; anything but `GEIST4_OK`
 * leaves the output argument untouched and reports one line into the
 * caller's error buffer.
 */
enum geist4_status {
  /**
   * The call succeeded.
   */
  GEIST4_OK = 0,
  /**
   * A required pointer argument was null.
   */
  GEIST4_ERR_NULL = 1,
  /**
   * The input bytes are not valid UTF-8; the error buffer names the
   * byte offset.
   */
  GEIST4_ERR_UTF8 = 2,
  /**
   * The extension name is not one the engine knows; the error buffer
   * lists the valid names.
   */
  GEIST4_ERR_UNKNOWN_EXT = 3,
  /**
   * Renderer output would have contained an interior NUL byte, which
   * the engine never produces; this is an internal invariant failure.
   */
  GEIST4_ERR_OUTPUT = 4,
  /**
   * The engine panicked; the panic was caught at the boundary and the
   * error buffer carries the message. This is always an engine bug.
   */
  GEIST4_ERR_PANIC = 5,
};
typedef int32_t geist4_status;

/**
 * A parsed document, owning the source it was parsed from.
 *
 * The document borrows its text from the input, so the handle keeps a
 * private copy of the input alive (decision 0028). Release with
 * `geist4_document_free`; every pointer borrowed from the handle — the
 * problem messages — is invalid afterwards.
 */
typedef struct geist4_document geist4_document;

/**
 * The parse options: which built-in extensions are registered.
 *
 * Created with `geist4_options_create`, configured with
 * `geist4_options_enable_builtin`, released with `geist4_options_free`.
 * Passing null where one is expected parses plain `CommonMark`.
 */
typedef struct geist4_options geist4_options;

/**
 * An owned UTF-8 string the engine handed out.
 *
 * `data` is always non-null and NUL-terminated; `len` counts the bytes
 * without the terminator. The caller owns the string and releases it
 * with `geist4_string_free`; mutating the bytes is allowed as long as
 * the NUL terminator stays in place.
 */
typedef struct geist4_string {
  /**
   * The bytes, NUL-terminated. Not null even when `len` is zero.
   */
  char *data;
  /**
   * The number of bytes before the terminator.
   */
  size_t len;
} geist4_string;

/**
 * One problem the parse noticed in its source.
 *
 * `message` points into memory the document handle owns: it stays
 * valid until that handle is freed.
 */
typedef struct geist4_problem {
  /**
   * What the problem is, as one line, UTF-8, not NUL-terminated.
   */
  const char *message;
  /**
   * The length of `message` in bytes.
   */
  size_t message_len;
  /**
   * The span's start, as a byte offset into the source.
   */
  size_t start;
  /**
   * The span's end, exclusive, as a byte offset into the source.
   */
  size_t end;
} geist4_problem;

/**
 * A list of problems, borrowed from the document that produced it.
 *
 * `items` is null only when `len` is zero. The memory belongs to the
 * `geist4_document`; it stays valid until that handle is freed.
 */
typedef struct geist4_problem_list {
  /**
   * The problems, in document order.
   */
  const struct geist4_problem *items;
  /**
   * How many problems the list holds.
   */
  size_t len;
} geist4_problem_list;

/**
 * The version of the engine, as a static NUL-terminated string the
 * caller never frees.
 *
 * # Safety
 * The caller must not free or mutate the returned pointer.
 */
const char *geist4_version(void);

/**
 * Creates an empty options handle: plain `CommonMark` until a built-in
 * is enabled.
 */
struct geist4_options *geist4_options_create(void);

/**
 * Frees an options handle. Null is accepted and ignored, matching
 * `free(NULL)`. Documents already parsed with it keep working: they
 * hold their own copy of the registration.
 *
 * # Safety
 * `options` must be null or a handle this library handed out that has
 * not been freed yet.
 */
void geist4_options_free(struct geist4_options *options);

/**
 * Registers a built-in extension by name.
 *
 * Names: `tables`, `strikethrough`, `autolink`, `tasklist`,
 * `tagfilter`, `footnotes`, `front_matter`, `heading_identifiers`,
 * `description_lists`, and the bundles `gfm` (the five GitHub Flavored
 * Markdown ones) and `all` (every built-in). Enabling an extension
 * that is already registered does nothing, so bundles and single names
 * compose freely. Registration order is run order; a bundle inserts
 * its members in the engine's canonical order.
 *
 * # Safety
 * `options` must be a live handle from `geist4_options_create`;
 * `name` must be readable for `name_len` bytes for the duration of the
 * call, or null when `name_len` is zero; `err` must be null or
 * writable for `err_cap` bytes.
 */
geist4_status geist4_options_enable_builtin(struct geist4_options *options,
                                            const char *name,
                                            size_t name_len,
                                            char *err,
                                            size_t err_cap);

/**
 * Parses a document.
 *
 * `options` may be null for plain `CommonMark`. The bytes must be
 * valid UTF-8; a NUL byte is ordinary content, as in any Markdown
 * source. `data` may be null only when `len` is zero, which parses an
 * empty document. On success the handle owns a copy of the input, so
 * the caller may free `data` as soon as the call returns. On failure
 * null is returned and the error buffer explains why.
 *
 * # Safety
 * `data` must be readable for `len` bytes for the duration of the
 * call, or null when `len` is zero; `err` must be null or writable for
 * `err_cap` bytes.
 */
struct geist4_document *geist4_parse(const struct geist4_options *options,
                                     const char *data,
                                     size_t len,
                                     char *err,
                                     size_t err_cap);

/**
 * Renders a parsed document's HTML into an owned string.
 *
 * The extensions registered on the options the document was parsed
 * with render the node kinds they declared, exactly as in the Rust
 * API. On failure `out` is untouched.
 *
 * # Safety
 * `document` must be a live handle from `geist4_parse`; `out` must be
 * writable; `err` must be null or writable for `err_cap` bytes.
 */
geist4_status geist4_document_html(const struct geist4_document *document,
                                   struct geist4_string *out,
                                   char *err,
                                   size_t err_cap);

/**
 * The document's problems: broken references and similar issues the
 * tree absorbed without changing the output.
 *
 * The list borrows from the document; it stays valid until
 * `geist4_document_free`. An empty list means no problems, not a null
 * pointer.
 *
 * # Safety
 * `document` must be a live handle from `geist4_parse`; `out` must be
 * writable; `err` must be null or writable for `err_cap` bytes.
 */
geist4_status geist4_document_problems(const struct geist4_document *document,
                                       struct geist4_problem_list *out,
                                       char *err,
                                       size_t err_cap);

/**
 * Frees a document handle and everything it owns: the document, its
 * private copy of the input, and the problem messages. Null is
 * accepted and ignored. Every pointer borrowed from the handle — the
 * problem messages — is invalid afterwards.
 *
 * # Safety
 * `document` must be null or a handle this library handed out that has
 * not been freed yet.
 */
void geist4_document_free(struct geist4_document *document);

/**
 * Parses and renders in one call, without a document handle.
 *
 * The convenient path for callers who only want HTML: on success `out`
 * holds the rendered document and the intermediate tree never leaves
 * this function. Semantics match `geist4_parse` followed by
 * `geist4_document_html`.
 *
 * # Safety
 * `data` must be readable for `len` bytes for the duration of the
 * call, or null when `len` is zero; `out` must be writable; `err` must
 * be null or writable for `err_cap` bytes.
 */
geist4_status geist4_render_html(const struct geist4_options *options,
                                 const char *data,
                                 size_t len,
                                 struct geist4_string *out,
                                 char *err,
                                 size_t err_cap);

/**
 * Frees a string the engine handed out. Null data is accepted and
 * ignored. After the call the string's bytes are invalid.
 *
 * # Safety
 * `value.data` must be null or a pointer one of this library's output
 * entry points handed out, freed exactly once; the allocation must not
 * have been grown or replaced by the caller.
 */
void geist4_string_free(struct geist4_string value);
