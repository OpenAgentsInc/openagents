// light-grid: rows of f32 arguments for light-check.sh, on standard output.
//   light-grid PARAMETERS ROWS
// The same rows every run: a fixed generator over [-2, 2], with every
// eighth argument one of a few exact values (0, 1, 0.5, -1).
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
  if (argc != 3) { fprintf(stderr, "usage: light-grid PARAMETERS ROWS\n"); return 2; }
  long params = atol(argv[1]), rows = atol(argv[2]);
  static const float exact[] = {0, 1, 0.5f, -1};
  uint64_t x = 0x9E3779B97F4A7C15ull;
  for (long i = 0; i < params * rows; i++) {
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    float v = (x & 7) == 0 ? exact[(x >> 3) & 3] : (float)((double)(x >> 40) / (double)(1ull << 24) * 4.0 - 2.0);
    fwrite(&v, 4, 1, stdout);
  }
  return 0;
}
