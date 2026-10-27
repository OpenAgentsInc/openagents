// Coax compiler (coax0): shared declarations.
// The language is specified in coax/spec.md.
// Scalar bootstrap and compile-time slice (§20): a transpiler from a Coax
// subset to C. Syntax nodes are plain structs allocated in a compile arena.
#pragma once
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
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
  *p = T{};
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
    memcpy(s.data, v.data(), sizeof(T) * v.size());
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
  return a.count == b.count && memcmp(a.data, b.data, (size_t)a.count) == 0;
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
// and "fix: ..." when there is one. Counts errors.
void error_at(const Source *src, Loc loc, const char *code, const std::string &message,
              const std::string &fix = "");
extern int32_t g_error_count;

// ---- Lexer ----

enum class Tok : uint8_t {
  eof, ident, keyword, int_lit, string_lit, directive, annotation,
  lparen, rparen, lbrace, rbrace, lbracket, rbracket,
  comma, semicolon, colon, coloncolon, colonequal, arrow, dot, dotdot,
  equal, plus_equal, minus_equal, star_equal, slash_equal, percent_equal,
  plus, minus, star, slash, percent,
  eqeq, noteq, lt, le, gt, ge, andand, oror, bang,
};

struct Token {
  Tok kind = Tok::eof;
  Loc loc;
  Str text;           // identifier, keyword, directive name (without '#'), or raw literal
  int64_t int_value = 0;
  Str string_value;   // string literal after escapes
};

bool lex(Arena *arena, const Source *src, std::vector<Token> *out);
const char *tok_name(Tok t);

// ---- Types ----

enum class Type : uint8_t { invalid, void_, int_, bool_, string_ };
const char *type_name(Type t);

// ---- Syntax tree ----

enum class ExprKind : uint8_t { int_lit, bool_lit, string_lit, ident, call, unary, binary, run };

struct Expr {
  ExprKind kind;
  Loc loc;
  Type type = Type::invalid;  // set by the checker
  int64_t int_value = 0;      // int_lit, bool_lit
  Str text;                   // ident name, string literal value, call name
  Tok op = Tok::eof;          // unary, binary
  Expr *lhs = nullptr, *rhs = nullptr;  // unary uses lhs
  Slice<Expr *> args;         // call
  struct ProcDecl *proc = nullptr;      // call: resolved target (null for builtins)
  struct ConstDecl *constant = nullptr; // ident: resolved global constant
  bool builtin_print = false;
};

enum class StmtKind : uint8_t { block, expr, var, assign, ret, if_, while_ };

struct Stmt {
  StmtKind kind;
  Loc loc;
  Slice<Stmt *> body;          // block
  Expr *expr = nullptr;        // expr, var init, assign value, return value, condition
  Str name;                    // var, assign target
  Loc type_loc;
  Str type_text;               // var: declared type, empty when inferred
  Type type = Type::invalid;   // var: resolved type
  bool uninit = false;         // var: `= ---`
  Tok op = Tok::equal;         // assign: = += -= *= /= %=
  Stmt *then_ = nullptr, *else_ = nullptr;  // if (then_ is a block), while (then_)
};

struct Param {
  Str name;
  Loc loc;
  Str type_text;
  Loc type_loc;
  Type type = Type::invalid;
};

struct ProcDecl {
  Str name;
  Loc loc;
  Slice<Param> params;
  Str ret_text;
  Loc ret_loc;
  Type ret = Type::void_;
  Stmt *body = nullptr;
};

struct Import {
  Str module;
  Loc loc;
};

struct ConstDecl {
  Str name;
  Loc loc;
  Expr *expr = nullptr;  // replaced with a literal after compile-time evaluation
  Type type = Type::invalid;
  uint8_t checked = 0, evaluated = 0; // 0 unseen, 1 visiting, 2 done
};

struct File {
  const Source *src = nullptr;
  Slice<Import> imports;
  Slice<ProcDecl *> procs;
  Slice<ConstDecl *> constants;
  Slice<Expr *> assertions;
};

bool parse(Arena *arena, const Source *src, const std::vector<Token> &tokens, File *out);

// ---- Checker and C emitter ----

bool check(Arena *arena, File *file);
// Bounded, side-effect-free AST evaluation; no host I/O or foreign calls.
bool evaluate(File *file);

struct EmitOptions {
  bool release = false;  // release: integer overflow wraps; debug: it traps
};
std::string emit_c(const File *file, const EmitOptions &opts);

}  // namespace coax
