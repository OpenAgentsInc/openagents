// The shared hosts' PNG writer (ui-host.h ui_shot_write): RGBA rows to a file, with no compression
// library. The Linux host (../linux/ui-host.c) carries the same code inline; this copy serves
// the hosts that have no platform image writer (Android).
#ifndef UI_PNG_H
#define UI_PNG_H
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// A PNG of RGBA rows. Each row is filtered against the one above (a flat interface is then mostly
// zeros) and deflated with the fixed Huffman codes and runs of a repeated byte, which is enough to
// make a picture of an interface a few tens of kilobytes without a compression library.
typedef struct { uint8_t *p; size_t n; uint32_t bits; int count; } UiBits;
static void ui_bits(UiBits *b, uint32_t v, int n) {
  b->bits |= v << b->count; b->count += n;
  while (b->count >= 8) { b->p[b->n++] = (uint8_t)b->bits; b->bits >>= 8; b->count -= 8; }
}
static void ui_code(UiBits *b, uint32_t code, int n) {   // a Huffman code goes most significant bit first
  uint32_t r = 0;
  for (int i = 0; i < n; i++) r |= ((code >> i) & 1) << (n - 1 - i);
  ui_bits(b, r, n);
}
static void ui_symbol(UiBits *b, int c) {
  if (c < 144) ui_code(b, 0x30u + (uint32_t)c, 8);
  else if (c < 256) ui_code(b, 0x190u + (uint32_t)(c - 144), 9);
  else if (c < 280) ui_code(b, (uint32_t)(c - 256), 7);
  else ui_code(b, 0xC0u + (uint32_t)(c - 280), 8);
}
static void ui_run(UiBits *b, int len) {   // 3 to 258 more of the byte before
  static const int base[29] = {3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258};
  static const int extra[29] = {0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0};
  int k = 28;
  while (base[k] > len) k--;
  ui_symbol(b, 257 + k);
  if (extra[k]) ui_bits(b, (uint32_t)(len - base[k]), extra[k]);
  ui_code(b, 0, 5);   // distance 1
}
static uint32_t crc_table[256];
static uint32_t ui_crc(uint32_t c, const uint8_t *p, size_t n) {
  if (!crc_table[1]) for (uint32_t i = 0; i < 256; i++) { uint32_t k = i; for (int j = 0; j < 8; j++) k = k & 1 ? 0xedb88320u ^ (k >> 1) : k >> 1; crc_table[i] = k; }
  c = ~c;
  for (size_t i = 0; i < n; i++) c = crc_table[(c ^ p[i]) & 255] ^ (c >> 8);
  return ~c;
}
static void ui_chunk(FILE *f, const char *type, const uint8_t *data, size_t n) {
  uint8_t len[4] = {(uint8_t)(n >> 24), (uint8_t)(n >> 16), (uint8_t)(n >> 8), (uint8_t)n};
  fwrite(len, 1, 4, f); fwrite(type, 1, 4, f);
  if (n) fwrite(data, 1, n, f);
  uint32_t c = ui_crc(0, (const uint8_t *)type, 4);
  if (n) c = ui_crc(c, data, n);
  uint8_t crc[4] = {(uint8_t)(c >> 24), (uint8_t)(c >> 16), (uint8_t)(c >> 8), (uint8_t)c};
  fwrite(crc, 1, 4, f);
}
static int ui_png_write(const char *png, const uint8_t *px, size_t w, size_t rows) {
  size_t raw = rows * (w * 4 + 1);
  uint8_t *scan = malloc(raw), *z = malloc(raw + raw / 8 + 64);
  if (!scan || !z) { free(scan); free(z); return 0; }
  for (size_t y = 0; y < rows; y++) {
    uint8_t *row = scan + y * (w * 4 + 1);
    row[0] = 2;   // the filter: each byte less the one above it
    for (size_t x = 0; x < w * 4; x++) row[1 + x] = (uint8_t)(px[y * w * 4 + x] - (y ? px[(y - 1) * w * 4 + x] : 0));
  }
  uint32_t a = 1, b = 0;
  for (size_t i = 0; i < raw; i++) { a = (a + scan[i]) % 65521; b = (b + a) % 65521; }
  UiBits bits = {z, 0, 0, 0};
  z[bits.n++] = 0x78; z[bits.n++] = 0x01;
  ui_bits(&bits, 1, 1); ui_bits(&bits, 1, 2);   // the last block, fixed codes
  for (size_t i = 0; i < raw;) {
    size_t run = 0;
    if (i > 0) while (i + run < raw && run < 258 && scan[i + run] == scan[i - 1]) run++;
    if (run >= 3) { ui_run(&bits, (int)run); i += run; }
    else { ui_symbol(&bits, scan[i]); i++; }
  }
  ui_symbol(&bits, 256);
  if (bits.count) ui_bits(&bits, 0, 8 - bits.count);
  size_t n = bits.n;
  z[n++] = (uint8_t)(b >> 8); z[n++] = (uint8_t)b; z[n++] = (uint8_t)(a >> 8); z[n++] = (uint8_t)a;
  FILE *f = fopen(png, "wb");
  int ok = 0;
  if (f) {
    static const uint8_t sig[8] = {137, 80, 78, 71, 13, 10, 26, 10};
    uint8_t head[13] = {(uint8_t)(w >> 24), (uint8_t)(w >> 16), (uint8_t)(w >> 8), (uint8_t)w, (uint8_t)(rows >> 24), (uint8_t)(rows >> 16), (uint8_t)(rows >> 8), (uint8_t)rows, 8, 6, 0, 0, 0};
    fwrite(sig, 1, 8, f); ui_chunk(f, "IHDR", head, 13); ui_chunk(f, "IDAT", z, n); ui_chunk(f, "IEND", NULL, 0);
    ok = fclose(f) == 0;
  }
  free(scan); free(z);
  return ok;
}
#endif
