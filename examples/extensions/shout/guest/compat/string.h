/* Minimal <string.h> for the wasm32-unknown-unknown target, which
 * ships no sysroot headers. Only the declarations the generated
 * bindings use; the definitions live in shim.c. */
#ifndef COMPAT_STRING_H
#define COMPAT_STRING_H
#include <stddef.h>

void *memcpy(void *dst, const void *src, size_t n);
size_t strlen(const char *s);

#endif
