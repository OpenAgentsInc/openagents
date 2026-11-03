/* Measurement effects for the dictation evaluation (GitHub #12665). Effects
   only: this file includes dictate-native.c unchanged, so the evaluation runs
   the CLI's own controller binding and resident-model worker, and adds
   - a microsecond clock;
   - a hook on cxdict_model_clear that reads the finished call's engine
     timing (psionic_parakeet_last_info) and the CUDA encoder/decoder device
     times before the CLI's clear discards them;
   - a hook on cxdict_dct_add that keeps each segment's text, so seams can be
     scored;
   - the model open's own split: the Coax open (hash, layout, vocabulary,
     wrapped with the linker's --wrap=pkrt_open), the NVRTC compile and the
     weight upload (parakeet-cuda.h).
   Every decision about what to run and how to score it is Coax
   (dictate-eval.coax, dictate-score.coax). */
#define cxdict_model_clear cxdict_model_clear_engine
#define cxdict_dct_add cxdict_dct_add_controller
#include "../dictate-native.c"
#undef cxdict_model_clear
#undef cxdict_dct_add
#if defined(PSIONIC_PARAKEET_CUDA)
#include "parakeet-cuda.h"
#endif

/* Coax's []int view, spelled locally ({data, count}, as Coax lays it out). */
typedef struct { int64_t *data; int64_t count; } eval_ints;

int64_t cxeval_now_us(void) {
  struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}

/* Per-call timing, summed over the calls of one dictation:
   [0] calls, [1] engine inference us, [2] encoder us, [3] decoder us,
   [4] the clear after each call us (psionic_parakeet_clear: the wipe of the
   resident work, which the CLI runs before it delivers the text). */
static int64_t acc[5];
int64_t cxdict_model_clear(void) {
  psionic_parakeet_info info;
  if (engine && psionic_parakeet_last_info(engine, &info)) {
    acc[0] += 1; acc[1] += (int64_t)(info.inference_ms * 1000.0);
#if defined(PSIONIC_PARAKEET_CUDA)
    if (info.gpu) { acc[2] += (int64_t)(pk_cuda_seconds() * 1e6); acc[3] += (int64_t)(pk_cuda_decode_seconds() * 1e6); }
#endif
  }
  int64_t t = cxeval_now_us(); int64_t r = cxdict_model_clear_engine(); acc[4] += cxeval_now_us() - t;
  return r;
}

/* Segment texts, separated by 0x1e. */
static unsigned char seg[16384]; static int64_t seg_n;
int64_t cxdict_dct_add(int64_t ticket, coax_v_u8 text) {
  if (seg_n > 0 && seg_n < (int64_t)sizeof seg) seg[seg_n++] = 0x1e;
  for (int64_t i = 0; i < text.count && seg_n < (int64_t)sizeof seg; i++) seg[seg_n++] = text.data[i];
  return cxdict_dct_add_controller(ticket, text);
}

/* Copies the timing and segment texts out (out: 4 ints; text: bytes) and
   resets them for the next dictation. Returns the text's length. */
int64_t cxeval_take(eval_ints out, coax_v_u8 text) {
  for (int i = 0; i < 5 && i < out.count; i++) { out.data[i] = acc[i]; acc[i] = 0; }
  int64_t n = seg_n < text.count ? seg_n : text.count;
  memcpy(text.data, seg, (size_t)n); seg_n = 0;
  return n;
}

/* The Coax open (pkrt_open), timed with the linker's --wrap. Its views are
   passed through untouched; the struct layout matches Coax's {data,count}. */
typedef struct { void *data; int64_t count; } eval_view;
int64_t pkrt_work32(int64_t samples); int64_t pkrt_work64(int64_t samples);
static int64_t open_us;
uint8_t __real_pkrt_open(eval_view, eval_view, eval_view, eval_view, eval_view, eval_view, eval_view);   /* coax_t_PkRtStatus is uint8_t */
uint8_t __wrap_pkrt_open(eval_view a, eval_view b, eval_view c, eval_view d, eval_view e, eval_view f, eval_view g) {
  int64_t t = cxeval_now_us(); uint8_t r = __real_pkrt_open(a, b, c, d, e, f, g); open_us = cxeval_now_us() - t; return r;
}
/* The last open: [0] Coax open us (SHA-256 of both files, layout, vocabulary),
   [1] NVRTC compile us, [2] weight upload us, [3] decoder threads, and
   with room [4] [5] the f32 and f64 work arenas' bytes. */
int64_t cxeval_open_split(eval_ints out) {
  if (out.count < 4) return -1;
  out.data[0] = open_us;
#if defined(PSIONIC_PARAKEET_CUDA)
  out.data[1] = (int64_t)(pk_cuda_compile_seconds() * 1e6); out.data[2] = (int64_t)(pk_cuda_upload_seconds() * 1e6);
#else
  out.data[1] = 0; out.data[2] = 0;
#endif
  out.data[3] = engine ? psionic_parakeet_decoder_threads(engine) : 0;
  if (out.count >= 6) { out.data[4] = pkrt_work32(MAX_SAMPLES) * 4; out.data[5] = pkrt_work64(MAX_SAMPLES) * 8; }
  return 0;
}

/* One direct engine call on the resident model (no controller), for the
   raw no-speech probe: the text's length, or -1. */
int64_t cxeval_direct(coax_v_f32 x, coax_v_u8 out) {
  if (!engine || x.count < 320 || x.count > MAX_SAMPLES) return -1;
  char text[TEXT], err[512];
  if (!psionic_parakeet_text(engine, x.data, x.count, text, sizeof text, err, sizeof err)) return -1;
  int64_t n = (int64_t)strlen(text); if (n > out.count) n = out.count;
  memcpy(out.data, text, (size_t)n); psionic_parakeet_clear(engine);
  return n;
}
