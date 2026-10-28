// Compile-time execution of the checked subset. This deliberately walks the
// typed AST; a bytecode VM is a later, measured optimization. Values hold
// scalars, byte strings and aggregates, never host addresses or resources
// that could leak into a target executable. Every rule here matches the C
// the emitter writes: the same overflow, bounds, cast and order rules, except
// that compile-time arithmetic is checked in every build mode.
#include "coax.h"

#include <cmath>

namespace coax {

bool has_call(const Expr *e) {
  if (!e) return false;
  if (e->kind == ExprKind::call) return true;
  if (has_call(e->lhs) || has_call(e->rhs) || has_call(e->third)) return true;
  for (const Expr *a : e->args) if (has_call(a)) return true;
  for (const FieldInit &f : e->fields) if (has_call(f.value)) return true;
  return false;
}

namespace {

struct CtLocal {
  Str name;
  Value value;
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

// Keeps storage alive while a located element is in use.
struct Holder {
  std::vector<std::shared_ptr<std::vector<Value>>> keep;
  std::vector<std::unique_ptr<Value>> temps;
  Value *temp(Value v) {
    temps.push_back(std::make_unique<Value>(std::move(v)));
    return temps.back().get();
  }
};

enum class Flow : uint8_t { normal, ret, brk, cont };

__int128 as_int128(Type t, uint64_t bits) {
  return t->is_signed ? (__int128)(int64_t)bits : (__int128)bits;
}

uint64_t canon_bits(Type t, uint64_t bits) {
  if (t->bits >= 64) return bits;
  uint64_t mask = (UINT64_C(1) << t->bits) - 1;
  bits &= mask;
  if (t->is_signed && (bits >> (t->bits - 1)) & 1) bits |= ~mask;
  return bits;
}

bool fits_type(Type t, __int128 v) {
  if (t->is_signed) return v >= -((__int128)1 << (t->bits - 1)) && v <= ((__int128)1 << (t->bits - 1)) - 1;
  return v >= 0 && v <= (((__int128)1 << t->bits) - 1);
}

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

  bool tick(Loc loc, uint32_t cost = 1) {
    if (failed) return false;
    steps += cost;
    if (steps > MAX_STEPS) {
      err(loc, "E0400", "compile-time evaluation exceeded 100000 steps");
      return false;
    }
    return true;
  }

  // ---- values ----

  static Value scalar(Type t, uint64_t bits) {
    Value v;
    v.type = t;
    v.bits = bits;
    return v;
  }
  static Value floating(Type t, double f) {
    Value v;
    v.type = t;
    v.f = t == &T_F32 ? (double)(float)f : f;
    return v;
  }

  // The value of a declaration without an initializer: zero, or the
  // struct's field defaults. Arrays cost one step per element.
  Value zero(Type t, Loc loc) {
    Value v;
    v.type = t;
    if (t->kind == TypeKind::struct_) {
      StructDecl *d = t->struct_decl;
      field_defaults(d);
      v.agg = std::make_shared<std::vector<Value>>();
      for (const Field &f : d->fields) {
        if (failed) return v;
        v.agg->push_back(f.init ? literal(f.init) : zero(f.type, loc));
      }
    } else if (t->kind == TypeKind::array) {
      if (!tick(loc, (uint32_t)t->length)) return v;
      v.agg = std::make_shared<std::vector<Value>>();
      v.agg->reserve((size_t)t->length);
      Value element = zero(t->elem, loc);
      for (int64_t i = 0; i < t->length && !failed; i++) v.agg->push_back(clone(element));
    }
    return v;
  }

  // Aggregates copy deeply; views and strings keep sharing what they borrow.
  static Value clone(const Value &v) {
    Value c = v;
    if (is_aggregate(v.type) && v.agg) {
      c.agg = std::make_shared<std::vector<Value>>();
      c.agg->reserve(v.agg->size());
      for (const Value &e : *v.agg) c.agg->push_back(clone(e));
    }
    return c;
  }

  // Assignment writes into existing storage, so views of it see the change.
  static void assign_into(Value &dst, const Value &src) {
    if (is_aggregate(dst.type) && dst.agg && src.agg) {
      for (size_t i = 0; i < dst.agg->size(); i++) assign_into((*dst.agg)[i], (*src.agg)[i]);
    } else {
      dst = src;
    }
  }

  // A frozen literal or compile-time constant node as a value.
  Value literal(const Expr *e) {
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::bool_lit: case ExprKind::enum_value: return scalar(e->type, e->int_value);
      case ExprKind::float_lit: return floating(e->type, e->float_value);
      case ExprKind::string_lit: { Value v; v.type = &T_STRING; v.s = e->text; return v; }
      case ExprKind::ct_value: return clone(*e->ct);
      default: return {};
    }
  }

  bool contains_view(const Value &v) {
    if (v.type->kind == TypeKind::view) return true;
    if (v.agg && is_aggregate(v.type))
      for (const Value &e : *v.agg) if (contains_view(e)) return true;
    return false;
  }

  void freeze(Expr *e, const Value &value, Loc loc) {
    if (failed) return;
    if (contains_view(value)) {
      err(loc, "E0409", "a compile-time result cannot contain a view; views borrow memory that exists only while compiling");
      return;
    }
    e->type = value.type;
    e->lhs = e->rhs = e->third = nullptr;
    switch (value.type->kind) {
      case TypeKind::integer: e->kind = ExprKind::int_lit; e->int_value = value.bits; break;
      case TypeKind::bool_: e->kind = ExprKind::bool_lit; e->int_value = value.bits; break;
      case TypeKind::enum_: e->kind = ExprKind::enum_value; e->int_value = value.bits; break;
      case TypeKind::floating: e->kind = ExprKind::float_lit; e->float_value = value.f; break;
      case TypeKind::string_: e->kind = ExprKind::string_lit; e->text = value.s; break;
      default:
        e->kind = ExprKind::ct_value;
        e->ct = new Value(clone(value));
        break;
    }
  }

  void field_defaults(StructDecl *d) {
    if (d->defaults == 2) return;
    if (d->defaults == 1) {
      err(d->loc, "E0402", "compile-time cycle in the defaults of struct '" + to_std(d->name) + "'");
      return;
    }
    d->defaults = 1;
    for (Field &f : d->fields) {
      if (!f.init || failed) continue;
      CtFrame empty;
      Value v = expr(f.init, empty, false);
      freeze(f.init, v, f.init->loc);
    }
    d->defaults = 2;
  }

  Value constant(ConstDecl *c) {
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
    Value result = expr(c->expr, empty, false);
    if (!failed && !done) freeze(c->expr, result, c->loc);
    c->evaluated = 2;
    dependency_depth--;
    return result;
  }

  // ---- arithmetic: the same rules as the C runtime, checked in every mode ----

  Value int_op(Tok op, Type t, uint64_t a_bits, uint64_t b_bits, Type bt, Loc loc) {
    __int128 a = as_int128(t, a_bits), b = as_int128(bt ? bt : t, b_bits), r = 0;
    switch (op) {
      case Tok::plus: r = a + b; break;
      case Tok::minus: r = a - b; break;
      case Tok::star: r = a * b; break;
      case Tok::slash:
      case Tok::percent:
        if (b == 0) { err(loc, "E0407", "division by zero during compile-time evaluation"); return {}; }
        r = op == Tok::slash ? a / b : a % b;
        break;
      case Tok::amp: return scalar(t, canon_bits(t, a_bits & b_bits));
      case Tok::pipe: return scalar(t, canon_bits(t, a_bits | b_bits));
      case Tok::caret: return scalar(t, canon_bits(t, a_bits ^ b_bits));
      case Tok::shl: case Tok::shr: {
        if (b < 0 || b >= t->bits) {
          err(loc, "E0411", "shift by " + std::to_string((long long)b) + " is out of range for " + type_name(t));
          return {};
        }
        if (op == Tok::shl) return scalar(t, canon_bits(t, a_bits << (int)b));
        if (t->is_signed) return scalar(t, canon_bits(t, (uint64_t)((int64_t)a_bits >> (int)b)));
        uint64_t mask = t->bits >= 64 ? UINT64_MAX : (UINT64_C(1) << t->bits) - 1;
        return scalar(t, (a_bits & mask) >> (int)b);
      }
      default: break;
    }
    if (!fits_type(t, r)) {
      err(loc, "E0407", std::string("integer overflow in ") + type_name(t) + " during compile-time evaluation");
      return {};
    }
    return scalar(t, canon_bits(t, (uint64_t)r));
  }

  Value float_op(Tok op, Type t, double a, double b) {
    switch (op) {
      case Tok::plus: return floating(t, a + b);
      case Tok::minus: return floating(t, a - b);
      case Tok::star: return floating(t, a * b);
      case Tok::slash: return floating(t, a / b);
      default: return {};
    }
  }

  // Built-in math. f32 uses the float functions, as the generated C does.
  Value math(const Expr *e, CtFrame &frame, bool allow_calls) {
    double a = expr(e->args[0], frame, allow_calls).f, b = 0;
    if (!failed && e->args.count == 2) b = expr(e->args[1], frame, allow_calls).f;
    if (failed) return {};
    bool f32 = e->type == &T_F32;
    float x = (float)a, y = (float)b;
    double r = 0;
    switch (e->intrinsic) {
      case Intrinsic::sin: r = f32 ? ::sinf(x) : std::sin(a); break;
      case Intrinsic::cos: r = f32 ? ::cosf(x) : std::cos(a); break;
      case Intrinsic::tan: r = f32 ? ::tanf(x) : std::tan(a); break;
      case Intrinsic::exp: r = f32 ? ::expf(x) : std::exp(a); break;
      case Intrinsic::log: r = f32 ? ::logf(x) : std::log(a); break;
      case Intrinsic::sqrt: r = f32 ? ::sqrtf(x) : std::sqrt(a); break;
      case Intrinsic::floor: r = f32 ? ::floorf(x) : std::floor(a); break;
      case Intrinsic::abs: r = f32 ? ::fabsf(x) : std::fabs(a); break;
      case Intrinsic::pow: r = f32 ? ::powf(x, y) : std::pow(a, b); break;
      case Intrinsic::none: break;
    }
    if (!std::isfinite(r)) {
      err(e->loc, "E0412", to_std(e->text) + " has no finite result for these arguments while compiling");
      return {};
    }
    return floating(e->type, r);
  }

  Value do_cast(const Expr *e, Value v) {
    Type to = e->type, from = v.type;
    if (from == to) return v;
    if (to == &T_STRING) {
      // Compile-time bytes have no address; copy them into memory that lives
      // as long as the compiler, so a frozen string can still be emitted.
      char *bytes = new char[(size_t)v.count + 1];
      for (int64_t i = 0; i < v.count; i++) bytes[i] = (char)(*v.agg)[(size_t)(v.offset + i)].bits;
      Value s;
      s.type = &T_STRING;
      s.s = Str{bytes, (int32_t)v.count};
      return s;
    }
    if (is_float(to)) {
      if (is_float(from)) {
        if (to == &T_F32 && std::isfinite(v.f) && std::isinf((float)v.f)) {
          err(e->loc, "E0411", "cast of " + std::to_string(v.f) + " to f32 is out of range");
          return {};
        }
        return floating(to, v.f);
      }
      // One rounding, straight from the integer, as C converts.
      if (to == &T_F32) return floating(to, from->is_signed ? (double)(float)(int64_t)v.bits : (double)(float)v.bits);
      return floating(to, from->is_signed ? (double)(int64_t)v.bits : (double)v.bits);
    }
    if (is_float(from)) {
      double f = std::trunc(v.f);
      if (std::isnan(f) || f <= -1.8e19 || f >= 1.9e19 || !fits_type(to, (__int128)f)) {
        err(e->loc, "E0411", std::string("cast of ") + std::to_string(v.f) + " to " + type_name(to) + " is out of range");
        return {};
      }
      return scalar(to, canon_bits(to, (uint64_t)(__int128)f));
    }
    // Integers and enums.
    __int128 n = as_int128(from, v.bits);
    if (to->kind == TypeKind::enum_) {
      for (const EnumMember &m : to->enum_decl->members)
        if (as_int128(to, m.bits) == n) return scalar(to, m.bits);
      err(e->loc, "E0411", std::string("no member of ") + type_name(to) + " has the value " + std::to_string((long long)n));
      return {};
    }
    if (e->trunc) return scalar(to, canon_bits(to, v.bits));
    if (!fits_type(to, n)) {
      err(e->loc, "E0411", std::string("cast to ") + type_name(to) + " is out of range");
      return {};
    }
    return scalar(to, canon_bits(to, (uint64_t)n));
  }

  int64_t index_value(const Value &v) {
    return v.type->is_signed ? (int64_t)v.bits : (v.bits > (uint64_t)INT64_MAX ? -1 : (int64_t)v.bits);
  }

  // ---- places ----

  // The storage an expression names, or a temporary holding its value.
  Value *locate(Expr *e, CtFrame &frame, Holder &h, bool allow_calls, CtLocal **root = nullptr) {
    if (!tick(e->loc)) return nullptr;
    switch (e->kind) {
      case ExprKind::ident: {
        if (e->constant) {
          Value v = constant(e->constant);
          if (failed) return nullptr;
          if (e->constant->expr->kind == ExprKind::ct_value) return e->constant->expr->ct;
          return h.temp(v);
        }
        CtLocal *local = frame.find(e->text);
        if (!local) {
          err(e->loc, "E0401", "compile-time expression cannot capture runtime value '" + to_std(e->text) + "'");
          return nullptr;
        }
        if (root) *root = local;
        return &local->value;
      }
      case ExprKind::ct_value: return e->ct;
      case ExprKind::field: {
        if (e->field_index < 0) break;
        Value *base = locate(e->lhs, frame, h, allow_calls, root);
        if (!base) return nullptr;
        return &(*base->agg)[(size_t)e->field_index];
      }
      case ExprKind::index: {
        Type lt = e->lhs->type;
        if (lt == &T_STRING) break;
        Value *base = nullptr;
        Value view;
        if (lt->kind == TypeKind::view) {
          view = expr(e->lhs, frame, allow_calls);
          if (root) *root = nullptr;  // a view's elements are always initialized storage
        } else {
          base = locate(e->lhs, frame, h, allow_calls, root);
        }
        if (failed) return nullptr;
        Value iv = expr(e->rhs, frame, allow_calls);
        if (failed) return nullptr;
        int64_t i = index_value(iv);
        int64_t count = base ? lt->length : view.count;
        if (i < 0 || i >= count) {
          err(e->loc, "E0410", "index " + std::to_string((long long)(int64_t)iv.bits) + " is out of bounds for " +
                                   std::to_string((long long)count) + " elements");
          return nullptr;
        }
        if (base) return &(*base->agg)[(size_t)i];
        h.keep.push_back(view.agg);
        return &(*view.agg)[(size_t)(view.offset + i)];
      }
      default: break;
    }
    steps--;  // expr() charges its own step
    return h.temp(expr(e, frame, allow_calls));
  }

  // ---- expressions ----

  Value expr(Expr *e, CtFrame &frame, bool allow_calls) {
    if (!tick(e->loc)) return {};
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::bool_lit: case ExprKind::enum_value:
      case ExprKind::float_lit: case ExprKind::string_lit: case ExprKind::ct_value:
        return literal(e);
      case ExprKind::ident:
      case ExprKind::field:
      case ExprKind::index: {
        if (e->kind == ExprKind::field && e->field_index == -2) {
          // .count: an array's is its length; a view's or string's is stored.
          Type lt = e->lhs->type;
          if (lt->kind == TypeKind::array) {
            if (has_call(e->lhs)) expr(e->lhs, frame, allow_calls);
            return scalar(T_INT, (uint64_t)lt->length);
          }
          Value v = expr(e->lhs, frame, allow_calls);
          return scalar(T_INT, (uint64_t)(lt == &T_STRING ? v.s.count : v.count));
        }
        if (e->kind == ExprKind::index && e->lhs->type == &T_STRING) {
          Value s = expr(e->lhs, frame, allow_calls);
          if (failed) return {};
          Value iv = expr(e->rhs, frame, allow_calls);
          if (failed) return {};
          int64_t i = index_value(iv);
          if (i < 0 || i >= s.s.count) {
            err(e->loc, "E0410", "index " + std::to_string((long long)(int64_t)iv.bits) + " is out of bounds for a " +
                                     std::to_string(s.s.count) + "-byte string");
            return {};
          }
          return scalar(&T_U8, (uint8_t)s.s.data[i]);
        }
        Holder h;
        CtLocal *root = nullptr;
        steps--;
        Value *v = locate(e, frame, h, allow_calls, &root);
        if (!v || failed) return {};
        if (root && !root->initialized) {
          err(e->loc, "E0405", "compile-time read of an uninitialized variable");
          return {};
        }
        return clone(*v);
      }
      case ExprKind::run: {
        CtFrame isolated;
        Value value = expr(e->lhs, isolated, true);
        freeze(e, value, e->loc);
        return value;
      }
      case ExprKind::call: {
        if (!allow_calls) {
          err(e->loc, "E0408", "a constant procedure call requires #run");
          return {};
        }
        if (e->intrinsic != Intrinsic::none) return math(e, frame, allow_calls);
        if (e->builtin_print) {
          err(e->loc, "E0404", "compile-time execution cannot perform output or host I/O");
          return {};
        }
        if (e->proc->foreign) {
          err(e->loc, "E0404", "compile-time execution cannot call the host procedure '" + to_std(e->proc->name) + "'");
          return {};
        }
        if (calls.size() >= MAX_DEPTH) {
          err(e->loc, "E0403", "compile-time call depth exceeds 64");
          return {};
        }
        CtFrame callee;
        for (int i = 0; i < e->args.count; i++) {
          Value value = expr(e->args[i], frame, allow_calls);
          if (failed) return {};
          callee.locals.push_back({e->proc->params[i].name, std::move(value), true});
        }
        calls.push_back(e->loc);
        Value result;
        stmt(e->proc->body, callee, &result);
        calls.pop_back();
        return result;
      }
      case ExprKind::unary: {
        Value a = expr(e->lhs, frame, allow_calls);
        if (failed) return {};
        if (e->op == Tok::bang) return scalar(&T_BOOL, !a.bits);
        if (is_float(a.type)) return floating(a.type, -a.f);
        return int_op(Tok::minus, a.type, 0, a.bits, nullptr, e->loc);
      }
      case ExprKind::binary: {
        Value a = expr(e->lhs, frame, allow_calls);
        if (failed) return {};
        if (e->op == Tok::andand && !a.bits) return scalar(&T_BOOL, 0);
        if (e->op == Tok::oror && a.bits) return scalar(&T_BOOL, 1);
        Value b = expr(e->rhs, frame, allow_calls);
        if (failed) return {};
        Type t = a.type;
        switch (e->op) {
          case Tok::plus: case Tok::minus: case Tok::star: case Tok::slash: case Tok::percent:
          case Tok::amp: case Tok::pipe: case Tok::caret:
            if (is_float(t)) return float_op(e->op, t, a.f, b.f);
            return int_op(e->op, t, a.bits, b.bits, nullptr, e->loc);
          case Tok::shl: case Tok::shr: return int_op(e->op, t, a.bits, b.bits, b.type, e->loc);
          case Tok::andand: case Tok::oror: return scalar(&T_BOOL, b.bits != 0);
          default: break;
        }
        bool result = false;
        if (t == &T_STRING) {
          result = str_eq(a.s, b.s);
          if (e->op == Tok::noteq) result = !result;
        } else if (is_float(t)) {
          switch (e->op) {
            case Tok::eqeq: result = a.f == b.f; break;
            case Tok::noteq: result = a.f != b.f; break;
            case Tok::lt: result = a.f < b.f; break;
            case Tok::le: result = a.f <= b.f; break;
            case Tok::gt: result = a.f > b.f; break;
            case Tok::ge: result = a.f >= b.f; break;
            default: break;
          }
        } else {
          __int128 x = as_int128(t, a.bits), y = as_int128(t, b.bits);
          switch (e->op) {
            case Tok::eqeq: result = x == y; break;
            case Tok::noteq: result = x != y; break;
            case Tok::lt: result = x < y; break;
            case Tok::le: result = x <= y; break;
            case Tok::gt: result = x > y; break;
            case Tok::ge: result = x >= y; break;
            default: break;
          }
        }
        return scalar(&T_BOOL, result);
      }
      case ExprKind::cast: {
        Value v = expr(e->lhs, frame, allow_calls);
        if (failed) return {};
        return do_cast(e, std::move(v));
      }
      case ExprKind::ifx: {
        Value c = expr(e->lhs, frame, allow_calls);
        if (failed) return {};
        return expr(c.bits ? e->rhs : e->third, frame, allow_calls);
      }
      case ExprKind::struct_lit: {
        Value v = zero(e->type, e->loc);
        for (const FieldInit &fi : e->fields) {
          if (failed) return {};
          (*v.agg)[(size_t)fi.field] = expr(fi.value, frame, allow_calls);
        }
        return v;
      }
      case ExprKind::array_lit: {
        Value v;
        v.type = e->type;
        v.agg = std::make_shared<std::vector<Value>>();
        for (Expr *a : e->args) {
          if (failed) return {};
          v.agg->push_back(expr(a, frame, allow_calls));
        }
        return v;
      }
      case ExprKind::slice: {
        Type lt = e->lhs->type;
        Holder h;
        Value base;
        int64_t count = 0;
        if (lt->kind == TypeKind::array) {
          Value *storage = locate(e->lhs, frame, h, allow_calls);
          if (!storage || failed) return {};
          base.type = e->type;
          base.agg = storage->agg;
          base.offset = 0;
          count = lt->length;
        } else {
          base = expr(e->lhs, frame, allow_calls);
          if (failed) return {};
          count = lt == &T_STRING ? base.s.count : base.count;
        }
        int64_t lo = 0, hi = count;
        if (e->rhs) {
          Value l = expr(e->rhs, frame, allow_calls);
          if (failed) return {};
          Value r = expr(e->third, frame, allow_calls);
          if (failed) return {};
          lo = index_value(l);
          hi = index_value(r);
        }
        if (lo < 0 || hi < lo || hi > count) {
          err(e->loc, "E0410", "slice " + std::to_string((long long)lo) + "..<" + std::to_string((long long)hi) +
                                   " is out of bounds for " + std::to_string((long long)count) + " elements");
          return {};
        }
        if (lt == &T_STRING) {
          base.s = Str{base.s.data + lo, (int32_t)(hi - lo)};
          return base;
        }
        base.type = e->type;
        base.offset += lo;
        base.count = hi - lo;
        return base;
      }
    }
    return {};
  }

  // ---- statements ----

  Flow stmt(Stmt *s, CtFrame &frame, Value *returned) {
    if (!tick(s->loc)) return Flow::normal;
    switch (s->kind) {
      case StmtKind::block: {
        size_t mark = frame.locals.size();
        std::vector<Stmt *> deferred;
        Flow flow = Flow::normal;
        for (Stmt *child : s->body) {
          if (failed) break;
          if (child->kind == StmtKind::defer) {
            deferred.push_back(child->then_);
            continue;
          }
          if ((flow = stmt(child, frame, returned)) != Flow::normal) break;
        }
        // Cleanup runs in reverse registration order on every exit.
        for (size_t i = deferred.size(); i-- > 0 && !failed;) stmt(deferred[i], frame, returned);
        frame.locals.resize(mark);
        return flow;
      }
      case StmtKind::var: {
        Value value;
        if (s->expr) value = expr(s->expr, frame, true);
        else value = zero(s->type, s->loc);
        frame.locals.push_back({s->name, std::move(value), !s->uninit});
        return Flow::normal;
      }
      case StmtKind::assign: {
        Holder h;
        CtLocal *root = nullptr;
        Value *place = locate(s->target, frame, h, true, &root);
        if (!place || failed) return Flow::normal;
        Value value = expr(s->expr, frame, true);
        if (failed) return Flow::normal;
        if (s->op != Tok::equal) {
          if (root && !root->initialized) {
            err(s->loc, "E0405", "compile-time read of an uninitialized variable");
            return Flow::normal;
          }
          Tok op = s->op == Tok::plus_equal ? Tok::plus : s->op == Tok::minus_equal ? Tok::minus
                 : s->op == Tok::star_equal ? Tok::star : s->op == Tok::slash_equal ? Tok::slash
                 : s->op == Tok::percent_equal ? Tok::percent : s->op == Tok::amp_equal ? Tok::amp
                 : s->op == Tok::pipe_equal ? Tok::pipe : s->op == Tok::caret_equal ? Tok::caret
                 : s->op == Tok::shl_equal ? Tok::shl : Tok::shr;
          Type t = place->type;
          value = is_float(t) ? float_op(op, t, place->f, value.f) : int_op(op, t, place->bits, value.bits, value.type, s->loc);
          if (failed) return Flow::normal;
        }
        assign_into(*place, value);
        if (root) root->initialized = true;
        return Flow::normal;
      }
      case StmtKind::expr: expr(s->expr, frame, true); return Flow::normal;
      case StmtKind::ret:
        if (s->expr) *returned = expr(s->expr, frame, true);
        return Flow::ret;
      case StmtKind::break_: return Flow::brk;
      case StmtKind::continue_: return Flow::cont;
      case StmtKind::defer: return Flow::normal;  // registered by the enclosing block
      case StmtKind::if_: {
        Value condition = expr(s->expr, frame, true);
        if (failed) return Flow::normal;
        if (condition.bits) return stmt(s->then_, frame, returned);
        return s->else_ ? stmt(s->else_, frame, returned) : Flow::normal;
      }
      case StmtKind::while_:
        while (!failed) {
          Value c = expr(s->expr, frame, true);
          if (failed || !c.bits) break;
          Flow f = stmt(s->then_, frame, returned);
          if (f == Flow::ret) return f;
          if (f == Flow::brk) break;
        }
        return Flow::normal;
      case StmtKind::for_range: {
        Value lo = expr(s->expr, frame, true);
        if (failed) return Flow::normal;
        Value hi = expr(s->expr2, frame, true);
        if (failed) return Flow::normal;
        Type t = s->type;
        size_t mark = frame.locals.size();
        frame.locals.push_back({s->name, lo, true});
        for (__int128 i = as_int128(t, lo.bits); i < as_int128(t, hi.bits) && !failed; i++) {
          frame.locals[mark].value = scalar(t, canon_bits(t, (uint64_t)i));
          Flow f = stmt(s->then_, frame, returned);
          if (f == Flow::ret) { frame.locals.resize(mark); return f; }
          if (f == Flow::brk) break;
        }
        frame.locals.resize(mark);
        return Flow::normal;
      }
      case StmtKind::for_each: {
        Type ct = s->expr->type;
        Holder h;
        std::shared_ptr<std::vector<Value>> storage;
        int64_t offset = 0, count = 0;
        Value collection;
        if (ct->kind == TypeKind::array) {
          // Arrays are iterated in place: writes by the body are seen by later iterations.
          Value *a = locate(s->expr, frame, h, true);
          if (!a || failed) return Flow::normal;
          storage = a->agg;
          count = ct->length;
        } else {
          collection = expr(s->expr, frame, true);
          if (failed) return Flow::normal;
          if (ct == &T_STRING) count = collection.s.count;
          else { storage = collection.agg; offset = collection.offset; count = collection.count; }
        }
        size_t mark = frame.locals.size();
        frame.locals.push_back({s->name, {}, true});
        if (s->name2.count) frame.locals.push_back({s->name2, {}, true});
        for (int64_t i = 0; i < count && !failed; i++) {
          frame.locals[mark].value = ct == &T_STRING ? scalar(&T_U8, (uint8_t)collection.s.data[i])
                                                     : clone((*storage)[(size_t)(offset + i)]);
          if (s->name2.count) frame.locals[mark + 1].value = scalar(T_INT, (uint64_t)i);
          Flow f = stmt(s->then_, frame, returned);
          if (f == Flow::ret) { frame.locals.resize(mark); return f; }
          if (f == Flow::brk) break;
        }
        frame.locals.resize(mark);
        return Flow::normal;
      }
    }
    return Flow::normal;
  }

  // ---- folding #run sites in procedure bodies ----

  void fold(Expr *e) {
    if (!e || failed) return;
    if (e->kind == ExprKind::run) {
      CtFrame empty;
      expr(e, empty, true);
      return;
    }
    fold(e->lhs);
    fold(e->rhs);
    fold(e->third);
    for (Expr *a : e->args) fold(a);
    for (FieldInit &f : e->fields) fold(f.value);
  }

  void fold_stmt(Stmt *s) {
    if (!s || failed) return;
    fold(s->expr);
    fold(s->expr2);
    fold(s->target);
    for (Stmt *child : s->body) fold_stmt(child);
    fold_stmt(s->then_);
    fold_stmt(s->else_);
  }

  bool run() {
    for (StructDecl *d : file->structs) {
      if (failed) break;
      field_defaults(d);
    }
    for (ConstDecl *c : file->constants) {
      if (failed) break;
      if (!is_untyped(c->type)) constant(c);  // untyped constants were inlined as literals
    }
    for (Expr *a : file->assertions) {
      if (failed) break;
      CtFrame empty;
      Value condition = expr(a, empty, false);
      if (!failed && !condition.bits) err(a->loc, "E0406", "compile-time assertion failed");
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
