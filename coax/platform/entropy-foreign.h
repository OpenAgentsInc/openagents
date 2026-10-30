#ifndef COAX_ENTROPY_FOREIGN_H
#define COAX_ENTROPY_FOREIGN_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* OS entropy for Coax programs through #foreign scalars, for small values
   such as WebSocket mask keys, handshake nonces and backoff jitter. It is a
   thin bridge over coax_entropy_native (entropy-native.c): the request goes
   through the same Coax admission and completion policy
   (coax/lib/crypto/entropy.coax), so this file decides nothing.

   cxrand_fill(n) draws n bytes (at most CXRAND_MAX) and returns 0, or the
   policy's ENTROPY_* error with no bytes held. cxrand_byte(i) returns byte i
   of the last successful draw, or -1 past it. Each draw replaces the last;
   a failed draw clears it. Not for keys or long-lived secrets: the bytes sit
   in a static buffer until the next draw. */
#define CXRAND_MAX (64)
int64_t cxrand_fill(int64_t n);
int64_t cxrand_byte(int64_t i);
#ifdef __cplusplus
}
#endif
#endif
