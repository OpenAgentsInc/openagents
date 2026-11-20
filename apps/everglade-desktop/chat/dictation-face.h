#ifndef OPENAGENTS_DICTATION_FACE_H
#define OPENAGENTS_DICTATION_FACE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Serialized controller access. Audio stays in this opaque in-memory owner.
 * Times are monotonic uptime milliseconds; audio timestamp names first sample.
 * Phases:0 disabled/inactive,1 idle,2 held,3 release tail,4 clip ready,5 decoding.
 * Outcomes:0 none,1 tap,2 no speech,3 cancelled,4 error,5 success,6 duration cap. */
void *dct_new(void);
void dct_free(void *state);
void dct_enable(void *state,int enabled,int64_t now_ms);
void dct_active(void *state,int active);
void dct_ready(void *state,int ready);
int dct_enabled(void *state);
int dct_model_ready(void *state);
int dct_is_active(void *state);
void dct_preparing(void *state,int reason);
int dct_preparing_reason(void *state);
int dct_capture_allowed(void *state);
uint64_t dct_begin(void *state,int64_t now_ms);
void dct_release(void *state,int64_t now_ms);
void dct_cancel(void *state);
/* Reasons:1 permission,2 model,3 capture/clock,4 recognition,5 audio format. */
void dct_fault(void *state,int reason);
void dct_audio(void *state,const float *mono,int32_t count,int32_t rate,int64_t start_ms);
void dct_tick(void *state,int64_t now_ms);
int dct_phase(void *state);
int dct_outcome(void *state);
int dct_fault_reason(void *state);
uint64_t dct_ticket(void *state);
int32_t dct_samples(void *state);
/* Complete disjoint model-sized segments, or0 for an invalid/end offset. */
int32_t dct_segment(void *state,int32_t offset);
/* Model-only silence; captured audio coverage excludes these samples.
 * Segment plus both counts fits the model's per-call bound. Invalid offsets
 * and inactive/cancelled clips return zero. */
int32_t dct_segment_leading_silence(void *state,int32_t offset);
int32_t dct_segment_trailing_silence(void *state,int32_t offset);
int32_t dct_copy_audio(void *state,int32_t offset,float *out,int32_t capacity);
int dct_decode_begin(void *state,uint64_t ticket);
/* True only for this current, uncancelled decoding ticket and successful result.
 * Clears captured clip; never sends, types, writes a file or uses a network. */
int dct_result(void *state,uint64_t ticket,int success);
int dct_append_text(void *state,uint64_t ticket,const uint8_t *text,int32_t length);
int32_t dct_text_count(void *state);
int32_t dct_copy_text(void *state,uint8_t *out,int32_t capacity);
int dct_device_matches(const uint8_t *wanted,int32_t wanted_n,const uint8_t *observed,int32_t observed_n,int is_default);
int dct_device_format(double rate,int64_t channels,int64_t stride,int host_time);
#ifdef __cplusplus
}
#endif
#endif
