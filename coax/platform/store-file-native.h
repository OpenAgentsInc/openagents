#ifndef COAX_STORE_FILE_NATIVE_H
#define COAX_STORE_FILE_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* The verified Nostr store's append-only file (coax/lib/nostr/store_saved.coax).
   File effects only: framing, replay and every admission decision are Coax.
   The host calls cxstore_set_path before the Coax program starts. */
int cxstore_set_path(const char *path);
/* Opens (creating, mode 0600) and exclusively locks the file, and reads it
   whole: its size, or -1. */
int64_t cxstore_load(void);
/* Byte i of what cxstore_load read, or -1. */
int64_t cxstore_byte(int64_t i);
/* Buffers one byte (0..255) for the next commit: 0, or -1. */
int64_t cxstore_put(int64_t b);
/* Appends the buffered bytes at the end of the file and fsyncs: 0, or -1
   after cutting the file back to its size before the append. The buffer is
   empty afterwards either way. */
int64_t cxstore_commit(void);
/* Cuts the file to n bytes and fsyncs: 0, or -1. */
int64_t cxstore_truncate(int64_t n);
#ifdef __cplusplus
}
#endif
#endif
