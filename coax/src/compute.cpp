// `coax cuda` admission: native code observes the checked tree; the decisions
// are lib/compiler/compute.coax (the CU_ procedures). Build-only stage0 has no
// compute policy and refuses the command.
#include "coax.h"

namespace coax {

// The versioned type-code bridge described at the top of compute.coax.
static uint32_t compute_code(Type t) {
  switch (t->kind) {
    case TypeKind::void_: return 0;
    case TypeKind::bool_: return 1;
    case TypeKind::floating: return t->bits == 32 ? 2 : 3;
    case TypeKind::untyped_float: return 3;
    case TypeKind::untyped_int: return 7;
    case TypeKind::integer: {
      uint32_t base = t->is_signed ? 4 : 8;
      return base + (t->bits == 8 ? 0 : t->bits == 16 ? 1 : t->bits == 32 ? 2 : 3);
    }
    case TypeKind::string_: return 12;
    case TypeKind::struct_: return 13;
    case TypeKind::enum_: return 14;
    case TypeKind::array: return 15;
    case TypeKind::view: return 16;
    default: return 17;
  }
}

#ifndef COAX_BOOTSTRAP_IMPORTER
static const Expr *constant_of(const Expr *e) {
  if (e && e->kind == ExprKind::ident && e->constant && e->constant->expr) e = e->constant->expr;
  return e && e->kind == ExprKind::int_lit ? e : nullptr;
}
static bool mentions(const Expr *e, const char *name) {
  if (!e) return false;
  if (e->kind == ExprKind::ident && !e->constant && str_eq(e->text, name)) return true;
  if (mentions(e->lhs, name) || mentions(e->rhs, name) || mentions(e->third, name)) return true;
  for (const Expr *a : e->args) if (mentions(a, name)) return true;
  return false;
}
// A lane loop is exactly `for lane: 0..<32` over constants; a loop named lane
// with other bounds is an ordinary loop.
static bool is_lane_loop(const Stmt *s) {
  if (s->kind != StmtKind::for_range || !str_eq(s->name, "lane")) return false;
  const Expr *lo = constant_of(s->expr), *hi = constant_of(s->expr2);
  return lo && hi && lo->int_value == 0 && hi->int_value == 32;
}

// Warp kernels: lane loops, per-lane arrays and device primitives, checked
// against the CU_lane_* and CU_primitive_* policy and recorded in g_warp.
struct WarpAdmission {
  const File *file;
  bool ok = true;
  std::map<std::string, const Stmt *> arrays;  // local fixed arrays by name
  std::set<std::string> lane_arrays, inner;    // per-lane arrays; locals declared in the current lane loop
  std::map<std::string, std::pair<int64_t, int64_t>> ranges;  // constant-bound loop counters: [lo, hi)
  void refuse(Loc loc, uint32_t reason) {
    coax_string text = CU_reason_text(reason), fix = CU_fix();
    error_at(file->src, loc, "E0700", std::string(text.data, (size_t)text.count) + " is outside the CUDA compute subset",
             std::string(fix.data, (size_t)fix.count));
    ok = false;
  }
  const Stmt *array_of(const Expr *e) {
    if (!e || e->kind != ExprKind::ident || e->constant) return nullptr;
    auto at = arrays.find(to_std(e->text));
    return at == arrays.end() ? nullptr : at->second;
  }
  // Pass 1: which arrays are per-lane.
  void mark_expr(const Expr *e, bool in_lane) {
    if (!e) return;
    if (e->kind == ExprKind::index && in_lane && array_of(e->lhs) && mentions(e->rhs, "lane")) lane_arrays.insert(to_std(e->lhs->text));
    if (e->kind == ExprKind::call && is_mma_primitive(e->proc))
      for (const Expr *a : e->args) if (a->kind == ExprKind::slice && array_of(a->lhs)) lane_arrays.insert(to_std(a->lhs->text));
    mark_expr(e->lhs, in_lane); mark_expr(e->rhs, in_lane); mark_expr(e->third, in_lane);
    for (const Expr *a : e->args) mark_expr(a, in_lane);
  }
  void mark(const Stmt *s, bool in_lane) {
    if (!s) return;
    if (s->kind == StmtKind::var && s->type->kind == TypeKind::array) arrays[to_std(s->name)] = s;
    bool lane = in_lane || is_lane_loop(s);
    mark_expr(s->expr, lane); mark_expr(s->expr2, lane); mark_expr(s->target, lane);
    for (const Stmt *c : s->body) mark(c, lane);
    mark(s->then_, lane); mark(s->else_, lane);
  }
  // The range [lo, hi] of an offset built from constants and the counters
  // of constant-bound loops with + and *, all non-negative.
  bool span(const Expr *e, int64_t *lo, int64_t *hi) {
    if (const Expr *c = constant_of(e)) { *lo = *hi = (int64_t)c->int_value; return *lo >= 0; }
    if (e->kind == ExprKind::ident && !e->constant && ranges.count(to_std(e->text))) {
      auto r = ranges[to_std(e->text)];
      *lo = r.first; *hi = r.second - 1;
      return *lo >= 0 && *hi >= *lo;
    }
    if (e->kind == ExprKind::binary && (e->op == Tok::plus || e->op == Tok::star)) {
      int64_t a, b, c, d;
      if (!span(e->lhs, &a, &b) || !span(e->rhs, &c, &d) || b > 4096 || d > 4096) return false;
      if (e->op == Tok::plus) { *lo = a + c; *hi = b + d; } else { *lo = a * c; *hi = b * d; }
      return true;
    }
    return false;
  }
  // A sum with exactly one term lane*k (or k*lane); the other terms make an
  // offset whose range is [lo, hi]. Returns the scale k.
  void terms(const Expr *e, std::vector<const Expr *> *out) {
    if (e->kind == ExprKind::binary && e->op == Tok::plus) { terms(e->lhs, out); terms(e->rhs, out); }
    else out->push_back(e);
  }
  bool form(const Expr *e, int64_t *k, int64_t *lo, int64_t *hi) {
    std::vector<const Expr *> parts;
    terms(e, &parts);
    *lo = *hi = 0;
    bool found = false;
    for (const Expr *t : parts) {
      if (mentions(t, "lane")) {
        if (found || t->kind != ExprKind::binary || t->op != Tok::star) return false;
        const Expr *x = t->lhs, *y = t->rhs;
        if (constant_of(x)) std::swap(x, y);
        if (!constant_of(y) || x->kind != ExprKind::ident || x->constant || !str_eq(x->text, "lane")) return false;
        *k = (int64_t)constant_of(y)->int_value;
        found = true;
        continue;
      }
      int64_t a, b;
      if (!span(t, &a, &b)) return false;
      *lo += a; *hi += b;
    }
    return found;
  }
  // Pass 2: every use of a per-lane array, every assignment in a lane loop.
  void check_expr(const Expr *e, bool in_lane, bool primitive_arg) {
    if (!e) return;
    if (e->kind == ExprKind::index && array_of(e->lhs) && lane_arrays.count(to_std(e->lhs->text))) {
      uint32_t r = CU_lane_use(in_lane, true, false);
      if (r) { refuse(e->loc, r); return; }
      int64_t k = 0, lo = 0, hi = 0;
      bool good = form(e->rhs, &k, &lo, &hi);
      int64_t length = array_of(e->lhs)->type->length;
      r = CU_lane_index(length, k, lo, good);
      if (!r) r = CU_lane_index(length, k, hi, good);
      if (r) { refuse(e->loc, r); return; }
      g_warp.lane_index[e] = k;
      return;
    }
    if (e->kind == ExprKind::slice && array_of(e->lhs) && lane_arrays.count(to_std(e->lhs->text))) {
      uint32_t r = CU_lane_use(in_lane, false, primitive_arg && !e->rhs);
      if (r) refuse(e->loc, r);
      return;
    }
    if (e->kind == ExprKind::ident && array_of(e) && lane_arrays.count(to_std(e->text))) { refuse(e->loc, CU_lane_use(in_lane, false, false)); return; }
    if (e->kind == ExprKind::call && is_mma_primitive(e->proc)) {
      if (in_lane) { refuse(e->loc, CU_lane_use(true, false, true)); return; }
      bool shaped = e->args.count == 3;
      int64_t len[3] = {0, 0, 0};
      uint32_t code[3] = {17, 17, 17};
      for (int i = 0; shaped && i < 3; i++) {
        const Expr *a = e->args[i];
        const Stmt *d = a->kind == ExprKind::slice && !a->rhs ? array_of(a->lhs) : nullptr;
        if (!d) { shaped = false; break; }
        len[i] = d->type->length;
        code[i] = compute_code(d->type->elem);
      }
      uint32_t r = shaped ? CU_primitive_mma(len[0], len[1], len[2], code[0], code[1], code[2]) : 27;
      if (r) { refuse(e->loc, r); return; }
      g_warp.mma_calls.insert(e);
      for (const Expr *a : e->args) check_expr(a, in_lane, true);
      return;
    }
    check_expr(e->lhs, in_lane, false); check_expr(e->rhs, in_lane, false); check_expr(e->third, in_lane, false);
    for (const Expr *a : e->args) check_expr(a, in_lane, false);
  }
  void check(const Stmt *s, bool in_lane, bool direct) {
    if (!s) return;
    if (is_lane_loop(s)) {
      uint32_t r = CU_lane_loop(true, true, 0, 32, in_lane);
      if (r) { refuse(s->loc, r); return; }
      g_warp.lane_loops.insert(s);
      std::set<std::string> saved = inner;
      inner.clear();
      inner.insert("lane");
      check(s->then_, true, true);
      inner = saved;
      return;
    }
    if (s->kind == StmtKind::for_range) {
      const Expr *lo = constant_of(s->expr), *hi = constant_of(s->expr2);
      if (lo && hi) ranges[to_std(s->name)] = {(int64_t)lo->int_value, (int64_t)hi->int_value};
    }
    if (in_lane) {
      if (s->kind == StmtKind::var || s->kind == StmtKind::for_range || s->kind == StmtKind::for_each) {
        inner.insert(to_std(s->name));
        if (s->name2.count) inner.insert(to_std(s->name2));
      }
      if ((s->kind == StmtKind::break_ || s->kind == StmtKind::continue_) && CU_lane_jump(direct)) { refuse(s->loc, CU_lane_jump(direct)); return; }
      if (s->kind == StmtKind::assign) {
        const Expr *t = s->target;
        while (t->kind == ExprKind::index || t->kind == ExprKind::field) {
          if (t->kind == ExprKind::index && t->lhs->type->kind == TypeKind::view) break;  // a store through a view
          if (t->kind == ExprKind::index && array_of(t->lhs) && lane_arrays.count(to_std(t->lhs->text))) break;
          t = t->lhs;
        }
        bool view_store = t->kind == ExprKind::index && t->lhs->type->kind == TypeKind::view;
        bool lane_store = t->kind == ExprKind::index && !view_store;
        bool outer = !view_store && !lane_store && t->kind == ExprKind::ident && !inner.count(to_std(t->text));
        uint32_t r = CU_lane_assign(outer, lane_store);
        if (r) { refuse(s->loc, r); return; }
      }
      if (s->kind == StmtKind::ret) { refuse(s->loc, CU_lane_jump(true)); return; }
    }
    check_expr(s->expr, in_lane, false); check_expr(s->expr2, in_lane, false); check_expr(s->target, in_lane, false);
    bool loop = s->kind == StmtKind::for_range || s->kind == StmtKind::for_each || s->kind == StmtKind::while_;
    for (const Stmt *c : s->body) check(c, in_lane, direct && !loop);
    check(s->then_, in_lane, direct && !loop);
    check(s->else_, in_lane, direct && !loop);
  }
  void kernel(const ProcDecl *p) {
    arrays.clear(); lane_arrays.clear(); inner.clear(); ranges.clear();
    mark(p->body, false);
    size_t before = g_warp.lane_loops.size();
    check(p->body, false, false);
    if (g_warp.lane_loops.size() > before) g_warp.warp_kernels.insert(p);
    for (const auto &[name, d] : arrays) if (lane_arrays.count(name)) g_warp.lane_arrays.insert(d);
  }
  // Lane loops belong in kernel bodies only.
  void helper(const Stmt *s) {
    if (!s) return;
    if (is_lane_loop(s)) { refuse(s->loc, CU_lane_loop(false, true, 0, 32, false)); return; }
    for (const Stmt *c : s->body) helper(c);
    helper(s->then_); helper(s->else_);
  }
};

struct ComputeAdmission {
  const File *file;
  std::vector<const ProcDecl *> done, active;
  bool ok = true;

  void refuse(Loc loc, uint32_t reason) {
    coax_string text = CU_reason_text(reason), fix = CU_fix();
    error_at(file->src, loc, "E0700", std::string(text.data, (size_t)text.count) + " is outside the CUDA compute subset",
             std::string(fix.data, (size_t)fix.count));
    ok = false;
  }
  bool type(Loc loc, Type t) {
    uint32_t elem = t->elem ? compute_code(t->elem) : 17;
    uint32_t r = CU_value(compute_code(t), elem, t->kind == TypeKind::array ? t->length : 0);
    if (r) refuse(loc, r);
    return r == 0;
  }
  void expr(const Expr *e) {
    if (!e) return;
    if (e->kind == ExprKind::call) {
      bool is_active = false;
      for (const ProcDecl *p : active) is_active = is_active || p == e->proc;
      uint32_t r = CU_call(e->builtin_print, e->proc && e->proc->foreign, is_active, (int64_t)active.size());
      if (r) { refuse(e->loc, r); return; }
      if (is_mma_primitive(e->proc)) {  // a device primitive: its body is its CPU meaning, not device code
        for (const Expr *a : e->args) expr(a);
        return;
      }
      if (e->byte_intrinsic != ByteIntrinsic::none) {
        r = CU_byte_intrinsic((uint32_t)e->byte_intrinsic);
        if (r) { refuse(e->loc, r); return; }
      }
    }
    if (e->kind == ExprKind::ct_value) {
      uint32_t r = CU_constant(compute_code(e->type));
      if (r) { refuse(e->loc, r); return; }
    }
    if (e->type->kind != TypeKind::void_ && !type(e->loc, e->type)) return;
    expr(e->lhs); expr(e->rhs); expr(e->third);
    for (const Expr *a : e->args) expr(a);
    for (const FieldInit &f : e->fields) expr(f.value);
    if (e->kind == ExprKind::call && e->proc) proc(e->proc);
  }
  void stmt(const Stmt *s) {
    if (!s) return;
    uint32_t r = CU_statement((uint32_t)s->kind);
    if (r) { refuse(s->loc, r); return; }
    if ((s->kind == StmtKind::var || s->kind == StmtKind::for_range || s->kind == StmtKind::for_each) && !type(s->loc, s->type)) return;
    expr(s->expr); expr(s->expr2); expr(s->target);
    for (const Stmt *c : s->body) stmt(c);
    stmt(s->then_); stmt(s->else_);
  }
  void proc(const ProcDecl *p) {
    for (const ProcDecl *q : done) if (q == p) return;
    done.push_back(p);
    uint32_t r = CU_procedure(p->foreign);
    if (r) { refuse(p->loc, r); return; }
    if (p->export_c) {
      r = CU_kernel(compute_code(p->ret), p->params.count, p->params.count ? compute_code(p->params[0].type) : 17);
      if (r) { refuse(p->loc, r); return; }
      for (int32_t i = 1; i < p->params.count; i++) {
        Type t = p->params[i].type;
        r = CU_kernel_parameter(compute_code(t), t->elem ? compute_code(t->elem) : 17);
        if (r) refuse(p->params[i].loc, r);
      }
    } else {
      r = CU_result(compute_code(p->ret));
      if (r) refuse(p->loc, r);
      for (const Param &prm : p->params) type(prm.loc, prm.type);
    }
    active.push_back(p);
    stmt(p->body);
    active.pop_back();
  }
};
#endif

// Every kernel and everything it reaches must stay inside the subset.
bool compute_admit(const File *file) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)file; (void)compute_code;
  fprintf(stderr, "coax: coax cuda requires the final compiler, not build-only stage0\n");
  return false;
#else
  ComputeAdmission a{file};
  WarpAdmission warp{file};
  g_warp = WarpInfo{};
  bool any = false;
  for (const ProcDecl *p : file->procs) {
    if (!p->export_c) continue;
    any = true;
    a.proc(p);
    warp.kernel(p);
  }
  for (const ProcDecl *p : a.done) if (!p->export_c) warp.helper(p->body);
  a.ok = a.ok && warp.ok;
  if (!any) {
    error_at(file->src, {1, 1, 0}, "E0701", "a CUDA module needs at least one #export_c kernel",
             "Mark each kernel #export_c; its first parameter is the int work item and it returns nothing.");
    return false;
  }
  return a.ok;
#endif
}

}  // namespace coax
