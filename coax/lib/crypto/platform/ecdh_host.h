#ifndef COAX_ECDH_HOST_H
#define COAX_ECDH_HOST_H
#include <stdint.h>
#include "ecdh.h"
/* Experimental identified-workspace wrapper. Pointers, capacities and
 * lifetimes are caller guarantees; the caller serializes access and keeps
 * output private until successful return. Inputs, output and work must be
 * disjoint. Hidden compiler/ABI copies and production-secret approval are
 * outside this contract. On cleanup-pending, retain work and output privately
 * and retry erasure before reuse. Invalid metadata refuses before ownership. */
#define COAX_ECDH_METADATA 4u
#define COAX_ECDH_CLEANUP_PENDING 5u
uint32_t coax_ecdh_raw(const uint8_t *secret,int64_t secret_len,const uint8_t *peer,int64_t peer_len,uint8_t *output,int64_t capacity,coax_t_EcdhScratch *work);
uint32_t coax_ecdh_cleanup(coax_t_EcdhScratch *work,uint8_t *output,int64_t capacity);
#endif
