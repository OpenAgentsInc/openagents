/* Serialized local streaming ASR, psionic-nemotron-streaming-en-v1 (CUDA).
 * Samples are finite mono F32 at 16 kHz. One current media epoch, at most
 * 60 s and 16,384 samples per push. Partial text is display-only; a caller
 * admits final text under its own current voice/composer ticket. */
#ifndef PSIONIC_INCREMENTAL_H
#define PSIONIC_INCREMENTAL_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct psionic_incremental psionic_incremental;
psionic_incremental *psionic_incremental_open(const char *directory,char *error,int64_t capacity);
/* New epochs increase strictly; reset clears audio, encoder and decoder
 * histories. Duplicate/old epochs and reordered pushes refuse unchanged. */
int psionic_incremental_reset(psionic_incremental *,uint64_t epoch);
int psionic_incremental_push(psionic_incremental *,uint64_t epoch,uint64_t sequence,const float *,int64_t count,int final);
int psionic_incremental_cancel(psionic_incremental *,uint64_t epoch);
/* Exact UTF-8 snapshot, no NUL required; -1 on stale/cancelled/faulted epoch
 * or insufficient capacity. Status: 0 success, 1 order/extent, 2 audio,
 * 3 backend/capacity. No transcript publication on refused pushes. */
int64_t psionic_incremental_text(psionic_incremental *,uint64_t epoch,uint8_t *,int64_t capacity);
/* Observations only: phase, samples, features, chunks, tokens, next sequence. */
int psionic_incremental_info(psionic_incremental *,int64_t out[6]);
const char *psionic_incremental_error(psionic_incremental *);
void psionic_incremental_close(psionic_incremental *);
/* Verification snapshots: last encoded 2x1024 rows and projected 2x640 rows.
 * Caller owns output. An admitted chunk must exist. */
int psionic_incremental_inspect(psionic_incremental *,float *encoded,float *projected);
int64_t psionic_incremental_tokens(psionic_incremental *,int64_t *tokens,int64_t *positions,int64_t capacity);
int psionic_incremental_cache(psionic_incremental *,int layer,float *keys,float *conv);
int psionic_incremental_features(psionic_incremental *,float *features);
/* Allocated GPU bytes: resident weights, transfer buffers, work, state. */
int psionic_incremental_memory(psionic_incremental *,int64_t out[4]);
#ifdef __cplusplus
}
#endif
#endif
