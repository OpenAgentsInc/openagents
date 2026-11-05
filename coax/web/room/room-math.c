/* The room server's sines, exponentials and powers (GitHub #12893), linked into both of its hosts in
   place of the C library's: oa-room (build.sh, the Dockerfile) and the browser's local host
   (worker.sh). The two C libraries round some inputs differently (macOS's libm and wasi-libc differ
   on about 1 tanf in 8, 1 expf or powf in 150), and the everglade tick (room-tick.c) calls them in
   its steps: the town's lifts, falls and turns came out a little apart in the two hosts, and a fall
   on the edge of a 10-foot band, or a guard's sight on the edge of its cone, then parted the two
   towns. These use only +, -, *, /, floor, fmod and the bits of a double, which IEEE 754 defines
   exactly, as aethyr/math.coax does (INVARIANTS.md "A simulation step uses strict math"), so the
   two hosts compute the same bits. The hosts build with -ffp-contract=off and -fno-builtin-NAME for
   each of these, so no a * b + c is fused and no call is folded or merged (sin and cos into
   __sincos_stret) behind them. Each float function works in double and rounds once.
   Checks: coax/web/room/room-math-check.coax (the same bits natively and in wasm, near the C library). */
#include <math.h>
#include <stdint.h>
#include <string.h>

#define RM_PIO2_HI 1.5707963267341256      /* pi/2's leading 33 bits: k * RM_PIO2_HI is exact */
#define RM_PIO2_LO 6.077100506506192e-11   /* pi/2 - RM_PIO2_HI */
#define RM_TWO_OVER_PI 0.6366197723675814
#define RM_TWO_PI 6.283185307179586
#define RM_LN2_HI 0.6931471803691238       /* ln 2's leading 32 bits */
#define RM_LN2_LO 1.9082149292705877e-10   /* ln 2 - RM_LN2_HI */
#define RM_FAR 100000.0                    /* past this an angle is first taken modulo 2 pi (fmod is exact) */

static uint64_t rm_bits(double x) { uint64_t b; memcpy(&b, &x, sizeof b); return b; }
static double rm_double(uint64_t b) { double x; memcpy(&x, &b, sizeof x); return x; }
static int rm_nan(double x) { return x != x; }
static int rm_inf(double x) { return x == INFINITY || x == -INFINITY; }

/* sin and cos of r in [-pi/4, pi/4], by their series to r^13 and r^14 (aethyr/math.coax) */
static double rm_sin_near(double r) {
  double z = r * r;
  return r + r * z * (-1.0 / 6 + z * (1.0 / 120 + z * (-1.0 / 5040 + z * (1.0 / 362880 + z * (-1.0 / 39916800 + z * (1.0 / 6227020800.0))))));
}
static double rm_cos_near(double r) {
  double z = r * r;
  return 1 + z * (-1.0 / 2 + z * (1.0 / 24 + z * (-1.0 / 720 + z * (1.0 / 40320 + z * (-1.0 / 3628800 + z * (1.0 / 479001600 + z * (-1.0 / 87178291200.0)))))));
}
/* x as k quarter turns and r past them: x = k pi/2 + r, |r| <= pi/4 (and a little) */
static double rm_turn(double x, int *q) {
  if (x > RM_FAR || x < -RM_FAR) x = fmod(x, RM_TWO_PI);
  double k = floor(x * RM_TWO_OVER_PI + 0.5);
  *q = (int)((int64_t)k & 3);
  return (x - k * RM_PIO2_HI) - k * RM_PIO2_LO;
}
double sin(double x) {
  if (rm_nan(x) || rm_inf(x)) return NAN;
  int q; double r = rm_turn(x, &q);
  return q == 0 ? rm_sin_near(r) : q == 1 ? rm_cos_near(r) : q == 2 ? -rm_sin_near(r) : -rm_cos_near(r);
}
double cos(double x) {
  if (rm_nan(x) || rm_inf(x)) return NAN;
  int q; double r = rm_turn(x, &q);
  return q == 0 ? rm_cos_near(r) : q == 1 ? -rm_sin_near(r) : q == 2 ? -rm_cos_near(r) : rm_sin_near(r);
}
double tan(double x) {
  if (rm_nan(x) || rm_inf(x)) return NAN;
  int q; double r = rm_turn(x, &q);
  double s = rm_sin_near(r), c = rm_cos_near(r);
  return (q & 1) ? -c / s : s / c;
}
/* e^x: 2^k e^r, |r| <= ln 2 / 2, by the series to r^11; 2^k laid in as exponent bits, in two
   halves past a double's normal range (each product exact but the last) */
static double rm_two_to(int k) { return rm_double((uint64_t)(k + 1023) << 52); }
double exp(double x) {
  if (rm_nan(x)) return x;
  if (x > 709.782712893384) return INFINITY;
  if (x < -745.1332191019412) return 0;
  double k = floor(x / (RM_LN2_HI + RM_LN2_LO) + 0.5);
  double r = (x - k * RM_LN2_HI) - k * RM_LN2_LO;
  double e = 1 + r * (1 + r * (1.0 / 2 + r * (1.0 / 6 + r * (1.0 / 24 + r * (1.0 / 120 + r * (1.0 / 720 + r * (1.0 / 5040
    + r * (1.0 / 40320 + r * (1.0 / 362880 + r * (1.0 / 3628800 + r * (1.0 / 39916800)))))))))));
  int n = (int)k;
  if (n > 1023) return e * rm_two_to(n - 1023) * rm_two_to(1023);
  if (n < -1022) return e * rm_two_to(n + 1000) * rm_two_to(-1000);
  return e * rm_two_to(n);
}
/* ln x: x = m 2^e, m in [sqrt 1/2, sqrt 2), ln m = 2 atanh(s), s = (m - 1)/(m + 1) <= 0.172, by its series to s^27 */
double log(double x) {
  if (rm_nan(x) || x < 0) return NAN;
  if (x == 0) return -INFINITY;
  if (x == INFINITY) return x;
  int shift = 0;
  if (x < 2.2250738585072014e-308) { x *= 18014398509481984.0; shift = 54; }   /* subnormal: times 2^54 first */
  uint64_t bits = rm_bits(x);
  int e = (int)((bits >> 52) & 2047) - 1023 - shift;
  double m = rm_double((bits & 4503599627370495ull) | 4607182418800017408ull);
  if (m > 1.4142135623730951) { m *= 0.5; e += 1; }   /* m in [sqrt 1/2, sqrt 2): ln x near 1 keeps its digits */
  double s = (m - 1) / (m + 1), z = s * s;
  double a = 2 * s * (1 + z * (1.0 / 3 + z * (1.0 / 5 + z * (1.0 / 7 + z * (1.0 / 9 + z * (1.0 / 11 + z * (1.0 / 13
    + z * (1.0 / 15 + z * (1.0 / 17 + z * (1.0 / 19 + z * (1.0 / 21 + z * (1.0 / 23 + z * (1.0 / 25 + z * (1.0 / 27))))))))))))));
  return (double)e * RM_LN2_HI + (a + (double)e * RM_LN2_LO);
}
/* a^b, with the C library's answers for zeros, infinities, NaNs and a negative a (C11 F.10.4.4) */
static int rm_whole(double b) { return !rm_inf(b) && floor(b) == b; }
static int rm_odd(double b) { return rm_whole(b) && b < 9007199254740992.0 && b > -9007199254740992.0 && fmod(b, 2) != 0; }
double pow(double a, double b) {
  if (b == 0 || a == 1) return 1;
  if (rm_nan(a) || rm_nan(b)) return NAN;
  if (rm_inf(b)) {
    double m = a < 0 ? -a : a;
    if (m == 1) return 1;
    return (m < 1) == (b > 0) ? 0 : INFINITY;
  }
  if (a == 0) {
    if (b > 0) return rm_odd(b) ? a : 0;
    return rm_odd(b) ? (1 / a) : INFINITY;   /* 1 / -0 is -inf */
  }
  if (rm_inf(a)) {
    if (a > 0) return b > 0 ? INFINITY : 0;
    if (b > 0) return rm_odd(b) ? -INFINITY : INFINITY;
    return rm_odd(b) ? -0.0 : 0;
  }
  if (a < 0) {
    if (!rm_whole(b)) return NAN;
    double v = exp(b * log(-a));
    return rm_odd(b) ? -v : v;
  }
  return exp(b * log(a));
}
float sinf(float x) { return (float)sin((double)x); }
float cosf(float x) { return (float)cos((double)x); }
float tanf(float x) { return (float)tan((double)x); }
float expf(float x) { return (float)exp((double)x); }
float logf(float x) { return (float)log((double)x); }
float powf(float a, float b) { return (float)pow((double)a, (double)b); }
