/* Firm reason: 6 (stands in for the Unreal host, which calls aethyr_host.h from C++; the Unreal
 * probe module compiles this same file).
 * Runs a fixed script against a world and reports what a host would read, so builds of the same
 * world (32-bit Wasm, native 64-bit, 64-bit Wasm, inside Unreal) can be compared: one line per
 * chunk with its size and FNV-1a hash, and the raw chunks to a file when a path is given.
 * Script: 34 frames at 1280x800, dt 1/60; before frame 2 leave the title (e, space, a click);
 * from frame 4 hold w. Chunks are kept for frames 0, 1 and 33.
 *   frame-dump [OUT.bin] */
#include "aethyr_host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static FILE *out;
static void (*say)(const char *);
static void put(const char *name, int frame, const void *p, size_t n) {
  uint64_t h = 1469598103934665603ull;
  for (size_t i = 0; i < n; i++) { h ^= ((const unsigned char *)p)[i]; h *= 1099511628211ull; }
  char text[160];
  snprintf(text, sizeof text, "AETHYR_PROBE f%d %s %u %08x%08x", frame, name, (unsigned)n, (unsigned)(h >> 32), (unsigned)h);
  if (say) say(text);
  if (out) { uint32_t k = (uint32_t)n; fwrite(&k, 4, 1, out); fwrite(p, 1, n, out); }
}
int aethyr_probe_run(const char *path, void (*line)(const char *)) {
  say = line; out = 0;
  if (path) { out = fopen(path, "wb"); if (!out) return 2; }
  size_t size = aethyr_state_size();
  void *state = calloc(1, size); aethyr_state_init(state);
  uint32_t vf = aethyr_vertex_floats(), cap = aethyr_max_vertices(), uf = aethyr_uniform_floats();
  float *v = calloc((size_t)cap * vf, 4), *u = calloc(uf, 4);
  if (!state || !v || !u) return 3;
  uint32_t head[5] = {vf, cap, uf, aethyr_stores(), (uint32_t)sizeof(void *)}; put("head", -1, head, 4 * 4);
  char text[96]; snprintf(text, sizeof text, "AETHYR_PROBE pointer_bytes %u state_bytes %u", head[4], (unsigned)size); if (say) say(text);
  for (int f = 0; f < 34; f++) {
    if (f == 2) { aethyr_key(state, 'e', 1); aethyr_key(state, 'e', 0); aethyr_key(state, ' ', 1); aethyr_key(state, ' ', 0); aethyr_pointer(state, 640, 400, 0); aethyr_pointer(state, 640, 400, 2); }
    if (f == 4) aethyr_key(state, 'w', 1);
    uint32_t n = aethyr_frame(state, v, u, 1280, 800, 1, 1.0f / 60.0f, 0);
    if (f == 0 || f == 1 || f == 33) {
      uint32_t meta[3] = {(uint32_t)f, n, aethyr_draw_count(state)}; put("meta", f, meta, sizeof meta);
      put("overlay", f, v, (size_t)n * vf * 4); put("uniforms", f, u, (size_t)uf * 4);
      for (uint32_t k = 0; k < aethyr_stores(); k++) put(k == 0 ? "store0" : "store1", f, aethyr_store(state, (int)k), aethyr_store_bytes(state, (int)k));
    }
  }
  if (out) fclose(out);
  free(state); free(v); free(u);
  return 0;
}
#ifndef AETHYR_PROBE_NO_MAIN
static void print_line(const char *s) { puts(s); }
int main(int argc, char **argv) { return aethyr_probe_run(argc == 2 ? argv[1] : 0, print_line); }
#endif
