#ifndef COAX_CRYPTO_AEAD_HOST_H
#define COAX_CRYPTO_AEAD_HOST_H
/* Internal prototype. Callers own live, aligned, stable, disjoint allocations,
 * must zero-initialize the host, serialize calls, and manage nonce uniqueness.
 * Lower-level cipher stack temporaries are not guaranteed erased. */
#include <stdint.h>
#include "aead-workspace.h"
#include "aead-admission.h"
typedef struct {coax_t_AeadOperation operation;coax_t_AeadScratch *workspace;uint8_t *output;int64_t capacity;} coax_aead_host;
/* 0 starts a successful computation; 1 refuses or authentication fails. A
 * failed start can still require cleanup, so inspect status in either case. */
int coax_aead_host_start(coax_aead_host *,coax_t_AeadScratch *,const uint8_t *,int64_t,const uint8_t *,int64_t,const uint8_t *,int64_t,const uint8_t *,int64_t,uint8_t *,int64_t,int);
/* -1 invalid, -2 running, -3 published, 0 cleaned, positive region to wipe. */
int64_t coax_aead_host_status(coax_aead_host *);
int coax_aead_host_cancel(coax_aead_host *);
/* 0 cleaned, 1 more cleanup, 2 invalid state, 3 wipe failure. */
int coax_aead_host_clean(coax_aead_host *);
/* 0 success, 1 refused before output, 2 post-publication wipe failed: output
 * may contain sensitive material and must be quarantined and erased. */
int coax_aead_host_publish(coax_aead_host *);
/* Closing unpublished success first requests cancellation and staged wipe. */
int coax_aead_host_close(coax_aead_host *);
#endif
