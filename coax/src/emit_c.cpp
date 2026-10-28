// C emitter: one self-contained C11 file per program. The runtime that
// Basic needs (print, checked integer arithmetic) is written into the file,
// so the output builds with any C compiler and no Coax install: Xcode,
// Emscripten and plain cc treat it like any other C source.
#include "coax.h"

namespace coax {

static const char *RUNTIME = R"COAXRT(#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>

typedef struct { const char *data; int64_t count; } coax_string;
typedef enum { COAX_ANY_INT, COAX_ANY_BOOL, COAX_ANY_STRING, COAX_ANY_REAL } coax_any_kind;
typedef struct { coax_any_kind kind; int64_t i; coax_string s; double r; } coax_any;

#define COAX_STR(lit, n) ((coax_string){(lit), (n)})

static coax_any coax_any_int(int64_t v) { coax_any a = {COAX_ANY_INT, v, {0, 0}}; return a; }
static coax_any coax_any_bool(_Bool v) { coax_any a = {COAX_ANY_BOOL, v, {0, 0}}; return a; }
static coax_any coax_any_string(coax_string v) { coax_any a = {COAX_ANY_STRING, 0, v}; return a; }

static coax_any coax_any_real(double v) { coax_any a = {COAX_ANY_REAL, 0, {0, 0}, v}; return a; }

static _Bool coax_string_eq(coax_string a, coax_string b) {
  return a.count == b.count && (a.count == 0 || memcmp(a.data, b.data, (size_t)a.count) == 0);
}

/* The format was checked at compile time: one argument per '%', '%%' is '%'. */
static void coax_print(coax_string fmt, int64_t n, const coax_any *args) {
  int64_t next = 0, run = 0;
  for (int64_t i = 0; i < fmt.count; i++) {
    if (fmt.data[i] != '%') continue;
    fwrite(fmt.data + run, 1, (size_t)(i - run), stdout);
    if (i + 1 < fmt.count && fmt.data[i + 1] == '%') { fputc('%', stdout); i++; run = i + 1; continue; }
    run = i + 1;
    if (next >= n) continue;
    const coax_any *a = &args[next++];
    switch (a->kind) {
      case COAX_ANY_INT: printf("%lld", (long long)a->i); break;
      case COAX_ANY_BOOL: fputs(a->i ? "true" : "false", stdout); break;
      case COAX_ANY_STRING: fwrite(a->s.data, 1, (size_t)a->s.count, stdout); break;
      case COAX_ANY_REAL: printf("%.17g", a->r); break;
    }
  }
  fwrite(fmt.data + run, 1, (size_t)(fmt.count - run), stdout);
}

static void coax_trap(const char *what, int32_t line) {
  fflush(stdout);
  fprintf(stderr, "%s:%d: %s\n", coax_source_path, line, what);
  abort();
}

static int64_t coax_add(int64_t a, int64_t b, int32_t line) {
  int64_t r; if (__builtin_add_overflow(a, b, &r)) coax_trap("integer overflow in '+'", line); return r;
}
static int64_t coax_sub(int64_t a, int64_t b, int32_t line) {
  int64_t r; if (__builtin_sub_overflow(a, b, &r)) coax_trap("integer overflow in '-'", line); return r;
}
static int64_t coax_mul(int64_t a, int64_t b, int32_t line) {
  int64_t r; if (__builtin_mul_overflow(a, b, &r)) coax_trap("integer overflow in '*'", line); return r;
}
static int64_t coax_div(int64_t a, int64_t b, int32_t line) {
  if (b == 0) coax_trap("division by zero", line);
  if (a == INT64_MIN && b == -1) coax_trap("integer overflow in '/'", line);
  return a / b;
}
static int64_t coax_mod(int64_t a, int64_t b, int32_t line) {
  if (b == 0) coax_trap("division by zero in '%'", line);
  if (b == -1) return 0;
  return a % b;
}
static int64_t coax_neg(int64_t a, int32_t line) {
  if (a == INT64_MIN) coax_trap("integer overflow in unary '-'", line); return -a;
}
/* Release builds: wrapping arithmetic, spelled out so no C undefined behavior remains. */
static int64_t coax_wadd(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
static int64_t coax_wsub(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
static int64_t coax_wmul(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }
static int64_t coax_wneg(int64_t a) { return (int64_t)(0 - (uint64_t)a); }
static int64_t coax_shift(int64_t a, int64_t b, int left, int32_t line) {
  if (b < 0 || b > 63) coax_trap("shift count must be in 0..63", line);
  if (left) return (int64_t)((uint64_t)a << b);
  uint64_t bits = (uint64_t)a >> b;
  if (a < 0 && b) bits |= UINT64_MAX << (64 - b);
  return (int64_t)bits;
}
static float coax_to_f32(double value, int32_t line) {
  if (!isfinite(value) || value < -FLT_MAX || value > FLT_MAX)
    coax_trap("floating-point value does not fit in f32", line);
  return (float)value;
}
static int64_t coax_to_int(double value, int32_t line) {
  if (!isfinite(value) || value < -9223372036854775808.0 || value >= 9223372036854775808.0)
    coax_trap("floating-point value does not fit in int", line);
  return (int64_t)value;
}
static int64_t coax_byte_at(coax_string s, int64_t index, int32_t line) {
  if (index < 0 || index >= s.count) coax_trap("byte index out of bounds", line);
  return (unsigned char)s.data[index];
}
)COAXRT";

namespace {

struct Emitter {
  const File *file;
  EmitOptions opts;
  std::string out;
  int32_t indent = 0;
  const Stmt *function_root = nullptr;
  std::vector<Type> temporaries;
  std::unordered_map<const Expr *, std::vector<int32_t>> ordered;

  void collect_expr(const Expr *e) {
    if (!e) return;
    bool binary = e->kind == ExprKind::binary;
    bool call = e->kind == ExprKind::call;
    if (binary && e->op != Tok::andand && e->op != Tok::oror) {
      int a = (int)temporaries.size(); temporaries.push_back(e->lhs->type);
      int b = (int)temporaries.size(); temporaries.push_back(e->rhs->type);
      ordered[e] = {a, b};
    } else if (call && e->args.count >= 2) {
      std::vector<int32_t> slots;
      for (int i = 0; i < e->args.count; i++) {
        if (e->builtin_print && i == 0) { slots.push_back(-1); continue; }
        slots.push_back((int32_t)temporaries.size());
        temporaries.push_back(e->args[i]->type);
      }
      ordered[e] = slots;
    }
    if (binary || e->kind == ExprKind::unary) { collect_expr(e->lhs); collect_expr(e->rhs); }
    if (call) for (Expr *a : e->args) collect_expr(a);
  }

  void collect_stmt(const Stmt *s) {
    if (!s) return;
    collect_expr(s->expr);
    for (const Stmt *child : s->body) collect_stmt(child);
    collect_stmt(s->then_); collect_stmt(s->else_);
  }

  void temp_name(int32_t id) { out += "coax_t_" + std::to_string(id); }

  bool ordered_expr(const Expr *e) {
    auto found = ordered.find(e);
    if (found == ordered.end()) return false;
    const auto &slots = found->second;
    Expr clone = *e;
    std::vector<Expr> values(slots.size());
    std::vector<Expr *> arguments(slots.size());
    out += "(";
    for (size_t i = 0; i < slots.size(); i++) {
      const Expr *input = e->kind == ExprKind::binary ? (i == 0 ? e->lhs : e->rhs) : e->args[(int)i];
      if (slots[i] < 0) { arguments[i] = const_cast<Expr *>(input); continue; }
      temp_name(slots[i]); out += " = "; expr(input); out += ", ";
      values[i].kind = ExprKind::ident;
      values[i].type = input->type;
      values[i].temporary = slots[i];
      arguments[i] = &values[i];
    }
    if (e->kind == ExprKind::binary) { clone.lhs = arguments[0]; clone.rhs = arguments[1]; }
    else clone.args = {arguments.data(), (int32_t)arguments.size()};
    expr(&clone);
    out += ")";
    return true;
  }

  void line_start() { out.append((size_t)indent * 2, ' '); }

  const char *c_type(Type t) const {
    switch (t) {
      case Type::void_: return "void";
      case Type::int_: return "int64_t";
      case Type::bool_: return opts.metal ? "bool" : "_Bool";
      case Type::f32_: return "float";
      case Type::f64_: return "double";
      case Type::string_: return "coax_string";
      default: return "void";
    }
  }

  // Coax names never collide with C keywords or the runtime: locals get
  // "l_", procedures "coax_p_".
  void local_name(Str s) { out += "l_"; out.append(s.data, (size_t)s.count); }
  void proc_name(Str s) { out += "coax_p_"; out.append(s.data, (size_t)s.count); }

  void string_literal(Str s) {
    out += "COAX_STR(\"";
    for (int32_t i = 0; i < s.count; i++) {
      unsigned char c = (unsigned char)s.data[i];
      if (c == '"' || c == '\\') { out += '\\'; out += (char)c; }
      else if (c == '\n') out += "\\n";
      else if (c == '\t') out += "\\t";
      else if (c == '\r') out += "\\r";
      else if (c < 0x20 || c >= 0x7f) {
        char buf[8];
        snprintf(buf, sizeof buf, "\\%03o", c);  // octal: never swallows a following digit
        out += buf;
      } else out += (char)c;
    }
    out += "\", " + std::to_string(s.count) + ")";
  }

  void function_name(const ProcDecl *p) {
    if (p->foreign || p->exported) out += to_std(p->name);
    else proc_name(p->name);
  }

  void prototype(const ProcDecl *p) {
    out += p->foreign ? "extern " : p->exported ? "" : "static ";
    out += c_type(p->ret);
    out += " ";
    function_name(p);
    out += "(";
    if (p->params.count == 0) out += "void";
    for (int32_t i = 0; i < p->params.count; i++) {
      if (i) out += ", ";
      out += c_type(p->params[i].type);
      out += " ";
      local_name(p->params[i].name);
    }
    out += ")";
  }

  void arith(const char *checked, const char *wrapping, Expr *a, Expr *b, int32_t line) {
    out += opts.release ? wrapping : checked;
    out += "(";
    expr(a);
    out += ", ";
    expr(b);
    if (!opts.release) out += ", " + std::to_string(line);
    out += ")";
  }

  void intrinsic_expr(const Expr *e) {
    Intrinsic intrinsic = e->intrinsic;
    if (intrinsic == Intrinsic::len) { out += "("; expr(e->args[0]); out += ").count"; return; }
    if (intrinsic == Intrinsic::byte_at) {
      out += "coax_byte_at("; expr(e->args[0]); out += ", "; expr(e->args[1]);
      out += ", " + std::to_string(e->loc.line) + ")"; return;
    }
    if (intrinsic == Intrinsic::integer && e->args[0]->type != Type::int_) {
      out += "coax_to_int("; expr(e->args[0]); out += ", " + std::to_string(e->loc.line) + ")"; return;
    }
    if (!opts.metal && intrinsic == Intrinsic::f32 && e->args[0]->type != Type::f32_) {
      out += "coax_to_f32("; expr(e->args[0]); out += ", " + std::to_string(e->loc.line) + ")"; return;
    }
    if (intrinsic == Intrinsic::integer || intrinsic == Intrinsic::f32 || intrinsic == Intrinsic::f64) {
      out += "(("; out += c_type(e->type); out += ")("; expr(e->args[0]); out += "))"; return;
    }
    std::string name = intrinsic == Intrinsic::abs ? (opts.metal ? "abs" : "fabs") : to_std(e->text);
    if (!opts.metal && e->type == Type::f32_) name += "f";
    out += name + "(";
    for (int i = 0; i < e->args.count; i++) { if (i) out += ", "; expr(e->args[i]); }
    out += ")";
  }

  void expr(const Expr *e) {
    if (ordered_expr(e)) return;
    switch (e->kind) {
      case ExprKind::int_lit:
        if (e->int_value == INT64_MIN) out += "INT64_MIN";
        else out += "INT64_C(" + std::to_string(e->int_value) + ")";
        break;
      case ExprKind::real_lit: {
        char value[64]; snprintf(value, sizeof value, "%.17g", e->real_value);
        std::string number = value;
        if (number.find_first_of(".eE") == std::string::npos) number += ".0";
        if (e->type == Type::f32_) number += "f";
        out += number;
        break;
      }
      case ExprKind::bool_lit: out += e->int_value ? (opts.metal ? "true" : "1") : (opts.metal ? "false" : "0"); break;
      case ExprKind::string_lit: string_literal(e->text); break;
      case ExprKind::ident:
        if (e->temporary >= 0) temp_name(e->temporary);
        else if (e->constant) expr(e->constant->expr);
        else local_name(e->text);
        break;
      case ExprKind::run:
        // Evaluation must replace every #run with a target-independent literal.
        fputs("coax: internal error: unevaluated #run reached the C emitter\n", stderr);
        abort();
      case ExprKind::call:
        if (e->intrinsic != Intrinsic::none) { intrinsic_expr(e); break; }
        if (e->builtin_print) {
          out += "coax_print(";
          string_literal(e->args[0]->text);
          int32_t n = e->args.count - 1;
          out += ", " + std::to_string(n) + ", ";
          if (n == 0) {
            out += "NULL";
          } else {
            out += "(coax_any[]){";
            for (int32_t i = 1; i < e->args.count; i++) {
              if (i > 1) out += ", ";
              const Expr *a = e->args[i];
              out += a->type == Type::int_ ? "coax_any_int(" : a->type == Type::bool_ ? "coax_any_bool(" : is_real(a->type) ? "coax_any_real(" : "coax_any_string(";
              expr(a);
              out += ")";
            }
            out += "}";
          }
          out += ")";
        } else {
          function_name(e->proc);
          out += "(";
          for (int32_t i = 0; i < e->args.count; i++) {
            if (i) out += ", ";
            expr(e->args[i]);
          }
          out += ")";
        }
        break;
      case ExprKind::unary:
        if (e->op == Tok::bang) {
          out += "(!";
          expr(e->lhs);
          out += ")";
        } else if (is_real(e->type)) {
          out += "(-"; expr(e->lhs); out += ")";
        } else if (opts.release) {
          out += "coax_wneg(";
          expr(e->lhs);
          out += ")";
        } else {
          out += "coax_neg(";
          expr(e->lhs);
          out += ", " + std::to_string(e->loc.line) + ")";
        }
        break;
      case ExprKind::binary: {
        int32_t line = e->loc.line;
        if (e->op == Tok::shl || e->op == Tok::shr) {
          out += "coax_shift("; expr(e->lhs); out += ", "; expr(e->rhs);
          out += e->op == Tok::shl ? ", 1, " : ", 0, "; out += std::to_string(line) + ")"; return;
        }
        if (is_real(e->type)) {
          out += "("; expr(e->lhs);
          out += e->op == Tok::plus ? " + " : e->op == Tok::minus ? " - " : e->op == Tok::star ? " * " : " / ";
          expr(e->rhs); out += ")"; return;
        }
        switch (e->op) {
          case Tok::plus: arith("coax_add", "coax_wadd", e->lhs, e->rhs, line); return;
          case Tok::minus: arith("coax_sub", "coax_wsub", e->lhs, e->rhs, line); return;
          case Tok::star: arith("coax_mul", "coax_wmul", e->lhs, e->rhs, line); return;
          // Division by zero is a trap in every build: there is no wrapping answer.
          case Tok::slash:
          case Tok::percent:
            out += e->op == Tok::slash ? "coax_div(" : "coax_mod(";
            expr(e->lhs);
            out += ", ";
            expr(e->rhs);
            out += ", " + std::to_string(line) + ")";
            return;
          default: break;
        }
        if (e->lhs->type == Type::string_) {
          out += e->op == Tok::noteq ? "(!coax_string_eq(" : "(coax_string_eq(";
          expr(e->lhs);
          out += ", ";
          expr(e->rhs);
          out += "))";
          return;
        }
        const char *op = "";
        switch (e->op) {
          case Tok::bitand_: op = " & "; break;
          case Tok::bitor_: op = " | "; break;
          case Tok::bitxor_: op = " ^ "; break;
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
        out += "(";
        expr(e->lhs);
        out += op;
        expr(e->rhs);
        out += ")";
        break;
      }
    }
  }

  void stmt(const Stmt *s) {
    switch (s->kind) {
      case StmtKind::block:
        line_start();
        out += "{\n";
        indent++;
        if (s == function_root) {
          for (size_t i = 0; i < temporaries.size(); i++) {
            line_start(); out += c_type(temporaries[i]); out += " "; temp_name((int32_t)i);
            out += temporaries[i] == Type::string_ ? " = {0};\n" : " = 0;\n";
          }
        }
        for (const Stmt *c : s->body) stmt(c);
        indent--;
        line_start();
        out += "}\n";
        break;
      case StmtKind::expr:
        line_start();
        expr(s->expr);
        out += ";\n";
        break;
      case StmtKind::var:
        line_start();
        out += c_type(s->type);
        out += " ";
        local_name(s->name);
        if (s->expr) {
          out += " = ";
          expr(s->expr);
        } else if (!s->uninit) {
          // Zero unless `---` (spec §6.1).
          out += s->type == Type::string_ ? " = COAX_STR(0, 0)" : " = 0";
        }
        out += ";\n";
        break;
      case StmtKind::assign: {
        line_start();
        local_name(s->name);
        out += " = ";
        if (s->op == Tok::equal) {
          expr(s->expr);
        } else {
          Expr target{};
          target.kind = ExprKind::ident;
          target.text = s->name;
          target.type = s->type;
          Expr bin{};
          bin.kind = ExprKind::binary;
          bin.loc = s->loc;
          bin.type = s->type;
          bin.lhs = &target;
          bin.rhs = s->expr;
          bin.op = s->op == Tok::plus_equal ? Tok::plus
                 : s->op == Tok::minus_equal ? Tok::minus
                 : s->op == Tok::star_equal ? Tok::star
                 : s->op == Tok::slash_equal ? Tok::slash
                 : Tok::percent;
          expr(&bin);
        }
        out += ";\n";
        break;
      }
      case StmtKind::ret:
        line_start();
        out += "return";
        if (s->expr) {
          out += " ";
          expr(s->expr);
        }
        out += ";\n";
        break;
      case StmtKind::if_: {
        line_start();
        out += "if (";
        expr(s->expr);
        out += ")\n";
        stmt(s->then_);
        if (s->else_) {
          line_start();
          out += "else\n";
          stmt(s->else_);
        }
        break;
      }
      case StmtKind::while_:
        line_start();
        out += "while (";
        expr(s->expr);
        out += ")\n";
        stmt(s->then_);
        break;
    }
  }

  void run() {
    out += "/* Generated by coax0 from ";
    out += file->src->path;
    out += ". Do not edit: change the .coax file. */\n";
    if (opts.metal) out += "#include <metal_stdlib>\nusing namespace metal;\n";
    else {
      out += "static const char *coax_source_path = \"";
      for (char c : file->src->path) { if (c == '"' || c == '\\') out += '\\'; out += c; }
      out += "\";\n";
      out += RUNTIME;
    }
    out += "\n";
    for (const ProcDecl *p : file->procs) {
      prototype(p);
      out += ";\n";
    }
    for (const ProcDecl *p : file->procs) {
      if (p->foreign) continue;
      function_root = p->body;
      temporaries.clear(); ordered.clear(); collect_stmt(p->body);
      out += "\n";
      prototype(p);
      out += "\n";
      stmt(p->body);
    }
    if (!opts.library) out += "\nint main(void) {\n  coax_p_main();\n  return 0;\n}\n";
  }
};

}  // namespace

std::string emit_c(const File *file, const EmitOptions &opts) {
  Emitter e{file, opts};
  e.run();
  return e.out;
}

std::string emit_header(const File *file) {
  Emitter e{file, {}};
  e.out = "/* Generated Coax fixed-scalar ABI. */\n#pragma once\n#include <stdint.h>\n#include <stdbool.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n";
  for (const ProcDecl *p : file->procs) {
    if (!p->exported && !p->foreign) continue;
    e.prototype(p); e.out += ";\n";
  }
  // C++ doesn't spell C's _Bool, while stdbool.h provides bool in C too.
  size_t start = 0;
  while ((start = e.out.find("_Bool", start)) != std::string::npos) { e.out.replace(start, 5, "bool"); start += 4; }
  e.out += "#ifdef __cplusplus\n}\n#endif\n";
  return e.out;
}

}  // namespace coax
