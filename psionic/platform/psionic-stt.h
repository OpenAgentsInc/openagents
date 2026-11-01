/* Psionic's Whisper as a library: speech to text inside another process.
 * psionic/platform/whisper-build.sh builds libpsionic-whisper.a next to
 * psionic-whisper; link it with -lm -lpthread, and on macOS with
 * -framework Metal -framework Foundation.
 *
 *   char err[256];
 *   psionic_stt *s = psionic_stt_open("ggml-base.en.bin", 8, 1, err, sizeof err);
 *   psionic_stt_text(s, samples, n, "Psionic,Verse", text, sizeof text, err, sizeof err);
 *
 * The model is mapped and decoded once by psionic_stt_open. A process holds
 * at most one open model. psionic_stt_text is safe to call from any thread;
 * calls run one at a time. Design: psionic/docs/transcribe.md. */
#ifndef PSIONIC_STT_H
#define PSIONIC_STT_H
#include <stddef.h>
#include <stdint.h>

typedef struct psionic_stt psionic_stt;

/* Maps the ggml Whisper model at PATH and starts THREADS threads. With GPU
 * set and a Metal device present, the encoder runs on the GPU. NULL with ERR
 * set when the file is missing or is not a Whisper model. */
psionic_stt *psionic_stt_open(const char *path, int threads, int gpu, char *err, size_t errcap);

/* The English transcript of N samples of 16 kHz mono audio in [-1, 1],
 * as one line in OUT ("" for no speech). WORDS is a comma-separated list
 * of names to spell as given, or NULL. Returns 1, or 0 with ERR set. */
int psionic_stt_text(psionic_stt *s, const float *samples, int64_t n, const char *words, char *out, size_t cap,
                     char *err, size_t errcap);

/* Whether the encoder runs on the GPU. */
int psionic_stt_gpu(const psionic_stt *s);

#endif
