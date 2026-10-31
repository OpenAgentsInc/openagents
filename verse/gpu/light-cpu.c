// light-cpu: the C build of a Coax procedure over the rows vk-kernel ran,
// compared with what the GPU wrote (docs/verse/native-gpu.md, phase G1).
//   light-cpu NAME IN.f32 GPU.f32
// NAME is an #export_c procedure of the file light-table.h was made from
// (light-check.sh writes it). Prints the rows, the largest error and the
// rows past the tolerance; exits 1 when any row is past it.
#include "light.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef float (*Fn)(const float *a);
typedef struct { const char *name; int params; Fn fn; } Entry;
#include "light-table.h"

static float *slurp(const char *path, size_t *count) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "light-cpu: cannot read %s\n", path); exit(2); }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  float *p = malloc(n > 0 ? (size_t)n : 1);
  if (!p || fread(p, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "light-cpu: cannot read %s\n", path); exit(2); }
  fclose(f);
  *count = (size_t)n / 4;
  return p;
}

// 2^-16 absolute (a 256th of one step of an 8-bit display value, where this
// light ends), or 2^-12 relative for large values. The GPU's exp, log and
// pow are a few units in the last place from the C library's; differences
// of terms near zero cancel down to that, and exp of a large argument
// carries the argument's own rounding (about x * 2^-24) into its result.
static double tolerance(double want) { return fmax(fabs(want) * 0x1p-12, 0x1p-16); }

int main(int argc, char **argv) {
  if (argc != 4) { fprintf(stderr, "usage: light-cpu NAME IN.f32 GPU.f32\n"); return 2; }
  const Entry *e = NULL;
  for (size_t i = 0; i < sizeof LIGHT / sizeof LIGHT[0]; i++)
    if (!strcmp(LIGHT[i].name, argv[1])) e = &LIGHT[i];
  if (!e) { fprintf(stderr, "light-cpu: no procedure %s\n", argv[1]); return 2; }
  size_t nin, ngpu;
  float *in = slurp(argv[2], &nin), *gpu = slurp(argv[3], &ngpu);
  size_t rows = nin / (size_t)e->params;
  if (rows != ngpu) { fprintf(stderr, "light-cpu: %zu rows in, %zu out\n", rows, ngpu); return 2; }
  size_t past = 0, nan = 0;
  double worst = 0, worst_rel = 0;
  for (size_t r = 0; r < rows; r++) {
    double want = e->fn(in + r * (size_t)e->params), got = gpu[r];
    if (isnan(want) || isnan(got) || isinf(want) || isinf(got)) {
      if (isnan(want) && isnan(got)) { nan++; continue; }
      if (want == got) continue;
      if (past < 3) fprintf(stderr, "light-cpu: %s row %zu: C %.9g, GPU %.9g\n", e->name, r, want, got);
      past++;
      continue;
    }
    double d = fabs(want - got);
    if (d > worst) worst = d;
    if (d / fmax(fabs(want), 1e-30) > worst_rel && fabs(want) > 1e-6) worst_rel = d / fabs(want);
    if (d > tolerance(want)) {
      if (past < 3) {
        fprintf(stderr, "light-cpu: %s row %zu (", e->name, r);
        for (int k = 0; k < e->params; k++) fprintf(stderr, "%s%.9g", k ? ", " : "", in[r * (size_t)e->params + k]);
        fprintf(stderr, "): C %.9g, GPU %.9g\n", want, got);
      }
      past++;
    }
  }
  printf("%-22s %zu rows, largest error %.3g (relative %.3g), %zu NaN on both, %zu past the tolerance\n",
         e->name, rows, worst, worst_rel, nan, past);
  return past ? 1 : 0;
}
