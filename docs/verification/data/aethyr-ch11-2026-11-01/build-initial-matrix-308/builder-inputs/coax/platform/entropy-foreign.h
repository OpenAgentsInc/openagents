#ifndef COAX_ENTROPY_FOREIGN_H
#define COAX_ENTROPY_FOREIGN_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* OS entropy for Coax programs through #foreign, for small values
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

/* cxrand_draw(out, n) draws n bytes straight into the caller's borrowed view
   (Coax `cxrand_draw :: (out: [] u8, n: int) -> int #foreign;`, spec 7.1): one
   call, no static copy. It returns 0 with out.data[0..n) written, or the
   policy's ENTROPY_* error with out untouched: the policy refuses a negative
   n, n > out.count and n > CXRAND_MAX (the scratch it draws through). The view is
   borrowed for the call only; this file keeps no pointer to it. */
/* The [] u8 view, spelled as a generated Coax header spells it (the schema
   macro lets this and a generated header be included together). */
#ifdef COAX_coax_v_u8_DEFINED
#ifndef COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
#error "incompatible Coax C type coax_v_u8"
#endif
#else
#define COAX_coax_v_u8_DEFINED
#define COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif
int64_t cxrand_draw(coax_v_u8 out, int64_t n);
#ifdef __cplusplus
}
#endif
#endif
