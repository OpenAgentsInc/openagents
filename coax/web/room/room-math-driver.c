/* The room server's math under test (room-math-check.coax, #12893): sinf, cosf, tanf, expf, logf,
   powf and their double forms over 20000 inputs and the edge cases, each line the name, the inputs
   as their doubles' bits and the result's bits. Built with room-math.c natively and for wasm32. */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint32_t seed = 12893;
static double next(void) { seed = seed * 1664525u + 1013904223u; return (double)(seed >> 8) / 16777216.0; }
static unsigned long long bits(double v) { uint64_t b; memcpy(&b, &v, 8); return (unsigned long long)b; }
static void f(const char *name, float x, float y, float v) { uint32_t b; memcpy(&b, &v, 4); printf("%s %016llx %016llx %08x\n", name, bits(x), bits(y), b); }
static void d(const char *name, double x, double y, double v) { printf("%s %016llx %016llx %016llx\n", name, bits(x), bits(y), bits(v)); }
static void all(double x, double y) {
  float xf = (float)x, yf = (float)y;
  f("sinf", xf, 0, sinf(xf)); f("cosf", xf, 0, cosf(xf)); f("tanf", xf, 0, tanf(xf));
  f("expf", xf, 0, expf(xf)); f("logf", yf, 0, logf(yf)); f("powf", yf, xf, powf(yf, xf));
  d("sin", x, 0, sin(x)); d("cos", x, 0, cos(x)); d("tan", x, 0, tan(x));
  d("exp", x, 0, exp(x)); d("log", y, 0, log(y)); d("pow", y, x, pow(y, x));
}
int main(void) {
  const double edge[] = {0, -0.0, 1, -1, 0.5, 1.5707963267948966, 3.141592653589793, 1e-30, 88, -103, 700, -740, 1e5, -1e5, 1.7e9, -2.5e9};
  for (int i = 0; i < (int)(sizeof edge / sizeof edge[0]); i++) all(edge[i], edge[i] < 0 ? -edge[i] : edge[i]);
  for (int i = 0; i < 20000; i++) { double s = next(); all((next() - 0.5) * (s < 0.5 ? 20 : s < 0.9 ? 400 : 4e9), next() * (s < 0.5 ? 3 : 1000)); }
  d("pow", -2, 3, pow(-2, 3)); d("pow", -2, 0.5, pow(-2, 0.5)); d("pow", 0, -1, pow(0, -1)); d("pow", -0.0, 3, pow(-0.0, 3));
  d("pow", INFINITY, -2, pow(INFINITY, -2)); d("pow", 0.5, INFINITY, pow(0.5, INFINITY)); d("log", 0, 0, log(0)); d("log", 5e-324, 0, log(5e-324));
  d("exp", -1000, 0, exp(-1000)); d("exp", 1000, 0, exp(1000)); d("sin", INFINITY, 0, sin(INFINITY));
  return 0;
}
