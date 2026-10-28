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
    case TypeKind::floating: return t == &T_F32 ? "float" : "double";
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

  // C's math.h names, with the float variants for f32; Metal overloads.
  std::string math_name(const Expr *e) {
    std::string name = e->intrinsic == Intrinsic::abs ? "fabs" : to_std(e->text);
    if (opts.metal) return e->intrinsic == Intrinsic::abs ? "abs" : name;
    return name + (e->type == &T_F32 ? "f" : "");
  }
  std::string proc_name(const ProcDecl *p) {
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
        std::string t = new_temp(c_type(o.e->type) + " *");
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
        std::vector<Operand> ops;
        for (const Expr *a : e->args) ops.push_back({a, false});
        std::string name = e->intrinsic != Intrinsic::none ? math_name(e) : proc_name(e->proc);
        return ordered(ops, [&](const std::vector<std::string> &t) {
          std::string s = name + "(";
          for (size_t i = 0; i < t.size(); i++) s += (i ? ", " : "") + t[i];
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
          std::string s = "((" + c_type(e->type) + "){{";
          for (size_t i = 0; i < t.size(); i++) s += (i ? ", " : "") + t[i];
          return s + "}})";
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
        std::string p = new_temp(c_type(target->type) + " *");
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
      p = new_temp(c_type(target->type) + " *");
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
        std::string t = c_type(s->type), n = std::to_string(temp_count++);
        std::string i = local_name(s->name);
        line("{");
        indent++;
        line(t + " coax_lo" + n + " = " + ex(s->expr) + ";");
        line(t + " coax_hi" + n + " = " + ex(s->expr2) + ";");
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
    std::string s = (header || p->export_c ? "" : p->foreign ? "extern " : "static ") + c_type(p->ret) + " " +
                    proc_name(p) + "(";
    if (p->params.count == 0) s += "void";
    for (int32_t i = 0; i < p->params.count; i++) {
      if (i) s += ", ";
      s += c_type(p->params[i].type);
      if (!header) s += " " + local_name(p->params[i].name);
      else if (!is_c_keyword(p->params[i].name)) s += " " + to_std(p->params[i].name);
    }
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

std::string emit_c(const File *file, const EmitOptions &opts) {
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
