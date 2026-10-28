#ifndef COAX_ENTROPY_NATIVE_H
#define COAX_ENTROPY_NATIVE_H
#include <stdint.h>
#include "entropy-policy.h" /* generated from coax/lib/crypto/entropy.coax */
#ifdef __cplusplus
extern "C" {
#endif
/* Callers own valid requested prefixes in disjoint output/scratch storage.
   Output commits only on full OS success. Scratch is ordinarily reset after a
   provider attempt; this is not a secure-erasure or physical-span proof. */
coax_t_Entropy_Result coax_entropy_native(uint8_t *out,int64_t capacity,
                                        uint8_t *scratch,int64_t scratch_capacity,
                                        int64_t requested);
#ifdef __cplusplus
}
#endif
#endif
