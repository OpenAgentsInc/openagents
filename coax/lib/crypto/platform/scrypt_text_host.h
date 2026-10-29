#ifndef COAX_CRYPTO_SCRYPT_TEXT_HOST_H
#define COAX_CRYPTO_SCRYPT_TEXT_HOST_H
#include "scrypt_host.h"
#include "nfkc.h"
/* Caller owns and retains this disjoint, aligned workspace; it can contain
 * secret material until a successful erase. Calls are serialized. */
typedef struct {uint32_t scratch[18432];uint8_t normalized[1024];} coax_scrypt_text_workspace;
/* 0 starts the job. 1 refuses (host cleanup may still be pending). 2 means
 * workspace erasure failed: quarantine it and finish any host cleanup. Early
 * pointer/admission refusal leaves pre-existing workspace bytes untouched.
 * Raw password remains caller-owned on every path. Internal prototype. */
int coax_scrypt_text_start(coax_scrypt_host *,coax_scrypt_text_workspace *,const uint8_t *,int64_t,const uint8_t *,int64_t,uint8_t *,int64_t,int64_t);
#endif
