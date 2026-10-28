// Coax compiler (coax0): shared declarations.
// The language is specified in coax/spec.md.
// Stage 2 slice (§20): sized scalars, structs, enums, fixed arrays, views,
// for/defer/ifx and the C export boundary, transpiled to C11. Syntax nodes
// are plain structs allocated in a compile arena.
#pragma once
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

namespace coax {

// ---- Memory: one arena for the whole compile; nothing is freed. ----

struct Arena {
  uint8_t *block = nullptr;
  size_t used = 0, cap = 0;
};

void *arena_alloc(Arena *a, size_t size);

template <class T> T *make(Arena *a) {
  T *p = (T *)arena_alloc(a, sizeof(T));
  new (p) T{};
  return p;
}

template <class T> struct Slice {
  T *data = nullptr;
  int32_t count = 0;
  T &operator[](int32_t i) const { return data[i]; }
  T *begin() const { return data; }
  T *end() const { return data + count; }
};

template <class T> Slice<T> to_slice(Arena *a, const std::vector<T> &v) {
  Slice<T> s;
  s.count = (int32_t)v.size();
  if (s.count) {
    s.data = (T *)arena_alloc(a, sizeof(T) * v.size());
    memcpy((void *)s.data, (const void *)v.data(), sizeof(T) * v.size());
  }
  return s;
}

struct Str {  // a view into source text or the arena; not NUL-terminated
  const char *data = nullptr;
  int32_t count = 0;
};

inline bool str_eq(Str a, const char *b) {
  size_t n = strlen(b);
  return (size_t)a.count == n && memcmp(a.data, b, n) == 0;
}
inline bool str_eq(Str a, Str b) {
  return a.count == b.count && (a.count == 0 || memcmp(a.data, b.data, (size_t)a.count) == 0);
}
inline std::string to_std(Str s) { return std::string(s.data, (size_t)s.count); }

// ---- Source and diagnostics ----

struct Loc {
  int32_t line = 0, col = 0, offset = 0;
};

struct Source {
  std::string path;
  std::string text;
};

// Prints "path:line:col: error[Exxxx]: message", the source line, a caret,
// and "fix: ..." when there is one; or one JSON object per line with
// --json. Counts errors.
void error_at(const Source *src, Loc loc, const char *code, const std::string &message,
              const std::string &fix = "");
extern int32_t g_error_count;
extern bool g_json_errors;

// Explanations for every error code, shared by `coax explain` and the book.
struct ErrorInfo {
  const char *code;
  const char *title;
  const char *explanation;
};
extern const ErrorInfo ERRORS[];
extern const int32_t ERROR_COUNT;

// ---- Lexer ----

enum class Tok : uint8_t {
  eof, ident, keyword, int_lit, float_lit, string_lit, directive, annotation,
  lparen, rparen, lbrace, rbrace, lbracket, rbracket,
  comma, semicolon, colon, coloncolon, colonequal, arrow, dot, dotdot, dotdotlt,
  equal, plus_equal, minus_equal, star_equal, slash_equal, percent_equal,
  amp_equal, pipe_equal, caret_equal, shl_equal, shr_equal,
  plus, minus, star, slash, percent, amp, pipe, caret, shl, shr,
  eqeq, noteq, lt, le, gt, ge, andand, oror, bang,
};

struct Token {
  Tok kind = Tok::eof;
  Loc loc;
  Str text;            // identifier, keyword, directive name (without '#'), or raw literal
  uint64_t int_value = 0;
  double float_value = 0;
  uint8_t float_bits = 0;  // float_lit: 32 or 64 for a 1.5f32 / 1.5f64 suffix, 0 for untyped
  Str string_value;    // string literal after escapes
};

bool lex(Arena *arena, const Source *src, std::vector<Token> *out);
const char *tok_name(Tok t);

// ---- Types ----
// Types are interned: two types are the same type exactly when their
// TypeInfo pointers are equal.

enum class TypeKind : uint8_t {
  invalid, void_, integer, floating, bool_, string_, untyped_int, untyped_float,
  struct_, enum_, array, view,
};

struct StructDecl;
struct EnumDecl;

struct TypeInfo {
  TypeKind kind = TypeKind::invalid;
  std::string name;          // as Coax spells it: "s32", "Vec2", "[4] int", "[] u8"
  int32_t size = 0, align = 1;  // host C layout (LP64)
  int32_t bits = 0;          // integer and floating widths
  bool is_signed = false;
  const TypeInfo *elem = nullptr;  // array, view; enum: the underlying integer type
  int64_t length = 0;        // array
  StructDecl *struct_decl = nullptr;
  EnumDecl *enum_decl = nullptr;
  int32_t id = 0;            // generated C names for arrays and views
};
using Type = const TypeInfo *;

extern TypeInfo T_INVALID, T_VOID, T_BOOL, T_STRING, T_UNTYPED_INT, T_UNTYPED_FLOAT, T_F32, T_F64;
extern TypeInfo T_S8, T_S16, T_S32, T_S64, T_U8, T_U16, T_U32, T_U64;
#define T_INT (&T_S64)  // int is s64: one type, spelled "int"

Type array_of(Type elem, int64_t length);
Type view_of(Type elem);
const char *type_name(Type t);
inline bool is_int(Type t) { return t->kind == TypeKind::integer; }
inline bool is_float(Type t) { return t->kind == TypeKind::floating; }
inline bool is_number(Type t) { return is_int(t) || is_float(t); }
inline bool is_untyped(Type t) { return t->kind == TypeKind::untyped_int || t->kind == TypeKind::untyped_float; }
inline bool is_aggregate(Type t) { return t->kind == TypeKind::struct_ || t->kind == TypeKind::array; }
const std::vector<Type> &all_composite_types();  // arrays and views, in creation order

// ---- Compile-time values ----
// A value holds scalars, byte strings, and aggregates (struct fields or array
// elements). Views share their storage with the aggregate they borrow. A
// value never holds a host address, so nothing of the compiler's memory can
// reach a target executable.

struct Value {
  Type type = &T_VOID;
  uint64_t bits = 0;  // integers (two's complement bit pattern), bool, enum
  double f = 0;       // f32 values are stored rounded to float
  Str s;
  std::shared_ptr<std::vector<Value>> agg;  // struct fields, array elements, view storage
  int64_t offset = 0, count = 0;            // view: window into agg
};

// ---- Syntax tree ----

struct Expr;

struct TypeExpr {
  enum Kind : uint8_t { named, array, view, dynamic } kind = named;
  Loc loc;
  Str name;                  // named
  Expr *length = nullptr;    // array
  TypeExpr *elem = nullptr;  // array, view, dynamic
};

enum class ExprKind : uint8_t {
  int_lit, float_lit, bool_lit, string_lit, ident, call, unary, binary, run,
  field, index, slice, struct_lit, array_lit, cast, ifx, enum_value, ct_value,
};

// Built-in math, the same in C, Metal and compile-time evaluation.
enum class Intrinsic : uint8_t { none, sin, cos, tan, exp, log, sqrt, floor, abs, pow };
Intrinsic find_intrinsic(Str name);

enum class BinaryIntrinsic : uint8_t { none, bytes_overlap, f32_from_bits, f64_from_bits, f32_bits, f64_bits };
BinaryIntrinsic find_binary_intrinsic(Str name);

struct FieldInit {
  Str name;
  Loc loc;
  Expr *value = nullptr;
  int32_t field = -1;  // resolved field index
};

struct Expr {
  ExprKind kind;
  Loc loc;
  Type type = &T_INVALID;     // set by the checker
  uint64_t int_value = 0;     // int_lit (bit pattern), bool_lit, enum_value
  double float_value = 0;     // float_lit
  Str text;                   // ident name, string literal value, call name, field name
  Tok op = Tok::eof;          // unary, binary
  Expr *lhs = nullptr, *rhs = nullptr, *third = nullptr;
  // unary/run/cast/field: lhs. binary: lhs, rhs. index: lhs[rhs].
  // slice: lhs[rhs ..< third] (rhs/third null for `[..]`). ifx: lhs then rhs else third.
  Slice<Expr *> args;         // call, array_lit
  Slice<FieldInit> fields;    // struct_lit
  TypeExpr *type_expr = nullptr;  // cast, struct_lit, array_lit
  bool trunc = false;         // cast,trunc
  bool negative = false;      // untyped int_lit: int_value holds a negative s64
  bool folded = false;        // untyped int_lit computed from an operation, not written
  int32_t field_index = -1;   // field
  struct ProcDecl *proc = nullptr;      // call: resolved target (null for builtins)
  struct ConstDecl *constant = nullptr; // ident: resolved global constant
  bool builtin_print = false;
  BinaryIntrinsic binary_intrinsic = BinaryIntrinsic::none;
  Intrinsic intrinsic = Intrinsic::none;  // call: built-in math (proc is null)
  Value *ct = nullptr;        // ct_value: an aggregate computed at compile time
  int32_t ct_id = -1;         // ct_value: emitted as a static constant
};

enum class StmtKind : uint8_t { block, expr, var, assign, ret, if_, while_, for_range, for_each, break_, continue_, defer, assert_ };

struct Stmt {
  StmtKind kind;
  Loc loc;
  Slice<Stmt *> body;          // block
  Expr *expr = nullptr;        // expr, var init, assign value, return value, condition,
                               // for_range lower bound, for_each collection
  Expr *expr2 = nullptr;       // for_range upper bound
  Expr *target = nullptr;      // assign: the place
  Str name, name2;             // var; for: element/counter and optional index
  Loc name2_loc;
  TypeExpr *type_expr = nullptr;  // var: declared type, null when inferred
  Type type = &T_INVALID;      // var/for: resolved type of `name`
  bool uninit = false;         // var: `= ---`
  Tok op = Tok::equal;         // assign: = += -= ...
  Stmt *then_ = nullptr, *else_ = nullptr;  // if (then_ is a block), loops and defer (then_)
};

struct Param {
  Str name;
  Loc loc;
  TypeExpr *type_expr = nullptr;
  Type type = &T_INVALID;
};

struct ProcDecl {
  Str name;
  Loc loc;
  Slice<Param> params;
  TypeExpr *ret_expr = nullptr;
  Type ret = &T_VOID;
  Stmt *body = nullptr;
  bool export_c = false;
  bool foreign = false;  // #foreign: a C symbol the host provides; no body
};

struct Field {
  Str name;
  Loc loc;
  TypeExpr *type_expr = nullptr;
  Type type = &T_INVALID;
  Expr *init = nullptr;  // a constant default, frozen to a literal by evaluation
  int32_t offset = 0;
};

struct StructDecl {
  Str name;
  Loc loc;
  Slice<Field> fields;
  TypeInfo *type = nullptr;
  uint8_t state = 0;     // layout: 0 unseen, 1 visiting, 2 done
  uint8_t defaults = 0;  // default evaluation: 0 unseen, 1 visiting, 2 done
};

struct EnumMember {
  Str name;
  Loc loc;
  Expr *value = nullptr;  // optional; otherwise previous + 1
  uint64_t bits = 0;
};

struct EnumDecl {
  Str name;
  Loc loc;
  TypeExpr *base_expr = nullptr;
  Slice<EnumMember> members;
  TypeInfo *type = nullptr;
};

struct Import {
  Str module;
  Loc loc;
};

struct ConstDecl {
  Str name;
  Loc loc;
  Expr *expr = nullptr;  // replaced with a literal after compile-time evaluation
  Type type = &T_INVALID;
  uint8_t checked = 0, evaluated = 0; // 0 unseen, 1 visiting, 2 done
};

// #test "name" { ... } and #forall a: u8, b: 0..<10 { ... }: checks the
// evaluator runs (coax test), never emitted. A check is a procedure with no
// name in the program's namespace; a #forall's bindings are its parameters,
// enumerated over their whole domain.
struct Binding {
  Expr *lo = nullptr, *hi = nullptr;  // a lo..<hi range of int; null for a type domain
};

struct CheckDecl {
  Str name;               // the #test's string, or the bindings' text for an unnamed #forall
  Loc loc;
  bool forall = false;
  Slice<Binding> domains;  // one per parameter of proc
  ProcDecl *proc = nullptr;
};

struct File {
  const Source *src = nullptr;
  Slice<Import> imports;
  Slice<ProcDecl *> procs;
  Slice<ConstDecl *> constants;
  Slice<StructDecl *> structs;
  Slice<EnumDecl *> enums;
  Slice<Expr *> assertions;
  Slice<CheckDecl *> checks;
  bool authority = false;  // #authority: every procedure must be exercised by a check
  bool library = false;  // set by the driver: no C main, main optional
  bool metal = false;    // checked against the Metal scalar profile
};

bool parse(Arena *arena, const Source *src, const std::vector<Token> &tokens, File *out);

// ---- Checker, evaluator and C emitter ----

bool check(Arena *arena, File *file);
// Bounded, side-effect-free AST evaluation; no host I/O or foreign calls.
bool evaluate(File *file);
// Whether an expression contains a procedure call (whose effects order can observe).
bool has_call(const Expr *e);

struct EmitOptions {
  bool release = false;  // release: integer overflow wraps; debug: it traps
  bool library = false;  // no C main; #export_c procedures are the interface
  bool metal = false;    // Metal Shading Language instead of C
};
std::string emit_c(const File *file, const EmitOptions &opts);
// A C header for the file's #export_c procedures and the types they use,
// with layout assertions the C compiler checks.
std::string emit_header(const File *file);
// A human-readable layout report for every struct.
std::string layout_report(const File *file);

// ---- Checks: coax test and coax mutate ----

struct CheckOutcome {
  bool passed = false;
  uint64_t cases = 0, steps = 0;
  Loc loc;                     // the failure
  std::string code, message;   // the failure's error code and message
  std::string counterexample;  // #forall: "a = 3, b = true"
};
// Runs one check in the evaluator with its own step budget. Never prints.
CheckOutcome run_check(File *file, CheckDecl *check, uint64_t max_steps);
// The source line and a caret under loc, as diagnostics print them.
void show_source(FILE *out, const Source *src, Loc loc);

}  // namespace coax
