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

typedef struct { const char *data; int64_t count; } coax_string;
typedef enum { COAX_ANY_INT, COAX_ANY_BOOL, COAX_ANY_STRING } coax_any_kind;
typedef struct { coax_any_kind kind; int64_t i; coax_string s; } coax_any;

#define COAX_STR(lit, n) ((coax_string){(lit), (n)})

static coax_any coax_any_int(int64_t v) { coax_any a = {COAX_ANY_INT, v, {0, 0}}; return a; }
static coax_any coax_any_bool(_Bool v) { coax_any a = {COAX_ANY_BOOL, v, {0, 0}}; return a; }
static coax_any coax_any_string(coax_string v) { coax_any a = {COAX_ANY_STRING, 0, v}; return a; }

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
)COAXRT";

namespace {

struct Emitter {
  const File *file;
  EmitOptions opts;
  std::string out;
  int32_t indent = 0;

  void line_start() { out.append((size_t)indent * 2, ' '); }

  static const char *c_type(Type t) {
    switch (t) {
      case Type::void_: return "void";
      case Type::int_: return "int64_t";
      case Type::bool_: return "_Bool";
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

  void prototype(const ProcDecl *p) {
    out += "static ";
    out += c_type(p->ret);
    out += " ";
    proc_name(p->name);
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

  void expr(const Expr *e) {
    switch (e->kind) {
      case ExprKind::int_lit:
        if (e->int_value == INT64_MIN) out += "INT64_MIN";
        else out += "INT64_C(" + std::to_string(e->int_value) + ")";
        break;
      case ExprKind::bool_lit: out += e->int_value ? "1" : "0"; break;
      case ExprKind::string_lit: string_literal(e->text); break;
      case ExprKind::ident: local_name(e->text); break;
      case ExprKind::call:
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
              out += a->type == Type::int_ ? "coax_any_int(" : a->type == Type::bool_ ? "coax_any_bool(" : "coax_any_string(";
              expr(a);
              out += ")";
            }
            out += "}";
          }
          out += ")";
        } else {
          proc_name(e->text);
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
          target.type = Type::int_;
          Expr bin{};
          bin.kind = ExprKind::binary;
          bin.loc = s->loc;
          bin.type = Type::int_;
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
    out += "static const char *coax_source_path = \"";
    for (char c : file->src->path) {
      if (c == '"' || c == '\\') out += '\\';
      out += c;
    }
    out += "\";\n";
    out += RUNTIME;
    out += "\n";
    for (const ProcDecl *p : file->procs) {
      prototype(p);
      out += ";\n";
    }
    for (const ProcDecl *p : file->procs) {
      out += "\n";
      prototype(p);
      out += "\n";
      stmt(p->body);
    }
    out += "\nint main(void) {\n  coax_p_main();\n  return 0;\n}\n";
  }
};

}  // namespace

std::string emit_c(const File *file, const EmitOptions &opts) {
  Emitter e{file, opts};
  e.run();
  return e.out;
}

}  // namespace coax
