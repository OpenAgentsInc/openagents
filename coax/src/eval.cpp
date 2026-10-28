// Compile-time execution of the checked scalar subset. This deliberately uses
// the typed AST first; a bytecode VM is a later, measured optimization. Values
// contain only integers, booleans and source-backed byte strings, never host
// addresses or resources that could leak into a target executable.
#include "coax.h"
#include <cmath>

namespace coax {
namespace {

struct CtValue {
  Type type = Type::void_;
  int64_t integer = 0;
  Str string;
  double real = 0;
};

struct CtLocal {
  Str name;
  CtValue value;
  bool initialized = true;
};

struct CtFrame {
  std::vector<CtLocal> locals;

  CtLocal *find(Str name) {
    for (size_t i = locals.size(); i-- > 0;)
      if (str_eq(locals[i].name, name)) return &locals[i];
    return nullptr;
  }
};

struct Evaluator {
  File *file;
  uint32_t steps = 0, dependency_depth = 0;
  bool failed = false;
  std::vector<Loc> calls;
  static constexpr uint32_t MAX_STEPS = 100000, MAX_DEPTH = 64;

  void err(Loc loc, const char *code, const std::string &message) {
    if (failed) return;
    failed = true;
    error_at(file->src, loc, code, message);
    // A bounded call trace refers to source, not generated C or interpreter code.
    for (size_t i = calls.size(); i-- > 0;)
      fprintf(stderr, "  note: compile-time call at %s:%d:%d\n",
              file->src->path.c_str(), calls[i].line, calls[i].col);
  }

  bool tick(Loc loc) {
    if (failed) return false;
    if (++steps > MAX_STEPS) {
      err(loc, "E0400", "compile-time evaluation exceeded 100000 steps");
      return false;
    }
    return true;
  }

  static void freeze(Expr *e, CtValue value) {
    e->type = value.type;
    e->int_value = value.integer;
    e->text = value.string;
    e->real_value = value.real;
    if (is_real(value.type)) { e->kind = ExprKind::real_lit; return; }
    e->kind = value.type == Type::int_ ? ExprKind::int_lit :
              value.type == Type::bool_ ? ExprKind::bool_lit : ExprKind::string_lit;
  }

  CtValue constant(ConstDecl *c) {
    if (c->evaluated == 1) {
      err(c->loc, "E0402", "compile-time constant cycle at '" + to_std(c->name) + "'");
      return {};
    }
    if (++dependency_depth > MAX_DEPTH) {
      err(c->loc, "E0403", "compile-time dependency depth exceeds 64");
      dependency_depth--;
      return {};
    }
    CtFrame empty;
    bool done = c->evaluated == 2;
    c->evaluated = 1;
    CtValue result = expr(c->expr, empty, false);
    if (!failed && !done) freeze(c->expr, result);
    c->evaluated = 2;
    dependency_depth--;
    return result;
  }

  CtValue integer_op(Tok op, int64_t a, int64_t b, Loc loc) {
    int64_t result = 0;
    bool overflow = false;
    switch (op) {
      case Tok::bitand_: result = a & b; break;
      case Tok::bitor_: result = a | b; break;
      case Tok::bitxor_: result = a ^ b; break;
      case Tok::shl: case Tok::shr:
        if (b < 0 || b > 63) { err(loc, "E0407", "shift count must be in 0..63"); return {}; }
        if (op == Tok::shl) result = (int64_t)((uint64_t)a << b);
        else { uint64_t bits = (uint64_t)a >> b; if (a < 0 && b) bits |= UINT64_MAX << (64 - b); result = (int64_t)bits; }
        break;
      case Tok::plus: overflow = __builtin_add_overflow(a, b, &result); break;
      case Tok::minus: overflow = __builtin_sub_overflow(a, b, &result); break;
      case Tok::star: overflow = __builtin_mul_overflow(a, b, &result); break;
      case Tok::slash:
      case Tok::percent:
        if (b == 0) { err(loc, "E0407", "division by zero during compile-time evaluation"); return {}; }
        if (a == INT64_MIN && b == -1) {
          overflow = op == Tok::slash;
          result = 0; // remainder is zero; do not invoke signed C++ overflow
        } else result = op == Tok::slash ? a / b : a % b;
        break;
      default: break;
    }
    if (overflow) err(loc, "E0407", "integer overflow during compile-time evaluation");
    return {Type::int_, result, {}};
  }

  CtValue floating(Type type, double value, Loc loc) {
    if (type == Type::f32_) value = (double)(float)value;
    if (!std::isfinite(value)) err(loc, "E0407", "non-finite compile-time floating-point result");
    return {type, 0, {}, value};
  }

  CtValue real_op(Tok op, CtValue a, CtValue b, Loc loc) {
    double v = op == Tok::plus ? a.real + b.real : op == Tok::minus ? a.real - b.real :
               op == Tok::star ? a.real * b.real : a.real / b.real;
    return floating(a.type, v, loc);
  }

  CtValue intrinsic(Expr *e, CtFrame &frame, bool allow_calls) {
    CtValue a = expr(e->args[0], frame, allow_calls);
    CtValue b;
    if (e->args.count == 2) b = expr(e->args[1], frame, allow_calls);
    if (failed) return {};
    Intrinsic kind = e->intrinsic;
    if (kind == Intrinsic::len) return {Type::int_, a.string.count, {}};
    if (kind == Intrinsic::byte_at) {
      if (b.integer < 0 || b.integer >= a.string.count) { err(e->loc, "E0407", "byte index out of bounds"); return {}; }
      return {Type::int_, (unsigned char)a.string.data[b.integer], {}};
    }
    double value = a.type == Type::int_ ? (double)a.integer : a.real;
    if (kind == Intrinsic::integer) {
      if (a.type == Type::int_) return a;
      if (!std::isfinite(value) || value < -9223372036854775808.0 || value >= 9223372036854775808.0) {
        err(e->loc, "E0407", "floating-point value does not fit in int"); return {};
      }
      return {Type::int_, (int64_t)value, {}};
    }
    if (kind == Intrinsic::f32 || kind == Intrinsic::f64) return floating(e->type, value, e->loc);
    switch (kind) {
      case Intrinsic::sin: value = std::sin(value); break;
      case Intrinsic::cos: value = std::cos(value); break;
      case Intrinsic::tan: value = std::tan(value); break;
      case Intrinsic::exp: value = std::exp(value); break;
      case Intrinsic::log: value = std::log(value); break;
      case Intrinsic::sqrt: value = std::sqrt(value); break;
      case Intrinsic::floor: value = std::floor(value); break;
      case Intrinsic::abs: value = std::fabs(value); break;
      case Intrinsic::pow: value = std::pow(value, b.real); break;
      default: break;
    }
    return floating(e->type, value, e->loc);
  }

  CtValue expr(Expr *e, CtFrame &frame, bool allow_calls) {
    if (!tick(e->loc)) return {};
    switch (e->kind) {
      case ExprKind::real_lit: return {e->type, 0, {}, e->real_value};
      case ExprKind::int_lit:
      case ExprKind::bool_lit:
      case ExprKind::string_lit: return {e->type, e->int_value, e->text};
      case ExprKind::ident: {
        if (e->constant) return constant(e->constant);
        if (CtLocal *local = frame.find(e->text)) {
          if (!local->initialized) err(e->loc, "E0405", "compile-time read of an uninitialized variable");
          return local->value;
        }
        err(e->loc, "E0401", "compile-time expression cannot capture runtime value '" + to_std(e->text) + "'");
        return {};
      }
      case ExprKind::run: {
        CtFrame isolated;
        CtValue value = expr(e->lhs, isolated, true);
        if (!failed) freeze(e, value);
        return value;
      }
      case ExprKind::call: {
        if (!allow_calls) {
          err(e->loc, "E0408", "a constant procedure call requires #run");
          return {};
        }
        if (e->intrinsic != Intrinsic::none) return intrinsic(e, frame, allow_calls);
        if (e->builtin_print || (e->proc && e->proc->foreign)) {
          err(e->loc, "E0404", "compile-time execution cannot perform output or host I/O");
          return {};
        }
        if (calls.size() >= MAX_DEPTH) {
          err(e->loc, "E0403", "compile-time call depth exceeds 64");
          return {};
        }
        CtFrame callee;
        for (int i = 0; i < e->args.count; i++) {
          CtValue value = expr(e->args[i], frame, allow_calls);
          if (failed) return {};
          callee.locals.push_back({e->proc->params[i].name, value, true});
        }
        calls.push_back(e->loc);
        CtValue result;
        stmt(e->proc->body, callee, &result);
        calls.pop_back();
        return result;
      }
      case ExprKind::unary: {
        CtValue a = expr(e->lhs, frame, allow_calls);
        if (failed) return {};
        if (e->op == Tok::bang) return {Type::bool_, !a.integer, {}};
        if (is_real(a.type)) return floating(a.type, -a.real, e->loc);
        if (a.integer == INT64_MIN) { err(e->loc, "E0407", "integer overflow during compile-time evaluation"); return {}; }
        return {Type::int_, -a.integer, {}};
      }
      case ExprKind::binary: {
        CtValue a = expr(e->lhs, frame, allow_calls);
        if (failed) return {};
        if (e->op == Tok::andand && !a.integer) return {Type::bool_, 0, {}};
        if (e->op == Tok::oror && a.integer) return {Type::bool_, 1, {}};
        CtValue b = expr(e->rhs, frame, allow_calls);
        if (failed) return {};
        bool result = false;
        switch (e->op) {
          case Tok::plus: case Tok::minus: case Tok::star: case Tok::slash: case Tok::percent:
            if (is_real(a.type)) return real_op(e->op, a, b, e->loc);
            return integer_op(e->op, a.integer, b.integer, e->loc);
          case Tok::bitand_: case Tok::bitor_: case Tok::bitxor_: case Tok::shl: case Tok::shr:
            return integer_op(e->op, a.integer, b.integer, e->loc);
          case Tok::eqeq: case Tok::noteq:
            result = a.type == Type::string_ ? str_eq(a.string, b.string) : is_real(a.type) ? a.real == b.real : a.integer == b.integer;
            if (e->op == Tok::noteq) result = !result;
            break;
          case Tok::lt: result = is_real(a.type) ? a.real < b.real : a.integer < b.integer; break;
          case Tok::le: result = is_real(a.type) ? a.real <= b.real : a.integer <= b.integer; break;
          case Tok::gt: result = is_real(a.type) ? a.real > b.real : a.integer > b.integer; break;
          case Tok::ge: result = is_real(a.type) ? a.real >= b.real : a.integer >= b.integer; break;
          case Tok::andand: case Tok::oror: result = b.integer != 0; break;
          default: break;
        }
        return {Type::bool_, result, {}};
      }
    }
    return {};
  }

  // True means a return propagated out of this statement.
  bool stmt(Stmt *s, CtFrame &frame, CtValue *returned) {
    if (!tick(s->loc)) return false;
    switch (s->kind) {
      case StmtKind::block: {
        size_t mark = frame.locals.size();
        bool did_return = false;
        for (Stmt *child : s->body) {
          if (failed || (did_return = stmt(child, frame, returned))) break;
        }
        frame.locals.resize(mark);
        return did_return;
      }
      case StmtKind::var: {
        CtValue value{s->type, 0, {"", 0}};
        if (s->expr) value = expr(s->expr, frame, true);
        frame.locals.push_back({s->name, value, !s->uninit});
        return false;
      }
      case StmtKind::assign: {
        CtValue value = expr(s->expr, frame, true);
        if (failed) return false;
        CtLocal *local = frame.find(s->name); // the checker has resolved this local
        if (s->op != Tok::equal) {
          if (!local->initialized) { err(s->loc, "E0405", "compile-time read of an uninitialized variable"); return false; }
          Tok op = s->op == Tok::plus_equal ? Tok::plus : s->op == Tok::minus_equal ? Tok::minus :
                   s->op == Tok::star_equal ? Tok::star : s->op == Tok::slash_equal ? Tok::slash : Tok::percent;
          value = is_real(local->value.type) ? real_op(op, local->value, value, s->loc) :
                  integer_op(op, local->value.integer, value.integer, s->loc);
        }
        local->value = value;
        local->initialized = true;
        return false;
      }
      case StmtKind::expr: expr(s->expr, frame, true); return false;
      case StmtKind::ret:
        if (s->expr) *returned = expr(s->expr, frame, true);
        return true;
      case StmtKind::if_: {
        CtValue condition = expr(s->expr, frame, true);
        if (failed) return false;
        if (condition.integer) return stmt(s->then_, frame, returned);
        return s->else_ && stmt(s->else_, frame, returned);
      }
      case StmtKind::while_:
        while (!failed && expr(s->expr, frame, true).integer) {
          if (failed) break;
          if (stmt(s->then_, frame, returned)) return true;
        }
        return false;
    }
    return false;
  }

  void fold(Expr *e) {
    if (!e || failed) return;
    if (e->kind == ExprKind::run) {
      CtFrame empty;
      expr(e, empty, true);
    } else if (e->kind == ExprKind::binary || e->kind == ExprKind::unary) {
      fold(e->lhs);
      fold(e->rhs);
    } else if (e->kind == ExprKind::call) {
      for (Expr *a : e->args) fold(a);
    }
  }

  void fold_stmt(Stmt *s) {
    if (!s || failed) return;
    fold(s->expr);
    for (Stmt *child : s->body) fold_stmt(child);
    fold_stmt(s->then_);
    fold_stmt(s->else_);
  }

  bool run() {
    for (ConstDecl *c : file->constants) {
      if (failed) break;
      constant(c);
    }
    for (Expr *a : file->assertions) {
      if (failed) break;
      CtFrame empty;
      CtValue condition = expr(a, empty, false);
      if (!failed && !condition.integer) err(a->loc, "E0406", "compile-time assertion failed");
    }
    for (ProcDecl *p : file->procs) fold_stmt(p->body);
    return !failed;
  }
};

}  // namespace

bool evaluate(File *file) {
  Evaluator evaluator{file};
  return evaluator.run();
}

}  // namespace coax
