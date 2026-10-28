// Checker: names, types, returns and print formats. Reports every error it
// finds, each with a code and, where it can, a fix.
#include "coax.h"

namespace coax {

const char *type_name(Type t) {
  switch (t) {
    case Type::invalid: return "<invalid>";
    case Type::void_: return "void";
    case Type::int_: return "int";
    case Type::bool_: return "bool";
    case Type::string_: return "string";
    case Type::f32_: return "f32";
    case Type::f64_: return "f64";
  }
  return "?";
}

Intrinsic find_intrinsic(Str name) {
  const char *names[] = {"", "f32", "f64", "int", "sin", "cos", "tan", "exp", "log", "sqrt", "floor", "abs", "pow", "len", "byte_at"};
  for (int i = 1; i < int(sizeof(names) / sizeof(names[0])); i++)
    if (str_eq(name, names[i])) return Intrinsic(i);
  return Intrinsic::none;
}

namespace {

// Types the spec names (§6.2) that coax0 does not implement yet.
const char *LATER_TYPES[] = {"s8", "s16", "s32", "u8", "u16", "u32", "u64", "uint",
                             "fixed32", "fixed64", "v2", "v3", "v4", "quat", "m3", "m4", "Any"};

struct Local {
  Str name;
  Type type;
  Loc loc;
  bool immutable = false;
};

struct Checker {
  Arena *arena;
  File *file;
  const Source *src;
  bool has_basic = false;
  std::vector<Local> locals;
  ProcDecl *current = nullptr;
  uint32_t constant_depth = 0;
  bool library = false, metal = false;

  void err(Loc loc, const char *code, const std::string &msg, const std::string &fix = "") {
    error_at(src, loc, code, msg, fix);
  }

  Type resolve_type(Str text, Loc loc) {
    if (str_eq(text, "int") || str_eq(text, "s64")) return Type::int_;
    if (str_eq(text, "f32")) return Type::f32_;
    if (str_eq(text, "f64")) return Type::f64_;
    if (str_eq(text, "bool")) return Type::bool_;
    if (str_eq(text, "string")) return Type::string_;
    for (const char *t : LATER_TYPES)
      if (str_eq(text, t)) {
        err(loc, "E0301", "type '" + to_std(text) + "' is not supported by coax0 yet", "use int, bool or string");
        return Type::invalid;
      }
    err(loc, "E0302", "unknown type '" + to_std(text) + "'");
    return Type::invalid;
  }

  ProcDecl *find_proc(Str name) {
    for (ProcDecl *p : file->procs)
      if (str_eq(p->name, name)) return p;
    return nullptr;
  }

  ConstDecl *find_constant(Str name) {
    for (ConstDecl *c : file->constants)
      if (str_eq(c->name, name)) return c;
    return nullptr;
  }

  Type constant_type(ConstDecl *c) {
    if (c->checked == 2) return c->type;
    if (c->checked == 1) {
      err(c->loc, "E0340", "constant dependency cycle at '" + to_std(c->name) + "'");
      return Type::invalid;
    }
    if (constant_depth >= 64) {
      err(c->loc, "E0347", "constant type dependency depth exceeds 64");
      return Type::invalid;
    }
    constant_depth++;
    c->checked = 1;
    c->type = expr(c->expr);
    if (c->type == Type::void_)
      err(c->loc, "E0341", "a constant must have a value");
    c->checked = 2;
    constant_depth--;
    return c->type;
  }

  Local *find_local(Str name) {
    for (size_t i = locals.size(); i-- > 0;)
      if (str_eq(locals[i].name, name)) return &locals[i];
    return nullptr;
  }

  bool declare(Str name, Type type, Loc loc, bool immutable = false) {
    if (Local *prev = find_local(name)) {
      err(loc, "E0303",
          "'" + to_std(name) + "' is already declared at line " + std::to_string(prev->loc.line) +
              "; Coax does not allow shadowing",
          "pick another name");
      return false;
    }
    if (find_proc(name)) {
      err(loc, "E0304", "'" + to_std(name) + "' is already the name of a procedure", "pick another name");
      return false;
    }
    if (find_constant(name) || str_eq(name, "type_name") || find_intrinsic(name) != Intrinsic::none) {
      err(loc, "E0342", "'" + to_std(name) + "' is already a constant or intrinsic", "pick another name");
      return false;
    }
    locals.push_back(Local{name, type, loc, immutable});
    return true;
  }

  // ---- expressions ----

  Type expr(Expr *e) {
    e->type = expr_inner(e);
    if (metal && (e->type == Type::int_ || e->type == Type::f64_ || e->type == Type::string_ || e->builtin_print))
      err(e->loc, "E0351", "unsupported expression in Metal scalar library");
    return e->type;
  }

  Type expr_inner(Expr *e) {
    switch (e->kind) {
      case ExprKind::int_lit: return Type::int_;
      case ExprKind::bool_lit: return Type::bool_;
      case ExprKind::string_lit: return Type::string_;
      case ExprKind::real_lit: return e->type;
      case ExprKind::run: {
        Type t = expr(e->lhs);
        if (t == Type::void_) err(e->loc, "E0341", "#run must produce a value");
        return t;
      }
      case ExprKind::ident: {
        if (Local *l = find_local(e->text)) return l->type;
        if (ConstDecl *c = find_constant(e->text)) {
          e->constant = c;
          return constant_type(c);
        }
        if (find_proc(e->text)) {
          err(e->loc, "E0305", "procedures are not values in coax0", "call it: " + to_std(e->text) + "()");
          return Type::invalid;
        }
        err(e->loc, "E0306", "'" + to_std(e->text) + "' is not declared");
        return Type::invalid;
      }
      case ExprKind::call: return call(e);
      case ExprKind::unary: {
        Type t = expr(e->lhs);
        if (t == Type::invalid) return t;
        if (e->op == Tok::minus && !is_number(t)) {
          err(e->loc, "E0307", std::string("'-' needs a number, found ") + type_name(t));
          return Type::invalid;
        }
        if (e->op == Tok::bang && t != Type::bool_) {
          err(e->loc, "E0308", std::string("'!' needs a bool, found ") + type_name(t),
              t == Type::int_ ? "compare it: x == 0" : "");
          return Type::invalid;
        }
        return t;
      }
      case ExprKind::binary: {
        Type a = expr(e->lhs), b = expr(e->rhs);
        if (a == Type::invalid || b == Type::invalid) return Type::invalid;
        switch (e->op) {
          case Tok::plus: case Tok::minus: case Tok::star: case Tok::slash: case Tok::percent:
            if (!is_number(a) || a != b || (e->op == Tok::percent && a != Type::int_)) {
              err(e->loc, "E0309",
                  std::string(tok_name(e->op)) + " needs matching numeric types (integer for %), found " + type_name(a) + " and " + type_name(b),
                  a == Type::string_ ? "build strings with print's % formatting" : "");
              return Type::invalid;
            }
            return a;
          case Tok::bitand_: case Tok::bitor_: case Tok::bitxor_: case Tok::shl: case Tok::shr:
            if (a != Type::int_ || b != Type::int_) {
              err(e->loc, "E0348", "bit operations need two integers");
              return Type::invalid;
            }
            return Type::int_;
          case Tok::lt: case Tok::le: case Tok::gt: case Tok::ge:
            if (!is_number(a) || a != b) {
              err(e->loc, "E0310", std::string(tok_name(e->op)) + " compares matching numeric types, found " + type_name(a) +
                                       " and " + type_name(b));
              return Type::invalid;
            }
            return Type::bool_;
          case Tok::eqeq: case Tok::noteq:
            if (a != b || a == Type::void_) {
              err(e->loc, "E0311", std::string("cannot compare ") + type_name(a) + " with " + type_name(b),
                  "there are no implicit conversions in Coax");
              return Type::invalid;
            }
            return Type::bool_;
          case Tok::andand: case Tok::oror:
            if (a != Type::bool_ || b != Type::bool_) {
              err(e->loc, "E0312", std::string(tok_name(e->op)) + " needs two bools, found " + type_name(a) +
                                       " and " + type_name(b));
              return Type::invalid;
            }
            return Type::bool_;
          default: break;
        }
        return Type::invalid;
      }
    }
    return Type::invalid;
  }

  Type intrinsic_call(Expr *e, Intrinsic intrinsic) {
    e->intrinsic = intrinsic;
    int count = intrinsic == Intrinsic::pow || intrinsic == Intrinsic::byte_at ? 2 : 1;
    if (e->args.count != count) {
      err(e->loc, "E0349", "wrong intrinsic argument count");
      return Type::invalid;
    }
    Type a = expr(e->args[0]);
    Type b = count == 2 ? expr(e->args[1]) : Type::invalid;
    if (intrinsic == Intrinsic::len || intrinsic == Intrinsic::byte_at) {
      if (a != Type::string_ || (count == 2 && b != Type::int_))
        err(e->loc, "E0349", "string intrinsic requires a string and an integer index where applicable");
      return Type::int_;
    }
    if (intrinsic == Intrinsic::f32 || intrinsic == Intrinsic::f64 || intrinsic == Intrinsic::integer) {
      if (!is_number(a)) err(e->loc, "E0349", "numeric casts require a number");
      return intrinsic == Intrinsic::f32 ? Type::f32_ : intrinsic == Intrinsic::f64 ? Type::f64_ : Type::int_;
    }
    if (!is_real(a) || (count == 2 && b != a)) err(e->loc, "E0349", "math intrinsic needs matching floating-point arguments");
    return a;
  }

  Type call(Expr *e) {
    Intrinsic builtin = find_intrinsic(e->text);
    if (builtin != Intrinsic::none) return intrinsic_call(e, builtin);
    if (str_eq(e->text, "type_name")) {
      if (e->args.count != 1) {
        err(e->loc, "E0343", "type_name needs exactly one expression");
        return Type::invalid;
      }
      Type t = expr(e->args[0]);
      if (t == Type::invalid) return t;
      if (t == Type::void_) {
        err(e->loc, "E0343", "type_name needs an expression with a value");
        return Type::invalid;
      }
      // The operand is typechecked but never executed, even at compile time.
      const char *name = type_name(t);
      e->kind = ExprKind::string_lit;
      e->text = Str{name, (int32_t)strlen(name)};
      return Type::string_;
    }
    if (find_constant(e->text)) {
      err(e->loc, "E0314", "'" + to_std(e->text) + "' is a constant, not a procedure");
      return Type::invalid;
    }
    if (str_eq(e->text, "print") && !find_proc(e->text)) {
      e->builtin_print = true;
      if (!has_basic) {
        err(e->loc, "E0313", "print is in module Basic, which is not imported",
            "add #import \"Basic\"; at the top of the file");
      }
      return print_call(e);
    }
    ProcDecl *p = find_proc(e->text);
    if (!p) {
      if (find_local(e->text))
        err(e->loc, "E0314", "'" + to_std(e->text) + "' is a variable, not a procedure");
      else
        err(e->loc, "E0315", "procedure '" + to_std(e->text) + "' is not declared");
      for (Expr *a : e->args) expr(a);
      return Type::invalid;
    }
    e->proc = p;
    if (e->args.count != p->params.count) {
      err(e->loc, "E0316",
          "'" + to_std(p->name) + "' takes " + std::to_string(p->params.count) + " argument" +
              (p->params.count == 1 ? "" : "s") + ", given " + std::to_string(e->args.count));
    }
    for (int32_t i = 0; i < e->args.count; i++) {
      Type t = expr(e->args[i]);
      if (i < p->params.count && t != Type::invalid && p->params[i].type != Type::invalid &&
          t != p->params[i].type) {
        err(e->args[i]->loc, "E0317",
            "argument " + std::to_string(i + 1) + " of '" + to_std(p->name) + "' is " +
                type_name(p->params[i].type) + ", given " + type_name(t));
      }
    }
    return p->ret;
  }

  // print(format, args...): the format must be a literal, and its '%'
  // count must match the arguments (spec §4.1). '%%' prints a '%'.
  Type print_call(Expr *e) {
    if (e->args.count == 0) {
      err(e->loc, "E0318", "print needs a format string", "print(\"text\\n\");");
      return Type::void_;
    }
    Expr *fmt = e->args[0];
    if (fmt->kind != ExprKind::string_lit) {
      expr(fmt);
      err(fmt->loc, "E0319", "print's format must be a string literal, so it can be checked",
          "print(\"%\\n\", value);");
      return Type::void_;
    }
    fmt->type = Type::string_;
    int32_t slots = 0;
    for (int32_t i = 0; i < fmt->text.count; i++) {
      if (fmt->text.data[i] != '%') continue;
      if (i + 1 < fmt->text.count && fmt->text.data[i + 1] == '%') { i++; continue; }
      slots++;
    }
    int32_t given = e->args.count - 1;
    if (slots != given) {
      err(fmt->loc, "E0320",
          "format has " + std::to_string(slots) + " '%' slot" + (slots == 1 ? "" : "s") + " but print was given " +
              std::to_string(given) + " value" + (given == 1 ? "" : "s"),
          "write '%%' for a literal percent sign");
    }
    for (int32_t i = 1; i < e->args.count; i++) {
      Type t = expr(e->args[i]);
      if (t == Type::void_) err(e->args[i]->loc, "E0321", "this call returns nothing, so it cannot be printed");
    }
    return Type::void_;
  }

  // ---- statements ----

  void stmt(Stmt *s) {
    switch (s->kind) {
      case StmtKind::block: {
        size_t mark = locals.size();
        for (Stmt *c : s->body) stmt(c);
        locals.resize(mark);
        break;
      }
      case StmtKind::expr: {
        Type t = expr(s->expr);
        if (s->expr->kind != ExprKind::call && t != Type::invalid)
          err(s->loc, "E0322", "this expression's value is not used", "assign it, or remove it");
        else if (s->expr->kind == ExprKind::call && t != Type::void_ && t != Type::invalid)
          err(s->loc, "E0323",
              "the result of '" + to_std(s->expr->text) + "' (" + type_name(t) + ") is not used",
              "use it: result := " + to_std(s->expr->text) + "(...);");
        break;
      }
      case StmtKind::var: {
        Type declared = Type::invalid;
        if (s->type_text.count) declared = resolve_type(s->type_text, s->type_loc);
        Type init = s->expr ? expr(s->expr) : Type::invalid;
        if (s->expr && init == Type::void_) {
          err(s->expr->loc, "E0324", "this call returns nothing, so it cannot initialize a variable");
          init = Type::invalid;
        }
        if (s->type_text.count && s->expr && declared != Type::invalid && init != Type::invalid && declared != init) {
          err(s->expr->loc, "E0325",
              "'" + to_std(s->name) + "' is " + type_name(declared) + " but is initialized with " + type_name(init));
        }
        s->type = s->type_text.count ? declared : init;
        if (metal && (s->type == Type::int_ || s->type == Type::f64_ || s->type == Type::string_))
          err(s->loc, "E0351", "unsupported Metal local type");
        declare(s->name, s->type, s->loc);
        break;
      }
      case StmtKind::assign: {
        Local *l = find_local(s->name);
        Type t = expr(s->expr);
        if (!l) {
          if (find_constant(s->name)) err(s->loc, "E0344", "constants cannot be assigned");
          else err(s->loc, "E0306", "'" + to_std(s->name) + "' is not declared",
                   "declare it: " + to_std(s->name) + " := ...;");
          break;
        }
        if (l->immutable) {
          err(s->loc, "E0344", "parameters cannot be assigned", "copy it into a local variable");
          break;
        }
        s->type = l->type;
        if (t == Type::invalid || l->type == Type::invalid) break;
        if (s->op != Tok::equal && (!is_number(l->type) || (s->op == Tok::percent_equal && l->type != Type::int_))) {
          err(s->loc, "E0326", std::string(tok_name(s->op)) + " needs an int variable, '" + to_std(s->name) +
                                   "' is " + type_name(l->type));
        } else if (t != l->type) {
          err(s->expr->loc, "E0327",
              "'" + to_std(s->name) + "' is " + type_name(l->type) + ", assigned " + type_name(t));
        }
        break;
      }
      case StmtKind::ret: {
        Type want = current->ret;
        if (!s->expr) {
          if (want != Type::void_ && want != Type::invalid)
            err(s->loc, "E0328", "'" + to_std(current->name) + "' must return " + type_name(want));
          break;
        }
        Type t = expr(s->expr);
        if (want == Type::void_)
          err(s->expr->loc, "E0329", "'" + to_std(current->name) + "' returns nothing, but a value is returned",
              "add '-> " + std::string(type_name(t)) + "' to its declaration");
        else if (t != Type::invalid && want != Type::invalid && t != want)
          err(s->expr->loc, "E0330",
              "'" + to_std(current->name) + "' returns " + type_name(want) + ", given " + type_name(t));
        break;
      }
      case StmtKind::if_:
      case StmtKind::while_: {
        if (metal && s->kind == StmtKind::while_) err(s->loc, "E0351", "loops are not supported in the initial Metal scalar profile");
        Type t = expr(s->expr);
        if (t != Type::invalid && t != Type::bool_)
          err(s->expr->loc, "E0331", std::string("condition must be bool, found ") + type_name(t),
              t == Type::int_ ? "compare it: x != 0" : "");
        stmt(s->then_);
        if (s->else_) stmt(s->else_);
        break;
      }
    }
  }

  static bool always_returns(const Stmt *s) {
    switch (s->kind) {
      case StmtKind::ret: return true;
      case StmtKind::block:
        for (const Stmt *c : s->body)
          if (always_returns(c)) return true;
        return false;
      case StmtKind::if_: return s->else_ && always_returns(s->then_) && always_returns(s->else_);
      default: return false;
    }
  }

  void gpu_expr(const Expr *e, std::unordered_map<const ProcDecl *, int> &seen, int depth) {
    if (!e) return;
    if (e->kind == ExprKind::call && e->proc) gpu_proc(e->proc, seen, depth + 1);
    if (e->kind == ExprKind::binary || e->kind == ExprKind::unary || e->kind == ExprKind::run) {
      gpu_expr(e->lhs, seen, depth); gpu_expr(e->rhs, seen, depth);
    }
    if (e->kind == ExprKind::call) for (Expr *arg : e->args) gpu_expr(arg, seen, depth);
  }
  void gpu_stmt(const Stmt *s, std::unordered_map<const ProcDecl *, int> &seen, int depth) {
    if (!s) return;
    gpu_expr(s->expr, seen, depth);
    for (Stmt *child : s->body) gpu_stmt(child, seen, depth);
    gpu_stmt(s->then_, seen, depth); gpu_stmt(s->else_, seen, depth);
  }
  void gpu_proc(const ProcDecl *p, std::unordered_map<const ProcDecl *, int> &seen, int depth) {
    if (seen[p] == 2) return;
    if (seen[p] == 1 || depth >= 64) { err(p->loc, "E0351", "recursive or excessively deep Metal call graph"); return; }
    seen[p] = 1;
    gpu_stmt(p->body, seen, depth);
    seen[p] = 2;
  }

  bool run() {
    int32_t before = g_error_count;
    for (const Import &im : file->imports) {
      if (str_eq(im.module, "Basic")) {
        if (has_basic) err(im.loc, "E0332", "module Basic is imported twice", "remove this import");
        has_basic = true;
      } else {
        err(im.loc, "E0333", "unknown module '" + to_std(im.module) + "'", "coax0 has one module: \"Basic\"");
      }
    }
    for (int32_t i = 0; i < file->procs.count; i++) {
      ProcDecl *p = file->procs[i];
      for (int32_t j = 0; j < i; j++)
        if (str_eq(file->procs[j]->name, p->name))
          err(p->loc, "E0334",
              "procedure '" + to_std(p->name) + "' is already declared at line " +
                  std::to_string(file->procs[j]->loc.line));
      if ((str_eq(p->name, "print") && has_basic) || str_eq(p->name, "type_name") || find_intrinsic(p->name) != Intrinsic::none)
        err(p->loc, "E0335", "'" + to_std(p->name) + "' is already a builtin", "pick another name");
      for (Param &prm : p->params) prm.type = resolve_type(prm.type_text, prm.type_loc);
      if (p->ret_text.count) p->ret = resolve_type(p->ret_text, p->ret_loc);
      if (p->foreign || p->exported) {
        if (p->ret == Type::string_) err(p->loc, "E0350", "C ABI supports fixed scalar types only");
        for (const Param &prm : p->params)
          if (prm.type == Type::string_) err(prm.loc, "E0350", "C ABI supports fixed scalar types only");
        const char *keywords[] = {"auto", "char", "const", "double", "enum", "extern", "float", "long", "register", "restrict", "short", "signed", "sizeof", "static", "struct", "typedef", "union", "unsigned", "void", "volatile", "namespace", "class", "template"};
        for (const char *word : keywords)
          if (str_eq(p->name, word)) err(p->loc, "E0350", "C ABI name is a reserved keyword");
        if (to_std(p->name).rfind("coax_", 0) == 0) err(p->loc, "E0350", "coax_ is reserved for compiler helpers");
      }
      if (metal && p->foreign) err(p->loc, "E0351", "Metal scalar libraries cannot call host foreign functions");
    }
    for (int32_t i = 0; i < file->constants.count; i++) {
      ConstDecl *c = file->constants[i];
      for (int32_t j = 0; j < i; j++)
        if (str_eq(c->name, file->constants[j]->name))
          err(c->loc, "E0345", "duplicate constant '" + to_std(c->name) + "'");
      if (find_proc(c->name) || str_eq(c->name, "type_name") || find_intrinsic(c->name) != Intrinsic::none || (has_basic && str_eq(c->name, "print")))
        err(c->loc, "E0345", "constant '" + to_std(c->name) + "' conflicts with a procedure or intrinsic");
    }
    for (ConstDecl *c : file->constants) constant_type(c);
    for (Expr *a : file->assertions) {
      Type t = expr(a);
      if (t != Type::invalid && t != Type::bool_) err(a->loc, "E0346", "#assert requires bool");
    }
    ProcDecl *entry = find_proc(Str{"main", 4});
    if (!entry && !library) {
      err(Loc{1, 1, 0}, "E0336", "the program has no 'main'", "add main :: () { ... }");
    } else if (entry && (entry->foreign || entry->exported || entry->params.count || entry->ret != Type::void_)) {
      err(entry->loc, "E0337", "'main' takes no parameters and returns nothing in coax0", "main :: () { ... }");
    }
    for (ProcDecl *p : file->procs) {
      if (p->foreign) continue;
      current = p;
      locals.clear();
      for (const Param &prm : p->params) declare(prm.name, prm.type, prm.loc, true);
      stmt(p->body);
      if (p->ret != Type::void_ && p->ret != Type::invalid && !always_returns(p->body))
        err(p->loc, "E0338", "'" + to_std(p->name) + "' can reach its end without returning " + type_name(p->ret),
            "add a return at the end");
    }
    if (metal) {
      std::unordered_map<const ProcDecl *, int> seen;
      for (ProcDecl *p : file->procs) {
        if (p->ret == Type::int_ || p->ret == Type::f64_ || p->ret == Type::string_) err(p->loc, "E0351", "Metal scalar library supports f32 and bool only");
        for (const Param &prm : p->params)
          if (prm.type == Type::int_ || prm.type == Type::f64_ || prm.type == Type::string_) err(prm.loc, "E0351", "unsupported Metal parameter type");
        gpu_proc(p, seen, 0);
      }
    }
    return g_error_count == before;
  }
};

}  // namespace

bool check(Arena *arena, File *file, bool library, bool metal) {
  Checker c{arena, file, file->src};
  c.library = library; c.metal = metal;
  return c.run();
}

}  // namespace coax
