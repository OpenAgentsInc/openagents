#include "wav.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t le32(const uint8_t *p) { return (uint32_t)p[0] | (uint32_t)p[1] << 8 | (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24; }
static uint16_t le16(const uint8_t *p) { return (uint16_t)(p[0] | p[1] << 8); }

// Walks RIFF chunks; returns the data chunk and the fmt fields.
static const uint8_t *riff(const uint8_t *buf, size_t n, uint32_t *rate, uint16_t *channels, size_t *bytes) {
  if (n < 12 || memcmp(buf, "RIFF", 4) || memcmp(buf + 8, "WAVE", 4)) return NULL;
  size_t p = 12;
  bool fmt = false;
  while (p + 8 <= n) {
    uint32_t len = le32(buf + p + 4);
    const uint8_t *c = buf + p + 8;
    if (!memcmp(buf + p, "fmt ", 4) && len >= 16 && p + 8 + 16 <= n) {
      if (le16(c) != 1 || le16(c + 14) != 16) return NULL;  // PCM, 16-bit only
      *channels = le16(c + 2);
      *rate = le32(c + 4);
      fmt = true;
    } else if (!memcmp(buf + p, "data", 4)) {
      if (!fmt) return NULL;
      size_t avail = n - (p + 8);
      // Streaming writers leave 0 or 0xffffffff here; take what's present.
      *bytes = len > avail || len == 0 ? avail : len;
      return c;
    }
    p += 8 + len + (len & 1);
  }
  return NULL;
}

const int16_t *wav_parse(const uint8_t *buf, size_t n, uint32_t *rate, size_t *samples) {
  uint16_t ch = 0;
  size_t bytes = 0;
  const uint8_t *d = riff(buf, n, rate, &ch, &bytes);
  if (!d || ch != 1 || ((uintptr_t)d & 1)) return NULL;
  *samples = bytes / 2;
  return (const int16_t *)d;
}

int16_t *wav_read(const char *path, uint32_t *rate, size_t *samples) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  fseek(f, 0, SEEK_END);
  long sz = ftell(f);
  fseek(f, 0, SEEK_SET);
  if (sz <= 0 || sz > 64L * 1024 * 1024) { fclose(f); return NULL; }
  uint8_t *buf = malloc((size_t)sz);
  if (!buf || fread(buf, 1, (size_t)sz, f) != (size_t)sz) { fclose(f); free(buf); return NULL; }
  fclose(f);
  uint16_t ch = 0;
  size_t bytes = 0;
  const uint8_t *d = riff(buf, (size_t)sz, rate, &ch, &bytes);
  int16_t *out = NULL;
  if (d && (ch == 1 || ch == 2)) {
    size_t frames = bytes / (2u * ch);
    out = malloc(frames * 2 + 2);
    if (out) {
      for (size_t i = 0; i < frames; i++) {
        int32_t s = (int16_t)le16(d + 2 * ch * i);
        if (ch == 2) s = (s + (int16_t)le16(d + 2 * ch * i + 2)) / 2;
        out[i] = (int16_t)s;
      }
      *samples = frames;
    }
  }
  free(buf);
  return out;
}

static void put32(uint8_t *p, uint32_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24); }
static void put16(uint8_t *p, uint16_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }

bool wav_write(const char *path, const int16_t *pcm, size_t samples, uint32_t rate) {
  uint8_t h[44];
  memcpy(h, "RIFF", 4); put32(h + 4, (uint32_t)(36 + samples * 2)); memcpy(h + 8, "WAVEfmt ", 8);
  put32(h + 16, 16); put16(h + 20, 1); put16(h + 22, 1); put32(h + 24, rate); put32(h + 28, rate * 2);
  put16(h + 32, 2); put16(h + 34, 16); memcpy(h + 36, "data", 4); put32(h + 40, (uint32_t)(samples * 2));
  FILE *f = fopen(path, "wb");
  if (!f) return false;
  bool ok = fwrite(h, 1, 44, f) == 44;
  for (size_t i = 0; ok && i < samples; i++) {
    uint8_t s[2];
    put16(s, (uint16_t)pcm[i]);
    ok = fwrite(s, 1, 2, f) == 2;
  }
  return fclose(f) == 0 && ok;
}
