#include <stdio.h>
#include <stdbool.h>
#include "meter.h"

// The host's side: a table of buffers, and Coax gets an index into it.
static const double *buffers[4];

double host_sample(int64_t h, int64_t index) { return buffers[h][index]; }

void host_report(int64_t h, double mean, double peak) {
  printf("buffer %lld: mean %.3f, peak %.3f\n", (long long)h, mean, peak);
}

int main(void) {
  static const double levels[] = {0.25, 0.5, 0.125, 0.875};
  buffers[1] = levels;
  bool ok = meter_scan(1, 4);
  printf("scanned: %s\n", ok ? "yes" : "no");
  return 0;
}
