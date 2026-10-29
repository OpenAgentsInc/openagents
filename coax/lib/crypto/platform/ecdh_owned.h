#ifndef COAX_ECDH_OWNED_H
#define COAX_ECDH_OWNED_H
#include <stdint.h>
/* Experimental one-slot owner. Handles are transferable bearer values: an
 * authorized caller must keep its handle private and serialize its own use.
 * Input and output storage must be truthful, live, exclusive, stable and
 * disjoint for the periods described below. Numeric checks cannot prove that.
 * The original secret input remains caller-owned and is not erased here.
 *
 * acquire returns zero on busy/exhaustion and a nonzero generation otherwise.
 * start borrows key/peer for that call; they must stay valid and unchanged
 * until it returns. Input ranges must remain allocated until close so that
 * later output admission can refuse overlap. A positive result remains in
 * owned staging and start returns PENDING; collect transfers it only after
 * work and staging erasure. A failure return after start denotes completed
 * owned-region cleanup. PENDING is incomplete, never success or failure.
 *
 * collect requires private writable output until it returns success or a
 * subsequent retry/cancel completes. It must stay live, writable, exclusive
 * and private across PENDING. BUSY means that call took no action and must be
 * retried; it does not admit a proposed output. Invalid output metadata leaves staging owned
 * and returns PENDING; cancel can discard it. retry and cancel use the same
 * handle and keep pending regions quarantined. close refuses incomplete work.
 * Handles identify this loaded process instance only. Fork, library reload,
 * process restart, signals, process failure, callback reentrancy and hidden
 * compiler copies are outside the contract. No target timing or production-
 * secret approval is implied. */
#define COAX_ECDH_OWNED_OK 0u
#define COAX_ECDH_OWNED_PEER 2u
#define COAX_ECDH_OWNED_SCALAR 3u
#define COAX_ECDH_OWNED_INVALID 4u
#define COAX_ECDH_OWNED_PENDING 5u
#define COAX_ECDH_OWNED_CANCELLED 6u
#define COAX_ECDH_OWNED_EFFECT 7u
#define COAX_ECDH_OWNED_BUSY 8u
uint64_t coax_ecdh_owned_acquire(void);
uint32_t coax_ecdh_owned_start(uint64_t handle,const uint8_t *secret,int64_t secret_len,const uint8_t *peer,int64_t peer_len);
uint32_t coax_ecdh_owned_collect(uint64_t handle,uint8_t *output,int64_t capacity);
uint32_t coax_ecdh_owned_retry(uint64_t handle);
uint32_t coax_ecdh_owned_cancel(uint64_t handle);
uint32_t coax_ecdh_owned_close(uint64_t handle);
#endif
