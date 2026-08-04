#include <stddef.h>

/* secp256k1-sys 0.11's tiny wasm sysroot omits this C standard prototype. */
void *memmove(void *destination, const void *source, size_t length);
