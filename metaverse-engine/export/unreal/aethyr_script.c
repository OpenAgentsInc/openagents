/* Firm reason: 6 (compiled into the Unreal bridge, which calls aethyr_host.h from C++; the same
 * file runs natively to make the reference).
 * Runs a script against a world through the host interface and reports, at each beat, a hash of
 * what a host reads. Two builds that print the same lines ran the same game. One line a step:
 *   size W H            the frame's size in pixels (default 1280 800)
 *   key CODE [up]       a key down (or up); CODE is a number or one character
 *   click X Y           a pointer press and release
 *   msg KIND TEXT       a message to the world (a world's test aids)
 *   frames N MS         N frames of MS milliseconds each
 *   expect K V          aethyr_fact(K) must be V
 *   beat NAME           report the hashes now
 * Blank lines and lines starting with # are skipped.
 * Returns 0 when every line ran and every expect held, else the 1-based line that did not. */
#include "aethyr_host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t fnv(const void *p, size_t n, uint64_t h) {
  for (size_t i = 0; i < n; i++) { h ^= ((const unsigned char *)p)[i]; h *= 1099511628211ull; }
  return h;
}
int aethyr_script_run(const char *text, size_t length, void (*line)(const char *)) {
  size_t size = aethyr_state_size();
  void *state = calloc(1, size);
  uint32_t vf = aethyr_vertex_floats(), cap = aethyr_max_vertices(), uf = aethyr_uniform_floats();
  float *v = calloc((size_t)cap * vf, 4), *u = calloc(uf, 4);
  if (!state || !v || !u) { free(state); free(v); free(u); return -1; }
  aethyr_state_init(state);
  float w = 1280, h = 800; uint32_t n = 0; int row = 0, failed = 0; char say[200];
  size_t at = 0;
  while (at < length && !failed) {
    char buf[256]; size_t k = 0;
    while (at < length && text[at] != '\n') { if (k + 1 < sizeof buf && text[at] != '\r') buf[k++] = text[at]; at++; }
    at++; buf[k] = 0; row++;
    char op[32] = {0}, a[160] = {0}, b[32] = {0};
    int fields = sscanf(buf, "%31s %159s %31s", op, a, b);
    if (fields < 1 || op[0] == '#') continue;
    if (!strcmp(op, "size") && fields == 3) { w = (float)atof(a); h = (float)atof(b); }
    else if (!strcmp(op, "key") && fields >= 2) {
      int code = (a[0] == '-' || (a[0] >= '0' && a[0] <= '9' && a[1])) ? atoi(a) : (unsigned char)a[0];
      aethyr_key(state, code, strcmp(b, "up") != 0);
    }
    else if (!strcmp(op, "click") && fields == 3) { aethyr_pointer(state, (float)atof(a), (float)atof(b), 0); aethyr_pointer(state, (float)atof(a), (float)atof(b), 2); }
    else if (!strcmp(op, "msg") && fields >= 2) { const char *body = strstr(buf, a) + strlen(a); while (*body == ' ') body++; aethyr_message(state, atoi(a), body, strlen(body)); }
    else if (!strcmp(op, "frames") && fields == 3) { int count = atoi(a); float dt = (float)atof(b) / 1000.0f; for (int i = 0; i < count; i++) n = aethyr_frame(state, v, u, w, h, 1, dt, 0); }
    else if (!strcmp(op, "expect") && fields == 3) {
      int got = aethyr_fact(state, atoi(a));
      if (got != atoi(b)) { snprintf(say, sizeof say, "AETHYR_SCRIPT FAIL line %d: fact %s is %d, expected %s", row, a, got, b); if (line) line(say); failed = row; }
    }
    else if (!strcmp(op, "beat") && fields >= 2) {
      uint64_t x = 1469598103934665603ull;
      x = fnv(&n, 4, x); x = fnv(v, (size_t)n * vf * 4, x); x = fnv(u, (size_t)uf * 4, x);
      for (uint32_t s = 0; s < aethyr_stores(); s++) x = fnv(aethyr_store(state, (int)s), aethyr_store_bytes(state, (int)s), x);
      snprintf(say, sizeof say, "AETHYR_SCRIPT beat %s mode %d zone %d phase %d hearts %d overlay %u live %u hash %08x%08x", a,
               aethyr_fact(state, 0), aethyr_fact(state, 1), aethyr_fact(state, 2), aethyr_fact(state, 3), n, aethyr_store_bytes(state, 1) / 24, (unsigned)(x >> 32), (unsigned)x);
      if (line) line(say);
    }
    else { snprintf(say, sizeof say, "AETHYR_SCRIPT FAIL line %d: not a step: %s", row, buf); if (line) line(say); failed = row; }
  }
  snprintf(say, sizeof say, "AETHYR_SCRIPT %s", failed ? "FAILED" : "done");
  if (line) line(say);
  free(state); free(v); free(u);
  return failed;
}
#ifdef AETHYR_SCRIPT_MAIN
/* The native reference: aethyr-script RUN.aescript prints the lines. */
static void print_line(const char *s) { puts(s); }
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  FILE *f = fopen(argv[1], "rb"); if (!f) return 2;
  static char text[262144]; size_t n = fread(text, 1, sizeof text, f); fclose(f);
  return aethyr_script_run(text, n, print_line) ? 1 : 0;
}
#endif
