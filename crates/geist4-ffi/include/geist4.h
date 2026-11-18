/* Generated with cbindgen:0.29.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * `block-start`: open with the whole line consumed.
 */
#define GEIST4_START_WHOLE_LINE 0

/**
 * `block-start`: seek past the content, open, keep the rest of the
 * line after `columns` columns as the block's first paragraph.
 */
#define GEIST4_START_COLUMNS 1

/**
 * `block-start`: seek to the content, open, keep the whole rest of
 * the line as the block's first paragraph.
 */
#define GEIST4_START_REST 2

/**
 * `block-start`: the task-list pattern — consume `columns` bytes,
 * claim the deepest plain container, open nothing.
 */
#define GEIST4_START_CLAIM 3

/**
 * `block-continue`: consume `columns` more columns and keep going.
 */
#define GEIST4_CONTINUE_KEEP 0

/**
 * `block-continue`: close the block and consume the line whole.
 */
#define GEIST4_CONTINUE_CLOSE_LINE 1

/**
 * Inline: leave the bytes as text.
 *
 * A C-surface value only: the Rust side declines by returning `None`.
 */
#define GEIST4_INLINE_LITERAL 0

/**
 * Inline: tag the scanned run with the decision's kind.
 */
#define GEIST4_INLINE_DELIMITER 1

/**
 * Inline: claim the bracket group's content as a node.
 */
#define GEIST4_INLINE_NODE 2

/**
 * A hook outcome: no HTML, no decision, decline.
 *
 * A C-surface value only: the Rust side declines by returning `None`.
 */
#define GEIST4_OUT_NONE 0

/**
 * A hook outcome: the HTML in `html` applies.
 */
#define GEIST4_OUT_HTML 1

/**
 * Attribute tag: `text` carries a string value.
 */
#define GEIST4_ATTR_STR 0

/**
 * Attribute tag: `number` carries an integer value.
 */
#define GEIST4_ATTR_INT 1

/**
 * Attribute tag: `number` carries a boolean value (zero or non-zero).
 */
#define GEIST4_ATTR_BOOL 2

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
  /**
   * A host extension's declared data broke the manifest's rules; the
   * error buffer carries the reason and nothing was registered.
   */
  GEIST4_ERR_INVALID = 6,
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
 * A borrowed byte span. The engine fills spans it passes to a hook;
 * the host fills spans it hands back, and the engine reads them
 * before the call returns.
 */
typedef struct Geist4Span {
  /**
   * The bytes, borrowed for the call. Null with `len == 0` is an
   * empty span.
   */
  const uint8_t *data;
  /**
   * The span's length in bytes.
   */
  size_t len;
} Geist4Span;

/**
 * One declared kind: a name, its two shape flags and the attribute
 * names the kind carries.
 */
typedef struct Geist4KindDecl {
  /**
   * The kind's name.
   */
  struct Geist4Span kind;
  /**
   * Non-zero when the kind opens a block that holds blocks.
   */
  int32_t container;
  /**
   * Non-zero when the kind may hold inline text.
   */
  int32_t inline_text;
  /**
   * The kind's attribute names.
   */
  const struct Geist4Span *attrs;
  /**
   * The number of attribute names.
   */
  size_t attrs_len;
} Geist4KindDecl;

/**
 * One attribute: a name and a typed value. The value's shape follows
 * `tag`: [`GEIST4_ATTR_STR`] reads `text`, [`GEIST4_ATTR_INT`] reads
 * `number`, [`GEIST4_ATTR_BOOL`] reads `number` as zero or non-zero.
 * Unused fields stay zero/null.
 */
typedef struct Geist4Attr {
  /**
   * The attribute's name.
   */
  struct Geist4Span name;
  /**
   * Which field carries the value: [`GEIST4_ATTR_STR`],
   * [`GEIST4_ATTR_INT`] or [`GEIST4_ATTR_BOOL`].
   */
  int32_t tag;
  /**
   * The value for the integer and boolean tags.
   */
  int64_t number;
  /**
   * The value for the string tag.
   */
  struct Geist4Span text;
} Geist4Attr;

/**
 * A `block-start` decision. The `tag` picks both the consume and how
 * the other fields read:
 *
 * * [`GEIST4_START_WHOLE_LINE`] — open with the whole line consumed;
 *   `kind` and the attributes apply.
 * * [`GEIST4_START_COLUMNS`] — seek `columns` columns past the
 *   content and open, leaving the rest of the line as the block's
 *   first paragraph; `kind`, `columns` and the attributes apply.
 * * [`GEIST4_START_REST`] — seek to the content and open, leaving
 *   the whole rest of the line as the block's first paragraph; `kind`
 *   and the attributes apply.
 * * [`GEIST4_START_CLAIM`] — the task-list pattern: consume `columns`
 *   bytes off the line, claim the deepest plain container with the
 *   attributes, and open nothing; `columns` and the attributes apply,
 *   `kind` is ignored.
 */
typedef struct Geist4StartDecision {
  /**
   * Which consume the decision names.
   */
  int32_t tag;
  /**
   * The extension kind to open; ignored for the claim tag.
   */
  struct Geist4Span kind;
  /**
   * The attributes to set on the opened or claimed node.
   */
  const struct Geist4Attr *attrs;
  /**
   * The number of attributes.
   */
  size_t attrs_len;
  /**
   * The columns to seek for [`GEIST4_START_COLUMNS`], or the bytes
   * to consume for [`GEIST4_START_CLAIM`].
   */
  uint32_t columns;
} Geist4StartDecision;

/**
 * A `block-continue` decision: [`GEIST4_CONTINUE_KEEP`] consumes the
 * line's remaining columns as `columns` names (`u32::MAX` for every
 * remaining column, `0` for none) and keeps the block open;
 * [`GEIST4_CONTINUE_CLOSE_LINE`] closes the block and consumes the
 * line whole.
 */
typedef struct Geist4ContinueDecision {
  /**
   * Which continue the decision names.
   */
  int32_t tag;
  /**
   * The columns to consume for the keep tag.
   */
  uint32_t columns;
} Geist4ContinueDecision;

/**
 * A rendering a hook supplies: [`GEIST4_OUT_HTML`] with `html` set,
 * or [`GEIST4_OUT_NONE`] to decline.
 */
typedef struct Geist4Rendered {
  /**
   * Which outcome the hook chose.
   */
  int32_t tag;
  /**
   * The HTML, for the [`GEIST4_OUT_HTML`] tag.
   */
  struct Geist4Span html;
} Geist4Rendered;

/**
 * An inline decision. The `tag` picks the action:
 *
 * * [`GEIST4_INLINE_LITERAL`] — leave the bytes as text.
 * * [`GEIST4_INLINE_DELIMITER`] — tag the run at the cursor with
 *   `kind` (the `inline-trigger` hook only).
 * * [`GEIST4_INLINE_NODE`] — claim the bracket group's content as an
 *   inline node of `kind`, keeping up to `end` bytes of it as the
 *   node's children (the `inline-bracket` hook only; the trigger
 *   hook has no claim API and treats this tag as literal, the way
 *   tier 2 does).
 */
typedef struct Geist4InlineDecision {
  /**
   * Which action the decision names.
   */
  int32_t tag;
  /**
   * The extension kind for the delimiter and node tags.
   */
  struct Geist4Span kind;
  /**
   * The content length to keep for the node tag; the whole group
   * when it reaches past the content.
   */
  uint32_t end;
  /**
   * The typed attributes for the node tag, read with
   * [`read_attrs`]; null with zero length for none. Ignored for the
   * other tags.
   */
  const struct Geist4Attr *attrs;
  /**
   * The attribute count; zero when `attrs` is null.
   */
  size_t attrs_len;
} Geist4InlineDecision;

/**
 * The engine-provided fetch a render-override hook pulls children
 * with. `index` counts from zero up to the child count the hook was
 * handed; the engine writes the child's HTML into `out`. Returns `0`
 * when the child was delivered, negative when `index` is past the
 * last child, which is how the host stops.
 */
typedef int32_t (*Geist4ChildFetch)(void *ctx, uint32_t index, struct Geist4Span *out);

/**
 * The tier-3 vtable: the declared data plus one function pointer per
 * hook, every hook optional. A `None` hook declines.
 *
 * The declared data is read and copied during
 * [`HostExtension::from_vtable`]; the host may free it afterwards.
 * The context pointer and the function pointers must stay valid for
 * as long as the extension does.
 */
typedef struct Geist4HostVTable {
  /**
   * The host's context, the first argument of every hook.
   */
  void *ctx;
  /**
   * Called with the context when the last engine-side reference to
   * the extension drops. Optional.
   */
  void (*free_ctx)(void *ctx);
  /**
   * The extension's name.
   */
  struct Geist4Span name;
  /**
   * The declared kinds.
   */
  const struct Geist4KindDecl *kinds;
  /**
   * The number of declared kinds.
   */
  size_t kinds_len;
  /**
   * The inline trigger bytes to register.
   */
  const uint8_t *triggers;
  /**
   * The number of trigger bytes.
   */
  size_t triggers_len;
  /**
   * The `init` hook, or null. See [`Geist4HookInit`] for the
   * contract.
   */
  int32_t (*init)(void *ctx);
  /**
   * The `block-start-look` hook, or null. See [`Geist4HookLook`]
   * for the contract.
   */
  int32_t (*block_start_look)(void *ctx,
                              const uint8_t *line,
                              size_t line_len,
                              int32_t indented,
                              uint32_t column);
  /**
   * The `block-start` hook, or null. See [`Geist4HookStart`] for
   * the contract.
   */
  int32_t (*block_start)(void *ctx,
                         const uint8_t *line,
                         size_t line_len,
                         int32_t indented,
                         uint32_t column,
                         const uint8_t *container_kind,
                         size_t container_kind_len,
                         struct Geist4StartDecision *out);
  /**
   * The `block-continue` hook, or null. See [`Geist4HookContinue`]
   * for the contract.
   */
  int32_t (*block_continue)(void *ctx,
                            const uint8_t *line,
                            size_t line_len,
                            int32_t indented,
                            uint32_t column,
                            int32_t blank,
                            struct Geist4ContinueDecision *out);
  /**
   * The `block-close` hook, or null. See [`Geist4HookClose`] for
   * the contract.
   */
  int32_t (*block_close)(void *ctx, struct Geist4Rendered *out);
  /**
   * The `inline-trigger` hook, or null. See [`Geist4HookTrigger`]
   * for the contract.
   */
  int32_t (*inline_trigger)(void *ctx,
                            const uint8_t *line,
                            size_t line_len,
                            uint32_t pos,
                            struct Geist4InlineDecision *out);
  /**
   * The `inline-bracket` hook, or null. See [`Geist4HookBracket`]
   * for the contract.
   */
  int32_t (*inline_bracket)(void *ctx,
                            const uint8_t *content,
                            size_t content_len,
                            struct Geist4InlineDecision *out);
  /**
   * The `post-process` hook, or null. See [`Geist4HookPostProcess`]
   * for the contract.
   */
  int32_t (*post_process)(void *ctx, const uint8_t *text, size_t text_len, struct Geist4Span *out);
  /**
   * The `render` hook, or null. See [`Geist4HookRender`] for the
   * contract.
   */
  int32_t (*render)(void *ctx,
                    const uint8_t *kind,
                    size_t kind_len,
                    const struct Geist4Attr *attrs,
                    size_t attrs_len,
                    const uint8_t *children_html,
                    size_t children_html_len,
                    struct Geist4Rendered *out);
  /**
   * The `render_override` hook, or null. See
   * [`Geist4HookRenderOverride`] for the contract.
   */
  int32_t (*render_override)(void *ctx,
                             const uint8_t *kind,
                             size_t kind_len,
                             const struct Geist4Attr *attrs,
                             size_t attrs_len,
                             uint32_t n_children,
                             Geist4ChildFetch fetch,
                             void *fetch_ctx,
                             struct Geist4Rendered *out);
} Geist4HostVTable;

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
 * Registers a host extension (tier 3) from a vtable the caller owns.
 *
 * The vtable's declared data — the name, the kinds and the trigger
 * bytes — is read and copied during the call, so the host may free
 * that memory when the call returns. The context pointer and the
 * function pointers must stay valid for as long as the options value
 * and every document parsed with it. A vtable that breaks the
 * manifest's rules is rejected with `GEIST4_ERR_INVALID` and the
 * reason in the error buffer; nothing is registered.
 *
 * # Safety
 * `options` must be a live handle from `geist4_options_create`;
 * `vtable` must be a valid read for the duration of the call, and its
 * spans must be valid reads; `err` must be null or writable for
 * `err_cap` bytes.
 */
geist4_status geist4_options_enable_host(struct geist4_options *options,
                                         const struct Geist4HostVTable *vtable,
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
