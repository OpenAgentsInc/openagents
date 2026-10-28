#ifndef COAX_CRYPTO_ZEROIZE_H
#define COAX_CRYPTO_ZEROIZE_H
#include <stdint.h>
#include "zeroize-policy.h" /* Generated from ../zeroize.coax. */
#ifdef __cplusplus
extern "C" {
#endif
/* Buffer pointers/capacities must be truthful and remain live until complete.
 * A fresh scope is zero-initialized. Reset refuses pending cleanup. */
typedef struct {coax_t_ZeroScope policy;void *buffers[8];} coax_secret_scope;
uint32_t coax_secure_zero(void *buffer,int64_t capacity,int64_t offset,int64_t length);
uint32_t coax_secret_register(coax_secret_scope *scope,uint64_t id,void *buffer,int64_t capacity,int64_t secret_prefix);
uint32_t coax_secret_copy(coax_secret_scope *scope,uint64_t source,uint64_t destination,int64_t length);
/* reason: 0 success, 1 error, 2 cancellation. Failed effects stay pending. */
uint32_t coax_secret_close(coax_secret_scope *scope,uint32_t reason);
uint32_t coax_secret_reset(coax_secret_scope *scope);
#ifdef __cplusplus
}
#endif
#endif
