// C emitter: one self-contained C11 file per program. The runtime that
// Basic needs (print, checked arithmetic, bounds and cast checks) is written
// into the file, so the output builds with any C compiler and no Coax
// install: Xcode, Emscripten and plain cc treat it like any other C source.
//
// Runtime operands are evaluated left to right (spec §7.3): when an operand
// list has a call and another operand that could trap or call, the operands
// are stored in temporaries first, in source order, inside one C comma
// expression. Short-circuit operators and ifx keep C's own ordering.
#include "coax.h"

#include <cmath>
#include <algorithm>
#include <functional>
#include <map>
#include <set>
#include <filesystem>

namespace coax {

static const char *RUNTIME = R"COAXRT(/* Coax float arithmetic is plain IEEE, one rounding per operation: no fused
   multiply-add, so results match compile-time evaluation and other compilers. */
#pragma STDC FP_CONTRACT OFF
/* Generated code carries the whole runtime and keeps every declared local. */
#if defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-function"
#pragma GCC diagnostic ignored "-Wunused-variable"
#endif
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

typedef struct { const char *data; int64_t count; } coax_string;
#define COAX_STR(lit, n) ((coax_string){(lit), (n)})

static void coax_trap(const char *what, int32_t line) {
  fflush(stdout);
  fprintf(stderr, "%s:%d: %s\n", coax_source_path, line, what);
  abort();
}

/* Checked arithmetic for every integer width. Debug builds call the
   checked forms, release builds the wrapping ones (spelled with the
   overflow builtins so no C undefined behavior remains). Division by zero,
   bad shifts, bounds and casts trap in every build. */
#define COAX_INT_OPS(T, N, SIGNED, MIN)                                                                  \
  static T coax_add_##N(T a, T b, int32_t l) { T r; if (__builtin_add_overflow(a, b, &r)) coax_trap("integer overflow in '+'", l); return r; } \
  static T coax_sub_##N(T a, T b, int32_t l) { T r; if (__builtin_sub_overflow(a, b, &r)) coax_trap("integer overflow in '-'", l); return r; } \
  static T coax_mul_##N(T a, T b, int32_t l) { T r; if (__builtin_mul_overflow(a, b, &r)) coax_trap("integer overflow in '*'", l); return r; } \
  static T coax_neg_##N(T a, int32_t l) { T r; if (__builtin_sub_overflow((T)0, a, &r)) coax_trap("integer overflow in unary '-'", l); return r; } \
  static T coax_wadd_##N(T a, T b) { T r; __builtin_add_overflow(a, b, &r); return r; }                \
  static T coax_wsub_##N(T a, T b) { T r; __builtin_sub_overflow(a, b, &r); return r; }                \
  static T coax_wmul_##N(T a, T b) { T r; __builtin_mul_overflow(a, b, &r); return r; }                \
  static T coax_wneg_##N(T a) { T r; __builtin_sub_overflow((T)0, a, &r); return r; }                  \
  static T coax_div_##N(T a, T b, int32_t l) {                                                          \
    if (b == 0) coax_trap("division by zero", l);                                                       \
    if (SIGNED && a == MIN && b == (T)-1) coax_trap("integer overflow in '/'", l);                      \
    return (T)(a / b);                                                                                  \
  }                                                                                                     \
  static T coax_mod_##N(T a, T b, int32_t l) {                                                          \
    if (b == 0) coax_trap("division by zero in '%'", l);                                                \
    if (SIGNED && b == (T)-1) return 0;                                                                 \
    return (T)(a % b);                                                                                  \
  }                                                                                                     \
  static T coax_shl_##N(T a, int64_t b, int32_t l) {                                                    \
    if (b < 0 || b >= (int64_t)(8 * sizeof(T))) coax_trap("shift amount out of range", l);              \
    return (T)((uint64_t)a << b);                                                                       \
  }                                                                                                     \
  static T coax_shr_##N(T a, int64_t b, int32_t l) {                                                    \
    if (b < 0 || b >= (int64_t)(8 * sizeof(T))) coax_trap("shift amount out of range", l);              \
    return (T)(a >> b);                                                                                 \
  }
COAX_INT_OPS(int8_t, s8, 1, INT8_MIN)
COAX_INT_OPS(int16_t, s16, 1, INT16_MIN)
COAX_INT_OPS(int32_t, s32, 1, INT32_MIN)
COAX_INT_OPS(int64_t, s64, 1, INT64_MIN)
COAX_INT_OPS(uint8_t, u8, 0, 0)
COAX_INT_OPS(uint16_t, u16, 0, 0)
COAX_INT_OPS(uint32_t, u32, 0, 0)
COAX_INT_OPS(uint64_t, u64, 0, 0)

static int64_t coax_index(int64_t i, int64_t n, int32_t l) {
  if (i < 0 || i >= n) coax_trap("index out of bounds", l);
  return i;
}
static void coax_slice_check(int64_t lo, int64_t hi, int64_t n, int32_t l) {
  if (lo < 0 || hi < lo || hi > n) coax_trap("slice out of bounds", l);
}
static uint8_t coax_byte(coax_string s, int64_t i, int32_t l) { return (uint8_t)s.data[coax_index(i, s.count, l)]; }
static coax_string coax_substr(coax_string s, int64_t lo, int64_t hi, int32_t l) {
  coax_slice_check(lo, hi, s.count, l);
  coax_string r = {s.data + lo, hi - lo};
  return r;
}
static _Bool coax_string_eq(coax_string a, coax_string b) {
  return a.count == b.count && (a.count == 0 || memcmp(a.data, b.data, (size_t)a.count) == 0);
}

/* Checked casts: the value must fit the target, in every build. */
static int64_t coax_cast_ss(int64_t v, int64_t lo, int64_t hi, int32_t l) {
  if (v < lo || v > hi) coax_trap("cast out of range", l);
  return v;
}
static uint64_t coax_cast_su(int64_t v, uint64_t hi, int32_t l) {
  if (v < 0 || (uint64_t)v > hi) coax_trap("cast out of range", l);
  return (uint64_t)v;
}
static int64_t coax_cast_us(uint64_t v, int64_t hi, int32_t l) {
  if (v > (uint64_t)hi) coax_trap("cast out of range", l);
  return (int64_t)v;
}
static uint64_t coax_cast_uu(uint64_t v, uint64_t hi, int32_t l) {
  if (v > hi) coax_trap("cast out of range", l);
  return v;
}
static int64_t coax_ftoi_s(double v, int64_t lo, int64_t hi, int32_t l) {
  if (!(v >= -9223372036854775808.0 && v < 9223372036854775808.0)) coax_trap("cast out of range", l);
  int64_t i = (int64_t)v;
  if (i < lo || i > hi) coax_trap("cast out of range", l);
  return i;
}
static _Bool coax_bytes_overlap(const void *a, int64_t n, const void *b, int64_t m) {
  if(n<=0 || m<=0) return 0;
  uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;
  return x<=y ? y-x<(uint64_t)n : x-y<(uint64_t)m;
}
static float coax_f32_from_bits(uint32_t x){float f;memcpy(&f,&x,4);return f;}
static double coax_f64_from_bits(uint64_t x){double f;memcpy(&f,&x,8);return f;}
static uint32_t coax_f32_bits(float f){uint32_t x;memcpy(&x,&f,4);return x;}
static uint64_t coax_f64_bits(double f){uint64_t x;memcpy(&x,&f,8);return x;}
static float coax_to_f32(double v, int32_t l) {
  /* Finite values that would round to infinity do not fit; inf and nan stay. */
  if (isfinite(v) && fabs(v) >= 3.4028235677973366e38) coax_trap("cast out of range", l);
  return (float)v;
}
static uint64_t coax_ftoi_u(double v, uint64_t hi, int32_t l) {
  if (!(v > -1.0 && v < 18446744073709551616.0)) coax_trap("cast out of range", l);
  uint64_t u = (uint64_t)v;
  if (u > hi) coax_trap("cast out of range", l);
  return u;
}

/* Output. Floats print the shortest text that reads back as the same
   value, with a ".0" when it would otherwise look like an integer. */
static void coax_put(coax_string s) { fwrite(s.data, 1, (size_t)s.count, stdout); }
static void coax_print_int(int64_t v) { printf("%lld", (long long)v); }
static void coax_print_uint(uint64_t v) { printf("%llu", (unsigned long long)v); }
static void coax_print_bool(_Bool v) { fputs(v ? "true" : "false", stdout); }
static void coax_print_float(double v, int is_f32) {
  char buf[64];
  if (v != v) { fputs("nan", stdout); return; }
  if (v > 1.7976931348623157e308) { fputs("inf", stdout); return; }
  if (v < -1.7976931348623157e308) { fputs("-inf", stdout); return; }
  int p = 1;
  for (; p < 17; p++) {
    snprintf(buf, sizeof buf, "%.*e", p - 1, v);
    if (is_f32 ? (float)strtod(buf, 0) == (float)v : strtod(buf, 0) == v) break;
  }
  const char *e = strchr(buf, 'e');
  int exponent = e ? atoi(e + 1) : 0;
  if (exponent >= -5 && exponent < 17) {
    int decimals = p - 1 - exponent;
    snprintf(buf, sizeof buf, "%.*f", decimals > 0 ? decimals : 0, v);
    fputs(buf, stdout);
    if (!strchr(buf, '.')) fputs(".0", stdout);
  } else {
    snprintf(buf, sizeof buf, "%.*g", p, v);
    fputs(buf, stdout);
  }
}
static void coax_show_string(coax_string s) {
  fputc('"', stdout);
  for (int64_t i = 0; i < s.count; i++) {
    char c = s.data[i];
    if (c == '"' || c == '\\') { fputc('\\', stdout); fputc(c, stdout); }
    else if (c == '\n') fputs("\\n", stdout);
    else if (c == '\t') fputs("\\t", stdout);
    else if (c == '\r') fputs("\\r", stdout);
    else if (c == 0) fputs("\\0", stdout);
    else fputc(c, stdout);
  }
  fputc('"', stdout);
}
)COAXRT";
// Emitted only into programs that use value_barrier, so the seeds stay as they are.
static const char *BARRIER_RUNTIME = R"COAXRT(/* value_barrier (spec section 9): the value comes back unchanged, and the
   optimizer can't see through it to branch on a secret. Native: an empty asm
   that claims to change the register. wasm32 has no such asm: a call to an
   identity that is never inlined, through a volatile. */
#if defined(__wasm__) || !defined(__GNUC__)
#if defined(__GNUC__)
__attribute__((noinline))
#endif
static uint64_t coax_value_barrier_u64(uint64_t x){volatile uint64_t v=x;return v;}
#if defined(__GNUC__)
__attribute__((noinline))
#endif
static uint32_t coax_value_barrier_u32(uint32_t x){volatile uint32_t v=x;return v;}
#else
static inline uint64_t coax_value_barrier_u64(uint64_t x){__asm__("" : "+r"(x));return x;}
static inline uint32_t coax_value_barrier_u32(uint32_t x){__asm__("" : "+r"(x));return x;}
#endif
)COAXRT";

static const char *WRAP_RUNTIME = R"COAXRT(
static uint32_t coax_add_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a+b);}
static uint32_t coax_sub_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a-b);}
static uint32_t coax_mul_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a*b);}
static uint32_t coax_rotl_u32(uint32_t a,uint32_t b){unsigned k=(unsigned)(b&31u);return (uint32_t)((a<<k)|(a>>((32u-k)&31u)));}
static uint32_t coax_rotr_u32(uint32_t a,uint32_t b){unsigned k=(unsigned)(b&31u);return (uint32_t)((a>>k)|(a<<((32u-k)&31u)));}
static uint64_t coax_add_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a+b);}
static uint64_t coax_sub_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a-b);}
static uint64_t coax_mul_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a*b);}
static uint64_t coax_rotl_u64(uint64_t a,uint64_t b){unsigned k=(unsigned)(b&63u);return (uint64_t)((a<<k)|(a>>((64u-k)&63u)));}
static uint64_t coax_rotr_u64(uint64_t a,uint64_t b){unsigned k=(unsigned)(b&63u);return (uint64_t)((a>>k)|(a<<((64u-k)&63u)));}
)COAXRT";

// The device runtime of `coax cuda`: the same checks as RUNTIME, spelled for
// NVRTC (C++, no libc headers, no overflow builtins). A failed check executes
// a trap instruction: the launch fails and the host sees the error.
static const char *CUDA_RUNTIME = R"COAXRT(/* Coax float arithmetic is plain IEEE, one rounding per operation. Compile
   with --fmad=false (and the default --prec-div=true --prec-sqrt=true
   --ftz=false) so +, -, *, / and sqrt round exactly as the C build does. */
typedef signed char int8_t; typedef short int16_t; typedef int int32_t; typedef long long int64_t;
typedef unsigned char uint8_t; typedef unsigned short uint16_t; typedef unsigned int uint32_t; typedef unsigned long long uint64_t;
#define INT64_C(x) x##LL
#define UINT64_C(x) x##ULL
#define INT64_MIN (-9223372036854775807LL - 1)
#define INT64_MAX 9223372036854775807LL
#define UINT64_MAX 18446744073709551615ULL
#pragma nv_diag_suppress 177
#define COAX_DEV static __device__ inline
COAX_DEV void coax_trap(const char *what, int32_t line) { (void)what; (void)line; __trap(); }
/* Narrow integers are computed in 64 bits and range checked. */
#define COAX_NARROW_OPS(T, N, LO, HI)                                                                     \
  COAX_DEV T coax_add_##N(T a, T b, int32_t l) { int64_t r = (int64_t)a + (int64_t)b; if (r < LO || r > HI) coax_trap("integer overflow in '+'", l); return (T)r; } \
  COAX_DEV T coax_sub_##N(T a, T b, int32_t l) { int64_t r = (int64_t)a - (int64_t)b; if (r < LO || r > HI) coax_trap("integer overflow in '-'", l); return (T)r; } \
  COAX_DEV T coax_mul_##N(T a, T b, int32_t l) { int64_t r = (int64_t)a * (int64_t)b; if (r < LO || r > HI) coax_trap("integer overflow in '*'", l); return (T)r; } \
  COAX_DEV T coax_neg_##N(T a, int32_t l) { int64_t r = -(int64_t)a; if (r < LO || r > HI) coax_trap("integer overflow in unary '-'", l); return (T)r; } \
  COAX_DEV T coax_wadd_##N(T a, T b) { return (T)(uint64_t)((uint64_t)(int64_t)a + (uint64_t)(int64_t)b); }                        \
  COAX_DEV T coax_wsub_##N(T a, T b) { return (T)(uint64_t)((uint64_t)(int64_t)a - (uint64_t)(int64_t)b); }                        \
  COAX_DEV T coax_wmul_##N(T a, T b) { return (T)(uint64_t)((uint64_t)(int64_t)a * (uint64_t)(int64_t)b); }                        \
  COAX_DEV T coax_wneg_##N(T a) { return (T)(uint64_t)(0ULL - (uint64_t)(int64_t)a); }
#define COAX_SHIFT_DIV_OPS(T, N, SIGNED, MIN)                                                             \
  COAX_DEV T coax_div_##N(T a, T b, int32_t l) {                                                          \
    if (b == 0) coax_trap("division by zero", l);                                                       \
    if (SIGNED && a == MIN && b == (T)-1) coax_trap("integer overflow in '/'", l);                      \
    return (T)(a / b);                                                                                  \
  }                                                                                                     \
  COAX_DEV T coax_mod_##N(T a, T b, int32_t l) {                                                          \
    if (b == 0) coax_trap("division by zero in '%'", l);                                                \
    if (SIGNED && b == (T)-1) return 0;                                                                 \
    return (T)(a % b);                                                                                  \
  }                                                                                                     \
  COAX_DEV T coax_shl_##N(T a, int64_t b, int32_t l) {                                                    \
    if (b < 0 || b >= (int64_t)(8 * sizeof(T))) coax_trap("shift amount out of range", l);              \
    return (T)((uint64_t)a << b);                                                                       \
  }                                                                                                     \
  COAX_DEV T coax_shr_##N(T a, int64_t b, int32_t l) {                                                    \
    if (b < 0 || b >= (int64_t)(8 * sizeof(T))) coax_trap("shift amount out of range", l);              \
    return (T)(a >> b);                                                                                 \
  }
COAX_NARROW_OPS(int8_t, s8, -128LL, 127LL)
COAX_NARROW_OPS(int16_t, s16, -32768LL, 32767LL)
COAX_NARROW_OPS(int32_t, s32, -2147483648LL, 2147483647LL)
COAX_NARROW_OPS(uint8_t, u8, 0LL, 255LL)
COAX_NARROW_OPS(uint16_t, u16, 0LL, 65535LL)
COAX_NARROW_OPS(uint32_t, u32, 0LL, 4294967295LL)
COAX_DEV int64_t coax_add_s64(int64_t a, int64_t b, int32_t l) { int64_t r = (int64_t)((uint64_t)a + (uint64_t)b); if (((a ^ r) & (b ^ r)) < 0) coax_trap("integer overflow in '+'", l); return r; }
COAX_DEV int64_t coax_sub_s64(int64_t a, int64_t b, int32_t l) { int64_t r = (int64_t)((uint64_t)a - (uint64_t)b); if (((a ^ b) & (a ^ r)) < 0) coax_trap("integer overflow in '-'", l); return r; }
COAX_DEV int64_t coax_mul_s64(int64_t a, int64_t b, int32_t l) {
  int64_t r = (int64_t)((uint64_t)a * (uint64_t)b);
  if ((a == -1 && b == INT64_MIN) || (b == -1 && a == INT64_MIN) || (a != 0 && (r / a != b))) coax_trap("integer overflow in '*'", l);
  return r;
}
COAX_DEV int64_t coax_neg_s64(int64_t a, int32_t l) { if (a == INT64_MIN) coax_trap("integer overflow in unary '-'", l); return -a; }
COAX_DEV int64_t coax_wadd_s64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
COAX_DEV int64_t coax_wsub_s64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
COAX_DEV int64_t coax_wmul_s64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }
COAX_DEV int64_t coax_wneg_s64(int64_t a) { return (int64_t)(0ULL - (uint64_t)a); }
COAX_DEV uint64_t coax_add_u64(uint64_t a, uint64_t b, int32_t l) { uint64_t r = a + b; if (r < a) coax_trap("integer overflow in '+'", l); return r; }
COAX_DEV uint64_t coax_sub_u64(uint64_t a, uint64_t b, int32_t l) { if (b > a) coax_trap("integer overflow in '-'", l); return a - b; }
COAX_DEV uint64_t coax_mul_u64(uint64_t a, uint64_t b, int32_t l) { uint64_t r = a * b; if (a != 0 && r / a != b) coax_trap("integer overflow in '*'", l); return r; }
COAX_DEV uint64_t coax_neg_u64(uint64_t a, int32_t l) { if (a != 0) coax_trap("integer overflow in unary '-'", l); return 0; }
COAX_DEV uint64_t coax_wadd_u64(uint64_t a, uint64_t b) { return a + b; }
COAX_DEV uint64_t coax_wsub_u64(uint64_t a, uint64_t b) { return a - b; }
COAX_DEV uint64_t coax_wmul_u64(uint64_t a, uint64_t b) { return a * b; }
COAX_DEV uint64_t coax_wneg_u64(uint64_t a) { return 0ULL - a; }
COAX_SHIFT_DIV_OPS(int8_t, s8, 1, (int8_t)-128)
COAX_SHIFT_DIV_OPS(int16_t, s16, 1, (int16_t)-32768)
COAX_SHIFT_DIV_OPS(int32_t, s32, 1, (int32_t)(-2147483647 - 1))
COAX_SHIFT_DIV_OPS(int64_t, s64, 1, INT64_MIN)
COAX_SHIFT_DIV_OPS(uint8_t, u8, 0, 0)
COAX_SHIFT_DIV_OPS(uint16_t, u16, 0, 0)
COAX_SHIFT_DIV_OPS(uint32_t, u32, 0, 0)
COAX_SHIFT_DIV_OPS(uint64_t, u64, 0, 0)
/* n is a count, never negative, so one unsigned comparison is the whole check. */
COAX_DEV int64_t coax_index(int64_t i, int64_t n, int32_t l) { if ((uint64_t)i >= (uint64_t)n) coax_trap("index out of bounds", l); return i; }
COAX_DEV void coax_slice_check(int64_t lo, int64_t hi, int64_t n, int32_t l) { if ((uint64_t)hi > (uint64_t)n || (uint64_t)lo > (uint64_t)hi) coax_trap("slice out of bounds", l); }
COAX_DEV int64_t coax_cast_ss(int64_t v, int64_t lo, int64_t hi, int32_t l) { if (v < lo || v > hi) coax_trap("cast out of range", l); return v; }
COAX_DEV uint64_t coax_cast_su(int64_t v, uint64_t hi, int32_t l) { if (v < 0 || (uint64_t)v > hi) coax_trap("cast out of range", l); return (uint64_t)v; }
COAX_DEV int64_t coax_cast_us(uint64_t v, int64_t hi, int32_t l) { if (v > (uint64_t)hi) coax_trap("cast out of range", l); return (int64_t)v; }
COAX_DEV uint64_t coax_cast_uu(uint64_t v, uint64_t hi, int32_t l) { if (v > hi) coax_trap("cast out of range", l); return v; }
COAX_DEV int64_t coax_ftoi_s(double v, int64_t lo, int64_t hi, int32_t l) {
  if (!(v >= -9223372036854775808.0 && v < 9223372036854775808.0)) coax_trap("cast out of range", l);
  int64_t i = (int64_t)v; if (i < lo || i > hi) coax_trap("cast out of range", l); return i;
}
COAX_DEV uint64_t coax_ftoi_u(double v, uint64_t hi, int32_t l) {
  if (!(v > -1.0 && v < 18446744073709551616.0)) coax_trap("cast out of range", l);
  uint64_t u = (uint64_t)v; if (u > hi) coax_trap("cast out of range", l); return u;
}
COAX_DEV float coax_f32_from_bits(uint32_t x) { return __int_as_float((int)x); }
COAX_DEV double coax_f64_from_bits(uint64_t x) { return __longlong_as_double((long long)x); }
COAX_DEV uint32_t coax_f32_bits(float f) { return (uint32_t)__float_as_int(f); }
COAX_DEV uint64_t coax_f64_bits(double f) { return (uint64_t)__double_as_longlong(f); }
COAX_DEV float coax_to_f32(double v, int32_t l) {
  if (v == v && v - v == 0.0 && fabs(v) >= 3.4028235677973366e38) coax_trap("cast out of range", l);
  return (float)v;
}
COAX_DEV uint32_t coax_add_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a+b);}
COAX_DEV uint32_t coax_sub_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a-b);}
COAX_DEV uint32_t coax_mul_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a*b);}
COAX_DEV uint32_t coax_rotl_u32(uint32_t a,uint32_t b){unsigned k=(unsigned)(b&31u);return (uint32_t)((a<<k)|(a>>((32u-k)&31u)));}
COAX_DEV uint32_t coax_rotr_u32(uint32_t a,uint32_t b){unsigned k=(unsigned)(b&31u);return (uint32_t)((a>>k)|(a<<((32u-k)&31u)));}
COAX_DEV uint64_t coax_add_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a+b);}
COAX_DEV uint64_t coax_sub_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a-b);}
COAX_DEV uint64_t coax_mul_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a*b);}
COAX_DEV uint64_t coax_rotl_u64(uint64_t a,uint64_t b){unsigned k=(unsigned)(b&63u);return (uint64_t)((a<<k)|(a>>((64u-k)&63u)));}
COAX_DEV uint64_t coax_rotr_u64(uint64_t a,uint64_t b){unsigned k=(unsigned)(b&63u);return (uint64_t)((a>>k)|(a<<((64u-k)&63u)));}
)COAXRT";

// Uses the named Coax record declared in Types; inserted only when used.
static const char *WIDE_RUNTIME = R"COAXRT(
static coax_t_CoaxU64Product coax_mul_wide(uint64_t a,uint64_t b) {
#if defined(__SIZEOF_INT128__) && !defined(__wasm__)
  unsigned __int128 p=(unsigned __int128)a*b;
  return (coax_t_CoaxU64Product){(uint64_t)(p>>64),(uint64_t)p};
#else
  uint64_t a0=(uint32_t)a,a1=a>>32,b0=(uint32_t)b,b1=b>>32;
  uint64_t p00=a0*b0,p01=a0*b1,p10=a1*b0,p11=a1*b1;
  uint64_t carry=(p00>>32)+(uint32_t)p01+(uint32_t)p10;
  return (coax_t_CoaxU64Product){p11+(p01>>32)+(p10>>32)+(carry>>32),
                              (p00&UINT64_C(0xffffffff))|(carry<<32)};
#endif
}
)COAXRT";

// Warp kernels (compute.cpp fills this before emission): lane loops,
// per-lane arrays and their element offsets, and the MMA primitive's calls.
struct WarpInfo {
  std::set<const Stmt *> lane_loops, lane_arrays;
  std::map<const Expr *, int64_t> lane_index;  // index expression lane*k+c -> k
  std::set<const ProcDecl *> warp_kernels;
  std::set<const Expr *> mma_calls;
  // Group kernels: thread loops and per-thread arrays by group size T,
  // per-thread indexes thread*k+c -> k, each kernel's threadgroup arrays,
  // matrix accumulators and the Apple matrix primitives' calls by kind.
  std::map<const Stmt *, int64_t> thread_loops, thread_arrays;
  std::map<const Expr *, int64_t> thread_index;
  std::map<const ProcDecl *, int64_t> group_kernels;
  std::map<const ProcDecl *, std::vector<const Stmt *>> shared_arrays;
  std::set<const Stmt *> shared_decls, accumulators;
  std::map<const Expr *, int> apple_calls;
};
WarpInfo g_warp;
static bool is_mma_primitive(const ProcDecl *p) { return p && str_eq(p->name, "cu_mma_bf16_16x8x16"); }
// Every device primitive: its Coax body is the CPU meaning; the device emitters lower calls to it.
static bool is_device_primitive(const ProcDecl *p) {
  return p && (is_mma_primitive(p) || str_eq(p->name, "am_sg_mma_64x32x32") || str_eq(p->name, "am_sg_spill_64x32") || str_eq(p->name, "am_matmul_bf16"));
}
// The integer value of a constant expression (a literal or a named constant).
static int64_t constant_of_expr(const Expr *e) {
  if (e && e->kind == ExprKind::ident && e->constant && e->constant->expr) e = e->constant->expr;
  return e && e->kind == ExprKind::int_lit ? (int64_t)e->int_value : -1;
}
// Set while `coax metal-compute` emits: f64 is the float-float pair coax_ff.
static bool g_metal_compute = false;

namespace {

// A unique, kind-prefixed spelling of a type for generated names.
std::string tag(Type t) {
  switch (t->kind) {
    case TypeKind::integer: return t == T_INT ? "s64" : t->name;
    case TypeKind::floating: return t->name;
    case TypeKind::bool_: return "bool";
    case TypeKind::string_: return "string";
    case TypeKind::struct_: case TypeKind::enum_: return "t_" + t->name;
    case TypeKind::array: return "a" + std::to_string(t->length) + "_" + tag(t->elem);
    case TypeKind::view: return "v_" + tag(t->elem);
    default: return "void";
  }
}

std::string c_type(Type t) {
  switch (t->kind) {
    case TypeKind::void_: return "void";
    case TypeKind::integer: return std::string(t->is_signed ? "int" : "uint") + std::to_string(t->bits) + "_t";
    case TypeKind::floating: return t == &T_F32 ? "float" : g_metal_compute ? "coax_ff" : "double";
    case TypeKind::bool_: return "_Bool";
    case TypeKind::string_: return "coax_string";
    default: return "coax_" + tag(t);
  }
}

std::string int_tag(Type t) {
  Type base = t->kind == TypeKind::enum_ ? t->elem : t;
  return std::string(base->is_signed ? "s" : "u") + std::to_string(base->bits);
}

bool needs_init(Type t) {
  if (t->kind == TypeKind::array) return needs_init(t->elem);
  if (t->kind != TypeKind::struct_) return false;
  for (const Field &f : t->struct_decl->fields)
    if (f.init || needs_init(f.type)) return true;
  return false;
}

std::string c_string_text(Str s) {
  std::string out = "\"";
  for (int32_t i = 0; i < s.count; i++) {
    unsigned char c = (unsigned char)s.data[i];
    if (c == '"' || c == '\\') { out += '\\'; out += (char)c; }
    else if (c == '\n') out += "\\n";
    else if (c == '\t') out += "\\t";
    else if (c == '\r') out += "\\r";
    else if (c < 0x20 || c >= 0x7f || c == '?') {
      char buf[8];
      snprintf(buf, sizeof buf, "\\%03o", c);  // octal: never swallows a following digit
      out += buf;
    } else out += (char)c;
  }
  return out + "\"";
}

std::string int_text(Type t, uint64_t bits) {
  Type base = t->kind == TypeKind::enum_ ? t->elem : t;
  if (base->is_signed) {
    int64_t v = (int64_t)bits;
    std::string lit = v == INT64_MIN ? "INT64_MIN" : "INT64_C(" + std::to_string(v) + ")";
    return base == T_INT && t == base ? lit : "((" + c_type(t) + ")" + lit + ")";
  }
  std::string lit = "UINT64_C(" + std::to_string(bits) + ")";
  return base == &T_U64 && t == base ? lit : "((" + c_type(t) + ")" + lit + ")";
}

std::string float_text(Type t, double v) {
  if (std::isnan(v)) return t == &T_F32 ? "((float)__builtin_nan(\"\"))" : "__builtin_nan(\"\")";
  if (std::isinf(v)) return std::string(v < 0 ? "(-" : "(") + (t == &T_F32 ? "__builtin_inff())" : "__builtin_inf())");
  char buf[64];
  snprintf(buf, sizeof buf, "%a", v);  // exact: no decimal rounding between Coax and C
  return std::string(buf) + (t == &T_F32 ? "f" : "");
}

// Expressions that cannot trap, call or observe order.
bool trivial(const Expr *e) {
  switch (e->kind) {
    case ExprKind::int_lit: case ExprKind::float_lit: case ExprKind::bool_lit: case ExprKind::string_lit:
    case ExprKind::enum_value: case ExprKind::ct_value: case ExprKind::ident:
      return true;
    case ExprKind::field: return trivial(e->lhs);
    default: return false;
  }
}

// An lvalue in the generated C.
bool is_place(const Expr *e) {
  switch (e->kind) {
    case ExprKind::ident: return !e->constant;
    case ExprKind::field: return e->field_index >= 0 && is_place(e->lhs);
    case ExprKind::index: return e->lhs->type->kind == TypeKind::view || (e->lhs->type->kind == TypeKind::array && is_place(e->lhs));
    default: return false;
  }
}

struct Operand {
  const Expr *e;
  bool lvalue;
};

std::string escape_path(const std::string &path);

std::string debug_path(const Source *src, Loc loc) {
  return std::filesystem::absolute(source_point(src,loc).path).lexically_normal().string();
}

std::string debug_line(const File *file, Loc loc) {
  auto point=source_point(file->src,loc);
  return "#line "+std::to_string(point.loc.line)+" \""+escape_path(debug_path(file->src,loc))+"\"\n";
}

struct Emitter {
  const File *file;
  EmitOptions opts;
  std::string out;          // the current function body
  int32_t indent = 0;
  int32_t temp_count = 0;
  std::vector<std::string> temps;           // declarations at the top of the current function
  std::vector<const Expr *> ct_values;      // compile-time aggregates, emitted as static constants
  bool wrapping = false;
  bool wide = false;
  bool barrier = false;                       // value_barrier is used: emit its helpers
  std::vector<std::vector<const Stmt *>> defers;  // per open block
  std::vector<size_t> loop_scopes;          // defers.size() when each open loop began
  const ProcDecl *current = nullptr;
  bool ret_temp = false;
  Loc debug_location{};
  bool mapping_active=false;

  void line_start() {
    if(opts.debug_info && mapping_active)out+=debug_line(file,debug_location);
    out.append((size_t)indent * 2,' ');
  }
  void line(const std::string &s) {
    line_start();
    out += s;
    out += '\n';
  }

  std::string new_temp(const std::string &type) {
    std::string name = "coax_t" + std::to_string(temp_count++);
    temps.push_back(type + " " + name + ";");
    return name;
  }

  std::string local_name(Str s) { return "l_" + to_std(s); }

  // "(x == y)" rather than "((x == y))", which compilers warn about.
  std::string condition(const Expr *e) {
    std::string c = ex(e);
    if (c.size() >= 2 && c.front() == '(' && c.back() == ')') {
      int depth = 0;
      size_t i = 0;
      for (; i < c.size(); i++) {
        depth += c[i] == '(' ? 1 : c[i] == ')' ? -1 : 0;
        if (depth == 0) break;
      }
      if (i == c.size() - 1) return c;
    }
    return "(" + c + ")";
  }

  // Device code: CUDA (coax cuda) or Metal (coax metal-compute).
  bool device() const { return opts.cuda || opts.metal_compute; }
  std::string group_barrier() const {
    return opts.metal_compute ? "threadgroup_barrier(mem_flags::mem_threadgroup | mem_flags::mem_device);" : "__syncthreads();";
  }
  bool apple_tensor = false;  // an am_matmul_bf16 call: the module needs Metal 4 tensor ops
  // The pointer type of a temporary holding the address of a place. Metal
  // pointers name their address space: a view's elements are device memory,
  // locals are thread memory.
  std::string pointer_type(const Expr *place) {
    if (!opts.metal_compute) return c_type(place->type) + " *";
    const Expr *e = place;
    bool space_device = false;
    while (e && (e->kind == ExprKind::index || e->kind == ExprKind::field)) {
      if (e->kind == ExprKind::index && e->lhs->type->kind == TypeKind::view) { space_device = true; break; }
      e = e->lhs;
    }
    return std::string(space_device ? "device " : "thread ") + c_type(place->type) + " *";
  }
  // C's math.h names, with the float variants for f32; Metal overloads.
  std::string math_name(const Expr *e) {
    std::string name = e->intrinsic == Intrinsic::abs ? "fabs" : to_std(e->text);
    if (opts.metal_compute) {
      if (e->type != &T_F32) return "coax_ff_" + to_std(e->text);  // exp, log, sqrt, floor, abs (admission)
      if (e->intrinsic == Intrinsic::abs || e->intrinsic == Intrinsic::floor) return name;
      return "precise::" + name;
    }
    if (opts.metal) return e->intrinsic == Intrinsic::abs ? "abs" : name;
    return name + (e->type == &T_F32 ? "f" : "");
  }
  std::string proc_name(const ProcDecl *p) {
    if (device() && p->export_c) return "coax_k_" + to_std(p->name);  // the kernel wrapper keeps the name
    return p->export_c || p->foreign ? to_std(p->name) : "coax_p_" + to_std(p->name);
  }

  // Evaluate operands left to right when order is observable; build() gets
  // the C text for each operand. A place (an lvalue in Coax) stays one in C:
  // a comma expression is not, so its address is taken inside it,
  // `(*(t = ..., &(place)))`, and it can be assigned to or addressed again.
  template <class F> std::string ordered(const std::vector<Operand> &ops, F build, bool place = false) {
    int calls = 0, nontrivial = 0;
    for (const Operand &o : ops) {
      if (has_call(o.e)) calls++;
      if (!trivial(o.e)) nontrivial++;
    }
    std::vector<std::string> texts;
    if (calls == 0 || nontrivial < 2) {
      for (const Operand &o : ops) texts.push_back(ex(o.e));
      return build(texts);
    }
    std::string prefix;
    for (const Operand &o : ops) {
      if (trivial(o.e)) {
        texts.push_back(ex(o.e));
        continue;
      }
      if (o.lvalue) {
        std::string t = new_temp(pointer_type(o.e));
        prefix += t + " = &(" + ex(o.e) + "), ";
        texts.push_back("(*" + t + ")");
      } else {
        std::string t = new_temp(c_type(o.e->type));
        prefix += t + " = " + ex(o.e) + ", ";
        texts.push_back(t);
      }
    }
    if (place) return "(*(" + prefix + "&(" + build(texts) + ")))";
    return "(" + prefix + build(texts) + ")";
  }

  std::string zero_value(Type t) {
    if (device() && (t->kind == TypeKind::array || t->kind == TypeKind::view)) return c_type(t) + "{}";
    if (opts.metal_compute && t == &T_F64) return "coax_ff_make(0.0f, 0.0f)";
    if (needs_init(t)) return "coax_init_" + tag(t) + "()";
    switch (t->kind) {
      case TypeKind::string_: return "COAX_STR(0, 0)";
      case TypeKind::struct_: case TypeKind::array: case TypeKind::view: return "((" + c_type(t) + "){0})";
      case TypeKind::floating: return t == &T_F32 ? "0.0f" : "0.0";
      default: return "((" + c_type(t) + ")0)";
    }
  }

  std::string ct_name(const Expr *e) {
    Expr *m = (Expr *)e;
    if (m->ct_id < 0) {
      m->ct_id = (int32_t)ct_values.size();
      ct_values.push_back(e);
    }
    return "coax_k" + std::to_string(m->ct_id);
  }

  // A compile-time value as a static initializer.
  std::string init_list(const Value &v) {
    Type t = v.type;
    switch (t->kind) {
      case TypeKind::integer: case TypeKind::enum_: return int_text(t, v.bits);
      case TypeKind::bool_: return v.bits ? "1" : "0";
      case TypeKind::floating: return float_text(t, v.f);
      case TypeKind::string_: return "{" + c_string_text(v.s) + ", " + std::to_string(v.s.count) + "}";
      case TypeKind::struct_: {
        std::string s = "{";
        const StructDecl *d = t->struct_decl;
        for (int32_t i = 0; i < d->fields.count; i++) {
          if (i) s += ", ";
          s += "." + to_std(d->fields[i].name) + " = " + init_list((*v.agg)[(size_t)i]);
        }
        if (!d->fields.count) s += "0";
        return s + "}";
      }
      case TypeKind::array: {
        std::string s = "{{";
        for (size_t i = 0; i < v.agg->size(); i++) {
          if (i) s += ", ";
          s += init_list((*v.agg)[i]);
        }
        return s + "}}";
      }
      default: return "{0}";
    }
  }

  // ---- expressions ----

  std::string arith(const char *name, const Expr *e) {
    std::string fn = std::string("coax_") + (opts.release && (strcmp(name, "add") == 0 || strcmp(name, "sub") == 0 || strcmp(name, "mul") == 0) ? "w" : "") +
                     name + "_" + int_tag(e->type);
    bool wrapping = fn.compare(0, 6, "coax_w") == 0;
    std::string line = std::to_string(e->loc.line);
    return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &t) {
      return fn + "(" + t[0] + ", " + t[1] + (wrapping ? "" : ", " + line) + ")";
    });
  }

  std::string cast(const Expr *e) {
    Type to = e->type, from = e->lhs->type;
    std::string x = ex(e->lhs), line = std::to_string(e->loc.line);
    if (from == to) return x;
    if (to == &T_STRING) return "coax_string_of_bytes(" + x + ")";
    if (opts.metal_compute && is_float(to)) {
      if (to == &T_F32 && from == &T_F64) return "coax_ff_to_f32(" + x + ", " + line + ")";
      if (to == &T_F64) {
        if (from == &T_F32) return "coax_ff_from_f32(" + x + ")";
        Type b = from->kind == TypeKind::enum_ ? from->elem : from;
        return std::string(b->is_signed ? "coax_ff_from_s64((int64_t)(" : "coax_ff_from_u64((uint64_t)(") + x + "))";
      }
    }
    if (opts.metal_compute && from == &T_F32 && !is_float(to) && to->kind != TypeKind::enum_) {
      Type tb = to;
      std::string lo = tb->is_signed ? (tb->bits == 64 ? "INT64_MIN" : "INT64_C(" + std::to_string(-(int64_t)1 << (tb->bits - 1)) + ")") : "0";
      std::string hi_s = "INT64_C(" + std::to_string(tb->bits == 64 ? INT64_MAX : ((int64_t)1 << (tb->bits - 1)) - 1) + ")";
      std::string hi_u = "UINT64_C(" + std::to_string(tb->bits == 64 ? UINT64_MAX : ((uint64_t)1 << tb->bits) - 1) + ")";
      return "((" + c_type(to) + ")" + (tb->is_signed ? "coax_ftoi_s((float)(" + x + "), " + lo + ", " + hi_s + ", " + line + ")"
                                                      : "coax_ftoi_u((float)(" + x + "), " + hi_u + ", " + line + ")") + ")";
    }
    if (to == &T_F32 && from == &T_F64 && !opts.metal) return "coax_to_f32(" + x + ", " + line + ")";
    if (is_float(to)) return "((" + c_type(to) + ")(" + x + "))";
    if (to->kind == TypeKind::enum_) {
      Type b = from->kind == TypeKind::enum_ ? from->elem : from;
      return b->is_signed ? "coax_to_" + tag(to) + "((int64_t)(" + x + "), 0, 1, " + line + ")"
                          : "coax_to_" + tag(to) + "(0, (uint64_t)(" + x + "), 0, " + line + ")";
    }
    Type tb = to->kind == TypeKind::enum_ ? to->elem : to;
    std::string ct = c_type(to);
    std::string lo = tb->is_signed ? (tb->bits == 64 ? "INT64_MIN" : "INT64_C(" + std::to_string(-(int64_t)1 << (tb->bits - 1)) + ")") : "0";
    std::string hi_s = "INT64_C(" + std::to_string(tb->bits == 64 ? INT64_MAX : ((int64_t)1 << (tb->bits - 1)) - 1) + ")";
    std::string hi_u = "UINT64_C(" + std::to_string(tb->bits == 64 ? UINT64_MAX : ((uint64_t)1 << tb->bits) - 1) + ")";
    if (is_float(from)) {
      return "((" + ct + ")" + (tb->is_signed ? "coax_ftoi_s((double)(" + x + "), " + lo + ", " + hi_s + ", " + line + ")"
                                              : "coax_ftoi_u((double)(" + x + "), " + hi_u + ", " + line + ")") + ")";
    }
    if (e->trunc) return "((" + ct + ")(" + x + "))";
    Type fb = from->kind == TypeKind::enum_ ? from->elem : from;
    std::string fn;
    if (fb->is_signed) fn = tb->is_signed ? "coax_cast_ss((int64_t)(" + x + "), " + lo + ", " + hi_s
                                          : "coax_cast_su((int64_t)(" + x + "), " + hi_u;
    else fn = tb->is_signed ? "coax_cast_us((uint64_t)(" + x + "), " + hi_s : "coax_cast_uu((uint64_t)(" + x + "), " + hi_u;
    return "((" + ct + ")" + fn + ", " + line + "))";
  }

  std::string ex(const Expr *e) {
    std::string line = std::to_string(e->loc.line);
    switch (e->kind) {
      case ExprKind::int_lit: return int_text(e->type, e->int_value);
      case ExprKind::enum_value: return int_text(e->type, e->int_value);
      case ExprKind::float_lit:
        if (opts.metal) {
          // Metal gets decimal text: 9 digits round-trip any f32.
          char buf[64];
          snprintf(buf, sizeof buf, "%.9g", e->float_value);
          std::string t = buf;
          if (t.find_first_of(".eEn") == std::string::npos) t += ".0";
          return t + "f";
        }
        if (opts.metal_compute) {
          // Decimal text: 9 digits round-trip any f32. An f64 is the pair
          // hi = f32(v), lo = f32(v - hi).
          auto text = [](float f) -> std::string {
            if (std::isnan(f)) return "as_type<float>(0x7fc00000u)";
            if (std::isinf(f)) return f < 0 ? "as_type<float>(0xff800000u)" : "as_type<float>(0x7f800000u)";
            char buf[64];
            snprintf(buf, sizeof buf, "%.9g", (double)f);
            std::string t = buf;
            if (t.find_first_of(".eEn") == std::string::npos) t += ".0";
            return t + "f";
          };
          if (e->type == &T_F32) return text((float)e->float_value);
          float hi = (float)e->float_value;
          float lo = std::isfinite(hi) ? (float)(e->float_value - (double)hi) : 0.0f;
          return "coax_ff_make(" + text(hi) + ", " + text(lo) + ")";
        }
        if (opts.cuda && !std::isfinite(e->float_value)) {
          // NVRTC has no __builtin_nan/inf: spell the bit pattern.
          bool f32 = e->type == &T_F32, nan = std::isnan(e->float_value), neg = e->float_value < 0;
          if (f32) return std::string("__int_as_float((int)") + (nan ? "0x7fc00000u" : neg ? "0xff800000u" : "0x7f800000u") + ")";
          return std::string("__longlong_as_double((long long)") + (nan ? "0x7ff8000000000000ULL" : neg ? "0xfff0000000000000ULL" : "0x7ff0000000000000ULL") + ")";
        }
        return float_text(e->type, e->float_value);
      case ExprKind::bool_lit: return e->int_value ? "1" : "0";
      case ExprKind::string_lit: return "COAX_STR(" + c_string_text(e->text) + ", " + std::to_string(e->text.count) + ")";
      case ExprKind::ct_value: return ct_name(e);
      case ExprKind::ident: return e->constant ? ex(e->constant->expr) : local_name(e->text);
      case ExprKind::run:
        // Evaluation must replace every #run with a target-independent literal.
        fputs("coax: internal error: unevaluated #run reached the C emitter\n", stderr);
        abort();
      case ExprKind::call: {
        if (e->byte_intrinsic != ByteIntrinsic::none) {
          if (e->byte_intrinsic == ByteIntrinsic::bytes_overlap) {
            std::string a=new_temp(c_type(e->args[0]->type)),b=new_temp(c_type(e->args[1]->type));
            return "("+a+" = "+ex(e->args[0])+", "+b+" = "+ex(e->args[1])+", coax_bytes_overlap("+a+".data, "+a+".count, "+b+".data, "+b+".count))";
          }
          if (e->byte_intrinsic == ByteIntrinsic::mul_wide) {
            wide=true;
            std::string a=new_temp("uint64_t"),b=new_temp("uint64_t");
            return "("+a+" = "+ex(e->args[0])+", "+b+" = "+ex(e->args[1])+", coax_mul_wide("+a+", "+b+"))";
          }
          if (e->byte_intrinsic >= ByteIntrinsic::add_wrap) {
            wrapping=true;
            std::string a=new_temp(c_type(e->type)),b=new_temp(c_type(e->type));
            return "("+a+" = "+ex(e->args[0])+", "+b+" = "+ex(e->args[1])+", coax_"+to_std(e->text)+(e->type==&T_U32?"_u32(":"_u64(")+a+", "+b+"))";
          }
          if (e->byte_intrinsic == ByteIntrinsic::value_barrier) barrier = true;
          if (e->byte_intrinsic == ByteIntrinsic::value_barrier)
            return std::string(e->type == &T_U32 ? "coax_value_barrier_u32(" : "coax_value_barrier_u64(") + ex(e->args[0]) + ")";
          return "coax_"+to_std(e->text)+"("+ex(e->args[0])+")";
        }
        if (opts.cuda && g_warp.mma_calls.count(e)) {
          // One warp-wide tensor-core multiply-add on the lanes' fragment registers.
          std::string a = local_name(e->args[0]->lhs->text), b = local_name(e->args[1]->lhs->text), c = local_name(e->args[2]->lhs->text);
          return "asm volatile(\"mma.sync.aligned.m16n8k16.row.col.f32.bf16.bf16.f32 {%0,%1,%2,%3}, {%4,%5,%6,%7}, {%8,%9}, {%0,%1,%2,%3};\" : \"+f\"(" +
                 c + "[0]), \"+f\"(" + c + "[1]), \"+f\"(" + c + "[2]), \"+f\"(" + c + "[3]) : \"r\"(" + a + "[0]), \"r\"(" + a + "[1]), \"r\"(" + a +
                 "[2]), \"r\"(" + a + "[3]), \"r\"(" + b + "[0]), \"r\"(" + b + "[1]))";
        }
        if (opts.metal_compute && g_warp.mma_calls.count(e)) {
          // The simdgroup's 16x8x16 product, lane by lane (coax_mma_bf16_16x8x16 in the runtime).
          // coax_mc keeps the A fragment's gathered rows between calls on the same A.
          static const std::string cache = "coax_mma_cache coax_mc = {};";
          if (std::find(temps.begin(), temps.end(), cache) == temps.end()) temps.push_back(cache);
          return "coax_mma_bf16_16x8x16(" + local_name(e->args[0]->lhs->text) + ", " + local_name(e->args[1]->lhs->text) + ", " +
                 local_name(e->args[2]->lhs->text) + ", coax_lane, coax_mc)";
        }
        if (opts.metal_compute && g_warp.apple_calls.count(e)) {
          int kind = g_warp.apple_calls.at(e);
          auto whole = [&](int i) { return local_name(e->args[i]->lhs->text); };
          if (kind == 1) return "coax_am_sg_mma(" + whole(0) + ", " + whole(1) + ".e, " + whole(2) + ".e, coax_lane)";
          if (kind == 2) return "(" + group_barrier().substr(0, group_barrier().size() - 1) + ", coax_am_sg_spill(" + whole(0) + ", " + whole(1) + ".e, coax_lane), " +
                                group_barrier().substr(0, group_barrier().size() - 1) + ")";
          apple_tensor = true;
          auto value = [&](int i) { return std::to_string(constant_of_expr(e->args[i])); };
          std::string s = "coax_am_matmul<" + value(1) + ", " + value(2) + ", " + std::to_string(g_warp.group_kernels.at(current) / 32) + ">(";
          for (int i = 3; i < 11; i++) s += ex(e->args[i]) + ", ";
          return s + line + ", coax_err)";
        }
        std::vector<Operand> ops;
        for (const Expr *a : e->args) ops.push_back({a, false});
        std::string name = e->intrinsic != Intrinsic::none ? math_name(e) : proc_name(e->proc);
        bool hidden = opts.metal_compute && e->intrinsic == Intrinsic::none;  // the lane and the error word
        return ordered(ops, [&](const std::vector<std::string> &t) {
          std::string s = name + "(";
          for (size_t i = 0; i < t.size(); i++) s += (i ? ", " : "") + t[i];
          if (hidden) s += std::string(t.empty() ? "" : ", ") + "coax_lane, coax_err";
          return s + ")";
        });
      }
      case ExprKind::unary:
        if (e->op == Tok::bang) return "(!" + ex(e->lhs) + ")";
        if (is_float(e->type)) return "(-" + ex(e->lhs) + ")";
        if (opts.release) return "coax_wneg_" + int_tag(e->type) + "(" + ex(e->lhs) + ")";
        return "coax_neg_" + int_tag(e->type) + "(" + ex(e->lhs) + ", " + line + ")";
      case ExprKind::binary: return binary(e);
      case ExprKind::field:
        if (e->field_index == -2) {
          Type lt = e->lhs->type;
          if (lt->kind == TypeKind::array) {
            std::string n = "INT64_C(" + std::to_string(lt->length) + ")";
            return has_call(e->lhs) ? "((void)(" + ex(e->lhs) + "), " + n + ")" : n;
          }
          return "(" + ex(e->lhs) + ").count";
        }
        return "(" + ex(e->lhs) + ")." + to_std(e->text);
      case ExprKind::index: {
        if (device()) {
          auto th = g_warp.thread_index.find(e);
          // This thread's own element: index - thread*k, which folds to the offset.
          if (th != g_warp.thread_index.end())
            return local_name(e->lhs->text) + "[(" + ex(e->rhs) + ") - l_thread * " + std::to_string(th->second) + "]";
          auto at = g_warp.lane_index.find(e);
          // The element is this lane's own: index - lane*k, which folds to the offset.
          if (at != g_warp.lane_index.end())
            return local_name(e->lhs->text) + "[(" + ex(e->rhs) + ") - l_lane * " + std::to_string(at->second) + "]";
        }
        Type lt = e->lhs->type;
        if (lt == &T_STRING)
          return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &t) {
            return "coax_byte(" + t[0] + ", " + t[1] + ", " + line + ")";
          });
        if (lt->kind == TypeKind::view)
          return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &t) {
            return "(*coax_elem_" + tag(lt) + "(" + t[0] + ", " + t[1] + ", " + line + "))";
          }, true);
        return ordered({{e->lhs, is_place(e->lhs)}, {e->rhs, false}}, [&](const std::vector<std::string> &t) {
          return "(" + t[0] + ").e[coax_index(" + t[1] + ", " + std::to_string(lt->length) + ", " + line + ")]";
        }, is_place(e));
      }
      case ExprKind::slice: {
        Type lt = e->lhs->type;
        std::vector<Operand> ops = {{e->lhs, lt->kind == TypeKind::array}};
        if (e->rhs) { ops.push_back({e->rhs, false}); ops.push_back({e->third, false}); }
        return ordered(ops, [&](const std::vector<std::string> &t) {
          std::string lo = e->rhs ? "(int64_t)(" + t[1] + ")" : "0", hi;
          if (lt == &T_STRING) {
            hi = e->rhs ? "(int64_t)(" + t[2] + ")" : "-1";
            return e->rhs ? "coax_substr(" + t[0] + ", " + lo + ", " + hi + ", " + line + ")" : t[0];
          }
          if (lt->kind == TypeKind::view) {
            if (!e->rhs) return t[0];
            return "coax_reslice_" + tag(lt) + "(" + t[0] + ", " + lo + ", (int64_t)(" + t[2] + "), " + line + ")";
          }
          std::string n = std::to_string(lt->length);
          hi = e->rhs ? "(int64_t)(" + t[2] + ")" : n;
          return "coax_mkview_" + tag(e->type) + "((" + t[0] + ").e, " + n + ", " + lo + ", " + hi + ", " + line + ")";
        });
      }
      case ExprKind::struct_lit: {
        const StructDecl *d = e->type->struct_decl;
        std::vector<Operand> ops;
        for (const FieldInit &fi : e->fields) ops.push_back({fi.value, false});
        return ordered(ops, [&](const std::vector<std::string> &t) {
          std::string s = "((" + c_type(e->type) + "){";
          bool first = true;
          for (int32_t k = 0; k < d->fields.count; k++) {
            const Field &f = d->fields[k];
            std::string value;
            for (int32_t i = 0; i < e->fields.count; i++)
              if (e->fields[i].field == k) value = t[(size_t)i];
            if (value.empty()) {
              if (f.init) value = ex(f.init);
              else if (needs_init(f.type)) value = zero_value(f.type);
              else continue;  // a compound literal zeroes what it does not name
            }
            s += (first ? "." : ", .") + to_std(f.name) + " = " + value;
            first = false;
          }
          if (first) s += "0";
          return s + "})";
        });
      }
      case ExprKind::array_lit: {
        std::vector<Operand> ops;
        for (const Expr *a : e->args) ops.push_back({a, false});
        return ordered(ops, [&](const std::vector<std::string> &t) {
          std::string s = device() ? c_type(e->type) + "{{" : "((" + c_type(e->type) + "){{";
          for (size_t i = 0; i < t.size(); i++) s += (i ? ", " : "") + t[i];
          return s + (device() ? "}}" : "}})");
        });
      }
      case ExprKind::cast: return cast(e);
      case ExprKind::ifx: return "(" + ex(e->lhs) + " ? " + ex(e->rhs) + " : " + ex(e->third) + ")";
    }
    return "0";
  }

  std::string binary(const Expr *e) {
    Type t = e->lhs->type;
    std::string line = std::to_string(e->loc.line);
    if (is_int(t)) {
      switch (e->op) {
        case Tok::plus: return arith("add", e);
        case Tok::minus: return arith("sub", e);
        case Tok::star: return arith("mul", e);
        case Tok::slash: return arith("div", e);
        case Tok::percent: return arith("mod", e);
        case Tok::shl: case Tok::shr: {
          std::string fn = std::string(e->op == Tok::shl ? "coax_shl_" : "coax_shr_") + int_tag(t);
          return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &x) {
            return fn + "(" + x[0] + ", (int64_t)(" + x[1] + "), " + line + ")";
          });
        }
        default: break;
      }
    }
    if (t == &T_STRING && (e->op == Tok::eqeq || e->op == Tok::noteq))
      return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &x) {
        return std::string(e->op == Tok::noteq ? "(!" : "(") + "coax_string_eq(" + x[0] + ", " + x[1] + "))";
      });
    const char *op = "";
    switch (e->op) {
      case Tok::plus: op = " + "; break;
      case Tok::minus: op = " - "; break;
      case Tok::star: op = " * "; break;
      case Tok::slash: op = " / "; break;
      case Tok::amp: op = " & "; break;
      case Tok::pipe: op = " | "; break;
      case Tok::caret: op = " ^ "; break;
      case Tok::eqeq: op = " == "; break;
      case Tok::noteq: op = " != "; break;
      case Tok::lt: op = " < "; break;
      case Tok::le: op = " <= "; break;
      case Tok::gt: op = " > "; break;
      case Tok::ge: op = " >= "; break;
      case Tok::andand: op = " && "; break;
      case Tok::oror: op = " || "; break;
      default: break;
    }
    if (e->op == Tok::andand || e->op == Tok::oror) return "(" + ex(e->lhs) + op + ex(e->rhs) + ")";
    bool bits = e->op == Tok::amp || e->op == Tok::pipe || e->op == Tok::caret;
    return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &x) {
      std::string s = "(" + x[0] + op + x[1] + ")";
      return bits ? "((" + c_type(e->type) + ")" + s + ")" : s;
    });
  }

  // ---- print: split the checked format at compile time ----

  std::string show_call(Type t, const std::string &v, bool top) {
    switch (t->kind) {
      case TypeKind::integer: return t->is_signed ? "coax_print_int(" + v + ")" : "coax_print_uint(" + v + ")";
      case TypeKind::floating: return "coax_print_float(" + v + (t == &T_F32 ? ", 1)" : ", 0)");
      case TypeKind::bool_: return "coax_print_bool(" + v + ")";
      case TypeKind::string_: return top ? "coax_put(" + v + ")" : "coax_show_string(" + v + ")";
      case TypeKind::enum_: return "coax_show_" + tag(t) + "(" + v + ")";
      default: return "coax_show_" + tag(t) + "(&" + v + ")";
    }
  }

  void print(const Expr *e) {
    Str fmt = e->args[0]->text;
    std::vector<std::string> values;
    bool all_trivial = true;
    for (int32_t i = 1; i < e->args.count; i++) all_trivial = all_trivial && trivial(e->args[i]);
    line_start();
    out += "{";
    for (int32_t i = 1; i < e->args.count; i++) {
      const Expr *a = e->args[i];
      bool aggregate = is_aggregate(a->type) || a->type->kind == TypeKind::view;
      if ((all_trivial && !aggregate) || (all_trivial && is_place(a))) {
        values.push_back(ex(a));
      } else {
        // Every value is computed, in order, before anything is written.
        std::string t = "coax_a" + std::to_string(i);
        out += " " + c_type(a->type) + " " + t + " = " + ex(a) + ";";
        values.push_back(t);
      }
    }
    int32_t run = 0;
    size_t next = 0;
    auto flush = [&](int32_t upto) {
      if (upto > run) out += " coax_put(COAX_STR(" + c_string_text(Str{fmt.data + run, upto - run}) + ", " +
                             std::to_string(upto - run) + "));";
    };
    for (int32_t i = 0; i < fmt.count; i++) {
      if (fmt.data[i] != '%') continue;
      if (i + 1 < fmt.count && fmt.data[i + 1] == '%') {
        flush(i + 1);  // keep one '%'
        i++;
        run = i + 1;
        continue;
      }
      flush(i);
      run = i + 1;
      const Expr *a = e->args[(int32_t)next + 1];
      out += " " + show_call(a->type, values[next], true) + ";";
      next++;
    }
    flush(fmt.count);
    out += " }\n";
  }

  // ---- statements ----

  void emit_defers_from(size_t depth) {
    for (size_t s = defers.size(); s-- > depth;)
      for (size_t i = defers[s].size(); i-- > 0;) stmt(defers[s][i]);
  }

  bool any_defers_from(size_t depth) {
    for (size_t s = depth; s < defers.size(); s++)
      if (!defers[s].empty()) return true;
    return false;
  }

  void block_body(const Stmt *s, const std::vector<std::string> &prelude = {}) {
    line("{");
    indent++;
    for (const std::string &p : prelude) line(p);
    defers.push_back({});
    for (const Stmt *c : s->body) {
      if (c->kind == StmtKind::defer) defers.back().push_back(c->then_);
      else stmt(c);
    }
    // Normal exit: cleanup in reverse registration order.
    std::vector<const Stmt *> mine = defers.back();
    defers.pop_back();
    for (size_t i = mine.size(); i-- > 0;) stmt(mine[i]);
    indent--;
    line("}");
  }

  static Tok compound_op(Tok op) {
    switch (op) {
      case Tok::plus_equal: return Tok::plus;
      case Tok::minus_equal: return Tok::minus;
      case Tok::star_equal: return Tok::star;
      case Tok::slash_equal: return Tok::slash;
      case Tok::percent_equal: return Tok::percent;
      case Tok::amp_equal: return Tok::amp;
      case Tok::pipe_equal: return Tok::pipe;
      case Tok::caret_equal: return Tok::caret;
      case Tok::shl_equal: return Tok::shl;
      case Tok::shr_equal: return Tok::shr;
      default: return Tok::eof;
    }
  }

  void assign(const Stmt *s) {
    const Expr *target = s->target;
    if (s->op == Tok::equal) {
      bool order = (has_call(target) && !trivial(s->expr)) || (has_call(s->expr) && !trivial(target));
      if (!order) {
        line(ex(target) + " = " + ex(s->expr) + ";");
      } else {
        // The place first, then the value (spec §7.3).
        std::string p = new_temp(pointer_type(target));
        line(p + " = &(" + ex(target) + ");");
        line("*" + p + " = " + ex(s->expr) + ";");
      }
      return;
    }
    // x op= v: the place is evaluated once, then v, then the update.
    Expr place{}, value{}, bin{};
    bool direct = !has_call(target) && !has_call(s->expr);
    std::string p, v;
    if (!direct) {
      p = new_temp(pointer_type(target));
      v = new_temp(c_type(s->expr->type));
      line(p + " = &(" + ex(target) + ");");
      line(v + " = " + ex(s->expr) + ";");
    }
    // Build `place op value` as a binary node over stand-in identifiers.
    place.kind = ExprKind::ident;
    place.type = target->type;
    value.kind = ExprKind::ident;
    value.type = s->expr->type;
    bin.kind = ExprKind::binary;
    bin.loc = s->loc;
    bin.type = target->type;
    bin.op = compound_op(s->op);
    std::string target_text = direct ? ex(target) : "(*" + p + ")";
    std::string marker_a = "\x01", marker_b = "\x02";
    place.text = Str{marker_a.c_str(), 1};
    value.text = Str{marker_b.c_str(), 1};
    bin.lhs = &place;
    bin.rhs = direct ? s->expr : &value;
    std::string text = binary(&bin);
    // Replace the stand-ins with the real C text.
    std::string a = local_name(place.text), b = local_name(value.text);
    for (size_t at; (at = text.find(a)) != std::string::npos;) text.replace(at, a.size(), target_text);
    for (size_t at; (at = text.find(b)) != std::string::npos;) text.replace(at, b.size(), v);
    line(target_text + " = " + text + ";");
  }

  void stmt(const Stmt *s) {
    struct Restore {Loc &target;Loc old;~Restore(){target=old;}} restore{debug_location,debug_location};
    debug_location=s->loc;
    switch (s->kind) {
      case StmtKind::block: block_body(s); break;
      case StmtKind::expr:
        if (s->expr->kind == ExprKind::call && s->expr->builtin_print) print(s->expr);
        else line(ex(s->expr) + ";");
        break;
      case StmtKind::var: {
        if (device() && g_warp.lane_arrays.count(s)) {
          // Lane l keeps elements l*k .. l*k+k-1: this lane's k values.
          line(c_type(s->type->elem) + " " + local_name(s->name) + "[" + std::to_string(s->type->length / 32) + "] = {};");
          break;
        }
        if (device() && g_warp.thread_arrays.count(s)) {
          // Thread t keeps elements t*k .. t*k+k-1 in registers.
          line(c_type(s->type->elem) + " " + local_name(s->name) + "[" + std::to_string(s->type->length / g_warp.thread_arrays.at(s)) + "] = {};");
          break;
        }
        if (device() && g_warp.shared_decls.count(s)) {
          // Threadgroup memory: the kernel entry owns it; zeroed by the whole group.
          std::string n = local_name(s->name), c = c_type(s->type), T = std::to_string(g_warp.group_kernels.at(current));
          if (opts.metal_compute) line("threadgroup " + c + " &" + n + " = *coax_tg_" + n + ";");
          else { line("__shared__ " + c + " coax_tg_" + n + ";"); line(c + " &" + n + " = coax_tg_" + n + ";"); }
          line("for (int64_t coax_z = " + std::string(opts.metal_compute ? "(int64_t)coax_lane" : "(int64_t)threadIdx.x") + "; coax_z < " +
               std::to_string(s->type->length) + "; coax_z += " + T + ") " + n + ".e[coax_z] = 0;");
          line(group_barrier());
          break;
        }
        if (device() && g_warp.accumulators.count(s)) {
          // A matrix accumulator: each simdgroup's 8x8 tiles of it, in its own registers.
          std::string n = local_name(s->name);
          line("simdgroup_float8x8 " + n + "[8];");
          line("for (int coax_z = 0; coax_z < 8; coax_z++) " + n + "[coax_z] = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);");
          break;
        }
        std::string decl = c_type(s->type) + " " + local_name(s->name);
        if (s->expr) decl += " = " + ex(s->expr);
        else if (!s->uninit) decl += " = " + zero_value(s->type);  // zero or defaults, unless `---`
        line(decl + ";");
        break;
      }
      case StmtKind::assign: assign(s); break;
      case StmtKind::ret:
        if (!any_defers_from(0)) {
          line(s->expr ? "return " + ex(s->expr) + ";" : "return;");
          break;
        }
        // The value first, then cleanup, then the return.
        if (s->expr) line("coax_ret = " + ex(s->expr) + ";");
        ret_temp = ret_temp || s->expr;
        emit_defers_from(0);
        if (opts.debug_info) out+=debug_line(file,s->loc);
        line(s->expr ? "return coax_ret;" : "return;");
        break;
      case StmtKind::break_:
      case StmtKind::continue_:
        emit_defers_from(loop_scopes.back());
        line(s->kind == StmtKind::break_ ? "break;" : "continue;");
        break;
      case StmtKind::defer: break;  // registered by the enclosing block
      case StmtKind::assert_:  // checked in every build, like bounds and casts
        line("if (!" + condition(s->expr) + ") coax_trap(\"assertion failed\", " + std::to_string(s->loc.line) + ");");
        break;
      case StmtKind::if_:
        line("if " + condition(s->expr));
        stmt(s->then_);
        if (s->else_) {
          line("else");
          stmt(s->else_);
        }
        break;
      case StmtKind::while_:
        line("while " + condition(s->expr));
        loop(s->then_);
        break;
      case StmtKind::for_range: {
        if (device() && g_warp.lane_loops.count(s)) {
          // Each lane runs the body once, as its own `lane`.
          loop(s->then_, {"const int64_t " + local_name(s->name) + (opts.metal_compute ? " = (int64_t)coax_lane;" : " = (int64_t)(threadIdx.x & 31u);")});
          break;
        }
        if (device() && g_warp.thread_loops.count(s)) {
          // A phase: every thread of the group runs the body as its own
          // `thread`, between two barriers.
          line(group_barrier());
          block_body(s->then_, {"const int64_t " + local_name(s->name) + (opts.metal_compute ? " = (int64_t)coax_lane;" : " = (int64_t)threadIdx.x;")});
          line(group_barrier());
          break;
        }
        std::string t = c_type(s->type), n = std::to_string(temp_count++);
        std::string i = local_name(s->name);
        line("{");
        indent++;
        line(t + " coax_lo" + n + " = " + ex(s->expr) + ";");
        line(t + " coax_hi" + n + " = " + ex(s->expr2) + ";");
        // CUDA: a loop over constant bounds with at most 128 steps is unrolled,
        // so fixed arrays indexed by its counter stay in registers.
        auto literal = [](const Expr *x) -> const Expr * {
          if (x->kind == ExprKind::ident && x->constant && x->constant->expr) x = x->constant->expr;
          return x->kind == ExprKind::int_lit ? x : nullptr;
        };
        const Expr *lo = literal(s->expr), *hi = literal(s->expr2);
        if (device() && lo && hi && (int64_t)hi->int_value - (int64_t)lo->int_value <= 128) line("#pragma unroll");
        line("for (" + t + " " + i + " = coax_lo" + n + "; " + i + " < coax_hi" + n + "; " + i + "++)");
        loop(s->then_);
        indent--;
        line("}");
        break;
      }
      case StmtKind::for_each: {
        Type ct = s->expr->type;
        std::string n = std::to_string(temp_count++);
        std::string c = "coax_c" + n, i = "coax_i" + n, count, element;
        line("{");
        indent++;
        if (ct->kind == TypeKind::array) {
          // Iterate in place: writes by the body are seen by later iterations.
          if (is_place(s->expr)) line(c_type(ct) + " *" + c + " = &(" + ex(s->expr) + ");");
          else {
            line(c_type(ct) + " " + c + "_value = " + ex(s->expr) + ";");
            line(c_type(ct) + " *" + c + " = &" + c + "_value;");
          }
          count = std::to_string(ct->length);
          element = c + "->e[" + i + "]";
        } else {
          line(c_type(ct) + " " + c + " = " + ex(s->expr) + ";");
          count = c + ".count";
          element = ct == &T_STRING ? "(uint8_t)" + c + ".data[" + i + "]" : c + ".data[" + i + "]";
        }
        line("for (int64_t " + i + " = 0; " + i + " < " + count + "; " + i + "++)");
        std::vector<std::string> prelude = {c_type(s->type) + " " + local_name(s->name) + " = " + element + ";"};
        if (s->name2.count) prelude.push_back("int64_t " + local_name(s->name2) + " = " + i + ";");
        loop(s->then_, prelude);
        indent--;
        line("}");
        break;
      }
    }
  }

  void loop(const Stmt *body, const std::vector<std::string> &prelude = {}) {
    loop_scopes.push_back(defers.size());
    block_body(body, prelude);
    loop_scopes.pop_back();
  }

  // ---- declarations ----

  std::string prototype(const ProcDecl *p, bool header = false) {
    std::string s = (opts.cuda ? "static __device__ " : opts.metal_compute ? "static inline " : header || p->export_c ? "" : p->foreign ? "extern " : "static ") + c_type(p->ret) + " " +
                    proc_name(p) + "(";
    if (p->params.count == 0 && !opts.metal_compute) s += "void";
    for (int32_t i = 0; i < p->params.count; i++) {
      if (i) s += ", ";
      s += c_type(p->params[i].type);
      if (!header) s += " " + local_name(p->params[i].name);
      else if (!is_c_keyword(p->params[i].name)) s += " " + to_std(p->params[i].name);
    }
    // Metal: every procedure carries the simdgroup lane and the launch's error words.
    if (opts.metal_compute) s += std::string(p->params.count ? ", " : "") + "uint coax_lane, thread uint *coax_err";
    // A group kernel's threadgroup arrays live in its entry, which passes them in.
    if (opts.metal_compute && g_warp.shared_arrays.count(p))
      for (const Stmt *d : g_warp.shared_arrays.at(p)) s += ", threadgroup " + c_type(d->type) + " *coax_tg_" + local_name(d->name);
    return s + ")";
  }

  std::string function(const ProcDecl *p) {
    if (p->foreign) return "";
    current = p;
    mapping_active=true;debug_location=p->loc;
    out.clear();
    temps.clear();
    ret_temp = false;
    indent = 1;
    std::string saved;
    // Emit the body's statements, then wrap them with the temporaries.
    defers.push_back({});
    for (const Stmt *c : p->body->body) {
      if (c->kind == StmtKind::defer) defers.back().push_back(c->then_);
      else stmt(c);
    }
    std::vector<const Stmt *> mine = defers.back();
    defers.pop_back();
    for (size_t i = mine.size(); i-- > 0;) stmt(mine[i]);
    std::string body = out;mapping_active=false;
    std::string s = "\n" + (opts.debug_info ? debug_line(file,p->loc) : "") + prototype(p) + " {\n";
    if (ret_temp) s += "  " + c_type(p->ret) + " coax_ret;\n";
    for (const std::string &t : temps) s += "  " + t + "\n";
    s += body + "}\n";
    if (opts.debug_info) s+="#line 1 \"<coax-generated>\"\n";
    return s;
  }
};

// ---- types: declarations in dependency order ----

struct TypeWriter {
  std::string out;
  std::vector<Type> done;
  bool header = false;

  bool seen(Type t) {
    for (Type d : done) if (d == t) return true;
    return false;
  }

  void need(Type t) {
    if (seen(t)) return;
    switch (t->kind) {
      case TypeKind::struct_: {
        done.push_back(t);
        const StructDecl *d = t->struct_decl;
        for (const Field &f : d->fields) if (f.type->kind != TypeKind::view) need(f.type);
        for (const Field &f : d->fields) if (f.type->kind == TypeKind::view) need(f.type);
        out += "struct coax_" + tag(t) + " {";
        for (const Field &f : d->fields) out += " " + c_type(f.type) + " " + to_std(f.name) + ";";
        if (!d->fields.count) out += " char coax_empty;";
        out += " };\n";
        break;
      }
      case TypeKind::array:
        need(t->elem);
        done.push_back(t);
        out += "typedef struct { " + c_type(t->elem) + " e[" + std::to_string(t->length) + "]; } " + c_type(t) + ";\n";
        break;
      case TypeKind::view:
        done.push_back(t);
        // C needs only the element's name for a pointer; a header still
        // declares the element so the host can use it.
        if (t->elem->kind != TypeKind::struct_ || header) need(t->elem);
        {
          // A view of a scalar is the same in every library, so headers
          // from two libraries can be included together.
          std::string def = "typedef struct { " + c_type(t->elem) + " *data; int64_t count; } " + c_type(t) + ";\n";
          TypeKind k = t->elem->kind;
          bool shared = k == TypeKind::integer || k == TypeKind::floating || k == TypeKind::bool_;
          if (shared) {
            std::string guard = "COAX_" + c_type(t) + "_DEFINED";
            def = "#ifndef " + guard + "\n#define " + guard + "\n" + def + "#endif\n";
          }
          out += def;
        }
        break;
      case TypeKind::enum_:
        done.push_back(t);
        out += "typedef " + c_type(t->elem) + " " + c_type(t) + ";\n";
        for (const EnumMember &m : t->enum_decl->members)
          out += "#define COAX_" + to_std(t->enum_decl->name) + "_" + to_std(m.name) + " " + int_text(t, m.bits) + "\n";
        break;
      default: break;
    }
  }

  void forward_structs(const File *file) {
    for (const StructDecl *d : file->structs)
      out += "typedef struct coax_" + tag(d->type) + " coax_" + tag(d->type) + ";\n";
  }

  // The C compiler checks that Coax's layout is the C layout.
  void layout_asserts(const File *file) {
    for (const StructDecl *d : file->structs) {
      std::string c = c_type(d->type);
      out += "COAX_LAYOUT(sizeof(" + c + ") == " + std::to_string(d->type->size) + " && _Alignof(" + c + ") == " +
             std::to_string(d->type->align) + ", \"layout of " + to_std(d->name) + "\");\n";
      for (const Field &f : d->fields)
        out += "COAX_LAYOUT(offsetof(" + c + ", " + to_std(f.name) + ") == " + std::to_string(f.offset) +
               ", \"offset of " + to_std(d->name) + "." + to_std(f.name) + "\");\n";
    }
  }
};

void collect_types(const File *file, std::vector<Type> *types) {
  auto add = [&](Type t) {
    for (Type x : *types) if (x == t) return;
    types->push_back(t);
  };
  for (const StructDecl *d : file->structs) add(d->type);
  for (const EnumDecl *d : file->enums) add(d->type);
  for (Type t : all_composite_types()) add(t);
}

std::string show_functions(const File *file, Emitter &em) {
  std::vector<Type> types;
  collect_types(file, &types);
  std::string protos, bodies;
  for (Type t : types) {
    std::string c = c_type(t), tg = tag(t);
    if (t->kind == TypeKind::enum_) {
      protos += "static void coax_show_" + tg + "(" + c + " v);\n";
      bodies += "static void coax_show_" + tg + "(" + c + " v) {\n  switch (v) {\n";
      for (const EnumMember &m : t->enum_decl->members)
        bodies += "    case " + int_text(t, m.bits) + ": fputs(\"" + to_std(m.name) + "\", stdout); return;\n";
      bodies += "    default: break;\n  }\n  fputs(\"" + to_std(t->enum_decl->name) + "(\", stdout); " +
                (t->elem->is_signed ? "coax_print_int((int64_t)v);" : "coax_print_uint((uint64_t)v);") +
                " fputc(')', stdout);\n}\n";
      // Checked conversion from an integer: it must name a member.
      bodies += "static " + c + " coax_to_" + tg + "(int64_t s, uint64_t u, int is_signed, int32_t l) {\n";
      for (const EnumMember &m : t->enum_decl->members) {
        bool negative = t->elem->is_signed && (int64_t)m.bits < 0;
        if (negative) bodies += "  if (is_signed && s == INT64_C(" + std::to_string((int64_t)m.bits) + ")) return " + int_text(t, m.bits) + ";\n";
        else bodies += "  if (is_signed ? (s >= 0 && (uint64_t)s == UINT64_C(" + std::to_string(m.bits) + ")) : u == UINT64_C(" +
                       std::to_string(m.bits) + ")) return " + int_text(t, m.bits) + ";\n";
      }
      bodies += "  coax_trap(\"value is not a member of enum " + to_std(t->enum_decl->name) + "\", l);\n  return 0;\n}\n";
      protos += "static " + c + " coax_to_" + tg + "(int64_t s, uint64_t u, int is_signed, int32_t l);\n";
      continue;
    }
    protos += "static void coax_show_" + tg + "(const " + c + " *v);\n";
    bodies += "static void coax_show_" + tg + "(const " + c + " *v) {\n";
    if (t->kind == TypeKind::struct_) {
      const StructDecl *d = t->struct_decl;
      bodies += "  fputs(\"" + to_std(d->name) + ".{\", stdout);\n";
      for (int32_t i = 0; i < d->fields.count; i++) {
        const Field &f = d->fields[i];
        bodies += "  fputs(\"" + std::string(i ? ", " : "") + to_std(f.name) + " = \", stdout); " +
                  em.show_call(f.type, "v->" + to_std(f.name), false) + ";\n";
      }
      bodies += "  fputc('}', stdout);\n";
    } else {
      std::string n = t->kind == TypeKind::array ? std::to_string(t->length) : "v->count";
      std::string el = t->kind == TypeKind::array ? "v->e[i]" : "v->data[i]";
      bodies += "  fputc('[', stdout);\n  for (int64_t i = 0; i < " + n + "; i++) {\n    if (i) fputs(\", \", stdout);\n    " +
                em.show_call(t->elem, el, false) + ";\n  }\n  fputc(']', stdout);\n";
    }
    bodies += "}\n";
  }
  return protos + bodies;
}

std::string view_helpers() {
  std::string s;
  for (Type t : all_composite_types()) {
    if (t->kind != TypeKind::view) continue;
    std::string c = c_type(t), e = c_type(t->elem), tg = tag(t);
    if (t->elem == &T_U8)
      s += "static coax_string coax_string_of_bytes(" + c + " v) { coax_string s = {(const char *)v.data, v.count}; return s; }\n";
    s += "static " + e + " *coax_elem_" + tg + "(" + c + " v, int64_t i, int32_t l) { return &v.data[coax_index(i, v.count, l)]; }\n";
    s += "static " + c + " coax_mkview_" + tg + "(" + e + " *data, int64_t n, int64_t lo, int64_t hi, int32_t l) {\n"
         "  coax_slice_check(lo, hi, n, l);\n  " + c + " r = {data + lo, hi - lo};\n  return r;\n}\n";
    s += "static " + c + " coax_reslice_" + tg + "(" + c + " v, int64_t lo, int64_t hi, int32_t l) {\n"
         "  coax_slice_check(lo, hi, v.count, l);\n  " + c + " r = {v.data + lo, hi - lo};\n  return r;\n}\n";
  }
  return s;
}

std::string init_functions(const File *file, Emitter &em) {
  std::vector<Type> types;
  collect_types(file, &types);
  std::string protos, bodies;
  for (Type t : types) {
    if (!needs_init(t)) continue;
    std::string c = c_type(t), tg = tag(t);
    protos += "static " + c + " coax_init_" + tg + "(void);\n";
    bodies += "static " + c + " coax_init_" + tg + "(void) {\n  " + c + " v;\n  memset(&v, 0, sizeof v);\n";
    if (t->kind == TypeKind::struct_) {
      for (const Field &f : t->struct_decl->fields) {
        if (f.init) bodies += "  v." + to_std(f.name) + " = " + em.ex(f.init) + ";\n";
        else if (needs_init(f.type)) bodies += "  v." + to_std(f.name) + " = coax_init_" + tag(f.type) + "();\n";
      }
    } else {
      bodies += "  for (int64_t i = 0; i < " + std::to_string(t->length) + "; i++) v.e[i] = coax_init_" + tag(t->elem) + "();\n";
    }
    bodies += "  return v;\n}\n";
  }
  return protos + bodies;
}

std::string escape_path(const std::string &path) {
  std::string s;
  for (unsigned char c : path) {
    if (c < 0x20 || c >= 0x7f) {
      char octal[5];snprintf(octal,sizeof octal,"\\%03o",c);s+=octal;
    } else {
      if (c == '"' || c == '\\') s += '\\';
      s += (char)c;
    }
  }
  return s;
}

}  // namespace

// Source-unit provenance is emitted only for composed programs, preserving
// standalone seed bytes. Runtime helpers receive joined line numbers.
static std::string source_runtime(const File *file) {
  if (file->src->units.size()<2) return RUNTIME;
  std::string out="struct coax_source_range { int first, last, base, fixed; const char *path; };\n";
  out+="static const struct coax_source_range coax_source_ranges[] = {\n";
  auto range=[&](int first,int last,int base,bool fixed,const std::string &path) {
    out+="  {"+std::to_string(first)+","+std::to_string(last)+","+std::to_string(base)+","+(fixed?"1":"0")+",\""+escape_path(path)+"\"},\n";
  };
  for (const auto &r:file->src->generated) {
    int first=1+(int)std::count(file->src->text.begin(),file->src->text.begin()+r.start,'\n');
    int last=1+(int)std::count(file->src->text.begin(),file->src->text.begin()+r.end,'\n');
    auto point=source_point(file->src,r.directive);range(first,last,point.loc.line,true,point.path);
  }
  for (const auto &unit:file->src->units) {
    int count=(int)std::count(file->src->text.begin()+unit->begin,file->src->text.begin()+unit->end,'\n');
    range(unit->first_line,unit->first_line+std::max(1,count)-1,1,false,unit->path);
  }
  out+="};\nstatic const struct coax_source_range *coax_find_source(int line) {\n";
  out+="  for (unsigned i=0;i<sizeof(coax_source_ranges)/sizeof(coax_source_ranges[0]);i++) if (line>=coax_source_ranges[i].first && line<=coax_source_ranges[i].last) return &coax_source_ranges[i];\n  return 0;\n}\n";
  out+="static const char *coax_path_for(int line) { const struct coax_source_range *r=coax_find_source(line); return r?r->path:coax_source_path; }\n";
  out+="static int coax_line_for(int line) { const struct coax_source_range *r=coax_find_source(line); return r?(r->fixed?r->base:r->base+line-r->first):line; }\n";
  std::string runtime=RUNTIME;std::string needle="coax_source_path, line, what";
  runtime.replace(runtime.find(needle),needle.size(),"coax_path_for(line), coax_line_for(line), what");
  return out+runtime;
}

// Procedures a CUDA module contains: the kernels (#export_c) and what they
// call, in declaration order. Checks and their helpers stay on the CPU.
std::vector<const ProcDecl *> cuda_procs(const File *file) {
  std::vector<const ProcDecl *> seen;
  std::function<void(const ProcDecl *)> visit_proc;
  std::function<void(const Expr *)> visit_expr = [&](const Expr *e) {
    if (!e) return;
    if (e->kind == ExprKind::call && e->proc && !is_device_primitive(e->proc)) visit_proc(e->proc);
    visit_expr(e->lhs); visit_expr(e->rhs); visit_expr(e->third);
    for (const Expr *a : e->args) visit_expr(a);
    for (const FieldInit &f : e->fields) visit_expr(f.value);
  };
  std::function<void(const Stmt *)> visit_stmt = [&](const Stmt *st) {
    if (!st) return;
    visit_expr(st->expr); visit_expr(st->expr2); visit_expr(st->target);
    for (const Stmt *c : st->body) visit_stmt(c);
    visit_stmt(st->then_); visit_stmt(st->else_);
  };
  visit_proc = [&](const ProcDecl *p) {
    for (const ProcDecl *q : seen) if (q == p) return;
    seen.push_back(p);
    visit_stmt(p->body);
  };
  for (const ProcDecl *p : file->procs) if (p->export_c && !p->foreign) visit_proc(p);
  std::vector<const ProcDecl *> ordered;
  for (const ProcDecl *p : file->procs)
    for (const ProcDecl *q : seen) if (q == p) { ordered.push_back(p); break; }
  return ordered;
}

// Apple matrix primitives (lib/compute/apple.coax) for `coax metal-compute`.
// The simdgroup tile product: 4 simdgroups of a 128-thread group; simdgroup
// g owns rows 32*(g%2) .. +31 and columns 16*(g/2) .. +15 of the 64x32
// accumulator, as eight 8x8 matrices (rows 8i, columns 8j at 2i+j). a holds
// 64 rows of 32 k, b 32 columns of 32 k; the hardware adds in its own order.
static const char *METAL_SIMDGROUP_RUNTIME = R"COAXRT(
COAX_DEV void coax_am_sg_mma(thread simdgroup_float8x8 *acc, threadgroup const float *a, threadgroup const float *b, uint tid) {
  uint g = tid / 32u; int sr = (int)(g & 1u) * 32, sc = (int)(g >> 1) * 16;
  for (int kk = 0; kk < 32; kk += 8) {
    simdgroup_float8x8 A[4], B[2];
    for (int i = 0; i < 4; i++) simdgroup_load(A[i], a + (sr + 8 * i) * 32 + kk, 32);
    for (int j = 0; j < 2; j++) simdgroup_load(B[j], b + (sc + 8 * j) * 32 + kk, 32, ulong2(0, 0), true);
    for (int i = 0; i < 4; i++)
      for (int j = 0; j < 2; j++) simdgroup_multiply_accumulate(acc[2 * i + j], A[i], B[j], acc[2 * i + j]);
  }
}
COAX_DEV void coax_am_sg_spill(thread simdgroup_float8x8 *acc, threadgroup float *dst, uint tid) {
  uint g = tid / 32u; int sr = (int)(g & 1u) * 32, sc = (int)(g >> 1) * 16;
  for (int i = 0; i < 4; i++)
    for (int j = 0; j < 2; j++) simdgroup_store(acc[2 * i + j], dst + (sr + 8 * i) * 32 + sc + 8 * j, 32);
}
)COAXRT";
// The Metal 4 tensor-op tile (MetalPerformancePrimitives matmul2d, BF16
// weights times f32 activations into f32, relaxed precision off): out[t][r]
// for TM tokens from t0 and TN rows from r0, clipped to tokens and rows. The
// whole group calls it with the same values; a failed check voids the launch.
static const char *METAL_TENSOR_RUNTIME = R"COAXRT(
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
template <int TM, int TN, int SG>
COAX_DEV void coax_am_matmul(int64_t rows, int64_t cols, int64_t tokens, int64_t r0, int64_t t0, coax_v_u32 w, coax_v_f32 x, coax_v_f32 out, int32_t line, COAX_ERR) {
  using namespace mpp::tensor_ops;
  if (rows < 1 || cols < 2 || cols % 2 != 0 || tokens < 1 || r0 < 0 || r0 >= rows || t0 < 0 || t0 >= tokens || rows > 2147483647 / cols ||
      tokens > 2147483647 / cols || tokens > 2147483647 / rows || w.count < rows * (cols / 2) || x.count < tokens * cols || out.count < tokens * rows) {
    coax_trap_(line, coax_err);
    return;
  }
  int c = (int)cols, nt = (int)min((int64_t)TM, tokens - t0), nr = (int)min((int64_t)TN, rows - r0);
  auto X = tensor<device float, dextents<int32_t, 2>, tensor_inline>(x.data + t0 * cols, dextents<int32_t, 2>(c, nt), array<int32_t, 2>({1, c}));
  auto W = tensor<device bfloat, dextents<int32_t, 2>, tensor_inline>((device bfloat *)w.data + r0 * cols, dextents<int32_t, 2>(c, nr), array<int32_t, 2>({1, c}));
  auto C = tensor<device float, dextents<int32_t, 2>, tensor_inline>(out.data + t0 * rows + r0, dextents<int32_t, 2>(nr, nt), array<int32_t, 2>({1, (int)rows}));
  constexpr auto d = matmul2d_descriptor(TM, TN, static_cast<int>(dynamic_extent), false, true, false);
  matmul2d<d, execution_simdgroups<SG>> op;
  op.run(X, W, C);
}
)COAXRT";

static std::string emit_cuda(const File *file, const EmitOptions &opts) {
  Emitter em{file, opts};
  std::vector<const ProcDecl *> procs = cuda_procs(file);
  std::string functions;
  for (const ProcDecl *p : procs) functions += em.function(p);
  // The array and view types those procedures use, in dependency order.
  std::vector<Type> used;
  auto add = [&](Type t) {
    if (t->kind != TypeKind::array && t->kind != TypeKind::view) return;
    for (Type u : used) if (u == t) return;
    used.push_back(t);
  };
  std::function<void(const Expr *)> types_expr = [&](const Expr *e) {
    if (!e) return;
    add(e->type);
    types_expr(e->lhs); types_expr(e->rhs); types_expr(e->third);
    for (const Expr *a : e->args) types_expr(a);
  };
  std::function<void(const Stmt *)> types_stmt = [&](const Stmt *st) {
    if (!st) return;
    add(st->type);
    types_expr(st->expr); types_expr(st->expr2); types_expr(st->target);
    if (st->expr) add(st->expr->type);
    for (const Stmt *c : st->body) types_stmt(c);
    types_stmt(st->then_); types_stmt(st->else_);
  };
  for (const ProcDecl *p : procs) {
    add(p->ret);
    for (const Param &prm : p->params) add(prm.type);
    types_stmt(p->body);
  }
  TypeWriter types;
  for (Type t : all_composite_types())
    for (Type u : used) if (u == t) { types.need(t); break; }
  std::string helpers;
  for (Type t : types.done) {
    if (t->kind != TypeKind::view) continue;
    std::string c = c_type(t), e = c_type(t->elem), tg = tag(t);
    helpers += "COAX_DEV " + e + " *coax_elem_" + tg + "(" + c + " v, int64_t i, int32_t l) { return &v.data[coax_index(i, v.count, l)]; }\n";
    helpers += "COAX_DEV " + c + " coax_mkview_" + tg + "(" + e + " *data, int64_t n, int64_t lo, int64_t hi, int32_t l) {\n"
               "  coax_slice_check(lo, hi, n, l);\n  " + c + " r = {data + lo, hi - lo};\n  return r;\n}\n";
    helpers += "COAX_DEV " + c + " coax_reslice_" + tg + "(" + c + " v, int64_t lo, int64_t hi, int32_t l) {\n"
               "  coax_slice_check(lo, hi, v.count, l);\n  " + c + " r = {v.data + lo, hi - lo};\n  return r;\n}\n";
  }
  std::string s = "/* Generated by coax0 from " + file->src->path + " (coax cuda). Do not edit: change the .coax file. */\n";
  s += CUDA_RUNTIME;
  s += "\n/* Types */\n" + types.out + helpers + "\n";
  for (const ProcDecl *p : procs) s += em.prototype(p) + ";\n";
  s += functions;
  // One entry per kernel: work item i of n runs the procedure once.
  s += "\n/* Kernels: launch with n items; item i calls the procedure with i. */\n";
  for (const ProcDecl *p : procs) {
    if (!p->export_c) continue;
    std::string w = "extern \"C\" __global__ void " + to_std(p->name) + "(int64_t coax_items";
    std::string call = em.proc_name(p) + "(coax_item";
    for (int32_t i = 1; i < p->params.count; i++) {
      w += ", " + c_type(p->params[i].type) + " " + em.local_name(p->params[i].name);
      call += ", " + em.local_name(p->params[i].name);
    }
    if (g_warp.group_kernels.count(p)) {  // one block of T threads per work item; launch with block = T
      std::string T = std::to_string(g_warp.group_kernels.at(p));
      s += w + ") {\n  if (blockDim.x != " + T + "u) __trap();\n  int64_t coax_item = (int64_t)blockIdx.x;\n"
           "  if (coax_item >= coax_items) return;\n  " + call + ");\n}\n";
    } else if (g_warp.warp_kernels.count(p))  // one warp per work item; launch 32 threads per item
      s += w + ") {\n  int64_t coax_item = ((int64_t)blockIdx.x * (int64_t)blockDim.x + (int64_t)threadIdx.x) >> 5;\n"
           "  if (coax_item >= coax_items) return;\n  " + call + ");\n}\n";
    else
      s += w + ") {\n  int64_t coax_item = (int64_t)blockIdx.x * (int64_t)blockDim.x + (int64_t)threadIdx.x;\n"
           "  if (coax_item >= coax_items) return;\n  " + call + ");\n}\n";
  }
  for (size_t at = 0; (at = s.find("_Bool", at)) != std::string::npos;) s.replace(at, 5, "bool");
  return s;
}

// The device runtime of `coax metal-compute`: the checks of RUNTIME spelled
// for Metal Shading Language, the float-float f64 profile (APPLE-FF-v1) and
// the simdgroup MMA. Metal has no trap, so a failed check counts itself in
// the launch's error words (coax_err[0] failures, coax_err[1] the largest
// failing line), takes a harmless value and lets the launch finish; the host
// reads the words at completion and discards the launch's results. Each
// trapping helper takes the error words as a hidden last argument through a
// macro of the same name as its C counterpart.
static const char *METAL_RUNTIME = R"COAXRT(/* Compile with fast math off (MTLMathModeSafe, precise functions): +, -, *, /
   round once each and are never contracted or reassociated, so the
   float-float sums below stay compensated. */
#include <metal_stdlib>
using namespace metal;
#pragma METAL fp math_mode(safe)
#pragma METAL fp contract(off)
#pragma clang diagnostic ignored "-Wunused-function"
/* metal_stdlib defines int8_t .. uint64_t (64-bit long and ulong). */
#define INT64_C(x) x##L
#define UINT64_C(x) x##UL
#define INT64_MIN (-9223372036854775807L - 1)
#define INT64_MAX 9223372036854775807L
#define UINT64_MAX 18446744073709551615UL
#define COAX_DEV static inline
/* The thread's own error words live in registers; the kernel entry adds them
   to the launch's words once, at the end (a device atomic inside a check
   would serialize every simdgroup shuffle after it). */
#define COAX_ERR thread uint *coax_err
COAX_DEV void coax_trap_(int32_t line, COAX_ERR) {
  coax_err[0] += 1u;
  coax_err[1] = max(coax_err[1], (uint)line);
}
/* Narrow integers are computed in 64 bits and range checked. */
#define COAX_NARROW_OPS(T, N, LO, HI)                                                                     \
  COAX_DEV T coax_add_##N##_(T a, T b, int32_t l, COAX_ERR) { int64_t r = (int64_t)a + (int64_t)b; if (r < LO || r > HI) coax_trap_(l, coax_err); return (T)r; } \
  COAX_DEV T coax_sub_##N##_(T a, T b, int32_t l, COAX_ERR) { int64_t r = (int64_t)a - (int64_t)b; if (r < LO || r > HI) coax_trap_(l, coax_err); return (T)r; } \
  COAX_DEV T coax_mul_##N##_(T a, T b, int32_t l, COAX_ERR) { int64_t r = (int64_t)a * (int64_t)b; if (r < LO || r > HI) coax_trap_(l, coax_err); return (T)r; } \
  COAX_DEV T coax_neg_##N##_(T a, int32_t l, COAX_ERR) { int64_t r = -(int64_t)a; if (r < LO || r > HI) coax_trap_(l, coax_err); return (T)r; } \
  COAX_DEV T coax_wadd_##N(T a, T b) { return (T)(uint64_t)((uint64_t)(int64_t)a + (uint64_t)(int64_t)b); }                        \
  COAX_DEV T coax_wsub_##N(T a, T b) { return (T)(uint64_t)((uint64_t)(int64_t)a - (uint64_t)(int64_t)b); }                        \
  COAX_DEV T coax_wmul_##N(T a, T b) { return (T)(uint64_t)((uint64_t)(int64_t)a * (uint64_t)(int64_t)b); }                        \
  COAX_DEV T coax_wneg_##N(T a) { return (T)(uint64_t)(0UL - (uint64_t)(int64_t)a); }
#define COAX_SHIFT_DIV_OPS(T, N, SIGNED, MIN)                                                             \
  COAX_DEV T coax_div_##N##_(T a, T b, int32_t l, COAX_ERR) {                                             \
    if (b == 0) { coax_trap_(l, coax_err); return 0; }                                                  \
    if (SIGNED && a == MIN && b == (T)-1) { coax_trap_(l, coax_err); return a; }                        \
    return (T)(a / b);                                                                                  \
  }                                                                                                     \
  COAX_DEV T coax_mod_##N##_(T a, T b, int32_t l, COAX_ERR) {                                             \
    if (b == 0) { coax_trap_(l, coax_err); return 0; }                                                  \
    if (SIGNED && b == (T)-1) return 0;                                                                 \
    return (T)(a % b);                                                                                  \
  }                                                                                                     \
  COAX_DEV T coax_shl_##N##_(T a, int64_t b, int32_t l, COAX_ERR) {                                       \
    if (b < 0 || b >= (int64_t)(8 * sizeof(T))) { coax_trap_(l, coax_err); return 0; }                  \
    return (T)((uint64_t)a << b);                                                                       \
  }                                                                                                     \
  COAX_DEV T coax_shr_##N##_(T a, int64_t b, int32_t l, COAX_ERR) {                                       \
    if (b < 0 || b >= (int64_t)(8 * sizeof(T))) { coax_trap_(l, coax_err); return 0; }                  \
    return (T)(a >> b);                                                                                 \
  }
COAX_NARROW_OPS(int8_t, s8, -128L, 127L)
COAX_NARROW_OPS(int16_t, s16, -32768L, 32767L)
COAX_NARROW_OPS(int32_t, s32, -2147483648L, 2147483647L)
COAX_NARROW_OPS(uint8_t, u8, 0L, 255L)
COAX_NARROW_OPS(uint16_t, u16, 0L, 65535L)
COAX_NARROW_OPS(uint32_t, u32, 0L, 4294967295L)
COAX_DEV int64_t coax_add_s64_(int64_t a, int64_t b, int32_t l, COAX_ERR) { int64_t r = (int64_t)((uint64_t)a + (uint64_t)b); if (((a ^ r) & (b ^ r)) < 0) coax_trap_(l, coax_err); return r; }
COAX_DEV int64_t coax_sub_s64_(int64_t a, int64_t b, int32_t l, COAX_ERR) { int64_t r = (int64_t)((uint64_t)a - (uint64_t)b); if (((a ^ b) & (a ^ r)) < 0) coax_trap_(l, coax_err); return r; }
COAX_DEV int64_t coax_mul_s64_(int64_t a, int64_t b, int32_t l, COAX_ERR) {
  int64_t r = (int64_t)((uint64_t)a * (uint64_t)b);
  if ((a == -1 && b == INT64_MIN) || (b == -1 && a == INT64_MIN) || (a != 0 && (r / a != b))) coax_trap_(l, coax_err);
  return r;
}
COAX_DEV int64_t coax_neg_s64_(int64_t a, int32_t l, COAX_ERR) { if (a == INT64_MIN) coax_trap_(l, coax_err); return (int64_t)(0UL - (uint64_t)a); }
COAX_DEV int64_t coax_wadd_s64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
COAX_DEV int64_t coax_wsub_s64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
COAX_DEV int64_t coax_wmul_s64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }
COAX_DEV int64_t coax_wneg_s64(int64_t a) { return (int64_t)(0UL - (uint64_t)a); }
COAX_DEV uint64_t coax_add_u64_(uint64_t a, uint64_t b, int32_t l, COAX_ERR) { uint64_t r = a + b; if (r < a) coax_trap_(l, coax_err); return r; }
COAX_DEV uint64_t coax_sub_u64_(uint64_t a, uint64_t b, int32_t l, COAX_ERR) { if (b > a) coax_trap_(l, coax_err); return a - b; }
COAX_DEV uint64_t coax_mul_u64_(uint64_t a, uint64_t b, int32_t l, COAX_ERR) { uint64_t r = a * b; if (a != 0 && r / a != b) coax_trap_(l, coax_err); return r; }
COAX_DEV uint64_t coax_neg_u64_(uint64_t a, int32_t l, COAX_ERR) { if (a != 0) coax_trap_(l, coax_err); return 0; }
COAX_DEV uint64_t coax_wadd_u64(uint64_t a, uint64_t b) { return a + b; }
COAX_DEV uint64_t coax_wsub_u64(uint64_t a, uint64_t b) { return a - b; }
COAX_DEV uint64_t coax_wmul_u64(uint64_t a, uint64_t b) { return a * b; }
COAX_DEV uint64_t coax_wneg_u64(uint64_t a) { return 0UL - a; }
COAX_SHIFT_DIV_OPS(int8_t, s8, 1, (int8_t)-128)
COAX_SHIFT_DIV_OPS(int16_t, s16, 1, (int16_t)-32768)
COAX_SHIFT_DIV_OPS(int32_t, s32, 1, (int32_t)(-2147483647 - 1))
COAX_SHIFT_DIV_OPS(int64_t, s64, 1, INT64_MIN)
COAX_SHIFT_DIV_OPS(uint8_t, u8, 0, 0)
COAX_SHIFT_DIV_OPS(uint16_t, u16, 0, 0)
COAX_SHIFT_DIV_OPS(uint32_t, u32, 0, 0)
COAX_SHIFT_DIV_OPS(uint64_t, u64, 0, 0)
#define coax_add_s8(a, b, l) coax_add_s8_(a, b, l, coax_err)
#define coax_add_s16(a, b, l) coax_add_s16_(a, b, l, coax_err)
#define coax_add_s32(a, b, l) coax_add_s32_(a, b, l, coax_err)
#define coax_add_s64(a, b, l) coax_add_s64_(a, b, l, coax_err)
#define coax_add_u8(a, b, l) coax_add_u8_(a, b, l, coax_err)
#define coax_add_u16(a, b, l) coax_add_u16_(a, b, l, coax_err)
#define coax_add_u32(a, b, l) coax_add_u32_(a, b, l, coax_err)
#define coax_add_u64(a, b, l) coax_add_u64_(a, b, l, coax_err)
#define coax_sub_s8(a, b, l) coax_sub_s8_(a, b, l, coax_err)
#define coax_sub_s16(a, b, l) coax_sub_s16_(a, b, l, coax_err)
#define coax_sub_s32(a, b, l) coax_sub_s32_(a, b, l, coax_err)
#define coax_sub_s64(a, b, l) coax_sub_s64_(a, b, l, coax_err)
#define coax_sub_u8(a, b, l) coax_sub_u8_(a, b, l, coax_err)
#define coax_sub_u16(a, b, l) coax_sub_u16_(a, b, l, coax_err)
#define coax_sub_u32(a, b, l) coax_sub_u32_(a, b, l, coax_err)
#define coax_sub_u64(a, b, l) coax_sub_u64_(a, b, l, coax_err)
#define coax_mul_s8(a, b, l) coax_mul_s8_(a, b, l, coax_err)
#define coax_mul_s16(a, b, l) coax_mul_s16_(a, b, l, coax_err)
#define coax_mul_s32(a, b, l) coax_mul_s32_(a, b, l, coax_err)
#define coax_mul_s64(a, b, l) coax_mul_s64_(a, b, l, coax_err)
#define coax_mul_u8(a, b, l) coax_mul_u8_(a, b, l, coax_err)
#define coax_mul_u16(a, b, l) coax_mul_u16_(a, b, l, coax_err)
#define coax_mul_u32(a, b, l) coax_mul_u32_(a, b, l, coax_err)
#define coax_mul_u64(a, b, l) coax_mul_u64_(a, b, l, coax_err)
#define coax_neg_s8(a, l) coax_neg_s8_(a, l, coax_err)
#define coax_neg_s16(a, l) coax_neg_s16_(a, l, coax_err)
#define coax_neg_s32(a, l) coax_neg_s32_(a, l, coax_err)
#define coax_neg_s64(a, l) coax_neg_s64_(a, l, coax_err)
#define coax_neg_u8(a, l) coax_neg_u8_(a, l, coax_err)
#define coax_neg_u16(a, l) coax_neg_u16_(a, l, coax_err)
#define coax_neg_u32(a, l) coax_neg_u32_(a, l, coax_err)
#define coax_neg_u64(a, l) coax_neg_u64_(a, l, coax_err)
#define coax_div_s8(a, b, l) coax_div_s8_(a, b, l, coax_err)
#define coax_div_s16(a, b, l) coax_div_s16_(a, b, l, coax_err)
#define coax_div_s32(a, b, l) coax_div_s32_(a, b, l, coax_err)
#define coax_div_s64(a, b, l) coax_div_s64_(a, b, l, coax_err)
#define coax_div_u8(a, b, l) coax_div_u8_(a, b, l, coax_err)
#define coax_div_u16(a, b, l) coax_div_u16_(a, b, l, coax_err)
#define coax_div_u32(a, b, l) coax_div_u32_(a, b, l, coax_err)
#define coax_div_u64(a, b, l) coax_div_u64_(a, b, l, coax_err)
#define coax_mod_s8(a, b, l) coax_mod_s8_(a, b, l, coax_err)
#define coax_mod_s16(a, b, l) coax_mod_s16_(a, b, l, coax_err)
#define coax_mod_s32(a, b, l) coax_mod_s32_(a, b, l, coax_err)
#define coax_mod_s64(a, b, l) coax_mod_s64_(a, b, l, coax_err)
#define coax_mod_u8(a, b, l) coax_mod_u8_(a, b, l, coax_err)
#define coax_mod_u16(a, b, l) coax_mod_u16_(a, b, l, coax_err)
#define coax_mod_u32(a, b, l) coax_mod_u32_(a, b, l, coax_err)
#define coax_mod_u64(a, b, l) coax_mod_u64_(a, b, l, coax_err)
#define coax_shl_s8(a, b, l) coax_shl_s8_(a, b, l, coax_err)
#define coax_shl_s16(a, b, l) coax_shl_s16_(a, b, l, coax_err)
#define coax_shl_s32(a, b, l) coax_shl_s32_(a, b, l, coax_err)
#define coax_shl_s64(a, b, l) coax_shl_s64_(a, b, l, coax_err)
#define coax_shl_u8(a, b, l) coax_shl_u8_(a, b, l, coax_err)
#define coax_shl_u16(a, b, l) coax_shl_u16_(a, b, l, coax_err)
#define coax_shl_u32(a, b, l) coax_shl_u32_(a, b, l, coax_err)
#define coax_shl_u64(a, b, l) coax_shl_u64_(a, b, l, coax_err)
#define coax_shr_s8(a, b, l) coax_shr_s8_(a, b, l, coax_err)
#define coax_shr_s16(a, b, l) coax_shr_s16_(a, b, l, coax_err)
#define coax_shr_s32(a, b, l) coax_shr_s32_(a, b, l, coax_err)
#define coax_shr_s64(a, b, l) coax_shr_s64_(a, b, l, coax_err)
#define coax_shr_u8(a, b, l) coax_shr_u8_(a, b, l, coax_err)
#define coax_shr_u16(a, b, l) coax_shr_u16_(a, b, l, coax_err)
#define coax_shr_u32(a, b, l) coax_shr_u32_(a, b, l, coax_err)
#define coax_shr_u64(a, b, l) coax_shr_u64_(a, b, l, coax_err)
/* A failed index or slice check reads element 0 of the same view instead. */
COAX_DEV int64_t coax_index_(int64_t i, int64_t n, int32_t l, COAX_ERR) { if ((uint64_t)i >= (uint64_t)n) { coax_trap_(l, coax_err); return 0; } return i; }
#define coax_index(i, n, l) coax_index_(i, n, l, coax_err)
COAX_DEV bool coax_slice_ok_(int64_t lo, int64_t hi, int64_t n, int32_t l, COAX_ERR) { if ((uint64_t)hi > (uint64_t)n || (uint64_t)lo > (uint64_t)hi) { coax_trap_(l, coax_err); return false; } return true; }
COAX_DEV int64_t coax_cast_ss_(int64_t v, int64_t lo, int64_t hi, int32_t l, COAX_ERR) { if (v < lo || v > hi) coax_trap_(l, coax_err); return v; }
COAX_DEV uint64_t coax_cast_su_(int64_t v, uint64_t hi, int32_t l, COAX_ERR) { if (v < 0 || (uint64_t)v > hi) coax_trap_(l, coax_err); return (uint64_t)v; }
COAX_DEV int64_t coax_cast_us_(uint64_t v, int64_t hi, int32_t l, COAX_ERR) { if (v > (uint64_t)hi) coax_trap_(l, coax_err); return (int64_t)v; }
COAX_DEV uint64_t coax_cast_uu_(uint64_t v, uint64_t hi, int32_t l, COAX_ERR) { if (v > hi) coax_trap_(l, coax_err); return v; }
#define coax_cast_ss(v, lo, hi, l) coax_cast_ss_(v, lo, hi, l, coax_err)
#define coax_cast_su(v, hi, l) coax_cast_su_(v, hi, l, coax_err)
#define coax_cast_us(v, hi, l) coax_cast_us_(v, hi, l, coax_err)
#define coax_cast_uu(v, hi, l) coax_cast_uu_(v, hi, l, coax_err)
COAX_DEV int64_t coax_ftoi_s_(float v, int64_t lo, int64_t hi, int32_t l, COAX_ERR) {
  if (!(v >= -9223372036854775808.0f && v < 9223372036854775808.0f)) { coax_trap_(l, coax_err); return 0; }
  int64_t i = (int64_t)v; if (i < lo || i > hi) coax_trap_(l, coax_err); return i;
}
COAX_DEV uint64_t coax_ftoi_u_(float v, uint64_t hi, int32_t l, COAX_ERR) {
  if (!(v > -1.0f && v < 18446744073709551616.0f)) { coax_trap_(l, coax_err); return 0; }
  uint64_t u = (uint64_t)v; if (u > hi) coax_trap_(l, coax_err); return u;
}
#define coax_ftoi_s(v, lo, hi, l) coax_ftoi_s_(v, lo, hi, l, coax_err)
#define coax_ftoi_u(v, hi, l) coax_ftoi_u_(v, hi, l, coax_err)
COAX_DEV float coax_f32_from_bits(uint32_t x) { return as_type<float>(x); }
COAX_DEV uint32_t coax_f32_bits(float f) { return as_type<uint>(f); }
COAX_DEV uint32_t coax_add_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a+b);}
COAX_DEV uint32_t coax_sub_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a-b);}
COAX_DEV uint32_t coax_mul_wrap_u32(uint32_t a,uint32_t b){return (uint32_t)(a*b);}
COAX_DEV uint32_t coax_rotl_u32(uint32_t a,uint32_t b){uint k=(uint)(b&31u);return (uint32_t)((a<<k)|(a>>((32u-k)&31u)));}
COAX_DEV uint32_t coax_rotr_u32(uint32_t a,uint32_t b){uint k=(uint)(b&31u);return (uint32_t)((a>>k)|(a<<((32u-k)&31u)));}
COAX_DEV uint64_t coax_add_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a+b);}
COAX_DEV uint64_t coax_sub_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a-b);}
COAX_DEV uint64_t coax_mul_wrap_u64(uint64_t a,uint64_t b){return (uint64_t)(a*b);}
COAX_DEV uint64_t coax_rotl_u64(uint64_t a,uint64_t b){uint k=(uint)(b&63u);return (uint64_t)((a<<k)|(a>>((64u-k)&63u)));}
COAX_DEV uint64_t coax_rotr_u64(uint64_t a,uint64_t b){uint k=(uint)(b&63u);return (uint64_t)((a>>k)|(a<<((64u-k)&63u)));}

/* APPLE-FF-v1: f64 is a float-float pair, value hi + lo with |lo| <= ulp(hi)/2
   (about 48 significant bits; the f32 exponent range, subnormals flushed).
   Sums use error-free two-sum, products the exact fma residual. */
struct coax_ff { float hi; float lo; };
COAX_DEV coax_ff coax_ff_make(float h, float l) { coax_ff r; r.hi = h; r.lo = l; return r; }
COAX_DEV coax_ff coax_ff_two_sum(float a, float b) { float s = a + b; float bb = s - a; return coax_ff_make(s, (a - (s - bb)) + (b - bb)); }
COAX_DEV coax_ff coax_ff_fast(float a, float b) { float s = a + b; return coax_ff_make(s, b - (s - a)); }
COAX_DEV coax_ff operator+(coax_ff a, coax_ff b) {
  coax_ff s = coax_ff_two_sum(a.hi, b.hi);
  if (!isfinite(s.hi)) return coax_ff_make(s.hi, 0.0f);
  coax_ff t = coax_ff_two_sum(a.lo, b.lo);
  s = coax_ff_fast(s.hi, s.lo + t.hi);
  return coax_ff_fast(s.hi, s.lo + t.lo);
}
COAX_DEV coax_ff operator-(coax_ff a) { return coax_ff_make(-a.hi, -a.lo); }
COAX_DEV coax_ff operator-(coax_ff a, coax_ff b) { return a + (-b); }
COAX_DEV coax_ff operator*(coax_ff a, coax_ff b) {
  float p = a.hi * b.hi;
  if (!isfinite(p) || p == 0.0f) return coax_ff_make(p, 0.0f);
  float e = fma(a.hi, b.hi, -p);
  e = e + (a.hi * b.lo + a.lo * b.hi);
  return coax_ff_fast(p, e);
}
COAX_DEV coax_ff operator/(coax_ff a, coax_ff b) {
  float q1 = a.hi / b.hi;
  if (!isfinite(q1) || q1 == 0.0f) return coax_ff_make(q1, 0.0f);
  coax_ff r = a - b * coax_ff_make(q1, 0.0f);
  float q2 = r.hi / b.hi;
  r = r - b * coax_ff_make(q2, 0.0f);
  float q3 = r.hi / b.hi;
  return coax_ff_fast(q1, q2) + coax_ff_make(q3, 0.0f);
}
COAX_DEV bool operator==(coax_ff a, coax_ff b) { return a.hi == b.hi && a.lo == b.lo; }
COAX_DEV bool operator!=(coax_ff a, coax_ff b) { return !(a == b); }
COAX_DEV bool operator<(coax_ff a, coax_ff b) { return a.hi < b.hi || (a.hi == b.hi && a.lo < b.lo); }
COAX_DEV bool operator>(coax_ff a, coax_ff b) { return b < a; }
COAX_DEV bool operator<=(coax_ff a, coax_ff b) { return a < b || a == b; }
COAX_DEV bool operator>=(coax_ff a, coax_ff b) { return b < a || a == b; }
COAX_DEV coax_ff coax_ff_from_f32(float x) { return coax_ff_make(x, 0.0f); }
COAX_DEV coax_ff coax_ff_from_s64(int64_t v) { float h = (float)v; return coax_ff_fast(h, (float)(v - (int64_t)h)); }
COAX_DEV coax_ff coax_ff_from_u64(uint64_t v) { float h = (float)v; return coax_ff_fast(h, (float)(int64_t)(v - (uint64_t)h)); }
/* Rounds the pair once; a finite pair that rounds past FLT_MAX fails, as the C build's f32(x) does. */
COAX_DEV float coax_ff_to_f32_(coax_ff a, int32_t l, COAX_ERR) { float r = a.hi + a.lo; if (isinf(r) && isfinite(a.hi)) coax_trap_(l, coax_err); return r; }
#define coax_ff_to_f32(a, l) coax_ff_to_f32_(a, l, coax_err)
COAX_DEV coax_ff coax_ff_abs(coax_ff a) { return a.hi < 0.0f ? -a : a; }
COAX_DEV coax_ff coax_ff_floor(coax_ff a) { float f = floor(a.hi); if (f == a.hi) return coax_ff_fast(f, floor(a.lo)); return coax_ff_make(f, 0.0f); }
/* Scales a pair by 2^k exactly (no overflow or flush for |k| in the ranges used). */
COAX_DEV coax_ff coax_ff_scale(coax_ff a, int k) { return coax_ff_make(ldexp(a.hi, k), ldexp(a.lo, k)); }
/* sqrt: scale by an even power of two into [1, 4) so the residual stays
   normal, one Newton correction, then scale back. */
COAX_DEV coax_ff coax_ff_sqrt(coax_ff a) {
  if (!(a.hi > 0.0f) || isinf(a.hi)) return coax_ff_make(precise::sqrt(a.hi), 0.0f);
  int k = ilogb(a.hi) & ~1;
  coax_ff m = coax_ff_scale(a, -k);
  float s = precise::sqrt(m.hi), p = s * s;
  coax_ff r = m - coax_ff_make(p, fma(s, s, -p));
  return coax_ff_scale(coax_ff_fast(s, r.hi / (2.0f * s)), k / 2);
}
/* exp: x = n ln2 + r, r/256 through a degree-8 expm1 series, then eight
   doublings of expm1 (e^2y - 1 = 2(e^y - 1) + (e^y - 1)^2), then 2^n. */
COAX_DEV coax_ff coax_ff_exp(coax_ff a) {
  if (isnan(a.hi)) return a;
  if (a.hi > 88.7228394f) return coax_ff_make(as_type<float>(0x7f800000u), 0.0f); /* above log FLT_MAX */
  if (a.hi < -103.98f) return coax_ff_make(0.0f, 0.0f);
  const coax_ff ln2 = coax_ff_make(0.693147182f, -1.90465421e-09f);
  const coax_ff inv[7] = {coax_ff_make(0.5f, 0.0f), coax_ff_make(0.166666672f, -4.96705388e-09f), coax_ff_make(0.0416666679f, -1.24176347e-09f),
    coax_ff_make(0.00833333377f, -4.34617203e-10f), coax_ff_make(0.00138888892f, -3.36310944e-11f), coax_ff_make(0.000198412701f, -2.72559687e-12f),
    coax_ff_make(2.48015876e-05f, -3.40699609e-13f)};
  float n = rint(a.hi / 0.693147182f);
  coax_ff r = a - ln2 * coax_ff_make(n, 0.0f);
  r = coax_ff_make(r.hi * 0.00390625f, r.lo * 0.00390625f);
  coax_ff s = r, p = r;
  for (int k = 0; k < 7; k++) { p = p * r; s = s + p * inv[k]; }
  for (int k = 0; k < 8; k++) s = coax_ff_make(2.0f * s.hi, 2.0f * s.lo) + s * s;
  s = s + coax_ff_make(1.0f, 0.0f);
  int e = (int)n;
  return coax_ff_make(ldexp(s.hi, e), ldexp(s.lo, e));
}
/* log: x = m 2^k with m in [sqrt(1/2), sqrt(2)); log m = 2 atanh u with
   u = (m - 1) / (m + 1), |u| <= 0.172, summed to u^25 (the next term is
   below 2^-50 of the sum); then k ln2. Near x = 1 the result keeps its
   relative accuracy because m - 1 is exact. */
COAX_DEV coax_ff coax_ff_log(coax_ff a) {
  if (!(a.hi > 0.0f) || isinf(a.hi)) return coax_ff_make(precise::log(a.hi), 0.0f);
  const coax_ff ln2 = coax_ff_make(0.693147182f, -1.90465421e-09f);
  const coax_ff odd[13] = {coax_ff_make(1.0f, 0.0f), coax_ff_make(0.333333343f, -9.93410776e-09f), coax_ff_make(0.200000003f, -2.98023228e-09f),
    coax_ff_make(0.142857149f, -6.386212e-09f), coax_ff_make(0.111111112f, -8.27842295e-10f), coax_ff_make(0.0909090936f, -2.70930212e-09f),
    coax_ff_make(0.0769230798f, -2.86560797e-09f), coax_ff_make(0.0666666701f, -3.47693763e-09f), coax_ff_make(0.0588235296f, -2.19134724e-10f),
    coax_ff_make(0.0526315793f, -3.92135824e-10f), coax_ff_make(0.0476190485f, -8.86973883e-10f), coax_ff_make(0.0434782617f, -8.09845746e-10f),
    coax_ff_make(0.0399999991f, 8.94069652e-10f)};
  int k = ilogb(a.hi);
  coax_ff m = coax_ff_scale(a, -k);
  if (m.hi > 1.41421354f) { m = coax_ff_scale(m, -1); k += 1; }
  const coax_ff one = coax_ff_make(1.0f, 0.0f);
  coax_ff u = (m - one) / (m + one), u2 = u * u, p = odd[12];
  for (int j = 11; j >= 0; j--) p = p * u2 + odd[j];
  coax_ff y = u * p;
  y = coax_ff_make(2.0f * y.hi, 2.0f * y.lo);
  return ln2 * coax_ff_make((float)k, 0.0f) + y;
}

/* cu_mma_bf16_16x8x16 for one simdgroup, in the PTX fragment layout of
   lib/compute/mma.coax: each lane gathers rows g and g+8 of A and columns 2t
   and 2t+1 of B from the lanes that hold them, adds the 16 exact products in
   f32 in k order and adds that sum into C. */
COAX_DEV float coax_bf16_lo(uint w) { return as_type<float>(w << 16); }
COAX_DEV float coax_bf16_hi(uint w) { return as_type<float>(w & 0xffff0000u); }
/* The gathered rows of A (the shuffles below) depend only on the
   simdgroup's A fragments, so a caller that multiplies one A by many B
   (dw_project) keeps them in coax_mc and gathers again only when some lane's
   A changed (simd_all keeps the branch uniform). */
typedef struct { uint a[4]; uint x[16]; bool ok; } coax_mma_cache;
COAX_DEV void coax_mma_bf16_16x8x16(thread uint *a, thread uint *b, thread float *c, uint lane, thread coax_mma_cache &mc) {
  uint g = lane >> 2, t = lane & 3u;
  uint a0 = a[0], a1 = a[1], a2 = a[2], a3 = a[3], b0 = b[0], b1 = b[1];
  bool same = mc.ok && a0 == mc.a[0] && a1 == mc.a[1] && a2 == mc.a[2] && a3 == mc.a[3];
  if (!simd_all(same)) {
    for (uint j = 0; j < 4; j++) {
      ushort sa = (ushort)(4 * g + j);
      mc.x[2 * j] = simd_shuffle(a0, sa); mc.x[2 * j + 1] = simd_shuffle(a1, sa);
      mc.x[8 + 2 * j] = simd_shuffle(a2, sa); mc.x[9 + 2 * j] = simd_shuffle(a3, sa);
    }
    mc.a[0] = a0; mc.a[1] = a1; mc.a[2] = a2; mc.a[3] = a3; mc.ok = true;
  }
  float s00 = 0.0f, s01 = 0.0f, s10 = 0.0f, s11 = 0.0f;
  /* A product of two bf16 values is exact in f32, so fma(x, y, s) rounds
     exactly as s + x * y. Step J of half H covers k = 8H + 2J and 8H + 2J + 1. */
#define COAX_MMA_STEP(J, H)                                                                        \
  {                                                                                                \
    ushort sb0 = (ushort)(8 * t + J), sb1 = (ushort)(8 * t + 4 + J);                               \
    uint x0 = mc.x[8 * H + 2 * J], x1 = mc.x[8 * H + 2 * J + 1];                                   \
    uint y0 = simd_shuffle(H ? b1 : b0, sb0), y1 = simd_shuffle(H ? b1 : b0, sb1);                 \
    float r0l = coax_bf16_lo(x0), r0h = coax_bf16_hi(x0), r1l = coax_bf16_lo(x1), r1h = coax_bf16_hi(x1); \
    float c0l = coax_bf16_lo(y0), c0h = coax_bf16_hi(y0), c1l = coax_bf16_lo(y1), c1h = coax_bf16_hi(y1); \
    s00 = fma(r0l, c0l, s00); s00 = fma(r0h, c0h, s00); s01 = fma(r0l, c1l, s01); s01 = fma(r0h, c1h, s01); \
    s10 = fma(r1l, c0l, s10); s10 = fma(r1h, c0h, s10); s11 = fma(r1l, c1l, s11); s11 = fma(r1h, c1h, s11); \
  }
  COAX_MMA_STEP(0, 0) COAX_MMA_STEP(1, 0) COAX_MMA_STEP(2, 0) COAX_MMA_STEP(3, 0)
  COAX_MMA_STEP(0, 1) COAX_MMA_STEP(1, 1) COAX_MMA_STEP(2, 1) COAX_MMA_STEP(3, 1)
#undef COAX_MMA_STEP
  c[0] += s00; c[1] += s01; c[2] += s10; c[3] += s11;
}
)COAXRT";

// `coax metal-compute FILE`: the kernels of a file and what they call, as
// Metal Shading Language. Each kernel takes one constant argument block at
// buffer index 0 and the launch's two error words at buffer index 1. The
// argument block is the work-item count (8 bytes) and then each parameter
// after the work item, in order: a view is 16 bytes (the device address of
// element 0, then the element count), an f64 is 8 bytes (hi, then lo, as
// f32), and every other scalar sits in the low bytes of its own 8.
std::string emit_metal_compute(const File *file, const EmitOptions &opts) {
  g_metal_compute = true;
  Emitter em{file, opts};
  std::vector<const ProcDecl *> procs = cuda_procs(file);
  std::string functions;
  for (const ProcDecl *p : procs) functions += em.function(p);
  std::vector<Type> used;
  auto add = [&](Type t) {
    if (t->kind != TypeKind::array && t->kind != TypeKind::view) return;
    for (Type u : used) if (u == t) return;
    used.push_back(t);
  };
  std::function<void(const Expr *)> types_expr = [&](const Expr *e) {
    if (!e) return;
    add(e->type);
    types_expr(e->lhs); types_expr(e->rhs); types_expr(e->third);
    for (const Expr *a : e->args) types_expr(a);
  };
  std::function<void(const Stmt *)> types_stmt = [&](const Stmt *st) {
    if (!st) return;
    add(st->type);
    types_expr(st->expr); types_expr(st->expr2); types_expr(st->target);
    for (const Stmt *c : st->body) types_stmt(c);
    types_stmt(st->then_); types_stmt(st->else_);
  };
  for (const ProcDecl *p : procs) {
    add(p->ret);
    for (const Param &prm : p->params) add(prm.type);
    types_stmt(p->body);
  }
  // Arrays and views hold scalars only (admission), so they never depend on each other.
  std::string types, helpers;
  for (Type t : used) {
    std::string c = c_type(t), e = c_type(t->elem), tg = tag(t);
    if (t->kind == TypeKind::array) { types += "typedef struct { " + e + " e[" + std::to_string(t->length) + "]; } " + c + ";\n"; continue; }
    types += "typedef struct { device " + e + " *data; int64_t count; } " + c + ";\n";
    helpers += "COAX_DEV device " + e + " *coax_elem_" + tg + "_(" + c + " v, int64_t i, int32_t l, COAX_ERR) { return &v.data[coax_index(i, v.count, l)]; }\n";
    helpers += "#define coax_elem_" + tg + "(v, i, l) coax_elem_" + tg + "_(v, i, l, coax_err)\n";
    helpers += "COAX_DEV " + c + " coax_reslice_" + tg + "_(" + c + " v, int64_t lo, int64_t hi, int32_t l, COAX_ERR) {\n"
               "  " + c + " r = {v.data, 0};\n  if (coax_slice_ok_(lo, hi, v.count, l, coax_err)) { r.data = v.data + lo; r.count = hi - lo; }\n  return r;\n}\n";
    helpers += "#define coax_reslice_" + tg + "(v, lo, hi, l) coax_reslice_" + tg + "_(v, lo, hi, l, coax_err)\n";
  }
  std::string s = "/* Generated by coax0 from " + file->src->path + " (coax metal-compute). Do not edit: change the .coax file. */\n";
  s += METAL_RUNTIME;
  s += "\n/* Types */\n" + types + helpers + "\n";
  bool simdgroups = false;
  for (const auto &[call, kind] : g_warp.apple_calls) simdgroups = simdgroups || kind != 3;
  if (simdgroups) s += METAL_SIMDGROUP_RUNTIME;
  if (em.apple_tensor) s += METAL_TENSOR_RUNTIME;
  for (const ProcDecl *p : procs) s += em.prototype(p) + ";\n";
  s += functions;
  s += "\n/* Kernels: one argument block at buffer(0) (the item count, then each parameter\n"
       "   in an 8-byte slot, views in 16), the error words at buffer(1). */\n";
  for (const ProcDecl *p : procs) {
    if (!p->export_c) continue;
    std::string name = to_std(p->name), block = "struct coax_args_" + name + " {\n  int64_t coax_items;\n";
    std::string call = em.proc_name(p) + "(coax_item";
    for (int32_t i = 1; i < p->params.count; i++) {
      Type t = p->params[i].type;
      std::string field = em.local_name(p->params[i].name);
      block += "  " + c_type(t) + " " + field + ";";
      int64_t size = t->kind == TypeKind::view ? 16 : t == &T_F64 ? 8 : t->kind == TypeKind::bool_ ? 1 : t->bits / 8;
      if (size < 8) block += " uint8_t coax_pad" + std::to_string(i) + "[" + std::to_string(8 - size) + "];";
      block += "\n";
      call += ", coax_a." + field;
    }
    block += "};\n";
    bool warp = g_warp.warp_kernels.count(p), group = g_warp.group_kernels.count(p);
    s += block;
    s += "kernel void " + name + "(constant coax_args_" + name + " &coax_a [[buffer(0)]], device atomic_uint *coax_errors [[buffer(1)]],\n"
         "    uint coax_gid [[thread_position_in_grid]], uint coax_simd_lane [[thread_index_in_simdgroup]]" +
         std::string(group ? ",\n    uint coax_tid [[thread_index_in_threadgroup]], uint coax_tpg [[threads_per_threadgroup]], uint coax_tgid [[threadgroup_position_in_grid]]" : "") + ") {\n";
    std::string shared_args;
    if (group) {  // one threadgroup of T threads per work item; the host launches T threads per group
      std::string T = std::to_string(g_warp.group_kernels.at(p));
      if (g_warp.shared_arrays.count(p))
        for (const Stmt *d : g_warp.shared_arrays.at(p)) {
          s += "  threadgroup " + c_type(d->type) + " coax_tg_" + em.local_name(d->name) + ";\n";
          shared_args += ", &coax_tg_" + em.local_name(d->name);
        }
      s += "  if (coax_tpg != " + T + "u) {  /* launched with the wrong group size: the launch is void */\n"
           "    if (coax_tid == 0) { atomic_fetch_add_explicit(&coax_errors[0], 1u, memory_order_relaxed); atomic_fetch_max_explicit(&coax_errors[1], " +
           std::to_string(p->loc.line) + "u, memory_order_relaxed); }\n    return;\n  }\n";
      s += "  int64_t coax_item = (int64_t)coax_tgid;\n  uint coax_lane = coax_tid;\n";
    } else if (warp)  // one simdgroup of 32 lanes per work item
      s += "  int64_t coax_item = (int64_t)(coax_gid >> 5);\n  uint coax_lane = coax_simd_lane;\n";
    else
      s += "  int64_t coax_item = (int64_t)coax_gid;\n  uint coax_lane = coax_simd_lane;\n";
    s += "  if (coax_item >= coax_a.coax_items) return;\n  uint coax_err[2] = {0, 0};\n  " + call + (p->params.count > 1 ? ", " : ", ") +
         "coax_lane, coax_err" + shared_args + ");\n"
         "  if (coax_err[0]) {\n"
         "    atomic_fetch_add_explicit(&coax_errors[0], coax_err[0], memory_order_relaxed);\n"
         "    atomic_fetch_max_explicit(&coax_errors[1], coax_err[1], memory_order_relaxed);\n  }\n}\n";
  }
  for (size_t at = 0; (at = s.find("_Bool", at)) != std::string::npos;) s.replace(at, 5, "bool");
  g_metal_compute = false;
  return s;
}

std::string emit_c(const File *file, const EmitOptions &opts) {
  if (opts.cuda) return emit_cuda(file, opts);
  if (opts.metal_compute) return emit_metal_compute(file, opts);
  Emitter em{file, opts};
  std::string functions;
  for (const ProcDecl *p : file->procs) functions += em.function(p);
  if (opts.metal) {
    // Scalar f32/bool functions only (the checker enforced it): no runtime.
    std::string s = "/* Generated by coax0 from " + file->src->path + ". Do not edit: change the .coax file. */\n";
    s += "#include <metal_stdlib>\nusing namespace metal;\n\n";
    for (const ProcDecl *p : file->procs) s += em.prototype(p) + ";\n";
    s += functions;
    for (size_t at = 0; (at = s.find("_Bool", at)) != std::string::npos;) s.replace(at, 5, "bool");
    return s;
  }
  std::string inits = init_functions(file, em);
  std::string shows = show_functions(file, em);

  TypeWriter types;
  types.forward_structs(file);
  std::vector<Type> all;
  collect_types(file, &all);
  for (Type t : all) types.need(t);
  types.layout_asserts(file);

  std::string s = "/* Generated by coax0 from " + file->src->path + ". Do not edit: change the .coax file. */\n";
  s += "static const char *coax_source_path = \"" + escape_path(file->src->path) + "\";\n";
  s += source_runtime(file);
  if (em.barrier) s += BARRIER_RUNTIME;
  if (em.wrapping) s += WRAP_RUNTIME;
  s += "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n";
  s += "\n/* Types */\n" + types.out;
  if (em.wide) s += WIDE_RUNTIME;
  s += view_helpers();
  if (!em.ct_values.empty()) {
    s += "\n/* Values computed at compile time */\n";
    for (size_t i = 0; i < em.ct_values.size(); i++) {
      const Expr *e = em.ct_values[i];
      s += "static const " + c_type(e->type) + " coax_k" + std::to_string(i) + " = " + em.init_list(*e->ct) + ";\n";
    }
  }
  s += "\n" + inits + shows + "\n";
  for (const ProcDecl *p : file->procs) s += em.prototype(p) + ";\n";
  s += functions;
  bool has_main = false;
  for (const ProcDecl *p : file->procs) has_main = has_main || str_eq(p->name, "main");
  if (!opts.library && has_main) s += "\nint main(void) {\n  coax_p_main();\n  return 0;\n}\n";
  return s;
}

std::string emit_header(const File *file) {
  std::string base = file->src->path;
  size_t slash = base.find_last_of('/');
  if (slash != std::string::npos) base = base.substr(slash + 1);
  size_t dot = base.find_last_of('.');
  if (dot != std::string::npos) base = base.substr(0, dot);
  std::string guard = "COAX_";
  for (char c : base) guard += (char)(isalnum((unsigned char)c) ? toupper((unsigned char)c) : '_');
  guard += "_H";

  // The types the exported procedures use, and everything they contain.
  TypeWriter types;
  types.header = true;
  for (const StructDecl *d : file->structs) {
    bool used = false;
    std::vector<Type> visiting;
    std::function<bool(Type)> uses = [&](Type t) -> bool {
      if (t == d->type) return true;
      for (Type v : visiting) if (v == t) return false;
      visiting.push_back(t);
      bool found = false;
      if (t->kind == TypeKind::array || t->kind == TypeKind::view) found = uses(t->elem);
      if (t->kind == TypeKind::struct_)
        for (const Field &f : t->struct_decl->fields) found = found || uses(f.type);
      visiting.pop_back();
      return found;
    };
    for (const ProcDecl *p : file->procs) {
      if (!p->export_c && !p->foreign) continue;
      used = used || uses(p->ret);
      for (const Param &prm : p->params) used = used || uses(prm.type);
    }
    if (used) types.out += "typedef struct coax_" + tag(d->type) + " coax_" + tag(d->type) + ";\n";
  }
  for (const ProcDecl *p : file->procs) {
    if (!p->export_c) continue;
    types.need(p->ret);
    for (const Param &prm : p->params) types.need(prm.type);
  }
  std::string asserts;
  for (const StructDecl *d : file->structs) {
    if (!types.seen(d->type)) continue;
    std::string c = c_type(d->type);
    asserts += "COAX_LAYOUT(sizeof(" + c + ") == " + std::to_string(d->type->size) + " && COAX_ALIGNOF(" + c +
               ") == " + std::to_string(d->type->align) + ", \"layout of " + to_std(d->name) + "\");\n";
    for (const Field &f : d->fields)
      asserts += "COAX_LAYOUT(offsetof(" + c + ", " + to_std(f.name) + ") == " + std::to_string(f.offset) +
                 ", \"offset of " + to_std(d->name) + "." + to_std(f.name) + "\");\n";
  }

  Emitter em{file, {}};
  std::string s = "/* Generated by coax0 from " + file->src->path + ". Do not edit: change the .coax file.\n"
                  "   Coax int is int64_t; a Coax string or view is a pointer and a count, not\n"
                  "   NUL-terminated; a failed check inside a call aborts the process. */\n";
  s += "#ifndef " + guard + "\n#define " + guard + "\n\n#include <stddef.h>\n#include <stdint.h>\n\n";
  s += "#ifdef __cplusplus\n#define _Bool bool\n#define COAX_LAYOUT(c, m) static_assert(c, m)\n#define COAX_ALIGNOF alignof\nextern \"C\" {\n#else\n"
       "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n#define COAX_ALIGNOF _Alignof\n#endif\n\n";
  s += "#ifndef COAX_STRING_DEFINED\n#define COAX_STRING_DEFINED\ntypedef struct { const char *data; int64_t count; } coax_string;\n#endif\n\n";
  s += types.out + asserts + "\n";
  for (const ProcDecl *p : file->procs)
    if (p->export_c) s += em.prototype(p, true) + ";\n";
  bool any_foreign = false;
  for (const ProcDecl *p : file->procs) any_foreign = any_foreign || p->foreign;
  if (any_foreign) {
    s += "\n/* Provided by the host (#foreign). */\n";
    for (const ProcDecl *p : file->procs)
      if (p->foreign) s += em.prototype(p, true) + ";\n";
  }
  s += "\n#ifdef __cplusplus\n}\n#undef _Bool\n#endif\n#undef COAX_LAYOUT\n#undef COAX_ALIGNOF\n#endif\n";
  return s;
}

std::string layout_report(const File *file) {
  std::string s;
  char buf[256];
  for (const StructDecl *d : file->structs) {
    snprintf(buf, sizeof buf, "%s  size %d  align %d\n", to_std(d->name).c_str(), d->type->size, d->type->align);
    s += buf;
    int32_t end = 0;
    for (const Field &f : d->fields) {
      if (f.offset > end) {
        snprintf(buf, sizeof buf, "  %6d  %4d  (padding)\n", end, f.offset - end);
        s += buf;
      }
      snprintf(buf, sizeof buf, "  %6d  %4d  %s: %s\n", f.offset, f.type->size, to_std(f.name).c_str(), type_name(f.type));
      s += buf;
      end = f.offset + f.type->size;
    }
    if (d->type->size > end && d->fields.count) {
      snprintf(buf, sizeof buf, "  %6d  %4d  (padding)\n", end, d->type->size - end);
      s += buf;
    }
  }
  for (const EnumDecl *d : file->enums) {
    snprintf(buf, sizeof buf, "%s  enum %s  size %d\n", to_std(d->name).c_str(), type_name(d->type->elem), d->type->size);
    s += buf;
    for (const EnumMember &m : d->members) {
      std::string v = d->type->elem->is_signed ? std::to_string((int64_t)m.bits) : std::to_string(m.bits);
      snprintf(buf, sizeof buf, "  %s = %s\n", to_std(m.name).c_str(), v.c_str());
      s += buf;
    }
  }
  if (s.empty()) s = "no structs or enums\n";
  return s;
}

}  // namespace coax

namespace coax {
static std::string debug_source_hash(const std::string &text) {
#ifndef COAX_BOOTSTRAP_IMPORTER
  uint8_t out[64];auto digest=S_hash({text.data(),(int64_t)text.size()});
  if(S_hex(digest,{out,64}))return std::string((char*)out,64);
#endif
  return ""; // build-only stage0 refuses --source-map; no native hash fallback
}
std::string emit_debug_map(const File *file) {
  Emitter em{file,{}};
  std::string out="{\"schema_version\":1,\"sources\":[";
  bool first=true;
  for (const auto &unit:file->src->units) {out+=(first?"":",")+json_string(unit->canonical);first=false;}
  out+="],\"source_hashes\":{";first=true;
  for(const auto &unit:file->src->units) {out+=(first?"":",")+json_string(unit->canonical)+":"+json_string(debug_source_hash(unit->text));first=false;}
  out+="},\"functions\":[";first=true;
  for (const auto *p:file->procs) {
    if(p->foreign) continue;
    auto point=source_point(file->src,p->loc);
    out+=(first?"":",")+std::string("{\"name\":")+json_string(to_std(p->name))+",\"c_name\":"+json_string(em.proc_name(p))+",\"file\":"+json_string(debug_path(file->src,p->loc))+",\"line\":"+std::to_string(point.loc.line)+",\"variables\":[";first=false;
    bool variable_first=true;
    auto variable=[&](Str name,Type type,Loc loc,const char *role) {
      auto at=source_point(file->src,loc);
      out+=(variable_first?"":",")+std::string("{\"name\":")+json_string(to_std(name))+",\"c_name\":"+json_string(em.local_name(name))+",\"type\":"+json_string(type->name)+",\"role\":"+json_string(role)+",\"line\":"+std::to_string(at.loc.line)+"}";variable_first=false;
    };
    for(const auto &arg:p->params) variable(arg.name,arg.type,arg.loc,"parameter");
    std::function<void(const Stmt*)> walk=[&](const Stmt *s) {
      if(!s) return;
      if(s->kind==StmtKind::var) variable(s->name,s->type,s->loc,"local");
      if(s->kind==StmtKind::for_range || s->kind==StmtKind::for_each) {
        variable(s->name,s->type,s->loc,"loop");
        if(s->name2.count) variable(s->name2,T_INT,s->name2_loc,"loop");
      }
      for(const auto *child:s->body) walk(child);
      walk(s->then_);walk(s->else_);
    };
    walk(p->body);out+="]}";
  }
  return out+"]}\n";
}
} // namespace coax
