// Minimal WAV I/O: mono or stereo 16-bit PCM in (stereo is mixed down), mono out.
#ifndef OA_VOICE_WAV_H
#define OA_VOICE_WAV_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Reads a PCM16 WAV into malloc'd mono samples. Returns NULL on failure.
int16_t *wav_read(const char *path, uint32_t *rate, size_t *samples);
bool wav_write(const char *path, const int16_t *pcm, size_t samples, uint32_t rate);
// Parses an in-memory WAV (e.g. Google TTS output); returns a pointer into
// buf at the PCM data and its sample count, or NULL.
const int16_t *wav_parse(const uint8_t *buf, size_t n, uint32_t *rate, size_t *samples);
#endif
