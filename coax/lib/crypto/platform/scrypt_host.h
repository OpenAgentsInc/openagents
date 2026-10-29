#ifndef COAX_CRYPTO_SCRYPT_HOST_H
#define COAX_CRYPTO_SCRYPT_HOST_H
/* Internal prototype; the caller must initialize this object to zero, keep
 * every supplied allocation valid, disjoint and stable until close, and
 * serialize all calls. Password is already NFKC UTF-8 and remains caller-owned.
 * Not an iOS key-custody, backup or production-secret interface. */
#include <stdint.h>
#include "scrypt-job.h"
#include "scrypt-admission.h"
#include "zeroize_large.h"

typedef struct {
    coax_t_ScryptJob job;
    coax_large_wipe wiping;
    uint8_t *scratch;
    int64_t scratch_count;
} coax_scrypt_host;
/* 0 accepted, 1 rejected: even a rejected start can leave cleanup pending. */
int coax_scrypt_host_start(coax_scrypt_host *,const uint8_t *,int64_t,const uint8_t *,int64_t,uint8_t *,int64_t,int64_t);
/* Return the job's status: -1 invalid, -2 running, -3 published, 0 cleaned,
 * or a positive region requiring cleanup. */
int64_t coax_scrypt_host_status(coax_scrypt_host *);
int coax_scrypt_host_step(coax_scrypt_host *,int64_t);
int coax_scrypt_host_cancel(coax_scrypt_host *);
/* 0 means cleaned, 1 more cleanup, 2 invalid state, 3 effect failure. */
int coax_scrypt_host_clean(coax_scrypt_host *);
/* 0 publishes a caller-owned key and clears the staged key. 1 refuses before
 * writing output. 2 means a post-publication wipe failed: output may contain
 * key material even if the attempted output wipe also failed. Quarantine and
 * erase that output; close must still erase the host. */
int coax_scrypt_host_publish(coax_scrypt_host *,uint8_t *,int64_t);
/* Wipes the host after cleanup. Closing an unpublished success first
 * abandons it and returns failure while staged-key cleanup is pending. */
int coax_scrypt_host_close(coax_scrypt_host *);
#endif
