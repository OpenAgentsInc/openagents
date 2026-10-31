// realm-run: a realm natively with no GPU, printing what each frame hands
// the GPU, so a native build can be held to the browser's wasm
// (verse/native/wasm-run.ts prints the same lines from the wasm).
//   realm-run --pack FILE [--facts FILE] [--frames N] [--size WxH] [--scale K]
//             [--keys F:KEY:DOWN,...] [--extra BYTES,...] [--golden] [--every K] [--against DIR]
// Each frame steps 1/60 s. --keys holds or lets go of a key (host.js KEYS:
// -6 up, -7 down, -4 left, -5 right, or a code point) at frame F. --extra
// are the sizes of the page's assets after the pack's four. Every K frames
// (and the last) it prints the frame, the vertex count, the draw count and
// FNV-1a hashes of the vertices, uniforms, draw list and each store.
// --against DIR compares the last frame's uniforms and stores with the
// wasm's (wasm-run.ts --dump DIR) and fails past libm's rounding.
#include "face.h"
#include "realm-host.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t fnv(uint64_t h, const void *p, size_t n) {
  const uint8_t *b = p;
  for (size_t i = 0; i < n; i++) h = (h ^ b[i]) * 0x100000001b3ull;
  return h;
}
#define FNV0 0xcbf29ce484222325ull

static char *slurp(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "realm-run: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);
  char *p = malloc((size_t)size + 1);
  if (fread(p, 1, (size_t)size, f) != (size_t)size) { fprintf(stderr, "realm-run: cannot read %s\n", path); exit(1); }
  fclose(f);
  p[size] = 0;
  *n = (size_t)size;
  return p;
}

int main(int argc, char **argv) {
  const char *pack = NULL, *facts = NULL, *keys = "", *extra = "", *against = NULL;
  int frames = 120, every = 30, golden = 0;
  float w = 1280, ht = 720, scale = 1;
  for (int i = 1; i < argc; i++) {
    const char *a = argv[i], *v = i + 1 < argc ? argv[i + 1] : "";
    if (!strcmp(a, "--pack")) pack = v, i++;
    else if (!strcmp(a, "--facts")) facts = v, i++;
    else if (!strcmp(a, "--frames")) frames = atoi(v), i++;
    else if (!strcmp(a, "--every")) every = atoi(v), i++;
    else if (!strcmp(a, "--size")) { sscanf(v, "%fx%f", &w, &ht); i++; }
    else if (!strcmp(a, "--scale")) scale = (float)atof(v), i++;
    else if (!strcmp(a, "--keys")) keys = v, i++;
    else if (!strcmp(a, "--extra")) extra = v, i++;
    else if (!strcmp(a, "--golden")) golden = 1;
    else if (!strcmp(a, "--against")) against = v, i++;
    else { fprintf(stderr, "realm-run: what is %s?\n", a); return 2; }
  }
  if (!pack) { fprintf(stderr, "usage: realm-run --pack FILE [--facts FILE] [--frames N] [--size WxH] [--keys F:KEY:DOWN,...]\n"); return 2; }
  RealmHost h;
  rh_open(&h, golden);
  if (rh_pack(&h, pack)) return 1;
  if (facts) {
    size_t n;
    char *text = slurp(facts, &n);
    while (n && text[n - 1] == '\n') n--;
    rh_send(&h, RH_PAGE_KIND, text, n);
    free(text);
  }
  rh_send(&h, RH_VIEWER_KIND, "", 0);
  uint32_t sizes[8];
  int nextra = 0;
  for (const char *p = extra; *p && nextra < 8;) {
    sizes[nextra++] = (uint32_t)strtoul(p, (char **)&p, 10);
    if (*p == ',') p++;
  }
  rh_deliver(&h, nextra, sizes);
  printf("realm-run: %d entries in the pack (%d layers), %d floats a vertex, %d uniforms\n", h.entry_count, h.layers, h.floats, h.uniform_floats);
  for (int f = 1; f <= frames; f++) {
    for (const char *p = keys; *p;) {
      int at = (int)strtol(p, (char **)&p, 10);
      int key = (int)strtol(p + 1, (char **)&p, 10);
      int down = (int)strtol(p + 1, (char **)&p, 10);
      if (at == f) {
        if (down) coaxial_key(h.state, key, 0);
        else coaxial_key_up(h.state, key, 0);
        rh_after(&h, 1);
      }
      if (*p == ',') p++;
    }
    uint32_t n = rh_frame(&h, w, ht, scale, 1.0f / 60);
    rh_after(&h, 1);
    if (f % every && f != frames) continue;
    uint32_t draws = coaxial_draw_count(h.state);
    printf("frame %d: %u vertices %u draws (%u water) %u blades %u sparks", f, n, draws, coaxial_water_draws(h.state),
           coaxial_grass_instances(h.state), coaxial_particle_instances(h.state));
    printf(" v %016llx", (unsigned long long)fnv(FNV0, rh_ptr(h.vertices), (size_t)n * h.floats * 4));
    printf(" u %016llx", (unsigned long long)fnv(FNV0, rh_ptr(h.uniforms), (size_t)h.uniform_floats * 4));
    printf(" d %016llx", (unsigned long long)(draws ? fnv(FNV0, rh_ptr(coaxial_draws(h.state)), (size_t)draws * 16) : FNV0));
    for (int k = 0; k < (int)coaxial_stores(); k++) {
      uint32_t bytes = coaxial_store_bytes(h.state, k) & ~3u;
      printf(" s%d %u %016llx", k, bytes, (unsigned long long)fnv(FNV0, rh_ptr(coaxial_store(h.state, k)), bytes));
    }
    uint32_t pb = coaxial_particle_bytes(h.state) & ~3u;
    printf(" p %016llx\n", (unsigned long long)fnv(FNV0, rh_ptr(coaxial_particles(h.state)), pb));
  }
  // The last frame's uniforms and stores against the wasm's (wasm-run.ts --dump DIR): the same
  // words, or within libm's rounding (glibc's sinf and wasi-libc's differ in the last bits).
  int far = 0;
  if (against) {
    const char *names[5] = {"u", "s0", "s1", "s2", "s3"};
    for (int k = 0; k < 5 && k <= (int)coaxial_stores(); k++) {
      uint32_t bytes = k ? coaxial_store_bytes(h.state, k - 1) & ~3u : (uint32_t)h.uniform_floats * 4;
      const float *ours = rh_ptr(k ? coaxial_store(h.state, k - 1) : h.uniforms);
      char path[512];
      snprintf(path, sizeof path, "%s/%s.bin", against, names[k]);
      size_t n;
      float *theirs = (float *)slurp(path, &n);
      if (n != bytes) { printf("realm-run: %s is %u bytes here and %zu in the wasm\n", names[k], bytes, n); far++; free(theirs); continue; }
      int differ = 0, beyond = 0;
      double largest = 0;
      for (size_t i = 0; i < n / 4; i++) {
        if (!memcmp(&ours[i], &theirs[i], 4)) continue;
        differ++;
        double d = fabs((double)ours[i] - theirs[i]), m = fmax(fabs((double)ours[i]), fabs((double)theirs[i]));
        if (d > largest) largest = d;
        if (!(d <= 1e-6 || d <= m * 1e-6)) beyond++;
      }
      printf("realm-run: %s: %zu words, %d differ (largest %.3g), %d beyond rounding\n", names[k], n / 4, differ, largest, beyond);
      far += beyond;
      free(theirs);
    }
  }
  rh_close(&h);
  return far ? 1 : 0;
}
