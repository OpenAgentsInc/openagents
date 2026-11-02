/* Resident original-weight Parakeet TDT v3, all model arithmetic in Coax.
 * Build: psionic/platform/parakeet-build.sh OUT
 * Link libpsionic-parakeet.a with -lm -lpthread; on Apple also Metal/Foundation.
 * Exactly one model may be open per process. Calls are serialized. Callers
 * must stop using the handle after close; close waits for an active call.
 * Inputs are disjoint from outputs and live for the whole call. */
#ifndef PSIONIC_PARAKEET_H
#define PSIONIC_PARAKEET_H
#include <stdint.h>
#include <stddef.h>
typedef struct psionic_parakeet psionic_parakeet;
typedef struct { int64_t samples,feature_frames,encoded_frames,tokens,text_bytes; int gpu; double inference_ms; } psionic_parakeet_info;
/* DIR contains exact pinned model.safetensors and tokenizer.json. gpu=1
 * requires Metal; absence/failure is an error, never a silent CPU fallback. */
psionic_parakeet *psionic_parakeet_open(const char *dir,int gpu,char *err,size_t errcap);
/* 16kHz mono f32 in [-1,1], 320..327520 samples (20ms minimum). OUT includes a NUL byte.
 * Success1/failure0. On failure OUT and the last successful snapshot stay
 * unchanged. Model/vocabulary are loaded once and reused across calls. */
int psionic_parakeet_text(psionic_parakeet *s,const float *samples,int64_t n,char *out,size_t cap,char *err,size_t errcap);
/* Decode recorded [frames,1024] encoder f32 tensors using this model's exact
 * TDT decoder. This is an offline/reference API, not a streaming contract. */
int psionic_parakeet_decode(psionic_parakeet *s,const float *encoded,int64_t frames,char *out,size_t cap,char *err,size_t errcap);
int psionic_parakeet_last_info(psionic_parakeet *s,psionic_parakeet_info *info);
/* Actual persistent decoder threads, including the calling thread:1..8;
 * a partial pthread startup keeps the successfully created workers.0 for
 * an invalid handle. Per-output arithmetic is independent of this count. */
int psionic_parakeet_decoder_threads(psionic_parakeet *s);
/* Explicitly discard resident utterance work and inspection results. Keeps
 * validated immutable weights/vocabulary and the warm decoder/GPU resources.
 * Caller-owned input/output bytes are untouched. Returns1 for the active
 * handle,0 for an invalid handle; serialized after any active recognition. */
int psionic_parakeet_clear(psionic_parakeet *s);
/* Copy last successful token IDs and encoded-frame indices, or -1 for
 * insufficient capacity/no result. No pointers into the engine escape. */
int64_t psionic_parakeet_tokens(psionic_parakeet *s,int64_t *tokens,int64_t *frames,size_t cap);
/* Last successful transcription's JSON metadata/tokens. OUT atomic on error. */
int psionic_parakeet_json(psionic_parakeet *s,char *out,size_t cap);
/* Copy last successful full-transcription features(stage0) or encoded
 * frames(stage1), as row-major f32. Independent snapshots survive failure;
 * unavailable after a successful decode-only call. Returns count or -1. */
int64_t psionic_parakeet_stage(psionic_parakeet *s,int stage,float *out,size_t cap);
void psionic_parakeet_close(psionic_parakeet *s);
#endif
