/* Minimal <stdlib.h> for the wasm32-unknown-unknown target, which
 * ships no sysroot headers. Only the declarations the generated
 * bindings use; the definitions live in shim.c. */
#ifndef COMPAT_STDLIB_H
#define COMPAT_STDLIB_H
#include <stddef.h>

void *realloc(void *ptr, size_t size);
void free(void *ptr);
void abort(void);

#endif
