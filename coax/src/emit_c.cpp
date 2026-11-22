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
#include <unordered_map>
#include <functional>
#include <map>
#include <set>
#include <filesystem>

namespace coax {

static const char *STRICT_MATH_SEED =
#include "../bootstrap/strict-math.inc"
;
static const char *TRANSACTION_SEED =
#include "../bootstrap/transaction-runtime.inc"
;

static std::string transaction_runtime() {
  std::string s=TRANSACTION_SEED;
  for(const auto &[from,to]:{std::pair{"coax_","coax_txr_"},{"TX_","coax_txr_"},{"COAX_","COAX_TXR_"}}){
    size_t at=0;while((at=s.find(from,at))!=std::string::npos){s.replace(at,strlen(from),to);at+=strlen(to);}
  }
  for(const char *signature:{"_Bool coax_txr_abort(","int64_t coax_txr_bytes(","uint32_t coax_txr_effects(","_Bool coax_txr_copy(","_Bool coax_txr_covers(","_Bool coax_txr_depth("}){
    size_t at=0;std::string needle="\n"+std::string(signature);
    while((at=s.find(needle,at))!=std::string::npos){s.insert(at+1,"static ");at+=8;}
  }
  s+=R"COAXUNDO(
/* The envelope owns allocations and observed addresses. Coax supplies the
   extent checks, before-image copies, restoration and return decision. */
typedef struct coax_tx_before {
  struct coax_tx_before *next; void *address; int64_t bytes; const void *type; _Bool saved;
  unsigned char before[];
} coax_tx_before;
typedef struct { coax_tx_before *head; unsigned depth; } coax_tx_context;
static _Bool coax_tx_save(coax_tx_context *tx, void *address, int64_t count, int64_t width, int line, const void *type) {
  int64_t bytes=coax_txr_bytes(count,width);
  if(bytes<0 || (uint64_t)bytes>SIZE_MAX-sizeof(coax_tx_before))coax_trap("transaction span overflow",line);
  if(!bytes)return 0;
  _Bool covered=0;
  for(coax_tx_before *at=tx->head;at;at=at->next){
    if(type && at->type==type && at->address==address && at->bytes==bytes)return 0;
    if(at->saved && coax_txr_covers((uint64_t)(uintptr_t)at->address,at->bytes,(uint64_t)(uintptr_t)address,bytes))covered=1;
  }
  if(!type && covered)return 0;
  coax_tx_before *entry=(coax_tx_before *)malloc(sizeof(*entry)+(covered?0:(size_t)bytes));
  if(!entry)coax_trap("transaction memory exhausted",line);
  entry->address=address;entry->bytes=bytes;entry->next=tx->head;entry->type=type;entry->saved=!covered;
  if(!covered && !coax_txr_copy((coax_txr_rv_u8){(const uint8_t *)address,bytes},(coax_txr_v_u8){entry->before,bytes}))coax_trap("transaction copy refused",line);
  tx->head=entry;return 1;
}
static void coax_tx_close(coax_tx_context *tx, _Bool rollback) {
  while(tx->head){coax_tx_before *entry=tx->head;tx->head=entry->next;
    if(rollback && entry->saved && !coax_txr_copy((coax_txr_rv_u8){entry->before,entry->bytes},(coax_txr_v_u8){(uint8_t *)entry->address,entry->bytes}))coax_trap("transaction restore refused",0);
    free(entry);
  }
}
)COAXUNDO";
  return s;
}

// Isolate the seed's complete generated runtime from the caller's symbols.
// Every definition is private to this translation unit, including exports.
static std::string strict_math_runtime() {
  std::string s=R"COAXNOTICE(/* Fixed Coax math, derived from fdlibm.
Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
Developed at SunSoft, a Sun Microsystems, Inc. business.
Permission to use, copy, modify, and distribute this software is freely
granted, provided that this notice is preserved. */
)COAXNOTICE";
  s+=STRICT_MATH_SEED;
  for (const auto &[from,to] : {std::pair{"coax_","coax_sm_"}, {"SM_","coax_sm_"}, {"COAX_","COAX_SM_"}}) {
    size_t at=0;
    while((at=s.find(from,at))!=std::string::npos) {s.replace(at,strlen(from),to);at+=strlen(to);}
  }
  size_t at=0;
  while((at=s.find("\ndouble coax_sm_",at))!=std::string::npos) {s.insert(at+1,"static ");at+=8;}
  for (const char *name : {"sin","cos","exp","log"})
    s += "\nstatic float coax_sm_"+std::string(name)+"f(float x) { return (float)coax_sm_"+name+"((double)x); }\n";
  return s;
}

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
static const char *ZERO_RUNTIME = R"COAXRT(
/* Observable stores on native and wasm. Allocation/lifetime remain caller
   premises; an empty view never offsets or dereferences a null pointer. */
static void coax_secure_zero(void *data, int64_t count, size_t width, int32_t line) {
  if(count<0 || (uint64_t)count>SIZE_MAX/width || (count && !data))coax_trap("invalid secure_zero view",line);
  volatile unsigned char *p=(volatile unsigned char *)data;
  size_t bytes=(size_t)count*width;
  for(size_t i=0;i<bytes;i++)p[i]=0;
}
)COAXRT";
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

// Emitted only into programs with an #inline procedure, so the seeds stay as they are.
static const char *INLINE_RUNTIME = R"COAXRT(/* #inline (spec section 7.1): the C compiler copies the body into each
   caller. wasm32 keeps the call: a 128-bit product is four 32-bit ones
   there, and a module would grow by the whole body at every call. */
#if defined(__GNUC__) && !defined(__wasm__)
#define COAX_INLINE static inline __attribute__((always_inline))
#else
#define COAX_INLINE static
#endif
)COAXRT";

/* Loop versioning (Emitter::versioned_for): a loop whose indexes are affine
   in for-counters with invariant bounds first computes each index's
   interval, and every subexpression's, in exact int64 endpoints. When every
   endpoint fits and every index is inside its view or array, a copy of the
   loop without those checks runs; otherwise the checked loop runs, so a trap
   happens exactly where it would have. */
static const char *IV_RUNTIME = R"COAXRT(
typedef struct { int64_t lo, hi; } coax_iv;
static inline __attribute__((unused)) int coax_iv_add(coax_iv a, coax_iv b, coax_iv *r) {
  return !__builtin_add_overflow(a.lo, b.lo, &r->lo) && !__builtin_add_overflow(a.hi, b.hi, &r->hi);
}
static inline __attribute__((unused)) int coax_iv_sub(coax_iv a, coax_iv b, coax_iv *r) {
  return !__builtin_sub_overflow(a.lo, b.hi, &r->lo) && !__builtin_sub_overflow(a.hi, b.lo, &r->hi);
}
static inline __attribute__((unused)) int coax_iv_mul(coax_iv a, coax_iv b, coax_iv *r) {
  int64_t p[4];
  if (__builtin_mul_overflow(a.lo, b.lo, &p[0]) || __builtin_mul_overflow(a.lo, b.hi, &p[1]) ||
      __builtin_mul_overflow(a.hi, b.lo, &p[2]) || __builtin_mul_overflow(a.hi, b.hi, &p[3])) return 0;
  r->lo = p[0]; r->hi = p[0];
  for (int i = 1; i < 4; i++) { if (p[i] < r->lo) r->lo = p[i]; if (p[i] > r->hi) r->hi = p[i]; }
  return 1;
}
)COAXRT";

// add_checked, sub_checked, mul_checked: 1 with the result in *r when it fits
// the type, else 0 with *r untouched; add_sat, sub_sat, mul_sat: the result,
// or the bound it passed. One definition for every build, so a debug build, a
// release build and #run agree (spec section 9).
static const char *INTOPS_RUNTIME = R"COAXRT(
#define COAX_INTOPS_CHECKED(T, N)                                                                          \
  static inline __attribute__((unused)) int coax_add_checked_##N(T a, T b, T *r) { T v; if (__builtin_add_overflow(a, b, &v)) return 0; *r = v; return 1; } \
  static inline __attribute__((unused)) int coax_sub_checked_##N(T a, T b, T *r) { T v; if (__builtin_sub_overflow(a, b, &v)) return 0; *r = v; return 1; } \
  static inline __attribute__((unused)) int coax_mul_checked_##N(T a, T b, T *r) { T v; if (__builtin_mul_overflow(a, b, &v)) return 0; *r = v; return 1; }
#define COAX_INTOPS_SIGNED(T, N, MIN, MAX)                                                                 \
  COAX_INTOPS_CHECKED(T, N)                                                                                \
  static inline __attribute__((unused)) T coax_add_sat_##N(T a, T b) { T v; if (!__builtin_add_overflow(a, b, &v)) return v; return b < 0 ? MIN : MAX; } \
  static inline __attribute__((unused)) T coax_sub_sat_##N(T a, T b) { T v; if (!__builtin_sub_overflow(a, b, &v)) return v; return b < 0 ? MAX : MIN; } \
  static inline __attribute__((unused)) T coax_mul_sat_##N(T a, T b) { T v; if (!__builtin_mul_overflow(a, b, &v)) return v; return (a < 0) != (b < 0) ? MIN : MAX; }
#define COAX_INTOPS_UNSIGNED(T, N, MAX)                                                                    \
  COAX_INTOPS_CHECKED(T, N)                                                                                \
  static inline __attribute__((unused)) T coax_add_sat_##N(T a, T b) { T v; if (!__builtin_add_overflow(a, b, &v)) return v; return MAX; } \
  static inline __attribute__((unused)) T coax_sub_sat_##N(T a, T b) { T v; if (!__builtin_sub_overflow(a, b, &v)) return v; return 0; } \
  static inline __attribute__((unused)) T coax_mul_sat_##N(T a, T b) { T v; if (!__builtin_mul_overflow(a, b, &v)) return v; return MAX; }
COAX_INTOPS_SIGNED(int8_t, s8, INT8_MIN, INT8_MAX)
COAX_INTOPS_SIGNED(int16_t, s16, INT16_MIN, INT16_MAX)
COAX_INTOPS_SIGNED(int32_t, s32, INT32_MIN, INT32_MAX)
COAX_INTOPS_SIGNED(int64_t, s64, INT64_MIN, INT64_MAX)
COAX_INTOPS_UNSIGNED(uint8_t, u8, UINT8_MAX)
COAX_INTOPS_UNSIGNED(uint16_t, u16, UINT16_MAX)
COAX_INTOPS_UNSIGNED(uint32_t, u32, UINT32_MAX)
COAX_INTOPS_UNSIGNED(uint64_t, u64, UINT64_MAX)
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
  return p && (is_mma_primitive(p) || str_eq(p->name, "am_sg_mma_64x32x32") || str_eq(p->name, "am_sg_mma_f16_64x32x32") || str_eq(p->name, "am_sg_spill_64x32") || str_eq(p->name, "am_sg_zero_64x32") || str_eq(p->name, "am_matmul_bf16") || str_eq(p->name, "am_matmul_bf16_x16") || str_eq(p->name, "am_matmul_iq4") || str_eq(p->name, "am_matmul_iq4_bf16") || str_eq(p->name, "am_dot_f16_tg_write") || str_eq(p->name, "am_dot_f16_tg_write_rms") || str_eq(p->name, "am_rmsnorm_f32") || str_eq(p->name, "am_ctrl_up_apply_f16") || str_eq(p->name, "am_ctrl_up_apply_gf") || str_eq(p->name, "am_ctrl_up_apply_gf_norm") || str_eq(p->name, "am_ctrl_up_apply_gf_rm_norm") || str_eq(p->name, "am_ctrl_up_apply_gf_batch") || str_eq(p->name, "am_mix_fused") || str_eq(p->name, "am_mix_batch_f32") || str_eq(p->name, "am_router_top8") || str_eq(p->name, "am_conv_kv_both") || str_eq(p->name, "am_attn_gqa_1pass") || str_eq(p->name, "am_kv_attn_gqa_1pass") || str_eq(p->name, "am_attn_scores_m4") || str_eq(p->name, "am_attn_values_m4") || str_eq(p->name, "am_dot_q4_conv_2_write") || str_eq(p->name, "am_dot_q4_2_write_ex") || str_eq(p->name, "am_dot_q4s_2_write_ex") || str_eq(p->name, "am_dot_q4s_conv_2_write") || str_eq(p->name, "am_dot_q4s_1row_tg") || str_eq(p->name, "am_dot_q4s_qrgkv_sdyn") || str_eq(p->name, "am_moe_gu_iq4_tg") || str_eq(p->name, "am_moe_gu_q4_tg") || str_eq(p->name, "am_moe_down_iq4_tg") || str_eq(p->name, "am_moe_down_q4_tg") || str_eq(p->name, "am_moe_gu_q4s_tg") || str_eq(p->name, "am_moe_down_q4s_tg") || str_eq(p->name, "am_moe_gu_iq4s_tg") || str_eq(p->name, "am_moe_down_iq4s_tg") || str_eq(p->name, "am_dot_q4_qrgkv") || str_eq(p->name, "am_dot_q4s_2_write_nb") || str_eq(p->name, "am_dot_q4s_8row_nb") || str_eq(p->name, "am_dot_q4s_mma32_nb") || str_eq(p->name, "am_dot_q8_mma32_nb") || str_eq(p->name, "am_dot_q4s_qrgkv_sdyn_nb") || str_eq(p->name, "am_kv_attn_gqa_1pass_nb") || str_eq(p->name, "am_dot_q4s_conv_nb") || str_eq(p->name, "am_moe_plan_nb") || str_eq(p->name, "am_moe_gu_iq4_nb") || str_eq(p->name, "am_moe_gu_q4s_nb") || str_eq(p->name, "am_moe_gu_iq4s_nb") || str_eq(p->name, "am_moe_down_iq4_nb") || str_eq(p->name, "am_moe_down_q4s_nb") || str_eq(p->name, "am_moe_down_iq4s_nb") || str_eq(p->name, "am_moe_down_p1_iq4s_nb") || str_eq(p->name, "am_moe_down_p1_q4s_nb") || str_eq(p->name, "am_conv_commit_nb") || str_eq(p->name, "am_dot_q4s_small_nb") || str_eq(p->name, "am_mix_fused_nb") || str_eq(p->name, "am_ctrl_up_apply_gf_rm_nb") || str_eq(p->name, "am_dspark_fold_logits_s1"));
}
static int am_tg_prim_size(const ProcDecl *p) {
  if (!p) return 0;
  if (str_eq(p->name, "am_attn_scores_m4")) return 7680;
  if (str_eq(p->name, "am_attn_values_m4")) return 4096;
  if (str_eq(p->name, "am_dot_q8_mma32_nb")) return 2560;
  if (str_eq(p->name, "am_kv_attn_gqa_1pass_nb")) return 2336;
  if (str_eq(p->name, "am_kv_attn_gqa_1pass")) return 1536;
  if (str_eq(p->name, "am_dot_q4s_mma32_nb")) return 1280;
  if (str_eq(p->name, "am_ctrl_up_apply_gf_batch")) return 1024;
  if (str_eq(p->name, "am_dot_f16_tg_write") || str_eq(p->name, "am_dot_f16_tg_write_rms") || str_eq(p->name, "am_rmsnorm_f32") || str_eq(p->name, "am_ctrl_up_apply_f16") || str_eq(p->name, "am_mix_fused") || str_eq(p->name, "am_mix_fused_nb") || str_eq(p->name, "am_mix_batch_f32") || str_eq(p->name, "am_attn_gqa_1pass")) return 640;
  if (str_eq(p->name, "am_ctrl_up_apply_gf_rm_nb")) return 544;
  if (str_eq(p->name, "am_moe_plan_nb")) return 512;
  if (str_eq(p->name, "am_moe_gu_iq4_tg") || str_eq(p->name, "am_moe_gu_iq4s_tg")) return 260;
  if (str_eq(p->name, "am_moe_down_iq4_tg") || str_eq(p->name, "am_moe_down_iq4s_tg") || str_eq(p->name, "am_moe_gu_iq4_nb") || str_eq(p->name, "am_moe_gu_iq4s_nb") || str_eq(p->name, "am_moe_down_iq4_nb") || str_eq(p->name, "am_moe_down_iq4s_nb") || str_eq(p->name, "am_moe_down_p1_iq4s_nb")) return 256;
  if (str_eq(p->name, "am_dspark_fold_logits_s1")) return 96;
  if (str_eq(p->name, "am_dot_q4s_small_nb")) return 40;
  if (str_eq(p->name, "am_dot_q4s_1row_tg")) return 16;
  if (str_eq(p->name, "am_ctrl_up_apply_gf_rm_norm") || str_eq(p->name, "am_moe_gu_q4_tg") || str_eq(p->name, "am_moe_gu_q4s_tg")) return 4;
  return 0;
}
static int expr_am_tg_size(const Expr *e) {
  if (!e) return 0;
  int r = (e->kind == ExprKind::call) ? am_tg_prim_size(e->proc) : 0;
  r = std::max(r, expr_am_tg_size(e->lhs));
  r = std::max(r, expr_am_tg_size(e->rhs));
  r = std::max(r, expr_am_tg_size(e->third));
  for (const Expr *a : e->args) r = std::max(r, expr_am_tg_size(a));
  return r;
}
static int stmt_am_tg_size(const Stmt *s) {
  if (!s) return 0;
  int r = std::max({expr_am_tg_size(s->expr), expr_am_tg_size(s->expr2), expr_am_tg_size(s->target)});
  for (const Stmt *c : s->body) r = std::max(r, stmt_am_tg_size(c));
  r = std::max(r, stmt_am_tg_size(s->then_));
  return std::max(r, stmt_am_tg_size(s->else_));
}
static int proc_am_tg_size(const ProcDecl *p) { return p ? stmt_am_tg_size(p->body) : 0; }
static bool proc_uses_am_tg(const ProcDecl *p) { return proc_am_tg_size(p) > 0; }
// The integer value of a constant expression (a literal or a named constant).
static int64_t constant_of_expr(const Expr *e) {
  if (e && e->kind == ExprKind::ident && e->constant && e->constant->expr) e = e->constant->expr;
  return e && e->kind == ExprKind::int_lit ? (int64_t)e->int_value : -1;
}

// ---- 32-bit integers in Metal kernels ----
// An Apple GPU adds, multiplies and compares 64-bit integers in several
// 32-bit steps, so `coax metal-compute` emits a second body of each kernel
// whose int locals, loop counters and operations are 32-bit wherever a
// bound proves the 64-bit value fits. The entry runs that body only when
// the launch meets the bounds the proof assumed: the item count and every
// int parameter the proof relies on at most NARROW_BOUND in magnitude, and,
// when a 32-bit index checks a view parameter, every view parameter's
// count below 2^31. Otherwise it runs the 64-bit body. Both bodies compute
// the same values, so the result never depends on which one ran.
//
// The bounds are intervals, flow-insensitive: a local's interval holds
// every value any of its assignments can store (iterated to a fixed point,
// a local that keeps growing is unbounded), a loop counter's its bounds',
// a thread loop's counter [0, T - 1]. An interval carries the parameters
// it assumed (mask: bit 0 the item count, bit i parameter i, bit 63 the
// view counts).
static const int64_t NARROW_BOUND = (int64_t)1 << 20;
struct NIv {
  int64_t lo = 1, hi = 0;  // empty until assigned
  uint64_t mask = 0;
  bool empty() const { return lo > hi; }
  bool fits32() const { return !empty() && lo >= INT32_MIN && hi <= INT32_MAX; }
  bool operator==(const NIv &o) const { return lo == o.lo && hi == o.hi && mask == o.mask; }
};
static NIv niv_full() { return {INT64_MIN, INT64_MAX, 0}; }
static NIv niv_make(__int128 lo, __int128 hi, uint64_t mask) {
  if (lo < INT64_MIN || hi > INT64_MAX) return niv_full();
  NIv r; r.lo = (int64_t)lo; r.hi = (int64_t)hi; r.mask = mask;
  if (r.lo == INT64_MIN && r.hi == INT64_MAX) r.mask = 0;
  return r;
}
static NIv niv_join(const NIv &a, const NIv &b) {
  if (a.empty()) return b;
  if (b.empty()) return a;
  return niv_make(std::min(a.lo, b.lo), std::max(a.hi, b.hi), a.mask | b.mask);
}
static NIv niv_range(Type t) {
  Type b = t->kind == TypeKind::enum_ ? t->elem : t;
  if (!b || b->kind != TypeKind::integer || b->bits >= 64) return b && !b->is_signed && b->bits >= 64 ? niv_make(0, INT64_MAX, 0) : niv_full();
  if (b->is_signed) return niv_make(-((__int128)1 << (b->bits - 1)), ((__int128)1 << (b->bits - 1)) - 1, 0);
  return niv_make(0, ((__int128)1 << b->bits) - 1, 0);
}
static NIv niv_binary(Tok op, const NIv &a, const NIv &b) {
  if (a.empty() || b.empty()) return NIv{};
  uint64_t m = a.mask | b.mask;
  __int128 al = a.lo, ah = a.hi, bl = b.lo, bh = b.hi;
  switch (op) {
    case Tok::plus: return niv_make(al + bl, ah + bh, m);
    case Tok::minus: return niv_make(al - bh, ah - bl, m);
    case Tok::star: {
      // Each corner fits in 127 bits: the operands are 64-bit.
      __int128 p[4] = {al * bl, al * bh, ah * bl, ah * bh};
      return niv_make(std::min(std::min(p[0], p[1]), std::min(p[2], p[3])), std::max(std::max(p[0], p[1]), std::max(p[2], p[3])), m);
    }
    case Tok::slash: case Tok::percent: {
      // A divisor of zero traps, and the checked operation then yields 0.
      if (b.lo < 0) {
        // Either sign: |a / b| <= |a|, and |a % b| < |b| with a's sign.
        if (a.lo == INT64_MIN || b.lo == INT64_MIN) return niv_full();
        __int128 A = std::max(-al, ah), M = std::max(-bl, bh);
        if (op == Tok::slash) return niv_make(-A, A, m);
        return niv_make(a.lo >= 0 ? 0 : -std::min(A, M - 1), a.hi <= 0 ? 0 : std::min(A, M - 1), m);
      }
      if (b.hi == 0) return niv_make(0, 0, m);
      NIv r;
      if (bl < 1) { bl = 1; r = niv_make(0, 0, m); }
      if (op == Tok::slash) {
        __int128 q[4] = {al / bl, al / bh, ah / bl, ah / bh};  // truncation toward zero, as C and Coax
        return niv_join(r, niv_make(std::min(std::min(q[0], q[1]), std::min(q[2], q[3])), std::max(std::max(q[0], q[1]), std::max(q[2], q[3])), m));
      }
      if (a.lo >= 0) return niv_join(r, niv_make(0, std::min(ah, bh - 1), m));  // the remainder takes the dividend's sign
      if (a.hi <= 0) return niv_join(r, niv_make(std::max(al, -(bh - 1)), 0, m));
      return niv_join(r, niv_make(-(bh - 1), bh - 1, m));
    }
    case Tok::amp:
      if (a.lo >= 0 && b.lo >= 0) return niv_make(0, std::min(ah, bh), m);
      if (a.lo >= 0) return niv_make(0, ah, a.mask);
      if (b.lo >= 0) return niv_make(0, bh, b.mask);
      return niv_full();
    case Tok::shr:
      if (b.lo < 0 || b.hi > 63) return niv_full();
      if (a.lo >= 0) return niv_make(al >> bh, ah >> bl, m);
      return niv_make(al >> bl, a.hi >= 0 ? ah >> bl : ah >> bh, m);
    default: return niv_full();
  }
}

struct Narrow {
  const ProcDecl *proc = nullptr;
  int64_t threads = 32;                            // a thread loop's counter is below this
  std::map<const Expr *, const Stmt *> local;      // an identifier's int local (a var or a for counter)
  std::map<const Expr *, int32_t> param;           // an identifier's parameter
  std::map<const Expr *, const Stmt *> vlocal;     // an identifier's view local
  std::set<const Stmt *> vassigned;                // view locals assigned after their declaration
  // A call to a procedure whose body is one `return` of an int: that
  // expression's interval, with the callee's parameters at the arguments'.
  std::map<const Expr *, int32_t> callee_param;
  std::set<const ProcDecl *> callees_resolved;
  std::vector<std::vector<NIv>> frames;
  std::map<const Stmt *, std::vector<const Stmt *>> assigns;  // a local's assignments
  std::vector<const Stmt *> loops;                 // the loops around the statement being resolved
  std::map<const Stmt *, size_t> depth;            // a local's: how many loops are around its declaration
  std::map<const Stmt *, std::vector<const Stmt *>> around;  // an assignment's loops
  std::set<int32_t> assigned_params;
  std::map<const Stmt *, NIv> var;                 // each int local's interval
  std::map<const Expr *, NIv> iv;                  // each integer expression's (the last pass)
  std::set<const Stmt *> narrow_vars;              // int locals and counters the 32-bit body declares int32_t
  uint64_t guard = 0;                              // what the 32-bit body assumed
  bool any = false;
  bool recording = false;

  bool is_local(const Stmt *s) const { return (s->kind == StmtKind::var || s->kind == StmtKind::for_range) && s->type == T_INT; }

  // Identifiers to their declarations; each local's assignments.
  void resolve_expr(const Expr *e, std::vector<std::map<std::string, const Stmt *>> &scopes, const std::map<std::string, int32_t> &params) {
    if (!e) return;
    if (e->kind == ExprKind::ident && !e->constant) {
      std::string n(e->text.data, (size_t)e->text.count);
      bool found = false;
      for (size_t i = scopes.size(); i-- > 0 && !found;) {
        auto f = scopes[i].find(n);
        if (f != scopes[i].end()) {
          found = true;
          if (f->second && is_local(f->second)) local[e] = f->second;
          else if (f->second) vlocal[e] = f->second;
        }
      }
      if (!found) { auto p = params.find(n); if (p != params.end()) param[e] = p->second; }
    }
    resolve_expr(e->lhs, scopes, params); resolve_expr(e->rhs, scopes, params); resolve_expr(e->third, scopes, params);
    for (int32_t k = 0; k < e->args.count; k++) resolve_expr(e->args[k], scopes, params);
    for (int32_t k = 0; k < e->fields.count; k++) resolve_expr(e->fields[k].value, scopes, params);
  }
  void declare(Str name, const Stmt *s, std::vector<std::map<std::string, const Stmt *>> &scopes) {
    // Any declaration hides a parameter of that name; int scalars and views are tracked.
    bool view = s && s->kind == StmtKind::var && s->type->kind == TypeKind::view;
    scopes.back()[std::string(name.data, (size_t)name.count)] = s && (is_local(s) || view) ? s : nullptr;
  }
  void resolve(const Stmt *s, std::vector<std::map<std::string, const Stmt *>> &scopes, const std::map<std::string, int32_t> &params) {
    if (!s) return;
    switch (s->kind) {
      case StmtKind::block:
        scopes.push_back({});
        for (int32_t k = 0; k < s->body.count; k++) resolve(s->body[k], scopes, params);
        scopes.pop_back();
        break;
      case StmtKind::var:
        resolve_expr(s->expr, scopes, params);
        declare(s->name, s, scopes);
        if (is_local(s)) { assigns[s]; depth[s] = loops.size(); }
        break;
      case StmtKind::assign:
        resolve_expr(s->target, scopes, params); resolve_expr(s->expr, scopes, params);
        around[s] = loops;
        if (s->target && s->target->kind == ExprKind::ident) {
          auto l = local.find(s->target);
          if (l != local.end()) assigns[l->second].push_back(s);
          auto p = param.find(s->target);
          if (p != param.end()) assigned_params.insert(p->second);
          auto v = vlocal.find(s->target);
          if (v != vlocal.end()) vassigned.insert(v->second);
        }
        break;
      case StmtKind::for_range:
        resolve_expr(s->expr, scopes, params); resolve_expr(s->expr2, scopes, params);
        scopes.push_back({});
        declare(s->name, s, scopes);
        if (is_local(s)) assigns[s];
        loops.push_back(s);
        resolve(s->then_, scopes, params);
        loops.pop_back();
        scopes.pop_back();
        break;
      case StmtKind::for_each:
        resolve_expr(s->expr, scopes, params);
        scopes.push_back({});
        declare(s->name, nullptr, scopes);
        if (s->name2.count) declare(s->name2, nullptr, scopes);
        loops.push_back(s);
        resolve(s->then_, scopes, params);
        loops.pop_back();
        scopes.pop_back();
        break;
      case StmtKind::while_:
        resolve_expr(s->expr, scopes, params);
        loops.push_back(s);
        resolve(s->then_, scopes, params);
        loops.pop_back();
        break;
      default:
        resolve_expr(s->expr, scopes, params); resolve_expr(s->expr2, scopes, params); resolve_expr(s->target, scopes, params);
        resolve(s->then_, scopes, params); resolve(s->else_, scopes, params);
        break;
    }
  }

  NIv param_iv(int32_t i) const {
    Type t = proc->params[i].type;
    if (t != T_INT) return niv_range(t);
    if (assigned_params.count(i)) return niv_full();
    if (i == 0) return niv_make(0, NARROW_BOUND - 1, 1);  // the item: below the item count
    if (i >= 63) return niv_full();
    return niv_make(0, NARROW_BOUND, (uint64_t)1 << i);
  }
  NIv eval(const Expr *e) {
    NIv r = eval_inner(e);
    if (recording && e && is_int(e->type)) iv[e] = r;
    return r;
  }
  NIv eval_inner(const Expr *e) {
    if (!e || !is_int(e->type)) { if (e && recording) eval_children(e); return niv_full(); }
    if (e->type != T_INT) { eval_children(e); return e->kind == ExprKind::int_lit ? niv_make((int64_t)e->int_value, (int64_t)e->int_value, 0) : niv_range(e->type); }
    switch (e->kind) {
      case ExprKind::int_lit: return niv_make((int64_t)e->int_value, (int64_t)e->int_value, 0);
      case ExprKind::ident: {
        if (e->constant) {
          const Expr *c = e->constant->expr;
          return c && c->kind == ExprKind::int_lit ? niv_make((int64_t)c->int_value, (int64_t)c->int_value, 0) : niv_full();
        }
        auto l = local.find(e);
        if (l != local.end()) { auto v = var.find(l->second); return v == var.end() ? niv_full() : v->second; }
        if (!frames.empty()) {
          auto c = callee_param.find(e);
          return c == callee_param.end() || c->second >= (int32_t)frames.back().size() ? niv_full() : frames.back()[(size_t)c->second];
        }
        auto p = param.find(e);
        if (p != param.end()) return param_iv(p->second);
        return niv_full();
      }
      case ExprKind::call: {
        const ProcDecl *f = e->proc;
        std::vector<NIv> args;
        for (int32_t k = 0; k < e->args.count; k++) args.push_back(eval(e->args[k]));
        if (!f || f->foreign || !f->body || f->body->body.count != 1 || frames.size() >= 4) return niv_full();
        const Stmt *r = f->body->body[0];
        if (r->kind != StmtKind::ret || !r->expr || r->expr->type != T_INT || f->params.count != e->args.count) return niv_full();
        if (!callees_resolved.count(f)) {
          callees_resolved.insert(f);
          std::map<std::string, int32_t> ps;
          for (int32_t i = 0; i < f->params.count; i++) ps[std::string(f->params[i].name.data, (size_t)f->params[i].name.count)] = i;
          std::function<void(const Expr *)> mark = [&](const Expr *x) {
            if (!x) return;
            if (x->kind == ExprKind::ident && !x->constant) {
              auto q = ps.find(std::string(x->text.data, (size_t)x->text.count));
              if (q != ps.end()) callee_param[x] = q->second;
            }
            mark(x->lhs); mark(x->rhs); mark(x->third);
            for (int32_t k = 0; k < x->args.count; k++) mark(x->args[k]);
          };
          mark(r->expr);
        }
        bool saved = recording;
        recording = false;  // the callee's expressions are not this kernel's
        frames.push_back(args);
        NIv v = eval(r->expr);
        frames.pop_back();
        recording = saved;
        return v;
      }
      case ExprKind::binary: {
        NIv a = eval(e->lhs), b = eval(e->rhs);
        return niv_binary(e->op, a, b);
      }
      case ExprKind::unary: {
        NIv a = eval(e->lhs);
        if (e->op != Tok::minus || a.empty()) return a.empty() ? a : niv_full();
        return niv_make(-(__int128)a.hi, -(__int128)a.lo, a.mask);
      }
      case ExprKind::cast: {
        NIv a = eval(e->lhs);
        if (!is_int(e->lhs->type)) return niv_full();
        if (e->trunc && e->lhs->type->bits > 64) return niv_full();
        if (e->lhs->type == T_INT || (e->lhs->type->is_signed && e->lhs->type->bits <= 64) || e->lhs->type->bits < 64) return a;
        return niv_full();
      }
      case ExprKind::ifx: {
        eval(e->lhs);
        return niv_join(eval(e->rhs), eval(e->third));
      }
      case ExprKind::field:
        eval_children(e);
        if (e->field_index == -2 && e->lhs->type->kind == TypeKind::view) {
          if (e->lhs->kind == ExprKind::ident && param.count(e->lhs) && !assigned_params.count(param.at(e->lhs)))
            return niv_make(0, INT32_MAX, (uint64_t)1 << 63);
          return niv_make(0, INT64_MAX, 0);
        }
        if (e->field_index == -2 && e->lhs->type->kind == TypeKind::array) return niv_make(e->lhs->type->length, e->lhs->type->length, 0);
        return niv_full();
      default:
        eval_children(e);
        return niv_full();
    }
  }
  void eval_children(const Expr *e) {
    if (!recording || !e) return;
    eval(e->lhs); eval(e->rhs); eval(e->third);
    for (int32_t k = 0; k < e->args.count; k++) eval(e->args[k]);
    for (int32_t k = 0; k < e->fields.count; k++) eval(e->fields[k].value);
  }
  // Every value a local's assignments can store, under the current intervals.
  NIv defs(const Stmt *l) {
    NIv r;
    if (l->kind == StmtKind::for_range) {
      if (g_warp.thread_loops.count(l)) return niv_make(0, g_warp.thread_loops.at(l) - 1, 0);
      if (g_warp.lane_loops.count(l)) return niv_make(0, 31, 0);
      NIv lo = eval(l->expr), hi = eval(l->expr2);
      if (lo.empty() || hi.empty()) return NIv{};
      r = niv_make(lo.lo, std::max((__int128)lo.lo, (__int128)hi.hi - 1), lo.mask | hi.mask);
    } else if (l->expr) r = eval(l->expr);
    else r = l->uninit ? niv_full() : niv_make(0, 0, 0);
    // Increments (v += e, v = v + e, e not negative and not reading v) add at
    // most e's largest value once per pass of the loops between v's
    // declaration and the increment: a thread loop's body runs once, a
    // for loop's its bounds' count of times, a while or for-each loop
    // without a bound.
    __int128 added = 0;
    uint64_t added_mask = 0;
    bool bounded = true;
    std::vector<const Stmt *> rest;
    for (const Stmt *a : assigns[l]) {
      const Expr *inc = increment(a, l);
      if (!inc || l->kind != StmtKind::var) { rest.push_back(a); continue; }
      NIv e = eval(inc);
      if (e.empty()) continue;
      __int128 trips = 1;
      const std::vector<const Stmt *> &ls = around[a];
      for (size_t k = depth[l]; k < ls.size() && bounded; k++) {
        const Stmt *L = ls[k];
        if (g_warp.thread_loops.count(L) || g_warp.lane_loops.count(L)) continue;
        if (L->kind != StmtKind::for_range) { bounded = false; break; }
        NIv lo = eval(L->expr), hi = eval(L->expr2);
        if (lo.empty() || hi.empty() || !lo.fits32() || !hi.fits32()) { bounded = false; break; }
        trips *= std::max((__int128)0, (__int128)hi.hi - lo.lo);
        added_mask |= lo.mask | hi.mask;
        if (trips > ((__int128)1 << 40)) bounded = false;
      }
      if (e.lo < 0 || !e.fits32()) bounded = false;
      if (!bounded) break;
      added += trips * e.hi;
      added_mask |= e.mask;
    }
    if (!bounded) { rest = assigns[l]; added = 0; added_mask = 0; }
    for (const Stmt *a : rest) {
      NIv v = eval(a->expr);
      if (a->op != Tok::equal) {
        static const std::map<Tok, Tok> base = {{Tok::plus_equal, Tok::plus}, {Tok::minus_equal, Tok::minus}, {Tok::star_equal, Tok::star},
                                                {Tok::slash_equal, Tok::slash}, {Tok::percent_equal, Tok::percent}, {Tok::amp_equal, Tok::amp},
                                                {Tok::shr_equal, Tok::shr}};
        auto b = base.find(a->op);
        auto cur = var.find(l);
        v = b == base.end() || cur == var.end() ? niv_full() : niv_binary(b->second, cur->second, v);
      }
      r = niv_join(r, v);
    }
    if (added && !r.empty()) r = niv_make(r.lo, (__int128)r.hi + added, r.mask | added_mask);
    return r;
  }
  bool reads(const Expr *e, const Stmt *l) const {
    if (!e) return false;
    if (e->kind == ExprKind::ident) { auto f = local.find(e); if (f != local.end() && f->second == l) return true; }
    if (reads(e->lhs, l) || reads(e->rhs, l) || reads(e->third, l)) return true;
    for (int32_t k = 0; k < e->args.count; k++) if (reads(e->args[k], l)) return true;
    for (int32_t k = 0; k < e->fields.count; k++) if (reads(e->fields[k].value, l)) return true;
    return false;
  }
  // The amount an assignment adds to l, when it is v += e or v = v + e (e + v).
  const Expr *increment(const Stmt *a, const Stmt *l) const {
    if (a->op == Tok::plus_equal) return reads(a->expr, l) ? nullptr : a->expr;
    if (a->op != Tok::equal || !a->expr || a->expr->kind != ExprKind::binary || a->expr->op != Tok::plus) return nullptr;
    const Expr *x = a->expr->lhs, *y = a->expr->rhs;
    auto self = [&](const Expr *v) { auto f = v->kind == ExprKind::ident ? local.find(v) : local.end(); return f != local.end() && f->second == l; };
    if (self(x) && !reads(y, l)) return y;
    if (self(y) && !reads(x, l)) return x;
    return nullptr;
  }
  void walk(const Stmt *s) {
    if (!s) return;
    eval(s->expr); eval(s->expr2); eval(s->target);
    for (int32_t k = 0; k < s->body.count; k++) walk(s->body[k]);
    walk(s->then_); walk(s->else_);
  }

  void analyze(const ProcDecl *p) {
    proc = p;
    std::map<std::string, int32_t> params;
    for (int32_t i = 0; i < p->params.count; i++) params[std::string(p->params[i].name.data, (size_t)p->params[i].name.count)] = i;
    std::vector<std::map<std::string, const Stmt *>> scopes;
    resolve(p->body, scopes, params);
    // A fixed point: intervals only grow, and a local that grew six times is unbounded.
    // Every local starts empty (no value yet); one still empty at the fixed
    // point becomes unbounded, and the iteration goes on from there.
    std::map<const Stmt *, int> grew;
    for (auto &[l, unused] : assigns) { (void)unused; var[l] = NIv{}; }
    for (bool again = true; again;) {
      for (bool changed = true; changed;) {
        changed = false;
        for (auto &[l, unused] : assigns) {
          (void)unused;
          NIv n = niv_join(var[l], defs(l));
          if (n == var[l]) continue;
          if (++grew[l] > 6) n = niv_full();
          if (n == var[l]) continue;
          var[l] = n;
          changed = true;
        }
      }
      again = false;
      for (auto &[l, v] : var) if (v.empty()) { v = niv_full(); again = true; }
    }
    recording = true;
    walk(p->body);
    recording = false;
    for (auto &[l, v] : var) {
      if (!v.fits32()) continue;
      if (l->kind == StmtKind::for_range && !g_warp.thread_loops.count(l) && !g_warp.lane_loops.count(l) &&
          !(iv.count(l->expr) && iv[l->expr].fits32() && iv.count(l->expr2) && iv[l->expr2].fits32())) continue;
      narrow_vars.insert(l);
      guard |= v.mask;
      if (l->kind == StmtKind::for_range && iv.count(l->expr)) guard |= iv[l->expr].mask | iv[l->expr2].mask;
      any = true;
    }
    sites(p->body);
  }
  // e is computed in 32 bits: an int value whose interval fits.
  bool fits(const Expr *e) const { auto f = iv.find(e); return e && e->type == T_INT && f != iv.end() && f->second.fits32(); }
  uint64_t mask(const Expr *e) const { auto f = iv.find(e); return f == iv.end() ? 0 : f->second.mask; }
  static bool comparison(Tok op) { return op == Tok::eqeq || op == Tok::noteq || op == Tok::lt || op == Tok::le || op == Tok::gt || op == Tok::ge; }
  // A binary operation the 32-bit body computes in 32 bits: + - * of values
  // that fit with a result that fits, / and % by a positive divisor, or a
  // comparison of two values that fit.
  bool binary_site(const Expr *e) const {
    if (!e || e->kind != ExprKind::binary || !fits(e->lhs) || !fits(e->rhs)) return false;
    if (comparison(e->op)) return true;
    if (e->op != Tok::plus && e->op != Tok::minus && e->op != Tok::star && e->op != Tok::slash && e->op != Tok::percent) return false;
    return fits(e);
  }
  // A view parameter, or a view local declared as a slice of one and never
  // assigned: its count is at most the parameter's.
  bool view_param(const Expr *v, int depth = 0) const {
    if (!v || v->kind != ExprKind::ident || depth > 16) return false;
    auto p = param.find(v);
    if (p != param.end()) return !assigned_params.count(p->second);
    auto l = vlocal.find(v);
    if (l == vlocal.end() || vassigned.count(l->second)) return false;
    const Expr *init = l->second->expr;
    return init && init->kind == ExprKind::slice && view_param(init->lhs, depth + 1);
  }
  // An index the 32-bit body checks in 32 bits: a fitting index into a
  // fixed array or a view parameter (whose count the entry checked).
  bool index_site(const Expr *e) const {
    if (!e || e->kind != ExprKind::index || !fits(e->rhs)) return false;
    Type t = e->lhs->type;
    if (t->kind == TypeKind::array) return t->length <= INT32_MAX;
    return t->kind == TypeKind::view && view_param(e->lhs);
  }
  void sites(const Expr *e) {
    if (!e) return;
    if (binary_site(e)) { guard |= mask(e->lhs) | mask(e->rhs) | mask(e); any = true; }
    if (index_site(e)) { guard |= mask(e->rhs) | (e->lhs->type->kind == TypeKind::view ? (uint64_t)1 << 63 : 0); any = true; }
    sites(e->lhs); sites(e->rhs); sites(e->third);
    for (int32_t k = 0; k < e->args.count; k++) sites(e->args[k]);
    for (int32_t k = 0; k < e->fields.count; k++) sites(e->fields[k].value);
  }
  void sites(const Stmt *s) {
    if (!s) return;
    sites(s->expr); sites(s->expr2); sites(s->target);
    for (int32_t k = 0; k < s->body.count; k++) sites(s->body[k]);
    sites(s->then_); sites(s->else_);
  }
};
std::map<const ProcDecl *, Narrow> g_narrow;

// Set while `coax metal-compute` emits: f64 is the float-float pair coax_ff.
static bool g_metal_compute = false;

namespace {

// A unique, kind-prefixed spelling of a type for generated names.
std::string tag(Type t) {
  if (t->physical) return tag(t->physical);
  switch (t->kind) {
    case TypeKind::integer: return t == T_INT ? "s64" : t->name;
    case TypeKind::floating: return t->name;
    case TypeKind::bool_: return "bool";
    case TypeKind::string_: return "string";
    case TypeKind::struct_: case TypeKind::enum_:
      // A generic struct instance is its template's name and its bindings' key: t_Pair__3int.
      if (t->kind == TypeKind::struct_ && t->struct_decl->key.count) {
        std::string k = to_std(t->struct_decl->name) + "__" + to_std(t->struct_decl->key);
        return t->scope ? "q" + std::to_string(t->scope) + "t_" + k : "t_" + k;
      }
      // A name two modules declare is spelled by its module scope; coax_q never comes from a user name.
      if (t->scope) return "q" + std::to_string(t->scope) + "t_" + to_std(t->kind == TypeKind::struct_ ? t->struct_decl->name : t->enum_decl->name);
      return "t_" + t->name;
    case TypeKind::union_: return (t->scope ? "q"+std::to_string(t->scope) : "")+"u_"+to_std(t->union_decl->name);
    case TypeKind::result: {std::string a=tag(t->elem),b=tag(t->sum_error);return "r_"+std::to_string(a.size())+"_"+a+"_"+std::to_string(b.size())+"_"+b;}
    case TypeKind::optional: {std::string a=tag(t->elem);return "o_"+std::to_string(a.size())+"_"+a;}
    case TypeKind::array: return "a" + std::to_string(t->length) + "_" + tag(t->elem);
    case TypeKind::view: return std::string(t->length==1 ? "rv_" : "v_") + tag(t->elem);
    default: return "void";
  }
}

std::string c_type(Type t) {
  if (t->physical) return c_type(t->physical);
  switch (t->kind) {
    case TypeKind::void_: return "void";
    case TypeKind::integer: return std::string(t->is_signed ? "int" : "uint") + std::to_string(t->bits) + "_t";
    case TypeKind::floating: return t == &T_F32 ? "float" : g_metal_compute ? "coax_ff" : "double";
    case TypeKind::bool_: return "_Bool";
    case TypeKind::string_: return "coax_string";
    default: return "coax_" + tag(t);
  }
}

int32_t sum_count(Type t) {return t->kind==TypeKind::union_ ? t->union_decl->variants.count : 2;}
Type sum_payload(Type t,int32_t alternative) {
  if(t->kind==TypeKind::union_)return t->union_decl->variants[alternative].type;
  if(t->kind==TypeKind::result)return alternative==0 ? t->elem : t->sum_error;
  return alternative==0 ? &T_VOID : t->elem;
}
std::string sum_member(int32_t alternative){return "v"+std::to_string(alternative);}
std::string sum_construct(Type t,int32_t alternative,const std::string &payload) {
  std::string text="(("+c_type(t)+"){.tag = "+std::to_string(alternative);
  if(sum_payload(t,alternative)!=&T_VOID)text+=", .payload."+sum_member(alternative)+" = "+payload;
  return text+"})";
}

std::string int_tag(Type t) {
  Type base = t->kind == TypeKind::enum_ ? t->elem : t;
  return std::string(base->is_signed ? "s" : "u") + std::to_string(base->bits);
}

bool needs_init(Type t) {
  if (t->physical) return false;
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

// Ordered lowering decisions (r1a-ordered-lowering), shared by the C emitter
// and `coax lower`: both read the checked graph's effects (CK_effects), so the
// inspected order is the executed one. `traps` is false where trap order is
// not observable (GPU and shader emission).
struct LoweringPolicy { bool wrapping = false, traps = true; };
bool lowering_may_trap(const Expr *e, LoweringPolicy p) {
  return p.traps && !trivial(e) && (e->effects & (2 | (p.wrapping ? 0 : 4)));
}
// Operands are evaluated through temporaries in source order when one calls
// and another is not trivial, or when two may trap.
bool lowering_sequenced(const std::vector<const Expr *> &ops, LoweringPolicy p) {
  int calls = 0, nontrivial = 0, traps = 0;
  for(const Expr *e:ops)if(e && (e->effects & 8))return true;
  for (const Expr *e : ops) {
    if (has_call(e)) calls++;
    if (!trivial(e)) nontrivial++;
    if (lowering_may_trap(e, p)) traps++;
  }
  return (calls > 0 && nontrivial >= 2) || traps >= 2;
}
// `place = value`: the place's address is taken before the value runs.
bool lowering_place_first(const Expr *target, const Expr *value, LoweringPolicy p) {
  if((target->effects|value->effects)&8)return true;
  return (has_call(target) && !trivial(value)) || (has_call(value) && !trivial(target)) ||
         (lowering_may_trap(target, p) && lowering_may_trap(value, p));
}
// True when some operation inside e evaluates its operands through
// temporaries: e's C text assigns them, so it must not be spelled twice.
bool lowering_inner_sequenced(const Expr *e, LoweringPolicy p) {
  if (!e) return false;
  std::vector<const Expr *> ops;
  for (const Expr *x : {e->lhs, e->rhs, e->third}) if (x) ops.push_back(x);
  for (const Expr *a : e->args) ops.push_back(a);
  for (const FieldInit &f : e->fields) if (f.value) ops.push_back(f.value);
  if (ops.size() >= 2 && lowering_sequenced(ops, p)) return true;
  for (const Expr *x : ops) if (lowering_inner_sequenced(x, p)) return true;
  return false;
}
// `place op= value`: false when the place may be spelled twice in C. A place
// whose indexes are sequenced (`c[a[i-1]*256+a[i]] += 1`) is staged too, or
// `place = place op value` would assign its temporaries twice in one
// expression, which C leaves undefined.
bool lowering_compound_staged(const Expr *target, const Expr *value, LoweringPolicy p) {
  if((target->effects|value->effects)&8)return true;
  return has_call(target) || has_call(value) || (lowering_may_trap(target, p) && lowering_may_trap(value, p)) ||
         lowering_inner_sequenced(target, p);
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
  bool read_only = false;
};

std::string escape_path(const std::string &path);

std::string debug_path(const Source *src, Loc loc) {
  return std::filesystem::absolute(source_point(src,loc).path).lexically_normal().string();
}

std::string debug_line(const File *file, Loc loc) {
  auto point=source_point(file->src,loc);
  return "#line "+std::to_string(point.loc.line)+" \""+escape_path(debug_path(file->src,loc))+"\"\n";
}

static std::string transaction_capture_name(Type t){return "coax_tx_capture_"+tag(t);}

struct Emitter {
  const File *file;
  EmitOptions opts;
  bool verification = false;
  // A native check binary (native-check-run.cpp): output and host effects
  // stop the check (coax_nc_refuse) so the evaluator runs it instead, a
  // foreign procedure is such a stub, and each procedure marks itself run.
  bool check_binary = false;
  std::map<const ProcDecl *, size_t> coverage_ids;
  // Native mutants (#13582, coax/docs/native-mutants.md): the check binary
  // with every mutation site behind `coax_nm_mutant == ID`, 0 the unmutated
  // program. A site is the node the evaluator's mutant changes; its two
  // forms are that node emitted before and after the mutant's own change.
  NativeMutantPlan *mutants = nullptr;
  std::map<const Expr *, size_t> mutant_exprs;               // site index by expression
  std::map<const Stmt *, std::vector<size_t>> mutant_stmts;  // site indexes by statement
  const Stmt *mutant_inner = nullptr;                        // the statement whose own sites are being written
  bool versioned = false;               // a loop was versioned: the output needs IV_RUNTIME
  bool erasure = false;                // observable numeric-view stores are used
  int version_depth = 0;                // inside either copy of a versioned loop: no nested versioning
  std::set<const Expr *> unchecked;     // the fast copy's indexes, proven in range by its guard
  const Narrow *n32 = nullptr;          // emitting a Metal kernel's 32-bit body (Narrow)
  std::map<const ProcDecl *, std::string> verification_names;
  std::map<const Expr *, int32_t> verification_ct_ids;
  std::string out;          // the current function body
  int32_t indent = 0;
  int32_t temp_count = 0;
  std::vector<std::string> temps;           // declarations at the top of the current function
  std::vector<const Expr *> ct_values;      // compile-time aggregates, emitted as static constants
  // --split: the unit being emitted names its own compile-time values, so a
  // value added in one unit doesn't renumber another's (#12938).
  bool split = false;
  std::map<const Expr *, int32_t> *unit_ct_ids = nullptr;
  std::vector<const Expr *> *unit_ct_values = nullptr;
  bool wrapping = false;
  bool intops = false;                        // add_checked..mul_sat are used: emit their helpers
  bool wide = false;
  bool barrier = false;                       // value_barrier is used: emit its helpers
  bool inlined = false;                       // an #inline procedure is emitted: define COAX_INLINE
  bool strict_math = false;
  bool transactions = false;
  std::map<size_t,std::string> transaction_scopes;
  struct TxLocal { Str name; Type type; bool writable; bool initialized; };
  std::vector<TxLocal> transaction_locals;
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
    return opts.metal_compute ? "threadgroup_barrier(mem_flags::mem_threadgroup);" : "__syncthreads();";
  }
  bool apple_tensor = false;  // an am_matmul_bf16 call: the module needs Metal 4 tensor ops
  // The pointer type of a temporary holding the address of a place. Metal
  // pointers name their address space: a view's elements are device memory,
  // locals are thread memory.
  std::string pointer_type(const Expr *place, bool read = false) {
    if (!opts.metal_compute) {
      const Expr *root=place;
      while(root && (root->kind==ExprKind::index || root->kind==ExprKind::field)) {
        if(root->lhs->type->kind==TypeKind::view && root->lhs->type->length==1) read=true;
        root=root->lhs;
      }
      return std::string(read ? "const " : "") + c_type(place->type) + " *";
    }
    const Expr *e = place;
    bool space_device = false;
    while (e && (e->kind == ExprKind::index || e->kind == ExprKind::field)) {
      if (e->kind == ExprKind::index && e->lhs->type->kind == TypeKind::view) { space_device = true; break; }
      e = e->lhs;
    }
    // A group kernel's threadgroup array (matched by name, as compute.cpp admits it).
    bool space_group = false;
    if (!space_device && e && e->kind == ExprKind::ident && g_warp.shared_arrays.count(current))
      for (const Stmt *d : g_warp.shared_arrays.at(current)) if (str_eq(e->text, d->name)) space_group = true;
    return std::string(space_device ? "device " : space_group ? "threadgroup " : "thread ") + c_type(place->type) + " *";
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
    if (!opts.cuda && (e->intrinsic==Intrinsic::sin || e->intrinsic==Intrinsic::cos ||
                      e->intrinsic==Intrinsic::exp || e->intrinsic==Intrinsic::log)) {
      strict_math=true;
      return "coax_sm_"+name+(e->type==&T_F32?"f":"");
    }
    return name + (e->type == &T_F32 ? "f" : "");
  }
  std::string proc_name(const ProcDecl *p) {
    if (verification) {
      auto found = verification_names.find(p);
      if (found != verification_names.end()) return found->second;
    }
    if (device() && p->export_c) return "coax_k_" + to_std(p->name);  // the kernel wrapper keeps the name
    if (p->export_c || p->foreign) return to_std(p->name);
    std::string base = p->scope ? "coax_q" + std::to_string(p->scope) + "p_" + to_std(p->name) : "coax_p_" + to_std(p->name);
    return p->origin ? base + "__" + to_std(p->key) : base;
  }

  // Evaluate operands left to right when order is observable; build() gets
  // the C text for each operand. A place (an lvalue in Coax) stays one in C:
  // a comma expression is not, so its address is taken inside it,
  // `(*(t = ..., &(place)))`, and it can be assigned to or addressed again.
  // The checked graph's effects (CK_effects) decide it: an operand that calls,
  // or two that may trap, are evaluated in Coax order through temporaries,
  // never left to C's unspecified operand order.
  LoweringPolicy policy() const { return {opts.wrapping, !device() && !opts.metal}; }
  template <class F> std::string ordered(const std::vector<Operand> &ops, F build, bool place = false) {
    std::vector<const Expr *> exprs;
    for (const Operand &o : ops) exprs.push_back(o.e);
    std::vector<std::string> texts;
    if (!lowering_sequenced(exprs, policy())) {
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
        std::string t = new_temp(pointer_type(o.e,o.read_only));
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
    if (t->physical) return zero_value(t->physical);
    if (device() && (t->kind == TypeKind::array || t->kind == TypeKind::view)) return c_type(t) + "{}";
    if (opts.metal_compute && t == &T_F64) return "coax_ff_make(0.0f, 0.0f)";
    if (needs_init(t)) return "coax_init_" + tag(t) + "()";
    switch (t->kind) {
      case TypeKind::string_: return "COAX_STR(0, 0)";
      case TypeKind::optional: return sum_construct(t,t->sum_default,"");
      case TypeKind::struct_: case TypeKind::array: case TypeKind::view: return "((" + c_type(t) + "){0})";
      case TypeKind::floating: return t == &T_F32 ? "0.0f" : "0.0";
      default: return "((" + c_type(t) + ")0)";
    }
  }

  std::string ct_name(const Expr *e) {
    if (verification) {
      auto found = verification_ct_ids.find(e);
      if (found == verification_ct_ids.end()) {
        int32_t id = (int32_t)ct_values.size();
        verification_ct_ids.emplace(e, id);
        ct_values.push_back(e);
        return "coax_k" + std::to_string(id);
      }
      return "coax_k" + std::to_string(found->second);
    }
    if (unit_ct_ids) {
      auto found = unit_ct_ids->find(e);
      if (found != unit_ct_ids->end()) return "coax_k" + std::to_string(found->second);
      int32_t id = (int32_t)unit_ct_values->size();
      unit_ct_ids->emplace(e, id);
      unit_ct_values->push_back(e);
      return "coax_k" + std::to_string(id);
    }
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
    if(t->physical) return init_list((*v.agg)[0]);
    switch (t->kind) {
      case TypeKind::integer: case TypeKind::enum_: return int_text(t, v.bits);
      case TypeKind::bool_: return v.bits ? "1" : "0";
      case TypeKind::floating: return float_text(t, v.f);
      case TypeKind::string_: return "{" + c_string_text(v.s) + ", " + std::to_string(v.s.count) + "}";
      case TypeKind::union_: case TypeKind::result: case TypeKind::optional: {
        std::string text="{.tag = "+std::to_string(v.sum_tag);
        if(sum_payload(t,(int32_t)v.sum_tag)!=&T_VOID)text+=", .payload."+sum_member((int32_t)v.sum_tag)+" = "+init_list((*v.agg)[0]);
        return text+"}";
      }
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
    std::string fn = std::string("coax_") + (opts.wrapping && (strcmp(name, "add") == 0 || strcmp(name, "sub") == 0 || strcmp(name, "mul") == 0) ? "w" : "") +
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
    if (to == &T_STRING) return std::string("coax_string_of_bytes") + (from->length==1 ? "_readonly" : "") + "(" + x + ")";
    if (to->kind == TypeKind::view && from->kind == TypeKind::view) {
      std::string v=new_temp(c_type(from));
      return "(" + v + " = " + x + ", ((" + c_type(to) + "){" + v + ".data, " + v + ".count}))";
    }
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

  std::map<const Expr *,std::string> lowered;
  bool control(const Expr *e) const {
    if(!e)return false;
    // A mutation site with operands: they are evaluated once, into temporaries, before either form.
    if(!mutant_exprs.empty() && (e->kind==ExprKind::binary || e->kind==ExprKind::call) && mutant_exprs.count(e))return true;
    if((e->effects&8) || e->kind==ExprKind::try_ || e->kind==ExprKind::or_else)return true;
    for(const Expr *child:{e->lhs,e->rhs,e->third})if(control(child))return true;
    for(const Expr *child:e->args)if(control(child))return true;
    for(const FieldInit &field:e->fields)if(control(field.value))return true;
    return false;
  }
  std::string capture(const Expr *e,bool address=false) {
    std::string value=ex(e);
    if(e->type==&T_VOID){line(value+";");return "((void)0)";}
    std::string temp=new_temp(address ? pointer_type(e) : c_type(e->type));
    line(temp+" = "+(address ? "&("+value+")" : value)+";");
    return address ? "(*"+temp+")" : temp;
  }
  std::string lower_control(const Expr *e) {
    if(e->kind==ExprKind::try_ || e->kind==ExprKind::or_else){
      std::string value=capture(e->lhs);
      line("if ("+value+".tag >= "+std::to_string(sum_count(e->lhs->type))+") coax_trap(\"invalid sum tag\", "+std::to_string(e->loc.line)+");");
      int32_t success=e->lhs->type->kind==TypeKind::optional ? 1 : 0;
      std::string payload="("+value+").payload."+sum_member(success);
      if(e->kind==ExprKind::try_){
        line("if ("+value+".tag != "+std::to_string(success)+") {");indent++;
        std::string error=sum_payload(e->lhs->type,1-success)==&T_VOID ? "" : "("+value+").payload."+sum_member(1-success);
        std::string returned=new_temp(c_type(e->propagate_type));
        line(returned+" = "+sum_construct(e->propagate_type,e->sum_variant,error)+";");
        emit_defers_from(0,transaction_failure(e->propagate_type,returned));line("return "+returned+";");indent--;line("}");
        return e->type==&T_VOID ? "((void)0)" : payload;
      }
      std::string result=e->type==&T_VOID ? "" : new_temp(c_type(e->type));
      line("if ("+value+".tag == "+std::to_string(success)+") {");indent++;
      if(!result.empty())line(result+" = "+payload+";");
      indent--;line("} else {");indent++;
      std::string fallback=ex(e->rhs);line((result.empty()?"":result+" = ")+fallback+";");
      indent--;line("}");return result.empty()?"((void)0)":result;
    }
    if(e->kind==ExprKind::ifx || (e->kind==ExprKind::binary && (e->op==Tok::andand || e->op==Tok::oror))){
      std::string condition=capture(e->lhs),result=new_temp(c_type(e->type));
      line("if ("+condition+") {");indent++;
      if(e->kind==ExprKind::ifx){std::string yes=ex(e->rhs);line(result+" = "+yes+";");}
      else if(e->op==Tok::andand){std::string rhs=ex(e->rhs);line(result+" = "+rhs+";");}
      else line(result+" = 1;");
      indent--;line("} else {");indent++;
      if(e->kind==ExprKind::ifx){std::string no=ex(e->third);line(result+" = "+no+";");}
      else if(e->op==Tok::oror){std::string rhs=ex(e->rhs);line(result+" = "+rhs+";");}
      else line(result+" = 0;");
      indent--;line("}");return result;
    }
    std::vector<const Expr *> children;
    auto stage=[&](const Expr *child,bool address=false){if(child){lowered[child]=capture(child,address);children.push_back(child);}};
    if(e->kind==ExprKind::slice)stage(e->lhs,e->lhs->type->kind==TypeKind::array && is_place(e->lhs));
    else if(e->kind==ExprKind::index)stage(e->lhs,e->lhs->type->kind==TypeKind::array && is_place(e->lhs));
    else if(e->kind==ExprKind::field)stage(e->lhs,is_place(e->lhs));
    else stage(e->lhs);
    stage(e->rhs);stage(e->third);
    for(const Expr *arg:e->args)stage(arg);
    for(const FieldInit &field:e->fields)stage(field.value);
    std::string text=raw_ex(e);
    for(const Expr *child:children)lowered.erase(child);
    return text;
  }
  std::string ex(const Expr *e) {
    auto found=lowered.find(e);if(found!=lowered.end())return found->second;
    if(!mutant_exprs.empty()){auto site=mutant_exprs.find(e);if(site!=mutant_exprs.end())return mutant_ex(e,site->second);}
    return control(e) ? lower_control(e) : raw_ex(e);
  }

  // ---- native mutants ----
  std::string mutant_is(size_t index) {
    mutants->emitted[index] = 1;
    return "(coax_nm_mutant == " + std::to_string(mutants->sites[index].id) + ")";
  }
  // An expression site. A literal is a choice of two constants. `&&` and `||`
  // keep their short circuit: the left operand once, then the right one only
  // when the operator chosen needs it. Any other operator or lowered call
  // takes its operands once, in source order, then one of its two forms.
  std::string mutant_ex(const Expr *e, size_t index) {
    const NativeMutantSite &site = mutants->sites[index];
    const std::string is = mutant_is(index);
    if (e->kind == ExprKind::binary && (e->op == Tok::andand || e->op == Tok::oror)) {
      const bool conjunction = e->op == Tok::andand;
      std::string left = capture(e->lhs), result = new_temp(c_type(e->type));
      line("if (" + is + " ? " + (conjunction ? "!" : "") + left + " : " + (conjunction ? "" : "!") + left + ") {");
      indent++;
      std::string right = ex(e->rhs);
      line(result + " = " + right + ";");
      indent--; line("} else {"); indent++;
      line(result + " = " + left + ";");
      indent--; line("}");
      return result;
    }
    std::vector<const Expr *> children;
    if (e->kind == ExprKind::binary || e->kind == ExprKind::call) {
      auto stage = [&](const Expr *child) { if (child) { lowered[child] = capture(child); children.push_back(child); } };
      stage(e->lhs); stage(e->rhs);
      for (const Expr *argument : e->args) stage(argument);
    }
    std::string original = raw_ex(e);
    site.apply();
    std::string changed = raw_ex(e);
    site.undo();
    for (const Expr *child : children) lowered.erase(child);
    return "(" + is + " ? " + changed + " : " + original + ")";
  }
  // A condition whose statement has a negation site.
  std::string mutant_condition(const Stmt *s, const std::string &text) {
    auto found = mutant_stmts.find(s);
    if (found == mutant_stmts.end()) return text;
    for (size_t index : found->second)
      if (mutants->sites[index].kind == 2) return "(" + mutant_is(index) + " != (" + text + "))";  // a condition is 0 or 1
    return text;
  }
  // A statement's own sites: removed under one mutant, its compound operator
  // exchanged under another. The statement is written once for each form.
  void mutant_stmt(const Stmt *s, const std::vector<size_t> &sites) {
    const Stmt *outer = mutant_inner;
    mutant_inner = s;
    size_t removal = SIZE_MAX, exchange = SIZE_MAX;
    for (size_t index : sites) {
      if (mutants->sites[index].kind == 3) removal = index;
      if (mutants->sites[index].kind == 4) exchange = index;
    }
    if (removal != SIZE_MAX) { line("if (!" + mutant_is(removal) + ") {"); indent++; }
    if (exchange != SIZE_MAX) {
      const NativeMutantSite &site = mutants->sites[exchange];
      line("if " + mutant_is(exchange) + " {"); indent++;
      site.apply(); stmt(s); site.undo();
      indent--; line("} else {"); indent++;
      stmt(s);
      indent--; line("}");
    } else stmt(s);
    if (removal != SIZE_MAX) { indent--; line("}"); }
    mutant_inner = outer;
  }

  std::string raw_ex(const Expr *e) {
    std::string line = std::to_string(e->loc.line);
    if (n32 && n32->binary_site(e)) {
      const char *op = e->op == Tok::plus ? " + " : e->op == Tok::minus ? " - " : e->op == Tok::star ? " * " : e->op == Tok::slash ? " / " :
                       e->op == Tok::percent ? " % " : e->op == Tok::eqeq ? " == " : e->op == Tok::noteq ? " != " : e->op == Tok::lt ? " < " :
                       e->op == Tok::le ? " <= " : e->op == Tok::gt ? " > " : " >= ";
      // The intervals prove no 32-bit step overflows (so no INT32_MIN / -1);
      // a divisor that may be zero or negative keeps the checked helper.
      bool zero = (e->op == Tok::slash || e->op == Tok::percent) && n32->iv.at(e->rhs).lo <= 0;
      return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &x) {
        if (zero) return std::string(e->op == Tok::slash ? "coax_div32(" : "coax_mod32(") + "(int32_t)(" + x[0] + "), (int32_t)(" + x[1] + "), " + line + ")";
        return "((int32_t)(" + x[0] + ")" + op + "(int32_t)(" + x[1] + "))";
      });
    }
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
          if (e->byte_intrinsic == ByteIntrinsic::secure_zero) {
            erasure=true;
            std::string v=new_temp(c_type(e->args[0]->type));
            return "("+v+" = "+ex(e->args[0])+", coax_secure_zero("+v+".data, "+v+".count, sizeof(*"+v+".data), "+std::to_string(e->loc.line)+"))";
          }
          if (e->byte_intrinsic == ByteIntrinsic::bytes_overlap && check_binary) return "(coax_nc_refuse(\"storage overlap\"), (_Bool)0)";
          if (e->byte_intrinsic == ByteIntrinsic::bytes_overlap) {
            std::string a=new_temp(c_type(e->args[0]->type)),b=new_temp(c_type(e->args[1]->type));
            std::string x=ex(e->args[0]); std::string y=ex(e->args[1]);  // in source order (operands of + are unsequenced)
            return "("+a+" = "+x+", "+b+" = "+y+", coax_bytes_overlap("+a+".data, "+a+".count, "+b+".data, "+b+".count))";
          }
          if (e->byte_intrinsic == ByteIntrinsic::mul_wide) {
            wide=true;
            std::string a=new_temp("uint64_t"),b=new_temp("uint64_t");
            std::string x=ex(e->args[0]); std::string y=ex(e->args[1]);
            return "("+a+" = "+x+", "+b+" = "+y+", coax_mul_wide("+a+", "+b+"))";
          }
          if (e->byte_intrinsic >= ByteIntrinsic::add_checked) {
            // add_checked..mul_sat (spec section 9): operands once, left to right, then one helper
            intops=true;
            const bool checked=e->byte_intrinsic<=ByteIntrinsic::mul_checked;
            const Type t=checked?e->type->elem:e->type;
            static const char *const ops[]={"add","sub","mul"};
            const int op=(int)e->byte_intrinsic-(int)(checked?ByteIntrinsic::add_checked:ByteIntrinsic::add_sat);
            std::string a=new_temp(c_type(t)),b=new_temp(c_type(t));
            std::string x=ex(e->args[0]); std::string y=ex(e->args[1]);
            std::string fn=std::string("coax_")+ops[op]+(checked?"_checked_":"_sat_")+int_tag(t);
            if(!checked) return "("+a+" = "+x+", "+b+" = "+y+", "+fn+"("+a+", "+b+"))";
            std::string r=new_temp(c_type(t));
            return "("+a+" = "+x+", "+b+" = "+y+", "+fn+"("+a+", "+b+", &"+r+") ? "+sum_construct(e->type,1,r)+" : "+sum_construct(e->type,0,"")+")";
          }
          if (e->byte_intrinsic >= ByteIntrinsic::add_wrap && e->byte_intrinsic <= ByteIntrinsic::mul_wrap && e->type != &T_U32 && e->type != &T_U64) {
            // add_wrap, sub_wrap, mul_wrap on the other widths: the runtime's wrapping forms
            static const char *const ops[]={"coax_wadd_","coax_wsub_","coax_wmul_"};
            std::string a=new_temp(c_type(e->type)),b=new_temp(c_type(e->type));
            std::string x=ex(e->args[0]); std::string y=ex(e->args[1]);
            return "("+a+" = "+x+", "+b+" = "+y+", "+ops[(int)e->byte_intrinsic-(int)ByteIntrinsic::add_wrap]+int_tag(e->type)+"("+a+", "+b+"))";
          }
          if (e->byte_intrinsic >= ByteIntrinsic::add_wrap) {
            wrapping=true;
            std::string a=new_temp(c_type(e->type)),b=new_temp(c_type(e->type));
            std::string x=ex(e->args[0]); std::string y=ex(e->args[1]);
            return "("+a+" = "+x+", "+b+" = "+y+", coax_"+to_std(e->text)+(e->type==&T_U32?"_u32(":"_u64(")+a+", "+b+"))";
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
          if (kind == 6) return "coax_am_sg_mma_f16(" + whole(0) + ", (threadgroup const half *)" + whole(1) + ".e, (threadgroup const half *)" + whole(2) + ".e, coax_lane)";
          if (kind == 2) return "(" + group_barrier().substr(0, group_barrier().size() - 1) + ", coax_am_sg_spill(" + whole(0) + ", " + whole(1) + ".e, coax_lane), " +
                                group_barrier().substr(0, group_barrier().size() - 1) + ")";
          if (kind == 5) return "coax_am_sg_zero(" + whole(0) + ")";
          apple_tensor = true;
          auto value = [&](int i) { return std::to_string(constant_of_expr(e->args[i])); };
          if (kind == 4 || kind == 7) {
            std::string fn = (kind == 7) ? "coax_am_matmul_iq4_bf16<" : "coax_am_matmul_iq4<";
            std::string s = fn + value(1) + ", " + value(2) + ", " + std::to_string(g_warp.group_kernels.at(current) / 32) + ">(";
            for (int i = 3; i < 11; i++) s += ex(e->args[i]) + ", ";
            return s + "(threadgroup bfloat *)" + whole(11) + ".e, coax_lane, " + line + ", coax_err)";
          }
          std::string s = std::string(kind == 8 ? "coax_am_matmul_x16<" : "coax_am_matmul<") + value(1) + ", " + value(2) + ", " + std::to_string(g_warp.group_kernels.at(current) / 32) + ">(";
          for (int i = 3; i < 11; i++) s += ex(e->args[i]) + ", ";
          return s + line + ", coax_err)";
        }
        if (opts.metal_compute && e->proc && e->args.count == 5) {
          if (str_eq(e->proc->name, "am_dot_iq4_lane") || str_eq(e->proc->name, "am_dot_q4_lane") ||
              str_eq(e->proc->name, "am_dot_f16_lane")) {
            apple_tensor = true;
            std::string fn = str_eq(e->proc->name, "am_dot_iq4_lane") ? "coax_am_dot_iq4_lane(" :
                             str_eq(e->proc->name, "am_dot_q4_lane") ? "coax_am_dot_q4_lane(" :
                             "coax_am_dot_f16_lane(";
            std::string s = fn;
            for (int i = 0; i < 5; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
        }
        if (opts.metal_compute && e->proc && e->args.count == 8) {
          if (str_eq(e->proc->name, "am_dot_iq4_situ_lane") || str_eq(e->proc->name, "am_dot_q4_situ_lane") ||
              str_eq(e->proc->name, "am_dot_iq4_moe_lane") || str_eq(e->proc->name, "am_dot_q4_moe_lane")) {
            apple_tensor = true;
            std::string fn = str_eq(e->proc->name, "am_dot_iq4_situ_lane") ? "coax_am_dot_iq4_situ_lane(" :
                             str_eq(e->proc->name, "am_dot_q4_situ_lane") ? "coax_am_dot_q4_situ_lane(" :
                             str_eq(e->proc->name, "am_dot_iq4_moe_lane") ? "coax_am_dot_iq4_moe_lane(" :
                             "coax_am_dot_q4_moe_lane(";
            std::string s = fn;
            for (int i = 0; i < 8; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
        }
        if (opts.metal_compute && e->proc && e->args.count == 9 && str_eq(e->proc->name, "am_dot_q4_2_write")) {
          apple_tensor = true;
          std::string s = "coax_am_dot_q4_2_write(";
          for (int i = 0; i < 9; i++) s += ex(e->args[i]) + ", ";
          return s + "coax_lane, " + line + ", coax_err)";
        }
        if (opts.metal_compute && e->proc && e->args.count == 13) {
          if (str_eq(e->proc->name, "am_dot_iq4_situ_2_write") || str_eq(e->proc->name, "am_dot_iq4_moe_2_write")) {
            apple_tensor = true;
            std::string s = str_eq(e->proc->name, "am_dot_iq4_situ_2_write") ? "coax_am_dot_iq4_situ_2_write(" : "coax_am_dot_iq4_moe_2_write(";
            for (int i = 0; i < 13; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
        }
        if (opts.metal_compute && e->proc) {
          if (e->args.count == 9 && (str_eq(e->proc->name, "am_dot_f16_tg_write") || str_eq(e->proc->name, "am_rmsnorm_f32"))) {
            apple_tensor = true;
            std::string s = str_eq(e->proc->name, "am_dot_f16_tg_write") ? "coax_am_dot_f16_tg_write(" : "coax_am_rmsnorm_f32(";
            for (int i = 0; i < 9; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 15 && str_eq(e->proc->name, "am_ctrl_up_apply_f16")) {
            apple_tensor = true;
            std::string s = "coax_am_ctrl_up_apply_f16(";
            for (int i = 0; i < 15; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 14 && str_eq(e->proc->name, "am_mix_fused")) {
            apple_tensor = true;
            std::string s = "coax_am_mix_fused(";
            for (int i = 0; i < 14; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 8 && str_eq(e->proc->name, "am_router_top8")) {
            apple_tensor = true;
            std::string s = "coax_am_router_top8(";
            for (int i = 0; i < 8; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 20 && str_eq(e->proc->name, "am_moe_gu_iq4_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_iq4_tg(";
            for (int i = 0; i < 20; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 20 && str_eq(e->proc->name, "am_moe_gu_q4_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_q4_tg(";
            for (int i = 0; i < 20; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_moe_down_iq4_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_iq4_tg(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_moe_down_q4_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_q4_tg(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 17 && str_eq(e->proc->name, "am_dot_q4_qrgkv")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4_qrgkv(";
            for (int i = 0; i < 17; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 17 && str_eq(e->proc->name, "am_conv_kv_both")) {
            apple_tensor = true;
            std::string s = "coax_am_conv_kv_both(";
            for (int i = 0; i < 17; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 18 && str_eq(e->proc->name, "am_attn_gqa_1pass")) {
            apple_tensor = true;
            std::string s = "coax_am_attn_gqa_1pass(";
            for (int i = 0; i < 18; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 27 && str_eq(e->proc->name, "am_kv_attn_gqa_1pass")) {
            apple_tensor = true;
            std::string s = "coax_am_kv_attn_gqa_1pass(";
            for (int i = 0; i < 27; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_dot_q4_conv_2_write")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4_conv_2_write(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 13 && str_eq(e->proc->name, "am_ctrl_up_apply_gf")) {
            apple_tensor = true;
            std::string s = "coax_am_ctrl_up_apply_gf(";
            for (int i = 0; i < 13; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 16 && str_eq(e->proc->name, "am_ctrl_up_apply_gf_norm")) {
            apple_tensor = true;
            std::string s = "coax_am_ctrl_up_apply_gf_norm(";
            for (int i = 0; i < 16; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 16 && str_eq(e->proc->name, "am_ctrl_up_apply_gf_rm_norm")) {
            apple_tensor = true;
            std::string s = "coax_am_ctrl_up_apply_gf_rm_norm(";
            for (int i = 0; i < 16; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 21 && str_eq(e->proc->name, "am_moe_gu_q4s_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_q4s_tg(";
            for (int i = 0; i < 21; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 13 && str_eq(e->proc->name, "am_moe_down_q4s_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_q4s_tg(";
            for (int i = 0; i < 13; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 21 && str_eq(e->proc->name, "am_moe_gu_iq4s_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_iq4s_tg(";
            for (int i = 0; i < 21; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 13 && str_eq(e->proc->name, "am_moe_down_iq4s_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_iq4s_tg(";
            for (int i = 0; i < 13; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 15 && str_eq(e->proc->name, "am_dot_q4s_2_write_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_2_write_nb(";
            for (int i = 0; i < 15; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_dot_q4s_8row_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_8row_nb(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_dot_q4s_mma32_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_mma32_nb(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 11 && str_eq(e->proc->name, "am_dot_q8_mma32_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q8_mma32_nb(";
            for (int i = 0; i < 11; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_dspark_fold_logits_s1")) {
            apple_tensor = true;
            std::string s = "coax_am_dspark_fold_logits_s1(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 20 && str_eq(e->proc->name, "am_dot_q4s_qrgkv_sdyn_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_qrgkv_sdyn_nb(";
            for (int i = 0; i < 20; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 28 && str_eq(e->proc->name, "am_kv_attn_gqa_1pass_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_kv_attn_gqa_1pass_nb(";
            for (int i = 0; i < 28; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 16 && str_eq(e->proc->name, "am_dot_q4s_conv_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_conv_nb(";
            for (int i = 0; i < 16; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 14 && str_eq(e->proc->name, "am_moe_plan_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_plan_nb(";
            for (int i = 0; i < 14; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 18 && str_eq(e->proc->name, "am_moe_gu_iq4_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_iq4_nb(";
            for (int i = 0; i < 18; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 19 && str_eq(e->proc->name, "am_moe_gu_q4s_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_q4s_nb(";
            for (int i = 0; i < 19; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 19 && str_eq(e->proc->name, "am_moe_gu_iq4s_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_gu_iq4s_nb(";
            for (int i = 0; i < 19; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 17 && str_eq(e->proc->name, "am_moe_down_iq4_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_iq4_nb(";
            for (int i = 0; i < 17; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 18 && str_eq(e->proc->name, "am_moe_down_q4s_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_q4s_nb(";
            for (int i = 0; i < 18; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 18 && str_eq(e->proc->name, "am_moe_down_iq4s_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_iq4s_nb(";
            for (int i = 0; i < 18; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 14 && str_eq(e->proc->name, "am_moe_down_p1_iq4s_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_p1_iq4s_nb(";
            for (int i = 0; i < 14; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 14 && str_eq(e->proc->name, "am_moe_down_p1_q4s_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_moe_down_p1_q4s_nb(";
            for (int i = 0; i < 14; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 8 && str_eq(e->proc->name, "am_conv_commit_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_conv_commit_nb(";
            for (int i = 0; i < 8; i++) s += ex(e->args[i]) + ", ";
            return s + line + ", coax_err)";
          }
          if (e->args.count == 11 && str_eq(e->proc->name, "am_dot_q4s_small_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_small_nb(";
            for (int i = 0; i < 11; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 16 && str_eq(e->proc->name, "am_mix_fused_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_mix_fused_nb(";
            for (int i = 0; i < 16; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 18 && str_eq(e->proc->name, "am_ctrl_up_apply_gf_rm_nb")) {
            apple_tensor = true;
            std::string s = "coax_am_ctrl_up_apply_gf_rm_nb(";
            for (int i = 0; i < 18; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 14 && str_eq(e->proc->name, "am_dot_q4_2_write_ex")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4_2_write_ex(";
            for (int i = 0; i < 14; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 15 && str_eq(e->proc->name, "am_dot_q4s_2_write_ex")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_2_write_ex(";
            for (int i = 0; i < 15; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 13 && str_eq(e->proc->name, "am_dot_q4s_conv_2_write")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_conv_2_write(";
            for (int i = 0; i < 13; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_dot_q4s_1row_tg")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_1row_tg(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 19 && str_eq(e->proc->name, "am_dot_q4s_qrgkv_sdyn")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_q4s_qrgkv_sdyn(";
            for (int i = 0; i < 19; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, " + line + ", coax_err)";
          }
          if (e->args.count == 11 && str_eq(e->proc->name, "am_dot_f16_tg_write_rms")) {
            apple_tensor = true;
            std::string s = "coax_am_dot_f16_tg_write_rms(";
            for (int i = 0; i < 11; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 15 && str_eq(e->proc->name, "am_ctrl_up_apply_gf_batch")) {
            apple_tensor = true;
            std::string s = "coax_am_ctrl_up_apply_gf_batch(";
            for (int i = 0; i < 15; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 9 && str_eq(e->proc->name, "am_mix_batch_f32")) {
            apple_tensor = true;
            std::string s = "coax_am_mix_batch_f32(";
            for (int i = 0; i < 9; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 13 && str_eq(e->proc->name, "am_attn_scores_m4")) {
            apple_tensor = true;
            std::string s = "coax_am_attn_scores_m4(";
            for (int i = 0; i < 13; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
          if (e->args.count == 12 && str_eq(e->proc->name, "am_attn_values_m4")) {
            apple_tensor = true;
            std::string s = "coax_am_attn_values_m4(";
            for (int i = 0; i < 12; i++) s += ex(e->args[i]) + ", ";
            return s + "coax_lane, coax_am_tg, " + line + ", coax_err)";
          }
        }
        if (opts.metal_compute && e->proc && e->args.count == 1) {
          if (str_eq(e->proc->name, "am_simd_sum")) return "simd_sum(" + ex(e->args[0]) + ")";
          if (str_eq(e->proc->name, "am_simd_max")) return "simd_max(" + ex(e->args[0]) + ")";
          if (str_eq(e->proc->name, "am_to_f16")) return "as_type<ushort>((half)(" + ex(e->args[0]) + "))";
          if (str_eq(e->proc->name, "am_iq4_lut")) {
            apple_tensor = true;
            return "coax_iq4_lut[(" + ex(e->args[0]) + ") & 15u]";
          }
          if (str_eq(e->proc->name, "am_f16")) return "((float)as_type<half>((ushort)(" + ex(e->args[0]) + ")))";
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
      case ExprKind::sum: case ExprKind::some:
        return sum_construct(e->type,e->sum_variant,e->lhs ? ex(e->lhs) : "");
      case ExprKind::null_: return sum_construct(e->type,0,"");
      case ExprKind::try_: case ExprKind::or_else: return lower_control(e);
      case ExprKind::unary:
        if (e->op == Tok::bang) return "(!" + ex(e->lhs) + ")";
        if (is_float(e->type)) {
          // A negative literal operand (a folded negative constant) would make
          // `--`, C's decrement token (#13570); a space keeps them apart.
          const std::string x = ex(e->lhs);
          return (!x.empty() && x[0] == '-' ? "(- " : "(-") + x + ")";
        }
        if (opts.wrapping) return "coax_wneg_" + int_tag(e->type) + "(" + ex(e->lhs) + ")";
        return "coax_neg_" + int_tag(e->type) + "(" + ex(e->lhs) + ", " + line + ")";
      case ExprKind::binary: return binary(e);
      case ExprKind::field:
        if(e->lhs->type->physical) return ex(e->lhs);
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
        if (n32 && n32->index_site(e)) {
          if (lt->kind == TypeKind::view)
            return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &t) {
              return "(*coax_elem32_" + tag(lt) + "(" + t[0] + ", (int32_t)(" + t[1] + "), " + line + "))";
            }, true);
          return ordered({{e->lhs, is_place(e->lhs)}, {e->rhs, false}}, [&](const std::vector<std::string> &t) {
            return "(" + t[0] + ").e[coax_index32((int32_t)(" + t[1] + "), " + std::to_string(lt->length) + ", " + line + ")]";
          }, is_place(e));
        }
        if (!unchecked.empty() && unchecked.count(e)) {
          std::string object = ex(e->lhs);
          std::string at = plain_index(e->rhs);
          return "(" + object + (lt->kind == TypeKind::view ? ").data[" : ").e[") + at + "]";
        }
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
        std::vector<Operand> ops = {{e->lhs, lt->kind == TypeKind::array, e->type->kind==TypeKind::view && e->type->length==1}};
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
        if(e->type->physical) return e->fields.count ? ex(e->fields[0].value) : zero_value(e->type->physical);
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
      case ExprKind::ifx: {
        // Emit operands in source order: the operands of + are unsequenced, and
        // each ex() may number temporaries, so GCC and Clang would differ.
        std::string c = ex(e->lhs); std::string a = ex(e->rhs); std::string b = ex(e->third);
        return "(" + c + " ? " + a + " : " + b + ")";
      }
    }
    return "0";
  }

  std::string binary(const Expr *e) {
    Type t = e->lhs->type;
    std::string line = std::to_string(e->loc.line);
    if(t->kind==TypeKind::optional){
      // Coax admits only optional/null equality here, never payload equality.
      std::string a=new_temp(c_type(t)),b=new_temp(c_type(t));
      std::string left=ex(e->lhs);std::string right=ex(e->rhs);
      return "("+a+" = "+left+", "+b+" = "+right+", (("+a+".tag >= 2 || "+b+".tag >= 2) ? (coax_trap(\"invalid sum tag\", "+line+"), 0) : ("+a+".tag"+(e->op==Tok::noteq?" != ":" == ")+b+".tag)))";
    }
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
    if (e->op == Tok::andand || e->op == Tok::oror) {
      // Left operand first: ex() numbers temporaries, and + leaves the order unspecified.
      std::string l = ex(e->lhs); std::string r = ex(e->rhs);
      return "(" + l + op + r + ")";
    }
    bool bits = e->op == Tok::amp || e->op == Tok::pipe || e->op == Tok::caret;
    return ordered({{e->lhs, false}, {e->rhs, false}}, [&](const std::vector<std::string> &x) {
      std::string s = "(" + x[0] + op + x[1] + ")";
      return bits ? "((" + c_type(e->type) + ")" + s + ")" : s;
    });
  }

  // ---- print: split the checked format at compile time ----

  std::string show_call(Type t, const std::string &v, bool top) {
    if(t->physical) return show_call(t->physical,v,top);
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
    std::vector<const Expr *> staged;
    for(int32_t i=1;i<e->args.count;i++)if(control(e->args[i])){
      for(int32_t k=1;k<e->args.count;k++){lowered[e->args[k]]=capture(e->args[k]);staged.push_back(e->args[k]);}
      break;
    }
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
    for(const Expr *a:staged)lowered.erase(a);
  }

  // ---- statements ----

  std::string transaction_failure(Type type,const std::string &value) {
    if(type->kind!=TypeKind::result&&type->kind!=TypeKind::optional)return "0";
    return "coax_txr_abort("+std::string(type->kind==TypeKind::result?"1":"2")+", "+value+".tag, 1)";
  }
  void emit_defers_from(size_t depth,const std::string &failure="0") {
    for (size_t s = defers.size(); s-- > depth;){
      for (size_t i = defers[s].size(); i-- > 0;) stmt(defers[s][i]);
      auto tx=transaction_scopes.find(s);if(tx!=transaction_scopes.end())line("coax_tx_close(&"+tx->second+", "+failure+");");
    }
  }

  bool any_defers_from(size_t depth) {
    for (size_t s = depth; s < defers.size(); s++)
      if (!defers[s].empty()||transaction_scopes.count(s)) return true;
    return false;
  }

  void block_body(const Stmt *s, const std::vector<std::string> &prelude = {}) {
    const size_t local_mark=transaction_locals.size();
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
    transaction_locals.resize(local_mark);
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

  // Large struct values are built where they are stored (#13735). A struct
  // literal of a large type as a compound literal is a temporary as big as
  // the type on the stack, and coax_init_T returns another copy by value: a
  // reset of a megabyte state overflowed iOS's 1 MB main thread. Small types
  // keep the by-value form, which reads better and costs nothing.
  static constexpr int32_t kInPlaceBytes = 4096;
  bool in_place_type(Type t) const {
    return !device() && !t->physical && (t->kind == TypeKind::struct_ || t->kind == TypeKind::array) && t->size >= kInPlaceBytes;
  }
  bool in_place_lit(const Expr *e) const {
    return e && e->kind == ExprKind::struct_lit && in_place_type(e->type) && !control(e);
  }
  // Writes t's zeros and declared defaults over *ptr.
  std::string clear_in_place(Type t, const std::string &ptr) {
    return needs_init(t) ? "coax_initp_" + tag(t) + "(" + ptr + ");" : "memset(" + ptr + ", 0, sizeof *(" + ptr + "));";
  }
  // Stores struct literal e into *ptr, a stable pointer expression. The
  // explicit values are evaluated first, in source order, since they may
  // read the place; then the place takes the defaults and each value. A
  // literal or a scalar local cannot read the place, so it needs no temporary.
  void build_in_place(const std::string &ptr, const Expr *e) {
    const StructDecl *d = e->type->struct_decl;
    std::vector<std::string> values;
    for (const FieldInit &fi : e->fields) {
      const Expr *v = fi.value;
      Type vt = v->type;
      bool scalar = vt && (vt->kind == TypeKind::integer || vt->kind == TypeKind::floating || vt->kind == TypeKind::bool_ ||
                           vt->kind == TypeKind::enum_ || vt->kind == TypeKind::string_);
      bool stable = v->kind == ExprKind::int_lit || v->kind == ExprKind::float_lit || v->kind == ExprKind::bool_lit ||
                    v->kind == ExprKind::string_lit || v->kind == ExprKind::enum_value || (v->kind == ExprKind::ident && scalar);
      values.push_back(stable ? ex(v) : capture(v));
    }
    line(clear_in_place(e->type, ptr));
    for (size_t i = 0; i < values.size(); i++)
      line("(" + ptr + ")->" + to_std(d->fields[e->fields[(int32_t)i].field].name) + " = " + values[i] + ";");
  }

  void assign(const Stmt *s) {
    const Expr *target = s->target;
    if (s->op == Tok::equal && in_place_lit(s->expr) && !control(target)) {
      std::string p;
      if (target->kind == ExprKind::ident) p = "&(" + ex(target) + ")";
      else { p = new_temp(pointer_type(target)); line(p + " = &(" + ex(target) + ");"); }
      build_in_place(p, s->expr);
      return;
    }
    if (s->op == Tok::equal) {
      bool order = control(target) || control(s->expr) || lowering_place_first(target, s->expr, policy());
      if (!order) {
        // Same order as the C the old emitter produced under Clang: target, then value.
        std::string to = ex(target); std::string value = ex(s->expr);
        line(to + " = " + value + ";");
      } else {
        // The place first, then the value (spec §7.3).
        std::string p = new_temp(pointer_type(target));
        line(p + " = &(" + ex(target) + ");");
        line("*" + p + " = " + ex(s->expr) + ";");
      }
      return;
    }
    // A 32-bit local updated by + - or * of a 32-bit value: the local's
    // interval holds every value it stores, so the update fits as well.
    if (n32 && target->kind == ExprKind::ident && (s->op == Tok::plus_equal || s->op == Tok::minus_equal || s->op == Tok::star_equal)) {
      auto l = n32->local.find(target);
      if (l != n32->local.end() && n32->narrow_vars.count(l->second) && n32->fits(s->expr)) {
        std::string t = ex(target);
        const char *op = s->op == Tok::plus_equal ? " + " : s->op == Tok::minus_equal ? " - " : " * ";
        line(t + " = (int32_t)(" + t + ")" + op + "(int32_t)(" + ex(s->expr) + ");");
        return;
      }
    }
    // x op= v: the place is evaluated once, then v, then the update.
    Expr place{}, value{}, bin{};
    bool direct = !control(target) && !control(s->expr) && !lowering_compound_staged(target, s->expr, policy());
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
    if (!mutant_stmts.empty() && s != mutant_inner) {
      auto sites = mutant_stmts.find(s);
      if (sites != mutant_stmts.end() && (s->kind == StmtKind::assign || s->kind == StmtKind::expr)) { mutant_stmt(s, sites->second); return; }
    }
    struct Restore {Loc &target;Loc old;~Restore(){target=old;}} restore{debug_location,debug_location};
    debug_location=s->loc;
    switch (s->kind) {
      case StmtKind::transaction_: {
        transactions=true;
        std::string context="coax_tx_"+std::to_string(temp_count++);
        line("{");indent++;line("coax_tx_context "+context+" = {0};");
        std::set<std::string> captured;
        for(size_t i=transaction_locals.size();i-->0;){
          const auto &local=transaction_locals[i];
          const std::string name=local_name(local.name),where=std::to_string(s->loc.line);
          if(!captured.insert(name).second||!local.initialized)continue;
          if(local.writable)line("coax_tx_save(&"+context+", &"+name+", 1, sizeof("+name+"), "+where+", NULL);");
          if(transaction_views(local.type))line(transaction_capture_name(local.type)+"(&"+context+", &"+name+", "+where+");");
        }
        const size_t scope=defers.size();defers.push_back({});transaction_scopes.emplace(scope,context);
        stmt(s->then_);
        line("coax_tx_close(&"+context+", 0);");
        transaction_scopes.erase(scope);defers.pop_back();indent--;line("}");break;
      }
      case StmtKind::block: block_body(s); break;
      case StmtKind::expr:
        if (s->expr->kind == ExprKind::call && s->expr->builtin_print && check_binary) line("coax_nc_refuse(\"output\");");
        else if (s->expr->kind == ExprKind::call && s->expr->builtin_print) print(s->expr);
        else if (s->uninit) line("(void)(" + ex(s->expr) + ");");  // _ = e; (parser: a stmt_expr with its flag set)
        else line(ex(s->expr) + ";");
        break;
      case StmtKind::var: {
        transaction_locals.push_back({s->name,s->type,true,!s->uninit});
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
          if (s->uninit) break;  // `---`: the kernel writes each element before it reads it

          // An f64 element on Metal is a float-float pair, which 0 does not convert to.
          std::string zero = opts.metal_compute && zero_value(s->type->elem) == "coax_ff_make(0.0f, 0.0f)" ? zero_value(s->type->elem) : "0";
          line(std::string(n32 ? "for (int32_t coax_z = (int32_t)coax_lane" : opts.metal_compute ? "for (int64_t coax_z = (int64_t)coax_lane" : "for (int64_t coax_z = (int64_t)threadIdx.x") + "; coax_z < " +
               std::to_string(s->type->length) + "; coax_z += " + T + ") " + n + ".e[coax_z] = " + zero + ";");
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
        std::string decl = (n32 && n32->narrow_vars.count(s) ? std::string("int32_t") : c_type(s->type)) + " " + local_name(s->name);
        // A large local is built in place, not copied from a temporary (#13735).
        if (s->expr ? in_place_lit(s->expr) : !s->uninit && in_place_type(s->type) && needs_init(s->type)) {
          line(decl + ";");
          if (s->expr) build_in_place("&" + local_name(s->name), s->expr);
          else line(clear_in_place(s->type, "&" + local_name(s->name)));
          break;
        }
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
        emit_defers_from(0,s->expr?transaction_failure(current->ret,"coax_ret"):"0");
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
        if (mutants) {
          // Sites inside a condition write statements before it, so an
          // `else if` gets its own block.
          std::string test = ex(s->expr);
          line("if (" + mutant_condition(s, test) + ")");
          stmt(s->then_);
          if (s->else_) { line("else {"); indent++; stmt(s->else_); indent--; line("}"); }
          break;
        }
        line("if " + condition(s->expr));
        stmt(s->then_);
        if (s->else_) {
          line("else");
          stmt(s->else_);
        }
        break;
      case StmtKind::switch_: {
        std::string value=capture(s->expr);
        if(is_sum(s->expr->type))line("if ("+value+".tag >= "+std::to_string(sum_count(s->expr->type))+") coax_trap(\"invalid sum tag\", "+std::to_string(s->loc.line)+");");
        line("{");indent++;
        for(int32_t i=0;i<s->cases.count;i++){
          const SwitchCase *arm=s->cases[i];
          std::string tag=is_sum(s->expr->type)?value+".tag":value;
          std::string alternative=is_sum(s->expr->type) ? "UINT32_C("+std::to_string(arm->tag_value)+")" : int_text(s->expr->type,arm->tag_value);
          line(std::string(i?"else if (":"if (")+tag+" == "+alternative+") {");indent++;
          if(arm->binding)line("const "+c_type(arm->binding->type)+" "+local_name(arm->binding->name)+" = "+value+".payload."+sum_member((int32_t)arm->tag_value)+";");
          const size_t mark=transaction_locals.size();
          if(arm->binding)transaction_locals.push_back({arm->binding->name,arm->binding->type,false,true});
          stmt(arm->body);transaction_locals.resize(mark);indent--;line("}");
        }
        if(s->cases.count)line("else coax_trap(\"invalid switch tag\", "+std::to_string(s->loc.line)+");");
        indent--;line("}");break;
      }
      case StmtKind::while_:
        if(mutants){
          line("while (1) {");indent++;
          std::string test=ex(s->expr);line("if (!"+mutant_condition(s,"("+test+")")+") break;");
          loop(s->then_);indent--;line("}");
        }else if(control(s->expr)){
          line("while (1) {");indent++;
          std::string test=ex(s->expr);line("if (!("+test+")) break;");
          loop(s->then_);indent--;line("}");
        }else{line("while " + condition(s->expr));loop(s->then_);}
        break;
      case StmtKind::for_range: {
        if (device() && g_warp.lane_loops.count(s)) {
          // Each lane runs the body once, as its own `lane`.
          if (n32) loop(s->then_, {"const int32_t " + local_name(s->name) + " = (int32_t)coax_lane;"});
          else loop(s->then_, {"const int64_t " + local_name(s->name) + (opts.metal_compute ? " = (int64_t)coax_lane;" : " = (int64_t)(threadIdx.x & 31u);")});
          break;
        }
        if (device() && g_warp.thread_loops.count(s)) {
          // A phase: every thread of the group runs the body as its own
          // `thread`, between two barriers.
          line(group_barrier());
          if (n32) block_body(s->then_, {"const int32_t " + local_name(s->name) + " = (int32_t)coax_lane;"});
          else block_body(s->then_, {"const int64_t " + local_name(s->name) + (opts.metal_compute ? " = (int64_t)coax_lane;" : " = (int64_t)threadIdx.x;")});
          line(group_barrier());
          break;
        }
        std::string t = n32 && n32->narrow_vars.count(s) ? "int32_t" : c_type(s->type), n = std::to_string(temp_count++);
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
        std::string head = "for (" + t + " " + i + " = coax_lo" + n + "; " + i + " < coax_hi" + n + "; " + i + "++)";
        const size_t local_mark=transaction_locals.size();transaction_locals.push_back({s->name,s->type,true,true});
        if (!versioned_for(s, n, head)) {
          line(head);
          loop(s->then_);
        }
        transaction_locals.resize(local_mark);
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
        const size_t local_mark=transaction_locals.size();transaction_locals.push_back({s->name,s->type,true,true});
        if(s->name2.count)transaction_locals.push_back({s->name2,T_INT,true,true});
        loop(s->then_, prelude);
        transaction_locals.resize(local_mark);
        indent--;
        line("}");
        break;
      }
    }
  }

  // ---- loop versioning ----
  // A for loop L, outside any other versioned loop, in an optimized C
  // product (native optimization 1 or more, as --release selects; not a
  // device, Metal or verification build). -O0 output has no versioned loops. Its candidate indexes are v[x]
  // where v is a local array (its length is its type's, wherever it is
  // declared) or a view named by an identifier L never declares or assigns,
  // and x is affine: integer literals and constants, identifiers L
  // never declares or assigns, `.count` of such a view, L's counter, and the
  // counters of for loops inside L (declared once, with affine bounds, used
  // only inside their own loop), combined with + - * and / by a positive
  // integer literal. The guard evaluates
  // every subexpression's interval with exact endpoints (IV_RUNTIME); the
  // fast copy runs only when all of them fit in int64 and every candidate
  // index is inside its view or array.
  struct VScan {
    const Stmt *loop = nullptr;
    std::map<std::string, int> declared;
    std::set<std::string> assigned;
    std::map<std::string, const Stmt *> counters;              // nested for loops, by counter name
    std::map<const Stmt *, std::vector<const Stmt *>> outer;  // a nested for loop: the nested for loops around it
    std::vector<std::pair<const Expr *, std::vector<const Stmt *>>> indexes;  // with the nested for loops around each
    std::vector<const Stmt *> stack;
  };
  static std::string name_of(Str n) { return std::string(n.data, (size_t)n.count); }
  void vscan_expr(const Expr *e, VScan &v) {
    if (!e) return;
    if (e->kind == ExprKind::index) v.indexes.push_back({e, v.stack});
    vscan_expr(e->lhs, v); vscan_expr(e->rhs, v); vscan_expr(e->third, v);
    for (int32_t k = 0; k < e->args.count; k++) vscan_expr(e->args[k], v);
    for (int32_t k = 0; k < e->fields.count; k++) vscan_expr(e->fields[k].value, v);
  }
  void vscan_stmt(const Stmt *s, VScan &v) {
    if (!s) return;
    switch (s->kind) {
      case StmtKind::block: for (int32_t k = 0; k < s->body.count; k++) vscan_stmt(s->body[k], v); break;
      case StmtKind::var: v.declared[name_of(s->name)]++; vscan_expr(s->expr, v); break;
      case StmtKind::assign:
        if (s->target && s->target->kind == ExprKind::ident) v.assigned.insert(name_of(s->target->text));
        vscan_expr(s->target, v); vscan_expr(s->expr, v); break;
      case StmtKind::for_range:
        v.declared[name_of(s->name)]++;
        v.counters[name_of(s->name)] = s;
        v.outer[s] = v.stack;
        vscan_expr(s->expr, v); vscan_expr(s->expr2, v);
        v.stack.push_back(s); vscan_stmt(s->then_, v); v.stack.pop_back();
        break;
      case StmtKind::for_each:
        v.declared[name_of(s->name)]++;
        if (s->name2.count) v.declared[name_of(s->name2)]++;
        vscan_expr(s->expr, v); vscan_stmt(s->then_, v); break;
      case StmtKind::switch_:
        vscan_expr(s->expr, v);
        for (const SwitchCase *arm : s->cases) {
          if (arm->binding) v.declared[name_of(arm->binding->name)]++;
          vscan_stmt(arm->body, v);
        }
        break;
      default:
        vscan_expr(s->expr, v); vscan_expr(s->expr2, v); vscan_expr(s->target, v);
        vscan_stmt(s->then_, v); vscan_stmt(s->else_, v); break;
    }
  }
  bool invariant(const std::string &n, const VScan &v) const {
    return !v.declared.count(n) && !v.assigned.count(n) && n != name_of(v.loop->name);
  }
  // A nested counter usable where the nested for loops `around` enclose the use.
  const Stmt *counter(const std::string &n, const VScan &v, const std::vector<const Stmt *> &around) const {
    auto c = v.counters.find(n);
    if (c == v.counters.end() || v.declared.at(n) != 1 || v.assigned.count(n) || c->second->type != T_INT) return nullptr;
    if (std::find(around.begin(), around.end(), c->second) == around.end()) return nullptr;
    return c->second;
  }
  bool affine(const Expr *e, const VScan &v, const std::vector<const Stmt *> &around, int depth = 0) const {
    if (!e || e->type != T_INT || depth > 64) return false;
    switch (e->kind) {
      case ExprKind::int_lit: return true;
      case ExprKind::ident: {
        if (e->constant) return e->constant->expr && e->constant->expr->kind == ExprKind::int_lit;
        std::string n = name_of(e->text);
        if (n == name_of(v.loop->name)) return v.loop->type == T_INT && !v.declared.count(n) && !v.assigned.count(n);
        if (const Stmt *c = counter(n, v, around)) {
          const std::vector<const Stmt *> &o = v.outer.at(c);
          return affine(c->expr, v, o, depth + 1) && affine(c->expr2, v, o, depth + 1);
        }
        return invariant(n, v);
      }
      case ExprKind::binary:
        if (e->op == Tok::slash)
          return e->rhs->kind == ExprKind::int_lit && (int64_t)e->rhs->int_value > 0 && affine(e->lhs, v, around, depth + 1);
        return (e->op == Tok::plus || e->op == Tok::minus || e->op == Tok::star) && affine(e->lhs, v, around, depth + 1) &&
               affine(e->rhs, v, around, depth + 1);
      case ExprKind::field:
        return e->field_index == -2 && e->lhs->kind == ExprKind::ident && !e->lhs->constant &&
               e->lhs->type->kind == TypeKind::view && invariant(name_of(e->lhs->text), v);
      default: return false;
    }
  }
  // The guard's interval for e, as a coax_iv temporary; ok is the guard flag.
  std::string interval(const Expr *e, const VScan &v, const std::vector<const Stmt *> &around, const std::string &ok,
                       const std::string &lo, const std::string &hi, std::map<const Stmt *, std::string> &done) {
    std::string t = "coax_iv" + std::to_string(temp_count++);
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::field: {
        std::string x = ex(e);
        line("coax_iv " + t + " = {" + x + ", " + x + "};");
        return t;
      }
      case ExprKind::ident: {
        std::string n = name_of(e->text);
        if (!e->constant && n == name_of(v.loop->name)) {
          line("coax_iv " + t + " = {" + lo + ", " + lo + "};");
          line(ok + " = " + ok + " && " + lo + " < " + hi + ";");
          line("if (" + ok + ") " + t + ".hi = " + hi + " - 1;");
          return t;
        }
        if (const Stmt *c = e->constant ? nullptr : counter(n, v, around)) {
          auto d = done.find(c);
          if (d != done.end()) return d->second;
          const std::vector<const Stmt *> &o = v.outer.at(c);
          std::string a = interval(c->expr, v, o, ok, lo, hi, done), b = interval(c->expr2, v, o, ok, lo, hi, done);
          line("coax_iv " + t + " = {" + a + ".lo, " + a + ".lo};");
          line(ok + " = " + ok + " && " + a + ".lo < " + b + ".hi;");
          line("if (" + ok + ") " + t + ".hi = " + b + ".hi - 1;");
          done[c] = t;
          return t;
        }
        std::string x = ex(e);
        line("coax_iv " + t + " = {" + x + ", " + x + "};");
        return t;
      }
      default: {
        std::string a = interval(e->lhs, v, around, ok, lo, hi, done), b = interval(e->rhs, v, around, ok, lo, hi, done);
        // Division by a positive literal truncates toward zero, which keeps the order.
        if (e->op == Tok::slash) {
          line("coax_iv " + t + " = {" + a + ".lo / " + b + ".lo, " + a + ".hi / " + b + ".lo};");
          return t;
        }
        const char *f = e->op == Tok::plus ? "coax_iv_add" : e->op == Tok::minus ? "coax_iv_sub" : "coax_iv_mul";
        line("coax_iv " + t + " = {0, 0};");
        line(ok + " = " + ok + " && " + f + "(" + a + ", " + b + ", &" + t + ");");
        return t;
      }
    }
  }
  // An affine index in plain C: the guard has shown no step overflows.
  std::string plain_index(const Expr *e) {
    if (e->kind == ExprKind::binary) {
      std::string a = plain_index(e->lhs);
      std::string b = plain_index(e->rhs);
      return "(" + a + (e->op == Tok::plus ? " + " : e->op == Tok::minus ? " - " : e->op == Tok::slash ? " / " : " * ") + b + ")";
    }
    return "(int64_t)(" + ex(e) + ")";
  }
  bool versioned_for(const Stmt *s, const std::string &n, const std::string &head) {
    if (opts.optimization < 1 || device() || opts.metal || verification || version_depth > 0 || s->type != T_INT) return false;
    VScan v;
    v.loop = s;
    vscan_stmt(s->then_, v);
    if (v.declared.count(name_of(s->name))) return false;  // L's counter shadowed inside L
    std::vector<std::pair<const Expr *, std::vector<const Stmt *>>> picked;
    for (auto &x : v.indexes) {
      const Expr *e = x.first;
      Type lt = e->lhs->type;
      if (lt == &T_STRING || (lt->kind != TypeKind::view && lt->kind != TypeKind::array)) continue;
      if (e->lhs->kind != ExprKind::ident || e->lhs->constant) continue;
      if (lt->kind == TypeKind::view && !invariant(name_of(e->lhs->text), v)) continue;
      if (!affine(e->rhs, v, x.second)) continue;
      picked.push_back(x);
    }
    if (picked.empty()) return false;
    std::string ok = "coax_ok" + n, lo = "coax_lo" + n, hi = "coax_hi" + n;
    line("int " + ok + " = 1;");
    std::map<const Stmt *, std::string> done;
    for (auto &x : picked) {
      const Expr *e = x.first;
      std::string iv = interval(e->rhs, v, x.second, ok, lo, hi, done);
      std::string count = e->lhs->type->kind == TypeKind::view ? "(" + ex(e->lhs) + ").count" : "INT64_C(" + std::to_string(e->lhs->type->length) + ")";
      line(ok + " = " + ok + " && " + iv + ".lo >= 0 && " + iv + ".hi < " + count + ";");
    }
    versioned = true;
    version_depth++;
    line("if (" + ok + ")");
    line("{");
    indent++;
    for (auto &x : picked) unchecked.insert(x.first);
    line(head);
    loop(s->then_);
    unchecked.clear();
    indent--;
    line("}");
    line("else");
    line("{");
    indent++;
    line(head);
    loop(s->then_);
    indent--;
    line("}");
    version_depth--;
    return true;
  }

  void loop(const Stmt *body, const std::vector<std::string> &prelude = {}) {
    loop_scopes.push_back(defers.size());
    if (mutants) {
      // One tick an iteration: a mutant's work is limited as the evaluator's steps limit it.
      std::vector<std::string> counted = prelude;
      counted.push_back("COAX_NM_TICK();");
      block_body(body, counted);
    } else block_body(body, prelude);
    loop_scopes.pop_back();
  }

  // ---- declarations ----

  std::string prototype(const ProcDecl *p, bool header = false) {
    // #inline, where this unit holds the one definition: not a device kernel's helper, a header or a --split unit.
    const bool inline_here = p->inline_c && !header && !split && !opts.cuda && !opts.metal_compute;
    inlined = inlined || inline_here;
    const bool metal_cold_fallback = opts.metal_compute && g_narrow.count(p) && !(n32 && n32->proc == p);
    std::string s = (opts.cuda ? "static __device__ " : metal_cold_fallback ? "static __attribute__((noinline)) " : opts.metal_compute ? "static inline " : header || (p->export_c && !verification) ? "" : p->foreign && !check_binary ? "extern " : split ? "" : inline_here ? "COAX_INLINE " : "static ") + c_type(p->ret) + " " +
                    proc_name(p) + (n32 && n32->proc == p ? "_n32" : "") + "(";
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
    if (opts.metal_compute && proc_uses_am_tg(p)) s += ", threadgroup float *coax_am_tg";
    return s + ")";
  }

  std::string function(const ProcDecl *p) {
    if (p->foreign && check_binary) return "\n" + prototype(p) + " {\n  coax_nc_refuse(\"a host procedure\");\n}\n";
    if (p->foreign) return "";
    current = p;
    transaction_locals.clear();
    for(const auto &param:p->params)transaction_locals.push_back({param.name,param.type,true,true});
    mapping_active=true;debug_location=p->loc;
    out.clear();
    temps.clear();
    lowered.clear();
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
    if (check_binary) {
      auto id = coverage_ids.find(p);
      if (id != coverage_ids.end()) s += "  coax_nc_cov[" + std::to_string(id->second) + "] = 1;\n";
      if (mutants) s += "  COAX_NM_TICK();\n";
    }
    if (ret_temp) s += "  " + c_type(p->ret) + " coax_ret;\n";
    for (const std::string &t : temps) s += "  " + t + "\n";
    s += body + "}\n";
    if (opts.debug_info) s+="#line 1 \"<coax-generated>\"\n";
    return s;
  }
};

// ---- types: declarations in dependency order ----

// Header composition checks direct C ABI definitions, not logical Coax
// declaration identity. Referenced named types are checked independently by
// TypeWriter::need, so recursive borrowed records require no recursive schema
// expansion. Defaults and checkout paths do not change this C interface.
static std::string header_type_schema(Type t) {
  std::string schema;
  auto part = [&](const std::string &value) {
    schema += std::to_string(value.size()) + ":" + value;
  };
  const bool identity=t->kind==TypeKind::struct_ && t->struct_decl->identity!=0;
  part(identity ? "coax-c-id-lp64-v1" : is_sum(t) ? "coax-c-sum-lp64-v1" : "coax-c-type-v1");
  if(identity) part(std::to_string(t->struct_decl->identity));
  switch (t->kind) {
    case TypeKind::struct_: part("struct"); break;
    case TypeKind::enum_: part("enum"); break;
    case TypeKind::array: part("array"); break;
    case TypeKind::view: part("view"); break;
    case TypeKind::union_: part("union"); break;
    case TypeKind::result: part("result"); break;
    case TypeKind::optional: part("optional"); break;
    default: part("other"); break;
  }
  part(c_type(t));
  part(std::to_string(t->size));
  part(std::to_string(t->align));
  if(is_sum(t)){
    part("u32");part(std::to_string(t->sum_payload_offset));part(std::to_string(sum_count(t)));
    for(int32_t i=0;i<sum_count(t);i++){
      part(std::to_string(i));part(c_type(sum_payload(t,i)));
      if(t->kind==TypeKind::union_)part(to_std(t->union_decl->variants[i].name));
    }
  } else if (t->kind == TypeKind::struct_) {
    part(std::to_string(t->struct_decl->fields.count));
    for (const Field &field : t->struct_decl->fields) {
      part(to_std(field.name));
      part(c_type(field.type));
      part(std::to_string(field.offset));
    }
  } else if (t->kind == TypeKind::enum_) {
    part(c_type(t->elem));
    part(std::to_string(t->enum_decl->members.count));
    for (const EnumMember &member : t->enum_decl->members) {
      part(to_std(member.name));
      part(std::to_string(member.bits));
    }
  } else if (t->kind == TypeKind::array || t->kind == TypeKind::view) {
    part(c_type(t->elem));
    if (t->kind == TypeKind::array) part(std::to_string(t->length));
  }
  return schema;
}

static std::string header_type_definition(Type t, const std::string &definition) {
  static const char hex[] = "0123456789abcdef";
  std::string encoded;
  for (unsigned char byte : header_type_schema(t)) {
    encoded += hex[byte >> 4];
    encoded += hex[byte & 15];
  }
  std::string guard = "COAX_" + c_type(t) + "_DEFINED";
  std::string schema = "COAX_SCHEMA_" + c_type(t) + "_" + encoded;
  return "#ifdef " + guard + "\n#ifndef " + schema +
         "\n#error \"incompatible Coax C type " + c_type(t) + "\"\n#endif\n#else\n#define " +
         guard + "\n#define " + schema + "\n" + definition + "#endif\n";
}

struct TypeWriter {
  std::string out;
  std::vector<Type> done;
  bool header = false;

  bool seen(Type t) {
    for (Type d : done) if (d == t || ((t->kind==TypeKind::array || t->kind==TypeKind::view || is_sum(t)) && d->kind==t->kind && tag(d)==tag(t))) return true;
    return false;
  }

  void need(Type t) {
    if (seen(t)) return;
    if(t->physical) { done.push_back(t); need(t->physical); return; }
    switch (t->kind) {
      case TypeKind::struct_: {
        done.push_back(t);
        const StructDecl *d = t->struct_decl;
        for (const Field &f : d->fields) if (f.type->kind != TypeKind::view) need(f.type);
        for (const Field &f : d->fields) if (f.type->kind == TypeKind::view) need(f.type);
        std::string definition = "struct coax_" + tag(t) + " {";
        for (const Field &f : d->fields) definition += " " + c_type(f.type) + " " + to_std(f.name) + ";";
        if (!d->fields.count) definition += " char coax_empty;";
        definition += " };\n";
        out += header ? header_type_definition(t, definition) : definition;
        break;
      }
      case TypeKind::union_: case TypeKind::result: case TypeKind::optional: {
        done.push_back(t);
        bool payload=false;
        for(int32_t i=0;i<sum_count(t);i++)if(sum_payload(t,i)!=&T_VOID){need(sum_payload(t,i));payload=true;}
        std::string definition="struct "+c_type(t)+" { uint32_t tag;";
        if(payload){
          definition+=" union {";
          for(int32_t i=0;i<sum_count(t);i++)if(sum_payload(t,i)!=&T_VOID)
            definition+=" "+c_type(sum_payload(t,i))+" "+sum_member(i)+";";
          definition+=" } payload;";
        }
        definition+=" };\n";
        out+=header ? header_type_definition(t,definition) : definition;
        break;
      }
      case TypeKind::array: {
        need(t->elem);
        done.push_back(t);
        std::string definition = "typedef struct { " + c_type(t->elem) + " e[" + std::to_string(t->length) + "]; } " + c_type(t) + ";\n";
        out += header ? header_type_definition(t, definition) : definition;
        break;
      }
      case TypeKind::view:
        done.push_back(t);
        if(is_sum(t->elem)){
          // The sum has a forward alias. Publish its pointer descriptor before
          // walking a recursive union's payloads, which may use this view.
          std::string definition="typedef struct { "+std::string(t->length==1 ? "const " : "")+c_type(t->elem)+" *data; int64_t count; } "+c_type(t)+";\n";
          out+=header ? header_type_definition(t,definition) : definition;
          need(t->elem);break;
        }
        // C needs only the element's name for a pointer; a header still
        // declares the element so the host can use it.
        if (t->elem->kind != TypeKind::struct_ || header) need(t->elem);
        {
          // A view of a scalar is the same in every library, so headers
          // from two libraries can be included together.
          std::string def = "typedef struct { " + std::string(t->length==1 ? "const " : "") + c_type(t->elem) + " *data; int64_t count; } " + c_type(t) + ";\n";
          TypeKind k = t->elem->kind;
          bool shared = k == TypeKind::integer || k == TypeKind::floating || k == TypeKind::bool_;
          if (header) def = header_type_definition(t, def);
          else if (shared) {
            std::string guard = "COAX_" + c_type(t) + "_DEFINED";
            def = "#ifndef " + guard + "\n#define " + guard + "\n" + def + "#endif\n";
          }
          out += def;
        }
        break;
      case TypeKind::enum_: {
        done.push_back(t);
        std::string definition = "typedef " + c_type(t->elem) + " " + c_type(t) + ";\n";
        for (const EnumMember &m : t->enum_decl->members)
          definition += "#define COAX_" + (t->scope ? "q" + std::to_string(t->scope) + "_" : std::string()) + to_std(t->enum_decl->name) + "_" + to_std(m.name) + " " + int_text(t, m.bits) + "\n";
        out += header ? header_type_definition(t, definition) : definition;
        break;
      }
      default: break;
    }
  }

  void forward_structs(const File *file) {
    for(const UnionDecl *d:file->unions)out+="typedef struct "+c_type(d->type)+" "+c_type(d->type)+";\n";
    for(Type t:all_composite_types())if(is_sum(t))out+="typedef struct "+c_type(t)+" "+c_type(t)+";\n";
    for (const StructDecl *d : file->structs) if(!d->type->physical)
      out += "typedef struct coax_" + tag(d->type) + " coax_" + tag(d->type) + ";\n";
  }

  // The C compiler checks that Coax's layout is the C layout.
  void sum_layout_asserts(const char *alignment="_Alignof") {
    for(Type t:done)if(is_sum(t)){
      std::string c=c_type(t);
      out+="COAX_LAYOUT(sizeof("+c+") == "+std::to_string(t->size)+" && "+alignment+"("+c+") == "+std::to_string(t->align)+", \"sum layout of "+c+"\");\n";
      out+="COAX_LAYOUT(offsetof("+c+", tag) == 0, \"sum tag of "+c+"\");\n";
      if(t->sum_payload_offset)out+="COAX_LAYOUT(offsetof("+c+", payload) == "+std::to_string(t->sum_payload_offset)+", \"sum payload of "+c+"\");\n";
    }
  }
  void layout_asserts(const File *file) {
    sum_layout_asserts();
    for (const StructDecl *d : file->structs) {
      if(d->type->physical) continue;
      std::string c = c_type(d->type);
      out += "COAX_LAYOUT(sizeof(" + c + ") == " + std::to_string(d->type->size) + " && _Alignof(" + c + ") == " +
             std::to_string(d->type->align) + ", \"layout of " + to_std(d->name) + "\");\n";
      for (const Field &f : d->fields)
        out += "COAX_LAYOUT(offsetof(" + c + ", " + to_std(f.name) + ") == " + std::to_string(f.offset) +
               ", \"offset of " + to_std(d->name) + "." + to_std(f.name) + "\");\n";
    }
  }
};

static std::string transaction_captures(const std::vector<Type> &types) {
  std::map<std::string,Type> needed;
  for(Type type:types)if(transaction_views(type))needed.emplace(transaction_capture_name(type),type);
  std::string out;
  for(const auto &[name,type]:needed)out+="static void "+name+"(coax_tx_context *, const "+c_type(type)+" *, int);\n";
  for(const auto &[name,type]:needed){
    out+="static void "+name+"(coax_tx_context *tx, const "+c_type(type)+" *value, int line) {\n";
    out+="  if (!coax_txr_depth(++tx->depth)) coax_trap(\"transaction traversal limit\",line);\n";
    if(type->kind==TypeKind::view){
      out+="  static char type_tag;\n";
      out+="  if (coax_tx_save(tx,value->data,value->count,(int64_t)sizeof(*value->data),line,&type_tag)) {\n";
      if(transaction_views(type->elem))out+="    for (int64_t i=0;i<value->count;i++) "+transaction_capture_name(type->elem)+"(tx,&value->data[i],line);\n";
      out+="  }\n";
    }else if(type->kind==TypeKind::array){
      out+="  for (int64_t i=0;i<"+std::to_string(type->length)+";i++) "+transaction_capture_name(type->elem)+"(tx,&value->e[i],line);\n";
    }else if(type->kind==TypeKind::struct_){
      for(const auto &field:type->struct_decl->fields)if(transaction_views(field.type))
        out+="  "+transaction_capture_name(field.type)+"(tx,&value->"+to_std(field.name)+",line);\n";
    }else if(is_sum(type)){
      out+="  if (value->tag >= "+std::to_string(sum_count(type))+") coax_trap(\"invalid sum tag\",line);\n  switch (value->tag) {\n";
      for(int32_t i=0;i<sum_count(type);i++)if(transaction_views(sum_payload(type,i)))
        out+="    case "+std::to_string(i)+": "+transaction_capture_name(sum_payload(type,i))+"(tx,&value->payload."+sum_member(i)+",line); break;\n";
      out+="    default: break;\n  }\n";
    }
    out+="  tx->depth--;\n}\n";
  }
  return out;
}

void collect_types(const File *file, std::vector<Type> *types) {
  auto add = [&](Type t) {
    if(t->physical) return;
    for (Type x : *types) if (x == t || ((t->kind==TypeKind::array || t->kind==TypeKind::view) && x->kind==t->kind && tag(x)==tag(t))) return;
    types->push_back(t);
  };
  for (const StructDecl *d : file->structs) add(d->type);
  for (const EnumDecl *d : file->enums) add(d->type);
  for(const UnionDecl *d:file->unions)add(d->type);
  for (Type t : all_composite_types()) add(t);
}

// Each type's print and enum-conversion helpers. With `uses` (the C that
// calls them), only the helpers that C reaches are kept, through the helpers
// they call in turn: a type that is never printed or converted to gets none
// (ax-library-check-cache).
std::string show_functions(const File *file, Emitter &em, const std::string *uses = nullptr, std::string *protos_out = nullptr) {
  std::vector<Type> types;
  collect_types(file, &types);
  struct Item { std::string protos, bodies; std::vector<std::string> names; };
  std::vector<Item> items;
  // --split: defined once, in the shared unit, for every unit (#12938).
  const std::string local = em.split ? "" : "static ";
  for (Type t : types) {
    items.emplace_back();
    std::string &protos = items.back().protos, &bodies = items.back().bodies;
    std::string c = c_type(t), tg = tag(t);
    items.back().names = {"coax_show_" + tg, "coax_to_" + tg};
    if (t->kind == TypeKind::enum_) {
      protos += local + "void coax_show_" + tg + "(" + c + " v);\n";
      bodies += local + "void coax_show_" + tg + "(" + c + " v) {\n  switch (v) {\n";
      for (const EnumMember &m : t->enum_decl->members)
        bodies += "    case " + int_text(t, m.bits) + ": fputs(\"" + to_std(m.name) + "\", stdout); return;\n";
      bodies += "    default: break;\n  }\n  fputs(\"" + to_std(t->enum_decl->name) + "(\", stdout); " +
                (t->elem->is_signed ? "coax_print_int((int64_t)v);" : "coax_print_uint((uint64_t)v);") +
                " fputc(')', stdout);\n}\n";
      // Checked conversion from an integer: it must name a member.
      bodies += local + c + " coax_to_" + tg + "(int64_t s, uint64_t u, int is_signed, int32_t l) {\n";
      for (const EnumMember &m : t->enum_decl->members) {
        bool negative = t->elem->is_signed && (int64_t)m.bits < 0;
        if (negative) bodies += "  if (is_signed && s == INT64_C(" + std::to_string((int64_t)m.bits) + ")) return " + int_text(t, m.bits) + ";\n";
        else bodies += "  if (is_signed ? (s >= 0 && (uint64_t)s == UINT64_C(" + std::to_string(m.bits) + ")) : u == UINT64_C(" +
                       std::to_string(m.bits) + ")) return " + int_text(t, m.bits) + ";\n";
      }
      bodies += "  coax_trap(\"value is not a member of enum " + to_std(t->enum_decl->name) + "\", l);\n  return 0;\n}\n";
      protos += local + c + " coax_to_" + tg + "(int64_t s, uint64_t u, int is_signed, int32_t l);\n";
      continue;
    }
    protos += local + "void coax_show_" + tg + "(const " + c + " *v);\n";
    bodies += local + "void coax_show_" + tg + "(const " + c + " *v) {\n";
    if(is_sum(t)){
      bodies+="  switch(v->tag) {\n";
      for(int32_t i=0;i<sum_count(t);i++){
        Type payload=sum_payload(t,i);
        std::string label=t->kind==TypeKind::union_ ? to_std(t->union_decl->variants[i].name) : t->kind==TypeKind::result ? (i==0?"ok":"err") : (i==0?"none":"some");
        bodies+="  case "+std::to_string(i)+": fputs(\""+label+"\", stdout);";
        if(payload!=&T_VOID)bodies+=" fputc('(', stdout); "+em.show_call(payload,"v->payload."+sum_member(i),false)+"; fputc(')', stdout);";
        bodies+=" return;\n";
      }
      bodies+="  default: coax_trap(\"invalid sum tag\",0);\n  }\n";
    } else if (t->kind == TypeKind::struct_) {
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
  std::vector<bool> kept(items.size(), !uses);
  if (uses) {
    // A helper's name is used when it appears followed by '(' in kept C.
    auto calls = [](const std::string &text, const std::string &name) {
      for (size_t at = text.find(name); at != std::string::npos; at = text.find(name, at + 1))
        if (at + name.size() < text.size() && text[at + name.size()] == '(' &&
            (at == 0 || !(isalnum((unsigned char)text[at - 1]) || text[at - 1] == '_')))
          return true;
      return false;
    };
    // The same answer in one pass over each text (#12925: searching the
    // whole program's C once per helper name took seconds): every identifier
    // that starts where no identifier character precedes it and is followed
    // by '(' is a call; a helper is kept when one of its names is called.
    auto ident = [](char c) { return isalnum((unsigned char)c) || c == '_'; };
    std::unordered_map<std::string, std::vector<size_t>> named;
    bool plain = true;
    for (size_t i = 0; i < items.size(); i++)
      for (const std::string &name : items[i].names) {
        plain = plain && !name.empty() && std::all_of(name.begin(), name.end(), ident);
        named[name].push_back(i);
      }
    std::vector<const std::string *> pending{uses};
    while (!pending.empty()) {
      const std::string *text = pending.back();
      pending.pop_back();
      if (!plain) {
        for (size_t i = 0; i < items.size(); i++) {
          if (kept[i]) continue;
          for (const std::string &name : items[i].names)
            if (calls(*text, name)) { kept[i] = true; pending.push_back(&items[i].bodies); break; }
        }
        continue;
      }
      const std::string &t = *text;
      for (size_t at = 0; at < t.size();) {
        if (!ident(t[at]) || (at > 0 && ident(t[at - 1]))) { at++; continue; }
        size_t end = at;
        while (end < t.size() && ident(t[end])) end++;
        if (end < t.size() && t[end] == '(') {
          auto found = named.find(t.substr(at, end - at));
          if (found != named.end())
            for (size_t i : found->second)
              if (!kept[i]) { kept[i] = true; pending.push_back(&items[i].bodies); }
        }
        at = end;
      }
    }
  }
  std::string protos, bodies;
  for (size_t i = 0; i < items.size(); i++) if (kept[i]) { protos += items[i].protos; bodies += items[i].bodies; }
  if (protos_out) { *protos_out = protos; return bodies; }
  return protos + bodies;
}

std::string view_helpers() {
  std::string s;
  std::set<std::string> emitted;
  for (Type t : all_composite_types()) {
    if (t->kind != TypeKind::view || !emitted.insert(tag(t)).second) continue;
    std::string c = c_type(t), e = (t->length==1 ? "const " : "") + c_type(t->elem), tg = tag(t);
    if (t->elem == &T_U8)
      s += "static coax_string coax_string_of_bytes" + std::string(t->length==1 ? "_readonly" : "") + "(" + c + " v) { coax_string s = {(const char *)v.data, v.count}; return s; }\n";
    s += "static " + e + " *coax_elem_" + tg + "(" + c + " v, int64_t i, int32_t l) { return &v.data[coax_index(i, v.count, l)]; }\n";
    s += "static " + c + " coax_mkview_" + tg + "(" + e + " *data, int64_t n, int64_t lo, int64_t hi, int32_t l) {\n"
         "  coax_slice_check(lo, hi, n, l);\n  " + c + " r = {data + lo, hi - lo};\n  return r;\n}\n";
    s += "static " + c + " coax_reslice_" + tg + "(" + c + " v, int64_t lo, int64_t hi, int32_t l) {\n"
         "  coax_slice_check(lo, hi, v.count, l);\n  " + c + " r = {v.data + lo, hi - lo};\n  return r;\n}\n";
  }
  return s;
}

std::string init_functions(const File *file, Emitter &em, std::string *protos_out = nullptr) {
  std::vector<Type> types;
  collect_types(file, &types);
  std::string protos, bodies;
  const std::string local = em.split ? "" : "static ";  // --split: one definition (#12938)
  for (Type t : types) {
    if (!needs_init(t)) continue;
    // coax_initp_T writes T's defaults in place over any bytes, nested
    // fields and elements too, so no copy of T lands on a stack (#13735):
    // a value can be megabytes and a thread's stack 512 KB. coax_init_T is
    // the by-value form for expressions.
    std::string c = c_type(t), tg = tag(t);
    protos += local + "void coax_initp_" + tg + "(" + c + " *v);\n";
    protos += local + c + " coax_init_" + tg + "(void);\n";
    bodies += local + "void coax_initp_" + tg + "(" + c + " *v) {\n  memset(v, 0, sizeof *v);\n";
    if (t->kind == TypeKind::struct_) {
      for (const Field &f : t->struct_decl->fields) {
        if (f.init) bodies += "  v->" + to_std(f.name) + " = " + em.ex(f.init) + ";\n";
        else if (needs_init(f.type)) bodies += "  coax_initp_" + tag(f.type) + "(&v->" + to_std(f.name) + ");\n";
      }
    } else {
      bodies += "  for (int64_t i = 0; i < " + std::to_string(t->length) + "; i++) coax_initp_" + tag(t->elem) + "(&v->e[i]);\n";
    }
    bodies += "}\n";
    bodies += local + c + " coax_init_" + tg + "(void) {\n  " + c + " v;\n  coax_initp_" + tg + "(&v);\n  return v;\n}\n";
  }
  if (protos_out) { *protos_out = protos; return bodies; }
  return protos + bodies;
}

// The types a --library header declares: those the exported and foreign
// procedures use, and everything they contain.
TypeWriter header_types(const File *file) {
  TypeWriter types;
  types.header = true;
  for (const ProcDecl *p : file->procs) {
    // A #foreign prototype names its view parameters' types too.
    if ((!p->export_c && !p->foreign) || p->check_unit) continue;
    types.need(p->ret);
    for (const Param &prm : p->params) types.need(prm.type);
  }
  return types;
}

// A header type a host constructs: a struct or array with a declared default.
bool host_default(Type t) {
  return (t->kind == TypeKind::struct_ || t->kind == TypeKind::array) && needs_init(t);
}

// State construction for hosts (spec 17.2): each header type with a declared
// default gets coax_default_T(T *p), which writes those defaults in place over
// any bytes, so a host's zeroed or reused state starts as `s: [1] T` does
// natively. In place, because a state can be larger than a host's stack.
// Weak, because two units that share a type both define it, identically.
std::string state_defaults(const File *file, Emitter &em) {
  std::string protos, bodies;
  for (Type t : header_types(file).done) {
    if (!host_default(t)) continue;
    std::string c = c_type(t), tg = tag(t);
    protos += "void coax_default_" + tg + "(" + c + " *p);\n";
    bodies += "__attribute__((weak)) void coax_default_" + tg + "(" + c + " *p) {\n  memset(p, 0, sizeof *p);\n";
    if (t->kind == TypeKind::struct_) {
      for (const Field &f : t->struct_decl->fields) {
        if (f.init) bodies += "  p->" + to_std(f.name) + " = " + em.ex(f.init) + ";\n";
        else if (needs_init(f.type)) bodies += "  coax_default_" + tag(f.type) + "(&p->" + to_std(f.name) + ");\n";
      }
    } else {
      bodies += "  for (int64_t i = 0; i < " + std::to_string(t->length) + "; i++) coax_default_" + tag(t->elem) + "(&p->e[i]);\n";
    }
    bodies += "}\n";
  }
  if (bodies.empty()) return "";
  return "\n/* Struct defaults for hosts: coax_default_T writes T's declared defaults in place. */\n" + protos + bodies;
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
static bool sum_runtime_profile(const File *file) {
  if(file->unions.count)return true;
  for(Type t:all_composite_types())if(is_sum(t))return true;
  std::function<bool(const Stmt *)> switched=[&](const Stmt *stmt){
    if(!stmt)return false;
    if(stmt->kind==StmtKind::switch_)return true;
    for(const Stmt *child:stmt->body)if(switched(child))return true;
    return switched(stmt->then_) || switched(stmt->else_);
  };
  for(const ProcDecl *proc:file->procs)if(switched(proc->body))return true;
  for(const CheckDecl *check:file->checks)if(switched(check->proc->body))return true;
  return false;
}

static std::string source_runtime(const File *file, bool verification = false) {
  auto declared_runtime=[&](std::string runtime){
    if(sum_runtime_profile(file)){
      const std::string declaration="static void coax_trap(";
      runtime.replace(runtime.find(declaration),declaration.size(),"static _Noreturn void coax_trap(");
    }
    return runtime;
  };
  if (file->src->units.size()<2 && !verification) return declared_runtime(RUNTIME);
  std::string out="struct coax_source_range { int first, last, base, fixed; const char *path; };\n";
  if (verification) {
    const std::string &path = file->src->units.empty() ? file->src->path : file->src->units.front()->canonical;
    out += "static const char *coax_verification_source_path = \"" + escape_path(path) + "\";\n";
  }
  out+="static const struct coax_source_range coax_source_ranges[] = {\n";
  auto range=[&](int first,int last,int base,bool fixed,const std::string &path) {
    out+="  {"+std::to_string(first)+","+std::to_string(last)+","+std::to_string(base)+","+(fixed?"1":"0")+",\""+escape_path(path)+"\"},\n";
  };
  for (const auto &r:file->src->generated) {
    int first=1+(int)std::count(file->src->text.begin(),file->src->text.begin()+r.start,'\n');
    int last=1+(int)std::count(file->src->text.begin(),file->src->text.begin()+r.end,'\n');
    auto point=source_point(file->src,r.directive);
    const SourceUnit *unit = verification ? source_unit_at(file->src, r.directive) : nullptr;
    range(first,last,point.loc.line,true,unit ? unit->canonical : point.path);
  }
  for (const auto &unit:file->src->units) {
    int count=(int)std::count(file->src->text.begin()+unit->begin,file->src->text.begin()+unit->end,'\n');
    range(unit->first_line,unit->first_line+std::max(1,count)-1,1,false,verification ? unit->canonical : unit->path);
  }
  out+="};\nstatic const struct coax_source_range *coax_find_source(int line) {\n";
  out+="  for (unsigned i=0;i<sizeof(coax_source_ranges)/sizeof(coax_source_ranges[0]);i++) if (line>=coax_source_ranges[i].first && line<=coax_source_ranges[i].last) return &coax_source_ranges[i];\n  return 0;\n}\n";
  out+="static const char *coax_path_for(int line) { const struct coax_source_range *r=coax_find_source(line); return r?r->path:"+
       std::string(verification ? "coax_verification_source_path" : "coax_source_path")+"; }\n";
  out+="static int coax_line_for(int line) { const struct coax_source_range *r=coax_find_source(line); return r?(r->fixed?r->base:r->base+line-r->first):line; }\n";
  std::string runtime=RUNTIME;std::string needle="coax_source_path, line, what";
  runtime.replace(runtime.find(needle),needle.size(),"coax_path_for(line), coax_line_for(line), what");
  if (verification) {
    const std::string trap = "static void coax_trap(const char *what, int32_t line) {";
    const std::string handler =
        "typedef void (*Coax_VerificationTrapHandler_v1)(const char *, int64_t, const char *, int64_t, int64_t);\n"
        "static Coax_VerificationTrapHandler_v1 coax_verification_trap_handler_v1;\n"
        "void coax_verification_set_trap_v1(Coax_VerificationTrapHandler_v1 handler) { coax_verification_trap_handler_v1 = handler; }\n\n";
    size_t at = runtime.find(trap);
    runtime.insert(at, handler);
    const std::string flush = "  fflush(stdout);\n";
    at = runtime.find(flush, at + handler.size()) + flush.size();
    runtime.insert(at,
        "  if (coax_verification_trap_handler_v1) {\n"
        "    const char *path = coax_path_for(line);\n"
        "    coax_verification_trap_handler_v1(what, (int64_t)strlen(what), path, (int64_t)strlen(path), coax_line_for(line));\n"
        "  }\n");
  }
  return out+declared_runtime(std::move(runtime));
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
    for(const SwitchCase *arm:st->cases)visit_stmt(arm->body);
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
COAX_DEV void coax_am_sg_zero(thread simdgroup_float8x8 *acc) {
  for (int i = 0; i < 8; i++) acc[i] = simdgroup_float8x8(0.0f);
}
COAX_DEV void coax_am_sg_mma_f16(thread simdgroup_float8x8 *acc, threadgroup const half *a, threadgroup const half *b, uint tid) {
  uint g = tid / 32u; int sr = (int)(g & 1u) * 32, sc = (int)(g >> 1) * 16;
  for (int kk = 0; kk < 32; kk += 8) {
    simdgroup_half8x8 A[4], B[2];
    for (int i = 0; i < 4; i++) simdgroup_load(A[i], a + (sr + 8 * i) * 32 + kk, 32);
    for (int j = 0; j < 2; j++) simdgroup_load(B[j], b + (sc + 8 * j) * 32 + kk, 32, ulong2(0, 0), true);
    for (int i = 0; i < 4; i++)
      for (int j = 0; j < 2; j++) simdgroup_multiply_accumulate(acc[2 * i + j], A[i], B[j], acc[2 * i + j]);
  }
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
template <int TM, int TN, int SG>
COAX_DEV void coax_am_matmul_x16(int64_t rows, int64_t cols, int64_t tokens, int64_t r0, int64_t t0, coax_v_u32 w, coax_v_u16 x, coax_v_f32 out, int32_t line, COAX_ERR) {
  using namespace mpp::tensor_ops;
  if (rows < 1 || cols < 2 || cols % 2 != 0 || tokens < 1 || r0 < 0 || r0 >= rows || t0 < 0 || t0 >= tokens || rows > 2147483647 / cols ||
      tokens > 2147483647 / cols || tokens > 2147483647 / rows || w.count < rows * (cols / 2) || x.count < tokens * cols || out.count < tokens * rows) {
    coax_trap_(line, coax_err);
    return;
  }
  int c = (int)cols, nt = (int)min((int64_t)TM, tokens - t0), nr = (int)min((int64_t)TN, rows - r0);
  auto X = tensor<device bfloat, dextents<int32_t, 2>, tensor_inline>((device bfloat *)x.data + t0 * cols, dextents<int32_t, 2>(c, nt), array<int32_t, 2>({1, c}));
  auto W = tensor<device bfloat, dextents<int32_t, 2>, tensor_inline>((device bfloat *)w.data + r0 * cols, dextents<int32_t, 2>(c, nr), array<int32_t, 2>({1, c}));
  auto C = tensor<device float, dextents<int32_t, 2>, tensor_inline>(out.data + t0 * rows + r0, dextents<int32_t, 2>(nr, nt), array<int32_t, 2>({1, (int)rows}));
  constexpr auto d = matmul2d_descriptor(TM, TN, static_cast<int>(dynamic_extent), false, true, true);
  matmul2d<d, execution_simdgroups<SG>> op;
  op.run(X, W, C);
}
static constant float coax_iq4_lut[16] = {-127.0f, -104.0f, -83.0f, -65.0f, -49.0f, -35.0f, -22.0f, -10.0f, 1.0f, 13.0f, 25.0f, 38.0f, 53.0f, 69.0f, 89.0f, 113.0f};
COAX_DEV float coax_am_dot_iq4_lane(coax_v_u16 w, int64_t base, coax_v_f32 x, int64_t x_off, int64_t blocks, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || base < 0 || base + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u);
  int xq = (int)(qtr * 4u);
  const device uint16_t *wp = w.data + base;
  const device float *xp = x.data + x_off;
  float acc = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
    const device uint16_t *bw = wp + b * 9;
    const device float *bx = xp + b * 32 + xq;
    float4 xl = *(const device float4 *)bx;
    float4 xh = *(const device float4 *)(bx + 16);
    float sc = float(as_type<half>(bw[0]));
    packed_ushort2 _vv = *(const device packed_ushort2 *)(bw + 1 + k0); uint32_t v0 = _vv.x, v1 = _vv.y;
    float s = coax_iq4_lut[v0 & 15u] * xl.x
            + coax_iq4_lut[(v0 >> 4u) & 15u] * xh.x
            + coax_iq4_lut[(v0 >> 8u) & 15u] * xl.y
            + coax_iq4_lut[v0 >> 12u] * xh.y
            + coax_iq4_lut[v1 & 15u] * xl.z
            + coax_iq4_lut[(v1 >> 4u) & 15u] * xh.z
            + coax_iq4_lut[(v1 >> 8u) & 15u] * xl.w
            + coax_iq4_lut[v1 >> 12u] * xh.w;
    acc += sc * s;
  }
  return acc;
}
COAX_DEV float coax_q4_alu_dot8_f16(packed_ushort2 p, half4 xl, half4 xh, float xsum) {
  uint32_t a = p.x, c = p.y;
  half4 s = half4(a & 15u, (a >> 8u) & 15u, c & 15u, (c >> 8u) & 15u) * xl
          + half4((a >> 4u) & 15u, a >> 12u, (c >> 4u) & 15u, c >> 12u) * xh;
  return float((s.x + s.y) + (s.z + s.w)) - 8.0f * xsum;
}
COAX_DEV float coax_q4_alu_dot16_f16(packed_ushort4 p, half4 xl0, half4 xl1, half4 xh0, half4 xh1, float xsum) {
  uint32_t a = p.x, c = p.y, d = p.z, e = p.w;
  half4 s = half4(a & 15u, (a >> 8u) & 15u, c & 15u, (c >> 8u) & 15u) * xl0
          + half4((a >> 4u) & 15u, a >> 12u, (c >> 4u) & 15u, c >> 12u) * xh0
          + half4(d & 15u, (d >> 8u) & 15u, e & 15u, (e >> 8u) & 15u) * xl1
          + half4((d >> 4u) & 15u, d >> 12u, (e >> 4u) & 15u, e >> 12u) * xh1;
  return float((s.x + s.y) + (s.z + s.w)) - 8.0f * xsum;
}
COAX_DEV float coax_q4s_dot16_u2(uint2 v, half4 xl0, half4 xl1, half4 xh0, half4 xh1, float xsum) {
  half4 s = half4(as_type<uchar4>(v.x & 0x0f0f0f0fu)) * xl0
          + half4(as_type<uchar4>((v.x >> 4u) & 0x0f0f0f0fu)) * xh0
          + half4(as_type<uchar4>(v.y & 0x0f0f0f0fu)) * xl1
          + half4(as_type<uchar4>((v.y >> 4u) & 0x0f0f0f0fu)) * xh1;
  return float((s.x + s.y) + (s.z + s.w)) - 8.0f * xsum;
}
COAX_DEV float coax_q4s_dot16_pre(half4 p0, half4 p1, half4 p2, half4 p3, half4 xl0, half4 xl1, half4 xh0, half4 xh1, float xsum) {
  half4 s = p0 * xl0 + p1 * xh0 + p2 * xl1 + p3 * xh1;
  return float((s.x + s.y) + (s.z + s.w)) - 8.0f * xsum;
}
COAX_DEV void coax_sort_eids_asc(const device uint32_t *ids, int n_used, thread uint8_t *ord) {
  for (int i = 0; i < n_used; i++) ord[i] = (uint8_t)i;
  for (int i = 1; i < n_used; i++) {
    uint8_t k = ord[i]; uint32_t ke = ids[k]; int j = i - 1;
    while (j >= 0 && ids[ord[j]] > ke) { ord[j + 1] = ord[j]; j--; }
    ord[j + 1] = k;
  }
}
COAX_DEV float coax_inv_rms_parts(coax_v_f32 parts, int64_t p_off, int64_t n_parts, int64_t dim, float eps, uint32_t lane) {
  if (n_parts <= 0) return 1.0f;
  float s = 0.0f;
  for (int i = (int)lane; i < (int)n_parts; i += 32) s += parts.data[p_off + i];
  return rsqrt(simd_sum(s) / float(dim) + eps);
}
COAX_DEV float coax_am_dot_q4_lane(coax_v_u16 w, int64_t base, coax_v_f32 x, int64_t x_off, int64_t blocks, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || base < 0 || base + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u);
  int xq = (int)(qtr * 4u);
  const device uint16_t *wp = w.data + base;
  const device float *xp = x.data + x_off;
  float acc = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    float4 xlf = *(const device float4 *)bx;
    float4 xhf = *(const device float4 *)(bx + 16);
    float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
    const device uint16_t *bw = wp + b * 9;
    acc += float(as_type<half>(bw[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw + 1 + k0), half4(xlf), half4(xhf), xsum);
  }
  return acc;
}
static constant half coax_iq4_lut_f16s[16] = {-7.9375h, -6.5h, -5.1875h, -4.0625h, -3.0625h, -2.1875h, -1.375h, -0.625h, 0.0625h, 0.8125h, 1.5625h, 2.375h, 3.3125h, 4.3125h, 5.5625h, 7.0625h};
COAX_DEV float coax_iq4_dot16_f16s(packed_ushort4 p, half4 xl0, half4 xl1, half4 xh0, half4 xh1) {
  uint32_t a = p.x, c = p.y, d = p.z, e = p.w;
  half4 s = half4(coax_iq4_lut_f16s[a & 15u], coax_iq4_lut_f16s[(a >> 8u) & 15u], coax_iq4_lut_f16s[c & 15u], coax_iq4_lut_f16s[(c >> 8u) & 15u]) * xl0
          + half4(coax_iq4_lut_f16s[(a >> 4u) & 15u], coax_iq4_lut_f16s[a >> 12u], coax_iq4_lut_f16s[(c >> 4u) & 15u], coax_iq4_lut_f16s[c >> 12u]) * xh0
          + half4(coax_iq4_lut_f16s[d & 15u], coax_iq4_lut_f16s[(d >> 8u) & 15u], coax_iq4_lut_f16s[e & 15u], coax_iq4_lut_f16s[(e >> 8u) & 15u]) * xl1
          + half4(coax_iq4_lut_f16s[(d >> 4u) & 15u], coax_iq4_lut_f16s[d >> 12u], coax_iq4_lut_f16s[(e >> 4u) & 15u], coax_iq4_lut_f16s[e >> 12u]) * xh1;
  return float((s.x + s.y) + (s.z + s.w));
}
COAX_DEV float coax_am_dot_iq4_situ_lane(coax_v_u16 w, int64_t gb, int64_t ub, coax_v_f32 x, int64_t x_off, int64_t blocks, float beta, float lb, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || gb < 0 || gb + blocks * 9 > w.count || ub < 0 || ub + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  uint32_t lane = tid & 31u;
  uint32_t is_up = lane >> 4;
  uint32_t sub = lane & 15u;
  uint32_t blk_lane = sub >> 1;
  uint32_t half_b = sub & 1u;
  int k0 = (int)(half_b * 4u);
  int xq = (int)(half_b * 8u);
  const device uint16_t *wp = w.data + (is_up ? ub : gb);
  const device float *xp = x.data + x_off;
  float acc = 0.0f;
  int iblocks = (int)blocks;
  if (iblocks == 72) {
    for (int b = (int)blk_lane; b < 72; b += 8) {
      const device float *bx = xp + b * 32 + xq;
      half4 xl0 = half4(*(const device float4 *)bx);
      half4 xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16));
      half4 xh1 = half4(*(const device float4 *)(bx + 20));
      const device uint16_t *bw = wp + b * 9;
      float sc = 16.0f * float(as_type<half>(bw[0]));
      acc += sc * coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw + 1 + k0), xl0, xl1, xh0, xh1);
    }
  } else {
    for (int b = (int)blk_lane; b < iblocks; b += 8) {
      const device float *bx = xp + b * 32 + xq;
      half4 xl0 = half4(*(const device float4 *)bx);
      half4 xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16));
      half4 xh1 = half4(*(const device float4 *)(bx + 20));
      const device uint16_t *bw = wp + b * 9;
      float sc = 16.0f * float(as_type<half>(bw[0]));
      acc += sc * coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw + 1 + k0), xl0, xl1, xh0, xh1);
    }
  }
  acc += simd_shuffle_xor(acc, 8);
  acc += simd_shuffle_xor(acc, 4);
  acc += simd_shuffle_xor(acc, 2);
  acc += simd_shuffle_xor(acc, 1);
  float u = simd_shuffle(acc, 16);
  if (lane != 0) return 0.0f;
  float act = beta * tanh(acc / beta) / (1.0f + exp(-acc));
  if (lb > 0.0f) u = lb * tanh(u / lb);
  return act * u;
}
COAX_DEV float coax_am_dot_q4_situ_lane(coax_v_u16 w, int64_t gb, int64_t ub, coax_v_f32 x, int64_t x_off, int64_t blocks, float beta, float lb, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || gb < 0 || gb + blocks * 9 > w.count || ub < 0 || ub + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  uint32_t lane = tid & 31u;
  uint32_t is_up = lane >> 4;
  uint32_t sub = lane & 15u;
  uint32_t blk_lane = sub >> 1;
  uint32_t half_b = sub & 1u;
  int k0 = (int)(half_b * 4u);
  int xq = (int)(half_b * 8u);
  const device uint16_t *wp = w.data + (is_up ? ub : gb);
  const device float *xp = x.data + x_off;
  float acc = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
    float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
    float4 fs = (f0 + f1) + (f2 + f3);
    float xsum = (fs.x + fs.y) + (fs.z + fs.w);
    const device uint16_t *bw = wp + b * 9;
    acc += float(as_type<half>(bw[0])) * coax_q4_alu_dot16_f16(*(const device packed_ushort4 *)(bw + 1 + k0), half4(f0), half4(f1), half4(f2), half4(f3), xsum);
  }
  acc += simd_shuffle_xor(acc, 8);
  acc += simd_shuffle_xor(acc, 4);
  acc += simd_shuffle_xor(acc, 2);
  acc += simd_shuffle_xor(acc, 1);
  float u = simd_shuffle(acc, 16);
  if (lane != 0) return 0.0f;
  float act = beta * tanh(acc / beta) / (1.0f + exp(-acc));
  if (lb > 0.0f) u = lb * tanh(u / lb);
  return act * u;
}
COAX_DEV float coax_am_dot_iq4_moe_lane(coax_v_u16 w, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 x, int64_t exp_words, int64_t row_off, int64_t cols, int64_t n_used, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (n_used < 0 || cols < 0 || (cols & 31) != 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || row_off < 0 || exp_words < 0) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u);
  int xq = (int)(qtr * 4u);
  float acc = 0.0f;
  for (int slot = 0; slot < (int)n_used; slot++) {
    int64_t eid = (int64_t)ids.data[slot];
    int64_t rb = eid * exp_words + row_off;
    if (rb + blocks * 9 > w.count) { coax_trap_(line, coax_err); return 0.0f; }
    float ew = wts.data[slot];
    const device uint16_t *wp = w.data + rb;
    const device float *xp = x.data + slot * cols;
    float sa = 0.0f;
    for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
      const device uint16_t *bw = wp + b * 9;
      const device float *bx = xp + b * 32 + xq;
      float4 xl = *(const device float4 *)bx;
      float4 xh = *(const device float4 *)(bx + 16);
      float sc = float(as_type<half>(bw[0]));
      packed_ushort2 _vv = *(const device packed_ushort2 *)(bw + 1 + k0); uint32_t v0 = _vv.x, v1 = _vv.y;
      float s = coax_iq4_lut[v0 & 15u] * xl.x
              + coax_iq4_lut[(v0 >> 4u) & 15u] * xh.x
              + coax_iq4_lut[(v0 >> 8u) & 15u] * xl.y
              + coax_iq4_lut[v0 >> 12u] * xh.y
              + coax_iq4_lut[v1 & 15u] * xl.z
              + coax_iq4_lut[(v1 >> 4u) & 15u] * xh.z
              + coax_iq4_lut[(v1 >> 8u) & 15u] * xl.w
              + coax_iq4_lut[v1 >> 12u] * xh.w;
      sa += sc * s;
    }
    acc += ew * sa;
  }
  return simd_sum(acc);
}
COAX_DEV float coax_am_dot_q4_moe_lane(coax_v_u16 w, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 x, int64_t exp_words, int64_t row_off, int64_t cols, int64_t n_used, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (n_used < 0 || cols < 0 || (cols & 31) != 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || row_off < 0 || exp_words < 0) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  for (int slot = 0; slot < (int)n_used; slot++) {
    int64_t eid = (int64_t)ids.data[slot];
    if (eid * exp_words + row_off + blocks * 9 > w.count) { coax_trap_(line, coax_err); return 0.0f; }
  }
  uint32_t lane = tid & 31u;
  int iblocks = (int)blocks;
  int total_b = (int)n_used * iblocks;
  float acc = 0.0f;
  if (iblocks == 36) {
    for (int tb = (int)lane; tb < total_b; tb += 32) {
      int slot = tb / 36;
      int b = tb - slot * 36;
      int64_t eid = (int64_t)ids.data[slot];
      float ew = wts.data[slot];
      const device float *bx = x.data + slot * 1152 + b * 32;
      const device uint16_t *bw = w.data + eid * exp_words + row_off + b * 9;
      float sc = ew * float(as_type<half>(bw[0]));
      float s = 0.0f;
      for (int q = 0; q < 2; q++) {
        float4 f0 = *(const device float4 *)(bx + q * 8), f1 = *(const device float4 *)(bx + q * 8 + 4);
        float4 f2 = *(const device float4 *)(bx + q * 8 + 16), f3 = *(const device float4 *)(bx + q * 8 + 20);
        float4 fs = (f0 + f1) + (f2 + f3);
        float xsum = (fs.x + fs.y) + (fs.z + fs.w);
        s += coax_q4_alu_dot16_f16(*(const device packed_ushort4 *)(bw + 1 + q * 4), half4(f0), half4(f1), half4(f2), half4(f3), xsum);
      }
      acc += sc * s;
    }
  } else {
    for (int tb = (int)lane; tb < total_b; tb += 32) {
      int slot = tb / iblocks;
      int b = tb - slot * iblocks;
      int64_t eid = (int64_t)ids.data[slot];
      float ew = wts.data[slot];
      const device float *bx = x.data + slot * (int)cols + b * 32;
      const device uint16_t *bw = w.data + eid * exp_words + row_off + b * 9;
      float sc = ew * float(as_type<half>(bw[0]));
      float s = 0.0f;
      for (int q = 0; q < 2; q++) {
        float4 f0 = *(const device float4 *)(bx + q * 8), f1 = *(const device float4 *)(bx + q * 8 + 4);
        float4 f2 = *(const device float4 *)(bx + q * 8 + 16), f3 = *(const device float4 *)(bx + q * 8 + 20);
        float4 fs = (f0 + f1) + (f2 + f3);
        float xsum = (fs.x + fs.y) + (fs.z + fs.w);
        s += coax_q4_alu_dot16_f16(*(const device packed_ushort4 *)(bw + 1 + q * 4), half4(f0), half4(f1), half4(f2), half4(f3), xsum);
      }
      acc += sc * s;
    }
  }
  return simd_sum(acc);
}
COAX_DEV float coax_am_dot_f16_lane(coax_v_u16 w, int64_t base, coax_v_f32 x, int64_t x_off, int64_t cols, uint32_t tid, int32_t line, COAX_ERR) {
  if (cols < 0 || base < 0 || base + cols > w.count || x_off < 0 || x_off + cols > x.count) {
    coax_trap_(line, coax_err);
    return 0.0f;
  }
  uint32_t lane = tid & 31u;
  const device uint16_t *wp = w.data + base;
  const device float *xp = x.data + x_off;
  int n4 = (int)(cols >> 2);
  float4 acc4 = float4(0.0f);
  const device half4 *w4 = (const device half4 *)wp;
  const device float4 *x4 = (const device float4 *)xp;
  for (int i = (int)lane; i < n4; i += 32) {
    acc4 += float4(w4[i]) * x4[i];
  }
  float acc = (acc4.x + acc4.y) + (acc4.z + acc4.w);
  int tail = n4 << 2;
  if (tail + (int)lane < (int)cols) {
    acc += float(as_type<half>(wp[tail + (int)lane])) * xp[tail + (int)lane];
  }
  return simd_sum(acc);
}
COAX_DEV void coax_am_dot_q4_2_write(coax_v_u16 w, int64_t b0, int64_t b1, coax_v_f32 x, int64_t x_off, int64_t blocks, coax_v_f32 out, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || b0 < 0 || b0 + blocks * 9 > w.count || b1 < 0 || b1 + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count || r0 < 0 || r0 >= out.count || r1 < 0 || r1 >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u);
  int xq = (int)(qtr * 4u);
  const device uint16_t *wp0 = w.data + b0;
  const device uint16_t *wp1 = w.data + b1;
  const device float *xp = x.data + x_off;
  float acc0 = 0.0f, acc1 = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    float4 xlf = *(const device float4 *)bx;
    float4 xhf = *(const device float4 *)(bx + 16);
    float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
    half4 xl = half4(xlf), xh = half4(xhf);
    const device uint16_t *bw0 = wp0 + b * 9;
    const device uint16_t *bw1 = wp1 + b * 9;
    acc0 += float(as_type<half>(bw0[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw0 + 1 + k0), xl, xh, xsum);
    acc1 += float(as_type<half>(bw1[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw1 + 1 + k0), xl, xh, xsum);
  }
  float t0 = simd_sum(acc0);
  float t1 = simd_sum(acc1);
  if (lane == 0) { out.data[r0] = t0; out.data[r1] = t1; }
}
COAX_DEV void coax_am_dot_q4_2_write_ex(coax_v_u16 w, int64_t b0, int64_t b1, coax_v_f32 x, int64_t x_off, int64_t blocks, coax_v_f32 parts, int64_t p_off, int64_t n_parts, float eps, float cap, coax_v_f32 out, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || b0 < 0 || b0 + blocks * 9 > w.count || b1 < 0 || b1 + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count || p_off < 0 || n_parts < 0 || p_off + n_parts > parts.count || r0 < 0 || r0 >= out.count || r1 < 0 || r1 >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  float inv_rms = coax_inv_rms_parts(parts, p_off, n_parts, blocks * 32, eps, lane);
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u);
  int xq = (int)(qtr * 4u);
  const device uint16_t *wp0 = w.data + b0;
  const device uint16_t *wp1 = w.data + b1;
  const device float *xp = x.data + x_off;
  float acc0 = 0.0f, acc1 = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    float4 xlf = *(const device float4 *)bx;
    float4 xhf = *(const device float4 *)(bx + 16);
    float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
    half4 xl = half4(xlf), xh = half4(xhf);
    const device uint16_t *bw0 = wp0 + b * 9;
    const device uint16_t *bw1 = wp1 + b * 9;
    acc0 += float(as_type<half>(bw0[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw0 + 1 + k0), xl, xh, xsum);
    acc1 += float(as_type<half>(bw1[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw1 + 1 + k0), xl, xh, xsum);
  }
  float t0 = simd_sum(acc0) * inv_rms;
  float t1 = simd_sum(acc1) * inv_rms;
  if (lane == 0) {
    if (cap > 0.0f) { t0 = cap * tanh(t0 / cap); t1 = cap * tanh(t1 / cap); }
    out.data[r0] = t0;
    if (r1 != r0) out.data[r1] = t1;
  }
}
COAX_DEV void coax_am_dot_q4s_2_write_ex(coax_v_u32 qs, coax_v_u16 ds, int64_t b0, int64_t b1, coax_v_f32 x, int64_t x_off, int64_t blocks, coax_v_f32 parts, int64_t p_off, int64_t n_parts, float eps, float cap, coax_v_f32 out, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || b0 < 0 || (b0 + blocks) * 4 > qs.count || b0 + blocks > ds.count || b1 < 0 || (b1 + blocks) * 4 > qs.count || b1 + blocks > ds.count || x_off < 0 || x_off + blocks * 32 > x.count || p_off < 0 || n_parts < 0 || p_off + n_parts > parts.count || r0 < 0 || r0 >= out.count || r1 < 0 || r1 >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  float inv_rms = coax_inv_rms_parts(parts, p_off, n_parts, blocks * 32, eps, lane);
  uint32_t blk_lane = lane >> 1;
  uint32_t hb = lane & 1u;
  int xq = (int)(hb * 8u);
  const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
  const device uint2 *qp1 = (const device uint2 *)(qs.data + b1 * 4) + hb;
  const device half *dp0 = (const device half *)(ds.data + b0);
  const device half *dp1 = (const device half *)(ds.data + b1);
  const device float *xp = x.data + x_off;
  float acc0 = 0.0f, acc1 = 0.0f;
  if (blocks == 72) {
    for (int b = (int)blk_lane; b < 72; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xsum = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      acc0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xsum);
      acc1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xsum);
    }
  } else {
    int iblocks = (int)blocks;
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xsum = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      acc0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xsum);
      acc1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xsum);
    }
  }
  float t0 = simd_sum(acc0) * inv_rms;
  float t1 = simd_sum(acc1) * inv_rms;
  if (lane == 0) {
    if (cap > 0.0f) { t0 = cap * tanh(t0 / cap); t1 = cap * tanh(t1 / cap); }
    out.data[r0] = t0;
    if (r1 != r0) out.data[r1] = t1;
  }
}
COAX_DEV void coax_am_dot_q4s_conv_2_write(coax_v_u32 qs, coax_v_u16 ds, int64_t b0, int64_t b1, coax_v_f32 x, int64_t blocks, int64_t add_res, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || b0 < 0 || (b0 + blocks) * 4 > qs.count || b0 + blocks > ds.count || b1 < 0 || (b1 + blocks) * 4 > qs.count || b1 + blocks > ds.count || blocks * 32 > x.count || r0 < 0 || r1 < 0 || r0 >= out.count || r1 >= out.count || (r0 + 1) * 4 > cw.count || (r1 + 1) * 4 > cw.count || (r0 + 1) * 3 > hist.count || (r1 + 1) * 3 > hist.count || (add_res != 0 && (r0 >= res.count || r1 >= res.count))) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 1;
  uint32_t hb = lane & 1u;
  int xq = (int)(hb * 8u);
  const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
  const device uint2 *qp1 = (const device uint2 *)(qs.data + b1 * 4) + hb;
  const device half *dp0 = (const device half *)(ds.data + b0);
  const device half *dp1 = (const device half *)(ds.data + b1);
  const device float *xp = x.data;
  float acc0 = 0.0f, acc1 = 0.0f;
  if (blocks == 96) {
    for (int b = (int)blk_lane; b < 96; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xsum = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      acc0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xsum);
      acc1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xsum);
    }
  } else {
    int iblocks = (int)blocks;
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xsum = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      acc0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xsum);
      acc1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xsum);
    }
  }
  float t0 = simd_sum(acc0);
  float t1 = simd_sum(acc1);
  if (lane == 0) {
    device float *h0 = hist.data + r0 * 3;
    float x0 = h0[0], x1 = h0[1], x2 = h0[2];
    h0[0] = x1; h0[1] = x2; h0[2] = t0;
    float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
    float a0 = (t0 + x0 * c0.x + x1 * c0.y + x2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
    out.data[r0] = add_res ? (res.data[r0] + a0) : a0;
    if (r1 != r0) {
      device float *h1 = hist.data + r1 * 3;
      float y0 = h1[0], y1 = h1[1], y2 = h1[2];
      h1[0] = y1; h1[1] = y2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      float a1 = (t1 + y0 * c1.x + y1 * c1.y + y2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
      out.data[r1] = add_res ? (res.data[r1] + a1) : a1;
    }
  }
}
COAX_DEV void coax_am_dot_q4s_1row_tg(coax_v_u32 qs, coax_v_u16 ds, int64_t blk_base, coax_v_f32 x, int64_t x_off, int64_t blocks, coax_v_f32 parts, int64_t p_off, int64_t n_parts, float eps, coax_v_f32 out, int64_t out_idx, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (blocks < 0 || blk_base < 0 || (blk_base + blocks) * 4 > qs.count || blk_base + blocks > ds.count || x_off < 0 || x_off + blocks * 32 > x.count || p_off < 0 || n_parts < 0 || p_off + n_parts > parts.count || out_idx < 0 || out_idx >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  uint32_t blk_lane = tid >> 1;
  uint32_t hb = tid & 1u;
  int xq = (int)(hb * 8u);
  const device uint2 *qp = (const device uint2 *)(qs.data + blk_base * 4);
  const device half *dp = (const device half *)(ds.data + blk_base);
  const device float *xp = x.data + x_off;
  int iblocks = (int)blocks;
  float acc = 0.0f;
  for (int b = (int)blk_lane; b < iblocks; b += 64) {
    const device float *bx = xp + b * 32 + xq;
    float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
    float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
    float4 fs = (f0 + f1) + (f2 + f3);
    float xsum = (fs.x + fs.y) + (fs.z + fs.w);
    half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
    acc += float(dp[b]) * coax_q4s_dot16_u2(qp[b * 2 + (int)hb], xl0, xl1, xh0, xh1, xsum);
  }
  float wa = simd_sum(acc);
  float inv_rms = (sg == 0u) ? coax_inv_rms_parts(parts, p_off, n_parts, blocks * 32, eps, lane) : 1.0f;
  if (lane == 0) tg[sg] = wa;
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0) {
    out.data[out_idx] = ((tg[0] + tg[1]) + (tg[2] + tg[3])) * inv_rms;
  }
}
COAX_DEV void coax_am_dot_q4s_qrgkv_sdyn(int64_t row0, uint32_t dims0, uint32_t dims1, uint32_t dims2, int64_t cols, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 xq_v, coax_v_f32 xk_v, coax_v_f32 xv_v, coax_v_f32 parts, coax_v_f32 oq, coax_v_f32 or_, coax_v_f32 og, coax_v_f32 ok, coax_v_f32 ov, coax_v_f32 osinks, coax_v_f32 odyn, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t n_q = (int64_t)(dims0 & 65535u), n_r = (int64_t)((dims0 >> 16u) & 65535u);
  int64_t n_kv = (int64_t)(dims1 & 65535u), n_sinks = (int64_t)((dims1 >> 16u) & 65535u);
  int64_t n_dyn = (int64_t)(dims2 & 65535u), n_parts = (int64_t)((dims2 >> 16u) & 65535u);
  int64_t blocks = cols >> 5;
  int64_t e1 = n_q + n_r, e2 = e1 + n_q, e3 = e2 + n_kv, e4 = e3 + n_kv, e5 = e4 + n_sinks, e6 = e5 + n_dyn, tot = e6 + n_dyn;
  if (cols <= 0 || (cols & 31) != 0 || row0 < 0 || (n_q & 1) != 0 || (n_r & 1) != 0 || (n_kv & 1) != 0 || (n_sinks & 1) != 0 || (n_dyn & 1) != 0 || (n_parts > 0 && 3 * n_parts > parts.count) || tot * blocks * 4 > qs.count || tot * blocks > ds.count || cols > xq_v.count || cols > xk_v.count || cols > xv_v.count || n_q > oq.count || n_r > or_.count || n_q > og.count || n_kv > ok.count || n_kv > ov.count || n_sinks > osinks.count || 2 * n_dyn > odyn.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= tot) return;
  const device float *xp;
  device float *op;
  int64_t orow, poff;
  if (r0 < n_q) { xp = xq_v.data; op = oq.data; orow = r0; poff = 0; }
  else if (r0 < e1) { xp = xq_v.data; op = or_.data; orow = r0 - n_q; poff = 0; }
  else if (r0 < e2) { xp = xq_v.data; op = og.data; orow = r0 - e1; poff = 0; }
  else if (r0 < e3) { xp = xk_v.data; op = ok.data; orow = r0 - e2; poff = n_parts; }
  else if (r0 < e4) { xp = xv_v.data; op = ov.data; orow = r0 - e3; poff = 2 * n_parts; }
  else if (r0 < e5) { xp = xq_v.data; op = osinks.data; orow = r0 - e4; poff = 0; }
  else if (r0 < e6) { xp = xk_v.data; op = odyn.data; orow = r0 - e5; poff = n_parts; }
  else { xp = xv_v.data; op = odyn.data; orow = r0 - e5; poff = 2 * n_parts; }
  uint32_t lane = tid & 31u;
  float inv_rms = coax_inv_rms_parts(parts, poff, n_parts, cols, eps, lane);
  uint32_t blk_lane = lane >> 1;
  uint32_t hb = lane & 1u;
  int xq = (int)(hb * 8u);
  float acc0 = 0.0f, acc1 = 0.0f;
  if (blocks == 72) {
    const device uint2 *qp0 = (const device uint2 *)(qs.data + r0 * 288) + hb;
    const device uint2 *qp1 = qp0 + 144;
    const device half *dp0 = (const device half *)(ds.data + r0 * 72);
    const device half *dp1 = dp0 + 72;
    for (int b = (int)blk_lane; b < 72; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xsum = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      acc0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xsum);
      acc1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xsum);
    }
  } else {
    const device uint2 *qp0 = (const device uint2 *)(qs.data + r0 * blocks * 4) + hb;
    const device uint2 *qp1 = qp0 + blocks * 2;
    const device half *dp0 = (const device half *)(ds.data + r0 * blocks);
    const device half *dp1 = dp0 + blocks;
    int iblocks = (int)blocks;
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xsum = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      acc0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xsum);
      acc1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xsum);
    }
  }
  float t0 = simd_sum(acc0) * inv_rms;
  float t1 = simd_sum(acc1) * inv_rms;
  if (lane == 0) { op[orow] = t0; op[orow + 1] = t1; }
}
COAX_DEV void coax_am_dot_iq4_situ_2_write(coax_v_u16 w, int64_t gb0, int64_t ub0, int64_t gb1, int64_t ub1, coax_v_f32 x, int64_t x_off, int64_t blocks, float beta, float lb, coax_v_f32 out, int64_t o0, int64_t o1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || gb0 < 0 || gb0 + blocks * 9 > w.count || ub0 < 0 || ub0 + blocks * 9 > w.count || gb1 < 0 || gb1 + blocks * 9 > w.count || ub1 < 0 || ub1 + blocks * 9 > w.count || x_off < 0 || x_off + blocks * 32 > x.count || o0 < 0 || o0 >= out.count || o1 < 0 || o1 >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t is_up = lane >> 4;
  uint32_t sub = lane & 15u;
  uint32_t blk_lane = sub >> 1;
  uint32_t half_b = sub & 1u;
  int k0 = (int)(half_b * 4u);
  int xq = (int)(half_b * 8u);
  const device uint16_t *wp0 = w.data + (is_up ? ub0 : gb0);
  const device uint16_t *wp1 = w.data + (is_up ? ub1 : gb1);
  const device float *xp = x.data + x_off;
  float acc0 = 0.0f, acc1 = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    half4 xl0 = half4(*(const device float4 *)bx);
    half4 xl1 = half4(*(const device float4 *)(bx + 4));
    half4 xh0 = half4(*(const device float4 *)(bx + 16));
    half4 xh1 = half4(*(const device float4 *)(bx + 20));
    const device uint16_t *bw0 = wp0 + b * 9;
    const device uint16_t *bw1 = wp1 + b * 9;
    float sc0 = 16.0f * float(as_type<half>(bw0[0]));
    float sc1 = 16.0f * float(as_type<half>(bw1[0]));
    acc0 += sc0 * coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw0 + 1 + k0), xl0, xl1, xh0, xh1);
    acc1 += sc1 * coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw1 + 1 + k0), xl0, xl1, xh0, xh1);
  }
  acc0 += simd_shuffle_xor(acc0, 8); acc0 += simd_shuffle_xor(acc0, 4); acc0 += simd_shuffle_xor(acc0, 2); acc0 += simd_shuffle_xor(acc0, 1);
  acc1 += simd_shuffle_xor(acc1, 8); acc1 += simd_shuffle_xor(acc1, 4); acc1 += simd_shuffle_xor(acc1, 2); acc1 += simd_shuffle_xor(acc1, 1);
  float vu0 = simd_shuffle(acc0, 16);
  float vu1 = simd_shuffle(acc1, 16);
  if (lane == 0) {
    float a0 = beta * tanh(acc0 / beta) / (1.0f + exp(-acc0));
    float a1 = beta * tanh(acc1 / beta) / (1.0f + exp(-acc1));
    if (lb > 0.0f) { vu0 = lb * tanh(vu0 / lb); vu1 = lb * tanh(vu1 / lb); }
    out.data[o0] = a0 * vu0;
    out.data[o1] = a1 * vu1;
  }
}
COAX_DEV void coax_am_dot_iq4_moe_2_write(coax_v_u16 w, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 x, int64_t exp_words, int64_t r0_off, int64_t r1_off, int64_t cols, int64_t n_used, int64_t add, coax_v_f32 out, int64_t o0, int64_t o1, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (n_used < 0 || cols < 0 || (cols & 31) != 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || r0_off < 0 || r1_off < 0 || exp_words < 0 || o0 < 0 || o0 >= out.count || o1 < 0 || o1 >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  for (int slot = 0; slot < (int)n_used; slot++) {
    int64_t eid = (int64_t)ids.data[slot];
    if (eid * exp_words + r0_off + blocks * 9 > w.count || eid * exp_words + r1_off + blocks * 9 > w.count) {
      coax_trap_(line, coax_err);
      return;
    }
  }
  uint32_t lane = tid & 31u;
  int iblocks = (int)blocks;
  int total_b = (int)n_used * iblocks;
  float acc0 = 0.0f, acc1 = 0.0f;
  if (iblocks == 36) {
    for (int tb = (int)lane; tb < total_b; tb += 32) {
      int slot = tb / 36;
      int b = tb - slot * 36;
      int64_t eid = (int64_t)ids.data[slot];
      float ew = 16.0f * wts.data[slot];
      const device float *bx = x.data + slot * 1152 + b * 32;
      const device uint16_t *bw0 = w.data + eid * exp_words + r0_off + b * 9;
      const device uint16_t *bw1 = w.data + eid * exp_words + r1_off + b * 9;
      float sc0 = ew * float(as_type<half>(bw0[0]));
      float sc1 = ew * float(as_type<half>(bw1[0]));
      float s0 = 0.0f, s1 = 0.0f;
      for (int q = 0; q < 2; q++) {
        half4 xl0 = half4(*(const device float4 *)(bx + q * 8));
        half4 xl1 = half4(*(const device float4 *)(bx + q * 8 + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + q * 8 + 16));
        half4 xh1 = half4(*(const device float4 *)(bx + q * 8 + 20));
        s0 += coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw0 + 1 + q * 4), xl0, xl1, xh0, xh1);
        s1 += coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw1 + 1 + q * 4), xl0, xl1, xh0, xh1);
      }
      acc0 += sc0 * s0;
      acc1 += sc1 * s1;
    }
  } else {
    for (int tb = (int)lane; tb < total_b; tb += 32) {
      int slot = tb / iblocks;
      int b = tb - slot * iblocks;
      int64_t eid = (int64_t)ids.data[slot];
      if ((eid + 1) * exp_words > w.count) { coax_trap_(line, coax_err); return; }
      float ew = 16.0f * wts.data[slot];
      const device float *bx = x.data + slot * (int)cols + b * 32;
      const device uint16_t *bw0 = w.data + eid * exp_words + r0_off + b * 9;
      const device uint16_t *bw1 = w.data + eid * exp_words + r1_off + b * 9;
      float sc0 = ew * float(as_type<half>(bw0[0]));
      float sc1 = ew * float(as_type<half>(bw1[0]));
      float s0 = 0.0f, s1 = 0.0f;
      for (int q = 0; q < 2; q++) {
        half4 xl0 = half4(*(const device float4 *)(bx + q * 8));
        half4 xl1 = half4(*(const device float4 *)(bx + q * 8 + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + q * 8 + 16));
        half4 xh1 = half4(*(const device float4 *)(bx + q * 8 + 20));
        s0 += coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw0 + 1 + q * 4), xl0, xl1, xh0, xh1);
        s1 += coax_iq4_dot16_f16s(*(const device packed_ushort4 *)(bw1 + 1 + q * 4), xl0, xl1, xh0, xh1);
      }
      acc0 += sc0 * s0;
      acc1 += sc1 * s1;
    }
  }
  float t0 = simd_sum(acc0);
  float t1 = simd_sum(acc1);
  if (lane == 0) {
    if (add != 0) { out.data[o0] += t0; out.data[o1] += t1; }
    else { out.data[o0] = t0; out.data[o1] = t1; }
  }
}
COAX_DEV void coax_am_dot_f16_tg_write(coax_v_u16 w, int64_t base, coax_v_f32 x, int64_t x_off, int64_t cols, int64_t add, float eps, coax_v_f32 out, int64_t out_idx, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (cols < 0 || base < 0 || base + cols > w.count || x_off < 0 || x_off + cols > x.count || out_idx < 0 || out_idx >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  const device uint16_t *wp = w.data + base;
  const device float *xp = x.data + x_off;
  int n4 = (int)(cols >> 2);
  float4 acc4 = float4(0.0f);
  float4 ss4 = float4(0.0f);
  const device half4 *w4 = (const device half4 *)wp;
  const device float4 *x4 = (const device float4 *)xp;
  if (eps > 0.0f) {
    for (int i = (int)tid; i < n4; i += 128) {
      float4 xv = x4[i];
      acc4 += float4(w4[i]) * xv;
      ss4 += xv * xv;
    }
  } else {
    for (int i = (int)tid; i < n4; i += 128) {
      acc4 += float4(w4[i]) * x4[i];
    }
  }
  float acc = (acc4.x + acc4.y) + (acc4.z + acc4.w);
  float ss = (ss4.x + ss4.y) + (ss4.z + ss4.w);
  int tail = n4 << 2;
  if (tail + (int)tid < (int)cols) {
    float xv = xp[tail + (int)tid];
    acc += float(as_type<half>(wp[tail + (int)tid])) * xv;
    if (eps > 0.0f) ss += xv * xv;
  }
  float wa = simd_sum(acc);
  float ws = (eps > 0.0f) ? simd_sum(ss) : 0.0f;
  if (lane == 0) {
    tg[sg] = wa;
    if (eps > 0.0f) tg[4 + sg] = ws;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0) {
    float tot = (tg[0] + tg[1]) + (tg[2] + tg[3]);
    if (eps > 0.0f && cols > 0) {
      float tss = (tg[4] + tg[5]) + (tg[6] + tg[7]);
      tot *= rsqrt(tss / (float)cols + eps);
    }
    if (add != 0) out.data[out_idx] += tot;
    else out.data[out_idx] = tot;
  }
}
COAX_DEV void coax_am_dot_f16_tg_write_rms(coax_v_u16 w, int64_t base, coax_v_f32 x, int64_t x_off, int64_t cols, coax_v_f32 parts, int64_t p_off, int64_t n_parts, float eps, coax_v_f32 out, int64_t out_idx, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (cols < 0 || base < 0 || base + cols > w.count || x_off < 0 || x_off + cols > x.count || p_off < 0 || n_parts < 0 || p_off + n_parts > parts.count || out_idx < 0 || out_idx >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  const device uint16_t *wp = w.data + base;
  const device float *xp = x.data + x_off;
  int n4 = (int)(cols >> 2);
  float4 acc4 = float4(0.0f);
  const device half4 *w4 = (const device half4 *)wp;
  const device float4 *x4 = (const device float4 *)xp;
  for (int i = (int)tid; i < n4; i += 128) {
    acc4 += float4(w4[i]) * x4[i];
  }
  float acc = (acc4.x + acc4.y) + (acc4.z + acc4.w);
  int tail = n4 << 2;
  if (tail + (int)tid < (int)cols) {
    acc += float(as_type<half>(wp[tail + (int)tid])) * xp[tail + (int)tid];
  }
  float wa = simd_sum(acc);
  float inv_rms = (sg == 0u) ? coax_inv_rms_parts(parts, p_off, n_parts, cols, eps, lane) : 1.0f;
  if (lane == 0) tg[sg] = wa;
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0) {
    out.data[out_idx] = ((tg[0] + tg[1]) + (tg[2] + tg[3])) * inv_rms;
  }
}
COAX_DEV void coax_am_rmsnorm_f32(int64_t dim, int64_t has_w, float eps, coax_v_f32 x, int64_t x_off, coax_v_f32 w, int64_t w_off, coax_v_f32 out, int64_t o_off, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (dim <= 0 || x_off < 0 || x_off + dim > x.count || o_off < 0 || o_off + dim > out.count || (has_w != 0 && (w_off < 0 || w_off + dim > w.count))) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  if ((dim & 3) == 0 && dim <= 3072) {
    int n4 = (int)(dim >> 2);
    int i0 = (int)tid, i1 = i0 + 256, i2 = i0 + 512;
    const device float4 *x4 = (const device float4 *)(x.data + x_off);
    float4 v0 = (i0 < n4) ? x4[i0] : float4(0.0f);
    float4 v1 = (i1 < n4) ? x4[i1] : float4(0.0f);
    float4 v2 = (i2 < n4) ? x4[i2] : float4(0.0f);
    float ss = dot(v0, v0) + dot(v1, v1) + dot(v2, v2);
    float ts = simd_sum(ss);
    if (lane == 0) tg[sg] = ts;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float tot = (tg[0] + tg[1]) + (tg[2] + tg[3]) + (tg[4] + tg[5]) + (tg[6] + tg[7]);
    float inv = rsqrt(tot / (float)dim + eps);
    device float4 *o4 = (device float4 *)(out.data + o_off);
    if (has_w != 0) {
      const device float4 *w4 = (const device float4 *)(w.data + w_off);
      if (i0 < n4) o4[i0] = v0 * inv * w4[i0];
      if (i1 < n4) o4[i1] = v1 * inv * w4[i1];
      if (i2 < n4) o4[i2] = v2 * inv * w4[i2];
    } else {
      if (i0 < n4) o4[i0] = v0 * inv;
      if (i1 < n4) o4[i1] = v1 * inv;
      if (i2 < n4) o4[i2] = v2 * inv;
    }
    return;
  }
  const device float *xp = x.data + x_off;
  device float *op = out.data + o_off;
  float ss = 0.0f;
  for (int i = (int)tid; i < (int)dim; i += 256) { float v = xp[i]; ss += v * v; }
  float ts = simd_sum(ss);
  if (lane == 0) tg[sg] = ts;
  threadgroup_barrier(mem_flags::mem_threadgroup);
  float tot = (tg[0] + tg[1]) + (tg[2] + tg[3]) + (tg[4] + tg[5]) + (tg[6] + tg[7]);
  float inv = rsqrt(tot / (float)dim + eps);
  if (has_w != 0) {
    const device float *wp = w.data + w_off;
    for (int i = (int)tid; i < (int)dim; i += 256) op[i] = xp[i] * inv * wp[i];
  } else {
    for (int i = (int)tid; i < (int)dim; i += 256) op[i] = xp[i] * inv;
  }
}
COAX_DEV void coax_am_ctrl_up_apply_f16(int64_t dim, int64_t rank, int64_t row0, coax_v_u16 w_gain, int64_t wg_off, coax_v_u16 w_forget, int64_t wf_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, coax_v_f32 out, int64_t o_off, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (dim <= 0 || rank < 0 || rank > 64 || row0 < 0 || wg_off < 0 || wg_off + rank * dim > w_gain.count || wf_off < 0 || wf_off + rank * dim > w_forget.count || g_off < 0 || g_off + rank > gate.count || c_off < 0 || c_off + dim > cur.count || m_off < 0 || m_off + dim > msg.count || o_off < 0 || o_off + dim > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  if ((int)tid < (int)rank) tg[tid] = gate.data[g_off + tid];
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int64_t row = row0 + (int64_t)tid;
  if (row < dim) {
    const device half *wgp = (const device half *)(w_gain.data + wg_off) + row;
    const device half *wfp = (const device half *)(w_forget.data + wf_off) + row;
    int idim = (int)dim, irank = (int)rank;
    float sg = 0.0f, sf = 0.0f;
    int k = 0;
    for (; k + 3 < irank; k += 4) {
      float g0 = tg[k], g1 = tg[k + 1], g2 = tg[k + 2], g3 = tg[k + 3];
      sg += float(wgp[0]) * g0 + float(wgp[idim]) * g1 + float(wgp[2 * idim]) * g2 + float(wgp[3 * idim]) * g3;
      sf += float(wfp[0]) * g0 + float(wfp[idim]) * g1 + float(wfp[2 * idim]) * g2 + float(wfp[3 * idim]) * g3;
      wgp += 4 * idim;
      wfp += 4 * idim;
    }
    for (; k < irank; k++) {
      float gv = tg[k];
      sg += float(wgp[0]) * gv;
      sf += float(wfp[0]) * gv;
      wgp += idim;
      wfp += idim;
    }
    float c = cur.data[c_off + row];
    float m = msg.data[m_off + row];
    out.data[o_off + row] = c - tanh(sf) * c + tanh(sg) * m;
  }
}
COAX_DEV void coax_am_mix_fused(int64_t mode, int64_t dim, int64_t n_blocks, float eps, coax_v_f32 blocks_buf, coax_v_f32 cur, coax_v_f32 query, int64_t q_off, coax_v_f32 aux, int64_t aux_off, coax_v_f32 out0, int64_t o0_off, coax_v_f32 out1, int64_t o1_off, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int n_states = (int)n_blocks + 1;
  if (dim <= 0 || n_blocks < 0 || n_states > 32 || n_blocks * dim > blocks_buf.count || dim > cur.count || q_off < 0 || q_off + dim > query.count || o0_off < 0 || o0_off + dim > out0.count || (mode >= 1 && (aux_off < 0 || aux_off + dim > aux.count)) || (mode == 2 && (o1_off < 0 || o1_off + 2 * dim > out1.count))) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup float *tg_ss = tg;
  threadgroup float *tg_dot = tg + 256;
  threadgroup float *tg_w = tg + 512;
  threadgroup float *tg_r1 = tg + 544;
  threadgroup float *tg_r2 = tg + 552;
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  if ((dim & 3) == 0 && dim <= 3072) {
    int n4 = (int)(dim >> 2);
    int i0 = (int)tid, i1 = i0 + 256, i2 = i0 + 512;
    float4 a0 = float4(0.0f), a1 = float4(0.0f), a2 = float4(0.0f);
    if (n_blocks == 0) {
      const device float4 *c4 = (const device float4 *)cur.data;
      float4 c0 = (i0 < n4) ? c4[i0] : float4(0.0f);
      float4 c1 = (i1 < n4) ? c4[i1] : float4(0.0f);
      float4 c2 = (i2 < n4) ? c4[i2] : float4(0.0f);
      float ts = simd_sum(dot(c0, c0) + dot(c1, c1) + dot(c2, c2));
      if (lane == 0) tg_ss[sg] = ts;
      threadgroup_barrier(mem_flags::mem_threadgroup);
      float tss = (tg_ss[0] + tg_ss[1]) + (tg_ss[2] + tg_ss[3]) + (tg_ss[4] + tg_ss[5]) + (tg_ss[6] + tg_ss[7]);
      float iv = rsqrt(tss / (float)dim + 1.0e-6f);
      a0 = c0 * iv; a1 = c1 * iv; a2 = c2 * iv;
    } else {
      const device float4 *q4 = (const device float4 *)(query.data + q_off);
      float4 qv0 = (i0 < n4) ? q4[i0] : float4(0.0f);
      float4 qv1 = (i1 < n4) ? q4[i1] : float4(0.0f);
      float4 qv2 = (i2 < n4) ? q4[i2] : float4(0.0f);
      for (int s = 0; s < n_states; s++) {
        const device float4 *v4 = (s < (int)n_blocks) ? (const device float4 *)(blocks_buf.data + s * dim) : (const device float4 *)cur.data;
        float ss = 0.0f, dd = 0.0f;
        if (i0 < n4) { float4 v = v4[i0]; ss += dot(v, v); dd += dot(v, qv0); }
        if (i1 < n4) { float4 v = v4[i1]; ss += dot(v, v); dd += dot(v, qv1); }
        if (i2 < n4) { float4 v = v4[i2]; ss += dot(v, v); dd += dot(v, qv2); }
        float ts = simd_sum(ss), td = simd_sum(dd);
        if (lane == 0) { tg_ss[s * 8 + sg] = ts; tg_dot[s * 8 + sg] = td; }
      }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      if (tid < 32u) {
        float inv_d = 1.0f / (float)dim;
        float inv_sqrt_d = rsqrt((float)dim);
        float sc = -3.0e38f, iv = 0.0f;
        if ((int)tid < n_states) {
          int b = (int)tid * 8;
          float tss = (tg_ss[b] + tg_ss[b+1]) + (tg_ss[b+2] + tg_ss[b+3]) + (tg_ss[b+4] + tg_ss[b+5]) + (tg_ss[b+6] + tg_ss[b+7]);
          float tdot = (tg_dot[b] + tg_dot[b+1]) + (tg_dot[b+2] + tg_dot[b+3]) + (tg_dot[b+4] + tg_dot[b+5]) + (tg_dot[b+6] + tg_dot[b+7]);
          iv = rsqrt(tss * inv_d + 1.0e-6f);
          sc = tdot * iv * inv_sqrt_d;
        }
        float mx = simd_max(sc);
        float w = ((int)tid < n_states) ? exp(sc - mx) : 0.0f;
        float inv_e = 1.0f / simd_sum(w);
        if ((int)tid < n_states) tg_w[tid] = w * inv_e * iv;
      }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      for (int s = 0; s < n_states; s++) {
        float ws = tg_w[s];
        const device float4 *v4 = (s < (int)n_blocks) ? (const device float4 *)(blocks_buf.data + s * dim) : (const device float4 *)cur.data;
        if (i0 < n4) a0 += ws * v4[i0];
        if (i1 < n4) a1 += ws * v4[i1];
        if (i2 < n4) a2 += ws * v4[i2];
      }
    }
    device float4 *o0_4 = (device float4 *)(out0.data + o0_off);
    if (mode == 0) {
      if (i0 < n4) o0_4[i0] = a0;
      if (i1 < n4) o0_4[i1] = a1;
      if (i2 < n4) o0_4[i2] = a2;
    } else if (mode == 1) {
      float s1 = 0.0f;
      if (i0 < n4) s1 += dot(a0, a0);
      if (i1 < n4) s1 += dot(a1, a1);
      if (i2 < n4) s1 += dot(a2, a2);
      float ts1 = simd_sum(s1);
      if (lane == 0) tg_r1[sg] = ts1;
      threadgroup_barrier(mem_flags::mem_threadgroup);
      float tot = (tg_r1[0] + tg_r1[1]) + (tg_r1[2] + tg_r1[3]) + (tg_r1[4] + tg_r1[5]) + (tg_r1[6] + tg_r1[7]);
      float inv = rsqrt(tot / (float)dim + eps);
      const device float4 *nw4 = (const device float4 *)(aux.data + aux_off);
      if (i0 < n4) o0_4[i0] = a0 * inv * nw4[i0];
      if (i1 < n4) o0_4[i1] = a1 * inv * nw4[i1];
      if (i2 < n4) o0_4[i2] = a2 * inv * nw4[i2];
    } else {
      if (i0 < n4) o0_4[i0] = a0;
      if (i1 < n4) o0_4[i1] = a1;
      if (i2 < n4) o0_4[i2] = a2;
      const device float4 *m4 = (const device float4 *)(aux.data + aux_off);
      float4 m0 = (i0 < n4) ? m4[i0] : float4(0.0f);
      float4 m1 = (i1 < n4) ? m4[i1] : float4(0.0f);
      float4 m2 = (i2 < n4) ? m4[i2] : float4(0.0f);
      float s1 = 0.0f, s2 = 0.0f;
      if (i0 < n4) { s1 += dot(a0, a0); s2 += dot(m0, m0); }
      if (i1 < n4) { s1 += dot(a1, a1); s2 += dot(m1, m1); }
      if (i2 < n4) { s1 += dot(a2, a2); s2 += dot(m2, m2); }
      float ts1 = simd_sum(s1), ts2 = simd_sum(s2);
      if (lane == 0) { tg_r1[sg] = ts1; tg_r2[sg] = ts2; }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      float tot1 = (tg_r1[0] + tg_r1[1]) + (tg_r1[2] + tg_r1[3]) + (tg_r1[4] + tg_r1[5]) + (tg_r1[6] + tg_r1[7]);
      float tot2 = (tg_r2[0] + tg_r2[1]) + (tg_r2[2] + tg_r2[3]) + (tg_r2[4] + tg_r2[5]) + (tg_r2[6] + tg_r2[7]);
      float inv1 = rsqrt(tot1 / (float)dim + 1.0e-6f);
      float inv2 = rsqrt(tot2 / (float)dim + 1.0e-6f);
      device float4 *o1_a = (device float4 *)(out1.data + o1_off);
      device float4 *o1_b = (device float4 *)(out1.data + o1_off + dim);
      if (i0 < n4) { o1_a[i0] = a0 * inv1; o1_b[i0] = m0 * inv2; }
      if (i1 < n4) { o1_a[i1] = a1 * inv1; o1_b[i1] = m1 * inv2; }
      if (i2 < n4) { o1_a[i2] = a2 * inv1; o1_b[i2] = m2 * inv2; }
    }
    return;
  }
  const device float *qp = query.data + q_off;
  for (int s = 0; s < n_states; s++) {
    const device float *vp = (s < (int)n_blocks) ? (blocks_buf.data + s * dim) : cur.data;
    float ss = 0.0f, dd = 0.0f;
    for (int i = (int)tid; i < (int)dim; i += 256) { float v = vp[i]; ss += v * v; dd += v * qp[i]; }
    float ts = simd_sum(ss), td = simd_sum(dd);
    if (lane == 0) { tg_ss[s * 8 + sg] = ts; tg_dot[s * 8 + sg] = td; }
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid < 32u) {
    float inv_d = 1.0f / (float)dim, inv_sqrt_d = rsqrt((float)dim);
    float sc = -3.0e38f, iv = 0.0f;
    if ((int)tid < n_states) {
      int b = (int)tid * 8;
      float tss = (tg_ss[b] + tg_ss[b+1]) + (tg_ss[b+2] + tg_ss[b+3]) + (tg_ss[b+4] + tg_ss[b+5]) + (tg_ss[b+6] + tg_ss[b+7]);
      float tdot = (tg_dot[b] + tg_dot[b+1]) + (tg_dot[b+2] + tg_dot[b+3]) + (tg_dot[b+4] + tg_dot[b+5]) + (tg_dot[b+6] + tg_dot[b+7]);
      iv = rsqrt(tss * inv_d + 1.0e-6f);
      sc = tdot * iv * inv_sqrt_d;
    }
    float mx = simd_max(sc);
    float w = ((int)tid < n_states) ? exp(sc - mx) : 0.0f;
    float inv_e = 1.0f / simd_sum(w);
    if ((int)tid < n_states) tg_w[tid] = w * inv_e * iv;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  device float *o0p = out0.data + o0_off;
  float s1 = 0.0f, s2 = 0.0f;
  for (int i = (int)tid; i < (int)dim; i += 256) {
    float acc = 0.0f;
    for (int s = 0; s < n_states; s++) {
      const device float *vp = (s < (int)n_blocks) ? (blocks_buf.data + s * dim) : cur.data;
      acc += tg_w[s] * vp[i];
    }
    o0p[i] = acc;
    s1 += acc * acc;
    if (mode == 2) { float mv = aux.data[aux_off + i]; s2 += mv * mv; }
  }
  if (mode == 0) return;
  float ts1 = simd_sum(s1), ts2 = simd_sum(s2);
  if (lane == 0) { tg_r1[sg] = ts1; tg_r2[sg] = ts2; }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  float tot1 = (tg_r1[0] + tg_r1[1]) + (tg_r1[2] + tg_r1[3]) + (tg_r1[4] + tg_r1[5]) + (tg_r1[6] + tg_r1[7]);
  if (mode == 1) {
    float inv = rsqrt(tot1 / (float)dim + eps);
    const device float *nwp = aux.data + aux_off;
    for (int i = (int)tid; i < (int)dim; i += 256) o0p[i] = o0p[i] * inv * nwp[i];
  } else {
    float tot2 = (tg_r2[0] + tg_r2[1]) + (tg_r2[2] + tg_r2[3]) + (tg_r2[4] + tg_r2[5]) + (tg_r2[6] + tg_r2[7]);
    float inv1 = rsqrt(tot1 / (float)dim + 1.0e-6f);
    float inv2 = rsqrt(tot2 / (float)dim + 1.0e-6f);
    const device float *mp = aux.data + aux_off;
    device float *o1p = out1.data + o1_off;
    for (int i = (int)tid; i < (int)dim; i += 256) { o1p[i] = o0p[i] * inv1; o1p[dim + i] = mp[i] * inv2; }
  }
}
COAX_DEV void coax_am_router_top8(int64_t n_exp, int64_t n_used, int64_t has_bias, float scale, coax_v_f32 logits, coax_v_f32 bias, coax_v_u32 out_ids, coax_v_f32 out_wts, uint32_t lane, int32_t line, COAX_ERR) {
  if (n_exp < 1 || n_exp > 128 || n_used < 1 || n_used > 32 || n_exp > logits.count || (has_bias != 0 && n_exp > bias.count) || n_used > out_ids.count || n_used > out_wts.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int e0 = (int)lane, e1 = e0 + 32, e2 = e0 + 64, e3 = e0 + 96;
  float p0 = 0.0f, s0 = -3.0e38f, p1 = 0.0f, s1 = -3.0e38f, p2 = 0.0f, s2 = -3.0e38f, p3 = 0.0f, s3 = -3.0e38f;
  if (e0 < (int)n_exp) { p0 = 1.0f / (1.0f + exp(-logits.data[e0])); s0 = has_bias ? (p0 + bias.data[e0]) : p0; }
  if (e1 < (int)n_exp) { p1 = 1.0f / (1.0f + exp(-logits.data[e1])); s1 = has_bias ? (p1 + bias.data[e1]) : p1; }
  if (e2 < (int)n_exp) { p2 = 1.0f / (1.0f + exp(-logits.data[e2])); s2 = has_bias ? (p2 + bias.data[e2]) : p2; }
  if (e3 < (int)n_exp) { p3 = 1.0f / (1.0f + exp(-logits.data[e3])); s3 = has_bias ? (p3 + bias.data[e3]) : p3; }
  float wsum = 0.0f, my_w = 0.0f;
  uint32_t my_id = 0u;
  for (int k = 0; k < (int)n_used; k++) {
    float bs = s0, bp = p0; int be = e0;
    if (s1 > bs) { bs = s1; be = e1; bp = p1; }
    if (s2 > bs) { bs = s2; be = e2; bp = p2; }
    if (s3 > bs) { bs = s3; be = e3; bp = p3; }
    float gmax = simd_max(bs);
    float neg_e = (bs == gmax) ? (float)(-be) : -999999.0f;
    int win_e = (int)(-simd_max(neg_e));
    float win_p = simd_sum((be == win_e) ? bp : 0.0f);
    wsum += win_p;
    if ((int)lane == k) { my_id = (uint32_t)win_e; my_w = win_p; }
    if (be == win_e) {
      if (win_e == e0) s0 = -3.0e38f;
      else if (win_e == e1) s1 = -3.0e38f;
      else if (win_e == e2) s2 = -3.0e38f;
      else if (win_e == e3) s3 = -3.0e38f;
    }
  }
  if (wsum < 0.00006103515625f) wsum = 0.00006103515625f;
  if ((int)lane < (int)n_used) {
    out_ids.data[lane] = my_id;
    out_wts.data[lane] = my_w * (scale / wsum);
  }
}
COAX_DEV void coax_am_conv_kv_both(int64_t item, int64_t n_head_kv, int64_t n_kv_elem, int64_t pos, int64_t slot_pos, int64_t has_dyn, float threshold, float eps, coax_v_f32 k_raw, coax_v_f32 v_raw, coax_v_f32 kw, coax_v_f32 vw, coax_v_f32 dyn, coax_v_f32 k_hist, coax_v_f32 v_hist, coax_v_u16 k_cache, coax_v_u16 v_cache, uint32_t lane, int32_t line, COAX_ERR) {
  int64_t is_v = item / n_head_kv;
  int64_t h = item % n_head_kv;
  int64_t base = h * 128;
  int64_t dbase = slot_pos * n_kv_elem + base;
  int64_t dyn_base = (is_v * n_head_kv + h) * 4;
  if (n_head_kv <= 0 || pos < 0 || base + 128 > k_raw.count || base + 128 > v_raw.count || (base + 128) * 4 > kw.count || (base + 128) * 4 > vw.count || (base + 128) * 4 > k_hist.count || (base + 128) * 4 > v_hist.count || dbase + 128 > k_cache.count || dbase + 128 > v_cache.count || (has_dyn != 0 && dyn_base + 4 > dyn.count)) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t s0 = (uint32_t)(pos + 1) & 3u, s1 = (uint32_t)(pos + 2) & 3u, s2 = (uint32_t)(pos + 3) & 3u, sw = (uint32_t)pos & 3u;
  float inv_s = has_dyn ? 0.3333333333333333f : 0.4472135954999579f;
  float4 dv = has_dyn ? *(const device float4 *)(dyn.data + dyn_base) : float4(0.0f);
  int64_t c0 = base + (int64_t)(lane * 4u);
  const device float *rawp = (is_v == 0) ? k_raw.data : v_raw.data;
  const device float *wp = (is_v == 0) ? kw.data : vw.data;
  device float *hp = (is_v == 0) ? k_hist.data : v_hist.data;
  float4 xr = *(const device float4 *)(rawp + c0);
  float vals[4];
  for (int j = 0; j < 4; j++) {
    int64_t c = c0 + j;
    device float *hc = hp + c * 4;
    float4 h4 = *(const device float4 *)hc;
    float x0 = h4[s0], x1 = h4[s1], x2 = h4[s2], x3 = xr[j];
    hc[sw] = x3;
    float4 cw = *(const device float4 *)(wp + c * 4);
    float a = x0 * cw.x + x1 * cw.y + x2 * cw.z + x3 * cw.w;
    if (has_dyn) a += dv.x * x3 + dv.y * x2 + dv.z * x1 + dv.w * x0;
    vals[j] = (x3 + a) * inv_s;
  }
  float4 vv = float4(vals[0], vals[1], vals[2], vals[3]);
  float tot = simd_sum(dot(vv, vv));
  if (is_v == 0) {
    float fac = threshold / (sqrt(tot) + 1.0e-6f);
    if (fac > 1.0f) fac = 1.0f;
    *(device half4 *)(k_cache.data + dbase + lane * 4u) = half4(vv * fac);
  } else {
    float inv = rsqrt(tot * (1.0f / 128.0f) + eps);
    *(device half4 *)(v_cache.data + dbase + lane * 4u) = half4(vv * inv);
  }
}
COAX_DEV void coax_am_attn_gqa_1pass(int64_t h, uint32_t dims0, uint32_t dims1, uint32_t kv_info, int64_t pos, float qk_clip, float rel_clip, float q_tau, coax_v_f32 q, coax_v_f32 q_scale, coax_v_f32 r, coax_v_f32 r_scale, coax_v_f32 rel_bias, coax_v_f32 sinks, coax_v_f32 gate, coax_v_u16 k_cache, coax_v_u16 v_cache, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int n_head_kv = (int)((dims0 >> 8u) & 255u);
  int head_dim = (int)((dims0 >> 16u) & 255u);
  int n_rel = (int)((dims0 >> 24u) & 255u);
  int extent = (int)(dims1 & 65535u);
  int is_swa = (int)((dims1 >> 16u) & 1u);
  int swa = (int)((dims1 >> 17u) & 32767u);
  int kv_cap = (int)(kv_info & 65535u);
  int has_qs = (int)((kv_info >> 16u) & 1u);
  int has_rs = (int)((kv_info >> 17u) & 1u);
  int64_t kv_h = h / 3;
  int64_t n_kv_elem = (int64_t)n_head_kv * 128;
  if (head_dim != 128 || h < 0 || (h + 1) * 128 > q.count || (h + 1) * 128 > gate.count || (h + 1) * 128 > out.count || h >= sinks.count || (h + 1) * n_rel > r.count || (has_qs && (h + 1) * 128 > q_scale.count) || (has_rs && (h + 1) * n_rel > r_scale.count) || (int64_t)kv_cap * n_kv_elem > k_cache.count || (int64_t)kv_cap * n_kv_elem > v_cache.count || (int64_t)extent * n_rel > rel_bias.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  int64_t start = (is_swa && pos >= swa) ? (pos - swa + 1) : 0;
  int n_keys = (int)(pos - start + 1);
  float scale = 1.0f / sqrt(128.0f);
  float sk = sinks.data[h];
  int64_t qb = h * 128 + (int64_t)(lane * 4u);
  float4 qv = *(const device float4 *)(q.data + qb);
  if (has_qs) qv *= *(const device float4 *)(q_scale.data + qb);
  float qfac = qk_clip / (sqrt(simd_sum(dot(qv, qv))) + 1.0e-6f);
  if (qfac > 1.0f) qfac = 1.0f;
  qv *= (qfac * q_tau);
  float rv = 0.0f;
  if ((int)lane < n_rel) {
    int64_t ridx = h * n_rel + (int64_t)lane;
    rv = r.data[ridx];
    if (has_rs) rv *= r_scale.data[ridx];
  }
  float rfac = rel_clip / (sqrt(simd_sum(rv * rv)) + 1.0e-6f);
  if (rfac > 1.0f) rfac = 1.0f;
  float rreg = rv * (rfac * q_tau);
  float m = -3.0e38f, l = 0.0f;
  float4 acc = float4(0.0f);
  int64_t kh_off = kv_h * 128 + (int64_t)(lane * 4u);
  for (int kidx = (int)sg; kidx < n_keys; kidx += 4) {
    int64_t kp = start + kidx;
    int64_t slot = is_swa ? (kp % kv_cap) : kp;
    int64_t kb = slot * n_kv_elem + kh_off;
    float4 kv = float4(*(const device half4 *)(k_cache.data + kb));
    float d = simd_sum(dot(qv, kv));
    int dist = (int)(pos - kp);
    float rbp = (dist < extent && (int)lane < n_rel) ? (rel_bias.data[dist * n_rel + (int)lane] * rreg) : 0.0f;
    float sc = d * scale + simd_sum(rbp) - sk;
    float nm = (sc > m) ? sc : m;
    float alpha = (m > -1.0e20f) ? exp(m - nm) : 0.0f;
    float wt = exp(sc - nm);
    m = nm;
    l = l * alpha + wt;
    float4 vv = float4(*(const device half4 *)(v_cache.data + kb));
    acc = acc * alpha + wt * vv;
  }
  threadgroup float *msh = tg;
  threadgroup float *lsh = tg + 4;
  threadgroup float *vsh = tg + 8;
  if (lane == 0) { msh[sg] = m; lsh[sg] = l; }
  *(threadgroup float4 *)(vsh + sg * 128 + lane * 4u) = acc;
  threadgroup_barrier(mem_flags::mem_threadgroup);
  float gmax = 0.0f;
  for (int s = 0; s < 4; s++) if (msh[s] > gmax) gmax = msh[s];
  float gsum = exp(-gmax);
  float vtot = 0.0f;
  for (int s = 0; s < 4; s++) {
    float lv = lsh[s];
    if (lv > 0.0f) {
      float sc = exp(msh[s] - gmax);
      gsum += lv * sc;
      vtot += vsh[s * 128 + tid] * sc;
    }
  }
  int64_t oidx = h * 128 + (int64_t)tid;
  out.data[oidx] = (vtot / gsum) / (1.0f + exp(-gate.data[oidx]));
}
COAX_DEV void coax_am_dot_q4_conv_2_write(coax_v_u16 w, int64_t b0, int64_t b1, coax_v_f32 x, int64_t blocks, int64_t add_res, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || b0 < 0 || b0 + blocks * 9 > w.count || b1 < 0 || b1 + blocks * 9 > w.count || blocks * 32 > x.count || r0 < 0 || r1 < 0 || r0 >= out.count || r1 >= out.count || (r0 + 1) * 4 > cw.count || (r1 + 1) * 4 > cw.count || (r0 + 1) * 3 > hist.count || (r1 + 1) * 3 > hist.count || (add_res != 0 && (r0 >= res.count || r1 >= res.count))) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 1;
  uint32_t half_b = lane & 1u;
  int k0 = (int)(half_b * 4u);
  int xq = (int)(half_b * 8u);
  const device uint16_t *wp0 = w.data + b0;
  const device uint16_t *wp1 = w.data + b1;
  const device float *xp = x.data;
  float acc0 = 0.0f, acc1 = 0.0f;
  for (int b = (int)blk_lane; b < (int)blocks; b += 16) {
    const device float *bx = xp + b * 32 + xq;
    float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
    float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
    float4 fs = (f0 + f1) + (f2 + f3);
    float xsum = (fs.x + fs.y) + (fs.z + fs.w);
    half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
    const device uint16_t *bw0 = wp0 + b * 9;
    const device uint16_t *bw1 = wp1 + b * 9;
    acc0 += float(as_type<half>(bw0[0])) * coax_q4_alu_dot16_f16(*(const device packed_ushort4 *)(bw0 + 1 + k0), xl0, xl1, xh0, xh1, xsum);
    acc1 += float(as_type<half>(bw1[0])) * coax_q4_alu_dot16_f16(*(const device packed_ushort4 *)(bw1 + 1 + k0), xl0, xl1, xh0, xh1, xsum);
  }
  float t0 = simd_sum(acc0);
  float t1 = simd_sum(acc1);
  if (lane == 0) {
    device float *h0 = hist.data + r0 * 3;
    float x0 = h0[0], x1 = h0[1], x2 = h0[2];
    h0[0] = x1; h0[1] = x2; h0[2] = t0;
    float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
    float a0 = (t0 + x0 * c0.x + x1 * c0.y + x2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
    out.data[r0] = add_res ? (res.data[r0] + a0) : a0;
    if (r1 != r0) {
      device float *h1 = hist.data + r1 * 3;
      float y0 = h1[0], y1 = h1[1], y2 = h1[2];
      h1[0] = y1; h1[1] = y2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      float a1 = (t1 + y0 * c1.x + y1 * c1.y + y2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
      out.data[r1] = add_res ? (res.data[r1] + a1) : a1;
    }
  }
}
COAX_DEV void coax_iq4_init_tlut(threadgroup half2 *tlut, uint32_t tid) {
  tlut[tid]        = half2(coax_iq4_lut_f16s[tid & 15u], coax_iq4_lut_f16s[tid >> 4u]);
  tlut[tid + 128u] = half2(coax_iq4_lut_f16s[tid & 15u], coax_iq4_lut_f16s[(tid + 128u) >> 4u]);
  threadgroup_barrier(mem_flags::mem_threadgroup);
}
COAX_DEV float coax_iq4_dot8_tg(packed_ushort2 p, half2 x0, half2 x1, half2 x2, half2 x3, threadgroup const half2 *tlut) {
  uint32_t a = p.x, c = p.y;
  half2 s = tlut[a & 255u] * x0 + tlut[a >> 8u] * x1 + tlut[c & 255u] * x2 + tlut[c >> 8u] * x3;
  return float(s.x + s.y);
}
COAX_DEV float coax_iq4_dot16_tg(packed_ushort4 p, half2 x0, half2 x1, half2 x2, half2 x3, half2 x4, half2 x5, half2 x6, half2 x7, threadgroup const half2 *tlut) {
  uint32_t a = p.x, b = p.y, c = p.z, d = p.w;
  half2 s = tlut[a & 255u] * x0 + tlut[a >> 8u] * x1 + tlut[b & 255u] * x2 + tlut[b >> 8u] * x3;
  s += tlut[c & 255u] * x4 + tlut[c >> 8u] * x5 + tlut[d & 255u] * x6 + tlut[d >> 8u] * x7;
  return float(s.x + s.y);
}
COAX_DEV float coax_iq4s_dot16_u2(uint2 p, half2 x0, half2 x1, half2 x2, half2 x3, half2 x4, half2 x5, half2 x6, half2 x7, threadgroup const half2 *tlut) {
  uchar4 b0 = as_type<uchar4>(p.x), b1 = as_type<uchar4>(p.y);
  half2 s = tlut[b0.x] * x0 + tlut[b0.y] * x1 + tlut[b0.z] * x2 + tlut[b0.w] * x3;
  s += tlut[b1.x] * x4 + tlut[b1.y] * x5 + tlut[b1.z] * x6 + tlut[b1.w] * x7;
  return float(s.x + s.y);
}
static __attribute__((noinline)) int coax_router_pick_slot_noinline(int n_exp, int n_used, int has_bias, float scale, const device float *logits, const device float *bias, int slot, int write_out, device uint32_t *out_ids, device float *out_wts, uint32_t lane) {
  int e0 = (int)lane, e1 = e0 + 32, e2 = e0 + 64, e3 = e0 + 96;
  float p0 = 0.0f, s0 = -3.0e38f, p1 = 0.0f, s1 = -3.0e38f, p2 = 0.0f, s2 = -3.0e38f, p3 = 0.0f, s3 = -3.0e38f;
  if (e0 < n_exp) { p0 = 1.0f / (1.0f + exp(-logits[e0])); s0 = has_bias ? (p0 + bias[e0]) : p0; }
  if (e1 < n_exp) { p1 = 1.0f / (1.0f + exp(-logits[e1])); s1 = has_bias ? (p1 + bias[e1]) : p1; }
  if (e2 < n_exp) { p2 = 1.0f / (1.0f + exp(-logits[e2])); s2 = has_bias ? (p2 + bias[e2]) : p2; }
  if (e3 < n_exp) { p3 = 1.0f / (1.0f + exp(-logits[e3])); s3 = has_bias ? (p3 + bias[e3]) : p3; }
  float wsum = 0.0f, my_w = 0.0f;
  uint32_t my_id = 0u;
  int slot_id = 0;
  int lim = write_out ? n_used : (slot + 1);
  for (int k = 0; k < lim; k++) {
    float bs = s0, bp = p0; int be = e0;
    if (s1 > bs) { bs = s1; be = e1; bp = p1; }
    if (s2 > bs) { bs = s2; be = e2; bp = p2; }
    if (s3 > bs) { bs = s3; be = e3; bp = p3; }
    float gmax = simd_max(bs);
    float neg_e = (bs == gmax) ? (float)(-be) : -999999.0f;
    int win_e = (int)(-simd_max(neg_e));
    if (k == slot) slot_id = win_e;
    if (write_out) {
      float win_p = simd_sum((be == win_e) ? bp : 0.0f);
      wsum += win_p;
      if ((int)lane == k) { my_id = (uint32_t)win_e; my_w = win_p; }
    }
    if (be == win_e) {
      if (win_e == e0) s0 = -3.0e38f;
      else if (win_e == e1) s1 = -3.0e38f;
      else if (win_e == e2) s2 = -3.0e38f;
      else if (win_e == e3) s3 = -3.0e38f;
    }
  }
  if (write_out) {
    if (wsum < 0.00006103515625f) wsum = 0.00006103515625f;
    if ((int)lane < n_used) { out_ids[lane] = my_id; out_wts[lane] = my_w * (scale / wsum); }
  }
  return slot_id;
}
static __attribute__((noinline)) void coax_am_moe_gu_iq4_slow(int64_t slot, int64_t r0, int64_t n_ff, int64_t blocks, int64_t rw, int64_t eb, float inv_rms, float beta, float lb, coax_v_u16 w, coax_v_f32 x, coax_v_f32 out, threadgroup const half2 *tlut, uint32_t lane) {
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  uint32_t blk_lane = lane >> 1;
  uint32_t hb = lane & 1u;
  int k0 = (int)(hb * 4u), xq = (int)(hb * 8u);
  const device uint16_t *wg0 = w.data + eb + r0 * rw;
  const device uint16_t *wu0 = wg0 + n_ff * rw;
  const device uint16_t *wg1 = w.data + eb + r1 * rw;
  const device uint16_t *wu1 = wg1 + n_ff * rw;
  const device float *xp = x.data;
  int iblocks = (int)blocks;
  float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
  for (int b = (int)blk_lane; b < iblocks; b += 16) {
    const device float *bx = xp + b * 32 + xq;
    half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
    half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
    half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
    half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
    const device uint16_t *bg0 = wg0 + b * 9, *bu0 = wu0 + b * 9;
    const device uint16_t *bg1 = wg1 + b * 9, *bu1 = wu1 + b * 9;
    ag0 += float(as_type<half>(bg0[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bg0 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    au0 += float(as_type<half>(bu0[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bu0 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    ag1 += float(as_type<half>(bg1[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bg1 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    au1 += float(as_type<half>(bu1[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bu1 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
  }
  float inv16 = inv_rms * 16.0f;
  float g0 = simd_sum(ag0) * inv16, u0 = simd_sum(au0) * inv16;
  float g1 = simd_sum(ag1) * inv16, u1 = simd_sum(au1) * inv16;
  if (lane == 0) {
    float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
    float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
    if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
    out.data[slot * n_ff + r0] = a0 * u0;
    if (r0 + 1 < n_ff) out.data[slot * n_ff + r0 + 1] = a1 * u1;
  }
}
COAX_DEV void coax_am_moe_gu_iq4_tg(int64_t slot, int64_t row0, int64_t n_ff, int64_t cols, int64_t n_exp, int64_t n_used, int64_t has_bias, float r_scale, float beta, float lb, int64_t n_parts, float eps, coax_v_u16 w, coax_v_f32 x, coax_v_f32 parts, coax_v_f32 logits, coax_v_f32 bias, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t rw = blocks * 9;
  if (n_exp < 1 || n_exp > 128 || n_used < 1 || n_used > 32 || slot < 0 || slot >= n_used || row0 < 0 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || n_parts < 0 || (n_parts > 0 && n_parts > parts.count) || cols > x.count || n_exp > logits.count || (has_bias != 0 && n_exp > bias.count) || n_used > ids.count || n_used > wts.count || (slot + 1) * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  threadgroup int *tg_eid = (threadgroup int *)(tg + 256);
  threadgroup float *tg_inv = tg + 257;
  tlut[tid]        = half2(coax_iq4_lut_f16s[tid & 15u], coax_iq4_lut_f16s[tid >> 4u]);
  tlut[tid + 128u] = half2(coax_iq4_lut_f16s[tid & 15u], coax_iq4_lut_f16s[(tid + 128u) >> 4u]);
  if (tid < 32u) {
    int e = coax_router_pick_slot_noinline((int)n_exp, (int)n_used, (int)has_bias, r_scale, logits.data, bias.data, (int)slot, (slot == 0 && row0 == 0) ? 1 : 0, ids.data, wts.data, tid);
    if (tid == 0u) tg_eid[0] = e;
  } else if (tid < 64u) {
    float inv = coax_inv_rms_parts(parts, 0, n_parts, cols, eps, tid - 32u);
    if (tid == 32u) tg_inv[0] = inv;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int64_t eid = (int64_t)tg_eid[0];
  float inv_rms = tg_inv[0];
  int64_t eb = eid * 2 * n_ff * rw;
  if (eb + 2 * n_ff * rw > w.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  uint32_t lane = tid & 31u;
  if (blocks == 72 && r0 + 2 <= n_ff) {
    int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u);
    int k0 = hb * 4, xq = hb * 8;
    const device uint16_t *wg0 = w.data + eb + r0 * 648;
    const device uint16_t *wu0 = wg0 + n_ff * 648;
    const device uint16_t *wg1 = wg0 + 648;
    const device uint16_t *wu1 = wu0 + 648;
    const device float *xp = x.data;
    float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
    for (int b = blk_lane; b < 72; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
      half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
      half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
      const device uint16_t *bg0 = wg0 + b * 9, *bu0 = wu0 + b * 9;
      const device uint16_t *bg1 = wg1 + b * 9, *bu1 = wu1 + b * 9;
      ag0 += float(as_type<half>(bg0[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bg0 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      au0 += float(as_type<half>(bu0[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bu0 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      ag1 += float(as_type<half>(bg1[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bg1 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      au1 += float(as_type<half>(bu1[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bu1 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    }
    float inv16 = inv_rms * 16.0f;
    float g0 = simd_sum(ag0) * inv16, u0 = simd_sum(au0) * inv16;
    float g1 = simd_sum(ag1) * inv16, u1 = simd_sum(au1) * inv16;
    if (lane == 0) {
      float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
      float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
      if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
      out.data[slot * n_ff + r0] = a0 * u0;
      out.data[slot * n_ff + r0 + 1] = a1 * u1;
    }
  } else {
    coax_am_moe_gu_iq4_slow(slot, r0, n_ff, blocks, rw, eb, inv_rms, beta, lb, w, x, out, tlut, lane);
  }
}
static __attribute__((noinline)) void coax_am_moe_gu_q4s_slow(int64_t slot, int64_t r0, int64_t n_ff, int64_t blocks, int64_t eb, float inv_rms, float beta, float lb, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 out, uint32_t lane) {
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8;
  int64_t bg0 = eb + r0 * blocks, bu0 = bg0 + n_ff * blocks;
  int64_t bg1 = eb + r1 * blocks, bu1 = bg1 + n_ff * blocks;
  const device uint2 *qg0 = (const device uint2 *)(qs.data + bg0 * 4) + hb;
  const device uint2 *qu0 = (const device uint2 *)(qs.data + bu0 * 4) + hb;
  const device uint2 *qg1 = (const device uint2 *)(qs.data + bg1 * 4) + hb;
  const device uint2 *qu1 = (const device uint2 *)(qs.data + bu1 * 4) + hb;
  const device half *dg0 = (const device half *)(ds.data + bg0);
  const device half *du0 = (const device half *)(ds.data + bu0);
  const device half *dg1 = (const device half *)(ds.data + bg1);
  const device half *du1 = (const device half *)(ds.data + bu1);
  const device float *xp = x.data;
  int iblocks = (int)blocks;
  float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
  for (int b = blk_lane; b < iblocks; b += 16) {
    const device float *bx = xp + b * 32 + xq;
    float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
    float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
    float4 fs = (f0 + f1) + (f2 + f3);
    float xs = (fs.x + fs.y) + (fs.z + fs.w);
    half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
    ag0 += float(dg0[b]) * coax_q4s_dot16_u2(qg0[b * 2], xl0, xl1, xh0, xh1, xs);
    au0 += float(du0[b]) * coax_q4s_dot16_u2(qu0[b * 2], xl0, xl1, xh0, xh1, xs);
    ag1 += float(dg1[b]) * coax_q4s_dot16_u2(qg1[b * 2], xl0, xl1, xh0, xh1, xs);
    au1 += float(du1[b]) * coax_q4s_dot16_u2(qu1[b * 2], xl0, xl1, xh0, xh1, xs);
  }
  float g0 = simd_sum(ag0) * inv_rms, u0 = simd_sum(au0) * inv_rms;
  float g1 = simd_sum(ag1) * inv_rms, u1 = simd_sum(au1) * inv_rms;
  if (lane == 0) {
    float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
    float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
    if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
    out.data[slot * n_ff + r0] = a0 * u0;
    if (r0 + 1 < n_ff) out.data[slot * n_ff + r0 + 1] = a1 * u1;
  }
}
COAX_DEV void coax_am_moe_gu_q4s_tg(int64_t slot, int64_t row0, int64_t n_ff, int64_t cols, int64_t n_exp, int64_t n_used, int64_t has_bias, float r_scale, float beta, float lb, int64_t n_parts, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 parts, coax_v_f32 logits, coax_v_f32 bias, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (n_exp < 1 || n_exp > 128 || n_used < 1 || n_used > 32 || slot < 0 || slot >= n_used || row0 < 0 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || n_parts < 0 || (n_parts > 0 && n_parts > parts.count) || cols > x.count || n_exp > logits.count || (has_bias != 0 && n_exp > bias.count) || n_used > ids.count || n_used > wts.count || (slot + 1) * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup int *tg_eid = (threadgroup int *)tg;
  threadgroup float *tg_inv = tg + 1;
  if (tid < 32u) {
    int e = coax_router_pick_slot_noinline((int)n_exp, (int)n_used, (int)has_bias, r_scale, logits.data, bias.data, (int)slot, (slot == 0 && row0 == 0) ? 1 : 0, ids.data, wts.data, tid);
    if (tid == 0u) tg_eid[0] = e;
  } else if (tid < 64u) {
    float inv = coax_inv_rms_parts(parts, 0, n_parts, cols, eps, tid - 32u);
    if (tid == 32u) tg_inv[0] = inv;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int64_t eid = (int64_t)tg_eid[0];
  float inv_rms = tg_inv[0];
  int64_t eb = eid * 2 * n_ff * blocks;
  if ((eb + 2 * n_ff * blocks) * 4 > qs.count || eb + 2 * n_ff * blocks > ds.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  uint32_t lane = tid & 31u;
  if (blocks == 72 && r0 + 2 <= n_ff) {
    int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8;
    int64_t bg0 = eb + r0 * 72, bu0 = bg0 + n_ff * 72;
    const device uint2 *qg0 = (const device uint2 *)(qs.data + bg0 * 4) + hb;
    const device uint2 *qu0 = (const device uint2 *)(qs.data + bu0 * 4) + hb;
    const device uint2 *qg1 = qg0 + 144, *qu1 = qu0 + 144;
    const device half *dg0 = (const device half *)(ds.data + bg0), *dg1 = dg0 + 72;
    const device half *du0 = (const device half *)(ds.data + bu0), *du1 = du0 + 72;
    const device float *xp = x.data;
    float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
    for (int b = blk_lane; b < 72; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xs = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      ag0 += float(dg0[b]) * coax_q4s_dot16_u2(qg0[b * 2], xl0, xl1, xh0, xh1, xs);
      au0 += float(du0[b]) * coax_q4s_dot16_u2(qu0[b * 2], xl0, xl1, xh0, xh1, xs);
      ag1 += float(dg1[b]) * coax_q4s_dot16_u2(qg1[b * 2], xl0, xl1, xh0, xh1, xs);
      au1 += float(du1[b]) * coax_q4s_dot16_u2(qu1[b * 2], xl0, xl1, xh0, xh1, xs);
    }
    float g0 = simd_sum(ag0) * inv_rms, u0 = simd_sum(au0) * inv_rms;
    float g1 = simd_sum(ag1) * inv_rms, u1 = simd_sum(au1) * inv_rms;
    if (lane == 0) {
      float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
      float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
      if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
      out.data[slot * n_ff + r0] = a0 * u0;
      out.data[slot * n_ff + r0 + 1] = a1 * u1;
    }
  } else {
    coax_am_moe_gu_q4s_slow(slot, r0, n_ff, blocks, eb, inv_rms, beta, lb, qs, ds, x, out, lane);
  }
}
static __attribute__((noinline)) void coax_am_moe_down_q4s_slow(int64_t r0, int64_t rows, int64_t cols, int64_t n_used, int64_t blocks, int64_t ew, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, uint32_t lane, int32_t line, COAX_ERR) {
  for (int s = 0; s < (int)n_used; s++) {
    int64_t eid = (int64_t)ids.data[s];
    if ((eid + 1) * ew * 4 > qs.count || (eid + 1) * ew > ds.count) { coax_trap_(line, coax_err); return; }
  }
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  uint8_t ord[32];
  int inu = (int)n_used;
  if (inu <= 32) coax_sort_eids_asc(ids.data, inu, ord);
  float a0 = 0.0f, a1 = 0.0f;
  for (int ii = 0; ii < inu; ii++) {
    int s = (inu <= 32) ? (int)ord[ii] : ii;
    int64_t eid = (int64_t)ids.data[s];
    float ewt = wts.data[s];
    int64_t b0 = eid * ew + r0 * blocks;
    const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
    const device half *dp0 = (const device half *)(ds.data + b0);
    const device float *xp = x.data + s * (int)cols;
    float s0 = 0.0f, s1 = 0.0f;
    for (int b = blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
      float4 fs = (f0 + f1) + (f2 + f3);
      float xs = (fs.x + fs.y) + (fs.z + fs.w);
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
      s0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xs);
      if (r0 + 1 < rows) s1 += float(dp0[blocks + b]) * coax_q4s_dot16_u2(qp0[blocks * 2 + b * 2], xl0, xl1, xh0, xh1, xs);
    }
    float v0 = simd_sum(s0), v1 = simd_sum(s1);
    if (lane == 0) {
      a0 += ewt * v0;
      a1 += ewt * v1;
    }
  }
  float t0 = a0, t1 = a1;
  if (lane == 0) {
    device float *h0 = hist.data + r0 * 3;
    float u0 = h0[0], u1 = h0[1], u2 = h0[2];
    h0[0] = u1; h0[1] = u2; h0[2] = t0;
    float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
    out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
    if (r0 + 1 < rows) {
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  }
}
COAX_DEV void coax_am_moe_down_q4s_tg(int64_t row0, int64_t rows, int64_t cols, int64_t n_used, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t ew = rows * blocks;
  if (n_used < 0 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || rows > out.count || rows > res.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= rows) return;
  if (n_used == 8 && blocks == 36 && r0 + 2 <= rows) {
    bool bad = (lane < 8u) && ((((int64_t)ids.data[lane] + 1) * ew * 4 > qs.count) || (((int64_t)ids.data[lane] + 1) * ew > ds.count));
    if (simd_any(bad)) { coax_trap_(line, coax_err); return; }
    int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8;
    uint8_t ord[8]; coax_sort_eids_asc(ids.data, 8, ord);
    float a0 = 0.0f, a1 = 0.0f;
    for (int ii = 0; ii < 8; ii++) {
      int s = (int)ord[ii];
      int64_t eid = (int64_t)ids.data[s];
      float ewt = wts.data[s];
      int64_t b0 = eid * ew + r0 * 36;
      const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
      const device uint2 *qp1 = qp0 + 72;
      const device half *dp0 = (const device half *)(ds.data + b0);
      const device half *dp1 = dp0 + 36;
      const device float *xp = x.data + s * 1152;
      float s0 = 0.0f, s1 = 0.0f;
      for (int b = blk_lane; b < 36; b += 16) {
        const device float *bx = xp + b * 32 + xq;
        float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4);
        float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
        float4 fs = (f0 + f1) + (f2 + f3);
        float xs = (fs.x + fs.y) + (fs.z + fs.w);
        half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
        s0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xs);
        s1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xs);
      }
      float v0 = simd_sum(s0), v1 = simd_sum(s1);
      if (lane == 0) {
        a0 += ewt * v0;
        a1 += ewt * v1;
      }
    }
    float t0 = a0, t1 = a1;
    if (lane == 0) {
      device float *h0 = hist.data + r0 * 3;
      float u0 = h0[0], u1 = h0[1], u2 = h0[2];
      h0[0] = u1; h0[1] = u2; h0[2] = t0;
      float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
      out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  } else {
    coax_am_moe_down_q4s_slow(r0, rows, cols, n_used, blocks, ew, qs, ds, x, ids, wts, cw, hist, res, out, lane, line, coax_err);
  }
}
static __attribute__((noinline)) void coax_am_moe_gu_iq4s_slow(int64_t slot, int64_t r0, int64_t n_ff, int64_t blocks, int64_t eb, float inv_rms, float beta, float lb, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 out, threadgroup const half2 *tlut, uint32_t lane) {
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  int64_t bg0 = eb + r0 * blocks, bu0 = bg0 + n_ff * blocks, bg1 = eb + r1 * blocks, bu1 = bg1 + n_ff * blocks;
  const device uint2 *qg0 = (const device uint2 *)(qs.data + bg0 * 4) + hb, *qu0 = (const device uint2 *)(qs.data + bu0 * 4) + hb;
  const device uint2 *qg1 = (const device uint2 *)(qs.data + bg1 * 4) + hb, *qu1 = (const device uint2 *)(qs.data + bu1 * 4) + hb;
  const device half *dg0 = (const device half *)(ds.data + bg0), *du0 = (const device half *)(ds.data + bu0);
  const device half *dg1 = (const device half *)(ds.data + bg1), *du1 = (const device half *)(ds.data + bu1);
  const device float *xp = x.data;
  float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
  for (int b = blk_lane; b < iblocks; b += 16) {
    const device float *bx = xp + b * 32 + xq;
    half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
    half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
    half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
    half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
    ag0 += float(dg0[b]) * coax_iq4s_dot16_u2(qg0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    au0 += float(du0[b]) * coax_iq4s_dot16_u2(qu0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    ag1 += float(dg1[b]) * coax_iq4s_dot16_u2(qg1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    au1 += float(du1[b]) * coax_iq4s_dot16_u2(qu1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
  }
  float inv16 = inv_rms * 16.0f;
  float g0 = simd_sum(ag0) * inv16, u0 = simd_sum(au0) * inv16;
  float g1 = simd_sum(ag1) * inv16, u1 = simd_sum(au1) * inv16;
  if (lane == 0) {
    float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
    float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
    if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
    out.data[slot * n_ff + r0] = a0 * u0;
    if (r0 + 1 < n_ff) out.data[slot * n_ff + r0 + 1] = a1 * u1;
  }
}
COAX_DEV void coax_am_moe_gu_iq4s_tg(int64_t slot, int64_t row0, int64_t n_ff, int64_t cols, int64_t n_exp, int64_t n_used, int64_t has_bias, float r_scale, float beta, float lb, int64_t n_parts, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 parts, coax_v_f32 logits, coax_v_f32 bias, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (n_exp < 1 || n_exp > 128 || n_used < 1 || n_used > 32 || slot < 0 || slot >= n_used || row0 < 0 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || n_parts < 0 || (n_parts > 0 && n_parts > parts.count) || cols > x.count || n_exp > logits.count || (has_bias != 0 && n_exp > bias.count) || n_used > ids.count || n_used > wts.count || (slot + 1) * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  threadgroup int *tg_eid = (threadgroup int *)(tg + 256);
  threadgroup float *tg_inv = tg + 257;
  tlut[tid]        = half2(coax_iq4_lut_f16s[tid & 15u], coax_iq4_lut_f16s[tid >> 4u]);
  tlut[tid + 128u] = half2(coax_iq4_lut_f16s[tid & 15u], coax_iq4_lut_f16s[(tid + 128u) >> 4u]);
  if (tid < 32u) {
    int e = coax_router_pick_slot_noinline((int)n_exp, (int)n_used, (int)has_bias, r_scale, logits.data, bias.data, (int)slot, (slot == 0 && row0 == 0) ? 1 : 0, ids.data, wts.data, tid);
    if (tid == 0u) tg_eid[0] = e;
  } else if (tid < 64u) {
    float inv = coax_inv_rms_parts(parts, 0, n_parts, cols, eps, tid - 32u);
    if (tid == 32u) tg_inv[0] = inv;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int64_t eid = (int64_t)tg_eid[0];
  float inv_rms = tg_inv[0];
  int64_t eb = eid * 2 * n_ff * blocks;
  if ((eb + 2 * n_ff * blocks) * 4 > qs.count || eb + 2 * n_ff * blocks > ds.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  uint32_t lane = tid & 31u;
  if (blocks == 72 && r0 + 2 <= n_ff) {
    int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8;
    int64_t bg0 = eb + r0 * 72, bu0 = bg0 + n_ff * 72;
    const device uint2 *qg0 = (const device uint2 *)(qs.data + bg0 * 4) + hb;
    const device uint2 *qu0 = (const device uint2 *)(qs.data + bu0 * 4) + hb;
    const device uint2 *qg1 = qg0 + 144, *qu1 = qu0 + 144;
    const device half *dg0 = (const device half *)(ds.data + bg0), *dg1 = dg0 + 72;
    const device half *du0 = (const device half *)(ds.data + bu0), *du1 = du0 + 72;
    const device float *xp = x.data;
    float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
    for (int b = blk_lane; b < 72; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
      half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
      half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
      ag0 += float(dg0[b]) * coax_iq4s_dot16_u2(qg0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      au0 += float(du0[b]) * coax_iq4s_dot16_u2(qu0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      ag1 += float(dg1[b]) * coax_iq4s_dot16_u2(qg1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      au1 += float(du1[b]) * coax_iq4s_dot16_u2(qu1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    }
    float inv16 = inv_rms * 16.0f;
    float g0 = simd_sum(ag0) * inv16, u0 = simd_sum(au0) * inv16;
    float g1 = simd_sum(ag1) * inv16, u1 = simd_sum(au1) * inv16;
    if (lane == 0) {
      float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
      float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
      if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
      out.data[slot * n_ff + r0] = a0 * u0;
      out.data[slot * n_ff + r0 + 1] = a1 * u1;
    }
  } else {
    coax_am_moe_gu_iq4s_slow(slot, r0, n_ff, blocks, eb, inv_rms, beta, lb, qs, ds, x, out, tlut, lane);
  }
}
static __attribute__((noinline)) void coax_am_moe_down_iq4s_slow(int64_t r0, int64_t rows, int64_t cols, int64_t n_used, int64_t blocks, int64_t ew, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, threadgroup const half2 *tlut, uint32_t lane, int32_t line, COAX_ERR) {
  for (int s = 0; s < (int)n_used; s++) {
    int64_t eid = (int64_t)ids.data[s];
    if ((eid + 1) * ew * 4 > qs.count || (eid + 1) * ew > ds.count) { coax_trap_(line, coax_err); return; }
  }
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  uint8_t ord[32]; int inu = (int)n_used;
  if (inu <= 32) coax_sort_eids_asc(ids.data, inu, ord);
  float a0 = 0.0f, a1 = 0.0f;
  for (int ii = 0; ii < inu; ii++) {
    int s = (inu <= 32) ? (int)ord[ii] : ii;
    int64_t eid = (int64_t)ids.data[s];
    float ewt = 16.0f * wts.data[s];
    int64_t b0 = eid * ew + r0 * blocks;
    const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
    const device half *dp0 = (const device half *)(ds.data + b0);
    const device float *xp = x.data + s * (int)cols;
    float s0 = 0.0f, s1 = 0.0f;
    for (int b = blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
      half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
      half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
      s0 += float(dp0[b]) * coax_iq4s_dot16_u2(qp0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      if (r0 + 1 < rows) s1 += float(dp0[blocks + b]) * coax_iq4s_dot16_u2(qp0[blocks * 2 + b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    }
    float v0 = simd_sum(s0), v1 = simd_sum(s1);
    if (lane == 0) {
      a0 += ewt * v0;
      a1 += ewt * v1;
    }
  }
  float t0 = a0, t1 = a1;
  if (lane == 0) {
    device float *h0 = hist.data + r0 * 3;
    float u0 = h0[0], u1 = h0[1], u2 = h0[2];
    h0[0] = u1; h0[1] = u2; h0[2] = t0;
    float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
    out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
    if (r0 + 1 < rows) {
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  }
}
COAX_DEV void coax_am_moe_down_iq4s_tg(int64_t row0, int64_t rows, int64_t cols, int64_t n_used, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t ew = rows * blocks;
  if (n_used < 0 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || rows > out.count || rows > res.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  uint32_t lane = tid & 31u;
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= rows) return;
  if (n_used == 8 && blocks == 36 && r0 + 2 <= rows) {
    bool bad = (lane < 8u) && ((((int64_t)ids.data[lane] + 1) * ew * 4 > qs.count) || (((int64_t)ids.data[lane] + 1) * ew > ds.count));
    if (simd_any(bad)) { coax_trap_(line, coax_err); return; }
    int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8;
    uint8_t ord[8]; coax_sort_eids_asc(ids.data, 8, ord);
    float a0 = 0.0f, a1 = 0.0f;
    for (int ii = 0; ii < 8; ii++) {
      int s = (int)ord[ii];
      int64_t eid = (int64_t)ids.data[s];
      float ewt = 16.0f * wts.data[s];
      int64_t b0 = eid * ew + r0 * 36;
      const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
      const device uint2 *qp1 = qp0 + 72;
      const device half *dp0 = (const device half *)(ds.data + b0);
      const device half *dp1 = dp0 + 36;
      const device float *xp = x.data + s * 1152;
      float s0 = 0.0f, s1 = 0.0f;
      for (int b = blk_lane; b < 36; b += 16) {
        const device float *bx = xp + b * 32 + xq;
        half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
        half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
        half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
        s0 += float(dp0[b]) * coax_iq4s_dot16_u2(qp0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
        s1 += float(dp1[b]) * coax_iq4s_dot16_u2(qp1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      }
      float v0 = simd_sum(s0), v1 = simd_sum(s1);
      if (lane == 0) {
        a0 += ewt * v0;
        a1 += ewt * v1;
      }
    }
    float t0 = a0, t1 = a1;
    if (lane == 0) {
      device float *h0 = hist.data + r0 * 3;
      float u0 = h0[0], u1 = h0[1], u2 = h0[2];
      h0[0] = u1; h0[1] = u2; h0[2] = t0;
      float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
      out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  } else {
    coax_am_moe_down_iq4s_slow(r0, rows, cols, n_used, blocks, ew, qs, ds, x, ids, wts, cw, hist, res, out, tlut, lane, line, coax_err);
  }
}
COAX_DEV void coax_am_moe_gu_q4_tg(int64_t slot, int64_t row0, int64_t n_ff, int64_t cols, int64_t n_exp, int64_t n_used, int64_t has_bias, float r_scale, float beta, float lb, int64_t n_parts, float eps, coax_v_u16 w, coax_v_f32 x, coax_v_f32 parts, coax_v_f32 logits, coax_v_f32 bias, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t rw = blocks * 9;
  if (n_exp < 1 || n_exp > 128 || n_used < 1 || n_used > 32 || slot < 0 || slot >= n_used || row0 < 0 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || n_parts < 0 || (n_parts > 0 && n_parts > parts.count) || cols > x.count || n_exp > logits.count || (has_bias != 0 && n_exp > bias.count) || n_used > ids.count || n_used > wts.count || (slot + 1) * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup int *tg_eid = (threadgroup int *)tg;
  threadgroup float *tg_inv = tg + 1;
  if (tid < 32u) {
    int e = coax_router_pick_slot_noinline((int)n_exp, (int)n_used, (int)has_bias, r_scale, logits.data, bias.data, (int)slot, (slot == 0 && row0 == 0) ? 1 : 0, ids.data, wts.data, tid);
    if (tid == 0u) tg_eid[0] = e;
  } else if (tid < 64u) {
    float inv = coax_inv_rms_parts(parts, 0, n_parts, cols, eps, tid - 32u);
    if (tid == 32u) tg_inv[0] = inv;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int64_t eid = (int64_t)tg_eid[0];
  float inv_rms = tg_inv[0];
  int64_t eb = eid * 2 * n_ff * rw;
  if (eb + 2 * n_ff * rw > w.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  uint32_t lane = tid & 31u;
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u), xq = (int)(qtr * 4u);
  const device uint16_t *wg0 = w.data + eb + r0 * rw;
  const device uint16_t *wu0 = wg0 + n_ff * rw;
  const device uint16_t *wg1 = w.data + eb + r1 * rw;
  const device uint16_t *wu1 = wg1 + n_ff * rw;
  const device float *xp = x.data;
  int iblocks = (int)blocks;
  float ag0 = 0.0f, au0 = 0.0f, ag1 = 0.0f, au1 = 0.0f;
  for (int b = (int)blk_lane; b < iblocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    float4 xlf = *(const device float4 *)bx, xhf = *(const device float4 *)(bx + 16);
    float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
    half4 xl = half4(xlf), xh = half4(xhf);
    const device uint16_t *bg0 = wg0 + b * 9, *bu0 = wu0 + b * 9;
    const device uint16_t *bg1 = wg1 + b * 9, *bu1 = wu1 + b * 9;
    ag0 += float(as_type<half>(bg0[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bg0 + 1 + k0), xl, xh, xsum);
    au0 += float(as_type<half>(bu0[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bu0 + 1 + k0), xl, xh, xsum);
    ag1 += float(as_type<half>(bg1[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bg1 + 1 + k0), xl, xh, xsum);
    au1 += float(as_type<half>(bu1[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bu1 + 1 + k0), xl, xh, xsum);
  }
  float g0 = simd_sum(ag0) * inv_rms, u0 = simd_sum(au0) * inv_rms;
  float g1 = simd_sum(ag1) * inv_rms, u1 = simd_sum(au1) * inv_rms;
  if (lane == 0) {
    float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0));
    float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1));
    if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); }
    out.data[slot * n_ff + r0] = a0 * u0;
    if (r0 + 1 < n_ff) out.data[slot * n_ff + r0 + 1] = a1 * u1;
  }
}
static __attribute__((noinline)) void coax_am_moe_down_iq4_slow(int64_t r0, int64_t rows, int64_t cols, int64_t n_used, int64_t rw, int64_t ew, coax_v_u16 w, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, threadgroup const half2 *tlut, uint32_t lane, int32_t line, COAX_ERR) {
  for (int s = 0; s < (int)n_used; s++) {
    int64_t eid = (int64_t)ids.data[s];
    if ((eid + 1) * ew > w.count) { coax_trap_(line, coax_err); return; }
  }
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u);
  int k0 = hb * 4, xq = hb * 8, iblocks = (int)(cols >> 5);
  uint8_t ord[32]; int inu = (int)n_used;
  if (inu <= 32) coax_sort_eids_asc(ids.data, inu, ord);
  float a0 = 0.0f, a1 = 0.0f;
  for (int ii = 0; ii < inu; ii++) {
    int s = (inu <= 32) ? (int)ord[ii] : ii;
    int64_t eid = (int64_t)ids.data[s];
    float ewt = 16.0f * wts.data[s];
    const device uint16_t *wbase = w.data + eid * ew + r0 * rw;
    const device float *xp = x.data + s * (int)cols;
    float s0 = 0.0f, s1 = 0.0f;
    for (int b = blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32 + xq;
      half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
      half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
      half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
      const device uint16_t *bw = wbase + b * 9;
      s0 += float(as_type<half>(bw[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bw + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      if (r0 + 1 < rows) s1 += float(as_type<half>(bw[rw])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bw + rw + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    }
    float v0 = simd_sum(s0), v1 = simd_sum(s1);
    if (lane == 0) {
      a0 += ewt * v0;
      a1 += ewt * v1;
    }
  }
  float t0 = a0, t1 = a1;
  if (lane == 0) {
    device float *h0 = hist.data + r0 * 3;
    float u0 = h0[0], u1 = h0[1], u2 = h0[2];
    h0[0] = u1; h0[1] = u2; h0[2] = t0;
    float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
    out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
    if (r0 + 1 < rows) {
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  }
}
COAX_DEV void coax_am_moe_down_iq4_tg(int64_t row0, int64_t rows, int64_t cols, int64_t n_used, coax_v_u16 w, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t rw = blocks * 9;
  int64_t ew = rows * rw;
  if (n_used < 0 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || rows > out.count || rows > res.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  uint32_t lane = tid & 31u;
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= rows) return;
  if (n_used == 8 && blocks == 36 && r0 + 2 <= rows) {
    bool bad = (lane < 8u) && (((int64_t)ids.data[lane] + 1) * ew > w.count);
    if (simd_any(bad)) { coax_trap_(line, coax_err); return; }
    int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u);
    int k0 = hb * 4, xq = hb * 8;
    uint8_t ord[8]; coax_sort_eids_asc(ids.data, 8, ord);
    float a0 = 0.0f, a1 = 0.0f;
    for (int ii = 0; ii < 8; ii++) {
      int s = (int)ord[ii];
      int64_t eid = (int64_t)ids.data[s];
      float ewt = 16.0f * wts.data[s];
      const device uint16_t *wbase = w.data + eid * ew + r0 * 324;
      const device float *xp = x.data + s * 1152;
      float s0 = 0.0f, s1 = 0.0f;
      for (int b = blk_lane; b < 36; b += 16) {
        const device float *bx = xp + b * 32 + xq;
        half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
        half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
        half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
        const device uint16_t *bw = wbase + b * 9;
        s0 += float(as_type<half>(bw[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bw + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
        bw += 324;
        s1 += float(as_type<half>(bw[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bw + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      }
      float v0 = simd_sum(s0), v1 = simd_sum(s1);
      if (lane == 0) {
        a0 += ewt * v0;
        a1 += ewt * v1;
      }
    }
    float t0 = a0, t1 = a1;
    if (lane == 0) {
      device float *h0 = hist.data + r0 * 3;
      float u0 = h0[0], u1 = h0[1], u2 = h0[2];
      h0[0] = u1; h0[1] = u2; h0[2] = t0;
      float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
      out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  } else {
    coax_am_moe_down_iq4_slow(r0, rows, cols, n_used, rw, ew, w, x, ids, wts, cw, hist, res, out, tlut, lane, line, coax_err);
  }
}
static __attribute__((noinline)) void coax_am_moe_down_q4_slow(int64_t r0, int64_t rows, int64_t cols, int64_t n_used, int64_t rw, int64_t ew, coax_v_u16 w, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, uint32_t lane, int32_t line, COAX_ERR) {
  for (int s = 0; s < (int)n_used; s++) {
    int64_t eid = (int64_t)ids.data[s];
    if ((eid + 1) * ew > w.count) { coax_trap_(line, coax_err); return; }
  }
  int blk_lane = (int)(lane >> 2), qtr = (int)(lane & 3u);
  int k0 = qtr * 2, xq = qtr * 4, iblocks = (int)(cols >> 5);
  float a0 = 0.0f, a1 = 0.0f;
  for (int s = 0; s < (int)n_used; s++) {
    int64_t eid = (int64_t)ids.data[s];
    float ewt = wts.data[s];
    const device uint16_t *wbase = w.data + eid * ew + r0 * rw;
    const device float *xp = x.data + s * (int)cols;
    float s0 = 0.0f, s1 = 0.0f;
    for (int b = blk_lane; b < iblocks; b += 8) {
      const device float *bx = xp + b * 32 + xq;
      float4 xlf = *(const device float4 *)bx, xhf = *(const device float4 *)(bx + 16);
      float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
      half4 xl = half4(xlf), xh = half4(xhf);
      const device uint16_t *bw = wbase + b * 9;
      s0 += float(as_type<half>(bw[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw + 1 + k0), xl, xh, xsum);
      if (r0 + 1 < rows) s1 += float(as_type<half>(bw[rw])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw + rw + 1 + k0), xl, xh, xsum);
    }
    float v0 = simd_sum(s0), v1 = simd_sum(s1);
    if (lane == 0) {
      a0 += ewt * v0;
      a1 += ewt * v1;
    }
  }
  float t0 = a0, t1 = a1;
  if (lane == 0) {
    device float *h0 = hist.data + r0 * 3;
    float u0 = h0[0], u1 = h0[1], u2 = h0[2];
    h0[0] = u1; h0[1] = u2; h0[2] = t0;
    float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
    out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
    if (r0 + 1 < rows) {
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  }
}
COAX_DEV void coax_am_moe_down_q4_tg(int64_t row0, int64_t rows, int64_t cols, int64_t n_used, coax_v_u16 w, coax_v_f32 x, coax_v_u32 ids, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 out, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t rw = blocks * 9;
  int64_t ew = rows * rw;
  if (n_used < 0 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_used > ids.count || n_used > wts.count || n_used * cols > x.count || rows > out.count || rows > res.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= rows) return;
  if (n_used == 8 && blocks == 36 && r0 + 2 <= rows) {
    bool bad = (lane < 8u) && (((int64_t)ids.data[lane] + 1) * ew > w.count);
    if (simd_any(bad)) { coax_trap_(line, coax_err); return; }
    int blk_lane = (int)(lane >> 2), qtr = (int)(lane & 3u);
    int k0 = qtr * 2, xq = qtr * 4;
    float a0 = 0.0f, a1 = 0.0f;
    for (int s = 0; s < 8; s++) {
      int64_t eid = (int64_t)ids.data[s];
      float ewt = wts.data[s];
      const device uint16_t *wbase = w.data + eid * ew + r0 * 324;
      const device float *xp = x.data + s * 1152;
      float s0 = 0.0f, s1 = 0.0f;
      for (int b = blk_lane; b < 36; b += 8) {
        const device float *bx = xp + b * 32 + xq;
        float4 xlf = *(const device float4 *)bx, xhf = *(const device float4 *)(bx + 16);
        float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
        half4 xl = half4(xlf), xh = half4(xhf);
        const device uint16_t *bw = wbase + b * 9;
        s0 += float(as_type<half>(bw[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw + 1 + k0), xl, xh, xsum);
        bw += 324;
        s1 += float(as_type<half>(bw[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw + 1 + k0), xl, xh, xsum);
      }
      a0 += ewt * s0;
      a1 += ewt * s1;
    }
    float t0 = simd_sum(a0), t1 = simd_sum(a1);
    if (lane == 0) {
      device float *h0 = hist.data + r0 * 3;
      float u0 = h0[0], u1 = h0[1], u2 = h0[2];
      h0[0] = u1; h0[1] = u2; h0[2] = t0;
      float4 c0 = *(const device float4 *)(cw.data + r0 * 4);
      out.data[r0] = res.data[r0] + (t0 + u0 * c0.x + u1 * c0.y + u2 * c0.z + t0 * c0.w) * 0.4472135954999579f;
      int64_t r1 = r0 + 1;
      device float *h1 = hist.data + r1 * 3;
      float v0 = h1[0], v1 = h1[1], v2 = h1[2];
      h1[0] = v1; h1[1] = v2; h1[2] = t1;
      float4 c1 = *(const device float4 *)(cw.data + r1 * 4);
      out.data[r1] = res.data[r1] + (t1 + v0 * c1.x + v1 * c1.y + v2 * c1.z + t1 * c1.w) * 0.4472135954999579f;
    }
  } else {
    coax_am_moe_down_q4_slow(r0, rows, cols, n_used, rw, ew, w, x, ids, wts, cw, hist, res, out, lane, line, coax_err);
  }
}
COAX_DEV void coax_am_dot_q4_qrgkv(int64_t row0, int64_t n_q, int64_t n_r, int64_t n_kv, int64_t cols, int64_t n_parts, float eps, coax_v_u16 w, coax_v_f32 xq_v, coax_v_f32 xk_v, coax_v_f32 xv_v, coax_v_f32 parts, coax_v_f32 oq, coax_v_f32 or_, coax_v_f32 og, coax_v_f32 ok, coax_v_f32 ov, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  int64_t rw = blocks * 9;
  int64_t n_qr = n_q + n_r, n_qrg = n_qr + n_q, n_qrgk = n_qrg + n_kv, tot = n_qrgk + n_kv;
  if (cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_q < 0 || n_r < 0 || n_kv < 0 || (n_q & 1) != 0 || (n_r & 1) != 0 || (n_kv & 1) != 0 || n_parts < 0 || (n_parts > 0 && 3 * n_parts > parts.count) || tot * rw > w.count || cols > xq_v.count || cols > xk_v.count || cols > xv_v.count || n_q > oq.count || n_r > or_.count || n_q > og.count || n_kv > ok.count || n_kv > ov.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= tot) return;
  const device float *xp;
  device float *op;
  int64_t orow, poff;
  if (r0 < n_q) { xp = xq_v.data; op = oq.data; orow = r0; poff = 0; }
  else if (r0 < n_qr) { xp = xq_v.data; op = or_.data; orow = r0 - n_q; poff = 0; }
  else if (r0 < n_qrg) { xp = xq_v.data; op = og.data; orow = r0 - n_qr; poff = 0; }
  else if (r0 < n_qrgk) { xp = xk_v.data; op = ok.data; orow = r0 - n_qrg; poff = n_parts; }
  else { xp = xv_v.data; op = ov.data; orow = r0 - n_qrgk; poff = 2 * n_parts; }
  uint32_t lane = tid & 31u;
  float inv_rms = coax_inv_rms_parts(parts, poff, n_parts, cols, eps, lane);
  uint32_t blk_lane = lane >> 2;
  uint32_t qtr = lane & 3u;
  int k0 = (int)(qtr * 2u), xq = (int)(qtr * 4u);
  const device uint16_t *wp0 = w.data + r0 * rw;
  const device uint16_t *wp1 = wp0 + rw;
  int iblocks = (int)blocks;
  float acc0 = 0.0f, acc1 = 0.0f;
  for (int b = (int)blk_lane; b < iblocks; b += 8) {
    const device float *bx = xp + b * 32 + xq;
    float4 xlf = *(const device float4 *)bx;
    float4 xhf = *(const device float4 *)(bx + 16);
    float xsum = (xlf.x + xlf.y) + (xlf.z + xlf.w) + (xhf.x + xhf.y) + (xhf.z + xhf.w);
    half4 xl = half4(xlf), xh = half4(xhf);
    const device uint16_t *bw0 = wp0 + b * 9, *bw1 = wp1 + b * 9;
    acc0 += float(as_type<half>(bw0[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw0 + 1 + k0), xl, xh, xsum);
    acc1 += float(as_type<half>(bw1[0])) * coax_q4_alu_dot8_f16(*(const device packed_ushort2 *)(bw1 + 1 + k0), xl, xh, xsum);
  }
  float t0 = simd_sum(acc0) * inv_rms, t1 = simd_sum(acc1) * inv_rms;
  if (lane == 0) { op[orow] = t0; op[orow + 1] = t1; }
}
COAX_DEV void coax_am_ctrl_up_apply_gf_impl(int64_t dim, int64_t rank, int64_t row0, coax_v_u32 w_gf, int64_t w_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, bool do_norm, const device float *w_norm_p, coax_v_f32 out, int64_t o_off, device float *part_dst, uint32_t tid) {
  uint32_t lane = tid & 31u;
  int64_t r0 = row0 + (int64_t)(lane * 2u);
  int idim = (int)dim, irank = (int)rank;
  float v0 = 0.0f, v1 = 0.0f;
  if (row0 + 64 <= dim && (idim & 1) == 0 && irank == 64 && (((uintptr_t)(w_gf.data + w_off + row0)) & 7u) == 0) {
    float g0 = gate.data[g_off + lane];
    float g1 = gate.data[g_off + lane + 32];
    const device half4 *wp = (const device half4 *)(w_gf.data + w_off + r0);
    int s4 = idim >> 1;
    float2 a0 = float2(0.0f), a1 = float2(0.0f);
    for (int k = 0; k < 32; k += 4) {
      float k0 = simd_shuffle(g0, k), k1 = simd_shuffle(g0, k + 1), k2 = simd_shuffle(g0, k + 2), k3 = simd_shuffle(g0, k + 3);
      half4 p0 = wp[0], p1 = wp[s4], p2 = wp[2 * s4], p3 = wp[3 * s4];
      a0 += float2(p0.xy) * k0 + float2(p1.xy) * k1 + float2(p2.xy) * k2 + float2(p3.xy) * k3;
      a1 += float2(p0.zw) * k0 + float2(p1.zw) * k1 + float2(p2.zw) * k2 + float2(p3.zw) * k3;
      wp += 4 * s4;
    }
    for (int k = 0; k < 32; k += 4) {
      float k0 = simd_shuffle(g1, k), k1 = simd_shuffle(g1, k + 1), k2 = simd_shuffle(g1, k + 2), k3 = simd_shuffle(g1, k + 3);
      half4 p0 = wp[0], p1 = wp[s4], p2 = wp[2 * s4], p3 = wp[3 * s4];
      a0 += float2(p0.xy) * k0 + float2(p1.xy) * k1 + float2(p2.xy) * k2 + float2(p3.xy) * k3;
      a1 += float2(p0.zw) * k0 + float2(p1.zw) * k1 + float2(p2.zw) * k2 + float2(p3.zw) * k3;
      wp += 4 * s4;
    }
    float2 cv = *(const device float2 *)(cur.data + c_off + r0);
    float2 mv = *(const device float2 *)(msg.data + m_off + r0);
    v0 = cv.x - tanh(a0.y) * cv.x + tanh(a0.x) * mv.x;
    v1 = cv.y - tanh(a1.y) * cv.y + tanh(a1.x) * mv.y;
    if (do_norm) {
      float2 wv = *(const device float2 *)(w_norm_p + r0);
      *(device float2 *)(out.data + o_off + r0) = float2(v0 * wv.x, v1 * wv.y);
    } else {
      *(device float2 *)(out.data + o_off + r0) = float2(v0, v1);
    }
  } else {
    if (r0 < dim) {
      const device half2 *wp0 = (const device half2 *)(w_gf.data + w_off) + r0;
      float2 a0 = float2(0.0f);
      for (int k = 0; k < irank; k++) a0 += float2(wp0[k * idim]) * gate.data[g_off + k];
      float c0 = cur.data[c_off + r0], m0 = msg.data[m_off + r0];
      v0 = c0 - tanh(a0.y) * c0 + tanh(a0.x) * m0;
      out.data[o_off + r0] = do_norm ? (v0 * w_norm_p[r0]) : v0;
    }
    if (r0 + 1 < dim) {
      const device half2 *wp1 = (const device half2 *)(w_gf.data + w_off) + (r0 + 1);
      float2 a1 = float2(0.0f);
      for (int k = 0; k < irank; k++) a1 += float2(wp1[k * idim]) * gate.data[g_off + k];
      float c1 = cur.data[c_off + r0 + 1], m1 = msg.data[m_off + r0 + 1];
      v1 = c1 - tanh(a1.y) * c1 + tanh(a1.x) * m1;
      out.data[o_off + r0 + 1] = do_norm ? (v1 * w_norm_p[r0 + 1]) : v1;
    }
  }
  if (do_norm) {
    float ss = simd_sum(v0 * v0 + v1 * v1);
    if (lane == 0) *part_dst = ss;
  }
}
COAX_DEV void coax_am_ctrl_up_apply_gf(int64_t dim, int64_t rank, int64_t row0, coax_v_u32 w_gf, int64_t w_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, coax_v_f32 out, int64_t o_off, uint32_t tid, int32_t line, COAX_ERR) {
  if (dim <= 0 || rank < 0 || rank > 64 || row0 < 0 || w_off < 0 || w_off + rank * dim > w_gf.count || g_off < 0 || g_off + rank > gate.count || c_off < 0 || c_off + dim > cur.count || m_off < 0 || m_off + dim > msg.count || o_off < 0 || o_off + dim > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  coax_am_ctrl_up_apply_gf_impl(dim, rank, row0, w_gf, w_off, gate, g_off, cur, c_off, msg, m_off, false, nullptr, out, o_off, nullptr, tid);
}
COAX_DEV void coax_am_ctrl_up_apply_gf_norm(int64_t dim, int64_t rank, int64_t row0, coax_v_u32 w_gf, int64_t w_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, coax_v_f32 w_norm, coax_v_f32 out, int64_t o_off, coax_v_f32 rms_parts, int64_t part_idx, uint32_t tid, int32_t line, COAX_ERR) {
  if (dim <= 0 || rank < 0 || rank > 64 || row0 < 0 || w_off < 0 || w_off + rank * dim > w_gf.count || g_off < 0 || g_off + rank > gate.count || c_off < 0 || c_off + dim > cur.count || m_off < 0 || m_off + dim > msg.count || dim > w_norm.count || o_off < 0 || o_off + dim > out.count || part_idx < 0 || part_idx >= rms_parts.count) {
    coax_trap_(line, coax_err);
    return;
  }
  coax_am_ctrl_up_apply_gf_impl(dim, rank, row0, w_gf, w_off, gate, g_off, cur, c_off, msg, m_off, true, w_norm.data, out, o_off, rms_parts.data + part_idx, tid);
}
COAX_DEV void coax_am_ctrl_up_apply_gf_rm_norm(int64_t dim, int64_t rank, int64_t row0, coax_v_u32 w_gf, int64_t w_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, coax_v_f32 w_norm, coax_v_f32 out, int64_t o_off, coax_v_f32 rms_parts, int64_t part_idx, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (dim <= 0 || rank < 0 || rank > 64 || row0 < 0 || w_off < 0 || w_off + dim * rank > w_gf.count || g_off < 0 || g_off + rank > gate.count || c_off < 0 || c_off + dim > cur.count || m_off < 0 || m_off + dim > msg.count || dim > w_norm.count || o_off < 0 || o_off + dim > out.count || part_idx < 0 || part_idx >= rms_parts.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u, sg = tid >> 5;
  float ss = 0.0f;
  if (row0 + 32 <= dim && rank == 64 && (((uintptr_t)(w_gf.data + w_off)) & 7u) == 0) {
    float g0 = gate.data[g_off + lane * 2u], g1 = gate.data[g_off + lane * 2u + 1u];
    const device half4 *w4 = (const device half4 *)(w_gf.data + w_off);
    int64_t base = row0 + (int64_t)(sg * 8u);
    float my_sg = 0.0f, my_sf = 0.0f;
    for (int j = 0; j < 8; j++) {
      half4 p0 = w4[(base + j) * 32 + (int)lane];
      float sg0 = simd_sum(float(p0.x) * g0 + float(p0.z) * g1);
      float sf0 = simd_sum(float(p0.y) * g0 + float(p0.w) * g1);
      if ((int)lane == j) { my_sg = sg0; my_sf = sf0; }
    }
    float vv = 0.0f;
    if (lane < 8u) {
      int64_t r = base + (int64_t)lane;
      float c = cur.data[c_off + r], m = msg.data[m_off + r];
      vv = c - tanh(my_sf) * c + tanh(my_sg) * m;
      out.data[o_off + r] = vv * w_norm.data[r];
    }
    ss = simd_sum(vv * vv);
  } else {
    int irank = (int)rank;
    int64_t base = row0 + (int64_t)(sg * 8u);
    const device half2 *w2 = (const device half2 *)(w_gf.data + w_off);
    for (int j = 0; j < 8; j++) {
      int64_t r = base + j;
      if (r < dim) {
        float psg = 0.0f, psf = 0.0f;
        for (int k = (int)lane; k < irank; k += 32) {
          float gv = gate.data[g_off + k];
          half2 p = w2[r * irank + k];
          psg += float(p.x) * gv;
          psf += float(p.y) * gv;
        }
        float sgv = simd_sum(psg), sfv = simd_sum(psf);
        if (lane == 0) {
          float c = cur.data[c_off + r], m = msg.data[m_off + r];
          float v = c - tanh(sfv) * c + tanh(sgv) * m;
          out.data[o_off + r] = v * w_norm.data[r];
          ss += v * v;
        }
      }
    }
  }
  if (lane == 0) tg[sg] = ss;
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0) rms_parts.data[part_idx] = (tg[0] + tg[1]) + (tg[2] + tg[3]);
}
COAX_DEV void coax_am_kv_attn_gqa_1pass(int64_t h, uint32_t dims0, uint32_t dims1, uint32_t kv_info, int64_t pos, int64_t slot_pos, float qk_clip, float rel_clip, float q_tau, float eps, coax_v_f32 k_raw, coax_v_f32 v_raw, coax_v_f32 kw, coax_v_f32 vw, coax_v_f32 dyn, coax_v_f32 k_hist, coax_v_f32 v_hist, coax_v_f32 q, coax_v_f32 q_scale, coax_v_f32 r, coax_v_f32 r_scale, coax_v_f32 rel_bias, coax_v_f32 sinks, coax_v_f32 gate, coax_v_u16 k_cache, coax_v_u16 v_cache, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int n_head = (int)(dims0 & 255u);
  int n_head_kv = (int)((dims0 >> 8u) & 255u);
  int head_dim = (int)((dims0 >> 16u) & 255u);
  int n_rel = (int)((dims0 >> 24u) & 255u);
  int extent = (int)(dims1 & 65535u);
  int is_swa = (int)((dims1 >> 16u) & 1u);
  int swa = (int)((dims1 >> 17u) & 32767u);
  int kv_cap = (int)(kv_info & 65535u);
  int has_qs = (int)((kv_info >> 16u) & 1u);
  int has_rs = (int)((kv_info >> 17u) & 1u);
  int has_dyn = (int)((kv_info >> 18u) & 1u);
  int64_t kv_h = h / 3;
  bool is_writer = ((h % 3) == 0);
  int64_t n_kv_elem = (int64_t)n_head_kv * 128;
  int64_t base = kv_h * 128;
  int64_t dbase = slot_pos * n_kv_elem + base;
  if (head_dim != 128 || n_rel > 32 || pos < 0 || h < 0 || h >= n_head || kv_h >= n_head_kv || base + 128 > k_raw.count || base + 128 > v_raw.count || (base + 128) * 4 > kw.count || (base + 128) * 4 > vw.count || (base + 128) * 4 > k_hist.count || (base + 128) * 4 > v_hist.count || (has_dyn && (n_head_kv + kv_h + 1) * 4 > dyn.count) || (h + 1) * 128 > q.count || (h + 1) * 128 > gate.count || (h + 1) * 128 > out.count || h >= sinks.count || (h + 1) * n_rel > r.count || (has_qs && (h + 1) * 128 > q_scale.count) || (has_rs && (h + 1) * n_rel > r_scale.count) || (int64_t)kv_cap * n_kv_elem > k_cache.count || (int64_t)kv_cap * n_kv_elem > v_cache.count || (int64_t)extent * n_rel > rel_bias.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup float *tg_k = tg;          /* 128 */
  threadgroup float *tg_v = tg + 128;    /* 128 */
  threadgroup float *tg_q = tg + 256;    /* 128 */
  threadgroup float *tg_r = tg + 384;    /* 32 */
  threadgroup float *msh  = tg + 416;    /* 8 */
  threadgroup float *lsh  = tg + 424;    /* 8 */
  threadgroup float *vsh  = tg + 432;    /* 1024 */
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  /* Phase 1: Warp 0 = K conv+clip (writer stores), Warp 1 = V conv+norm (writer stores), Warp 2 = Q & R clip! */
  if (sg == 0u || sg == 1u) {
    int is_v = (int)sg;
    uint32_t s0 = (uint32_t)(pos + 1) & 3u, s1 = (uint32_t)(pos + 2) & 3u, s2 = (uint32_t)(pos + 3) & 3u, sw = (uint32_t)pos & 3u;
    float inv_s = has_dyn ? 0.3333333333333333f : 0.4472135954999579f;
    float4 dv = has_dyn ? *(const device float4 *)(dyn.data + (is_v * n_head_kv + kv_h) * 4) : float4(0.0f);
    int64_t c0 = base + (int64_t)(lane * 4u);
    const device float *rawp = is_v ? v_raw.data : k_raw.data;
    const device float *wp = is_v ? vw.data : kw.data;
    device float *hp = is_v ? v_hist.data : k_hist.data;
    float4 xr = *(const device float4 *)(rawp + c0);
    float vals[4];
    for (int j = 0; j < 4; j++) {
      int64_t c = c0 + j;
      device float *hc = hp + c * 4;
      float4 h4 = *(const device float4 *)hc;
      float x0 = h4[s0], x1 = h4[s1], x2 = h4[s2], x3 = xr[j];
      if (is_writer) hc[sw] = x3;
      float4 cw = *(const device float4 *)(wp + c * 4);
      float a = x0 * cw.x + x1 * cw.y + x2 * cw.z + x3 * cw.w;
      if (has_dyn) a += dv.x * x3 + dv.y * x2 + dv.z * x1 + dv.w * x0;
      vals[j] = (x3 + a) * inv_s;
    }
    float4 vv = float4(vals[0], vals[1], vals[2], vals[3]);
    float tot = simd_sum(dot(vv, vv));
    if (is_v == 0) {
      float fac = qk_clip / (sqrt(tot) + 1.0e-6f);
      if (fac > 1.0f) fac = 1.0f;
      half4 hv = half4(vv * fac);
      if (is_writer) *(device half4 *)(k_cache.data + dbase + lane * 4u) = hv;
      *(threadgroup float4 *)(tg_k + lane * 4u) = float4(hv);
    } else {
      float inv = rsqrt(tot * (1.0f / 128.0f) + eps);
      half4 hv = half4(vv * inv);
      if (is_writer) *(device half4 *)(v_cache.data + dbase + lane * 4u) = hv;
      *(threadgroup float4 *)(tg_v + lane * 4u) = float4(hv);
    }
  } else if (sg == 2u) {
    int64_t qb = h * 128 + (int64_t)(lane * 4u);
    float4 qv = *(const device float4 *)(q.data + qb);
    if (has_qs) qv *= *(const device float4 *)(q_scale.data + qb);
    float qfac = qk_clip / (sqrt(simd_sum(dot(qv, qv))) + 1.0e-6f);
    if (qfac > 1.0f) qfac = 1.0f;
    *(threadgroup float4 *)(tg_q + lane * 4u) = qv * (qfac * q_tau);
    float rv = 0.0f;
    if ((int)lane < n_rel) {
      int64_t ridx = h * n_rel + (int64_t)lane;
      rv = r.data[ridx];
      if (has_rs) rv *= r_scale.data[ridx];
    }
    float rfac = rel_clip / (sqrt(simd_sum(rv * rv)) + 1.0e-6f);
    if (rfac > 1.0f) rfac = 1.0f;
    tg_r[lane] = rv * (rfac * q_tau);
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  /* Phase 2: All 8 warps stream KV cache across 24 TGs (aligned kp&7 warp assignment for bit-exact SWA wrap parity!) */
  int64_t start = (is_swa && pos >= swa) ? (pos - swa + 1) : 0;
  float scale = 1.0f / sqrt(128.0f);
  float4 q0 = *(threadgroup float4 *)(tg_q + lane * 4u);
  float r0 = tg_r[lane];
  float sk0 = sinks.data[h];
  float m0 = -3.0e38f, l0 = 0.0f;
  float4 a0 = float4(0.0f);
  int64_t kh_off = base + (int64_t)(lane * 4u);
  for (int64_t kp_base = (start & ~7LL); kp_base <= pos; kp_base += 8) {
    int64_t kp = kp_base + (int64_t)sg;
    if (kp < start || kp > pos) continue;
    float4 kv, vv;
    if (kp == pos) {
      kv = *(threadgroup float4 *)(tg_k + lane * 4u);
      vv = *(threadgroup float4 *)(tg_v + lane * 4u);
    } else {
      int64_t slot = (is_swa && kp >= kv_cap) ? (int64_t)((int)kp % kv_cap) : kp;
      int64_t kb = slot * n_kv_elem + kh_off;
      kv = float4(*(const device half4 *)(k_cache.data + kb));
      vv = float4(*(const device half4 *)(v_cache.data + kb));
    }
    int dist = (int)(pos - kp);
    float rbv = (dist < extent && (int)lane < n_rel) ? rel_bias.data[dist * n_rel + (int)lane] : 0.0f;
    float sc0 = simd_sum(dot(q0, kv)) * scale + simd_sum(rbv * r0) - sk0;
    if (sc0 > m0) {
      float al0 = (l0 > 0.0f) ? exp(m0 - sc0) : 0.0f;
      m0 = sc0;
      l0 = l0 * al0 + 1.0f;
      a0 = a0 * al0 + vv;
    } else {
      float wt0 = exp(sc0 - m0);
      l0 += wt0;
      a0 += wt0 * vv;
    }
  }
  if (lane == 0) { msh[sg] = m0; lsh[sg] = l0; }
  *(threadgroup float4 *)(vsh + sg * 128 + lane * 4u) = a0;
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (sg == 0u) {
    float mv = (lane < 8u) ? msh[lane] : -3.0e38f;
    float lv = (lane < 8u) ? lsh[lane] : 0.0f;
    float gmax = simd_max(mv);
    if (gmax < 0.0f) gmax = 0.0f;
    float sc = (lane < 8u && lv > 0.0f) ? exp(mv - gmax) : 0.0f;
    float inv_gsum = 1.0f / (exp(-gmax) + simd_sum(lv * sc));
    float4 vtot = float4(0.0f);
    #pragma unroll
    for (int s = 0; s < 8; s++) {
      vtot += *(threadgroup float4 *)(vsh + s * 128 + lane * 4u) * simd_shuffle(sc, s);
    }
    int64_t oidx = h * 128 + (int64_t)(lane * 4u);
    float4 gv = *(const device float4 *)(gate.data + oidx);
    *(device float4 *)(out.data + oidx) = (vtot * inv_gsum) / (float4(1.0f) + exp(-gv));
  }
}
COAX_DEV void coax_am_dot_q4s_2_write_nb(coax_v_u32 qs, coax_v_u16 ds, int64_t b0, int64_t b1, coax_v_f32 x, int64_t x_off, int64_t x_stride, int64_t blocks, int64_t nb, float cap, coax_v_f32 out, int64_t out_off, int64_t out_stride, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  if (blocks < 0 || nb < 1 || nb > 8 || b0 < 0 || (b0 + blocks) * 4 > qs.count || b0 + blocks > ds.count || b1 < 0 || (b1 + blocks) * 4 > qs.count || b1 + blocks > ds.count || x_off < 0 || x_off + (nb - 1) * x_stride + blocks * 32 > x.count || out_off < 0 || r0 < 0 || r1 < 0 || out_off + (nb - 1) * out_stride + r0 >= out.count || out_off + (nb - 1) * out_stride + r1 >= out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u, blk_lane = lane >> 1, hb = lane & 1u;
  int xq = (int)(hb * 8u), inb = (int)nb, iblocks = (int)blocks;
  const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
  const device half *dp0 = (const device half *)(ds.data + b0);
  const device float *xp = x.data + x_off;
  if (r1 == r0) {
    float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
    #define Q4_1R(T, A) if ((T) < inb) { \
      const device float *bx = xp + (int64_t)(T) * x_stride + bo; \
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
      float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
      A += d0 * coax_q4s_dot16_pre(p0, p1, p2, p3, half4(f0), half4(f1), half4(f2), half4(f3), xs); }
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      uint2 w0 = qp0[b * 2]; float d0 = float(dp0[b]);
      half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu));
      half4 p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
      half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu));
      half4 p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
      int bo = b * 32 + xq;
      Q4_1R(0,a0) Q4_1R(1,a1) Q4_1R(2,a2) Q4_1R(3,a3)
      Q4_1R(4,a4) Q4_1R(5,a5) Q4_1R(6,a6) Q4_1R(7,a7)
    }
    #undef Q4_1R
    #define Q4_1W(T, A) if ((T) < inb) { \
      float v = simd_sum(A); \
      if (lane == 0) { if (cap > 0.0f) v = cap * tanh(v / cap); out.data[out_off + (int64_t)(T) * out_stride + r0] = v; } }
    Q4_1W(0,a0) Q4_1W(1,a1) Q4_1W(2,a2) Q4_1W(3,a3)
    Q4_1W(4,a4) Q4_1W(5,a5) Q4_1W(6,a6) Q4_1W(7,a7)
    #undef Q4_1W
    return;
  }
  const device uint2 *qp1 = (const device uint2 *)(qs.data + b1 * 4) + hb;
  const device half *dp1 = (const device half *)(ds.data + b1);
  for (int tbase = 0; tbase < inb; tbase += 4) {
    int rem = inb - tbase; if (rem > 4) rem = 4;
    const device float *xt = xp + (int64_t)tbase * x_stride;
    float a0_0=0,a1_0=0,a0_1=0,a1_1=0,a0_2=0,a1_2=0,a0_3=0,a1_3=0;
    #define Q4NB_STEP(T, A0, A1) if ((T) < rem) { \
      const device float *bx = xt + (int64_t)(T) * x_stride + bo; \
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
      float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
      half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3); \
      A0 += d0 * coax_q4s_dot16_pre(p0, p1, p2, p3, xl0, xl1, xh0, xh1, xs); \
      A1 += d1 * coax_q4s_dot16_pre(q0, q1, q2, q3, xl0, xl1, xh0, xh1, xs); }
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      uint2 w0 = qp0[b * 2], w1 = qp1[b * 2];
      float d0 = float(dp0[b]), d1 = float(dp1[b]);
      half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
      half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
      half4 q0 = half4(as_type<uchar4>(w1.x & 0x0f0f0f0fu)), q1 = half4(as_type<uchar4>((w1.x >> 4u) & 0x0f0f0f0fu));
      half4 q2 = half4(as_type<uchar4>(w1.y & 0x0f0f0f0fu)), q3 = half4(as_type<uchar4>((w1.y >> 4u) & 0x0f0f0f0fu));
      int bo = b * 32 + xq;
      Q4NB_STEP(0,a0_0,a1_0) Q4NB_STEP(1,a0_1,a1_1) Q4NB_STEP(2,a0_2,a1_2) Q4NB_STEP(3,a0_3,a1_3)
    }
    #undef Q4NB_STEP
    #define Q4NB_WR(T, A0, A1) if ((T) < rem) { \
      float t0 = simd_sum(A0), t1 = simd_sum(A1); \
      if (lane == 0) { \
        if (cap > 0.0f) { t0 = cap * tanh(t0 / cap); t1 = cap * tanh(t1 / cap); } \
        int64_t oo = out_off + (int64_t)(tbase + (T)) * out_stride; \
        out.data[oo + r0] = t0; out.data[oo + r1] = t1; } }
    Q4NB_WR(0,a0_0,a1_0) Q4NB_WR(1,a0_1,a1_1) Q4NB_WR(2,a0_2,a1_2) Q4NB_WR(3,a0_3,a1_3)
    #undef Q4NB_WR
  }
}
COAX_DEV void coax_am_dot_q4s_8row_nb(int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t row_base, int64_t x_off, int64_t out_off, float cap, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 out, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (cols <= 0 || (cols & 31) != 0 || rows <= 0 || nb < 1 || nb > 8 || row0 < 0 || row_base < 0 || (row_base + rows) * blocks * 4 > qs.count || (row_base + rows) * blocks > ds.count || x_off < 0 || x_off + nb * cols > x.count || out_off < 0 || out_off + nb * rows > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u, sg = tid >> 5, blk_lane = lane >> 1, hb = lane & 1u;
  int xq = (int)(hb * 8u), inb = (int)nb, iblocks = (int)blocks;
  const device float *xp = x.data + x_off;
  for (int ri = 0; ri < 2; ri++) {
    int64_t r0 = row0 + (int64_t)(sg * 2u) + ri;
    if (r0 >= rows) continue;
    int64_t b0_row = (row_base + r0) * blocks;
    const device uint2 *qp0 = (const device uint2 *)(qs.data + b0_row * 4) + hb;
    const device half *dp0 = (const device half *)(ds.data + b0_row);
    float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
    #define Q8_1R(T, A) if ((T) < inb) { \
      const device float *bx = xp + (int64_t)(T) * cols + bo; \
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
      float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
      A += d0 * coax_q4s_dot16_pre(p0, p1, p2, p3, half4(f0), half4(f1), half4(f2), half4(f3), xs); }
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      uint2 w0 = qp0[b * 2]; float d0 = float(dp0[b]);
      half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
      half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
      int bo = b * 32 + xq;
      Q8_1R(0,a0) Q8_1R(1,a1) Q8_1R(2,a2) Q8_1R(3,a3)
      Q8_1R(4,a4) Q8_1R(5,a5) Q8_1R(6,a6) Q8_1R(7,a7)
    }
    #undef Q8_1R
    #define Q8_WR(T, A) if ((T) < inb) { \
      float v = simd_sum(A); \
      if (lane == 0) { \
        if (cap > 0.0f) v = cap * tanh(v / cap); \
        out.data[out_off + (int64_t)(T) * rows + r0] = v; } }
    Q8_WR(0,a0) Q8_WR(1,a1) Q8_WR(2,a2) Q8_WR(3,a3)
    Q8_WR(4,a4) Q8_WR(5,a5) Q8_WR(6,a6) Q8_WR(7,a7)
    #undef Q8_WR
  }
}
COAX_DEV void coax_am_dot_q4s_mma32_nb(int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t row_base, int64_t x_off, int64_t out_off, float cap, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (cols <= 0 || (cols & 31) != 0 || rows <= 0 || nb < 1 || nb > 8 || row0 < 0 || row_base < 0 || (row_base + rows) * blocks * 4 > qs.count || (row_base + rows) * blocks > ds.count || x_off < 0 || x_off + nb * cols > x.count || out_off < 0 || out_off + nb * rows > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half *tg_X = (threadgroup half *)tg;          /* 8 * 64 half = 256 floats */
  threadgroup half *tg_W = (threadgroup half *)(tg + 256);  /* 32 * 64 half = 1024 floats */
  int iblocks = (int)blocks, inb = (int)nb;
  uint32_t sg = tid >> 5, r_loc = tid >> 2, q_loc = tid & 3u;
  int64_t r_glob = row0 + (int64_t)r_loc;
  bool r_ok = (r_glob < rows);
  int64_t r_off = r_ok ? ((row_base + r_glob) * blocks) : 0;
  const device uint32_t *qrow = qs.data + r_off * 4 + (int64_t)q_loc;
  const device half *drow = (const device half *)(ds.data + r_off);
  const device float *xp = x.data + x_off;
  simdgroup_float8x8 acc = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);
  for (int b = 0; b < iblocks; b += 2) {
    int b1 = b + 1; bool has_b1 = (b1 < iblocks);
    int p = (int)tid * 4, t = p >> 6, c = p & 63;
    if (t < inb && (b * 32 + c) < (int)cols) {
      *(threadgroup half4 *)(tg_X + p) = half4(*(const device float4 *)(xp + (int64_t)t * cols + b * 32 + c));
    } else {
      *(threadgroup half4 *)(tg_X + p) = half4(0.0h);
    }
    int woff = (int)r_loc * 64 + (int)q_loc * 4;
    if (r_ok) {
      uint32_t w0 = qrow[b * 4]; half d0 = drow[b];
      *(threadgroup half4 *)(tg_W + woff)      = (half4(as_type<uchar4>(w0 & 0x0f0f0f0fu)) - 8.0h) * d0;
      *(threadgroup half4 *)(tg_W + woff + 16) = (half4(as_type<uchar4>((w0 >> 4u) & 0x0f0f0f0fu)) - 8.0h) * d0;
      if (has_b1) {
        uint32_t w1 = qrow[b1 * 4]; half d1 = drow[b1];
        *(threadgroup half4 *)(tg_W + woff + 32) = (half4(as_type<uchar4>(w1 & 0x0f0f0f0fu)) - 8.0h) * d1;
        *(threadgroup half4 *)(tg_W + woff + 48) = (half4(as_type<uchar4>((w1 >> 4u) & 0x0f0f0f0fu)) - 8.0h) * d1;
      } else {
        *(threadgroup half4 *)(tg_W + woff + 32) = half4(0.0h);
        *(threadgroup half4 *)(tg_W + woff + 48) = half4(0.0h);
      }
    } else {
      *(threadgroup half4 *)(tg_W + woff)      = half4(0.0h);
      *(threadgroup half4 *)(tg_W + woff + 16) = half4(0.0h);
      *(threadgroup half4 *)(tg_W + woff + 32) = half4(0.0h);
      *(threadgroup half4 *)(tg_W + woff + 48) = half4(0.0h);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const threadgroup half *w_sg = tg_W + sg * 8 * 64;
    for (int k = 0; k < 64; k += 8) {
      simdgroup_half8x8 A, B;
      simdgroup_load(A, tg_X + k, 64, ulong2(0, 0), false);
      simdgroup_load(B, w_sg + k, 64, ulong2(0, 0), true);
      simdgroup_multiply_accumulate(acc, A, B, acc);
    }
    if (b + 2 < iblocks) threadgroup_barrier(mem_flags::mem_threadgroup);
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  threadgroup float *tg_out = (threadgroup float *)tg_W + sg * 64;
  simdgroup_store(acc, tg_out, 8, ulong2(0, 0), false);
  simdgroup_barrier(mem_flags::mem_threadgroup);
  uint32_t lane = tid & 31u;
  for (int e = (int)lane; e < 64; e += 32) {
    int t = e >> 3, rl = e & 7;
    int64_t rg = row0 + (int64_t)(sg * 8u) + rl;
    if (t < inb && rg < rows) {
      float v = tg_out[e];
      if (cap > 0.0f) v = cap * tanh(v / cap);
      out.data[out_off + (int64_t)t * rows + rg] = v;
    }
  }
}
COAX_DEV void coax_am_dot_q8_mma32_nb(int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t row_base, int64_t x_off, int64_t out_off, float cap, coax_v_u16 w, coax_v_f32 x, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (cols <= 0 || (cols & 31) != 0 || rows <= 0 || nb < 1 || nb > 8 || row0 < 0 || row_base < 0 || (row_base + rows) * blocks * 17 > w.count || x_off < 0 || x_off + nb * cols > x.count || out_off < 0 || out_off + nb * rows > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half *tg_X = (threadgroup half *)tg;          /* 8 * 128 half = 512 floats */
  threadgroup half *tg_W = (threadgroup half *)(tg + 512);  /* 32 * 128 half = 2048 floats */
  int iblocks = (int)blocks, inb = (int)nb;
  uint32_t sg = tid >> 5, r_loc = tid >> 2, b_loc = tid & 3u;
  int64_t r_glob = row0 + (int64_t)r_loc;
  bool r_ok = (r_glob < rows);
  const device uint16_t *wrow = w.data + (r_ok ? ((row_base + r_glob) * blocks * 17) : 0);
  const device float *xp = x.data + x_off;
  simdgroup_float8x8 acc = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);
  for (int b = 0; b < iblocks; b += 4) {
    for (int s = 0; s < 2; s++) {
      int p = ((int)tid + s * 128) * 4, t = p >> 7, c = p & 127;
      if (t < inb && (b * 32 + c) < (int)cols) {
        *(threadgroup half4 *)(tg_X + p) = half4(*(const device float4 *)(xp + (int64_t)t * cols + b * 32 + c));
      } else {
        *(threadgroup half4 *)(tg_X + p) = half4(0.0h);
      }
    }
    int bi = b + (int)b_loc;
    int woff = (int)r_loc * 128 + (int)b_loc * 32;
    if (r_ok && bi < iblocks) {
      const device uint16_t *bw = wrow + (int64_t)bi * 17;
      half d = as_type<half>(bw[0]);
      const device char *qb = (const device char *)(bw + 1);
      for (int q = 0; q < 32; q += 4) {
        char4 c4 = *(const device packed_char4 *)(qb + q);
        *(threadgroup half4 *)(tg_W + woff + q) = half4(c4) * d;
      }
    } else {
      for (int z = 0; z < 32; z += 4) *(threadgroup half4 *)(tg_W + woff + z) = half4(0.0h);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const threadgroup half *w_sg = tg_W + sg * 8 * 128;
    for (int k = 0; k < 128; k += 8) {
      simdgroup_half8x8 A, B;
      simdgroup_load(A, tg_X + k, 128, ulong2(0, 0), false);
      simdgroup_load(B, w_sg + k, 128, ulong2(0, 0), true);
      simdgroup_multiply_accumulate(acc, A, B, acc);
    }
    if (b + 4 < iblocks) threadgroup_barrier(mem_flags::mem_threadgroup);
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  threadgroup float *tg_out = (threadgroup float *)tg_W + sg * 64;
  simdgroup_store(acc, tg_out, 8, ulong2(0, 0), false);
  simdgroup_barrier(mem_flags::mem_threadgroup);
  uint32_t lane = tid & 31u;
  for (int e = (int)lane; e < 64; e += 32) {
    int t = e >> 3, rl = e & 7;
    int64_t rg = row0 + (int64_t)(sg * 8u) + rl;
    if (t < inb && rg < rows) {
      float v = tg_out[e];
      if (cap > 0.0f) v = cap * tanh(v / cap);
      out.data[out_off + (int64_t)t * rows + rg] = v;
    }
  }
}
COAX_DEV void coax_am_dspark_fold_logits_s1(int64_t item, int64_t nv, int64_t step_idx, float cap, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 latent, coax_v_f32 base, coax_v_f32 fold_b, coax_v_f32 out_m, coax_v_u32 out_i, coax_v_f32 out_s, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (item < 0 || item >= 64 || nv <= 0 || step_idx < 0 || nv * 32 > qs.count || nv * 8 > ds.count || 256 > latent.count || (step_idx + 1) * nv > base.count || nv > fold_b.count || item >= out_m.count || item >= out_i.count || item >= out_s.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t chunk = (nv + 63) / 64;
  int64_t lo = item * chunk;
  int64_t hi = lo + chunk;
  if (hi > nv) hi = nv;
  int64_t b_off = step_idx * nv;
  uint32_t sub = tid >> 3, blk = tid & 7u;
  const device float *lx = latent.data + blk * 32;
  float4 f0 = *(const device float4 *)(lx + 0), f1 = *(const device float4 *)(lx + 4);
  float4 f2 = *(const device float4 *)(lx + 16), f3 = *(const device float4 *)(lx + 20);
  float4 fs0 = (f0 + f1) + (f2 + f3); float xs0 = (fs0.x + fs0.y) + (fs0.z + fs0.w);
  half4 l0 = half4(f0), l1 = half4(f1), h0 = half4(f2), h1 = half4(f3);
  float4 g0 = *(const device float4 *)(lx + 8), g1 = *(const device float4 *)(lx + 12);
  float4 g2 = *(const device float4 *)(lx + 24), g3 = *(const device float4 *)(lx + 28);
  float4 fs1 = (g0 + g1) + (g2 + g3); float xs1 = (fs1.x + fs1.y) + (fs1.z + fs1.w);
  half4 l2 = half4(g0), l3 = half4(g1), h2 = half4(g2), h3 = half4(g3);
  float bm = -1.0e30f, bs = 0.0f;
  uint32_t bi = 0u;
  for (int64_t r = lo + (int64_t)sub; r < hi; r += 32) {
    uint4 w = ((const device uint4 *)(qs.data + r * 32))[blk];
    float d = float(((const device half *)(ds.data + r * 8))[blk]);
    half4 p0 = half4(as_type<uchar4>(w.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w.x >> 4u) & 0x0f0f0f0fu));
    half4 p2 = half4(as_type<uchar4>(w.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w.y >> 4u) & 0x0f0f0f0fu));
    half4 q0 = half4(as_type<uchar4>(w.z & 0x0f0f0f0fu)), q1 = half4(as_type<uchar4>((w.z >> 4u) & 0x0f0f0f0fu));
    half4 q2 = half4(as_type<uchar4>(w.w & 0x0f0f0f0fu)), q3 = half4(as_type<uchar4>((w.w >> 4u) & 0x0f0f0f0fu));
    float val = d * (coax_q4s_dot16_pre(p0, p1, p2, p3, l0, l1, h0, h1, xs0) + coax_q4s_dot16_pre(q0, q1, q2, q3, l2, l3, h2, h3, xs1));
    val += simd_shuffle_xor(val, 4);
    val += simd_shuffle_xor(val, 2);
    val += simd_shuffle_xor(val, 1);
    if (blk == 0u) {
      float v = base.data[b_off + r] + fold_b.data[r] + val;
      if (cap > 0.0f) v = cap * tanh(v / cap);
      if (v > bm) {
        bs = (bs > 0.0f) ? (bs * exp(bm - v) + 1.0f) : 1.0f;
        bm = v;
        bi = (uint32_t)r;
      } else {
        bs += exp(v - bm);
      }
    }
  }
  threadgroup float *pm = tg;
  threadgroup uint32_t *pi = (threadgroup uint32_t *)(tg + 32);
  threadgroup float *ps = tg + 64;
  if (blk == 0u) { pm[sub] = bm; pi[sub] = bi; ps[sub] = bs; }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0u) {
    float gm = -1.0e30f; uint32_t gi = 0u;
    for (int k = 0; k < 32; k++) {
      if (ps[k] > 0.0f && (pm[k] > gm || (pm[k] == gm && pi[k] < gi))) { gm = pm[k]; gi = pi[k]; }
    }
    float gs = 0.0f;
    for (int k = 0; k < 32; k++) {
      if (ps[k] > 0.0f) gs += ps[k] * exp(pm[k] - gm);
    }
    out_m.data[item] = gm;
    out_i.data[item] = gi;
    out_s.data[item] = gs;
  }
}
COAX_DEV void coax_am_dot_q4s_qrgkv_sdyn_nb(int64_t row0, uint32_t dims0, uint32_t dims1, uint32_t dims2, int64_t cols, int64_t nb, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 xq_v, coax_v_f32 xk_v, coax_v_f32 xv_v, coax_v_f32 parts, coax_v_f32 oq, coax_v_f32 or_, coax_v_f32 og, coax_v_f32 ok, coax_v_f32 ov, coax_v_f32 osinks, coax_v_f32 odyn, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t n_q = (int64_t)(dims0 & 65535u), n_r = (int64_t)((dims0 >> 16u) & 65535u);
  int64_t n_kv = (int64_t)(dims1 & 65535u), n_sinks = (int64_t)((dims1 >> 16u) & 65535u);
  int64_t n_dyn = (int64_t)(dims2 & 65535u), n_parts = (int64_t)((dims2 >> 16u) & 65535u);
  int64_t blocks = cols >> 5;
  int64_t e1 = n_q + n_r, e2 = e1 + n_q, e3 = e2 + n_kv, e4 = e3 + n_kv, e5 = e4 + n_sinks, e6 = e5 + n_dyn, tot = e6 + n_dyn;
  if (cols <= 0 || (cols & 31) != 0 || nb < 1 || nb > 8 || row0 < 0 || n_parts < 0 || (n_parts > 0 && 3 * nb * n_parts > parts.count) || tot * blocks * 4 > qs.count || tot * blocks > ds.count || nb * cols > xq_v.count || nb * cols > xk_v.count || nb * cols > xv_v.count || nb * n_q > oq.count || nb * n_r > or_.count || nb * n_q > og.count || nb * n_kv > ok.count || nb * n_kv > ov.count || nb * n_sinks > osinks.count || nb * 2 * n_dyn > odyn.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)(tid >> 5);
  if (r0 >= tot) return;
  const device float *xp = xq_v.data;
  device float *op = oq.data;
  int64_t orow = 0, ostr = n_q, pslot = 0;
  if (r0 < n_q) { xp = xq_v.data; op = oq.data; orow = r0; ostr = n_q; pslot = 0; }
  else if (r0 < e1) { xp = xq_v.data; op = or_.data; orow = r0 - n_q; ostr = n_r; pslot = 0; }
  else if (r0 < e2) { xp = xq_v.data; op = og.data; orow = r0 - e1; ostr = n_q; pslot = 0; }
  else if (r0 < e3) { xp = xk_v.data; op = ok.data; orow = r0 - e2; ostr = n_kv; pslot = 1; }
  else if (r0 < e4) { xp = xv_v.data; op = ov.data; orow = r0 - e3; ostr = n_kv; pslot = 2; }
  else if (r0 < e5) { xp = xq_v.data; op = osinks.data; orow = r0 - e4; ostr = n_sinks; pslot = 0; }
  else if (r0 < e6) { xp = xk_v.data; op = odyn.data; orow = r0 - e5; ostr = 2 * n_dyn; pslot = 1; }
  else { xp = xv_v.data; op = odyn.data; orow = r0 - e5; ostr = 2 * n_dyn; pslot = 2; }
  uint32_t lane = tid & 31u, blk_lane = lane >> 1, hb = lane & 1u;
  int xq = (int)(hb * 8u), inb = (int)nb, iblocks = (int)blocks;
  const device uint2 *qp0 = (const device uint2 *)(qs.data + r0 * blocks * 4) + hb;
  const device half *dp0 = (const device half *)(ds.data + r0 * blocks);
  float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
  #define QKV_1R(T, A) if ((T) < inb) { \
    const device float *bx = xp + (int64_t)(T) * cols + bo; \
    float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
    float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
    float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
    A += d0 * coax_q4s_dot16_pre(p0, p1, p2, p3, half4(f0), half4(f1), half4(f2), half4(f3), xs); }
  for (int b = (int)blk_lane; b < iblocks; b += 16) {
    uint2 w0 = qp0[b * 2]; float d0 = float(dp0[b]);
    half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu));
    half4 p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
    half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu));
    half4 p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
    int bo = b * 32 + xq;
    QKV_1R(0,a0) QKV_1R(1,a1) QKV_1R(2,a2) QKV_1R(3,a3)
    QKV_1R(4,a4) QKV_1R(5,a5) QKV_1R(6,a6) QKV_1R(7,a7)
  }
  #undef QKV_1R
  #define QKV_1W(T, A) if ((T) < inb) { \
    float inv = (n_parts > 0) ? coax_inv_rms_parts(parts, (pslot * nb + (T)) * n_parts, n_parts, cols, eps, lane) : 1.0f; \
    float v = simd_sum(A) * inv; \
    if (lane == 0) op[(int64_t)(T) * ostr + orow] = v; }
  QKV_1W(0,a0) QKV_1W(1,a1) QKV_1W(2,a2) QKV_1W(3,a3)
  QKV_1W(4,a4) QKV_1W(5,a5) QKV_1W(6,a6) QKV_1W(7,a7)
  #undef QKV_1W
}
COAX_DEV void coax_am_kv_attn_gqa_1pass_nb(int64_t h, uint32_t dims0, uint32_t dims1, uint32_t kv_info, int64_t pos0, float qk_clip, float rel_clip, float eps, coax_v_f32 tau_buf, coax_v_f32 k_raw, coax_v_f32 v_raw, coax_v_f32 kw, coax_v_f32 vw, coax_v_f32 dyn, coax_v_f32 k_hist, coax_v_f32 v_hist, coax_v_f32 q, coax_v_f32 q_scale, coax_v_f32 r, coax_v_f32 r_scale, coax_v_f32 rel_bias, coax_v_f32 sinks, coax_v_f32 gate, coax_v_u16 k_cache, coax_v_u16 v_cache, coax_v_f32 k_raw_log, coax_v_f32 v_raw_log, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int n_head = (int)(dims0 & 255u), n_head_kv = (int)((dims0 >> 8u) & 255u), head_dim = (int)((dims0 >> 16u) & 255u), n_rel = (int)((dims0 >> 24u) & 255u);
  int extent = (int)(dims1 & 65535u), is_swa = (int)((dims1 >> 16u) & 1u), swa = (int)((dims1 >> 17u) & 32767u);
  int kv_cap = (int)(kv_info & 65535u), has_qs = (int)((kv_info >> 16u) & 1u), has_rs = (int)((kv_info >> 17u) & 1u), has_dyn = (int)((kv_info >> 18u) & 1u), nb = (int)((kv_info >> 19u) & 15u);
  int64_t kv_h = h / 3;
  bool is_writer = ((h % 3) == 0);
  int64_t n_q = (int64_t)n_head * 128, n_kv = (int64_t)n_head_kv * 128, n_hr = (int64_t)n_head * n_rel;
  int64_t base = kv_h * 128;
  if (head_dim != 128 || n_rel > 32 || pos0 < 0 || nb < 1 || nb > 8 || h < 0 || h >= n_head || kv_h >= n_head_kv || nb > tau_buf.count || nb * n_kv > k_raw.count || nb * n_kv > v_raw.count || (base + 128) * 4 > kw.count || (base + 128) * 4 > vw.count || (base + 128) * 4 > k_hist.count || (base + 128) * 4 > v_hist.count || (has_dyn && nb * 2 * n_head_kv * 4 > dyn.count) || nb * n_q > q.count || nb * n_q > gate.count || nb * n_q > out.count || nb * n_head > sinks.count || nb * n_hr > r.count || (has_qs && (h + 1) * 128 > q_scale.count) || (has_rs && (h + 1) * n_rel > r_scale.count) || (int64_t)kv_cap * n_kv > k_cache.count || (int64_t)kv_cap * n_kv > v_cache.count || nb * n_kv > k_raw_log.count || nb * n_kv > v_raw_log.count || (int64_t)extent * n_rel > rel_bias.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half *tg_k = (threadgroup half *)tg;        /* 8 * 128 half = 512 floats */
  threadgroup half *tg_v = (threadgroup half *)(tg + 512);/* 8 * 128 half = 512 floats */
  threadgroup float *tg_q = tg + 1024;                    /* 1024 floats */
  threadgroup float *tg_r = tg + 2048;                    /* 256 floats */
  threadgroup float *msh  = tg + 2304;                    /* 8 floats */
  threadgroup float *lsh  = tg + 2312;                    /* 8 floats */
  threadgroup float *vsh  = tg;                           /* 1024 floats aliased onto tg_k+tg_v after kp loop! total 2320 <= 2336 */
  uint32_t lane = tid & 31u, sg = tid >> 5;
  if (sg == 0u || sg == 1u) {
    int is_v = (int)sg;
    float inv_s = has_dyn ? 0.3333333333333333f : 0.4472135954999579f;
    int64_t c0 = base + (int64_t)(lane * 4u);
    const device float *rawp = is_v ? v_raw.data : k_raw.data;
    const device float *wp = is_v ? vw.data : kw.data;
    const device float *hp = is_v ? v_hist.data : k_hist.data;
    device float *logp = is_v ? v_raw_log.data : k_raw_log.data;
    device uint16_t *cachep = is_v ? v_cache.data : k_cache.data;
    threadgroup half *tgp = is_v ? tg_v : tg_k;
    float4 hr0 = *(const device float4 *)(hp + (c0 + 0) * 4);
    float4 hr1 = *(const device float4 *)(hp + (c0 + 1) * 4);
    float4 hr2 = *(const device float4 *)(hp + (c0 + 2) * 4);
    float4 hr3 = *(const device float4 *)(hp + (c0 + 3) * 4);
    float4 cw0 = *(const device float4 *)(wp + (c0 + 0) * 4);
    float4 cw1 = *(const device float4 *)(wp + (c0 + 1) * 4);
    float4 cw2 = *(const device float4 *)(wp + (c0 + 2) * 4);
    float4 cw3 = *(const device float4 *)(wp + (c0 + 3) * 4);
    for (int t = 0; t < nb; t++) {
      int64_t p = pos0 + t;
      uint32_t s0 = (uint32_t)(p + 1) & 3u, s1 = (uint32_t)(p + 2) & 3u, s2 = (uint32_t)(p + 3) & 3u, sw = (uint32_t)p & 3u;
      float4 dv = has_dyn ? *(const device float4 *)(dyn.data + t * (2 * n_head_kv * 4) + (is_v * n_head_kv + kv_h) * 4) : float4(0.0f);
      float4 xr = *(const device float4 *)(rawp + t * n_kv + c0);
      if (is_writer) *(device float4 *)(logp + t * n_kv + c0) = xr;
      float v0, v1, v2, v3;
      { float x0 = hr0[s0], x1 = hr0[s1], x2 = hr0[s2], x3 = xr.x; hr0[sw] = x3; float a = x0*cw0.x + x1*cw0.y + x2*cw0.z + x3*cw0.w; if (has_dyn) a += dv.x*x3 + dv.y*x2 + dv.z*x1 + dv.w*x0; v0 = (x3 + a) * inv_s; }
      { float x0 = hr1[s0], x1 = hr1[s1], x2 = hr1[s2], x3 = xr.y; hr1[sw] = x3; float a = x0*cw1.x + x1*cw1.y + x2*cw1.z + x3*cw1.w; if (has_dyn) a += dv.x*x3 + dv.y*x2 + dv.z*x1 + dv.w*x0; v1 = (x3 + a) * inv_s; }
      { float x0 = hr2[s0], x1 = hr2[s1], x2 = hr2[s2], x3 = xr.z; hr2[sw] = x3; float a = x0*cw2.x + x1*cw2.y + x2*cw2.z + x3*cw2.w; if (has_dyn) a += dv.x*x3 + dv.y*x2 + dv.z*x1 + dv.w*x0; v2 = (x3 + a) * inv_s; }
      { float x0 = hr3[s0], x1 = hr3[s1], x2 = hr3[s2], x3 = xr.w; hr3[sw] = x3; float a = x0*cw3.x + x1*cw3.y + x2*cw3.z + x3*cw3.w; if (has_dyn) a += dv.x*x3 + dv.y*x2 + dv.z*x1 + dv.w*x0; v3 = (x3 + a) * inv_s; }
      float4 vv = float4(v0, v1, v2, v3);
      float tot = simd_sum(dot(vv, vv));
      half4 hv;
      if (is_v == 0) {
        float fac = qk_clip / (sqrt(tot) + 1.0e-6f);
        if (fac > 1.0f) fac = 1.0f;
        hv = half4(vv * fac);
      } else {
        float inv = rsqrt(tot * (1.0f / 128.0f) + eps);
        hv = half4(vv * inv);
      }
      int64_t sp = is_swa ? (p % kv_cap) : p;
      if (is_writer) *(device half4 *)(cachep + sp * n_kv + base + lane * 4u) = hv;
      *(threadgroup half4 *)(tgp + t * 128 + lane * 4u) = hv;
    }
  } else if (sg == 2u) {
    float4 qs4 = has_qs ? *(const device float4 *)(q_scale.data + h * 128 + lane * 4u) : float4(1.0f);
    float rs1 = (has_rs && (int)lane < n_rel) ? r_scale.data[h * n_rel + (int)lane] : 1.0f;
    for (int t = 0; t < nb; t++) {
      float qt = tau_buf.data[t];
      float4 qv = *(const device float4 *)(q.data + t * n_q + h * 128 + lane * 4u) * qs4;
      float qfac = qk_clip / (sqrt(simd_sum(dot(qv, qv))) + 1.0e-6f);
      if (qfac > 1.0f) qfac = 1.0f;
      *(threadgroup float4 *)(tg_q + t * 128 + lane * 4u) = qv * (qfac * qt);
      float rv = ((int)lane < n_rel) ? (r.data[t * n_hr + h * n_rel + (int)lane] * rs1) : 0.0f;
      float rfac = rel_clip / (sqrt(simd_sum(rv * rv)) + 1.0e-6f);
      if (rfac > 1.0f) rfac = 1.0f;
      tg_r[t * 32 + (int)lane] = rv * (rfac * qt);
    }
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int64_t st0 = (is_swa && pos0 >= swa) ? (pos0 - swa + 1) : 0;
  int64_t st1 = (is_swa && pos0 + 1 >= swa) ? (pos0 + 1 - swa + 1) : 0;
  int64_t st2 = (is_swa && pos0 + 2 >= swa) ? (pos0 + 2 - swa + 1) : 0;
  int64_t st3 = (is_swa && pos0 + 3 >= swa) ? (pos0 + 3 - swa + 1) : 0;
  int64_t st4 = (is_swa && pos0 + 4 >= swa) ? (pos0 + 4 - swa + 1) : 0;
  int64_t st5 = (is_swa && pos0 + 5 >= swa) ? (pos0 + 5 - swa + 1) : 0;
  int64_t st6 = (is_swa && pos0 + 6 >= swa) ? (pos0 + 6 - swa + 1) : 0;
  int64_t st7 = (is_swa && pos0 + 7 >= swa) ? (pos0 + 7 - swa + 1) : 0;
  float scale = 1.0f / sqrt(128.0f);
  float sk0 = sinks.data[0 * n_head + h];
  float sk1 = (nb > 1) ? sinks.data[1 * n_head + h] : 0.0f;
  float sk2 = (nb > 2) ? sinks.data[2 * n_head + h] : 0.0f;
  float sk3 = (nb > 3) ? sinks.data[3 * n_head + h] : 0.0f;
  float sk4 = (nb > 4) ? sinks.data[4 * n_head + h] : 0.0f;
  float sk5 = (nb > 5) ? sinks.data[5 * n_head + h] : 0.0f;
  float sk6 = (nb > 6) ? sinks.data[6 * n_head + h] : 0.0f;
  float sk7 = (nb > 7) ? sinks.data[7 * n_head + h] : 0.0f;
  float m0=-3.0e38f,l0=0, m1=-3.0e38f,l1=0, m2=-3.0e38f,l2=0, m3=-3.0e38f,l3=0;
  float m4=-3.0e38f,l4=0, m5=-3.0e38f,l5=0, m6=-3.0e38f,l6=0, m7=-3.0e38f,l7=0;
  float4 a0=float4(0),a1=float4(0),a2=float4(0),a3=float4(0),a4=float4(0),a5=float4(0),a6=float4(0),a7=float4(0);
  int64_t kh_off = base + (int64_t)(lane * 4u);
  int64_t kp_end = pos0 + nb - 1;
  for (int64_t kp_base = (st0 & ~7LL); kp_base <= kp_end; kp_base += 8) {
    int64_t kp = kp_base + (int64_t)sg;
    if (kp < st0 || kp > kp_end) continue;
    float4 kv, vv;
    if (kp >= pos0) {
      int tk = (int)(kp - pos0);
      kv = float4(*(threadgroup half4 *)(tg_k + tk * 128 + lane * 4u));
      vv = float4(*(threadgroup half4 *)(tg_v + tk * 128 + lane * 4u));
    } else {
      int64_t slot = (is_swa && kp >= kv_cap) ? (int64_t)((int)kp % kv_cap) : kp;
      int64_t kb = slot * n_kv + kh_off;
      kv = float4(*(const device half4 *)(k_cache.data + kb));
      vv = float4(*(const device half4 *)(v_cache.data + kb));
    }
    #define ATTN_UPD(T, ST, SK, M, L, A) if ((T) < nb && kp >= ST && kp <= pos0 + (T)) { \
      float4 qt = *(threadgroup float4 *)(tg_q + (T) * 128 + lane * 4u); \
      float rt = tg_r[(T) * 32 + (int)lane]; \
      int dist = (int)(pos0 + (T) - kp); \
      float rbv = (dist < extent && (int)lane < n_rel) ? rel_bias.data[dist * n_rel + (int)lane] : 0.0f; \
      float sc = simd_sum(dot(qt, kv)) * scale + simd_sum(rbv * rt) - SK; \
      if (sc > M) { float al = (L > 0.0f) ? exp(M - sc) : 0.0f; M = sc; L = L * al + 1.0f; A = A * al + vv; } \
      else { float wt = exp(sc - M); L += wt; A += wt * vv; } }
    ATTN_UPD(0, st0, sk0, m0, l0, a0) ATTN_UPD(1, st1, sk1, m1, l1, a1)
    ATTN_UPD(2, st2, sk2, m2, l2, a2) ATTN_UPD(3, st3, sk3, m3, l3, a3)
    ATTN_UPD(4, st4, sk4, m4, l4, a4) ATTN_UPD(5, st5, sk5, m5, l5, a5)
    ATTN_UPD(6, st6, sk6, m6, l6, a6) ATTN_UPD(7, st7, sk7, m7, l7, a7)
    #undef ATTN_UPD
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  #define ATTN_OUT(T, M, L, A) if ((T) < nb) { \
    if (lane == 0) { msh[sg] = M; lsh[sg] = L; } \
    *(threadgroup float4 *)(vsh + sg * 128 + lane * 4u) = A; \
    threadgroup_barrier(mem_flags::mem_threadgroup); \
    if (sg == 0u) { \
      float mv = (lane < 8u) ? msh[lane] : -3.0e38f; \
      float lv = (lane < 8u) ? lsh[lane] : 0.0f; \
      float gmax = simd_max(mv); if (gmax < 0.0f) gmax = 0.0f; \
      float sc = (lane < 8u && lv > 0.0f) ? exp(mv - gmax) : 0.0f; \
      float inv_gsum = 1.0f / (exp(-gmax) + simd_sum(lv * sc)); \
      float4 vtot = float4(0.0f); \
      for (int s = 0; s < 8; s++) vtot += *(threadgroup float4 *)(vsh + s * 128 + lane * 4u) * simd_shuffle(sc, s); \
      int64_t oidx = (int64_t)(T) * n_q + h * 128 + (int64_t)(lane * 4u); \
      float4 gv = *(const device float4 *)(gate.data + oidx); \
      *(device float4 *)(out.data + oidx) = (vtot * inv_gsum) / (float4(1.0f) + exp(-gv)); \
    } \
    if ((T) + 1 < nb) threadgroup_barrier(mem_flags::mem_threadgroup); }
  ATTN_OUT(0, m0, l0, a0) ATTN_OUT(1, m1, l1, a1) ATTN_OUT(2, m2, l2, a2) ATTN_OUT(3, m3, l3, a3)
  ATTN_OUT(4, m4, l4, a4) ATTN_OUT(5, m5, l5, a5) ATTN_OUT(6, m6, l6, a6) ATTN_OUT(7, m7, l7, a7)
  #undef ATTN_OUT
}
COAX_DEV void coax_am_dot_q4s_conv_nb(coax_v_u32 qs, coax_v_u16 ds, int64_t b0, int64_t b1, coax_v_f32 x, int64_t blocks, int64_t rows, int64_t nb, int64_t add_res, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 raw_log, coax_v_f32 out, int64_t r0, int64_t r1, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t cols = blocks * 32;
  if (blocks < 0 || rows <= 0 || nb < 1 || nb > 8 || b0 < 0 || (b0 + blocks) * 4 > qs.count || b0 + blocks > ds.count || b1 < 0 || (b1 + blocks) * 4 > qs.count || b1 + blocks > ds.count || nb * cols > x.count || r0 < 0 || r1 < 0 || r0 >= rows || r1 >= rows || nb * rows > out.count || nb * rows > raw_log.count || rows * 4 > cw.count || rows * 3 > hist.count || (add_res != 0 && nb * rows > res.count)) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u, blk_lane = lane >> 1, hb = lane & 1u;
  int xq = (int)(hb * 8u), inb = (int)nb, iblocks = (int)blocks;
  const device uint2 *qp0 = (const device uint2 *)(qs.data + b0 * 4) + hb;
  const device half *dp0 = (const device half *)(ds.data + b0);
  float u0 = 0, u1 = 0, u2 = 0, v0 = 0, v1 = 0, v2 = 0;
  float4 c0 = float4(0), c1 = float4(0);
  if (lane == 0) {
    u0 = hist.data[r0 * 3]; u1 = hist.data[r0 * 3 + 1]; u2 = hist.data[r0 * 3 + 2];
    c0 = *(const device float4 *)(cw.data + r0 * 4);
    if (r1 != r0) {
      v0 = hist.data[r1 * 3]; v1 = hist.data[r1 * 3 + 1]; v2 = hist.data[r1 * 3 + 2];
      c1 = *(const device float4 *)(cw.data + r1 * 4);
    }
  }
  if (r1 == r0) {
    float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
    #define CNV_1R(T, A) if ((T) < inb) { \
      const device float *bx = x.data + (int64_t)(T) * cols + bo; \
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
      float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
      A += d0 * coax_q4s_dot16_pre(p0, p1, p2, p3, half4(f0), half4(f1), half4(f2), half4(f3), xs); }
    for (int b = (int)blk_lane; b < iblocks; b += 16) {
      uint2 w0 = qp0[b * 2]; float d0 = float(dp0[b]);
      half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu));
      half4 p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
      half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu));
      half4 p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
      int bo = b * 32 + xq;
      CNV_1R(0,a0) CNV_1R(1,a1) CNV_1R(2,a2) CNV_1R(3,a3)
      CNV_1R(4,a4) CNV_1R(5,a5) CNV_1R(6,a6) CNV_1R(7,a7)
    }
    #undef CNV_1R
    #define CNV_1W(T, A) if ((T) < inb) { \
      float D0 = simd_sum(A); \
      if (lane == 0) { \
        int64_t tr = (int64_t)(T) * rows; \
        raw_log.data[tr + r0] = D0; \
        float y0 = (D0 + u0*c0.x + u1*c0.y + u2*c0.z + D0*c0.w) * 0.4472135954999579f; \
        out.data[tr + r0] = add_res ? (res.data[tr + r0] + y0) : y0; \
        u0 = u1; u1 = u2; u2 = D0; } }
    CNV_1W(0,a0) CNV_1W(1,a1) CNV_1W(2,a2) CNV_1W(3,a3)
    CNV_1W(4,a4) CNV_1W(5,a5) CNV_1W(6,a6) CNV_1W(7,a7)
    #undef CNV_1W
    return;
  }
  const device uint2 *qp1 = (const device uint2 *)(qs.data + b1 * 4) + hb;
  const device half *dp1 = (const device half *)(ds.data + b1);
  float a0_0=0,a0_1=0,a0_2=0,a0_3=0,a0_4=0,a0_5=0,a0_6=0,a0_7=0;
  float a1_0=0,a1_1=0,a1_2=0,a1_3=0,a1_4=0,a1_5=0,a1_6=0,a1_7=0;
  #define CNV_NB(T, A0, A1) if ((T) < inb) { \
    const device float *bx = x.data + (int64_t)(T) * cols + bo; \
    float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
    float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
    float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
    half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3); \
    A0 += d0 * coax_q4s_dot16_pre(p0, p1, p2, p3, xl0, xl1, xh0, xh1, xs); \
    A1 += d1 * coax_q4s_dot16_pre(q0, q1, q2, q3, xl0, xl1, xh0, xh1, xs); }
  for (int b = (int)blk_lane; b < iblocks; b += 16) {
    uint2 w0 = qp0[b * 2], w1 = qp1[b * 2];
    float d0 = float(dp0[b]), d1 = float(dp1[b]);
    half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
    half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
    half4 q0 = half4(as_type<uchar4>(w1.x & 0x0f0f0f0fu)), q1 = half4(as_type<uchar4>((w1.x >> 4u) & 0x0f0f0f0fu));
    half4 q2 = half4(as_type<uchar4>(w1.y & 0x0f0f0f0fu)), q3 = half4(as_type<uchar4>((w1.y >> 4u) & 0x0f0f0f0fu));
    int bo = b * 32 + xq;
    CNV_NB(0,a0_0,a1_0) CNV_NB(1,a0_1,a1_1) CNV_NB(2,a0_2,a1_2) CNV_NB(3,a0_3,a1_3)
    CNV_NB(4,a0_4,a1_4) CNV_NB(5,a0_5,a1_5) CNV_NB(6,a0_6,a1_6) CNV_NB(7,a0_7,a1_7)
  }
  #undef CNV_NB
  #define CNV_WR(T, A0, A1) if ((T) < inb) { \
    float D0 = simd_sum(A0), D1 = simd_sum(A1); \
    if (lane == 0) { \
      int64_t tr = (int64_t)(T) * rows; \
      raw_log.data[tr + r0] = D0; \
      float y0 = (D0 + u0*c0.x + u1*c0.y + u2*c0.z + D0*c0.w) * 0.4472135954999579f; \
      out.data[tr + r0] = add_res ? (res.data[tr + r0] + y0) : y0; \
      u0 = u1; u1 = u2; u2 = D0; \
      raw_log.data[tr + r1] = D1; \
      float y1 = (D1 + v0*c1.x + v1*c1.y + v2*c1.z + D1*c1.w) * 0.4472135954999579f; \
      out.data[tr + r1] = add_res ? (res.data[tr + r1] + y1) : y1; \
      v0 = v1; v1 = v2; v2 = D1; } }
  CNV_WR(0,a0_0,a1_0) CNV_WR(1,a0_1,a1_1) CNV_WR(2,a0_2,a1_2) CNV_WR(3,a0_3,a1_3)
  CNV_WR(4,a0_4,a1_4) CNV_WR(5,a0_5,a1_5) CNV_WR(6,a0_6,a1_6) CNV_WR(7,a0_7,a1_7)
  #undef CNV_WR
}
COAX_DEV void coax_am_moe_plan_nb(int64_t n_exp, int64_t n_used, int64_t nb, int64_t gpe, int64_t has_bias, float scale, coax_v_f32 logits, coax_v_f32 bias, coax_v_u32 ids, coax_v_f32 wts, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (n_exp < 1 || n_exp > 128 || n_used < 1 || n_used > 32 || nb < 1 || nb > 8 || gpe < 0 || nb * n_exp > logits.count || (has_bias != 0 && n_exp > bias.count) || nb * n_used > ids.count || nb * n_used > wts.count || nb * n_used > u_eids.count || nb * n_used > u_mask.count || nb * n_used * 8 > u_slot.count || u_cnt.count < 4) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup uint32_t *tg_ids = (threadgroup uint32_t *)tg;         /* 8 * 8 = 64 */
  threadgroup uint32_t *tg_emask = tg_ids + 64;                      /* 128 */
  threadgroup uint8_t *tg_eslot = (threadgroup uint8_t *)(tg_ids + 192); /* 128 * 8 = 1024B = 256 u32 -> total 448 <= 512 */
  uint32_t lane = tid & 31u, sg = tid >> 5;
  if (tid < 128u) tg_emask[tid] = 0u;
  if ((int)sg < (int)nb) {
    coax_router_pick_slot_noinline((int)n_exp, (int)n_used, (int)has_bias, scale, logits.data + (int64_t)sg * n_exp, bias.data, 0, 1, ids.data + (int64_t)sg * n_used, wts.data + (int64_t)sg * n_used, lane);
    if ((int)lane < (int)n_used) tg_ids[sg * 8u + lane] = ids.data[(int64_t)sg * n_used + (int64_t)lane];
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0u) {
    uint32_t active[4] = {0u, 0u, 0u, 0u};
    for (int t = 0; t < (int)nb; t++) {
      uint32_t bit = 1u << (uint32_t)t;
      for (int s = 0; s < (int)n_used; s++) {
        uint32_t eid = tg_ids[t * 8 + s];
        active[eid >> 5] |= (1u << (eid & 31u));
        tg_emask[eid] |= bit;
        tg_eslot[eid * 8 + t] = (uint8_t)s;
      }
    }
    int nu = 0;
    for (int w = 0; w < 4; w++) {
      uint32_t m = active[w];
      while (m) {
        int b = (int)ctz(m); m &= (m - 1u);
        uint32_t eid = (uint32_t)(w * 32 + b);
        uint32_t tm = tg_emask[eid];
        u_eids.data[nu] = eid;
        u_mask.data[nu] = tm;
        uint32_t cm = tm;
        while (cm) { int t = (int)ctz(cm); cm &= (cm - 1u); u_slot.data[nu * 8 + t] = (uint32_t)tg_eslot[eid * 8 + t]; }
        nu++;
      }
    }
    u_cnt.data[0] = (uint32_t)nu;
    u_cnt.data[1] = (uint32_t)(nu * (int)gpe);
    u_cnt.data[2] = 1u;
    u_cnt.data[3] = 1u;
  }
}
COAX_DEV void coax_am_moe_gu_iq4_nb(int64_t u, int64_t row0, int64_t n_ff, int64_t cols, int64_t nb, int64_t n_used, float beta, float lb, int64_t n_parts, float eps, coax_v_u16 w, coax_v_f32 x, coax_v_f32 parts, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_parts < 0 || (n_parts > 0 && nb * n_parts > parts.count) || nb * cols > x.count || nb * n_used * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  if (u < 0 || u >= (int64_t)u_cnt.data[0]) return;
  if (u >= u_eids.count || u >= u_mask.count || (u + 1) * 8 > u_slot.count) { coax_trap_(line, coax_err); return; }
  int64_t blocks = cols >> 5, rw = blocks * 9, eb = (int64_t)u_eids.data[u] * 2 * n_ff * rw;
  if (eb + 2 * n_ff * rw > w.count) { coax_trap_(line, coax_err); return; }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  uint32_t lane = tid & 31u, mask = u_mask.data[u];
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), k0 = hb * 4, xq = hb * 8, iblocks = (int)blocks;
  const device uint16_t *wg0 = w.data + eb + r0 * rw, *wu0 = wg0 + n_ff * rw;
  const device uint16_t *wg1 = w.data + eb + r1 * rw, *wu1 = wg1 + n_ff * rw;
  #define GU_WR_ONE(T, G0, U0, G1, U1) { \
    float inv16 = ((n_parts > 0) ? coax_inv_rms_parts(parts, (int64_t)(T) * n_parts, n_parts, cols, eps, lane) : 1.0f) * 16.0f; \
    float g0 = simd_sum(G0) * inv16, u0 = simd_sum(U0) * inv16; \
    float g1 = simd_sum(G1) * inv16, u1 = simd_sum(U1) * inv16; \
    if (lane == 0) { \
      float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0)); \
      float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1)); \
      if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); } \
      int64_t ob = ((int64_t)(T) * n_used + (int64_t)u_slot.data[u * 8 + (T)]) * n_ff; \
      out.data[ob + r0] = a0 * u0; if (r1 != r0) out.data[ob + r1] = a1 * u1; } }
  if (popcount(mask) == 1u) {
    int t0 = (int)ctz(mask);
    const device float *xp = x.data + (int64_t)t0 * cols + xq;
    float ag0 = 0, au0 = 0, ag1 = 0, au1 = 0;
    for (int b = blk_lane; b < iblocks; b += 16) {
      const device float *bx = xp + b * 32;
      half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
      half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
      half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
      half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
      const device uint16_t *bg0 = wg0 + b * 9, *bu0 = wu0 + b * 9, *bg1 = wg1 + b * 9, *bu1 = wu1 + b * 9;
      ag0 += float(as_type<half>(bg0[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bg0 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      au0 += float(as_type<half>(bu0[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bu0 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      ag1 += float(as_type<half>(bg1[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bg1 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      au1 += float(as_type<half>(bu1[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bu1 + 1 + k0), x0, x1, x2, x3, x4, x5, x6, x7, tlut);
    }
    GU_WR_ONE(t0, ag0, au0, ag1, au1)
  } else {
    uint32_t m = mask;
    while (m) {
      int t0 = (int)ctz(m); m &= (m - 1u);
      int t1 = m ? (int)ctz(m) : -1; if (m) m &= (m - 1u);
      const device float *xp0 = x.data + (int64_t)t0 * cols + xq;
      const device float *xp1 = (t1 >= 0) ? (x.data + (int64_t)t1 * cols + xq) : nullptr;
      float ag0=0,au0=0,ag1=0,au1=0, bg0a=0,bu0a=0,bg1a=0,bu1a=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        int bo = b * 32;
        const device float *bx0 = xp0 + bo;
        half4 xl0 = half4(*(const device float4 *)bx0), xl1 = half4(*(const device float4 *)(bx0 + 4));
        half4 xh0 = half4(*(const device float4 *)(bx0 + 16)), xh1 = half4(*(const device float4 *)(bx0 + 20));
        half2 a0(xl0.x,xh0.x),a1(xl0.y,xh0.y),a2(xl0.z,xh0.z),a3(xl0.w,xh0.w),a4(xl1.x,xh1.x),a5(xl1.y,xh1.y),a6(xl1.z,xh1.z),a7(xl1.w,xh1.w);
        half2 c0(0),c1(0),c2(0),c3(0),c4(0),c5(0),c6(0),c7(0);
        if (xp1) {
          const device float *bx1 = xp1 + bo;
          half4 yl0 = half4(*(const device float4 *)bx1), yl1 = half4(*(const device float4 *)(bx1 + 4));
          half4 yh0 = half4(*(const device float4 *)(bx1 + 16)), yh1 = half4(*(const device float4 *)(bx1 + 20));
          c0=half2(yl0.x,yh0.x);c1=half2(yl0.y,yh0.y);c2=half2(yl0.z,yh0.z);c3=half2(yl0.w,yh0.w);
          c4=half2(yl1.x,yh1.x);c5=half2(yl1.y,yh1.y);c6=half2(yl1.z,yh1.z);c7=half2(yl1.w,yh1.w);
        }
        #define IQ4_PAIR(WP, ACCA, ACCB) { \
          float ds_ = float(as_type<half>((WP)[0])); \
          packed_ushort4 p_ = *(const device packed_ushort4 *)((WP) + 1 + k0); \
          half2 w0=tlut[p_.x&255u],w1=tlut[p_.x>>8u],w2=tlut[p_.y&255u],w3=tlut[p_.y>>8u]; \
          half2 w4=tlut[p_.z&255u],w5=tlut[p_.z>>8u],w6=tlut[p_.w&255u],w7=tlut[p_.w>>8u]; \
          half2 s0 = w0*a0+w1*a1+w2*a2+w3*a3; s0 += w4*a4+w5*a5+w6*a6+w7*a7; ACCA += ds_ * float(s0.x + s0.y); \
          if (xp1) { half2 s1 = w0*c0+w1*c1+w2*c2+w3*c3; s1 += w4*c4+w5*c5+w6*c6+w7*c7; ACCB += ds_ * float(s1.x + s1.y); } }
        IQ4_PAIR(wg0 + b * 9, ag0, bg0a)
        IQ4_PAIR(wu0 + b * 9, au0, bu0a)
        IQ4_PAIR(wg1 + b * 9, ag1, bg1a)
        IQ4_PAIR(wu1 + b * 9, au1, bu1a)
        #undef IQ4_PAIR
      }
      GU_WR_ONE(t0, ag0, au0, ag1, au1)
      if (t1 >= 0) GU_WR_ONE(t1, bg0a, bu0a, bg1a, bu1a)
    }
  }
  #undef GU_WR_ONE
}
COAX_DEV void coax_am_moe_gu_q4s_nb(int64_t u, int64_t row0, int64_t n_ff, int64_t cols, int64_t nb, int64_t n_used, float beta, float lb, int64_t n_parts, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 parts, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 out, uint32_t tid, int32_t line, COAX_ERR) {
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_parts < 0 || (n_parts > 0 && nb * n_parts > parts.count) || nb * cols > x.count || nb * n_used * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  if (u < 0 || u >= (int64_t)u_cnt.data[0]) return;
  if (u >= u_eids.count || u >= u_mask.count || (u + 1) * 8 > u_slot.count) { coax_trap_(line, coax_err); return; }
  int64_t blocks = cols >> 5, eb = (int64_t)u_eids.data[u] * 2 * n_ff * blocks;
  if ((eb + 2 * n_ff * blocks) * 4 > qs.count || eb + 2 * n_ff * blocks > ds.count) { coax_trap_(line, coax_err); return; }
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  uint32_t lane = tid & 31u, mask = u_mask.data[u];
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  int64_t g0b = eb + r0 * blocks, u0b = g0b + n_ff * blocks, g1b = eb + r1 * blocks, u1b = g1b + n_ff * blocks;
  const device uint2 *qg0 = (const device uint2 *)(qs.data + g0b * 4) + hb, *qu0 = (const device uint2 *)(qs.data + u0b * 4) + hb;
  const device uint2 *qg1 = (const device uint2 *)(qs.data + g1b * 4) + hb, *qu1 = (const device uint2 *)(qs.data + u1b * 4) + hb;
  const device half *dg0 = (const device half *)(ds.data + g0b), *du0 = (const device half *)(ds.data + u0b);
  const device half *dg1 = (const device half *)(ds.data + g1b), *du1 = (const device half *)(ds.data + u1b);
  #define GUQ_WR(T, G0, U0, G1, U1) { \
    float inv = (n_parts > 0) ? coax_inv_rms_parts(parts, (int64_t)(T) * n_parts, n_parts, cols, eps, lane) : 1.0f; \
    float g0 = simd_sum(G0) * inv, u0 = simd_sum(U0) * inv, g1 = simd_sum(G1) * inv, u1 = simd_sum(U1) * inv; \
    if (lane == 0) { \
      float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0)); \
      float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1)); \
      if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); } \
      int64_t ob = ((int64_t)(T) * n_used + (int64_t)u_slot.data[u * 8 + (T)]) * n_ff; \
      out.data[ob + r0] = a0 * u0; if (r1 != r0) out.data[ob + r1] = a1 * u1; } }
  while (mask) {
    int t0 = (int)ctz(mask); mask &= (mask - 1u);
    if (!mask) {
      const device float *xp0 = x.data + (int64_t)t0 * cols + xq;
      float ag0=0,au0=0,ag1=0,au1=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        const device float *bx0 = xp0 + b * 32;
        float4 f0 = *(const device float4 *)bx0, f1 = *(const device float4 *)(bx0 + 4), f2 = *(const device float4 *)(bx0 + 16), f3 = *(const device float4 *)(bx0 + 20);
        float4 fs0 = (f0 + f1) + (f2 + f3); float xs0 = (fs0.x + fs0.y) + (fs0.z + fs0.w);
        half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
        ag0 += float(dg0[b]) * coax_q4s_dot16_u2(qg0[b * 2], xl0, xl1, xh0, xh1, xs0);
        au0 += float(du0[b]) * coax_q4s_dot16_u2(qu0[b * 2], xl0, xl1, xh0, xh1, xs0);
        ag1 += float(dg1[b]) * coax_q4s_dot16_u2(qg1[b * 2], xl0, xl1, xh0, xh1, xs0);
        au1 += float(du1[b]) * coax_q4s_dot16_u2(qu1[b * 2], xl0, xl1, xh0, xh1, xs0);
      }
      GUQ_WR(t0, ag0, au0, ag1, au1)
    } else {
      int t1 = (int)ctz(mask); mask &= (mask - 1u);
      const device float *xp0 = x.data + (int64_t)t0 * cols + xq;
      const device float *xp1 = x.data + (int64_t)t1 * cols + xq;
      float ag0=0,au0=0,ag1=0,au1=0, bg0a=0,bu0a=0,bg1a=0,bu1a=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        int bo = b * 32;
        const device float *bx0 = xp0 + bo;
        float4 f0 = *(const device float4 *)bx0, f1 = *(const device float4 *)(bx0 + 4), f2 = *(const device float4 *)(bx0 + 16), f3 = *(const device float4 *)(bx0 + 20);
        float4 fs0 = (f0 + f1) + (f2 + f3); float xs0 = (fs0.x + fs0.y) + (fs0.z + fs0.w);
        half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
        const device float *bx1 = xp1 + bo;
        float4 g0 = *(const device float4 *)bx1, g1 = *(const device float4 *)(bx1 + 4), g2 = *(const device float4 *)(bx1 + 16), g3 = *(const device float4 *)(bx1 + 20);
        float4 fs1 = (g0 + g1) + (g2 + g3); float xs1 = (fs1.x + fs1.y) + (fs1.z + fs1.w);
        half4 yl0 = half4(g0), yl1 = half4(g1), yh0 = half4(g2), yh1 = half4(g3);
        #define Q4S_PAIR(QP, DP, ACCA, ACCB) { \
          uint2 w_ = (QP)[b * 2]; float d_ = float((DP)[b]); \
          half4 p0 = half4(as_type<uchar4>(w_.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w_.x >> 4u) & 0x0f0f0f0fu)); \
          half4 p2 = half4(as_type<uchar4>(w_.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w_.y >> 4u) & 0x0f0f0f0fu)); \
          ACCA += d_ * coax_q4s_dot16_pre(p0, p1, p2, p3, xl0, xl1, xh0, xh1, xs0); \
          ACCB += d_ * coax_q4s_dot16_pre(p0, p1, p2, p3, yl0, yl1, yh0, yh1, xs1); }
        Q4S_PAIR(qg0, dg0, ag0, bg0a) Q4S_PAIR(qu0, du0, au0, bu0a)
        Q4S_PAIR(qg1, dg1, ag1, bg1a) Q4S_PAIR(qu1, du1, au1, bu1a)
        #undef Q4S_PAIR
      }
      GUQ_WR(t0, ag0, au0, ag1, au1)
      GUQ_WR(t1, bg0a, bu0a, bg1a, bu1a)
    }
  }
  #undef GUQ_WR
}
COAX_DEV void coax_am_moe_gu_iq4s_nb(int64_t u, int64_t row0, int64_t n_ff, int64_t cols, int64_t nb, int64_t n_used, float beta, float lb, int64_t n_parts, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 parts, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || n_ff <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || n_parts < 0 || (n_parts > 0 && nb * n_parts > parts.count) || nb * cols > x.count || nb * n_used * n_ff > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  if (u < 0 || u >= (int64_t)u_cnt.data[0]) return;
  if (u >= u_eids.count || u >= u_mask.count || (u + 1) * 8 > u_slot.count) { coax_trap_(line, coax_err); return; }
  int64_t blocks = cols >> 5, eb = (int64_t)u_eids.data[u] * 2 * n_ff * blocks;
  if ((eb + 2 * n_ff * blocks) * 4 > qs.count || eb + 2 * n_ff * blocks > ds.count) { coax_trap_(line, coax_err); return; }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  int64_t r0 = row0 + (int64_t)((tid >> 5) * 2u);
  if (r0 >= n_ff) return;
  int64_t r1 = (r0 + 1 < n_ff) ? (r0 + 1) : r0;
  uint32_t lane = tid & 31u, mask = u_mask.data[u];
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  int64_t g0b = eb + r0 * blocks, u0b = g0b + n_ff * blocks, g1b = eb + r1 * blocks, u1b = g1b + n_ff * blocks;
  const device uint2 *qg0 = (const device uint2 *)(qs.data + g0b * 4) + hb, *qu0 = (const device uint2 *)(qs.data + u0b * 4) + hb;
  const device uint2 *qg1 = (const device uint2 *)(qs.data + g1b * 4) + hb, *qu1 = (const device uint2 *)(qs.data + u1b * 4) + hb;
  const device half *dg0 = (const device half *)(ds.data + g0b), *du0 = (const device half *)(ds.data + u0b);
  const device half *dg1 = (const device half *)(ds.data + g1b), *du1 = (const device half *)(ds.data + u1b);
  #define GUIS_WR(T, G0, U0, G1, U1) { \
    float inv16 = ((n_parts > 0) ? coax_inv_rms_parts(parts, (int64_t)(T) * n_parts, n_parts, cols, eps, lane) : 1.0f) * 16.0f; \
    float g0 = simd_sum(G0) * inv16, u0 = simd_sum(U0) * inv16, g1 = simd_sum(G1) * inv16, u1 = simd_sum(U1) * inv16; \
    if (lane == 0) { \
      float a0 = beta * tanh(g0 / beta) / (1.0f + exp(-g0)); \
      float a1 = beta * tanh(g1 / beta) / (1.0f + exp(-g1)); \
      if (lb > 0.0f) { u0 = lb * tanh(u0 / lb); u1 = lb * tanh(u1 / lb); } \
      int64_t ob = ((int64_t)(T) * n_used + (int64_t)u_slot.data[u * 8 + (T)]) * n_ff; \
      out.data[ob + r0] = a0 * u0; if (r1 != r0) out.data[ob + r1] = a1 * u1; } }
  while (mask) {
    int t0 = (int)ctz(mask); mask &= (mask - 1u);
    if (!mask) {
      const device float *xp = x.data + (int64_t)t0 * cols + xq;
      float ag0 = 0, au0 = 0, ag1 = 0, au1 = 0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        const device float *bx = xp + b * 32;
        half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
        half2 x0(xl0.x, xh0.x), x1(xl0.y, xh0.y), x2(xl0.z, xh0.z), x3(xl0.w, xh0.w);
        half2 x4(xl1.x, xh1.x), x5(xl1.y, xh1.y), x6(xl1.z, xh1.z), x7(xl1.w, xh1.w);
        ag0 += float(dg0[b]) * coax_iq4s_dot16_u2(qg0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
        au0 += float(du0[b]) * coax_iq4s_dot16_u2(qu0[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
        ag1 += float(dg1[b]) * coax_iq4s_dot16_u2(qg1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
        au1 += float(du1[b]) * coax_iq4s_dot16_u2(qu1[b * 2], x0, x1, x2, x3, x4, x5, x6, x7, tlut);
      }
      GUIS_WR(t0, ag0, au0, ag1, au1)
    } else {
      int t1 = (int)ctz(mask); mask &= (mask - 1u);
      const device float *xp0 = x.data + (int64_t)t0 * cols + xq;
      const device float *xp1 = x.data + (int64_t)t1 * cols + xq;
      float ag0=0,au0=0,ag1=0,au1=0, bg0a=0,bu0a=0,bg1a=0,bu1a=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        int bo = b * 32;
        const device float *bx0 = xp0 + bo;
        half4 xl0 = half4(*(const device float4 *)bx0), xl1 = half4(*(const device float4 *)(bx0 + 4));
        half4 xh0 = half4(*(const device float4 *)(bx0 + 16)), xh1 = half4(*(const device float4 *)(bx0 + 20));
        half2 a0(xl0.x,xh0.x),a1(xl0.y,xh0.y),a2(xl0.z,xh0.z),a3(xl0.w,xh0.w),a4(xl1.x,xh1.x),a5(xl1.y,xh1.y),a6(xl1.z,xh1.z),a7(xl1.w,xh1.w);
        const device float *bx1 = xp1 + bo;
        half4 yl0 = half4(*(const device float4 *)bx1), yl1 = half4(*(const device float4 *)(bx1 + 4));
        half4 yh0 = half4(*(const device float4 *)(bx1 + 16)), yh1 = half4(*(const device float4 *)(bx1 + 20));
        half2 c0(yl0.x,yh0.x),c1(yl0.y,yh0.y),c2(yl0.z,yh0.z),c3(yl0.w,yh0.w),c4(yl1.x,yh1.x),c5(yl1.y,yh1.y),c6(yl1.z,yh1.z),c7(yl1.w,yh1.w);
        #define IQ4S_PAIR(QP, DP, ACCA, ACCB) { \
          float ds_ = float((DP)[b]); uint2 p_ = (QP)[b * 2]; \
          uchar4 b0 = as_type<uchar4>(p_.x), b1 = as_type<uchar4>(p_.y); \
          half2 w0=tlut[b0.x],w1=tlut[b0.y],w2=tlut[b0.z],w3=tlut[b0.w]; \
          half2 w4=tlut[b1.x],w5=tlut[b1.y],w6=tlut[b1.z],w7=tlut[b1.w]; \
          half2 s0 = w0*a0+w1*a1+w2*a2+w3*a3; s0 += w4*a4+w5*a5+w6*a6+w7*a7; ACCA += ds_ * float(s0.x + s0.y); \
          half2 s1 = w0*c0+w1*c1+w2*c2+w3*c3; s1 += w4*c4+w5*c5+w6*c6+w7*c7; ACCB += ds_ * float(s1.x + s1.y); }
        IQ4S_PAIR(qg0, dg0, ag0, bg0a) IQ4S_PAIR(qu0, du0, au0, bu0a)
        IQ4S_PAIR(qg1, dg1, ag1, bg1a) IQ4S_PAIR(qu1, du1, au1, bu1a)
        #undef IQ4S_PAIR
      }
      GUIS_WR(t0, ag0, au0, ag1, au1)
      GUIS_WR(t1, bg0a, bu0a, bg1a, bu1a)
    }
  }
  #undef GUIS_WR
}
COAX_DEV void coax_am_moe_down_iq4_nb(int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t n_used, coax_v_u16 w, coax_v_f32 x, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 raw_log, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5, rw = blocks * 9, ew = rows * rw;
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || nb * n_used * cols > x.count || nb * n_used > wts.count || nb * rows > res.count || nb * rows > raw_log.count || nb * rows > out.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int nu = (int)u_cnt.data[0];
  if (nu < 0 || (int64_t)nu > u_eids.count || (int64_t)nu > u_mask.count || (int64_t)nu * 8 > u_slot.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  int64_t r0 = row0 + (int64_t)(tid >> 5);
  if (r0 >= rows) return;
  uint32_t lane = tid & 31u;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), k0 = hb * 4, xq = hb * 8, iblocks = (int)blocks, inb = (int)nb;
  float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
  for (int u = 0; u < nu; u++) {
    uint32_t m = u_mask.data[u];
    int64_t eb = (int64_t)u_eids.data[u] * ew;
    if (eb + ew > w.count) { coax_trap_(line, coax_err); return; }
    const device uint16_t *wb = w.data + eb + r0 * rw;
    if ((m & (m - 1u)) == 0u) {
      int t = (int)ctz(m);
      int sl = (int)u_slot.data[u * 8 + t];
      float wt = 16.0f * wts.data[(int64_t)t * n_used + sl];
      const device float *xp = x.data + ((int64_t)t * n_used + sl) * cols + xq;
      float s0 = 0.0f;
      for (int b = blk_lane; b < iblocks; b += 16) {
        const device float *bx = xp + b * 32;
        half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
        half2 x0(xl0.x,xh0.x),x1(xl0.y,xh0.y),x2(xl0.z,xh0.z),x3(xl0.w,xh0.w),x4(xl1.x,xh1.x),x5(xl1.y,xh1.y),x6(xl1.z,xh1.z),x7(xl1.w,xh1.w);
        const device uint16_t *bw = wb + b * 9;
        s0 += float(as_type<half>(bw[0])) * coax_iq4_dot16_tg(*(const device packed_ushort4 *)(bw + 1 + k0), x0,x1,x2,x3,x4,x5,x6,x7, tlut);
      }
      if (t==0) a0=fma(wt,s0,a0); else if (t==1) a1=fma(wt,s0,a1); else if (t==2) a2=fma(wt,s0,a2); else if (t==3) a3=fma(wt,s0,a3);
      else if (t==4) a4=fma(wt,s0,a4); else if (t==5) a5=fma(wt,s0,a5); else if (t==6) a6=fma(wt,s0,a6); else a7=fma(wt,s0,a7);
    } else {
      float s0=0,s1=0,s2=0,s3=0,s4=0,s5=0,s6=0,s7=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        const device uint16_t *bw = wb + b * 9;
        float d0 = float(as_type<half>(bw[0]));
        packed_ushort4 p_ = *(const device packed_ushort4 *)(bw + 1 + k0);
        half2 w0=tlut[p_.x&255u],w1=tlut[p_.x>>8u],w2=tlut[p_.y&255u],w3=tlut[p_.y>>8u];
        half2 w4=tlut[p_.z&255u],w5=tlut[p_.z>>8u],w6=tlut[p_.w&255u],w7=tlut[p_.w>>8u];
        int bo = b * 32 + xq;
        #define MDI_S(T, S) if ((T) < inb && (m & (1u << (T)))) { \
          const device float *bx = x.data + ((int64_t)(T) * n_used + (int64_t)u_slot.data[u * 8 + (T)]) * cols + bo; \
          half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4)); \
          half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20)); \
          half2 x0(xl0.x,xh0.x),x1(xl0.y,xh0.y),x2(xl0.z,xh0.z),x3(xl0.w,xh0.w),x4(xl1.x,xh1.x),x5(xl1.y,xh1.y),x6(xl1.z,xh1.z),x7(xl1.w,xh1.w); \
          half2 dp = w0*x0+w1*x1+w2*x2+w3*x3; dp += w4*x4+w5*x5+w6*x6+w7*x7; S += d0 * float(dp.x + dp.y); }
        MDI_S(0,s0) MDI_S(1,s1) MDI_S(2,s2) MDI_S(3,s3) MDI_S(4,s4) MDI_S(5,s5) MDI_S(6,s6) MDI_S(7,s7)
        #undef MDI_S
      }
      #define MDI_A(T, A, S) if ((T) < inb && (m & (1u << (T)))) A = fma(16.0f * wts.data[(int64_t)(T) * n_used + u_slot.data[u * 8 + (T)]], (S), A);
      MDI_A(0,a0,s0) MDI_A(1,a1,s1) MDI_A(2,a2,s2) MDI_A(3,a3,s3) MDI_A(4,a4,s4) MDI_A(5,a5,s5) MDI_A(6,a6,s6) MDI_A(7,a7,s7)
      #undef MDI_A
    }
  }
  float u0 = 0, u1 = 0, u2 = 0; float4 c0 = float4(0);
  if (lane == 0) { u0 = hist.data[r0 * 3]; u1 = hist.data[r0 * 3 + 1]; u2 = hist.data[r0 * 3 + 2]; c0 = *(const device float4 *)(cw.data + r0 * 4); }
  #define MD_1W(T, A) if ((T) < inb) { \
    float D0 = simd_sum(A); \
    if (lane == 0) { \
      int64_t tr = (int64_t)(T) * rows; \
      raw_log.data[tr + r0] = D0; \
      out.data[tr + r0] = res.data[tr + r0] + (D0 + u0*c0.x + u1*c0.y + u2*c0.z + D0*c0.w) * 0.4472135954999579f; \
      u0 = u1; u1 = u2; u2 = D0; } }
  MD_1W(0,a0) MD_1W(1,a1) MD_1W(2,a2) MD_1W(3,a3) MD_1W(4,a4) MD_1W(5,a5) MD_1W(6,a6) MD_1W(7,a7)
  #undef MD_1W
}
COAX_DEV void coax_am_moe_down_q4s_nb(int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t n_used, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 raw_log, coax_v_f32 out, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5, ew = rows * blocks;
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || nb * n_used * cols > x.count || nb * n_used > wts.count || nb * rows > res.count || nb * rows > raw_log.count || nb * rows > out.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int nu = (int)u_cnt.data[0];
  if (nu < 0 || (int64_t)nu > u_eids.count || (int64_t)nu > u_mask.count || (int64_t)nu * 8 > u_slot.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t r0 = row0 + (int64_t)(tid >> 5);
  if (r0 >= rows) return;
  uint32_t lane = tid & 31u;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks, inb = (int)nb;
  float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
  for (int u = 0; u < nu; u++) {
    uint32_t m = u_mask.data[u];
    int64_t eb = (int64_t)u_eids.data[u] * ew;
    if ((eb + ew) * 4 > qs.count || eb + ew > ds.count) { coax_trap_(line, coax_err); return; }
    int64_t rb0 = eb + r0 * blocks;
    const device uint2 *qp = (const device uint2 *)(qs.data + rb0 * 4) + hb;
    const device half *dp = (const device half *)(ds.data + rb0);
    uint32_t pc = popcount(m);
    if (pc == 1u) {
      int t = (int)ctz(m);
      int sl = (int)u_slot.data[u * 8 + t];
      float wt = wts.data[(int64_t)t * n_used + sl];
      const device float *xp = x.data + ((int64_t)t * n_used + sl) * cols + xq;
      float s0 = 0.0f;
      for (int b = blk_lane; b < iblocks; b += 16) {
        const device float *bx = xp + b * 32;
        float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4), f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
        float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w);
        s0 += float(dp[b]) * coax_q4s_dot16_u2(qp[b * 2], half4(f0), half4(f1), half4(f2), half4(f3), xs);
      }
      if (t==0) a0=fma(wt,s0,a0); else if (t==1) a1=fma(wt,s0,a1); else if (t==2) a2=fma(wt,s0,a2); else if (t==3) a3=fma(wt,s0,a3);
      else if (t==4) a4=fma(wt,s0,a4); else if (t==5) a5=fma(wt,s0,a5); else if (t==6) a6=fma(wt,s0,a6); else a7=fma(wt,s0,a7);
    } else if (pc == 2u) {
      int t0 = (int)ctz(m), t1 = (int)ctz(m & (m - 1u));
      int sl0 = (int)u_slot.data[u * 8 + t0], sl1 = (int)u_slot.data[u * 8 + t1];
      float wt0 = wts.data[(int64_t)t0 * n_used + sl0], wt1 = wts.data[(int64_t)t1 * n_used + sl1];
      const device float *xp0 = x.data + ((int64_t)t0 * n_used + sl0) * cols + xq;
      const device float *xp1 = x.data + ((int64_t)t1 * n_used + sl1) * cols + xq;
      float s0 = 0.0f, s1 = 0.0f;
      for (int b = blk_lane; b < iblocks; b += 16) {
        uint2 w0 = qp[b * 2]; float d0 = float(dp[b]);
        half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
        half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
        int bo = b * 32;
        const device float *bx0 = xp0 + bo;
        float4 f0=*(const device float4 *)bx0, f1=*(const device float4 *)(bx0+4), f2=*(const device float4 *)(bx0+16), f3=*(const device float4 *)(bx0+20);
        float4 fs0=(f0+f1)+(f2+f3); s0 += d0 * coax_q4s_dot16_pre(p0,p1,p2,p3,half4(f0),half4(f1),half4(f2),half4(f3),(fs0.x+fs0.y)+(fs0.z+fs0.w));
        const device float *bx1 = xp1 + bo;
        float4 g0=*(const device float4 *)bx1, g1=*(const device float4 *)(bx1+4), g2=*(const device float4 *)(bx1+16), g3=*(const device float4 *)(bx1+20);
        float4 fs1=(g0+g1)+(g2+g3); s1 += d0 * coax_q4s_dot16_pre(p0,p1,p2,p3,half4(g0),half4(g1),half4(g2),half4(g3),(fs1.x+fs1.y)+(fs1.z+fs1.w));
      }
      #define MDQ_ADD1(T, W, S) if ((T)==0) a0=fma(W,S,a0); else if ((T)==1) a1=fma(W,S,a1); else if ((T)==2) a2=fma(W,S,a2); else if ((T)==3) a3=fma(W,S,a3); else if ((T)==4) a4=fma(W,S,a4); else if ((T)==5) a5=fma(W,S,a5); else if ((T)==6) a6=fma(W,S,a6); else a7=fma(W,S,a7);
      MDQ_ADD1(t0, wt0, s0) MDQ_ADD1(t1, wt1, s1)
      #undef MDQ_ADD1
    } else {
      int ts[8]; const device float *xps[8]; float wtv[8]; int nt = 0;
      for (uint32_t mm = m; mm; mm &= (mm - 1u)) {
        int t = (int)ctz(mm); int sl = (int)u_slot.data[u * 8 + t];
        ts[nt] = t; xps[nt] = x.data + ((int64_t)t * n_used + sl) * cols + xq;
        wtv[nt] = wts.data[(int64_t)t * n_used + sl]; nt++;
      }
      float s0=0,s1=0,s2=0,s3=0,s4=0,s5=0,s6=0,s7=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        uint2 w0 = qp[b * 2]; float d0 = float(dp[b]);
        half4 p0 = half4(as_type<uchar4>(w0.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w0.x >> 4u) & 0x0f0f0f0fu));
        half4 p2 = half4(as_type<uchar4>(w0.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w0.y >> 4u) & 0x0f0f0f0fu));
        int bo = b * 32;
        #define MDQ_S(K, S) if ((K) < nt) { \
          const device float *bx = xps[K] + bo; \
          float4 f0=*(const device float4 *)bx, f1=*(const device float4 *)(bx+4), f2=*(const device float4 *)(bx+16), f3=*(const device float4 *)(bx+20); \
          float4 fs=(f0+f1)+(f2+f3); S += d0 * coax_q4s_dot16_pre(p0,p1,p2,p3,half4(f0),half4(f1),half4(f2),half4(f3),(fs.x+fs.y)+(fs.z+fs.w)); }
        MDQ_S(0,s0) MDQ_S(1,s1) MDQ_S(2,s2) MDQ_S(3,s3) MDQ_S(4,s4) MDQ_S(5,s5) MDQ_S(6,s6) MDQ_S(7,s7)
        #undef MDQ_S
      }
      #define MDQ_A(K, S) if ((K) < nt) { int T = ts[K]; float W = wtv[K]; \
        if (T==0) a0=fma(W,S,a0); else if (T==1) a1=fma(W,S,a1); else if (T==2) a2=fma(W,S,a2); else if (T==3) a3=fma(W,S,a3); \
        else if (T==4) a4=fma(W,S,a4); else if (T==5) a5=fma(W,S,a5); else if (T==6) a6=fma(W,S,a6); else a7=fma(W,S,a7); }
      MDQ_A(0,s0) MDQ_A(1,s1) MDQ_A(2,s2) MDQ_A(3,s3) MDQ_A(4,s4) MDQ_A(5,s5) MDQ_A(6,s6) MDQ_A(7,s7)
      #undef MDQ_A
    }
  }
  float u0 = 0, u1 = 0, u2 = 0; float4 c0 = float4(0);
  if (lane == 0) { u0 = hist.data[r0 * 3]; u1 = hist.data[r0 * 3 + 1]; u2 = hist.data[r0 * 3 + 2]; c0 = *(const device float4 *)(cw.data + r0 * 4); }
  #define MDQ_1W(T, A) if ((T) < inb) { \
    float D0 = simd_sum(A); \
    if (lane == 0) { \
      int64_t tr = (int64_t)(T) * rows; \
      raw_log.data[tr + r0] = D0; \
      out.data[tr + r0] = res.data[tr + r0] + (D0 + u0*c0.x + u1*c0.y + u2*c0.z + D0*c0.w) * 0.4472135954999579f; \
      u0 = u1; u1 = u2; u2 = D0; } }
  MDQ_1W(0,a0) MDQ_1W(1,a1) MDQ_1W(2,a2) MDQ_1W(3,a3) MDQ_1W(4,a4) MDQ_1W(5,a5) MDQ_1W(6,a6) MDQ_1W(7,a7)
  #undef MDQ_1W
}
COAX_DEV void coax_am_moe_down_iq4s_nb(int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t n_used, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 wts, coax_v_f32 cw, coax_v_f32 hist, coax_v_f32 res, coax_v_f32 raw_log, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5, ew = rows * blocks;
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || nb * n_used * cols > x.count || nb * n_used > wts.count || nb * rows > res.count || nb * rows > raw_log.count || nb * rows > out.count || rows * 4 > cw.count || rows * 3 > hist.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int nu = (int)u_cnt.data[0];
  if (nu < 0 || (int64_t)nu > u_eids.count || (int64_t)nu > u_mask.count || (int64_t)nu * 8 > u_slot.count) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  int64_t r0 = row0 + (int64_t)(tid >> 5);
  if (r0 >= rows) return;
  uint32_t lane = tid & 31u;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks, inb = (int)nb;
  float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
  for (int u = 0; u < nu; u++) {
    uint32_t m = u_mask.data[u];
    int64_t eb = (int64_t)u_eids.data[u] * ew;
    if ((eb + ew) * 4 > qs.count || eb + ew > ds.count) { coax_trap_(line, coax_err); return; }
    int64_t rb0 = eb + r0 * blocks;
    const device uint2 *qp = (const device uint2 *)(qs.data + rb0 * 4) + hb;
    const device half *dp = (const device half *)(ds.data + rb0);
    uint32_t pc = popcount(m);
    if (pc == 1u) {
      int t = (int)ctz(m);
      int sl = (int)u_slot.data[u * 8 + t];
      float wt = 16.0f * wts.data[(int64_t)t * n_used + sl];
      const device float *xp = x.data + ((int64_t)t * n_used + sl) * cols + xq;
      float s0 = 0.0f;
      for (int b = blk_lane; b < iblocks; b += 16) {
        const device float *bx = xp + b * 32;
        half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
        half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
        half2 x0(xl0.x,xh0.x),x1(xl0.y,xh0.y),x2(xl0.z,xh0.z),x3(xl0.w,xh0.w),x4(xl1.x,xh1.x),x5(xl1.y,xh1.y),x6(xl1.z,xh1.z),x7(xl1.w,xh1.w);
        s0 += float(dp[b]) * coax_iq4s_dot16_u2(qp[b * 2], x0,x1,x2,x3,x4,x5,x6,x7, tlut);
      }
      if (t==0) a0=fma(wt,s0,a0); else if (t==1) a1=fma(wt,s0,a1); else if (t==2) a2=fma(wt,s0,a2); else if (t==3) a3=fma(wt,s0,a3);
      else if (t==4) a4=fma(wt,s0,a4); else if (t==5) a5=fma(wt,s0,a5); else if (t==6) a6=fma(wt,s0,a6); else a7=fma(wt,s0,a7);
    } else if (pc == 2u) {
      int t0 = (int)ctz(m), t1 = (int)ctz(m & (m - 1u));
      int sl0 = (int)u_slot.data[u * 8 + t0], sl1 = (int)u_slot.data[u * 8 + t1];
      float wt0 = 16.0f * wts.data[(int64_t)t0 * n_used + sl0], wt1 = 16.0f * wts.data[(int64_t)t1 * n_used + sl1];
      const device float *xp0 = x.data + ((int64_t)t0 * n_used + sl0) * cols + xq;
      const device float *xp1 = x.data + ((int64_t)t1 * n_used + sl1) * cols + xq;
      float s0 = 0.0f, s1 = 0.0f;
      for (int b = blk_lane; b < iblocks; b += 16) {
        float d0 = float(dp[b]); uint2 p_ = qp[b * 2];
        uchar4 b0 = as_type<uchar4>(p_.x), b1 = as_type<uchar4>(p_.y);
        half2 w0=tlut[b0.x],w1=tlut[b0.y],w2=tlut[b0.z],w3=tlut[b0.w];
        half2 w4=tlut[b1.x],w5=tlut[b1.y],w6=tlut[b1.z],w7=tlut[b1.w];
        int bo = b * 32;
        const device float *bx0 = xp0 + bo;
        half4 xl0=half4(*(const device float4 *)bx0), xl1=half4(*(const device float4 *)(bx0+4));
        half4 xh0=half4(*(const device float4 *)(bx0+16)), xh1=half4(*(const device float4 *)(bx0+20));
        half2 d0v = w0*half2(xl0.x,xh0.x)+w1*half2(xl0.y,xh0.y)+w2*half2(xl0.z,xh0.z)+w3*half2(xl0.w,xh0.w);
        d0v += w4*half2(xl1.x,xh1.x)+w5*half2(xl1.y,xh1.y)+w6*half2(xl1.z,xh1.z)+w7*half2(xl1.w,xh1.w);
        s0 += d0 * float(d0v.x + d0v.y);
        const device float *bx1 = xp1 + bo;
        half4 yl0=half4(*(const device float4 *)bx1), yl1=half4(*(const device float4 *)(bx1+4));
        half4 yh0=half4(*(const device float4 *)(bx1+16)), yh1=half4(*(const device float4 *)(bx1+20));
        half2 d1v = w0*half2(yl0.x,yh0.x)+w1*half2(yl0.y,yh0.y)+w2*half2(yl0.z,yh0.z)+w3*half2(yl0.w,yh0.w);
        d1v += w4*half2(yl1.x,yh1.x)+w5*half2(yl1.y,yh1.y)+w6*half2(yl1.z,yh1.z)+w7*half2(yl1.w,yh1.w);
        s1 += d0 * float(d1v.x + d1v.y);
      }
      #define MDIS_ADD1(T, W, S) if ((T)==0) a0=fma(W,S,a0); else if ((T)==1) a1=fma(W,S,a1); else if ((T)==2) a2=fma(W,S,a2); else if ((T)==3) a3=fma(W,S,a3); else if ((T)==4) a4=fma(W,S,a4); else if ((T)==5) a5=fma(W,S,a5); else if ((T)==6) a6=fma(W,S,a6); else a7=fma(W,S,a7);
      MDIS_ADD1(t0, wt0, s0) MDIS_ADD1(t1, wt1, s1)
      #undef MDIS_ADD1
    } else {
      int ts[8]; const device float *xps[8]; float wtv[8]; int nt = 0;
      for (uint32_t mm = m; mm; mm &= (mm - 1u)) {
        int t = (int)ctz(mm); int sl = (int)u_slot.data[u * 8 + t];
        ts[nt] = t; xps[nt] = x.data + ((int64_t)t * n_used + sl) * cols + xq;
        wtv[nt] = 16.0f * wts.data[(int64_t)t * n_used + sl]; nt++;
      }
      float s0=0,s1=0,s2=0,s3=0,s4=0,s5=0,s6=0,s7=0;
      for (int b = blk_lane; b < iblocks; b += 16) {
        float d0 = float(dp[b]); uint2 p_ = qp[b * 2];
        uchar4 b0 = as_type<uchar4>(p_.x), b1 = as_type<uchar4>(p_.y);
        half2 w0=tlut[b0.x],w1=tlut[b0.y],w2=tlut[b0.z],w3=tlut[b0.w];
        half2 w4=tlut[b1.x],w5=tlut[b1.y],w6=tlut[b1.z],w7=tlut[b1.w];
        int bo = b * 32;
        #define MDIS_S(K, S) if ((K) < nt) { \
          const device float *bx = xps[K] + bo; \
          half4 xl0=half4(*(const device float4 *)bx), xl1=half4(*(const device float4 *)(bx+4)); \
          half4 xh0=half4(*(const device float4 *)(bx+16)), xh1=half4(*(const device float4 *)(bx+20)); \
          half2 dpv = w0*half2(xl0.x,xh0.x)+w1*half2(xl0.y,xh0.y)+w2*half2(xl0.z,xh0.z)+w3*half2(xl0.w,xh0.w); \
          dpv += w4*half2(xl1.x,xh1.x)+w5*half2(xl1.y,xh1.y)+w6*half2(xl1.z,xh1.z)+w7*half2(xl1.w,xh1.w); \
          S += d0 * float(dpv.x + dpv.y); }
        MDIS_S(0,s0) MDIS_S(1,s1) MDIS_S(2,s2) MDIS_S(3,s3) MDIS_S(4,s4) MDIS_S(5,s5) MDIS_S(6,s6) MDIS_S(7,s7)
        #undef MDIS_S
      }
      #define MDIS_A(K, S) if ((K) < nt) { int T = ts[K]; float W = wtv[K]; \
        if (T==0) a0=fma(W,S,a0); else if (T==1) a1=fma(W,S,a1); else if (T==2) a2=fma(W,S,a2); else if (T==3) a3=fma(W,S,a3); \
        else if (T==4) a4=fma(W,S,a4); else if (T==5) a5=fma(W,S,a5); else if (T==6) a6=fma(W,S,a6); else a7=fma(W,S,a7); }
      MDIS_A(0,s0) MDIS_A(1,s1) MDIS_A(2,s2) MDIS_A(3,s3) MDIS_A(4,s4) MDIS_A(5,s5) MDIS_A(6,s6) MDIS_A(7,s7)
      #undef MDIS_A
    }
  }
  float u0 = 0, u1 = 0, u2 = 0; float4 c0 = float4(0);
  if (lane == 0) { u0 = hist.data[r0 * 3]; u1 = hist.data[r0 * 3 + 1]; u2 = hist.data[r0 * 3 + 2]; c0 = *(const device float4 *)(cw.data + r0 * 4); }
  #define MDIS_1W(T, A) if ((T) < inb) { \
    float D0 = simd_sum(A); \
    if (lane == 0) { \
      int64_t tr = (int64_t)(T) * rows; \
      raw_log.data[tr + r0] = D0; \
      out.data[tr + r0] = res.data[tr + r0] + (D0 + u0*c0.x + u1*c0.y + u2*c0.z + D0*c0.w) * 0.4472135954999579f; \
      u0 = u1; u1 = u2; u2 = D0; } }
  MDIS_1W(0,a0) MDIS_1W(1,a1) MDIS_1W(2,a2) MDIS_1W(3,a3) MDIS_1W(4,a4) MDIS_1W(5,a5) MDIS_1W(6,a6) MDIS_1W(7,a7)
  #undef MDIS_1W
}
COAX_DEV void coax_am_moe_down_p1_iq4s_nb(int64_t u, int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t n_used, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 slot_out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5, ew = rows * blocks;
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || nb * n_used * cols > x.count || nb * n_used * rows > slot_out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  if (u < 0 || u >= (int64_t)u_cnt.data[0]) return;
  if (u >= u_eids.count || u >= u_mask.count || (u + 1) * 8 > u_slot.count) { coax_trap_(line, coax_err); return; }
  int64_t eb = (int64_t)u_eids.data[u] * ew;
  if ((eb + ew) * 4 > qs.count || eb + ew > ds.count) { coax_trap_(line, coax_err); return; }
  threadgroup half2 *tlut = (threadgroup half2 *)tg;
  coax_iq4_init_tlut(tlut, tid);
  uint32_t sg = tid >> 5, lane = tid & 31u;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  for (int rp = 0; rp < 2; rp++) {
    int64_t r0 = row0 + (int64_t)(sg * 4u + (uint32_t)rp * 2u);
    if (r0 >= rows) continue;
    int64_t r1 = (r0 + 1 < rows) ? (r0 + 1) : r0;
    const device uint2 *qp0 = (const device uint2 *)(qs.data + (eb + r0 * blocks) * 4) + hb;
    const device half *dp0 = (const device half *)(ds.data + eb + r0 * blocks);
    const device uint2 *qp1 = (const device uint2 *)(qs.data + (eb + r1 * blocks) * 4) + hb;
    const device half *dp1 = (const device half *)(ds.data + eb + r1 * blocks);
    uint32_t mask = u_mask.data[u];
    while (mask) {
      int t0 = (int)ctz(mask); mask &= (mask - 1u);
      int sl0 = (int)u_slot.data[u * 8 + t0];
      if (!mask) {
        const device float *xp = x.data + ((int64_t)t0 * n_used + sl0) * cols + xq;
        float s0 = 0.0f, s1 = 0.0f;
        for (int b = blk_lane; b < iblocks; b += 16) {
          const device float *bx = xp + b * 32;
          half4 xl0 = half4(*(const device float4 *)bx), xl1 = half4(*(const device float4 *)(bx + 4));
          half4 xh0 = half4(*(const device float4 *)(bx + 16)), xh1 = half4(*(const device float4 *)(bx + 20));
          half2 a0(xl0.x,xh0.x),a1(xl0.y,xh0.y),a2(xl0.z,xh0.z),a3(xl0.w,xh0.w),a4(xl1.x,xh1.x),a5(xl1.y,xh1.y),a6(xl1.z,xh1.z),a7(xl1.w,xh1.w);
          s0 += float(dp0[b]) * coax_iq4s_dot16_u2(qp0[b * 2], a0,a1,a2,a3,a4,a5,a6,a7, tlut);
          s1 += float(dp1[b]) * coax_iq4s_dot16_u2(qp1[b * 2], a0,a1,a2,a3,a4,a5,a6,a7, tlut);
        }
        float v0 = simd_sum(s0) * 16.0f, v1 = simd_sum(s1) * 16.0f;
        if (lane == 0) {
          int64_t ob = ((int64_t)t0 * n_used + sl0) * rows;
          slot_out.data[ob + r0] = v0;
          if (r1 != r0) slot_out.data[ob + r1] = v1;
        }
      } else {
        int t1 = (int)ctz(mask); mask &= (mask - 1u);
        int sl1 = (int)u_slot.data[u * 8 + t1];
        const device float *xp0 = x.data + ((int64_t)t0 * n_used + sl0) * cols + xq;
        const device float *xp1 = x.data + ((int64_t)t1 * n_used + sl1) * cols + xq;
        float s00 = 0, s01 = 0, s10 = 0, s11 = 0;
        for (int b = blk_lane; b < iblocks; b += 16) {
          int bo = b * 32;
          const device float *bx0 = xp0 + bo;
          half4 xl0 = half4(*(const device float4 *)bx0), xl1 = half4(*(const device float4 *)(bx0 + 4));
          half4 xh0 = half4(*(const device float4 *)(bx0 + 16)), xh1 = half4(*(const device float4 *)(bx0 + 20));
          half2 a0(xl0.x,xh0.x),a1(xl0.y,xh0.y),a2(xl0.z,xh0.z),a3(xl0.w,xh0.w),a4(xl1.x,xh1.x),a5(xl1.y,xh1.y),a6(xl1.z,xh1.z),a7(xl1.w,xh1.w);
          const device float *bx1 = xp1 + bo;
          half4 yl0 = half4(*(const device float4 *)bx1), yl1 = half4(*(const device float4 *)(bx1 + 4));
          half4 yh0 = half4(*(const device float4 *)(bx1 + 16)), yh1 = half4(*(const device float4 *)(bx1 + 20));
          half2 c0(yl0.x,yh0.x),c1(yl0.y,yh0.y),c2(yl0.z,yh0.z),c3(yl0.w,yh0.w),c4(yl1.x,yh1.x),c5(yl1.y,yh1.y),c6(yl1.z,yh1.z),c7(yl1.w,yh1.w);
          #define DP_IQ4S(QP, DP, AA, BB) { \
            float ds_ = float((DP)[b]); uint2 p_ = (QP)[b * 2]; \
            uchar4 b0 = as_type<uchar4>(p_.x), b1 = as_type<uchar4>(p_.y); \
            half2 w0=tlut[b0.x],w1=tlut[b0.y],w2=tlut[b0.z],w3=tlut[b0.w]; \
            half2 w4=tlut[b1.x],w5=tlut[b1.y],w6=tlut[b1.z],w7=tlut[b1.w]; \
            half2 rA = w0*a0+w1*a1+w2*a2+w3*a3; rA += w4*a4+w5*a5+w6*a6+w7*a7; AA += ds_ * float(rA.x + rA.y); \
            half2 rB = w0*c0+w1*c1+w2*c2+w3*c3; rB += w4*c4+w5*c5+w6*c6+w7*c7; BB += ds_ * float(rB.x + rB.y); }
          DP_IQ4S(qp0, dp0, s00, s10) DP_IQ4S(qp1, dp1, s01, s11)
          #undef DP_IQ4S
        }
        float v00 = simd_sum(s00) * 16.0f, v01 = simd_sum(s01) * 16.0f;
        float v10 = simd_sum(s10) * 16.0f, v11 = simd_sum(s11) * 16.0f;
        if (lane == 0) {
          int64_t ob0 = ((int64_t)t0 * n_used + sl0) * rows;
          slot_out.data[ob0 + r0] = v00; if (r1 != r0) slot_out.data[ob0 + r1] = v01;
          int64_t ob1 = ((int64_t)t1 * n_used + sl1) * rows;
          slot_out.data[ob1 + r0] = v10; if (r1 != r0) slot_out.data[ob1 + r1] = v11;
        }
      }
    }
  }
}
COAX_DEV void coax_am_moe_down_p1_q4s_nb(int64_t u, int64_t row0, int64_t rows, int64_t cols, int64_t nb, int64_t n_used, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_u32 u_eids, coax_v_u32 u_mask, coax_v_u32 u_slot, coax_v_u32 u_cnt, coax_v_f32 slot_out, uint32_t tid, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5, ew = rows * blocks;
  if (u_cnt.count < 1 || nb < 1 || nb > 8 || n_used < 1 || rows <= 0 || cols <= 0 || (cols & 31) != 0 || row0 < 0 || nb * n_used * cols > x.count || nb * n_used * rows > slot_out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  if (u < 0 || u >= (int64_t)u_cnt.data[0]) return;
  if (u >= u_eids.count || u >= u_mask.count || (u + 1) * 8 > u_slot.count) { coax_trap_(line, coax_err); return; }
  int64_t eb = (int64_t)u_eids.data[u] * ew;
  if ((eb + ew) * 4 > qs.count || eb + ew > ds.count) { coax_trap_(line, coax_err); return; }
  uint32_t sg = tid >> 5, lane = tid & 31u;
  int blk_lane = (int)(lane >> 1), hb = (int)(lane & 1u), xq = hb * 8, iblocks = (int)blocks;
  for (int rp = 0; rp < 2; rp++) {
    int64_t r0 = row0 + (int64_t)(sg * 4u + (uint32_t)rp * 2u);
    if (r0 >= rows) continue;
    int64_t r1 = (r0 + 1 < rows) ? (r0 + 1) : r0;
    const device uint2 *qp0 = (const device uint2 *)(qs.data + (eb + r0 * blocks) * 4) + hb;
    const device half *dp0 = (const device half *)(ds.data + eb + r0 * blocks);
    const device uint2 *qp1 = (const device uint2 *)(qs.data + (eb + r1 * blocks) * 4) + hb;
    const device half *dp1 = (const device half *)(ds.data + eb + r1 * blocks);
    uint32_t mask = u_mask.data[u];
    while (mask) {
      int t0 = (int)ctz(mask); mask &= (mask - 1u);
      int sl0 = (int)u_slot.data[u * 8 + t0];
      if (!mask) {
        const device float *xp = x.data + ((int64_t)t0 * n_used + sl0) * cols + xq;
        float s0 = 0.0f, s1 = 0.0f;
        for (int b = blk_lane; b < iblocks; b += 16) {
          const device float *bx = xp + b * 32;
          float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4), f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20);
          float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w);
          half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
          s0 += float(dp0[b]) * coax_q4s_dot16_u2(qp0[b * 2], xl0, xl1, xh0, xh1, xs);
          s1 += float(dp1[b]) * coax_q4s_dot16_u2(qp1[b * 2], xl0, xl1, xh0, xh1, xs);
        }
        float v0 = simd_sum(s0), v1 = simd_sum(s1);
        if (lane == 0) {
          int64_t ob = ((int64_t)t0 * n_used + sl0) * rows;
          slot_out.data[ob + r0] = v0;
          if (r1 != r0) slot_out.data[ob + r1] = v1;
        }
      } else {
        int t1 = (int)ctz(mask); mask &= (mask - 1u);
        int sl1 = (int)u_slot.data[u * 8 + t1];
        const device float *xp0 = x.data + ((int64_t)t0 * n_used + sl0) * cols + xq;
        const device float *xp1 = x.data + ((int64_t)t1 * n_used + sl1) * cols + xq;
        float s00 = 0, s01 = 0, s10 = 0, s11 = 0;
        for (int b = blk_lane; b < iblocks; b += 16) {
          int bo = b * 32;
          const device float *bx0 = xp0 + bo;
          float4 f0 = *(const device float4 *)bx0, f1 = *(const device float4 *)(bx0 + 4), f2 = *(const device float4 *)(bx0 + 16), f3 = *(const device float4 *)(bx0 + 20);
          float4 fs0 = (f0 + f1) + (f2 + f3); float xs0 = (fs0.x + fs0.y) + (fs0.z + fs0.w);
          half4 xl0 = half4(f0), xl1 = half4(f1), xh0 = half4(f2), xh1 = half4(f3);
          const device float *bx1 = xp1 + bo;
          float4 g0 = *(const device float4 *)bx1, g1 = *(const device float4 *)(bx1 + 4), g2 = *(const device float4 *)(bx1 + 16), g3 = *(const device float4 *)(bx1 + 20);
          float4 fs1 = (g0 + g1) + (g2 + g3); float xs1 = (fs1.x + fs1.y) + (fs1.z + fs1.w);
          half4 yl0 = half4(g0), yl1 = half4(g1), yh0 = half4(g2), yh1 = half4(g3);
          #define DP_Q4S(QP, DP, AA, BB) { \
            uint2 w_ = (QP)[b * 2]; float d_ = float((DP)[b]); \
            half4 p0 = half4(as_type<uchar4>(w_.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((w_.x >> 4u) & 0x0f0f0f0fu)); \
            half4 p2 = half4(as_type<uchar4>(w_.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((w_.y >> 4u) & 0x0f0f0f0fu)); \
            AA += d_ * coax_q4s_dot16_pre(p0, p1, p2, p3, xl0, xl1, xh0, xh1, xs0); \
            BB += d_ * coax_q4s_dot16_pre(p0, p1, p2, p3, yl0, yl1, yh0, yh1, xs1); }
          DP_Q4S(qp0, dp0, s00, s10) DP_Q4S(qp1, dp1, s01, s11)
          #undef DP_Q4S
        }
        float v00 = simd_sum(s00), v01 = simd_sum(s01);
        float v10 = simd_sum(s10), v11 = simd_sum(s11);
        if (lane == 0) {
          int64_t ob0 = ((int64_t)t0 * n_used + sl0) * rows;
          slot_out.data[ob0 + r0] = v00; if (r1 != r0) slot_out.data[ob0 + r1] = v01;
          int64_t ob1 = ((int64_t)t1 * n_used + sl1) * rows;
          slot_out.data[ob1 + r0] = v10; if (r1 != r0) slot_out.data[ob1 + r1] = v11;
        }
      }
    }
  }
}
COAX_DEV void coax_am_conv_commit_nb(int64_t item, int64_t n_slots, int64_t n_kv, int64_t d, int64_t pos0, int64_t got, coax_v_f32 conv_all, coax_v_f32 raw_log, int32_t line, COAX_ERR) {
  int64_t ch_per = 2 * n_kv + 2 * d;
  int64_t conv_stride = 2 * n_kv * 4 + 2 * d * 3;
  int64_t raw_stride = 8 * ch_per;
  if (n_slots <= 0 || n_kv <= 0 || d <= 0 || pos0 < 0 || got < 1 || got > 8 || n_slots * conv_stride > conv_all.count || n_slots * raw_stride > raw_log.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t il = item / ch_per;
  if (il < 0 || il >= n_slots) return;
  int64_t ch = item % ch_per;
  int64_t cbase = il * conv_stride, rbase = il * raw_stride;
  if (ch < n_kv) {
    device float *h = conv_all.data + cbase + ch * 4;
    for (int t = 0; t < (int)got; t++) h[(uint32_t)(pos0 + t) & 3u] = raw_log.data[rbase + t * n_kv + ch];
  } else if (ch < 2 * n_kv) {
    int64_t c = ch - n_kv;
    device float *h = conv_all.data + cbase + n_kv * 4 + c * 4;
    for (int t = 0; t < (int)got; t++) h[(uint32_t)(pos0 + t) & 3u] = raw_log.data[rbase + 8 * n_kv + t * n_kv + c];
  } else if (ch < 2 * n_kv + d) {
    int64_t c = ch - 2 * n_kv;
    device float *h = conv_all.data + cbase + 2 * n_kv * 4 + c * 3;
    float h0 = h[0], h1 = h[1], h2 = h[2];
    for (int t = 0; t < (int)got; t++) { h0 = h1; h1 = h2; h2 = raw_log.data[rbase + 16 * n_kv + t * d + c]; }
    h[0] = h0; h[1] = h1; h[2] = h2;
  } else {
    int64_t c = ch - (2 * n_kv + d);
    device float *h = conv_all.data + cbase + 2 * n_kv * 4 + d * 3 + c * 3;
    float h0 = h[0], h1 = h[1], h2 = h[2];
    for (int t = 0; t < (int)got; t++) { h0 = h1; h1 = h2; h2 = raw_log.data[rbase + 16 * n_kv + 8 * d + t * d + c]; }
    h[0] = h0; h[1] = h1; h[2] = h2;
  }
}
COAX_DEV void coax_am_dot_q4s_small_nb(int64_t row, int64_t rows_per_grp, int64_t cols, int64_t nb, int64_t n_parts, float eps, coax_v_u32 qs, coax_v_u16 ds, coax_v_f32 x, coax_v_f32 parts, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t blocks = cols >> 5;
  if (row < 0 || rows_per_grp <= 0 || cols <= 0 || (cols & 31) != 0 || nb < 1 || nb > 8 || n_parts < 0 || (row + 1) * blocks * 4 > qs.count || (row + 1) * blocks > ds.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int64_t grp = row / rows_per_grp, rloc = row % rows_per_grp;
  if ((grp + 1) * nb * cols > x.count || (grp + 1) * nb * rows_per_grp > out.count || (n_parts > 0 && (grp + 1) * nb * n_parts > parts.count)) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u, sg = tid >> 5, blk_lane = tid >> 1, hb = tid & 1u;
  int xq = (int)(hb * 8u), iblocks = (int)blocks, inb = (int)nb;
  const device uint2 *qp = (const device uint2 *)(qs.data + row * blocks * 4);
  const device half *dp = (const device half *)(ds.data + row * blocks);
  const device float *xp = x.data + grp * nb * cols;
  float a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0;
  for (int b = (int)blk_lane; b < iblocks; b += 64) {
    uint2 wq = qp[b * 2 + (int)hb];
    float d = float(dp[b]);
    half4 p0 = half4(as_type<uchar4>(wq.x & 0x0f0f0f0fu)), p1 = half4(as_type<uchar4>((wq.x >> 4u) & 0x0f0f0f0fu));
    half4 p2 = half4(as_type<uchar4>(wq.y & 0x0f0f0f0fu)), p3 = half4(as_type<uchar4>((wq.y >> 4u) & 0x0f0f0f0fu));
    int bo = b * 32 + xq;
    #define QSM_T(T, A) if ((T) < inb) { \
      const device float *bx = xp + (int64_t)(T) * cols + bo; \
      float4 f0 = *(const device float4 *)bx, f1 = *(const device float4 *)(bx + 4); \
      float4 f2 = *(const device float4 *)(bx + 16), f3 = *(const device float4 *)(bx + 20); \
      float4 fs = (f0 + f1) + (f2 + f3); float xs = (fs.x + fs.y) + (fs.z + fs.w); \
      A += d * coax_q4s_dot16_pre(p0, p1, p2, p3, half4(f0), half4(f1), half4(f2), half4(f3), xs); }
    QSM_T(0,a0) QSM_T(1,a1) QSM_T(2,a2) QSM_T(3,a3)
    QSM_T(4,a4) QSM_T(5,a5) QSM_T(6,a6) QSM_T(7,a7)
    #undef QSM_T
  }
  #define QSM_R(T, A) if ((T) < inb) { float wa = simd_sum(A); if (lane == 0u) tg[(T) * 4 + (int)sg] = wa; }
  QSM_R(0,a0) QSM_R(1,a1) QSM_R(2,a2) QSM_R(3,a3)
  QSM_R(4,a4) QSM_R(5,a5) QSM_R(6,a6) QSM_R(7,a7)
  #undef QSM_R
  if (sg == 0u && n_parts > 0) {
    for (int t = 0; t < inb; t++) {
      float iv = coax_inv_rms_parts(parts, (grp * nb + t) * n_parts, n_parts, cols, eps, lane);
      if (lane == 0u) tg[32 + t] = iv;
    }
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if ((int)tid < inb) {
    int t = (int)tid;
    float iv = (n_parts > 0) ? tg[32 + t] : 1.0f;
    out.data[(grp * nb + t) * rows_per_grp + rloc] = ((tg[t * 4] + tg[t * 4 + 1]) + (tg[t * 4 + 2] + tg[t * 4 + 3])) * iv;
  }
}
COAX_DEV void coax_am_mix_fused_nb(int64_t mode, int64_t dim, int64_t n_blocks, int64_t nb, int64_t t_idx, float eps, coax_v_f32 blocks_buf, coax_v_f32 cur, coax_v_f32 query, int64_t q_off, coax_v_f32 aux, int64_t aux_off, coax_v_f32 out0, int64_t o0_off, coax_v_f32 out1, int64_t o1_off, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int n_states = (int)n_blocks + 1;
  if (dim <= 0 || n_blocks < 0 || n_states > 32 || nb < 1 || t_idx < 0 || t_idx >= nb || n_blocks * nb * dim > blocks_buf.count || nb * dim > cur.count || q_off < 0 || q_off + dim > query.count || aux_off < 0 || aux_off + dim > aux.count || o0_off < 0 || o0_off + dim > out0.count || (mode != 0 && (o1_off < 0 || o1_off + 2 * dim > out1.count))) {
    coax_trap_(line, coax_err);
    return;
  }
  threadgroup float *tg_ss = tg;
  threadgroup float *tg_dot = tg + 256;
  threadgroup float *tg_w = tg + 512;
  threadgroup float *tg_r1 = tg + 544;
  threadgroup float *tg_r2 = tg + 552;
  uint32_t lane = tid & 31u, sg = tid >> 5;
  int64_t cbase = t_idx * dim;
  if ((dim & 3) == 0 && dim <= 3072) {
    int n4 = (int)(dim >> 2);
    int i0 = (int)tid, i1 = i0 + 256, i2 = i0 + 512;
    float4 a0 = float4(0.0f), a1 = float4(0.0f), a2 = float4(0.0f);
    if (n_blocks == 0) {
      const device float4 *c4 = (const device float4 *)(cur.data + cbase);
      float4 c0 = (i0 < n4) ? c4[i0] : float4(0.0f);
      float4 c1 = (i1 < n4) ? c4[i1] : float4(0.0f);
      float4 c2 = (i2 < n4) ? c4[i2] : float4(0.0f);
      float ts = simd_sum(dot(c0, c0) + dot(c1, c1) + dot(c2, c2));
      if (lane == 0) tg_ss[sg] = ts;
      threadgroup_barrier(mem_flags::mem_threadgroup);
      float tss = (tg_ss[0] + tg_ss[1]) + (tg_ss[2] + tg_ss[3]) + (tg_ss[4] + tg_ss[5]) + (tg_ss[6] + tg_ss[7]);
      float iv = rsqrt(tss / (float)dim + 1.0e-6f);
      a0 = c0 * iv; a1 = c1 * iv; a2 = c2 * iv;
    } else {
      const device float4 *q4 = (const device float4 *)(query.data + q_off);
      float4 qv0 = (i0 < n4) ? q4[i0] : float4(0.0f);
      float4 qv1 = (i1 < n4) ? q4[i1] : float4(0.0f);
      float4 qv2 = (i2 < n4) ? q4[i2] : float4(0.0f);
      for (int s = 0; s < n_states; s++) {
        const device float4 *v4 = (s < (int)n_blocks) ? (const device float4 *)(blocks_buf.data + ((int64_t)s * nb + t_idx) * dim) : (const device float4 *)(cur.data + cbase);
        float ss = 0.0f, dd = 0.0f;
        if (i0 < n4) { float4 v = v4[i0]; ss += dot(v, v); dd += dot(v, qv0); }
        if (i1 < n4) { float4 v = v4[i1]; ss += dot(v, v); dd += dot(v, qv1); }
        if (i2 < n4) { float4 v = v4[i2]; ss += dot(v, v); dd += dot(v, qv2); }
        float ts = simd_sum(ss), td = simd_sum(dd);
        if (lane == 0) { tg_ss[s * 8 + sg] = ts; tg_dot[s * 8 + sg] = td; }
      }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      if (tid < 32u) {
        float inv_d = 1.0f / (float)dim;
        float inv_sqrt_d = rsqrt((float)dim);
        float sc = -3.0e38f, iv = 0.0f;
        if ((int)tid < n_states) {
          int b = (int)tid * 8;
          float tss = (tg_ss[b] + tg_ss[b+1]) + (tg_ss[b+2] + tg_ss[b+3]) + (tg_ss[b+4] + tg_ss[b+5]) + (tg_ss[b+6] + tg_ss[b+7]);
          float tdot = (tg_dot[b] + tg_dot[b+1]) + (tg_dot[b+2] + tg_dot[b+3]) + (tg_dot[b+4] + tg_dot[b+5]) + (tg_dot[b+6] + tg_dot[b+7]);
          iv = rsqrt(tss * inv_d + 1.0e-6f);
          sc = tdot * iv * inv_sqrt_d;
        }
        float mx = simd_max(sc);
        float w = ((int)tid < n_states) ? exp(sc - mx) : 0.0f;
        float inv_e = 1.0f / simd_sum(w);
        if ((int)tid < n_states) tg_w[tid] = w * inv_e * iv;
      }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      for (int s = 0; s < n_states; s++) {
        float ws = tg_w[s];
        const device float4 *v4 = (s < (int)n_blocks) ? (const device float4 *)(blocks_buf.data + ((int64_t)s * nb + t_idx) * dim) : (const device float4 *)(cur.data + cbase);
        if (i0 < n4) a0 += ws * v4[i0];
        if (i1 < n4) a1 += ws * v4[i1];
        if (i2 < n4) a2 += ws * v4[i2];
      }
    }
    device float4 *o0_4 = (device float4 *)(out0.data + o0_off);
    if (mode == 0) {
      float s1 = 0.0f;
      if (i0 < n4) s1 += dot(a0, a0);
      if (i1 < n4) s1 += dot(a1, a1);
      if (i2 < n4) s1 += dot(a2, a2);
      float ts1 = simd_sum(s1);
      if (lane == 0) tg_r1[sg] = ts1;
      threadgroup_barrier(mem_flags::mem_threadgroup);
      float tot = (tg_r1[0] + tg_r1[1]) + (tg_r1[2] + tg_r1[3]) + (tg_r1[4] + tg_r1[5]) + (tg_r1[6] + tg_r1[7]);
      float inv = rsqrt(tot / (float)dim + eps);
      const device float4 *nw4 = (const device float4 *)(aux.data + aux_off);
      if (i0 < n4) o0_4[i0] = a0 * inv * nw4[i0];
      if (i1 < n4) o0_4[i1] = a1 * inv * nw4[i1];
      if (i2 < n4) o0_4[i2] = a2 * inv * nw4[i2];
    } else {
      if (i0 < n4) o0_4[i0] = a0;
      if (i1 < n4) o0_4[i1] = a1;
      if (i2 < n4) o0_4[i2] = a2;
      const device float4 *m4 = (const device float4 *)(aux.data + aux_off);
      float4 m0 = (i0 < n4) ? m4[i0] : float4(0.0f);
      float4 m1 = (i1 < n4) ? m4[i1] : float4(0.0f);
      float4 m2 = (i2 < n4) ? m4[i2] : float4(0.0f);
      float s1 = 0.0f, s2 = 0.0f;
      if (i0 < n4) { s1 += dot(a0, a0); s2 += dot(m0, m0); }
      if (i1 < n4) { s1 += dot(a1, a1); s2 += dot(m1, m1); }
      if (i2 < n4) { s1 += dot(a2, a2); s2 += dot(m2, m2); }
      float ts1 = simd_sum(s1), ts2 = simd_sum(s2);
      if (lane == 0) { tg_r1[sg] = ts1; tg_r2[sg] = ts2; }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      float tot1 = (tg_r1[0] + tg_r1[1]) + (tg_r1[2] + tg_r1[3]) + (tg_r1[4] + tg_r1[5]) + (tg_r1[6] + tg_r1[7]);
      float tot2 = (tg_r2[0] + tg_r2[1]) + (tg_r2[2] + tg_r2[3]) + (tg_r2[4] + tg_r2[5]) + (tg_r2[6] + tg_r2[7]);
      float inv1 = rsqrt(tot1 / (float)dim + 1.0e-6f);
      float inv2 = rsqrt(tot2 / (float)dim + 1.0e-6f);
      device float4 *o1_a = (device float4 *)(out1.data + o1_off);
      device float4 *o1_b = (device float4 *)(out1.data + o1_off + dim);
      if (i0 < n4) { o1_a[i0] = a0 * inv1; o1_b[i0] = m0 * inv2; }
      if (i1 < n4) { o1_a[i1] = a1 * inv1; o1_b[i1] = m1 * inv2; }
      if (i2 < n4) { o1_a[i2] = a2 * inv1; o1_b[i2] = m2 * inv2; }
    }
    return;
  }
  const device float *qp = query.data + q_off;
  for (int s = 0; s < n_states; s++) {
    const device float *vp = (s < (int)n_blocks) ? (blocks_buf.data + ((int64_t)s * nb + t_idx) * dim) : (cur.data + cbase);
    float ss = 0.0f, dd = 0.0f;
    for (int i = (int)tid; i < (int)dim; i += 256) { float v = vp[i]; ss += v * v; dd += v * qp[i]; }
    float ts = simd_sum(ss), td = simd_sum(dd);
    if (lane == 0) { tg_ss[s * 8 + sg] = ts; tg_dot[s * 8 + sg] = td; }
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid < 32u) {
    float inv_d = 1.0f / (float)dim, inv_sqrt_d = rsqrt((float)dim);
    float sc = -3.0e38f, iv = 0.0f;
    if ((int)tid < n_states) {
      int b = (int)tid * 8;
      float tss = (tg_ss[b] + tg_ss[b+1]) + (tg_ss[b+2] + tg_ss[b+3]) + (tg_ss[b+4] + tg_ss[b+5]) + (tg_ss[b+6] + tg_ss[b+7]);
      float tdot = (tg_dot[b] + tg_dot[b+1]) + (tg_dot[b+2] + tg_dot[b+3]) + (tg_dot[b+4] + tg_dot[b+5]) + (tg_dot[b+6] + tg_dot[b+7]);
      iv = rsqrt(tss * inv_d + 1.0e-6f);
      sc = tdot * iv * inv_sqrt_d;
    }
    float mx = simd_max(sc);
    float w = ((int)tid < n_states) ? exp(sc - mx) : 0.0f;
    float inv_e = 1.0f / simd_sum(w);
    if ((int)tid < n_states) tg_w[tid] = w * inv_e * iv;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  device float *o0p = out0.data + o0_off;
  float s1 = 0.0f, s2 = 0.0f;
  for (int i = (int)tid; i < (int)dim; i += 256) {
    float acc = 0.0f;
    for (int s = 0; s < n_states; s++) {
      const device float *vp = (s < (int)n_blocks) ? (blocks_buf.data + ((int64_t)s * nb + t_idx) * dim) : (cur.data + cbase);
      acc += tg_w[s] * vp[i];
    }
    o0p[i] = acc;
    s1 += acc * acc;
    if (mode != 0) { float mv = aux.data[aux_off + i]; s2 += mv * mv; }
  }
  float ts1 = simd_sum(s1), ts2 = simd_sum(s2);
  if (lane == 0) { tg_r1[sg] = ts1; tg_r2[sg] = ts2; }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  float tot1 = (tg_r1[0] + tg_r1[1]) + (tg_r1[2] + tg_r1[3]) + (tg_r1[4] + tg_r1[5]) + (tg_r1[6] + tg_r1[7]);
  if (mode == 0) {
    float inv = rsqrt(tot1 / (float)dim + eps);
    const device float *nwp = aux.data + aux_off;
    for (int i = (int)tid; i < (int)dim; i += 256) o0p[i] = o0p[i] * inv * nwp[i];
  } else {
    float tot2 = (tg_r2[0] + tg_r2[1]) + (tg_r2[2] + tg_r2[3]) + (tg_r2[4] + tg_r2[5]) + (tg_r2[6] + tg_r2[7]);
    float inv1 = rsqrt(tot1 / (float)dim + 1.0e-6f);
    float inv2 = rsqrt(tot2 / (float)dim + 1.0e-6f);
    const device float *mp = aux.data + aux_off;
    device float *o1p = out1.data + o1_off;
    for (int i = (int)tid; i < (int)dim; i += 256) { o1p[i] = o0p[i] * inv1; o1p[dim + i] = mp[i] * inv2; }
  }
}
COAX_DEV void coax_am_ctrl_up_apply_gf_rm_nb(int64_t dim, int64_t rank, int64_t nb, int64_t row0, coax_v_u32 w_gf, int64_t w_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, coax_v_f32 w_norm, coax_v_f32 out, int64_t o_off, coax_v_f32 rms_parts, int64_t p_base, int64_t tile, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int64_t n_parts = (dim + 31) >> 5;
  if (dim <= 0 || rank < 0 || rank > 64 || nb < 1 || nb > 8 || row0 < 0 || w_off < 0 || w_off + dim * rank > w_gf.count || g_off < 0 || g_off + nb * rank > gate.count || c_off < 0 || c_off + nb * dim > cur.count || m_off < 0 || m_off + nb * dim > msg.count || dim > w_norm.count || o_off < 0 || o_off + nb * dim > out.count || p_base < 0 || tile < 0 || tile >= n_parts || p_base + nb * n_parts > rms_parts.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int inb = (int)nb, irank = (int)rank;
  int total_g = inb * irank;
  for (int i = (int)tid; i < total_g; i += 128) tg[i] = gate.data[g_off + i];
  threadgroup_barrier(mem_flags::mem_threadgroup);
  uint32_t lane = tid & 31u, sg = tid >> 5;
  int64_t base = row0 + (int64_t)(sg * 8u);
  if (row0 + 32 <= dim && irank == 64 && (((uintptr_t)(w_gf.data + w_off)) & 7u) == 0) {
    const device half4 *w4 = (const device half4 *)(w_gf.data + w_off);
    float s8[8];
    for (int j = 0; j < 8; j++) {
      half4 p0 = w4[(base + j) * 32 + (int)lane];
      float px = float(p0.x), py = float(p0.y), pz = float(p0.z), pw = float(p0.w);
      float r_sg = 0.0f, r_sf = 0.0f;
      for (int t = 0; t < inb; t++) {
        float g0 = tg[t * 64 + (int)(lane * 2u)], g1 = tg[t * 64 + (int)(lane * 2u + 1u)];
        float sg0 = simd_sum(px * g0 + pz * g1);
        float sf0 = simd_sum(py * g0 + pw * g1);
        if ((int)lane == t) { r_sg = sg0; r_sf = sf0; }
      }
      float vv = 0.0f;
      if ((int)lane < inb) {
        int64_t r = base + j;
        int64_t idx = (int64_t)lane * dim + r;
        float c = cur.data[c_off + idx], m = msg.data[m_off + idx];
        vv = c - tanh(r_sf) * c + tanh(r_sg) * m;
        out.data[o_off + idx] = vv * w_norm.data[r];
      }
      s8[j] = vv * vv;
    }
    if ((int)lane < inb) {
      tg[512 + sg * 8u + lane] = ((s8[0] + s8[1]) + (s8[2] + s8[3])) + ((s8[4] + s8[5]) + (s8[6] + s8[7]));
    }
  } else {
    const device half2 *w2 = (const device half2 *)(w_gf.data + w_off);
    if (lane == 0u) { for (int t = 0; t < inb; t++) tg[512 + sg * 8u + (uint32_t)t] = 0.0f; }
    for (int j = 0; j < 8; j++) {
      int64_t r = base + j;
      if (r < dim) {
        float nw = w_norm.data[r];
        for (int t = 0; t < inb; t++) {
          float psg = 0.0f, psf = 0.0f;
          for (int k = (int)lane; k < irank; k += 32) {
            float gv = tg[t * irank + k];
            half2 p = w2[r * irank + k];
            psg += float(p.x) * gv;
            psf += float(p.y) * gv;
          }
          float sgv = simd_sum(psg), sfv = simd_sum(psf);
          if (lane == 0u) {
            int64_t idx = (int64_t)t * dim + r;
            float c = cur.data[c_off + idx], m = msg.data[m_off + idx];
            float v = c - tanh(sfv) * c + tanh(sgv) * m;
            out.data[o_off + idx] = v * nw;
            tg[512 + sg * 8u + (uint32_t)t] += v * v;
          }
        }
      }
    }
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if ((int)tid < inb) {
    int t = (int)tid;
    rms_parts.data[p_base + (int64_t)t * n_parts + tile] = (tg[512 + t] + tg[520 + t]) + (tg[528 + t] + tg[536 + t]);
  }
}
template <int TM, int TN, int SG, typename XT, typename XV>
COAX_DEV void coax_am_matmul_iq4_impl(int64_t rows, int64_t cols, int64_t tokens, int64_t r0, int64_t t0, coax_v_u16 w, XV x, coax_v_f32 out, threadgroup bfloat *W_tg, uint32_t tid, int32_t line, COAX_ERR) {
  using namespace mpp::tensor_ops;
  int64_t n_blocks = cols >> 5;
  int64_t row_words = n_blocks * 9;
  if (rows < 1 || cols < 64 || (cols & 63) != 0 || tokens < 1 || r0 < 0 || r0 >= rows || t0 < 0 || t0 >= tokens || rows > 2147483647 / cols ||
      tokens > 2147483647 / cols || tokens > 2147483647 / rows || w.count < rows * row_words || x.count < tokens * cols || out.count < tokens * rows) {
    coax_trap_(line, coax_err);
    return;
  }
  const device XT *xdata = (const device XT *)x.data;
  int c = (int)cols, nt = (int)min((int64_t)TM, tokens - t0), nr = (int)min((int64_t)TN, rows - r0);
  constexpr int SUB_M = (TM == 128 || TM == 256) ? 64 : TM;
  int nt0 = min(SUB_M, nt);
  int nt1 = (TM >= 128 && nt > 64) ? min(64, nt - 64) : 0;
  int nt2 = (TM == 256 && nt > 128) ? min(64, nt - 128) : 0;
  int nt3 = (TM == 256 && nt > 192) ? min(64, nt - 192) : 0;
  auto X0 = tensor<device XT, dextents<int32_t, 2>, tensor_inline>((device XT *)(xdata + t0 * cols), dextents<int32_t, 2>(64, nt0), array<int32_t, 2>({1, c}));
  auto W0 = tensor<threadgroup bfloat, dextents<int32_t, 2>, tensor_inline>(W_tg, dextents<int32_t, 2>(64, nr), array<int32_t, 2>({1, 64}));
  auto C0 = tensor<device float, dextents<int32_t, 2>, tensor_inline>(out.data + t0 * rows + r0, dextents<int32_t, 2>(nr, nt0), array<int32_t, 2>({1, (int)rows}));
  constexpr auto d = matmul2d_descriptor(SUB_M, TN, 64, false, true, true, matmul2d_descriptor::mode::multiply_accumulate);
  matmul2d<d, execution_simdgroups<SG>> op;
  auto ct0 = op.template get_destination_cooperative_tensor<decltype(X0), decltype(W0), float>();
  auto ct1 = op.template get_destination_cooperative_tensor<decltype(X0), decltype(W0), float>();
  auto ct2 = op.template get_destination_cooperative_tensor<decltype(X0), decltype(W0), float>();
  auto ct3 = op.template get_destination_cooperative_tensor<decltype(X0), decltype(W0), float>();
  for (int i = 0; i < ct0.get_capacity(); i++) {
    ct0[i] = 0.0f;
    if constexpr (TM >= 128) ct1[i] = 0.0f;
    if constexpr (TM == 256) { ct2[i] = 0.0f; ct3[i] = 0.0f; }
  }
  const uint32_t nthreads = (uint32_t)(SG * 32);
  const uint32_t total_blocks = (uint32_t)(TN * 2);
  for (int k0 = 0; k0 < c; k0 += 64) {
    int kb = k0 >> 5;
    for (uint32_t bi = tid; bi < total_blocks; bi += nthreads) {
      int r = (int)(bi >> 1);
      int b = (int)(bi & 1);
      threadgroup bfloat *dst = W_tg + r * 64 + b * 32;
      if (r < nr) {
        const device uint16_t *src = w.data + (r0 + r) * row_words + (kb + b) * 9;
        float sc = float(as_type<half>(src[0]));
        #pragma unroll
        for (int k = 0; k < 8; k++) {
          uint32_t v = (uint32_t)src[1 + k];
          dst[2 * k]      = bfloat(sc * coax_iq4_lut[v & 15u]);
          dst[2 * k + 16] = bfloat(sc * coax_iq4_lut[(v >> 4u) & 15u]);
          dst[2 * k + 1]  = bfloat(sc * coax_iq4_lut[(v >> 8u) & 15u]);
          dst[2 * k + 17] = bfloat(sc * coax_iq4_lut[v >> 12u]);
        }
      } else {
        for (int k = 0; k < 32; k++) dst[k] = bfloat(0.0f);
      }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    auto Xk0 = tensor<device XT, dextents<int32_t, 2>, tensor_inline>((device XT *)(xdata + t0 * cols + k0), dextents<int32_t, 2>(64, nt0), array<int32_t, 2>({1, c}));
    op.run(Xk0, W0, ct0);
    if constexpr (TM >= 128) {
      if (nt1 > 0) {
        auto Xk1 = tensor<device XT, dextents<int32_t, 2>, tensor_inline>((device XT *)(xdata + (t0 + 64) * cols + k0), dextents<int32_t, 2>(64, nt1), array<int32_t, 2>({1, c}));
        op.run(Xk1, W0, ct1);
      }
    }
    if constexpr (TM == 256) {
      if (nt2 > 0) {
        auto Xk2 = tensor<device XT, dextents<int32_t, 2>, tensor_inline>((device XT *)(xdata + (t0 + 128) * cols + k0), dextents<int32_t, 2>(64, nt2), array<int32_t, 2>({1, c}));
        op.run(Xk2, W0, ct2);
      }
      if (nt3 > 0) {
        auto Xk3 = tensor<device XT, dextents<int32_t, 2>, tensor_inline>((device XT *)(xdata + (t0 + 192) * cols + k0), dextents<int32_t, 2>(64, nt3), array<int32_t, 2>({1, c}));
        op.run(Xk3, W0, ct3);
      }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
  }
  ct0.store(C0);
  if constexpr (TM >= 128) {
    if (nt1 > 0) {
      auto C1 = tensor<device float, dextents<int32_t, 2>, tensor_inline>(out.data + (t0 + 64) * rows + r0, dextents<int32_t, 2>(nr, nt1), array<int32_t, 2>({1, (int)rows}));
      ct1.store(C1);
    }
  }
  if constexpr (TM == 256) {
    if (nt2 > 0) {
      auto C2 = tensor<device float, dextents<int32_t, 2>, tensor_inline>(out.data + (t0 + 128) * rows + r0, dextents<int32_t, 2>(nr, nt2), array<int32_t, 2>({1, (int)rows}));
      ct2.store(C2);
    }
    if (nt3 > 0) {
      auto C3 = tensor<device float, dextents<int32_t, 2>, tensor_inline>(out.data + (t0 + 192) * rows + r0, dextents<int32_t, 2>(nr, nt3), array<int32_t, 2>({1, (int)rows}));
      ct3.store(C3);
    }
  }
}
template <int TM, int TN, int SG>
COAX_DEV void coax_am_matmul_iq4(int64_t rows, int64_t cols, int64_t tokens, int64_t r0, int64_t t0, coax_v_u16 w, coax_v_f32 x, coax_v_f32 out, threadgroup bfloat *W_tg, uint32_t tid, int32_t line, COAX_ERR) {
  coax_am_matmul_iq4_impl<TM, TN, SG, float>(rows, cols, tokens, r0, t0, w, x, out, W_tg, tid, line, coax_err);
}
template <int TM, int TN, int SG>
COAX_DEV void coax_am_matmul_iq4_bf16(int64_t rows, int64_t cols, int64_t tokens, int64_t r0, int64_t t0, coax_v_u16 w, coax_v_u16 x, coax_v_f32 out, threadgroup bfloat *W_tg, uint32_t tid, int32_t line, COAX_ERR) {
  coax_am_matmul_iq4_impl<TM, TN, SG, bfloat>(rows, cols, tokens, r0, t0, w, x, out, W_tg, tid, line, coax_err);
}
COAX_DEV void coax_am_ctrl_up_apply_gf_batch(int64_t dim, int64_t rank, int64_t n_tok, int64_t t0, int64_t row0, coax_v_u32 w_gf, int64_t w_off, coax_v_f32 gate, int64_t g_off, coax_v_f32 cur, int64_t c_off, coax_v_f32 msg, int64_t m_off, coax_v_f32 out, int64_t o_off, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  if (dim <= 0 || rank < 0 || rank > 64 || n_tok <= 0 || t0 < 0 || row0 < 0 || w_off < 0 || w_off + rank * dim > w_gf.count || g_off < 0 || g_off + n_tok * rank > gate.count || c_off < 0 || c_off + n_tok * dim > cur.count || m_off < 0 || m_off + n_tok * dim > msg.count || o_off < 0 || o_off + n_tok * dim > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int idim = (int)dim, irank = (int)rank;
  int nt = (int)min((int64_t)16, n_tok - t0);
  /* Load gate[t0..t0+nt-1, 0..rank-1] into tg[k * 16 + m] (1024 floats = 8 floats/thread). */
  for (int idx = (int)tid; idx < irank * 16; idx += 128) {
    int k = idx >> 4;
    int m = idx & 15;
    tg[idx] = (m < nt) ? gate.data[g_off + (t0 + m) * rank + k] : 0.0f;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  int half_g = (int)(tid >> 6);
  int row = (int)row0 + (int)(tid & 63u);
  int m0 = half_g * 8;
  int mc = nt - m0;
  if (mc > 8) mc = 8;
  if (row < idim && mc > 0) {
    const device half2 *wp = (const device half2 *)(w_gf.data + w_off) + row;
    float2 a0 = float2(0.0f), a1 = float2(0.0f), a2 = float2(0.0f), a3 = float2(0.0f);
    float2 a4 = float2(0.0f), a5 = float2(0.0f), a6 = float2(0.0f), a7 = float2(0.0f);
    for (int k = 0; k < irank; k++) {
      float2 wv = float2(wp[k * idim]);
      float4 g0 = *(const threadgroup float4 *)(tg + k * 16 + m0);
      float4 g1 = *(const threadgroup float4 *)(tg + k * 16 + m0 + 4);
      a0 += wv * g0.x; a1 += wv * g0.y; a2 += wv * g0.z; a3 += wv * g0.w;
      a4 += wv * g1.x; a5 += wv * g1.y; a6 += wv * g1.z; a7 += wv * g1.w;
    }
    float2 accs[8] = {a0, a1, a2, a3, a4, a5, a6, a7};
    for (int j = 0; j < mc; j++) {
      int64_t off = (t0 + m0 + j) * dim + row;
      float c = cur.data[c_off + off];
      float mv = msg.data[m_off + off];
      out.data[o_off + off] = c - tanh(accs[j].y) * c + tanh(accs[j].x) * mv;
    }
  }
}
COAX_DEV void coax_am_mix_batch_f32(int64_t dim, int64_t n_blocks, int64_t n_tok, int64_t q_idx, int64_t t_idx, coax_v_f32 blocks_buf, coax_v_f32 cur, coax_v_f32 query, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  int ns = (int)n_blocks + 1;
  if (dim <= 0 || n_blocks < 0 || ns > 16 || n_tok <= 0 || q_idx < 0 || t_idx < 0 || t_idx >= n_tok || n_blocks * n_tok * dim > blocks_buf.count || n_tok * dim > cur.count || (q_idx + 1) * dim > query.count || (q_idx * n_tok + t_idx + 1) * dim > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  uint32_t lane = tid & 31u;
  uint32_t sg = tid >> 5;
  int n4 = (int)(dim >> 2);
  int64_t qbase = q_idx * dim;
  int64_t cbase = t_idx * dim;
  int64_t obase = (q_idx * n_tok + t_idx) * dim;
  const device float4 *q4 = (const device float4 *)(query.data + qbase);
  const device float4 *c4 = (const device float4 *)(cur.data + cbase);
  device float4 *o4 = (device float4 *)(out.data + obase);
  float4 qv[3];
  int qi = 0;
  for (int i = (int)tid; i < n4 && qi < 3; i += 256) qv[qi++] = q4[i];
  for (int s = 0; s < ns; s++) {
    const device float4 *s4 = (s < (int)n_blocks) ? (const device float4 *)(blocks_buf.data + (s * n_tok + t_idx) * dim) : c4;
    float4 ss4 = float4(0.0f), dt4 = float4(0.0f);
    qi = 0;
    for (int i = (int)tid; i < n4; i += 256) {
      float4 v = s4[i];
      float4 qx = (qi < 3) ? qv[qi++] : q4[i];
      ss4 += v * v;
      dt4 += v * qx;
    }
    float ss = (ss4.x + ss4.y) + (ss4.z + ss4.w);
    float dt = (dt4.x + dt4.y) + (dt4.z + dt4.w);
    int tail = n4 << 2;
    if (tail + (int)tid < (int)dim) {
      int k = tail + (int)tid;
      float v = (s < (int)n_blocks) ? blocks_buf.data[(s * n_tok + t_idx) * dim + k] : cur.data[cbase + k];
      ss += v * v;
      dt += v * query.data[qbase + k];
    }
    float wss = simd_sum(ss), wdt = simd_sum(dt);
    if (lane == 0) { tg[s * 16 + sg] = wss; tg[s * 16 + 8 + sg] = wdt; }
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid < (uint32_t)ns) {
    float tss = 0.0f, tdt = 0.0f;
    for (int w = 0; w < 8; w++) { tss += tg[tid * 16 + w]; tdt += tg[tid * 16 + 8 + w]; }
    float inv = rsqrt(tss / float(dim) + 1.0e-6f);
    tg[256 + tid] = inv;
    tg[272 + tid] = tdt * inv * rsqrt(float(dim));
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  if (tid == 0) {
    float mx = tg[272];
    for (int s = 1; s < ns; s++) if (tg[272 + s] > mx) mx = tg[272 + s];
    float esum = 0.0f;
    for (int s = 0; s < ns; s++) { float w = exp(tg[272 + s] - mx); tg[288 + s] = w; esum += w; }
    float inv_e = 1.0f / esum;
    for (int s = 0; s < ns; s++) tg[288 + s] *= inv_e * tg[256 + s];
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);
  float wts[16];
  for (int s = 0; s < ns; s++) wts[s] = tg[288 + s];
  for (int i = (int)tid; i < n4; i += 256) {
    float4 acc = float4(0.0f);
    for (int s = 0; s < ns; s++) {
      const device float4 *s4 = (s < (int)n_blocks) ? (const device float4 *)(blocks_buf.data + (s * n_tok + t_idx) * dim) : c4;
      acc += wts[s] * s4[i];
    }
    o4[i] = acc;
  }
  int tail = n4 << 2;
  if (tail + (int)tid < (int)dim) {
    int k = tail + (int)tid;
    float acc = 0.0f;
    for (int s = 0; s < ns; s++) {
      float v = (s < (int)n_blocks) ? blocks_buf.data[(s * n_tok + t_idx) * dim + k] : cur.data[cbase + k];
      acc += wts[s] * v;
    }
    out.data[obase + k] = acc;
  }
}
COAX_DEV void coax_am_attn_scores_m4(int64_t item, uint32_t dims0, uint32_t dims1, uint32_t kv_info, int64_t pos0, int64_t n_tok, int64_t k_start, coax_v_f32 q, coax_v_f32 r, coax_v_f32 rel_bias, coax_v_f32 sinks, coax_v_u16 k_cache, coax_v_u16 scores, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  using namespace mpp::tensor_ops;
  int n_head = (int)(dims0 & 255u);
  int n_head_kv = (int)((dims0 >> 8u) & 255u);
  int head_dim = (int)((dims0 >> 16u) & 255u);
  int n_rel = (int)((dims0 >> 24u) & 255u);
  int extent = (int)(dims1 & 65535u);
  int is_swa = (int)((dims1 >> 16u) & 1u);
  int swa = (int)((dims1 >> 17u) & 32767u);
  int kv_cap = (int)(kv_info & 65535u);
  int score_stride = (int)((kv_info >> 16u) & 65535u);
  if (head_dim != 128 || n_rel != 16 || n_head < 1 || n_head_kv < 1 || n_tok < 1 || (int64_t)n_tok * n_head * 128 > q.count || (int64_t)n_tok * n_head * 16 > r.count || (int64_t)extent * 16 > rel_bias.count || (int64_t)n_tok * n_head > sinks.count || (int64_t)kv_cap * n_head_kv * 128 > k_cache.count || (int64_t)n_head * n_tok * score_stride > scores.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int span = (int)(pos0 + n_tok - k_start);
  int qt = (int)((n_tok + 63) / 64);
  int k_split = (int)(item & 3);
  int hq = (int)(item >> 2);
  int h = hq / qt;
  int t0 = (hq % qt) * 64;
  int nt = (int)min((int64_t)64, n_tok - t0);
  int last_t = t0 + nt - 1;
  int kv_h = h / (n_head / n_head_kv);
  int n_kv_elem = n_head_kv * 128;
  int64_t min_k = (is_swa && pos0 + t0 >= swa) ? (pos0 + t0 - swa + 1) : 0;
  int j_lo_tile = (min_k > k_start) ? (int)((min_k - k_start) / 32) : 0;
  int j_hi_val = (int)(pos0 + last_t - k_start);
  int j_hi_tile = (j_hi_val >= 0) ? (j_hi_val / 32) : -1;
  int n_valid = j_hi_tile - j_lo_tile + 1;
  if (n_valid <= 0) return;

  threadgroup half *Q_tg = (threadgroup half *)tg;           /* 64 * 128 = 8192 halfs (4096 floats) */
  threadgroup float *S_mat = tg + 4096;                      /* 64 * 32  = 2048 floats (also K_tg 32 * 128 halfs) */
  threadgroup half *R_tg = (threadgroup half *)(tg + 6144);  /* 64 * 16  = 1024 halfs (512 floats) */
  threadgroup half *RB_tg = (threadgroup half *)(tg + 6656); /* 96 * 16  = 1536 halfs (768 floats) */
  threadgroup float *sinks_tg = tg + 7424;                   /* 64 floats */

  /* Load Q[t0..t0+nt-1, h, 0..127], R[t0..t0+nt-1, h, 0..15], sinks[t0..t0+nt-1, h] ONCE for all k_tiles! */
  for (int idx = (int)tid; idx < 2048; idx += 128) {
    int dt = idx >> 5;
    int d4 = (idx & 31) << 2;
    half4 hv = half4(0.0h);
    if (dt < nt) {
      hv = half4(*(const device float4 *)(q.data + ((int64_t)(t0 + dt) * n_head + h) * 128 + d4));
    }
    *(threadgroup half4 *)(Q_tg + dt * 128 + d4) = hv;
  }
  for (int idx = (int)tid; idx < 256; idx += 128) {
    int dt = idx >> 2;
    int m4 = (idx & 3) << 2;
    half4 hv = half4(0.0h);
    if (dt < nt) {
      hv = half4(*(const device float4 *)(r.data + ((int64_t)(t0 + dt) * n_head + h) * 16 + m4));
    }
    *(threadgroup half4 *)(R_tg + dt * 16 + m4) = hv;
  }
  if ((int)tid < 64) {
    sinks_tg[tid] = ((int)tid < nt) ? sinks.data[(int64_t)(t0 + (int)tid) * n_head + h] : 0.0f;
  }
  threadgroup_barrier(mem_flags::mem_threadgroup);

  float scale = 1.0f / sqrt(128.0f);
  int step_k = (n_valid + 3) / 4;
  int kt_start = j_lo_tile + k_split * step_k;
  int kt_end = min(kt_start + step_k, j_hi_tile + 1);
  auto tQ = tensor<threadgroup half, dextents<int32_t, 2>, tensor_inline>(Q_tg, dextents<int32_t, 2>(128, nt), array<int32_t, 2>({1, 128}));
  constexpr auto d_qk = matmul2d_descriptor(64, 32, 128, false, true, true);
  matmul2d<d_qk, execution_simdgroups<4>> op;
  for (int kt = kt_start; kt < kt_end; kt++) {
    int j0 = kt * 32;
    int nk = min(32, span - j0);
    if (nk <= 0) continue;
    int slot0 = is_swa ? ((int)(k_start + j0) % kv_cap) : (int)(k_start + j0);
    auto tS = tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline>(S_mat, dextents<int32_t, 2>(nk, nt), array<int32_t, 2>({1, 32}));
    if (!is_swa || slot0 + nk <= kv_cap) {
      auto tK = tensor<device half, dextents<int32_t, 2>, tensor_inline>((device half *)(k_cache.data + (int64_t)slot0 * n_kv_elem + kv_h * 128), dextents<int32_t, 2>(128, nk), array<int32_t, 2>({1, n_kv_elem}));
      op.run(tQ, tK, tS);
    } else {
      threadgroup half *K_tg = (threadgroup half *)S_mat;
      for (int idx = (int)tid; idx < 1024; idx += 128) {
        int dj = idx >> 5;
        int d4 = (idx & 31) << 2;
        half4 hv = half4(0.0h);
        if (dj < nk) {
          int sl = (int)(k_start + j0 + dj) % kv_cap;
          hv = *(const device half4 *)(k_cache.data + (int64_t)sl * n_kv_elem + kv_h * 128 + d4);
        }
        *(threadgroup half4 *)(K_tg + dj * 128 + d4) = hv;
      }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      auto tK_tg = tensor<threadgroup half, dextents<int32_t, 2>, tensor_inline>(K_tg, dextents<int32_t, 2>(128, nk), array<int32_t, 2>({1, 128}));
      auto ct = op.template get_destination_cooperative_tensor<decltype(tQ), decltype(tK_tg), float>();
      op.run(tQ, tK_tg, ct);
      threadgroup_barrier(mem_flags::mem_threadgroup);
      ct.store(tS);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    bool no_rb = (k_start + j0 + 31 <= pos0 + t0) && (!is_swa || (pos0 + last_t) - (k_start + j0) < swa) && ((pos0 + t0) - (k_start + j0 + 31) >= extent);
    if (no_rb) {
      for (int e = (int)tid; e < nt * 32; e += 128) {
        int dt = e >> 5;
        int dj = e & 31;
        if (dj < nk) {
          scores.data[((int64_t)h * n_tok + (t0 + dt)) * score_stride + (j0 + dj)] = as_type<ushort>((half)(S_mat[e] * scale - sinks_tg[dt]));
        }
      }
    } else {
      int d_lo = (int)((pos0 + t0) - (k_start + j0) - 31);
      for (int idx = (int)tid; idx < 384; idx += 128) {
        int di = idx >> 2;
        int m4 = (idx & 3) << 2;
        int dist = d_lo + di;
        half4 hv = (dist >= 0 && dist < extent) ? half4(*(const device float4 *)(rel_bias.data + (int64_t)dist * 16 + m4)) : half4(0.0h);
        *(threadgroup half4 *)(RB_tg + di * 16 + m4) = hv;
      }
      threadgroup_barrier(mem_flags::mem_threadgroup);
      for (int e = (int)tid; e < nt * 32; e += 128) {
        int dt = e >> 5;
        int dj = e & 31;
        if (dj < nk) {
          int64_t p = pos0 + t0 + dt;
          int64_t kp = k_start + j0 + dj;
          if (kp <= p && (!is_swa || p - kp < swa)) {
            int dist = (int)(p - kp);
            float rb = 0.0f;
            if (dist < extent) {
              int di = dist - d_lo;
              const threadgroup half4 *rp = (const threadgroup half4 *)(R_tg + dt * 16);
              const threadgroup half4 *bp = (const threadgroup half4 *)(RB_tg + di * 16);
              rb = dot(float4(rp[0]), float4(bp[0])) + dot(float4(rp[1]), float4(bp[1])) + dot(float4(rp[2]), float4(bp[2])) + dot(float4(rp[3]), float4(bp[3]));
            }
            scores.data[((int64_t)h * n_tok + (t0 + dt)) * score_stride + (j0 + dj)] = as_type<ushort>((half)(S_mat[e] * scale + rb - sinks_tg[dt]));
          }
        }
      }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
  }
}
COAX_DEV void coax_am_attn_values_m4(int64_t item, uint32_t dims0, uint32_t dims1, uint32_t kv_info, int64_t pos0, int64_t n_tok, int64_t k_start, coax_v_u16 scores, coax_v_f32 inv_gsum, coax_v_u16 v_cache, coax_v_f32 gate, coax_v_f32 out, uint32_t tid, threadgroup float *tg, int32_t line, COAX_ERR) {
  using namespace mpp::tensor_ops;
  int n_head = (int)(dims0 & 255u);
  int n_head_kv = (int)((dims0 >> 8u) & 255u);
  int head_dim = (int)((dims0 >> 16u) & 255u);
  int is_swa = (int)((dims1 >> 16u) & 1u);
  int swa = (int)((dims1 >> 17u) & 32767u);
  int kv_cap = (int)(kv_info & 65535u);
  int score_stride = (int)((kv_info >> 16u) & 65535u);
  if (head_dim != 128 || n_head < 1 || n_head_kv < 1 || n_tok < 1 || (int64_t)n_head * n_tok * score_stride > scores.count || (int64_t)n_tok * n_head > inv_gsum.count || (int64_t)kv_cap * n_head_kv * 128 > v_cache.count || (int64_t)n_tok * n_head * 128 > gate.count || (int64_t)n_tok * n_head * 128 > out.count) {
    coax_trap_(line, coax_err);
    return;
  }
  int span = (int)(pos0 + n_tok - k_start);
  int qt = (int)((n_tok + 63) / 64);
  int per = qt * 2;
  int h = (int)(item / per);
  int rem = (int)(item % per);
  int t0 = (rem >> 1) * 64;
  int c0 = (rem & 1) * 64;
  int nt = (int)min((int64_t)64, n_tok - t0);
  int last_t = t0 + nt - 1;
  int kv_h = h / (n_head / n_head_kv);
  int n_kv_elem = n_head_kv * 128;
  int j_lo = (is_swa && pos0 + t0 >= swa) ? (int)(pos0 + t0 - swa + 1 - k_start) : 0;
  if (j_lo < 0) j_lo = 0;
  int j_hi = min((int)(pos0 + last_t - k_start + 1), span);
  int k_len = j_hi - j_lo;

  constexpr auto dv = matmul2d_descriptor(64, 64, static_cast<int>(dynamic_extent), false, false, true, matmul2d_descriptor::mode::multiply_accumulate);
  matmul2d<dv, execution_simdgroups<4>> op;
  using DevHalf2D = tensor<device half, dextents<int32_t, 2>, tensor_inline>;
  auto ct = op.template get_destination_cooperative_tensor<DevHalf2D, DevHalf2D, float>();
  for (int i = 0; i < ct.get_capacity(); i++) ct[i] = 0.0f;
  if (k_len > 0) {
    int slot_lo = is_swa ? ((int)(k_start + j_lo) % kv_cap) : (int)(k_start + j_lo);
    int len1 = (is_swa && slot_lo + k_len > kv_cap) ? (kv_cap - slot_lo) : k_len;
    int len2 = k_len - len1;
    auto tP1 = DevHalf2D((device half *)(scores.data + ((int64_t)h * n_tok + t0) * score_stride + j_lo), dextents<int32_t, 2>(len1, nt), array<int32_t, 2>({1, score_stride}));
    auto tV1 = DevHalf2D((device half *)(v_cache.data + (int64_t)slot_lo * n_kv_elem + kv_h * 128 + c0), dextents<int32_t, 2>(64, len1), array<int32_t, 2>({1, n_kv_elem}));
    op.run(tP1, tV1, ct);
    if (len2 > 0) {
      auto tP2 = DevHalf2D((device half *)(scores.data + ((int64_t)h * n_tok + t0) * score_stride + j_lo + len1), dextents<int32_t, 2>(len2, nt), array<int32_t, 2>({1, score_stride}));
      auto tV2 = DevHalf2D((device half *)(v_cache.data + kv_h * 128 + c0), dextents<int32_t, 2>(64, len2), array<int32_t, 2>({1, n_kv_elem}));
      op.run(tP2, tV2, ct);
    }
  }
  auto tO = tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline>(tg, dextents<int32_t, 2>(64, nt), array<int32_t, 2>({1, 64}));
  ct.store(tO);
  threadgroup_barrier(mem_flags::mem_threadgroup);
  for (int idx = (int)tid; idx < nt * 16; idx += 128) {
    int dt = idx >> 4;
    int dc = (idx & 15) << 2;
    float ig = inv_gsum.data[(int64_t)(t0 + dt) * n_head + h];
    int64_t oidx = ((int64_t)(t0 + dt) * n_head + h) * 128 + c0 + dc;
    float4 ov = *(const threadgroup float4 *)(tg + dt * 64 + dc);
    float4 gv = *(const device float4 *)(gate.data + oidx);
    *(device float4 *)(out.data + oidx) = (ov * ig) / (float4(1.0f) + exp(-gv));
  }
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
    for(const SwitchCase *arm:st->cases)types_stmt(arm->body);
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
#ifndef COAX_METAL_FAST_MATH
#pragma METAL fp math_mode(safe)
#pragma METAL fp contract(off)
#endif
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
/* The 32-bit body's check: n is below 2^31 (a fixed length, or a view count the entry checked). */
COAX_DEV int32_t coax_index32_(int32_t i, int32_t n, int32_t l, COAX_ERR) { if ((uint32_t)i >= (uint32_t)n) { coax_trap_(l, coax_err); return 0; } return i; }
#define coax_index32(i, n, l) coax_index32_(i, n, l, coax_err)
/* As coax_div_s64 and coax_mod_s64 where the quotient fits: zero traps and yields 0, and x % -1 is 0. */
COAX_DEV int32_t coax_div32_(int32_t a, int32_t b, int32_t l, COAX_ERR) { if (b == 0) { coax_trap_(l, coax_err); return 0; } return a / b; }
COAX_DEV int32_t coax_mod32_(int32_t a, int32_t b, int32_t l, COAX_ERR) { if (b == 0) { coax_trap_(l, coax_err); return 0; } if (b == -1) return 0; return a % b; }
#define coax_div32(a, b, l) coax_div32_(a, b, l, coax_err)
#define coax_mod32(a, b, l) coax_mod32_(a, b, l, coax_err)
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
  std::string functions, narrow_prototypes;
  g_narrow.clear();
  for (const ProcDecl *p : procs) {
    functions += em.function(p);
    if (!p->export_c || !p->params.count || p->params[0].type != T_INT) continue;
    Narrow &n = g_narrow[p];
    n.analyze(p);
    if (!n.any) { g_narrow.erase(p); continue; }
    em.n32 = &n;
    functions += em.function(p);
    narrow_prototypes += em.prototype(p) + ";\n";
    em.n32 = nullptr;
  }
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
    for(const SwitchCase *arm:st->cases)types_stmt(arm->body);
  };
  for (const ProcDecl *p : procs) {
    add(p->ret);
    for (const Param &prm : p->params) add(prm.type);
    types_stmt(p->body);
  }
  // Arrays and views hold scalars only (admission), so they never depend on each other.
  std::string types, helpers;
  bool has_v_u16 = false, has_v_u32 = false, has_v_f32 = false;
  for (Type t : used) {
    std::string c = c_type(t), e = c_type(t->elem), tg = tag(t);
    if (t->kind == TypeKind::array) { types += "typedef struct { " + e + " e[" + std::to_string(t->length) + "]; } " + c + ";\n"; continue; }
    if (c == "coax_v_u16") has_v_u16 = true;
    else if (c == "coax_v_u32") has_v_u32 = true;
    else if (c == "coax_v_f32") has_v_f32 = true;
    types += "typedef struct { device " + e + " *data; int64_t count; } " + c + ";\n";
    helpers += "COAX_DEV device " + e + " *coax_elem_" + tg + "_(" + c + " v, int64_t i, int32_t l, COAX_ERR) { return &v.data[coax_index(i, v.count, l)]; }\n";
    helpers += "#define coax_elem_" + tg + "(v, i, l) coax_elem_" + tg + "_(v, i, l, coax_err)\n";
    helpers += "COAX_DEV device " + e + " *coax_elem32_" + tg + "_(" + c + " v, int32_t i, int32_t l, COAX_ERR) { return &v.data[coax_index32(i, (int32_t)v.count, l)]; }\n";
    helpers += "#define coax_elem32_" + tg + "(v, i, l) coax_elem32_" + tg + "_(v, i, l, coax_err)\n";
    helpers += "COAX_DEV " + c + " coax_reslice_" + tg + "_(" + c + " v, int64_t lo, int64_t hi, int32_t l, COAX_ERR) {\n"
               "  " + c + " r = {v.data, 0};\n  if (coax_slice_ok_(lo, hi, v.count, l, coax_err)) { r.data = v.data + lo; r.count = hi - lo; }\n  return r;\n}\n";
    helpers += "#define coax_reslice_" + tg + "(v, lo, hi, l) coax_reslice_" + tg + "_(v, lo, hi, l, coax_err)\n";
  }
  if (em.apple_tensor) {
    if (!has_v_u16) types += "typedef struct { device uint16_t *data; int64_t count; } coax_v_u16;\n";
    if (!has_v_u32) types += "typedef struct { device uint32_t *data; int64_t count; } coax_v_u32;\n";
    if (!has_v_f32) types += "typedef struct { device float *data; int64_t count; } coax_v_f32;\n";
  }
  std::string s = "/* Generated by coax0 from " + file->src->path + " (coax metal-compute). Do not edit: change the .coax file. */\n";
  s += METAL_RUNTIME;
  s += "\n/* Types */\n" + types + helpers + "\n";
  bool simdgroups = false;
  for (const auto &[call, kind] : g_warp.apple_calls) simdgroups = simdgroups || kind != 3;
  if (simdgroups) s += METAL_SIMDGROUP_RUNTIME;
  if (em.apple_tensor) s += METAL_TENSOR_RUNTIME;
  for (const ProcDecl *p : procs) s += em.prototype(p) + ";\n";
  s += narrow_prototypes;
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
      if (int tgsz = proc_am_tg_size(p)) {
        s += "  threadgroup float coax_am_tg[" + std::to_string(tgsz) + "];\n";
        shared_args += ", coax_am_tg";
      }
      s += "  if (coax_tpg != " + T + "u) {  /* launched with the wrong group size: the launch is void */\n"
           "    if (coax_tid == 0) { atomic_fetch_add_explicit(&coax_errors[0], 1u, memory_order_relaxed); atomic_fetch_max_explicit(&coax_errors[1], " +
           std::to_string(p->loc.line) + "u, memory_order_relaxed); }\n    return;\n  }\n";
      s += "  int64_t coax_item = (int64_t)coax_tgid;\n  uint coax_lane = coax_tid;\n";
    } else if (warp)  // one simdgroup of 32 lanes per work item
      s += "  int64_t coax_item = (int64_t)(coax_gid >> 5);\n  uint coax_lane = coax_simd_lane;\n";
    else
      s += "  int64_t coax_item = (int64_t)coax_gid;\n  uint coax_lane = coax_simd_lane;\n";
    s += "  if (coax_item >= coax_a.coax_items) return;\n  uint coax_err[2] = {0, 0};\n";
    std::string args = (p->params.count > 1 ? ", " : ", ") + std::string("coax_lane, coax_err") + shared_args + ");\n";
    auto narrow = g_narrow.find(p);
    if (narrow != g_narrow.end()) {
      // The 32-bit body when the launch meets the bounds its intervals assumed (Narrow).
      uint64_t g = narrow->second.guard;
      std::string b = std::to_string(NARROW_BOUND), cond = "coax_a.coax_items <= " + b;
      for (int32_t i = 1; i < p->params.count && i < 63; i++)
        if (g & ((uint64_t)1 << i)) {
          std::string f = "coax_a." + em.local_name(p->params[i].name);
          cond += " && " + f + " >= 0 && " + f + " <= " + b;
        }
      if (g >> 63)
        for (int32_t i = 1; i < p->params.count; i++)
          if (p->params[i].type->kind == TypeKind::view) cond += " && coax_a." + em.local_name(p->params[i].name) + ".count <= 2147483647";
      std::string ncall = call;
      ncall.replace(0, em.proc_name(p).size(), em.proc_name(p) + "_n32");
      s += "  if (" + cond + ") " + ncall + args + "  else " + call + args;
    } else s += "  " + call + args;
    s += ""
         "  if (coax_err[0]) {\n"
         "    atomic_fetch_add_explicit(&coax_errors[0], coax_err[0], memory_order_relaxed);\n"
         "    atomic_fetch_max_explicit(&coax_errors[1], coax_err[1], memory_order_relaxed);\n  }\n}\n";
  }
  for (size_t at = 0; (at = s.find("_Bool", at)) != std::string::npos;) s.replace(at, 5, "bool");
  g_metal_compute = false;
  return s;
}

// The generic instances the program's own procedures call, directly or
// through other instances. An instance that only a check creates (a library
// check over int, or a test of the root's own) is never called by emitted code,
// so the C leaves it out (ax-library-check-cache).
static std::set<const ProcDecl *> called_instances(const File *file) {
  std::set<const ProcDecl *> reached;
  std::vector<const ProcDecl *> work;
  auto visit = [&](const ProcDecl *p) {
    if (p && p->origin && reached.insert(p).second) work.push_back(p);
  };
  std::function<void(const Expr *)> expr = [&](const Expr *e) {
    if (!e) return;
    visit(e->proc);
    visit(e->mutation_proc);
    expr(e->lhs); expr(e->rhs); expr(e->third);
    for (const Expr *a : e->args) expr(a);
    for (const FieldInit &f : e->fields) expr(f.value);
  };
  std::function<void(const Stmt *)> stmt = [&](const Stmt *st) {
    if (!st) return;
    expr(st->expr); expr(st->expr2); expr(st->target);
    for (const Stmt *c : st->body) stmt(c);
    stmt(st->then_); stmt(st->else_);
    for(const SwitchCase *arm:st->cases)stmt(arm->body);
  };
  for (const ProcDecl *p : file->procs) if (!p->origin && !p->check_unit) stmt(p->body);
  while (!work.empty()) {
    const ProcDecl *p = work.back();
    work.pop_back();
    stmt(p->body);
  }
  return reached;
}

// #inline (spec section 7.1). A procedure with the note has one body that the
// C compiler copies into each caller; a generic procedure's instances carry
// their template's note (check.cpp). main has no caller here, and a body that
// reaches itself can't be copied into itself: each is E0210.
bool inline_notes(File *file) {
  bool any = false;
  for (const ProcDecl *p : file->procs) any = any || p->inline_c;
  if (!any) return true;
  // The procedures a body calls, and through them every procedure it reaches.
  std::vector<const ProcDecl *> seen;
  std::function<void(const ProcDecl *)> reach;
  std::function<void(const Expr *)> expr = [&](const Expr *e) {
    if (!e) return;
    if (e->proc) reach(e->proc);
    expr(e->lhs); expr(e->rhs); expr(e->third);
    for (const Expr *a : e->args) expr(a);
    for (const FieldInit &f : e->fields) expr(f.value);
  };
  std::function<void(const Stmt *)> stmt = [&](const Stmt *st) {
    if (!st) return;
    expr(st->expr); expr(st->expr2); expr(st->target);
    for (const Stmt *c : st->body) stmt(c);
    stmt(st->then_); stmt(st->else_);
    for (const SwitchCase *arm : st->cases) stmt(arm->body);
  };
  reach = [&](const ProcDecl *p) {
    for (const ProcDecl *q : seen) if (q == p) return;
    seen.push_back(p);
    stmt(p->body);
  };
  bool ok = true;
  for (const ProcDecl *p : file->procs) {
    if (!p->inline_c) continue;
    std::string why;
    if (str_eq(p->name, "main")) why = "main";
    else {
      seen.clear();
      stmt(p->body);
      for (const ProcDecl *q : seen) if (q == p) why = "a procedure that calls itself, directly or through others";
    }
    if (why.empty()) continue;
    error_at(file->src, p->loc, "E0210", "#inline is not supported on " + why,
             "remove #inline from '" + to_std(p->name) + "'; the C compiler still inlines small procedures on its own");
    ok = false;
  }
  return ok;
}

// Check code and E0616. A check unit (one the entry reaches only through
// `#load "FILE" #test;`) is checked with the program and never emitted. A
// procedure that calls one of its procedures, directly or through others, is
// check code too (a check's helper that happens to sit in a product unit), so
// it is marked like them and left out with them: what is emitted then calls
// none of it. Only main and an #export_c procedure must be emitted, so one of
// those reaching a check unit is the error.
bool check_unit_calls(File *file) {
  bool any = false;
  for (const auto &unit : file->src->units) any = any || unit->check_unit;
  if (!any) return true;
  // Each procedure's calls, once.
  struct Call { const ProcDecl *callee; const Expr *at; };
  std::map<const ProcDecl *, std::vector<Call>> calls;
  std::vector<Call> *into = nullptr;
  std::function<void(const Expr *)> expr = [&](const Expr *e) {
    if (!e) return;
    if (e->proc) into->push_back({e->proc, e});
    if (e->mutation_proc) into->push_back({e->mutation_proc, e});
    expr(e->lhs); expr(e->rhs); expr(e->third);
    for (const Expr *a : e->args) expr(a);
    for (const FieldInit &f : e->fields) expr(f.value);
  };
  std::function<void(const Stmt *)> stmt = [&](const Stmt *st) {
    if (!st) return;
    expr(st->expr); expr(st->expr2); expr(st->target);
    for (const Stmt *c : st->body) stmt(c);
    stmt(st->then_); stmt(st->else_);
    for(const SwitchCase *arm:st->cases)stmt(arm->body);
  };
  for (ProcDecl *p : file->procs) {
    if (p->check_unit || p->foreign) continue;
    into = &calls[p];
    stmt(p->body);
  }
  // To a fixed point: a caller of check code is check code. why: the call that made it so.
  std::map<const ProcDecl *, Call> why;
  for (bool changed = true; changed;) {
    changed = false;
    for (ProcDecl *p : file->procs) {
      if (p->check_unit || p->foreign) continue;
      for (const Call &c : calls[p]) {
        if (!c.callee->check_unit) continue;
        p->check_unit = true;
        why[p] = c;
        changed = true;
        break;
      }
    }
  }
  bool ok = true;
  for (const ProcDecl *p : file->procs) {
    auto found = why.find(p);
    if (found == why.end() || !(p->export_c || str_eq(p->name, "main"))) continue;
    // Follow the calls down to the check unit's own procedure.
    const ProcDecl *last = found->second.callee;
    for (auto next = why.find(last); next != why.end(); next = why.find(last)) last = next->second.callee;
    const ProcDecl *first = found->second.callee;
    const std::string name = [](const ProcDecl *d) { return "'" + to_std((d->origin ? d->origin : d)->name) + "'"; }(first);
    const std::string end = "'" + to_std((last->origin ? last->origin : last)->name) + "'";
    error_at(file->src, found->second.at->loc, "E0616",
             "'" + to_std(p->name) + "' is emitted and calls " + name + (first == last ? "" : ", which reaches " + end) +
                 ", which a check load (#load ... #test;) brought in",
             "a check unit's procedures are for #test and #forall; move " + end + " into a unit loaded without #test");
    ok = false;
  }
  return ok;
}

size_t g_instance_c_bytes = 0;

// --split N (#12938): a --library unit's C as up to N translation units and
// a shared one, so a build compiles again only the units whose C changed (a
// gameplay edit to Everglade's carrier changed all 9 MB of one unit). There is
// one unit per 512 KiB of procedures' C (--split-unit BYTES), at most N; a
// unit under 2 MiB (four units' worth) is emitted whole. The output is one stream: a prelude every unit
// begins with (the runtime, the types, the view helpers and every procedure's
// prototype), then each unit's part after a line "/* coax-unit ID */"
// (coax/web/wasm.sh writes each part, after the prelude, to a file of its
// own). Procedures are grouped into runs of neighbours in one source file, a
// run ending after a procedure whose name's hash is 0 mod 8, so an edit moves
// no other procedure between runs; a run's unit is the hash of its file and
// first procedure's name, mod the unit count. A generic instance goes with the
// generic's file and name. Each unit names its own compile-time values
// (coax_k0...). The shared unit holds the init, show and host-default
// helpers. Procedures the program alone calls lose static: they are called
// across units (a release's link-time optimization, which sees every unit,
// internalizes them again).
static uint64_t split_hash(const std::string &text) {
  uint64_t h = 1469598103934665603ULL;  // FNV-1a
  for (unsigned char c : text) { h ^= c; h *= 1099511628211ULL; }
  return h;
}

// The bytes of the procedures' C, as a split emits them (no cost is recorded
// and no expression is numbered: the C that follows is emitted afresh).
static size_t split_function_bytes(const File *file, const EmitOptions &opts, const std::set<const ProcDecl *> &called) {
  Emitter em{file, opts};
  em.split = true;
  std::map<const Expr *, int32_t> ids;
  std::vector<const Expr *> values;
  em.unit_ct_ids = &ids; em.unit_ct_values = &values;
  size_t bytes = 0;
  for (const ProcDecl *p : file->procs)
    if (!p->check_unit && (!p->origin || called.count(p))) bytes += em.function(p).size();
  return bytes;
}

static std::string emit_c_split(const File *file, const EmitOptions &opts, uint64_t units) {
  Emitter em{file, opts};
  em.split = true;
  g_instance_c_bytes = 0;
  TimedLaps laps;
  const std::set<const ProcDecl *> called = called_instances(file);
  laps.lap("emit.instances");
  auto emitted = [&](const ProcDecl *p) { return !p->check_unit && (!p->origin || called.count(p)); };
  // Each procedure's unit: runs of a source file's own procedures, then instances.
  std::map<const ProcDecl *, uint64_t> unit_of;
  std::string run_key, last_path;
  bool cut = true;
  for (const ProcDecl *p : file->procs) {
    if (p->origin) continue;
    std::string path = source_point(file->src, p->loc).path, name = to_std(p->name);
    if (cut || path != last_path) run_key = path + "\n" + name;
    unit_of[p] = split_hash(run_key) % units;
    last_path = path;
    cut = split_hash(name) % 8 == 0;
  }
  for (const ProcDecl *p : file->procs) {
    if (!p->origin) continue;
    unit_of[p] = split_hash(source_point(file->src, p->origin->loc).path + "\n" + to_std(p->origin->name) + "\ninstance") % units;
  }
  struct Part { std::map<const Expr *, int32_t> ids; std::vector<const Expr *> values; std::string text; };
  std::vector<Part> parts(units + 1);  // parts[units]: the shared unit
  auto into = [&](Part &part) { em.unit_ct_ids = &part.ids; em.unit_ct_values = &part.values; };
  std::string functions;  // all of them, for the show helpers' reachability
  for (const ProcDecl *p : file->procs) {
    if (!emitted(p)) continue;
    const ProcDecl *o = p->origin ? p->origin : p;
    DeclCost *cost = g_decl_costs.at(file, o, "procedure", o->name, o->loc);
    Part &part = parts[unit_of.at(p)];
    into(part);
    std::string f;
    { DeclClock clock{cost ? &cost->emit_seconds : nullptr}; f = em.function(p); }
    if (cost) cost->c_bytes += f.size();
    if (p->origin) g_instance_c_bytes += f.size();
    part.text += f;
    functions += f;
  }
  laps.lap("emit.functions");
  Part &shared = parts[units];
  into(shared);
  std::string init_protos, show_protos;
  std::string inits = init_functions(file, em, &init_protos);
  std::string hosted = state_defaults(file, em);
  laps.lap("emit.inits");
  const std::string uses = inits + functions + hosted;
  std::string shows = show_functions(file, em, &uses, &show_protos);
  laps.lap("emit.shows");
  em.unit_ct_ids = nullptr; em.unit_ct_values = nullptr;
  shared.text = "\n" + inits + shows + "\n" + hosted;

  TypeWriter types;
  types.forward_structs(file);
  std::vector<Type> all;
  collect_types(file, &all);
  for (Type t : all) types.need(t);
  types.layout_asserts(file);
  laps.lap("emit.types");

  std::string s = "/* Generated by coax0 from " + file->src->path + " (coax c --split " + std::to_string(units) + "). Do not edit: change the .coax file. */\n";
  s += "static const char *coax_source_path = \"" + escape_path(file->src->path) + "\";\n";
  s += source_runtime(file);
  if (em.strict_math) s += strict_math_runtime();
  if (em.transactions) s += transaction_runtime();
  if (em.erasure) s += ZERO_RUNTIME;
  if (em.barrier) s += BARRIER_RUNTIME;
  if (em.inlined) s += INLINE_RUNTIME;
  if (em.wrapping) s += WRAP_RUNTIME;
  if (em.intops) s += INTOPS_RUNTIME;
  if (em.versioned) s += IV_RUNTIME;
  s += "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n";
  s += "\n/* Types */\n" + types.out;
  if (em.wide) s += WIDE_RUNTIME;
  s += view_helpers();
  if (em.transactions) s += transaction_captures(types.done);
  s += "\n" + init_protos + show_protos + "\n";
  for (const ProcDecl *p : file->procs) if (emitted(p)) s += em.prototype(p) + ";\n";
  for (uint64_t u = 0; u <= units; u++) {
    Part &part = parts[u];
    if (part.text.empty() || part.text == "\n\n") continue;
    s += "/* coax-unit " + (u == units ? std::string("shared") : std::to_string(u)) + " */\n";
    if (!part.values.empty()) {
      s += "\n/* Values computed at compile time */\n";
      for (size_t i = 0; i < part.values.size(); i++) {
        const Expr *e = part.values[i];
        s += "static const " + c_type(e->type) + " coax_k" + std::to_string(i) + " = " + em.init_list(*e->ct) + ";\n";
      }
    }
    s += part.text;
  }
  laps.lap("emit.assemble");
  return s;
}

std::string emit_c(const File *file, const EmitOptions &opts) {
  if (opts.cuda) return emit_cuda(file, opts);
  if (opts.metal_compute) return emit_metal_compute(file, opts);
  if (opts.split > 0 && opts.library && !opts.metal) {
    // One part per 512 KiB of procedures' C (--split-unit), at most N; a unit
    // with less than four parts' worth is emitted whole, as without --split
    // (splitting a small unit only makes a release larger: Belt's 1.6 MB of C
    // in 16 parts linked 2% larger).
    const uint64_t bytes = split_function_bytes(file, opts, called_instances(file));
    if (bytes / opts.split_unit >= 4) return emit_c_split(file, opts, std::min<uint64_t>((uint64_t)opts.split, bytes / opts.split_unit));
  }
  Emitter em{file, opts};
  std::string functions;
  g_instance_c_bytes = 0;
  TimedLaps laps;  // --timings: emit.instances, emit.functions, emit.inits, emit.shows, emit.types, emit.values, emit.assemble
  const std::set<const ProcDecl *> called = called_instances(file);
  laps.lap("emit.instances");
  auto emitted = [&](const ProcDecl *p) { return !p->check_unit && (!p->origin || called.count(p)); };
  for (const ProcDecl *p : file->procs) {
    if (!emitted(p)) continue;
    const ProcDecl *o = p->origin ? p->origin : p;
    DeclCost *cost = g_decl_costs.at(file, o, "procedure", o->name, o->loc);
    std::string f;
    { DeclClock clock{cost ? &cost->emit_seconds : nullptr}; f = em.function(p); }
    if (cost) cost->c_bytes += f.size();
    if (p->origin) g_instance_c_bytes += f.size();
    functions += f;
  }
  laps.lap("emit.functions");
  if (opts.metal) {
    // Scalar f32/bool functions only (the checker enforced it): no runtime.
    std::string s = "/* Generated by coax0 from " + file->src->path + ". Do not edit: change the .coax file. */\n";
    s += "#include <metal_stdlib>\nusing namespace metal;\n\n";
    for (const ProcDecl *p : file->procs) if (emitted(p)) s += em.prototype(p) + ";\n";
    s += functions;
    for (size_t at = 0; (at = s.find("_Bool", at)) != std::string::npos;) s.replace(at, 5, "bool");
    return s;
  }
  std::string inits = init_functions(file, em);
  std::string hosted = opts.library ? state_defaults(file, em) : std::string();
  laps.lap("emit.inits");
  const std::string uses = inits + functions + hosted;
  std::string shows = show_functions(file, em, &uses);
  laps.lap("emit.shows");

  TypeWriter types;
  types.forward_structs(file);
  std::vector<Type> all;
  collect_types(file, &all);
  for (Type t : all) types.need(t);
  types.layout_asserts(file);
  laps.lap("emit.types");

  std::string s = "/* Generated by coax0 from " + file->src->path + ". Do not edit: change the .coax file. */\n";
  s += "static const char *coax_source_path = \"" + escape_path(file->src->path) + "\";\n";
  s += source_runtime(file);
  if (em.strict_math) s += strict_math_runtime();
  if (em.transactions) s += transaction_runtime();
  if (em.erasure) s += ZERO_RUNTIME;
  if (em.barrier) s += BARRIER_RUNTIME;
  if (em.inlined) s += INLINE_RUNTIME;
  if (em.wrapping) s += WRAP_RUNTIME;
  if (em.intops) s += INTOPS_RUNTIME;
  if (em.versioned) s += IV_RUNTIME;
  s += "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n";
  s += "\n/* Types */\n" + types.out;
  if (em.wide) s += WIDE_RUNTIME;
  s += view_helpers();
  if (em.transactions) s += transaction_captures(types.done);
  if (!em.ct_values.empty()) {
    s += "\n/* Values computed at compile time */\n";
    for (size_t i = 0; i < em.ct_values.size(); i++) {
      const Expr *e = em.ct_values[i];
      s += "static const " + c_type(e->type) + " coax_k" + std::to_string(i) + " = " + em.init_list(*e->ct) + ";\n";
    }
  }
  laps.lap("emit.values");
  s += "\n" + inits + shows + "\n";
  for (const ProcDecl *p : file->procs) if (emitted(p)) s += em.prototype(p) + ";\n";
  s += functions;
  s += hosted;
  bool has_main = false;
  for (const ProcDecl *p : file->procs) has_main = has_main || str_eq(p->name, "main");
  if (!opts.library && has_main) s += "\nint main(void) {\n  coax_p_main();\n  return 0;\n}\n";
  laps.lap("emit.assemble");
  return s;
}

static std::string native_check_table_v2(const File *file, Emitter &em, const NativeCheckCPlan &plan) {
  std::string s = "\n/* Explicit verification ABI2: immutable binding metadata and admitted word tuples. */\n"
      "typedef struct Coax_VerificationEnumValue_v2 { uint64_t bits; const char *label; int64_t label_count; } Coax_VerificationEnumValue_v2;\n"
      "typedef struct Coax_VerificationBinding_v2 {\n"
      "  const char *name; int64_t name_count; const char *type_name; int64_t type_name_count;\n"
      "  int64_t kind, width, is_signed, first; uint64_t count;\n"
      "  const Coax_VerificationEnumValue_v2 *values; int64_t value_count;\n"
      "} Coax_VerificationBinding_v2;\n"
      "typedef struct Coax_VerificationCheck_v2 {\n"
      "  int64_t declaration_index; const char *name; int64_t name_count;\n"
      "  const char *source; int64_t source_count; int64_t line, column;\n"
      "  int64_t kind; const Coax_VerificationBinding_v2 *bindings; int64_t binding_count;\n"
      "  uint64_t case_count; int64_t (*run_values)(const uint64_t *, int64_t);\n"
      "} Coax_VerificationCheck_v2;\n";
  for (size_t row = 0; row < plan.checks.size(); ++row) {
    const NativeCheckCEntry &entry = plan.checks[row];
    const ProcDecl *proc = entry.check->proc;
    std::string prefix = "coax_verification_v2_" + std::to_string(row);
    for (size_t binding = 0; binding < entry.domains.size(); ++binding) {
      const NativeCheckDomain &domain = entry.domains[binding];
      if (domain.values.empty()) continue;
      s += "static const Coax_VerificationEnumValue_v2 " + prefix + "_values_" + std::to_string(binding) + "[] = {\n";
      for (size_t member = 0; member < domain.values.size(); ++member)
        s += "  {UINT64_C(" + std::to_string(domain.values[member]) + "), " + c_string_text(domain.labels[member]) + ", " +
             std::to_string(domain.labels[member].count) + "},\n";
      s += "};\n";
    }
    if (!entry.domains.empty()) {
      s += "static const Coax_VerificationBinding_v2 " + prefix + "_bindings[] = {\n";
      for (size_t binding = 0; binding < entry.domains.size(); ++binding) {
        const NativeCheckDomain &domain = entry.domains[binding];
        const Param &param = proc->params[(int32_t)binding];
        Type base = domain.type->kind == TypeKind::enum_ ? domain.type->elem : domain.type;
        Str label{domain.type->name.data(), (int32_t)domain.type->name.size()};
        s += "  {" + c_string_text(param.name) + ", " + std::to_string(param.name.count) + ", " + c_string_text(label) + ", " +
             std::to_string(label.count) + ", " + std::to_string(domain.kind) + ", " +
             std::to_string(base->bits) + ", " + (base->is_signed ? "1" : "0") + ", " +
             int_text(&T_S64, (uint64_t)domain.first) + ", UINT64_C(" + std::to_string(domain.count) + "), " +
             (domain.values.empty() ? "NULL" : prefix + "_values_" + std::to_string(binding)) + ", " +
             std::to_string(domain.values.size()) + "},\n";
      }
      s += "};\n";
    }
    s += "static int64_t " + prefix + "_run(const uint64_t *values, int64_t count) {\n"
         "  if (count != " + std::to_string(proc->params.count) +
         (proc->params.count ? " || !values" : "") + ") return -1;\n";
    if (!proc->params.count) s += "  (void)values;\n";
    std::vector<std::string> arguments;
    for (int32_t i = 0; i < proc->params.count; ++i) {
      Type type = proc->params[i].type;
      Type base = type->kind == TypeKind::enum_ ? type->elem : type;
      std::string word = "values[" + std::to_string(i) + "]";
      if (base->is_signed) {
        std::string signed_word = "coax_signed_" + std::to_string(i);
        s += "  int64_t " + signed_word + "; memcpy(&" + signed_word + ", &" + word + ", sizeof(" + signed_word + "));\n";
        word = signed_word;
      }
      arguments.push_back("(" + c_type(type) + ")" + word);
    }
    s += "  " + em.proc_name(proc) + "(";
    for (size_t i = 0; i < arguments.size(); ++i) s += (i ? ", " : "") + arguments[i];
    s += ");\n  return 0;\n}\n";
  }
  s += "static const Coax_VerificationCheck_v2 coax_verification_table_v2[] = {\n";
  for (size_t row = 0; row < plan.checks.size(); ++row) {
    const NativeCheckCEntry &entry = plan.checks[row];
    const CheckDecl *check = entry.check;
    auto point = source_point(file->src, check->loc);
    Str path{point.path.data(), (int32_t)point.path.size()};
    s += "  {" + std::to_string(entry.declaration_index) + ", " + c_string_text(check->name) + ", " +
         std::to_string(check->name.count) + ", " + c_string_text(path) + ", " + std::to_string(path.count) + ", " +
         std::to_string(point.loc.line) + ", " + std::to_string(point.loc.col) + ", " + (check->forall ? "2" : "1") + ", " +
         (entry.domains.empty() ? "NULL" : "coax_verification_v2_" + std::to_string(row) + "_bindings") + ", " +
         std::to_string(entry.domains.size()) + ", UINT64_C(" + std::to_string(entry.case_count) + "), coax_verification_v2_" +
         std::to_string(row) + "_run},\n";
  }
  s += "};\nconst Coax_VerificationCheck_v2 *coax_verification_checks_v2(int64_t *count) {\n"
       "  if (count) *count = " + std::to_string(plan.checks.size()) + ";\n  return coax_verification_table_v2;\n}\n";
  return s;
}

std::string emit_native_checks_c_plan(const File *file, const EmitOptions &opts, const NativeCheckCPlan &plan) {
  Emitter em{file, opts};
  em.verification = true;
  // Every definition in this verification unit has a generated private name.
  // Authored export names and arbitrary check display names cannot collide
  // with the table ABI or inject identifiers into emitted C.
  for (size_t i = 0; i < plan.procedures.size(); ++i)
    em.verification_names.emplace(plan.procedures[i], "coax_verification_body_" + std::to_string(i));
  std::string functions;
  g_instance_c_bytes = 0;
  for (const ProcDecl *p : plan.procedures) {
    std::string function = em.function(p);
    if (p->origin) g_instance_c_bytes += function.size();
    functions += function;
  }
  std::string inits = init_functions(file, em);
  std::string shows = show_functions(file, em);
  TypeWriter types;
  types.forward_structs(file);
  std::vector<Type> all;
  collect_types(file, &all);
  for (Type type : all) types.need(type);
  types.layout_asserts(file);

  std::string s = "/* Checked-AST verification bodies. Not product emission. */\n";
  s += "static const char *coax_source_path = \"" + escape_path(file->src->path) + "\";\n";
  s += source_runtime(file, true);
  if (em.strict_math) s += strict_math_runtime();
  if (em.transactions) s += transaction_runtime();
  if (em.erasure) s += ZERO_RUNTIME;
  if (em.barrier) s += BARRIER_RUNTIME;
  if (em.inlined) s += INLINE_RUNTIME;
  if (em.wrapping) s += WRAP_RUNTIME;
  if (em.intops) s += INTOPS_RUNTIME;
  s += "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n";
  s += "\n/* Types */\n" + types.out;
  if (em.wide) s += WIDE_RUNTIME;
  s += view_helpers();
  if (em.transactions) s += transaction_captures(types.done);
  if (!em.ct_values.empty()) {
    s += "\n/* Values computed at compile time */\n";
    for (size_t i = 0; i < em.ct_values.size(); ++i) {
      const Expr *e = em.ct_values[i];
      s += "static const " + c_type(e->type) + " coax_k" + std::to_string(i) + " = " + em.init_list(*e->ct) + ";\n";
    }
  }
  s += "\n" + inits + shows + "\n";
  for (const ProcDecl *p : plan.procedures) s += em.prototype(p) + ";\n";
  s += functions;
  if (plan.verification_abi == 2) return s + native_check_table_v2(file, em, plan);
  s += "\n/* Verification-only ABI v1: display strings are byte spans, not C identifiers. */\n"
       "typedef struct Coax_VerificationCheck_v1 {\n"
       "  int64_t declaration_index;\n"
       "  const char *name; int64_t name_count;\n"
       "  const char *source; int64_t source_count;\n"
       "  int64_t line, column;\n"
       "  void (*run)(void);\n"
       "} Coax_VerificationCheck_v1;\n"
       "static const Coax_VerificationCheck_v1 coax_verification_table_v1[] = {\n";
  for (const NativeCheckCEntry &entry : plan.checks) {
    const CheckDecl *check = entry.check;
    auto point = source_point(file->src, check->loc);
    // Admitted filesystem paths fit Str's bounded count; the ABI fields are int64_t.
    Str path{point.path.data(), (int32_t)point.path.size()};
    s += "  {" + std::to_string(entry.declaration_index) + ", " + c_string_text(check->name) + ", " +
         std::to_string(check->name.count) + ", " + c_string_text(path) + ", " + std::to_string(path.count) + ", " +
         std::to_string(point.loc.line) + ", " + std::to_string(point.loc.col) + ", " + em.proc_name(check->proc) + "},\n";
  }
  s += "};\nconst Coax_VerificationCheck_v1 *coax_verification_checks_v1(int64_t *count) {\n"
       "  if (count) *count = " + std::to_string(plan.checks.size()) + ";\n"
       "  return coax_verification_table_v1;\n}\n";
  return s;
}

// A native check binary (#13122, coax/docs/native-checks.md): the program's
// procedures, its checks' bodies and a driver main. The driver runs the
// checks argv[1], argv[1]+argv[2], ... (argv[2] defaults to 1), each #forall over its cases in the evaluator's
// order, on a thread with a large stack, and writes one line per event to
// file descriptor 3: "B i" before check i, "E i cases procs..." after it
// passes (procs: the coverage ids it ran), "T case\tline\tpath\twhat" when
// it traps (the runtime then aborts) and "R what" when it refuses (output or
// a host effect: the evaluator decides that check). The parent restarts the
// driver after the check that stopped it.
//
// With mutants (#13582, coax/docs/native-mutants.md) the same binary holds
// every mutation site behind `coax_nm_mutant`, and counts ticks: one for each
// procedure entered and each loop iteration. Run as above it is the unmutated
// program, and it also writes "K i ticks" before a check's "E" line. Run as
// `checks m ID SCHEDULE [FIRST]` it is mutant ID: it runs the checks SCHEDULE
// names, one "ROW TICKS" line each (FIRST before the others), each with that
// many ticks to spend, and writes "B row" before each and "S" after the last.
// A check that stops it writes why first: "T ..." for a trap, "R what" for a
// refusal, "L" when the ticks ran out.
std::vector<std::string> emit_check_binary_c(const File *file, const EmitOptions &opts, const NativeCheckCPlan &plan, size_t parts,
                                             NativeMutantPlan *mutants) {
  Emitter em{file, opts};
  em.verification = true;
  em.check_binary = true;
  em.split = true;  // procedures and helpers are defined once, across the parts
  if (mutants) {
    em.mutants = mutants;
    mutants->emitted.assign(mutants->sites.size(), 0);
    for (size_t i = 0; i < mutants->sites.size(); ++i) {
      const NativeMutantSite &site = mutants->sites[i];
      if (site.kind == 1 && site.expr) em.mutant_exprs.emplace(site.expr, i);
      else if (site.stmt) em.mutant_stmts[site.stmt].push_back(i);
    }
  }
  for (size_t i = 0; i < plan.procedures.size(); ++i) {
    em.verification_names.emplace(plan.procedures[i], "coax_verification_body_" + std::to_string(i));
    em.coverage_ids.emplace(plan.procedures[i], i);
  }
  if (parts < 1) parts = 1;
  // Each procedure goes to the part with the fewest bytes so far, largest first.
  std::vector<std::string> bodies(plan.procedures.size());
  size_t total = 0;
  for (size_t i = 0; i < plan.procedures.size(); ++i) { bodies[i] = em.function(plan.procedures[i]); total += bodies[i].size(); }
  g_instance_c_bytes = 0;
  parts = std::max<size_t>(1, std::min(parts, total / (128 << 10) + 1));
  std::vector<size_t> order(bodies.size());
  for (size_t i = 0; i < order.size(); ++i) order[i] = i;
  std::stable_sort(order.begin(), order.end(), [&](size_t a, size_t b) { return bodies[a].size() > bodies[b].size(); });
  std::vector<std::string> part_text(parts);
  std::vector<size_t> part_bytes(parts, 0);
  for (size_t i : order) {
    size_t at = (size_t)(std::min_element(part_bytes.begin(), part_bytes.end()) - part_bytes.begin());
    part_text[at] += bodies[i];
    part_bytes[at] += bodies[i].size();
  }
  std::string init_protos, show_protos;
  std::string inits = init_functions(file, em, &init_protos);
  std::string all_functions;
  for (const std::string &b : bodies) all_functions += b;
  const std::string uses = inits + all_functions;
  std::string shows = show_functions(file, em, &uses, &show_protos);
  std::string table = native_check_table_v2(file, em, plan);
  TypeWriter types;
  types.forward_structs(file);
  std::vector<Type> all;
  collect_types(file, &all);
  for (Type type : all) types.need(type);
  types.layout_asserts(file);

  std::string runtime = source_runtime(file, true);
  {  // one trap handler for every part, which the driver sets
    const std::string handler = "static Coax_VerificationTrapHandler_v1 coax_verification_trap_handler_v1;\n";
    size_t at = runtime.find(handler);
    if (at != std::string::npos) runtime.replace(at, handler.size(), "extern Coax_VerificationTrapHandler_v1 coax_verification_trap_handler_v1;\n");
    const std::string setter = "void coax_verification_set_trap_v1(Coax_VerificationTrapHandler_v1 handler) { coax_verification_trap_handler_v1 = handler; }\n";
    at = runtime.find(setter);
    if (at != std::string::npos) runtime.erase(at, setter.size());
  }
  std::string prelude = "/* A native check binary (coax native checks). Not product emission. */\n"
                        "#include <errno.h>\n#include <pthread.h>\n#include <unistd.h>\n";
  if (mutants) prelude += "#include <sys/resource.h>\n";
  prelude += "static const char *coax_source_path = \"" + escape_path(file->src->path) + "\";\n";
  prelude += runtime;
  prelude += "extern unsigned char coax_nc_cov[" + std::to_string(plan.procedures.size() + 1) + "];\n"
       "static void coax_nc_write(const char *at, size_t n) {\n"
       "  while (n) { ssize_t w = write(3, at, n); if (w < 0 && errno == EINTR) continue; if (w <= 0) _exit(97); at += w; n -= (size_t)w; }\n"
       "}\n"
       "static _Noreturn void coax_nc_refuse(const char *what) {\n"
       "  char line[256]; int n = snprintf(line, sizeof line, \"R %s\\n\", what);\n"
       "  coax_nc_write(line, (size_t)n); _exit(98);\n"
       "}\n";
  if (mutants)
    prelude += "extern int64_t coax_nm_mutant;\nextern uint64_t coax_nm_ticks, coax_nm_limit;\n"
               "static _Noreturn void coax_nm_over(void) { coax_nc_write(\"L\\n\", 2); _exit(96); }\n"
               "#define COAX_NM_TICK() do { if (++coax_nm_ticks > coax_nm_limit) coax_nm_over(); } while (0)\n";
  if (em.strict_math) prelude += strict_math_runtime();
  if (em.transactions) prelude += transaction_runtime();
  if (em.erasure) prelude += ZERO_RUNTIME;
  if (em.barrier) prelude += BARRIER_RUNTIME;
  if (em.inlined) prelude += INLINE_RUNTIME;
  if (em.wrapping) prelude += WRAP_RUNTIME;
  if (em.intops) prelude += INTOPS_RUNTIME;
  if (em.versioned) prelude += IV_RUNTIME;
  prelude += "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n";
  prelude += "\n/* Types */\n" + types.out;
  if (em.wide) prelude += WIDE_RUNTIME;
  prelude += view_helpers();
  if (em.transactions) prelude += transaction_captures(types.done);
  std::string values;
  for (size_t i = 0; i < em.ct_values.size(); ++i) {
    const Expr *e = em.ct_values[i];
    prelude += "extern const " + c_type(e->type) + " coax_k" + std::to_string(i) + ";\n";
    values += "const " + c_type(e->type) + " coax_k" + std::to_string(i) + " = " + em.init_list(*e->ct) + ";\n";
  }
  prelude += "\n" + init_protos + show_protos + "\n";
  for (const ProcDecl *p : plan.procedures) prelude += em.prototype(p) + ";\n";

  std::string shared = prelude + "\nunsigned char coax_nc_cov[" + std::to_string(plan.procedures.size() + 1) + "];\n"
                       "Coax_VerificationTrapHandler_v1 coax_verification_trap_handler_v1;\n" +
                       (mutants ? "int64_t coax_nm_mutant;\nuint64_t coax_nm_ticks, coax_nm_limit = UINT64_MAX;\n" : "") + values + "\n" + inits + shows + table;
  shared += "\n/* The driver. */\n"
       "static uint64_t coax_nc_case;\n"
       "static void coax_nc_trap(const char *what, int64_t what_count, const char *path, int64_t path_count, int64_t line) {\n"
       "  char text[8192]; int n = snprintf(text, sizeof text, \"T %llu\\t%lld\\t%.*s\\t%.*s\\n\", (unsigned long long)coax_nc_case, (long long)line,\n"
       "                                    (int)(path_count < 4000 ? path_count : 4000), path, (int)(what_count < 2000 ? what_count : 2000), what);\n"
       "  if (n > 0) coax_nc_write(text, (size_t)(n < (int)sizeof text ? n : (int)sizeof text - 1));\n"
       "}\n"
       "static int64_t coax_nc_start, coax_nc_step = 1;\n"
       "static void *coax_nc_main(void *unused) {\n"
       "  (void)unused;\n"
       "  int64_t count = 0;\n"
       "  const Coax_VerificationCheck_v2 *table = coax_verification_checks_v2(&count);\n"
       "  uint64_t words[64];\n"
       "  char text[64];\n"
       "  for (int64_t i = coax_nc_start; i < count; i += coax_nc_step) {\n"
       "    const Coax_VerificationCheck_v2 *c = &table[i];\n"
       "    int n = snprintf(text, sizeof text, \"B %lld\\n\", (long long)i); coax_nc_write(text, (size_t)n);\n"
       "    memset(coax_nc_cov, 0, sizeof coax_nc_cov);\n"
       + std::string(mutants ? "    coax_nm_ticks = 0;\n" : "") +
       "    for (uint64_t k = 0; k < c->case_count; k++) {\n"
       "      coax_nc_case = k;\n"
       "      uint64_t rest = k;\n"
       "      for (int64_t b = c->binding_count; b-- > 0;) {\n"
       "        const Coax_VerificationBinding_v2 *d = &c->bindings[b];\n"
       "        uint64_t at = rest % d->count; rest /= d->count;\n"
       "        words[b] = d->values ? d->values[at].bits : (uint64_t)d->first + at;\n"
       "      }\n"
       "      if (c->run_values(words, c->binding_count) != 0) coax_nc_refuse(\"a binding mismatch\");\n"
       "    }\n"
       + std::string(mutants ? "    n = snprintf(text, sizeof text, \"K %lld %llu\\n\", (long long)i, (unsigned long long)coax_nm_ticks); coax_nc_write(text, (size_t)n);\n" : "") +
       "    n = snprintf(text, sizeof text, \"E %lld %llu\", (long long)i, (unsigned long long)c->case_count); coax_nc_write(text, (size_t)n);\n"
       "    for (size_t p = 0; p < sizeof coax_nc_cov; p++) if (coax_nc_cov[p]) { n = snprintf(text, sizeof text, \" %zu\", p); coax_nc_write(text, (size_t)n); }\n"
       "    coax_nc_write(\"\\n\", 1);\n"
       "  }\n"
       "  return 0;\n"
       "}\n"
       + std::string(!mutants ? "" :
       "/* One mutant: the scheduled checks in order, each within its ticks. */\n"
       "static const char *coax_nm_schedule; static int64_t coax_nm_first = -1;\n"
       "static void coax_nm_trap(const char *what, int64_t what_count, const char *path, int64_t path_count, int64_t line) {\n"
       "  coax_nc_trap(what, what_count, path, path_count, line); _exit(94);\n"
       "}\n"
       "static void coax_nm_check(const Coax_VerificationCheck_v2 *table, int64_t count, int64_t row, uint64_t limit) {\n"
       "  uint64_t words[64]; char text[64];\n"
       "  if (row < 0 || row >= count) _exit(95);\n"
       "  const Coax_VerificationCheck_v2 *c = &table[row];\n"
       "  int n = snprintf(text, sizeof text, \"B %lld\\n\", (long long)row); coax_nc_write(text, (size_t)n);\n"
       "  coax_nm_ticks = 0; coax_nm_limit = limit;\n"
       "  for (uint64_t k = 0; k < c->case_count; k++) {\n"
       "    coax_nc_case = k;\n"
       "    uint64_t rest = k;\n"
       "    for (int64_t b = c->binding_count; b-- > 0;) {\n"
       "      const Coax_VerificationBinding_v2 *d = &c->bindings[b];\n"
       "      uint64_t at = rest % d->count; rest /= d->count;\n"
       "      words[b] = d->values ? d->values[at].bits : (uint64_t)d->first + at;\n"
       "    }\n"
       "    if (c->run_values(words, c->binding_count) != 0) coax_nc_refuse(\"a binding mismatch\");\n"
       "  }\n"
       "}\n"
       "static void *coax_nm_main(void *unused) {\n"
       "  (void)unused;\n"
       "  int64_t count = 0;\n"
       "  const Coax_VerificationCheck_v2 *table = coax_verification_checks_v2(&count);\n"
       "  FILE *schedule = fopen(coax_nm_schedule, \"r\");\n"
       "  if (!schedule) _exit(95);\n"
       "  long long row; unsigned long long limit;\n"
       "  if (coax_nm_first >= 0) {\n"
       "    while (fscanf(schedule, \"%lld %llu\", &row, &limit) == 2) if (row == coax_nm_first) { coax_nm_check(table, count, row, limit); break; }\n"
       "    rewind(schedule);\n"
       "  }\n"
       "  while (fscanf(schedule, \"%lld %llu\", &row, &limit) == 2) if (row != coax_nm_first) coax_nm_check(table, count, row, limit);\n"
       "  coax_nc_write(\"S\\n\", 2);\n"
       "  return 0;\n"
       "}\n") +
       "int main(int argc, char **argv) {\n"
       "  void *(*run)(void *) = coax_nc_main;\n"
       "  coax_verification_trap_handler_v1 = coax_nc_trap;\n"
       + std::string(!mutants ? "" :
       "  if (argc > 3 && strcmp(argv[1], \"m\") == 0) {\n"
       "    struct rlimit none = {0, 0}; setrlimit(RLIMIT_CORE, &none);\n"
       "    coax_nm_mutant = atoll(argv[2]); coax_nm_schedule = argv[3];\n"
       "    if (argc > 4) coax_nm_first = atoll(argv[4]);\n"
       "    coax_verification_trap_handler_v1 = coax_nm_trap;\n"
       "    run = coax_nm_main;\n"
       "  } else\n") +
       "  {\n"
       "  coax_nc_start = argc > 1 ? atoll(argv[1]) : 0;\n"
       "  if (argc > 2 && atoll(argv[2]) > 0) coax_nc_step = atoll(argv[2]);\n"
       "  }\n"
       "  pthread_attr_t attr; pthread_attr_init(&attr);\n"
       "  pthread_attr_setstacksize(&attr, (size_t)1 << 30);\n"
       "  pthread_t thread;\n"
       "  if (pthread_create(&thread, &attr, run, 0) != 0) run(0);\n"
       "  else pthread_join(thread, 0);\n"
       "  return 0;\n"
       "}\n";
  std::vector<std::string> out{shared};
  for (std::string &text : part_text) if (!text.empty()) out.push_back(prelude + text);
  return out;
}

std::string emit_header(const File *file) {
  // The types the exported procedures use, and everything they contain.
  TypeWriter types;
  types.header = true;
  for(const UnionDecl *d:file->unions)types.out+="typedef struct "+c_type(d->type)+" "+c_type(d->type)+";\n";
  for(Type t:all_composite_types())if(is_sum(t))types.out+="typedef struct "+c_type(t)+" "+c_type(t)+";\n";
  // Every type an exported or foreign procedure's signature reaches through
  // sum payloads, elements and fields, found once (#12925: searched again
  // for each struct, it took seconds for a large program).
  std::set<Type> reached;
  std::function<void(Type)> reach = [&](Type t) {
    if (!reached.insert(t).second) return;
    if(is_sum(t))for(int32_t i=0;i<sum_count(t);i++)reach(sum_payload(t,i));
    if (t->kind == TypeKind::array || t->kind == TypeKind::view) reach(t->elem);
    if (t->kind == TypeKind::struct_)
      for (const Field &f : t->struct_decl->fields) reach(f.type);
  };
  for (const ProcDecl *p : file->procs) {
    if ((!p->export_c && !p->foreign) || p->check_unit) continue;
    reach(p->ret);
    for (const Param &prm : p->params) reach(prm.type);
  }
  for (const StructDecl *d : file->structs)
    if (reached.count(d->type) && !d->type->physical) types.out += "typedef struct coax_" + tag(d->type) + " coax_" + tag(d->type) + ";\n";
  {
    TypeWriter used = header_types(file);
    types.out += used.out;
    types.done = used.done;
  }
  size_t definitions_end=types.out.size();types.sum_layout_asserts("COAX_ALIGNOF");
  std::string asserts=types.out.substr(definitions_end);types.out.resize(definitions_end);
  for (const StructDecl *d : file->structs) {
    if (d->type->physical || !types.seen(d->type)) continue;
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
  // Per-type guards permit repeated inclusion without hiding another entry
  // unit that happens to have the same basename. Compatible C prototypes and
  // forward typedefs may be repeated in C11 and C++.
  s += "#include <stddef.h>\n#include <stdint.h>\n\n";
  s += "#ifdef __cplusplus\n#define _Bool bool\n#define COAX_LAYOUT(c, m) static_assert(c, m)\n#define COAX_ALIGNOF alignof\nextern \"C\" {\n#else\n"
       "#define COAX_LAYOUT(c, m) _Static_assert(c, m)\n#define COAX_ALIGNOF _Alignof\n#endif\n\n";
  s += "#ifndef COAX_STRING_DEFINED\n#define COAX_STRING_DEFINED\ntypedef struct { const char *data; int64_t count; } coax_string;\n#endif\n\n";
  s += types.out + asserts + "\n";
  for (const ProcDecl *p : file->procs)
    if (p->export_c && !p->check_unit) s += em.prototype(p, true) + ";\n";
  for (Type t : types.done)
    if (host_default(t)) s += "void coax_default_" + tag(t) + "(" + c_type(t) + " *p);\n";
  bool any_foreign = false;
  for (const ProcDecl *p : file->procs) any_foreign = any_foreign || (p->foreign && !p->check_unit);
  if (any_foreign) {
    s += "\n/* Provided by the host (#foreign). */\n";
    for (const ProcDecl *p : file->procs)
      if (p->foreign && !p->check_unit) s += em.prototype(p, true) + ";\n";
  }
  s += "\n#ifdef __cplusplus\n}\n#undef _Bool\n#endif\n#undef COAX_LAYOUT\n#undef COAX_ALIGNOF\n";
  return s;
}

std::string layout_report(const File *file) {
  std::vector<Type> types;collect_types(file,&types);
  bool sums=false;for(Type t:types)sums=sums || is_sum(t);
  std::string s=sums ? "coax-layout-sums-lp64.v1\n" : "";
  if(sums)for(Type t:types)if(is_sum(t)){
    s+=std::string(type_name(t))+"  sum size "+std::to_string(t->size)+"  align "+std::to_string(t->align)+"  tag u32@0  payload@"+std::to_string(t->sum_payload_offset)+"\n";
    for(int32_t i=0;i<sum_count(t);i++)s+="  "+std::to_string(i)+": "+type_name(sum_payload(t,i))+"\n";
  }
  char buf[256];
  for (const StructDecl *d : file->structs) {
    // an instance is listed by its type: Pair(int)
    snprintf(buf, sizeof buf, "%s  size %d  align %d\n", d->origin ? type_name(d->type) : to_std(d->name).c_str(), d->type->size, d->type->align);
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
// The SHA-256 of a unit's bytes, computed once for those bytes: it is kept on
// the unit's lex, which a resident compiler holds between compiles.
static std::string unit_text_sha(const SourceUnit &unit) {
  if(!unit.lexed)return debug_source_hash(unit.text);
  if(unit.lexed->text_sha.empty()){unit.lexed->text_sha=debug_source_hash(unit.text);if(!unit.lexed->text_sha.empty())serve_keep_unit(unit.lexed);}
  return unit.lexed->text_sha;
}
std::string emit_debug_map(const File *file) {
  Emitter em{file,{}};
  std::string out="{\"schema_version\":1,\"sources\":[";
  bool first=true;
  for (const auto &unit:file->src->units) {out+=(first?"":",")+json_string(unit->canonical);first=false;}
  out+="],\"source_hashes\":{";first=true;
  for(const auto &unit:file->src->units) {out+=(first?"":",")+json_string(unit->canonical)+":"+json_string(unit_text_sha(*unit));first=false;}
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
      for(const SwitchCase *arm:s->cases){if(arm->binding)variable(arm->binding->name,arm->binding->type,arm->binding->loc,"case");walk(arm->body);}
    };
    walk(p->body);out+="]}";
  }
  return out+"]}\n";
}
} // namespace coax
