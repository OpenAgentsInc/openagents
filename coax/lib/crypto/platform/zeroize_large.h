#ifndef COAX_CRYPTO_ZEROIZE_LARGE_H
#define COAX_CRYPTO_ZEROIZE_LARGE_H
#include <stdint.h>
#include "zeroize-large-policy.h"

/* A fresh scope must be zero-initialized before begin. Caller owns one live
 * allocation, keeps it stable and disjoint from scope,
 * and serializes calls. A scope cannot be reused before explicit reset. */
typedef struct {coax_t_ZeroLarge policy;uint8_t *buffer;int active;} coax_large_wipe;
uint32_t coax_large_wipe_begin(coax_large_wipe *scope,void *buffer,int64_t length);
/* At most one 1 MiB chunk per call. ZERO_PENDING means more work remains. */
uint32_t coax_large_wipe_step(coax_large_wipe *scope);
uint32_t coax_large_wipe_reset(coax_large_wipe *scope);
#endif
