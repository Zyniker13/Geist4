/* The libc stand-ins the generated bindings and the guest need. The
 * component links no libc: a bump heap over a static buffer, a
 * no-op free (the host's memory limit bounds the damage), and a
 * trapping abort. */
#include <stddef.h>
#include <stdint.h>

static uint8_t heap[1 << 20];
static size_t heap_pos;

void *memcpy(void *dst, const void *src, size_t n) {
  uint8_t *d = (uint8_t *)dst;
  const uint8_t *s = (const uint8_t *)src;
  for (size_t i = 0; i < n; i++) d[i] = s[i];
  return dst;
}

size_t strlen(const char *s) {
  size_t n = 0;
  while (s[n] != '\0') n++;
  return n;
}

/* The component model requires a `cabi_realloc` export. The
 * generated bindings define one (weak, over libc's realloc, which
 * does not exist here); this strong definition wins and carries the
 * export. */
__attribute__((__export_name__("cabi_realloc")))
void *cabi_realloc(void *ptr, size_t old_size, size_t align, size_t new_size) {
  (void)align;
  if (new_size == 0) return NULL;
  size_t off = heap_pos;
  if (off + new_size > sizeof(heap)) __builtin_trap();
  uint8_t *np = heap + off;
  heap_pos = off + new_size;
  if (ptr != NULL) {
    size_t copy = old_size < new_size ? old_size : new_size;
    memcpy(np, ptr, copy);
  }
  return np;
}

void free(void *ptr) { (void)ptr; }

void abort(void) { __builtin_trap(); }
