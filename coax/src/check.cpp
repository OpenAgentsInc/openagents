// Checker: names, types, places, returns and print formats. Reports every
// error it finds, each with a code and, where it can, a fix. Untyped integer
// and float literals are folded exactly here and take the type their use
// requires, so `x: u8 = 200;` needs no cast and `x: u8 = 300;` is an error.
#include "coax.h"

namespace coax {

// ---- Types ----

static TypeInfo scalar(TypeKind kind, const char *name, int32_t size, int32_t bits, bool is_signed) {
  TypeInfo t;
  t.kind = kind;
  t.name = name;
  t.size = size;
  t.align = size ? size : 1;
  t.bits = bits;
  t.is_signed = is_signed;
  return t;
}

TypeInfo T_INVALID = scalar(TypeKind::invalid, "<invalid>", 0, 0, false);
TypeInfo T_VOID = scalar(TypeKind::void_, "void", 0, 0, false);
TypeInfo T_BOOL = scalar(TypeKind::bool_, "bool", 1, 8, false);
TypeInfo T_STRING = [] { TypeInfo t = scalar(TypeKind::string_, "string", 16, 0, false); t.align = 8; return t; }();
TypeInfo T_UNTYPED_INT = scalar(TypeKind::untyped_int, "integer literal", 0, 0, true);
TypeInfo T_UNTYPED_FLOAT = scalar(TypeKind::untyped_float, "float literal", 0, 0, true);
TypeInfo T_F32 = scalar(TypeKind::floating, "f32", 4, 32, true);
TypeInfo T_F64 = scalar(TypeKind::floating, "f64", 8, 64, true);
TypeInfo T_S8 = scalar(TypeKind::integer, "s8", 1, 8, true);
TypeInfo T_S16 = scalar(TypeKind::integer, "s16", 2, 16, true);
TypeInfo T_S32 = scalar(TypeKind::integer, "s32", 4, 32, true);
TypeInfo T_S64 = scalar(TypeKind::integer, "int", 8, 64, true);
TypeInfo T_U8 = scalar(TypeKind::integer, "u8", 1, 8, false);
TypeInfo T_U16 = scalar(TypeKind::integer, "u16", 2, 16, false);
TypeInfo T_U32 = scalar(TypeKind::integer, "u32", 4, 32, false);
TypeInfo T_U64 = scalar(TypeKind::integer, "u64", 8, 64, false);

static std::vector<std::unique_ptr<TypeInfo>> g_composites;
static std::vector<Type> g_composite_list;

const std::vector<Type> &all_composite_types() { return g_composite_list; }

static Type intern(TypeKind kind, Type elem, int64_t length) {
  for (const auto &t : g_composites)
    if (t->kind == kind && t->elem == elem && t->length == length) return t.get();
  auto t = std::make_unique<TypeInfo>();
  t->kind = kind;
  t->elem = elem;
  t->length = length;
  t->id = (int32_t)g_composites.size();
  if (kind == TypeKind::array) {
    t->name = "[" + std::to_string(length) + "] " + elem->name;
    t->size = (int32_t)(elem->size * length);
    t->align = elem->align;
  } else {
    t->name = "[] " + elem->name;
    t->size = 16;
    t->align = 8;
  }
  g_composites.push_back(std::move(t));
  g_composite_list.push_back(g_composites.back().get());
  return g_composites.back().get();
}

Type array_of(Type elem, int64_t length) { return intern(TypeKind::array, elem, length); }
Type view_of(Type elem) { return intern(TypeKind::view, elem, 0); }
const char *type_name(Type t) { return t->name.c_str(); }

Intrinsic find_intrinsic(Str name) {
  static const char *NAMES[] = {"", "sin", "cos", "tan", "exp", "log", "sqrt", "floor", "abs", "pow"};
  for (int i = 1; i < (int)(sizeof NAMES / sizeof NAMES[0]); i++)
    if (str_eq(name, NAMES[i])) return (Intrinsic)i;
  return Intrinsic::none;
}

BinaryIntrinsic find_binary_intrinsic(Str name) {
  const char *names[] = {"", "bytes_overlap", "f32_from_bits", "f64_from_bits", "f32_bits", "f64_bits"};
  for (int i=1; i<6; i++) if (str_eq(name,names[i])) return (BinaryIntrinsic)i;
  return BinaryIntrinsic::none;
}

namespace {

// Names a struct field or exported parameter cannot have, because the C (and
// C++, for headers) on the other side of the boundary would not compile.
const char *C_KEYWORDS[] = {
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else", "enum", "extern",
    "float", "for", "goto", "if", "inline", "int", "long", "register", "restrict", "return", "short", "signed",
    "sizeof", "static", "struct", "switch", "typedef", "union", "unsigned", "void", "volatile", "while", "bool",
    "true", "false", "class", "new", "delete", "this", "template", "namespace", "public", "private",
    "protected", "operator", "virtual", "friend", "typename", "using", "nullptr", "explicit", "mutable",
    "throw", "try", "catch", "asm", "export", "module", "requires", "concept", "alignas", "alignof",
    "noexcept", "constexpr", "decltype", "static_assert", "thread_local", "errno", "NULL",
};

bool is_c_keyword(Str s) {
  for (const char *k : C_KEYWORDS)
    if (str_eq(s, k)) return true;
  return s.count > 0 && s.data[0] == '_';
}

const char *LATER_TYPES[] = {"fixed32", "fixed64", "v2", "v3", "v4", "quat", "m3", "m4", "Any", "Type", "Code"};

Type builtin_type(Str text) {
  if (str_eq(text, "int") || str_eq(text, "s64")) return &T_S64;
  if (str_eq(text, "s8")) return &T_S8;
  if (str_eq(text, "s16")) return &T_S16;
  if (str_eq(text, "s32")) return &T_S32;
  if (str_eq(text, "u8")) return &T_U8;
  if (str_eq(text, "u16")) return &T_U16;
  if (str_eq(text, "u32")) return &T_U32;
  if (str_eq(text, "u64") || str_eq(text, "uint")) return &T_U64;
  if (str_eq(text, "f32")) return &T_F32;
  if (str_eq(text, "f64")) return &T_F64;
  if (str_eq(text, "bool")) return &T_BOOL;
  if (str_eq(text, "string")) return &T_STRING;
  return nullptr;
}

// Canonical 64-bit pattern of an integer of type t: sign- or zero-extended.
uint64_t canon(Type t, uint64_t bits) {
  if (t->bits >= 64) return bits;
  uint64_t mask = (UINT64_C(1) << t->bits) - 1;
  bits &= mask;
  if (t->is_signed && (bits >> (t->bits - 1)) & 1) bits |= ~mask;
  return bits;
}

bool fits(Type t, __int128 v) {
  if (t->is_signed) {
    __int128 lo = -((__int128)1 << (t->bits - 1)), hi = ((__int128)1 << (t->bits - 1)) - 1;
    return v >= lo && v <= hi;
  }
  return v >= 0 && v <= (((__int128)1 << t->bits) - 1);
}

std::string int128_text(__int128 v) {
  if (v == 0) return "0";
  bool neg = v < 0;
  unsigned __int128 m = neg ? (unsigned __int128)(-(v + 1)) + 1 : (unsigned __int128)v;
  std::string s;
  while (m) { s += (char)('0' + (int)(m % 10)); m /= 10; }
  if (neg) s += '-';
  return std::string(s.rbegin(), s.rend());
}

struct Local {
  Str name;
  Type type;
  Loc loc;
  const char *immutable = nullptr;  // why it cannot be assigned: "parameter", "loop variable"
};

struct Checker {
  Arena *arena;
  File *file;
  const Source *src;
  bool has_basic = false;
  std::vector<Local> locals;
  ProcDecl *current = nullptr;
  uint32_t constant_depth = 0;
  int32_t loop_depth = 0, defer_depth = 0;
  int32_t unevaluated = 0;  // inside type_name: fold, but never fail on a value

  void err(Loc loc, const char *code, const std::string &msg, const std::string &fix = "") {
    error_at(src, loc, code, msg, fix);
  }

  // ---- declarations ----

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
  StructDecl *find_struct(Str name) {
    for (StructDecl *d : file->structs)
      if (str_eq(d->name, name)) return d;
    return nullptr;
  }
  EnumDecl *find_enum(Str name) {
    for (EnumDecl *d : file->enums)
      if (str_eq(d->name, name)) return d;
    return nullptr;
  }
  Local *find_local(Str name) {
    for (size_t i = locals.size(); i-- > 0;)
      if (str_eq(locals[i].name, name)) return &locals[i];
    return nullptr;
  }

  Type constant_type(ConstDecl *c) {
    if (c->checked == 2) return c->type;
    if (c->checked == 1) {
      err(c->loc, "E0340", "constant dependency cycle at '" + to_std(c->name) + "'");
      return &T_INVALID;
    }
    if (constant_depth >= 64) {
      err(c->loc, "E0347", "constant type dependency depth exceeds 64");
      return &T_INVALID;
    }
    constant_depth++;
    c->checked = 1;
    // A constant is checked with no locals in scope, so it cannot capture one.
    // A constant made only of literals stays untyped, like a literal: COUNT ::
    // 64; works as an s32, a u8 or an array length.
    std::vector<Local> saved;
    saved.swap(locals);
    c->type = expr(c->expr, nullptr);
    if (c->type->kind == TypeKind::untyped_int) {
      // It must fit its default type; cast(u64) makes a wider constant.
      Expr probe = *c->expr;
      coerce(&probe, T_INT);
      if (probe.type == &T_INVALID) c->type = &T_INVALID;
    }
    locals.swap(saved);
    if (c->type == &T_VOID) err(c->loc, "E0341", "a constant must have a value");
    c->checked = 2;
    constant_depth--;
    return c->type;
  }

  // ---- types ----

  // A compile-time integer known while checking: literals, constants made
  // of literals, size_of, and arithmetic on them. #run lengths are planned.
  bool const_int(Expr *e, __int128 *out, int depth = 0) {
    if (depth > 64) return false;
    switch (e->kind) {
      case ExprKind::int_lit:
        if (!is_int(e->type) && e->type->kind != TypeKind::untyped_int) return false;
        *out = e->type->kind == TypeKind::untyped_int
                   ? (e->negative ? (__int128)(int64_t)e->int_value : (__int128)e->int_value)
                   : (e->type->is_signed ? (__int128)(int64_t)e->int_value : (__int128)e->int_value);
        return true;
      case ExprKind::ident:
        return e->constant && e->constant->checked == 2 && const_int(e->constant->expr, out, depth + 1);
      case ExprKind::unary: {
        __int128 a;
        if (e->op != Tok::minus || !const_int(e->lhs, &a, depth + 1)) return false;
        *out = -a;
        return true;
      }
      case ExprKind::binary: {
        __int128 a, b;
        if (!const_int(e->lhs, &a, depth + 1) || !const_int(e->rhs, &b, depth + 1)) return false;
        switch (e->op) {
          case Tok::plus: *out = a + b; return true;
          case Tok::minus: *out = a - b; return true;
          case Tok::star: *out = a * b; return true;
          case Tok::slash: if (b == 0) return false; *out = a / b; return true;
          case Tok::percent: if (b == 0) return false; *out = a % b; return true;
          default: return false;
        }
      }
      default: return false;
    }
  }

  Type resolve_type(TypeExpr *t, bool by_value = true) {
    switch (t->kind) {
      case TypeExpr::named: {
        if (Type b = builtin_type(t->name)) return b;
        if (StructDecl *d = find_struct(t->name)) {
          if (by_value) layout(d);
          return d->type;
        }
        if (EnumDecl *d = find_enum(t->name)) return d->type;
        for (const char *later : LATER_TYPES)
          if (str_eq(t->name, later)) {
            err(t->loc, "E0301", "type '" + to_std(t->name) + "' is not supported by coax0 yet");
            return &T_INVALID;
          }
        if (str_eq(t->name, "void")) {
          err(t->loc, "E0302", "'void' is not a type you can write", "leave out '-> void'");
          return &T_INVALID;
        }
        err(t->loc, "E0302", "unknown type '" + to_std(t->name) + "'");
        return &T_INVALID;
      }
      case TypeExpr::dynamic:
        err(t->loc, "E0301", "dynamic arrays '[..] T' are not supported by coax0 yet",
            "use a fixed array '[N] T' and a count");
        return &T_INVALID;
      case TypeExpr::view: {
        Type elem = resolve_type(t->elem, false);
        return elem == &T_INVALID ? elem : view_of(elem);
      }
      case TypeExpr::array: {
        Type elem = resolve_type(t->elem, true);
        Type n = value(t->length, T_INT);
        __int128 length = 0;
        if (n == &T_INVALID || elem == &T_INVALID) return &T_INVALID;
        if (!is_int(n) || !const_int(t->length, &length)) {
          err(t->length->loc, "E0357",
              "an array length must be an integer known while checking: a literal, a constant, or arithmetic on them");
          return &T_INVALID;
        }
        if (length <= 0 || length * elem->size > ((__int128)1 << 30)) {
          err(t->length->loc, "E0357", "array length " + int128_text(length) + " is out of range (1 to 1 GiB total)");
          return &T_INVALID;
        }
        return array_of(elem, (int64_t)length);
      }
    }
    return &T_INVALID;
  }

  // Field types and C layout: declaration order, natural alignment.
  void layout(StructDecl *d) {
    if (d->state == 2) return;
    if (d->state == 1) {
      err(d->loc, "E0353", "struct '" + to_std(d->name) + "' contains itself",
          "hold it through a view '[] " + to_std(d->name) + "' or an index");
      d->state = 2;
      return;
    }
    d->state = 1;
    int32_t offset = 0, align = 1;
    for (int32_t i = 0; i < d->fields.count; i++) {
      Field &f = d->fields[i];
      for (int32_t j = 0; j < i; j++)
        if (str_eq(d->fields[j].name, f.name))
          err(f.loc, "E0352", "duplicate field '" + to_std(f.name) + "' in struct '" + to_std(d->name) + "'");
      if (is_c_keyword(f.name))
        err(f.loc, "E0371", "field name '" + to_std(f.name) + "' is reserved in C or C++, where this struct also lives",
            "pick another name");
      f.type = resolve_type(f.type_expr);
      int32_t fa = f.type->align, fs = f.type->size;
      offset = (offset + fa - 1) / fa * fa;
      f.offset = offset;
      offset += fs;
      if (fa > align) align = fa;
    }
    d->type->size = (offset + align - 1) / align * align;
    d->type->align = align;
    if (d->type->size == 0) d->type->size = 1, d->type->align = 1;  // empty structs still have a size in C
    d->state = 2;
  }

  // ---- untyped literals ----

  static __int128 untyped_value(const Expr *e) {
    return e->negative ? (__int128)(int64_t)e->int_value : (__int128)e->int_value;
  }

  void set_untyped_int(Expr *e, __int128 v) {
    if (v < (__int128)INT64_MIN || v > (__int128)UINT64_MAX) {
      err(e->loc, "E0350", "constant " + int128_text(v) + " does not fit in 64 bits");
      e->type = &T_INVALID;
      return;
    }
    bool operation = e->kind != ExprKind::int_lit;
    e->kind = ExprKind::int_lit;
    e->type = &T_UNTYPED_INT;
    e->folded = e->folded || operation;
    e->negative = v < 0;
    e->int_value = (uint64_t)(int64_t)v;
    if (v > (__int128)INT64_MAX) e->int_value = (uint64_t)v;
    e->lhs = e->rhs = nullptr;
  }

  void set_untyped_float(Expr *e, double v) {
    e->kind = ExprKind::float_lit;
    e->type = &T_UNTYPED_FLOAT;
    e->float_value = v;
    e->lhs = e->rhs = nullptr;
  }

  static double as_double(const Expr *e) {
    return e->type->kind == TypeKind::untyped_float ? e->float_value : (double)untyped_value(e);
  }

  // Both operands are untyped: compute exactly now.
  Type fold_binary(Expr *e) {
    Expr *a = e->lhs, *b = e->rhs;
    bool any_float = a->type->kind == TypeKind::untyped_float || b->type->kind == TypeKind::untyped_float;
    switch (e->op) {
      case Tok::eqeq: case Tok::noteq: case Tok::lt: case Tok::le: case Tok::gt: case Tok::ge: {
        Type t = any_float ? &T_F64 : T_INT;
        coerce(a, t);
        coerce(b, t);
        return &T_BOOL;
      }
      case Tok::andand: case Tok::oror:
        err(e->loc, "E0312", std::string(tok_name(e->op)) + " needs two bools, found numbers");
        return &T_INVALID;
      default: break;
    }
    if (any_float) {
      double x = as_double(a), y = as_double(b), r = 0;
      switch (e->op) {
        case Tok::plus: r = x + y; break;
        case Tok::minus: r = x - y; break;
        case Tok::star: r = x * y; break;
        case Tok::slash: r = x / y; break;
        default:
          err(e->loc, "E0370", std::string(tok_name(e->op)) + " needs integers, found a float literal");
          return &T_INVALID;
      }
      set_untyped_float(e, r);
      return e->type;
    }
    __int128 x = untyped_value(a), y = untyped_value(b), r = 0;
    switch (e->op) {
      case Tok::plus: r = x + y; break;
      case Tok::minus: r = x - y; break;
      case Tok::star: r = x * y; break;
      case Tok::slash: case Tok::percent:
        if (y == 0) {
          if (unevaluated) { set_untyped_int(e, 0); return e->type; }
          err(e->loc, "E0407", "division by zero in a constant expression");
          return &T_INVALID;
        }
        r = e->op == Tok::slash ? x / y : x % y;
        break;
      case Tok::amp: r = x & y; break;
      case Tok::pipe: r = x | y; break;
      case Tok::caret: r = x ^ y; break;
      case Tok::shl: case Tok::shr:
        if (y < 0 || y > 63) { err(e->loc, "E0370", "shift amount must be 0 to 63"); return &T_INVALID; }
        r = e->op == Tok::shl ? x << (int)y : x >> (int)y;
        break;
      default: return &T_INVALID;
    }
    set_untyped_int(e, r);
    return e->type;
  }

  // Give an untyped literal its type, checking that the value fits.
  void coerce(Expr *e, Type to) {
    Type from = e->type;
    if (!is_untyped(from) || to == &T_INVALID) return;
    if (from->kind == TypeKind::untyped_int && is_int(to)) {
      __int128 v = untyped_value(e);
      if (!fits(to, v) && unevaluated) v = 0;
      if (!fits(to, v)) {
        if (e->folded)
          err(e->loc, "E0407", "integer overflow in a constant expression: " + int128_text(v) + " does not fit in " +
                                   type_name(to));
        else
          err(e->loc, "E0350", int128_text(v) + " does not fit in " + type_name(to),
              "use a wider type, or cast,trunc(" + std::string(type_name(to)) + ") to keep the low bits");
        e->type = &T_INVALID;
        return;
      }
      e->int_value = canon(to, (uint64_t)v);
      e->negative = false;
      e->type = to;
    } else if (is_float(to)) {
      double v = as_double(e);
      if (to == &T_F32 && !__builtin_isinf(v) && __builtin_isinf((float)v) && !unevaluated) {
        char text[32];
        snprintf(text, sizeof text, "%g", v);
        err(e->loc, "E0350", std::string(text) + " does not fit in f32", "use f64, or a smaller value");
        e->type = &T_INVALID;
        return;
      }
      e->kind = ExprKind::float_lit;
      e->float_value = to == &T_F32 ? (double)(float)v : v;
      e->type = to;
    } else if (from->kind == TypeKind::untyped_float && is_int(to)) {
      err(e->loc, "E0350", "a float literal cannot become " + std::string(type_name(to)),
          "write an integer, or cast(" + std::string(type_name(to)) + ") a float value");
      e->type = &T_INVALID;
    }
    // Any other target (bool, string, a struct): leave it untyped so the
    // caller reports the mismatch with its own context.
  }

  Type default_type(Type t) {
    if (t->kind == TypeKind::untyped_int) return T_INT;
    if (t->kind == TypeKind::untyped_float) return &T_F64;
    return t;
  }

  // Check e, give an untyped result the wanted type (or the default when
  // nothing is wanted), and return its final type.
  Type value(Expr *e, Type want) {
    Type t = expr(e, want);
    if (is_untyped(t)) coerce(e, want && is_number(want) ? want : default_type(t));
    return e->type;
  }

  // ---- places ----

  // Why e cannot be assigned, or nullptr when it can.
  const char *immutable_reason(const Expr *e) {
    switch (e->kind) {
      case ExprKind::ident:
        if (Local *l = find_local(e->text)) return l->immutable;
        return "constant";
      case ExprKind::field: return e->lhs->type->kind == TypeKind::struct_ ? immutable_reason(e->lhs) : "count";
      case ExprKind::index:
        if (e->lhs->type->kind == TypeKind::view) return nullptr;  // a view's elements belong to its owner
        if (e->lhs->type->kind == TypeKind::string_) return "string byte";
        return immutable_reason(e->lhs);
      default: return "temporary";
    }
  }

  void require_mutable(const Expr *e, Loc loc) {
    const char *why = immutable_reason(e);
    if (!why) return;
    std::string w = why;
    if (w == "constant") err(loc, "E0344", "constants cannot be assigned");
    else if (w == "parameter") err(loc, "E0344", "parameters cannot be assigned", "copy it into a local variable");
    else if (w == "string byte") err(loc, "E0359", "strings are immutable", "copy the bytes into a '[N] u8'");
    else if (w == "count") err(loc, "E0359", "'.count' cannot be assigned");
    else if (w == "temporary") err(loc, "E0359", "this value is a temporary and cannot be assigned", "store it in a variable first");
    else err(loc, "E0359", "a " + w + " cannot be assigned", "copy it into a local variable");
  }

  // ---- expressions ----

  Type expr(Expr *e, Type want = nullptr) {
    e->type = expr_inner(e, want);
    return e->type;
  }

  Type expr_inner(Expr *e, Type want) {
    switch (e->kind) {
      case ExprKind::int_lit: return is_untyped(e->type) || e->type == &T_INVALID ? &T_UNTYPED_INT : e->type;
      case ExprKind::float_lit: return e->type == &T_INVALID ? &T_UNTYPED_FLOAT : e->type;
      case ExprKind::bool_lit: return &T_BOOL;
      case ExprKind::string_lit: return &T_STRING;
      case ExprKind::enum_value: case ExprKind::ct_value: return e->type;
      case ExprKind::run: {
        Type t = value(e->lhs, want);
        if (t == &T_VOID) err(e->loc, "E0341", "#run must produce a value");
        return t;
      }
      case ExprKind::ident: {
        if (Local *l = find_local(e->text)) return l->type;
        if (ConstDecl *c = find_constant(e->text)) {
          Type t = constant_type(c);
          if (is_untyped(t) && (c->expr->kind == ExprKind::int_lit || c->expr->kind == ExprKind::float_lit)) {
            Loc loc = e->loc;
            *e = *c->expr;  // the use becomes the literal, and takes its type from context
            e->loc = loc;
            return t;
          }
          e->constant = c;
          return t;
        }
        if (find_proc(e->text)) {
          err(e->loc, "E0305", "procedures are not values in coax0", "call it: " + to_std(e->text) + "()");
          return &T_INVALID;
        }
        if (find_struct(e->text) || find_enum(e->text)) {
          err(e->loc, "E0354", "'" + to_std(e->text) + "' is a type, not a value",
              find_struct(e->text) ? "make one: " + to_std(e->text) + ".{}" : "name a member: " + to_std(e->text) + ".member");
          return &T_INVALID;
        }
        err(e->loc, "E0306", "'" + to_std(e->text) + "' is not declared");
        return &T_INVALID;
      }
      case ExprKind::call: return call(e, want);
      case ExprKind::unary: {
        Type t = expr(e->lhs, want);
        if (t == &T_INVALID) return t;
        if (e->op == Tok::minus) {
          if (t->kind == TypeKind::untyped_int) {
            // -128 is a written literal, not an operation that can overflow.
            bool folded = e->lhs->folded;
            set_untyped_int(e, -untyped_value(e->lhs));
            e->folded = folded;
            return e->type;
          }
          if (t->kind == TypeKind::untyped_float) { set_untyped_float(e, -e->lhs->float_value); return e->type; }
          if (!(is_float(t) || (is_int(t) && t->is_signed))) {
            err(e->loc, "E0307", std::string("'-' needs a signed integer or a float, found ") + type_name(t));
            return &T_INVALID;
          }
          return t;
        }
        if (t != &T_BOOL) {
          err(e->loc, "E0308", std::string("'!' needs a bool, found ") + type_name(t),
              is_number(t) || is_untyped(t) ? "compare it: x == 0" : "");
          return &T_INVALID;
        }
        return t;
      }
      case ExprKind::binary: return binary(e, want);
      case ExprKind::field: return field(e);
      case ExprKind::index: {
        Type t = expr(e->lhs);
        Type i = value(e->rhs, T_INT);
        if (t == &T_INVALID || i == &T_INVALID) return &T_INVALID;
        if (!is_int(i)) {
          err(e->rhs->loc, "E0356", std::string("an index must be an integer, found ") + type_name(i));
          return &T_INVALID;
        }
        if (t->kind == TypeKind::array && e->rhs->kind == ExprKind::int_lit && !unevaluated) {
          uint64_t raw = e->rhs->int_value;
          bool below = i->is_signed && (int64_t)raw < 0;
          if (below || raw >= (uint64_t)t->length) {
            err(e->rhs->loc, "E0374",
                "index " + (below ? std::to_string((int64_t)raw) : std::to_string(raw)) + " is outside " +
                    type_name(t),
                "valid indexes are 0 to " + std::to_string(t->length - 1));
            return &T_INVALID;
          }
        }
        if (t->kind == TypeKind::array || t->kind == TypeKind::view) return t->elem;
        if (t == &T_STRING) return &T_U8;
        err(e->loc, "E0355", std::string("cannot index ") + type_name(t),
            "only arrays, views and strings can be indexed");
        return &T_INVALID;
      }
      case ExprKind::slice: {
        Type t = expr(e->lhs);
        bool bounds_ok = true;
        for (Expr *b : {e->rhs, e->third})
          if (b) {
            Type bt = value(b, T_INT);
            if (bt != &T_INVALID && !is_int(bt)) {
              err(b->loc, "E0356", std::string("a slice bound must be an integer, found ") + type_name(bt));
              bounds_ok = false;
            }
          }
        if (t == &T_INVALID || !bounds_ok) return &T_INVALID;
        if (t == &T_STRING) return t;
        if (t->kind == TypeKind::view) return t;
        if (t->kind == TypeKind::array) {
          // A view of an array borrows its storage, so the array must be a
          // variable that may change; a view of a parameter would let callers
          // see writes to a value they passed by copy.
          if (const char *why = immutable_reason(e->lhs)) {
            err(e->loc, "E0366", "a view needs an array variable it may write through, and this is a " + std::string(why),
                "copy it into a local variable first");
            return &T_INVALID;
          }
          return view_of(t->elem);
        }
        err(e->loc, "E0355", std::string("cannot slice ") + type_name(t), "only arrays, views and strings can be sliced");
        return &T_INVALID;
      }
      case ExprKind::struct_lit: {
        StructDecl *d = find_struct(e->type_expr->name);
        if (!d) {
          for (const FieldInit &fi : e->fields) value(fi.value, nullptr);
          err(e->loc, find_enum(e->type_expr->name) ? "E0354" : "E0302",
              "'" + to_std(e->type_expr->name) + "' is not a struct");
          return &T_INVALID;
        }
        layout(d);
        for (int32_t i = 0; i < e->fields.count; i++) {
          FieldInit &fi = e->fields[i];
          for (int32_t j = 0; j < i; j++)
            if (str_eq(e->fields[j].name, fi.name)) err(fi.loc, "E0352", "field '" + to_std(fi.name) + "' is given twice");
          for (int32_t k = 0; k < d->fields.count; k++)
            if (str_eq(d->fields[k].name, fi.name)) fi.field = k;
          if (fi.field < 0) {
            value(fi.value, nullptr);
            err(fi.loc, "E0351", "struct '" + to_std(d->name) + "' has no field '" + to_std(fi.name) + "'",
                field_list(d));
            continue;
          }
          Type ft = d->fields[fi.field].type;
          Type vt = value(fi.value, ft);
          if (vt != &T_INVALID && ft != &T_INVALID && vt != ft)
            err(fi.value->loc, "E0367",
                "field '" + to_std(fi.name) + "' is " + type_name(ft) + ", given " + type_name(vt));
        }
        return d->type;
      }
      case ExprKind::array_lit: {
        Type t = resolve_type(e->type_expr);
        if (t == &T_INVALID) {
          for (Expr *a : e->args) value(a, nullptr);
          return t;
        }
        if (e->args.count != t->length)
          err(e->loc, "E0358",
              std::string(type_name(t)) + " needs " + std::to_string(t->length) + " elements, given " +
                  std::to_string(e->args.count));
        for (Expr *a : e->args) {
          Type at = value(a, t->elem);
          if (at != &T_INVALID && at != t->elem)
            err(a->loc, "E0367", std::string("elements are ") + type_name(t->elem) + ", given " + type_name(at));
        }
        return t;
      }
      case ExprKind::cast: return cast(e);
      case ExprKind::ifx: {
        Type c = value(e->lhs, &T_BOOL);
        if (c != &T_INVALID && c != &T_BOOL)
          err(e->lhs->loc, "E0331", std::string("condition must be bool, found ") + type_name(c));
        Type a = expr(e->rhs, want), b = expr(e->third, want);
        if (a == &T_INVALID || b == &T_INVALID) return &T_INVALID;
        if (is_untyped(a) && !is_untyped(b)) coerce(e->rhs, b);
        else if (is_untyped(b) && !is_untyped(a)) coerce(e->third, a);
        else if (is_untyped(a) && is_untyped(b)) {
          Type t = want && is_number(want) ? want
                 : (a->kind == TypeKind::untyped_float || b->kind == TypeKind::untyped_float) ? &T_F64 : T_INT;
          coerce(e->rhs, t);
          coerce(e->third, t);
        }
        a = e->rhs->type;
        b = e->third->type;
        if (a == &T_INVALID || b == &T_INVALID) return &T_INVALID;
        if (a != b) {
          err(e->loc, "E0365", std::string("ifx branches must have one type, found ") + type_name(a) + " and " +
                                   type_name(b));
          return &T_INVALID;
        }
        if (a == &T_VOID) {
          err(e->loc, "E0365", "ifx branches must produce values");
          return &T_INVALID;
        }
        return a;
      }
    }
    return &T_INVALID;
  }

  static std::string field_list(StructDecl *d) {
    std::string s = "its fields are:";
    for (const Field &f : d->fields) s += " " + to_std(f.name);
    if (!d->fields.count) s = "it has no fields";
    return s;
  }

  Type field(Expr *e) {
    // Enum.member
    if (e->lhs->kind == ExprKind::ident && !find_local(e->lhs->text) && !find_constant(e->lhs->text)) {
      if (EnumDecl *d = find_enum(e->lhs->text)) {
        for (const EnumMember &m : d->members)
          if (str_eq(m.name, e->text)) {
            e->kind = ExprKind::enum_value;
            e->int_value = m.bits;
            e->lhs->type = d->type;
            return d->type;
          }
        std::string names = "its members are:";
        for (const EnumMember &m : d->members) names += " " + to_std(m.name);
        err(e->loc, "E0351", "enum '" + to_std(d->name) + "' has no member '" + to_std(e->text) + "'", names);
        return &T_INVALID;
      }
    }
    Type t = expr(e->lhs);
    if (t == &T_INVALID) return t;
    if (str_eq(e->text, "count") &&
        (t->kind == TypeKind::array || t->kind == TypeKind::view || t == &T_STRING)) {
      e->field_index = -2;
      return T_INT;
    }
    if (t->kind == TypeKind::struct_) {
      StructDecl *d = t->struct_decl;
      for (int32_t i = 0; i < d->fields.count; i++)
        if (str_eq(d->fields[i].name, e->text)) {
          e->field_index = i;
          return d->fields[i].type;
        }
      err(e->loc, "E0351", "struct '" + to_std(d->name) + "' has no field '" + to_std(e->text) + "'", field_list(d));
      return &T_INVALID;
    }
    err(e->loc, "E0351", std::string(type_name(t)) + " has no field '" + to_std(e->text) + "'",
        t->kind == TypeKind::array || t->kind == TypeKind::view || t == &T_STRING ? "it has '.count'" : "");
    return &T_INVALID;
  }

  Type cast(Expr *e) {
    Type to = resolve_type(e->type_expr);
    Type from = expr(e->lhs, to);
    if (to == &T_INVALID || from == &T_INVALID) return &T_INVALID;
    if (is_untyped(from)) {
      bool direct = is_float(to) || (is_int(to) && from->kind == TypeKind::untyped_int);
      if (!e->trunc && direct) {
        // cast(u8) 200 is the literal 200 as u8, checked now.
        coerce(e->lhs, to);
        if (e->lhs->type == &T_INVALID) return &T_INVALID;
        Expr *lit = e->lhs;
        *e = *lit;
        return e->type;
      }
      coerce(e->lhs, default_type(from));
      from = e->lhs->type;
    }
    bool ok = false;
    if (is_number(from) && is_number(to)) ok = !e->trunc || (is_int(from) && is_int(to));
    else if (from->kind == TypeKind::enum_ && is_int(to)) ok = !e->trunc;
    else if (is_int(from) && to->kind == TypeKind::enum_) ok = !e->trunc;
    else if (from == to) ok = !e->trunc;
    else if (to == &T_STRING && from->kind == TypeKind::view && from->elem == &T_U8) ok = !e->trunc;  // borrows the bytes
    if (!ok) {
      err(e->loc, "E0362",
          std::string("cannot ") + (e->trunc ? "cast,trunc " : "cast ") + type_name(from) + " to " + type_name(to),
          e->trunc ? "cast,trunc converts between integer types"
                   : "cast converts numbers, enums to and from integers, and a [] u8 view to a string");
      return &T_INVALID;
    }
    return to;
  }

  Type binary(Expr *e, Type want) {
    bool comparison = e->op == Tok::eqeq || e->op == Tok::noteq || e->op == Tok::lt || e->op == Tok::le ||
                      e->op == Tok::gt || e->op == Tok::ge;
    bool shift = e->op == Tok::shl || e->op == Tok::shr;
    Type hint = comparison || e->op == Tok::andand || e->op == Tok::oror ? nullptr : want;
    Type a = expr(e->lhs, hint), b = expr(e->rhs, shift ? nullptr : hint);
    if (a == &T_INVALID || b == &T_INVALID) return &T_INVALID;
    if (is_untyped(a) && is_untyped(b)) return fold_binary(e);
    if (shift) {
      if (is_untyped(a)) coerce(e->lhs, hint && is_int(hint) ? hint : T_INT);
      if (is_untyped(b)) coerce(e->rhs, is_int(e->lhs->type) ? e->lhs->type : T_INT);
    } else {
      if (is_untyped(a)) coerce(e->lhs, b);
      if (is_untyped(b)) coerce(e->rhs, a);
    }
    a = e->lhs->type;
    b = e->rhs->type;
    if (a == &T_INVALID || b == &T_INVALID) return &T_INVALID;
    std::string pair = std::string(type_name(a)) + " and " + type_name(b);
    switch (e->op) {
      case Tok::plus: case Tok::minus: case Tok::star: case Tok::slash: case Tok::percent:
        if (a != b || !is_number(a) || (e->op == Tok::percent && !is_int(a))) {
          std::string need = e->op == Tok::percent ? " needs two integers of one type" : " needs two numbers of one type";
          std::string fix = a == &T_STRING ? "build strings with print's % formatting"
                          : is_number(a) && is_number(b) ? "convert one: cast(" + std::string(type_name(a)) + ") x" : "";
          err(e->loc, "E0309", std::string(tok_name(e->op)) + need + ", found " + pair, fix);
          return &T_INVALID;
        }
        return a;
      case Tok::amp: case Tok::pipe: case Tok::caret:
        if (a != b || !is_int(a)) {
          err(e->loc, "E0370", std::string(tok_name(e->op)) + " needs two integers of one type, found " + pair);
          return &T_INVALID;
        }
        return a;
      case Tok::shl: case Tok::shr:
        if (!is_int(a) || !is_int(b)) {
          err(e->loc, "E0370", std::string(tok_name(e->op)) + " shifts an integer by an integer, found " + pair);
          return &T_INVALID;
        }
        return a;
      case Tok::lt: case Tok::le: case Tok::gt: case Tok::ge:
        if (a != b || !is_number(a)) {
          err(e->loc, "E0310", std::string(tok_name(e->op)) + " compares two numbers of one type, found " + pair,
              is_number(a) && is_number(b) ? "convert one: cast(" + std::string(type_name(a)) + ") x" : "");
          return &T_INVALID;
        }
        return &T_BOOL;
      case Tok::eqeq: case Tok::noteq:
        if (a != b) {
          err(e->loc, "E0311", std::string("cannot compare ") + type_name(a) + " with " + type_name(b),
              "there are no implicit conversions in Coax");
          return &T_INVALID;
        }
        if (is_aggregate(a) || a->kind == TypeKind::view) {
          err(e->loc, "E0311", std::string("cannot compare ") + type_name(a) + " values with " + tok_name(e->op),
              "compare the fields or elements you mean");
          return &T_INVALID;
        }
        return &T_BOOL;
      case Tok::andand: case Tok::oror:
        if (a != &T_BOOL || b != &T_BOOL) {
          err(e->loc, "E0312", std::string(tok_name(e->op)) + " needs two bools, found " + pair);
          return &T_INVALID;
        }
        return &T_BOOL;
      default: break;
    }
    return &T_INVALID;
  }

  // A cast node made from a call, for int(x), f32(x) and byte_at(s, i).
  Type call_as_cast(Expr *e, Str type, Expr *operand) {
    TypeExpr *te = make<TypeExpr>(arena);
    te->kind = TypeExpr::named;
    te->loc = e->loc;
    te->name = type;
    e->kind = ExprKind::cast;
    e->type_expr = te;
    e->lhs = operand;
    e->args = {};
    return cast(e);
  }

  // sin(x) .. pow(x, y): floats of one type. Untyped arguments take a
  // typed argument's type, else the wanted float type, else f64.
  Type math_call(Expr *e, Intrinsic in, Type want) {
    std::string name = to_std(e->text);
    int32_t arity = in == Intrinsic::pow ? 2 : 1;
    e->intrinsic = in;
    if (e->args.count != arity) {
      err(e->loc, "E0375", name + " takes " + (arity == 1 ? "one value" : "two values") + ", given " +
                               std::to_string(e->args.count));
      for (Expr *a : e->args) value(a, nullptr);
      return &T_INVALID;
    }
    Type hint = want && is_float(want) ? want : nullptr;
    bool bad = false;
    for (Expr *a : e->args) bad = expr(a, hint) == &T_INVALID || bad;
    if (bad) return &T_INVALID;
    Type t = nullptr;
    for (Expr *a : e->args)
      if (!is_untyped(a->type)) { t = a->type; break; }
    if (!t) t = hint ? hint : &T_F64;
    for (Expr *a : e->args)
      if (is_untyped(a->type)) coerce(a, t);
    for (Expr *a : e->args) {
      if (a->type == &T_INVALID) return &T_INVALID;
      if (a->type != t || !is_float(t)) {
        err(a->loc, "E0375",
            name + (arity == 1 ? " needs a float" : " needs two floats of one type") + ", found " + type_name(a->type),
            is_int(a->type) ? "convert it first: f64(x)" : "");
        return &T_INVALID;
      }
    }
    return t;
  }

  Type call(Expr *e, Type want) {
    // T(x) is cast(T) x for the number types: int(x), f32(x), u8(x).
    if (Type b = builtin_type(e->text); b && is_number(b) && !e->type_expr) {
      if (e->args.count != 1) {
        err(e->loc, "E0362", to_std(e->text) + "(x) converts one value, given " + std::to_string(e->args.count));
        for (Expr *a : e->args) value(a, nullptr);
        return &T_INVALID;
      }
      return call_as_cast(e, e->text, e->args[0]);
    }
    if (str_eq(e->text, "len") && !find_proc(e->text)) {
      // len(x) is x.count, for strings, arrays and views.
      if (e->args.count != 1) {
        err(e->loc, "E0375", "len takes one value, given " + std::to_string(e->args.count));
        return &T_INVALID;
      }
      e->kind = ExprKind::field;
      e->lhs = e->args[0];
      e->args = {};
      e->text = Str{"count", 5};
      return field(e);
    }
    if (str_eq(e->text, "byte_at") && !find_proc(e->text)) {
      // byte_at(s, i) is cast(int) s[i]: a bounds-checked byte as an int.
      if (e->args.count != 2) {
        err(e->loc, "E0375", "byte_at takes a string and an index, given " + std::to_string(e->args.count) + " values");
        return &T_INVALID;
      }
      Expr *ix = make<Expr>(arena);
      ix->kind = ExprKind::index;
      ix->loc = e->loc;
      ix->lhs = e->args[0];
      ix->rhs = e->args[1];
      return call_as_cast(e, Str{"int", 3}, ix);
    }
    if (BinaryIntrinsic in = find_binary_intrinsic(e->text); in != BinaryIntrinsic::none) {
      e->binary_intrinsic = in;
      int n = in == BinaryIntrinsic::bytes_overlap ? 2 : 1;
      if (e->args.count != n) { err(e->loc,"E0378","wrong binary intrinsic argument count"); return &T_INVALID; }
      Type target = in == BinaryIntrinsic::f32_from_bits ? &T_U32 : in == BinaryIntrinsic::f64_from_bits ? &T_U64 : in == BinaryIntrinsic::f32_bits ? &T_F32 : &T_F64;
      for (int i=0; i<n; i++) {
        Type t = value(e->args[i], in == BinaryIntrinsic::bytes_overlap ? nullptr : target);
        bool span = t == &T_STRING || (t->kind == TypeKind::view && t->elem == &T_U8);
        if ((in == BinaryIntrinsic::bytes_overlap && !span) || (in != BinaryIntrinsic::bytes_overlap && t != target))
          err(e->loc,"E0378","binary intrinsic argument type mismatch");
      }
      return in == BinaryIntrinsic::bytes_overlap ? &T_BOOL : in == BinaryIntrinsic::f32_from_bits ? &T_F32 : in == BinaryIntrinsic::f64_from_bits ? &T_F64 : in == BinaryIntrinsic::f32_bits ? &T_U32 : &T_U64;
    }
    if (Intrinsic in = find_intrinsic(e->text); in != Intrinsic::none) return math_call(e, in, want);
    if (str_eq(e->text, "size_of") && e->type_expr) {
      Type t = resolve_type(e->type_expr);
      if (t == &T_INVALID) return t;
      set_untyped_int(e, t->size);
      return e->type;
    }
    if (str_eq(e->text, "type_name")) {
      if (e->args.count != 1) {
        err(e->loc, "E0343", "type_name needs exactly one expression");
        return &T_INVALID;
      }
      unevaluated++;
      Type t = value(e->args[0], nullptr);
      unevaluated--;
      if (t == &T_INVALID) return t;
      if (t == &T_VOID) {
        err(e->loc, "E0343", "type_name needs an expression with a value");
        return &T_INVALID;
      }
      // The operand is typechecked but never executed, even at compile time.
      const char *name = type_name(t);
      e->kind = ExprKind::string_lit;
      e->text = Str{name, (int32_t)strlen(name)};
      e->args = {};
      return &T_STRING;
    }
    if (find_constant(e->text)) {
      err(e->loc, "E0314", "'" + to_std(e->text) + "' is a constant, not a procedure");
      return &T_INVALID;
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
      else if (find_struct(e->text))
        err(e->loc, "E0354", "'" + to_std(e->text) + "' is a struct, not a procedure",
            "make one: " + to_std(e->text) + ".{field = value}");
      else
        err(e->loc, "E0315", "procedure '" + to_std(e->text) + "' is not declared");
      for (Expr *a : e->args) value(a, nullptr);
      return &T_INVALID;
    }
    e->proc = p;
    if (e->args.count != p->params.count) {
      err(e->loc, "E0316",
          "'" + to_std(p->name) + "' takes " + std::to_string(p->params.count) + " argument" +
              (p->params.count == 1 ? "" : "s") + ", given " + std::to_string(e->args.count));
    }
    for (int32_t i = 0; i < e->args.count; i++) {
      Type want = i < p->params.count ? p->params[i].type : nullptr;
      Type t = value(e->args[i], want);
      if (want && t != &T_INVALID && want != &T_INVALID && t != want) {
        err(e->args[i]->loc, "E0317",
            "argument " + std::to_string(i + 1) + " of '" + to_std(p->name) + "' is " + type_name(want) +
                ", given " + type_name(t),
            t->kind == TypeKind::array && want->kind == TypeKind::view ? "pass a view of it: name[..]" : "");
      }
    }
    return p->ret;
  }

  // print(format, args...): the format must be a literal, and its '%'
  // count must match the arguments (spec §4.1). '%%' prints a '%'.
  Type print_call(Expr *e) {
    if (e->args.count == 0) {
      err(e->loc, "E0318", "print needs a format string", "print(\"text\\n\");");
      return &T_VOID;
    }
    Expr *fmt = e->args[0];
    if (fmt->kind != ExprKind::string_lit) {
      value(fmt, nullptr);
      err(fmt->loc, "E0319", "print's format must be a string literal, so it can be checked",
          "print(\"%\\n\", value);");
      return &T_VOID;
    }
    fmt->type = &T_STRING;
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
      Type t = value(e->args[i], nullptr);
      if (t == &T_VOID) err(e->args[i]->loc, "E0321", "this call returns nothing, so it cannot be printed");
    }
    return &T_VOID;
  }

  // ---- statements ----

  bool declare(Str name, Type type, Loc loc, const char *immutable = nullptr) {
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
    if (find_constant(name) || is_builtin_name(name) || find_struct(name) || find_enum(name)) {
      err(loc, "E0342", "'" + to_std(name) + "' is already a constant, type or intrinsic", "pick another name");
      return false;
    }
    locals.push_back(Local{name, type, loc, immutable});
    return true;
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

  void loop_body(Stmt *body) {
    loop_depth++;
    stmt(body);
    loop_depth--;
  }

  void stmt(Stmt *s) {
    switch (s->kind) {
      case StmtKind::block: {
        size_t mark = locals.size();
        for (Stmt *c : s->body) stmt(c);
        locals.resize(mark);
        break;
      }
      case StmtKind::expr: {
        Type t = value(s->expr, nullptr);
        if (s->expr->kind != ExprKind::call && t != &T_INVALID)
          err(s->loc, "E0322", "this expression's value is not used", "assign it, or remove it");
        else if (s->expr->kind == ExprKind::call && t != &T_VOID && t != &T_INVALID)
          err(s->loc, "E0323",
              "the result of '" + to_std(s->expr->text) + "' (" + type_name(t) + ") is not used",
              "use it: result := " + to_std(s->expr->text) + "(...);");
        break;
      }
      case StmtKind::var: {
        Type declared = s->type_expr ? resolve_type(s->type_expr) : &T_INVALID;
        Type init = s->expr ? value(s->expr, s->type_expr ? declared : nullptr) : &T_INVALID;
        if (s->expr && init == &T_VOID) {
          err(s->expr->loc, "E0324", "this call returns nothing, so it cannot initialize a variable");
          init = &T_INVALID;
        }
        if (s->type_expr && s->expr && declared != &T_INVALID && init != &T_INVALID && declared != init) {
          err(s->expr->loc, "E0325",
              "'" + to_std(s->name) + "' is " + type_name(declared) + " but is initialized with " + type_name(init),
              is_number(declared) && is_number(init) ? "convert it: cast(" + std::string(type_name(declared)) + ") x"
              : init->kind == TypeKind::array && declared->kind == TypeKind::view ? "take a view: name[..]" : "");
        }
        s->type = s->type_expr ? declared : init;
        declare(s->name, s->type, s->loc);
        break;
      }
      case StmtKind::assign: {
        if (s->target->kind == ExprKind::ident && !find_local(s->target->text) && find_constant(s->target->text)) {
          err(s->loc, "E0344", "constants cannot be assigned");
          value(s->expr, nullptr);
          break;
        }
        Type target = expr(s->target);
        if (target == &T_INVALID) {
          value(s->expr, nullptr);
          break;
        }
        require_mutable(s->target, s->loc);
        Type t = value(s->expr, target);
        if (t == &T_INVALID) break;
        if (s->op != Tok::equal) {
          Tok op = compound_op(s->op);
          bool int_only = op == Tok::percent || op == Tok::amp || op == Tok::pipe || op == Tok::caret ||
                          op == Tok::shl || op == Tok::shr;
          bool shift = op == Tok::shl || op == Tok::shr;
          if (int_only ? !is_int(target) : !is_number(target)) {
            err(s->loc, "E0326", std::string(tok_name(s->op)) + " needs " + (int_only ? "an integer" : "a number") +
                                     " target, this is " + type_name(target));
          } else if (shift ? !is_int(t) : t != target) {
            err(s->expr->loc, "E0327", std::string("the target is ") + type_name(target) + ", given " + type_name(t));
          }
        } else if (t != target) {
          err(s->expr->loc, "E0327", std::string("the target is ") + type_name(target) + ", assigned " + type_name(t),
              is_number(target) && is_number(t) ? "convert it: cast(" + std::string(type_name(target)) + ") x" : "");
        }
        break;
      }
      case StmtKind::ret: {
        if (defer_depth) {
          err(s->loc, "E0361", "a deferred statement cannot return", "return after the defer runs instead");
          break;
        }
        Type want = current->ret;
        if (!s->expr) {
          if (want != &T_VOID && want != &T_INVALID)
            err(s->loc, "E0328", "'" + to_std(current->name) + "' must return " + type_name(want));
          break;
        }
        Type t = value(s->expr, want == &T_VOID ? nullptr : want);
        if (want == &T_VOID)
          err(s->expr->loc, "E0329", "'" + to_std(current->name) + "' returns nothing, but a value is returned",
              "add '-> " + std::string(type_name(t)) + "' to its declaration");
        else if (t != &T_INVALID && want != &T_INVALID && t != want)
          err(s->expr->loc, "E0330",
              "'" + to_std(current->name) + "' returns " + type_name(want) + ", given " + type_name(t));
        break;
      }
      case StmtKind::if_:
      case StmtKind::while_: {
        Type t = value(s->expr, &T_BOOL);
        if (t != &T_INVALID && t != &T_BOOL)
          err(s->expr->loc, "E0331", std::string("condition must be bool, found ") + type_name(t),
              is_number(t) ? "compare it: x != 0" : "");
        if (s->kind == StmtKind::while_) loop_body(s->then_);
        else stmt(s->then_);
        if (s->else_) stmt(s->else_);
        break;
      }
      case StmtKind::for_range: {
        Type a = expr(s->expr), b = expr(s->expr2);
        if (is_untyped(a) && !is_untyped(b)) coerce(s->expr, b);
        else if (is_untyped(b) && !is_untyped(a)) coerce(s->expr2, a);
        else if (is_untyped(a) && is_untyped(b)) { coerce(s->expr, T_INT); coerce(s->expr2, T_INT); }
        a = s->expr->type;
        b = s->expr2->type;
        Type t = &T_INVALID;
        if (a != &T_INVALID && b != &T_INVALID) {
          if (a != b || !is_int(a))
            err(s->expr->loc, "E0364", std::string("a range needs two integers of one type, found ") + type_name(a) +
                                           " and " + type_name(b));
          else t = a;
        }
        size_t mark = locals.size();
        s->type = t;
        declare(s->name, t, s->loc, "loop variable");
        loop_body(s->then_);
        locals.resize(mark);
        break;
      }
      case StmtKind::for_each: {
        Type c = value(s->expr, nullptr);
        Type elem = &T_INVALID;
        if (c->kind == TypeKind::array || c->kind == TypeKind::view) elem = c->elem;
        else if (c == &T_STRING) elem = &T_U8;
        else if (c != &T_INVALID)
          err(s->expr->loc, "E0364", std::string("for needs a range, an array, a view or a string, found ") + type_name(c),
              is_int(c) ? "count up to it: for i: 0..<n { }" : "");
        size_t mark = locals.size();
        s->type = elem;
        declare(s->name, elem, s->loc, "loop variable");
        if (s->name2.count) declare(s->name2, T_INT, s->name2_loc, "loop variable");
        loop_body(s->then_);
        locals.resize(mark);
        break;
      }
      case StmtKind::break_:
      case StmtKind::continue_:
        if (loop_depth == 0) {
          const char *what = s->kind == StmtKind::break_ ? "break" : "continue";
          if (defer_depth)
            err(s->loc, "E0361", std::string("a deferred statement cannot ") + what + " out of its block");
          else
            err(s->loc, "E0360", std::string(what) + " is only allowed inside a loop");
        }
        break;
      case StmtKind::defer: {
        int32_t saved = loop_depth;
        loop_depth = 0;
        defer_depth++;
        stmt(s->then_);
        defer_depth--;
        loop_depth = saved;
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

  // ---- declarations ----

  bool is_builtin_name(Str name) {
    return str_eq(name, "type_name") || (has_basic && str_eq(name, "print")) || str_eq(name, "len") ||
           str_eq(name, "byte_at") || find_binary_intrinsic(name) != BinaryIntrinsic::none || find_intrinsic(name) != Intrinsic::none;
  }

  void check_global_names() {
    // One namespace for procedures, constants, structs and enums.
    struct Name { Str name; Loc loc; const char *what; };
    std::vector<Name> names;
    for (StructDecl *d : file->structs) names.push_back({d->name, d->loc, "struct"});
    for (EnumDecl *d : file->enums) names.push_back({d->name, d->loc, "enum"});
    for (size_t i = 0; i < names.size(); i++) {
      for (size_t j = 0; j < i; j++)
        if (str_eq(names[i].name, names[j].name))
          err(names[i].loc, "E0372", "type '" + to_std(names[i].name) + "' is already declared at line " +
                                         std::to_string(names[j].loc.line));
      if (find_proc(names[i].name) || find_constant(names[i].name) || is_builtin_name(names[i].name) ||
          builtin_type(names[i].name))
        err(names[i].loc, "E0372",
            std::string(names[i].what) + " '" + to_std(names[i].name) + "' conflicts with another declaration or a built-in name");
    }
  }

  void check_enum(EnumDecl *d) {
    Type base = resolve_type(d->base_expr);
    if (base != &T_INVALID && !is_int(base)) {
      err(d->base_expr->loc, "E0373", std::string("an enum's underlying type must be an integer, found ") + type_name(base));
      base = &T_INVALID;
    }
    if (base == &T_INVALID) base = T_INT;
    d->type->elem = base;
    d->type->size = base->size;
    d->type->align = base->align;
    d->type->bits = base->bits;
    d->type->is_signed = base->is_signed;
    __int128 next = 0;
    for (int32_t i = 0; i < d->members.count; i++) {
      EnumMember &m = d->members[i];
      for (int32_t j = 0; j < i; j++)
        if (str_eq(d->members[j].name, m.name))
          err(m.loc, "E0352", "duplicate member '" + to_std(m.name) + "' in enum '" + to_std(d->name) + "'");
      if (m.value) {
        std::vector<Local> saved;
        saved.swap(locals);
        Type t = expr(m.value);
        locals.swap(saved);
        __int128 v = 0;
        if (t == &T_INVALID) continue;
        if (!(is_untyped(t) || is_int(t)) || !const_int(m.value, &v)) {
          err(m.value->loc, "E0363", "an enum value must be an integer known while checking");
          continue;
        }
        next = v;
      }
      if (!fits(base, next)) {
        err(m.loc, "E0363", "value " + int128_text(next) + " of '" + to_std(m.name) + "' does not fit in " +
                               type_name(base));
        next = 0;
      }
      m.bits = canon(base, (uint64_t)next);
      for (int32_t j = 0; j < i; j++)
        if (d->members[j].bits == m.bits)
          err(m.loc, "E0363", "'" + to_std(m.name) + "' has the same value as '" + to_std(d->members[j].name) + "'",
              "give it its own value");
      next++;
    }
  }

  void check_field_defaults(StructDecl *d) {
    for (Field &f : d->fields) {
      if (!f.init || f.type == &T_INVALID) continue;
      // Defaults are constants: checked with no locals, evaluated before use.
      std::vector<Local> saved;
      saved.swap(locals);
      Type t = value(f.init, f.type);
      locals.swap(saved);
      if (t != &T_INVALID && t != f.type)
        err(f.init->loc, "E0367", "field '" + to_std(f.name) + "' is " + type_name(f.type) + ", its default is " +
                                      type_name(t));
    }
  }

  // #foreign: the host provides a C symbol with this exact name. Only
  // integers, floats and bool cross; there is no pointer or ownership ABI.
  void check_foreign(ProcDecl *p) {
    auto scalar = [](Type t) { return t == &T_INVALID || is_number(t) || t == &T_BOOL; };
    std::string name = to_std(p->name);
    if (!scalar(p->ret) && p->ret != &T_VOID)
      err(p->loc, "E0376", "'" + name + "' returns " + type_name(p->ret) + "; #foreign procedures pass integers, floats and bool");
    for (const Param &prm : p->params)
      if (!scalar(prm.type))
        err(prm.loc, "E0376", "parameter '" + to_std(prm.name) + "' is " + type_name(prm.type) +
                                  "; #foreign procedures pass integers, floats and bool");
    if (is_c_keyword(p->name) || str_eq(p->name, "main"))
      err(p->loc, "E0376", "'" + name + "' cannot be a C symbol name");
    else if (p->name.count >= 5 && memcmp(p->name.data, "coax_", 5) == 0)
      err(p->loc, "E0376", "foreign names cannot start with 'coax_', which the runtime uses");
  }

  // ---- The Metal scalar profile (coax metal) ----
  // f32 and bool values, straight-line code and acyclic calls: what a
  // shader can run without the C runtime. Stage binding stays native.

  bool metal_type(Type t) { return t == &T_F32 || t == &T_BOOL || t == &T_VOID || t == &T_INVALID; }

  void metal_fail(Loc loc, const std::string &what) {
    err(loc, "E0377", what + " is outside the Metal scalar profile",
        "Metal libraries use f32 and bool, if/else and calls; no loops, strings, integers or host calls");
  }

  void metal_expr(const Expr *e, std::vector<const ProcDecl *> &path) {
    if (!e) return;
    if (!metal_type(e->type)) return metal_fail(e->loc, std::string("a ") + type_name(e->type) + " value");
    if (e->builtin_print) return metal_fail(e->loc, "print");
    if (e->binary_intrinsic != BinaryIntrinsic::none) return metal_fail(e->loc, "a binary storage intrinsic");
    if (e->kind == ExprKind::call && e->proc) {
      if (e->proc->foreign) return metal_fail(e->loc, "a #foreign call");
      for (const ProcDecl *q : path)
        if (q == e->proc) return metal_fail(e->loc, "a recursive call");
      metal_proc(e->proc, path);
    }
    metal_expr(e->lhs, path);
    metal_expr(e->rhs, path);
    metal_expr(e->third, path);
    for (const Expr *a : e->args) metal_expr(a, path);
  }

  void metal_stmt(const Stmt *s, std::vector<const ProcDecl *> &path) {
    if (!s) return;
    if (s->kind == StmtKind::while_ || s->kind == StmtKind::for_range || s->kind == StmtKind::for_each)
      return metal_fail(s->loc, "a loop");
    if (s->kind == StmtKind::var && !metal_type(s->type)) return metal_fail(s->loc, std::string("a ") + type_name(s->type) + " local");
    metal_expr(s->expr, path);
    metal_expr(s->target, path);
    for (const Stmt *c : s->body) metal_stmt(c, path);
    metal_stmt(s->then_, path);
    metal_stmt(s->else_, path);
  }

  std::vector<const ProcDecl *> metal_done;

  void metal_proc(const ProcDecl *p, std::vector<const ProcDecl *> &path) {
    for (const ProcDecl *q : metal_done)
      if (q == p) return;
    if (path.size() >= 64) return metal_fail(p->loc, "a call chain deeper than 64");
    path.push_back(p);
    metal_stmt(p->body, path);
    path.pop_back();
    metal_done.push_back(p);
  }

  void check_metal() {
    for (const ProcDecl *p : file->procs) {
      if (p->foreign) { metal_fail(p->loc, "a #foreign procedure"); continue; }
      if (!metal_type(p->ret)) metal_fail(p->loc, std::string("returning ") + type_name(p->ret));
      for (const Param &prm : p->params)
        if (!metal_type(prm.type)) metal_fail(prm.loc, std::string("a ") + type_name(prm.type) + " parameter");
    }
    for (const ProcDecl *p : file->procs) {
      std::vector<const ProcDecl *> path;
      if (!p->foreign) metal_proc(p, path);
    }
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
    for (StructDecl *d : file->structs) {
      d->type = new TypeInfo();
      d->type->kind = TypeKind::struct_;
      d->type->name = to_std(d->name);
      d->type->struct_decl = d;
    }
    for (EnumDecl *d : file->enums) {
      d->type = new TypeInfo();
      d->type->kind = TypeKind::enum_;
      d->type->name = to_std(d->name);
      d->type->enum_decl = d;
    }
    check_global_names();
    for (EnumDecl *d : file->enums) check_enum(d);
    for (StructDecl *d : file->structs) layout(d);
    bool any_export = false;
    for (int32_t i = 0; i < file->procs.count; i++) {
      ProcDecl *p = file->procs[i];
      for (int32_t j = 0; j < i; j++)
        if (str_eq(file->procs[j]->name, p->name))
          err(p->loc, "E0334",
              "procedure '" + to_std(p->name) + "' is already declared at line " +
                  std::to_string(file->procs[j]->loc.line));
      if (is_builtin_name(p->name))
        err(p->loc, "E0335", "'" + to_std(p->name) + "' is already a builtin", "pick another name");
      for (Param &prm : p->params) prm.type = resolve_type(prm.type_expr);
      if (p->ret_expr) p->ret = resolve_type(p->ret_expr);
      if (p->foreign) check_foreign(p);
      if (p->export_c) {
        any_export = true;
        if (str_eq(p->name, "main"))
          err(p->loc, "E0369", "'main' cannot be exported; it is the program's entry point");
        else if (p->name.count >= 5 && memcmp(p->name.data, "coax_", 5) == 0)
          err(p->loc, "E0369", "exported names cannot start with 'coax_', which the runtime uses");
      }
    }
    for (int32_t i = 0; i < file->constants.count; i++) {
      ConstDecl *c = file->constants[i];
      for (int32_t j = 0; j < i; j++)
        if (str_eq(c->name, file->constants[j]->name))
          err(c->loc, "E0345", "duplicate constant '" + to_std(c->name) + "'");
      if (find_proc(c->name) || is_builtin_name(c->name))
        err(c->loc, "E0345", "constant '" + to_std(c->name) + "' conflicts with a procedure or intrinsic");
    }
    for (ConstDecl *c : file->constants) constant_type(c);
    for (StructDecl *d : file->structs) check_field_defaults(d);
    for (Expr *a : file->assertions) {
      Type t = value(a, &T_BOOL);
      if (t != &T_INVALID && t != &T_BOOL) err(a->loc, "E0346", "#assert requires bool");
    }
    ProcDecl *entry = find_proc(Str{"main", 4});
    if (!entry) {
      if (!file->library)
        err(Loc{1, 1, 0}, "E0336", "the program has no 'main'",
            any_export ? "build it as a library: coax lib FILE.coax" : "add main :: () { ... }");
    } else if (entry->params.count || entry->ret != &T_VOID || entry->foreign) {
      err(entry->loc, "E0337", "'main' takes no parameters and returns nothing in coax0", "main :: () { ... }");
    }
    for (ProcDecl *p : file->procs) {
      if (p->foreign) continue;
      current = p;
      locals.clear();
      for (const Param &prm : p->params) declare(prm.name, prm.type, prm.loc, "parameter");
      stmt(p->body);
      if (p->ret != &T_VOID && p->ret != &T_INVALID && !always_returns(p->body))
        err(p->loc, "E0338", "'" + to_std(p->name) + "' can reach its end without returning " + type_name(p->ret),
            "add a return at the end");
    }
    if (file->metal && g_error_count == before) check_metal();
    return g_error_count == before;
  }
};

}  // namespace

bool check(Arena *arena, File *file) {
  Checker c{arena, file, file->src};
  return c.run();
}

}  // namespace coax
