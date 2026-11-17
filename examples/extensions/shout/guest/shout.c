/* The `shout` example's tier-2 guest, in C: a `::: shout` block
 * container whose content renders uppercased — logic the declarative
 * tier cannot express.
 *
 * The bindings next to this file are generated from the WIT:
 *
 *   wit-bindgen c crates/geist4-ext-loader/wit/geist4-extension.wit \
 *     --out-dir examples/extensions/shout/guest
 *
 * and the component is built with:
 *
 *   clang --target=wasm32-unknown-unknown -O2 -nostdlib -isystem compat \
 *     -Wl,--no-entry -Wl,--export-dynamic \
 *     shout.c shim.c geist4_extension.c geist4_extension_component_type.o \
 *     -o shout-core.wasm
 *   wasm-tools component new shout-core.wasm -o ../shout.wasm
 */
#include "geist4_extension.h"
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

/* Provided by shim.c (the component links no libc). */
void *cabi_realloc(void *ptr, size_t old_size, size_t align, size_t new_size);
void free(void *ptr);
void *memcpy(void *dst, const void *src, size_t n);

static const char KIND[] = "shout";
static const char MARKER[] = "::: shout";

/* The line is the opening fence: `::: shout` at the start, then
 * end-of-line or whitespace. */
static bool is_marker_line(geist4_extension_string_t *line) {
  size_t n = sizeof(MARKER) - 1;
  if (line->len < n) return false;
  for (size_t i = 0; i < n; i++) {
    if (line->ptr[i] != MARKER[i]) return false;
  }
  if (line->len == n) return true;
  uint8_t c = line->ptr[n];
  return c == ' ' || c == '\t' || c == '\n' || c == '\r';
}

/* The line is the closing fence: `:::` and nothing after it but
 * whitespace. */
static bool is_close_fence(geist4_extension_string_t *line) {
  if (line->len < 3) return false;
  if (line->ptr[0] != ':' || line->ptr[1] != ':' || line->ptr[2] != ':') {
    return false;
  }
  for (size_t i = 3; i < line->len; i++) {
    uint8_t c = line->ptr[i];
    if (c != ' ' && c != '\t' && c != '\n' && c != '\r') return false;
  }
  return true;
}

bool exports_geist4_extension_hooks_init(
    exports_geist4_extension_hooks_list_kind_decl_t *kinds,
    exports_geist4_extension_hooks_list_tuple2_string_option_value_t *options,
    geist4_extension_string_t *err) {
  (void)options;
  for (size_t i = 0; i < kinds->len; i++) {
    geist4_extension_string_t *k = &kinds->ptr[i].kind;
    if (k->len != sizeof(KIND) - 1) {
      geist4_extension_string_set(err, "unexpected kind");
      return false;
    }
    for (size_t j = 0; j < k->len; j++) {
      if (k->ptr[j] != KIND[j]) {
        geist4_extension_string_set(err, "unexpected kind");
        return false;
      }
    }
  }
  return true;
}

bool exports_geist4_extension_hooks_block_start_look(
    geist4_extension_string_t *line, bool indented, uint32_t column) {
  (void)indented;
  (void)column;
  return is_marker_line(line);
}

bool exports_geist4_extension_hooks_block_start(
    geist4_extension_string_t *line, bool indented, uint32_t column,
    geist4_extension_string_t *maybe_container_kind,
    exports_geist4_extension_hooks_start_t *ret) {
  (void)indented;
  (void)column;
  (void)maybe_container_kind;
  if (!is_marker_line(line)) return false;
  geist4_extension_string_set(&ret->kind, KIND);
  ret->attrs.ptr = NULL;
  ret->attrs.len = 0;
  ret->consume.tag = EXPORTS_GEIST4_EXTENSION_HOOKS_CONSUME_WHOLE_LINE;
  return true;
}

bool exports_geist4_extension_hooks_block_continue(
    geist4_extension_string_t *line, bool indented, uint32_t column, bool blank,
    exports_geist4_extension_hooks_continue_decision_t *ret) {
  (void)column;
  if (blank) return false;
  /* The closing fence closes the container and consumes the line
   * whole. Indented four or more columns it is content, not a fence. */
  if (!indented && is_close_fence(line)) {
    ret->tag = EXPORTS_GEIST4_EXTENSION_HOOKS_CONTINUE_DECISION_CLOSE_LINE;
    return true;
  }
  /* Anything else continues the container with the whole line as
   * content. */
  ret->tag = EXPORTS_GEIST4_EXTENSION_HOOKS_CONTINUE_DECISION_KEEP;
  ret->val.keep.tag = EXPORTS_GEIST4_EXTENSION_HOOKS_CONSUME_REST;
  return true;
}

/* Uppercases ASCII letters outside tags the engine's inline renderer
 * emitted, and wraps the result in the container's HTML. */
bool exports_geist4_extension_hooks_render(
    geist4_extension_string_t *kind,
    geist4_extension_list_tuple2_string_string_t *attrs,
    geist4_extension_string_t *children,
    geist4_extension_string_t *ret,
    geist4_extension_string_t *err) {
  (void)attrs;
  if (kind->len != sizeof(KIND) - 1) {
    geist4_extension_string_set(err, "unknown kind");
    return false;
  }
  for (size_t i = 0; i < kind->len; i++) {
    if (kind->ptr[i] != KIND[i]) {
      geist4_extension_string_set(err, "unknown kind");
      return false;
    }
  }
  size_t n = children->len;
  char *out = (char *)cabi_realloc(NULL, 0, 1, n + 64);
  size_t o = 0;
  const char *pre = "<div class=\"shout\">\n";
  for (size_t i = 0; pre[i] != '\0'; i++) out[o++] = pre[i];
  bool in_tag = false;
  for (size_t i = 0; i < n; i++) {
    char c = (char)children->ptr[i];
    if (in_tag) {
      out[o++] = c;
      if (c == '>') in_tag = false;
    } else if (c == '<') {
      in_tag = true;
      out[o++] = c;
    } else if (c >= 'a' && c <= 'z') {
      out[o++] = c - 32;
    } else {
      out[o++] = c;
    }
  }
  const char *post = "</div>\n";
  for (size_t i = 0; post[i] != '\0'; i++) out[o++] = post[i];
  out[o] = '\0';
  geist4_extension_string_set(ret, out);
  free(out);
  return true;
}

bool exports_geist4_extension_hooks_block_close(
    exports_geist4_extension_hooks_rendered_block_t *ret) {
  (void)ret;
  return false;
}

bool exports_geist4_extension_hooks_inline_trigger(
    geist4_extension_string_t *line, uint32_t pos,
    exports_geist4_extension_hooks_inline_action_t *ret) {
  (void)line;
  (void)pos;
  (void)ret;
  return false;
}

bool exports_geist4_extension_hooks_inline_bracket(
    geist4_extension_string_t *content,
    exports_geist4_extension_hooks_inline_action_t *ret) {
  (void)content;
  (void)ret;
  return false;
}

void exports_geist4_extension_hooks_post_process(
    geist4_extension_string_t *text, geist4_extension_string_t *ret) {
  /* The host's string is (ptr, len), not NUL-terminated: copy the
   * exact bytes. */
  char *buf = (char *)cabi_realloc(NULL, 0, 1, text->len + 1);
  memcpy(buf, text->ptr, text->len);
  buf[text->len] = '\0';
  geist4_extension_string_set(ret, buf);
  free(buf);
}
