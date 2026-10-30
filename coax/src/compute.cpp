// `coax cuda` and `coax metal-compute` admission: native code observes the
// checked tree; the decisions are lib/compiler/compute.coax (the CU_ and, for
// Metal, MT_ procedures). Build-only stage0 has no compute policy and refuses
// both commands.
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

// One refusal, worded for the target: E0700 for CUDA, E0702 for Metal.
static void compute_refuse(const File *file, Loc loc, uint32_t reason, bool metal) {
  coax_string text = CU_reason_text(reason), fix = metal ? MT_fix() : CU_fix();
  error_at(file->src, loc, metal ? "E0702" : "E0700",
           std::string(text.data, (size_t)text.count) + (metal ? " is outside the Metal compute subset" : " is outside the CUDA compute subset"),
           std::string(fix.data, (size_t)fix.count));
}

// Warp kernels: lane loops, per-lane arrays and device primitives, checked
// against the CU_lane_* and CU_primitive_* policy and recorded in g_warp.
struct WarpAdmission {
  const File *file;
  bool metal = false;
  bool ok = true;
  std::map<std::string, const Stmt *> arrays;  // local fixed arrays by name
  std::set<std::string> lane_arrays, inner;    // per-lane arrays; locals declared in the current lane loop
  std::map<std::string, std::pair<int64_t, int64_t>> ranges;  // constant-bound loop counters: [lo, hi)
  void refuse(Loc loc, uint32_t reason) {
    compute_refuse(file, loc, reason, metal);
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

// Group kernels: thread loops, per-thread and threadgroup arrays, matrix
// accumulators and the Apple matrix primitives, checked against the CU_thread_*,
// CU_shared_*, CU_group_* and CU_apple_* policy and recorded in g_warp.
static std::vector<std::pair<const Expr *, int64_t>> g_group_index;  // a thread-loop index of an outer array and its k
static int64_t elem_bytes(Type t) { return t->kind == TypeKind::bool_ ? 1 : t->bits / 8; }
static bool is_thread_loop(const Stmt *s) {
  return s->kind == StmtKind::for_range && str_eq(s->name, "thread") && constant_of(s->expr) && constant_of(s->expr2);
}
struct GroupAdmission {
  const File *file;
  bool metal = false;
  bool ok = true;
  void refuse(Loc loc, uint32_t reason) {
    compute_refuse(file, loc, reason, metal);
    ok = false;
  }
  // Per kernel.
  const ProcDecl *kernel = nullptr;
  int64_t size = 0;          // the group size T, 0 until a thread loop or group primitive names it
  bool sized_ok = true;
  struct ArrayUse { const Stmt *decl; bool top; bool in_thread = false, own = true, elsewhere = false, accumulator = false, other = false; int64_t k = 0; };
  std::map<std::string, ArrayUse> arrays;   // fixed arrays declared in shared code, by name
  std::map<std::string, std::pair<int64_t, int64_t>> ranges;  // constant-bound loop counters
  std::set<std::string> inner;              // locals declared inside the current thread loop
  bool any_thread = false;

  static int64_t value_of(const Expr *e) { const Expr *c = constant_of(e); return c ? (int64_t)c->int_value : -1; }
  void name_size(Loc loc, int64_t t) {
    uint32_t r = CU_group_size(t, size, false, false);
    if (r) { refuse(loc, r); sized_ok = false; return; }
    size = t;
  }
  bool span(const Expr *e, int64_t *lo, int64_t *hi) {
    if (const Expr *c = constant_of(e)) { *lo = *hi = (int64_t)c->int_value; return *lo >= 0; }
    if (e->kind == ExprKind::ident && !e->constant && ranges.count(to_std(e->text))) {
      auto r = ranges[to_std(e->text)];
      *lo = r.first; *hi = r.second - 1;
      return *lo >= 0 && *hi >= *lo;
    }
    if (e->kind == ExprKind::binary && (e->op == Tok::plus || e->op == Tok::star)) {
      int64_t a, b, c, d;
      if (!span(e->lhs, &a, &b) || !span(e->rhs, &c, &d) || b > (1 << 20) || d > (1 << 20)) return false;
      if (e->op == Tok::plus) { *lo = a + c; *hi = b + d; } else { *lo = a * c; *hi = b * d; }
      return true;
    }
    return false;
  }
  void terms(const Expr *e, std::vector<const Expr *> *out) {
    if (e->kind == ExprKind::binary && e->op == Tok::plus) { terms(e->lhs, out); terms(e->rhs, out); }
    else out->push_back(e);
  }
  // index = thread*k + c + sum of m*x with m a constant multiple of T*k and
  // x >= 0. Returns whether the form holds; k and c's range [lo, hi] out.
  bool owned(const Expr *e, int64_t *k, int64_t *lo, int64_t *hi, bool *stride, bool *strided = nullptr) {
    std::vector<const Expr *> parts;
    terms(e, &parts);
    *k = 0; *lo = *hi = 0; *stride = true;
    if (strided) *strided = false;
    const Expr *thread = nullptr;
    for (const Expr *t : parts) {
      if (!mentions(t, "thread")) continue;
      if (thread) return false;
      thread = t;
      if (t->kind == ExprKind::ident && str_eq(t->text, "thread")) { *k = 1; continue; }
      if (t->kind != ExprKind::binary || t->op != Tok::star) return false;
      const Expr *x = t->lhs, *y = t->rhs;
      if (constant_of(x)) std::swap(x, y);
      if (!constant_of(y) || x->kind != ExprKind::ident || x->constant || !str_eq(x->text, "thread")) return false;
      *k = (int64_t)constant_of(y)->int_value;
    }
    if (!thread || *k < 1) return false;
    for (const Expr *t : parts) {
      if (t == thread) continue;
      if (t->kind == ExprKind::binary && t->op == Tok::star && size > 0) {
        const Expr *x = t->lhs, *y = t->rhs;
        if (constant_of(x)) std::swap(x, y);
        int64_t m = constant_of(y) ? (int64_t)constant_of(y)->int_value : 0, a, b;
        if (m > 0 && m % (size * *k) == 0) {
          if (!span(x, &a, &b)) *stride = false;
          if (strided) *strided = true;
          continue;
        }
      }
      int64_t a, b;
      if (!span(t, &a, &b)) return false;
      *lo += a; *hi += b;
    }
    return true;
  }
  ArrayUse *array_of(const Expr *e) {
    if (!e || e->kind != ExprKind::ident || e->constant) return nullptr;
    auto at = arrays.find(to_std(e->text));
    if (at == arrays.end() || inner.count(to_std(e->text))) return nullptr;
    return &at->second;
  }
  static int apple_kind(const ProcDecl *p) {
    if (!p) return 0;
    if (str_eq(p->name, "am_sg_mma_64x32x32")) return 1;
    if (str_eq(p->name, "am_sg_spill_64x32")) return 2;
    if (str_eq(p->name, "am_matmul_bf16")) return 3;
    return 0;
  }
  // Pass 1: arrays, their uses and the group size.
  void declare(const Stmt *s, bool top, bool in_thread) {
    if (s->kind == StmtKind::var && s->type->kind == TypeKind::array && !in_thread)
      arrays[to_std(s->name)] = ArrayUse{s, top};
  }
  void scan_expr(const Expr *e, bool in_thread, const Stmt *loop) {
    if (!e) return;
    if (e->kind == ExprKind::call && apple_kind(e->proc)) {
      int kind = apple_kind(e->proc);
      if (kind == 3) { int64_t t = e->args.count ? value_of(e->args[0]) : -1; if (t > 0) name_size(e->loc, t); }
      else name_size(e->loc, 128);
      for (int32_t i = 0; i < e->args.count; i++) {
        const Expr *a = e->args[i];
        ArrayUse *u = a->kind == ExprKind::slice && !a->rhs ? array_of(a->lhs) : nullptr;
        if (u && kind != 3 && i == 0) { u->accumulator = true; continue; }
        if (u) { u->elsewhere = true; continue; }
        scan_expr(a, in_thread, loop);
      }
      return;
    }
    if (e->kind == ExprKind::index) {
      if (ArrayUse *u = array_of(e->lhs)) {
        if (in_thread) {
          u->in_thread = true;
          int64_t k, lo, hi; bool stride, strided;
          bool form = owned(e->rhs, &k, &lo, &hi, &stride, &strided);
          // Per-thread registers: thread*k+c with c in [0, k) into T*k values, no strides.
          bool own = form && stride && !strided && size > 0 && u->decl->type->length == size * k && lo >= 0 && hi < k;
          if (!own || (u->k && u->k != k)) u->own = false;
          u->k = k;
          g_group_index.push_back({e, k});
        } else u->elsewhere = true;
        scan_expr(e->rhs, in_thread, loop);
        return;
      }
    }
    if (ArrayUse *u = array_of(e)) { u->elsewhere = true; u->other = true; return; }
    if (e->kind == ExprKind::slice) if (ArrayUse *u = array_of(e->lhs)) { u->elsewhere = true; u->other = true; scan_expr(e->rhs, in_thread, loop); scan_expr(e->third, in_thread, loop); return; }
    scan_expr(e->lhs, in_thread, loop); scan_expr(e->rhs, in_thread, loop); scan_expr(e->third, in_thread, loop);
    for (const Expr *a : e->args) scan_expr(a, in_thread, loop);
  }
  void scan(const Stmt *s, bool top, bool in_thread, const Stmt *loop, bool nested_lane) {
    if (!s) return;
    if (is_thread_loop(s)) {
      any_thread = true;
      int64_t lo = (int64_t)constant_of(s->expr)->int_value, hi = (int64_t)constant_of(s->expr2)->int_value;
      uint32_t r = CU_thread_loop(true, true, lo, hi, in_thread || nested_lane);
      if (r) { refuse(s->loc, r); return; }
      name_size(s->loc, hi);
      std::set<std::string> saved = inner;
      scan(s->then_, false, true, s, nested_lane);
      inner = saved;
      return;
    }
    if (s->kind == StmtKind::for_range && str_eq(s->name, "thread")) { refuse(s->loc, CU_thread_loop(true, false, 0, 0, false)); return; }
    bool lane = is_lane_loop(s);
    if (s->kind == StmtKind::for_range) {
      const Expr *lo = constant_of(s->expr), *hi = constant_of(s->expr2);
      if (lo && hi) ranges[to_std(s->name)] = {(int64_t)lo->int_value, (int64_t)hi->int_value};
    }
    if (in_thread && (s->kind == StmtKind::var || s->kind == StmtKind::for_range || s->kind == StmtKind::for_each)) {
      inner.insert(to_std(s->name));
      if (s->name2.count) inner.insert(to_std(s->name2));
    }
    declare(s, top, in_thread);
    scan_expr(s->expr, in_thread, loop); scan_expr(s->expr2, in_thread, loop); scan_expr(s->target, in_thread, loop);
    for (const Stmt *c : s->body) scan(c, top && s->kind == StmtKind::block && s == kernel->body, in_thread, loop, nested_lane || lane);
    scan(s->then_, false, in_thread, loop, nested_lane || lane);
    scan(s->else_, false, in_thread, loop, nested_lane || lane);
  }
  // Pass 2: phases. Which threadgroup arrays a thread loop writes, then
  // whether its every access to them is owned; assignments and jumps.
  std::set<std::string> shared;
  void writes_in(const Stmt *s, std::set<std::string> *out) {
    if (!s) return;
    if (s->kind == StmtKind::assign) {
      const Expr *t = s->target;
      if (t->kind == ExprKind::index && t->lhs->kind == ExprKind::ident && shared.count(to_std(t->lhs->text)) && !inner.count(to_std(t->lhs->text)))
        out->insert(to_std(t->lhs->text));
    }
    for (const Stmt *c : s->body) writes_in(c, out);
    writes_in(s->then_, out); writes_in(s->else_, out);
  }
  void access_expr(const Expr *e, const std::set<std::string> &written) {
    if (!e) return;
    if (e->kind == ExprKind::index && e->lhs->kind == ExprKind::ident && written.count(to_std(e->lhs->text)) && !inner.count(to_std(e->lhs->text))) {
      int64_t k, lo, hi; bool stride;
      bool form = owned(e->rhs, &k, &lo, &hi, &stride);
      uint32_t r = CU_shared_access(true, CU_owned_index(form, k, lo, hi, stride));
      if (r) { refuse(e->loc, r); return; }
    }
    access_expr(e->lhs, written); access_expr(e->rhs, written); access_expr(e->third, written);
    for (const Expr *a : e->args) access_expr(a, written);
  }
  void phase(const Stmt *s, const std::set<std::string> &written, bool direct) {
    if (!s) return;
    if (s->kind == StmtKind::for_range) {
      const Expr *lo = constant_of(s->expr), *hi = constant_of(s->expr2);
      if (lo && hi) ranges[to_std(s->name)] = {(int64_t)lo->int_value, (int64_t)hi->int_value};
    }
    if (s->kind == StmtKind::var || s->kind == StmtKind::for_range || s->kind == StmtKind::for_each) {
      inner.insert(to_std(s->name));
      if (s->name2.count) inner.insert(to_std(s->name2));
    }
    if ((s->kind == StmtKind::break_ || s->kind == StmtKind::continue_) && direct) { refuse(s->loc, CU_thread_jump(true)); return; }
    if (s->kind == StmtKind::ret) { refuse(s->loc, CU_thread_jump(true)); return; }
    if (s->kind == StmtKind::assign) {
      const Expr *t = s->target;
      while (t->kind == ExprKind::index || t->kind == ExprKind::field) {
        if (t->kind == ExprKind::index && t->lhs->type->kind == TypeKind::view) break;
        t = t->lhs;
      }
      bool array = s->target->kind == ExprKind::index;
      bool outer = t->kind == ExprKind::ident && !inner.count(to_std(t->text));
      uint32_t r = CU_thread_assign(outer, array);
      if (r) { refuse(s->loc, r); return; }
    }
    access_expr(s->expr, written); access_expr(s->expr2, written); access_expr(s->target, written);
    bool loop = s->kind == StmtKind::for_range || s->kind == StmtKind::for_each;
    for (const Stmt *c : s->body) phase(c, written, direct && !loop);
    phase(s->then_, written, direct && !loop);
    phase(s->else_, written, direct && !loop);
  }
  // Shared code: no threadgroup or view stores, primitives where they belong.
  bool stores_view(const ProcDecl *p, std::set<const ProcDecl *> *seen) {
    if (!p || seen->count(p)) return false;
    seen->insert(p);
    std::function<bool(const Expr *)> ex = [&](const Expr *e) -> bool {
      if (!e) return false;
      if (e->kind == ExprKind::call && e->proc && !is_device_primitive(e->proc) && stores_view(e->proc, seen)) return true;
      if (ex(e->lhs) || ex(e->rhs) || ex(e->third)) return true;
      for (const Expr *a : e->args) if (ex(a)) return true;
      return false;
    };
    std::function<bool(const Stmt *)> st = [&](const Stmt *x) -> bool {
      if (!x) return false;
      if (x->kind == StmtKind::assign) {
        const Expr *t = x->target;
        while (t->kind == ExprKind::index || t->kind == ExprKind::field) { if (t->kind == ExprKind::index && t->lhs->type->kind == TypeKind::view) return true; t = t->lhs; }
      }
      if (ex(x->expr) || ex(x->expr2) || ex(x->target)) return true;
      for (const Stmt *c : x->body) if (st(c)) return true;
      return st(x->then_) || st(x->else_);
    };
    return st(p->body);
  }
  void shared_expr(const Expr *e) {
    if (!e) return;
    if (e->kind == ExprKind::call && e->proc && apple_kind(e->proc)) {
      int kind = apple_kind(e->proc);
      bool shaped = true;
      int64_t len[3] = {0, 0, 0}; bool tg[3] = {false, false, false};
      if (kind == 3) {
        shaped = e->args.count == 11;
        int64_t tm = shaped ? value_of(e->args[1]) : -1, tn = shaped ? value_of(e->args[2]) : -1;
        uint32_t r = CU_apple_tile(tm, tn, shaped && constant_of(e->args[1]) && constant_of(e->args[2]));
        if (r) { refuse(e->loc, r); return; }
        for (int i = 8; shaped && i < 11; i++) if (e->args[i]->type->kind != TypeKind::view) shaped = false;
        if (shaped && (compute_code(e->args[8]->type->elem) != 10 || compute_code(e->args[9]->type->elem) != 2 || compute_code(e->args[10]->type->elem) != 2)) shaped = false;
      } else {
        shaped = e->args.count == (kind == 1 ? 3 : 2);
        for (int32_t i = 0; shaped && i < e->args.count; i++) {
          const Expr *a = e->args[i];
          ArrayUse *u = a->kind == ExprKind::slice && !a->rhs ? array_of(a->lhs) : nullptr;
          if (!u || compute_code(u->decl->type->elem) != 2) { shaped = false; break; }
          len[i] = u->decl->type->length;
          tg[i] = shared.count(to_std(a->lhs->text)) > 0;
        }
      }
      uint32_t r = shaped ? CU_apple_primitive((uint32_t)kind, metal, true, size, len[0], len[1], len[2], tg[1], tg[2]) : (metal ? 41u : 42u);
      if (r) { refuse(e->loc, r); return; }
      g_warp.apple_calls[e] = kind;
      if (kind == 3) for (const Expr *a : e->args) shared_expr(a);
      return;
    }
    if (e->kind == ExprKind::call && e->proc && !is_device_primitive(e->proc)) {
      std::set<const ProcDecl *> seen;
      if (stores_view(e->proc, &seen)) { refuse(e->loc, CU_group_store(true, false)); return; }
    }
    shared_expr(e->lhs); shared_expr(e->rhs); shared_expr(e->third);
    for (const Expr *a : e->args) shared_expr(a);
  }
  void shared_code(const Stmt *s) {
    if (!s) return;
    if (is_thread_loop(s)) {
      g_warp.thread_loops[s] = size;
      std::set<std::string> written, saved = inner;
      auto saved_ranges = ranges;
      inner.clear();
      writes_in(s->then_, &written);
      inner.clear();
      inner.insert("thread");
      phase(s->then_, written, true);
      // Apple primitives inside a thread loop are refused.
      std::function<void(const Stmt *)> no_apple = [&](const Stmt *x) {
        if (!x) return;
        std::function<void(const Expr *)> ex = [&](const Expr *e) {
          if (!e) return;
          if (e->kind == ExprKind::call && apple_kind(e->proc)) { refuse(e->loc, metal ? 41 : 42); return; }
          ex(e->lhs); ex(e->rhs); ex(e->third);
          for (const Expr *a : e->args) ex(a);
        };
        ex(x->expr); ex(x->expr2); ex(x->target);
        for (const Stmt *c : x->body) no_apple(c);
        no_apple(x->then_); no_apple(x->else_);
      };
      no_apple(s->then_);
      inner = saved; ranges = saved_ranges;
      return;
    }
    if (s->kind == StmtKind::for_range) {
      const Expr *lo = constant_of(s->expr), *hi = constant_of(s->expr2);
      if (lo && hi) ranges[to_std(s->name)] = {(int64_t)lo->int_value, (int64_t)hi->int_value};
    }
    if (s->kind == StmtKind::assign) {
      const Expr *t = s->target;
      while (t->kind == ExprKind::index || t->kind == ExprKind::field) {
        if (t->kind == ExprKind::index && t->lhs->type->kind == TypeKind::view) { refuse(s->loc, CU_group_store(true, false)); return; }
        if (t->kind == ExprKind::index && t->lhs->kind == ExprKind::ident && shared.count(to_std(t->lhs->text))) { refuse(s->loc, CU_shared_store(false)); return; }
        t = t->lhs;
      }
    }
    shared_expr(s->expr); shared_expr(s->expr2); shared_expr(s->target);
    for (const Stmt *c : s->body) shared_code(c);
    shared_code(s->then_); shared_code(s->else_);
  }
  bool has_mma(const Stmt *s) {
    if (!s) return false;
    std::function<bool(const Expr *)> ex = [&](const Expr *e) -> bool {
      if (!e) return false;
      if (e->kind == ExprKind::call && is_mma_primitive(e->proc)) return true;
      if (ex(e->lhs) || ex(e->rhs) || ex(e->third)) return true;
      for (const Expr *a : e->args) if (ex(a)) return true;
      return false;
    };
    if (ex(s->expr) || ex(s->expr2) || ex(s->target)) return true;
    for (const Stmt *c : s->body) if (has_mma(c)) return true;
    return has_mma(s->then_) || has_mma(s->else_);
  }
  void admit(const ProcDecl *p) {
    kernel = p; size = 0; sized_ok = true; arrays.clear(); ranges.clear(); inner.clear(); shared.clear(); any_thread = false;
    g_group_index.clear();
    scan(p->body, true, false, nullptr, false);
    if (!any_thread && size == 0) return;  // not a group kernel
    if (!sized_ok) return;
    bool warp = g_warp.warp_kernels.count(p) > 0;
    uint32_t r = CU_group_size(size, size, warp, has_mma(p->body));
    if (r) { refuse(p->loc, r); return; }
    g_warp.group_kernels[p] = size;
    int64_t bytes = 0;
    for (auto &[name, u] : arrays) {
      if (u.accumulator) {
        if (CU_accumulator_use(!u.in_thread && !u.elsewhere)) { refuse(u.decl->loc, CU_accumulator_use(false)); continue; }
        g_warp.accumulators.insert(u.decl);
        continue;
      }
      uint32_t cls = CU_group_array(u.in_thread, u.own, u.elsewhere);
      if (cls == 1) { g_warp.thread_arrays[u.decl] = size; continue; }
      if (cls == 0) continue;  // shared code alone uses it: an ordinary local
      r = CU_shared_use(u.top, !u.other);
      if (r) { refuse(u.decl->loc, r); continue; }
      shared.insert(name);
      g_warp.shared_arrays[p].push_back(u.decl);
      g_warp.shared_decls.insert(u.decl);
      bytes += u.decl->type->length * elem_bytes(u.decl->type->elem);
    }
    // Arrays only a primitive reads (no thread loop) are threadgroup memory too.
    for (auto &[name, u] : arrays) {
      if (u.accumulator || u.in_thread || shared.count(name) || !u.elsewhere) continue;
      bool operand = false;
      std::function<void(const Stmt *)> st = [&](const Stmt *x) {
        if (!x) return;
        std::function<void(const Expr *)> ex = [&](const Expr *e) {
          if (!e) return;
          if (e->kind == ExprKind::call && apple_kind(e->proc) && apple_kind(e->proc) != 3)
            for (const Expr *a : e->args) if (a->kind == ExprKind::slice && a->lhs->kind == ExprKind::ident && str_eq(a->lhs->text, name.c_str())) operand = true;
          ex(e->lhs); ex(e->rhs); ex(e->third);
          for (const Expr *a : e->args) ex(a);
        };
        ex(x->expr); ex(x->expr2); ex(x->target);
        for (const Stmt *c : x->body) st(c);
        st(x->then_); st(x->else_);
      };
      st(p->body);
      if (!operand) continue;
      r = CU_shared_use(u.top, !u.other);
      if (r) { refuse(u.decl->loc, r); continue; }
      shared.insert(name);
      g_warp.shared_arrays[p].push_back(u.decl);
      g_warp.shared_decls.insert(u.decl);
      bytes += u.decl->type->length * elem_bytes(u.decl->type->elem);
    }
    r = CU_shared_bytes(bytes);
    if (r) refuse(p->loc, r);
    for (const auto &[e, k] : g_group_index) {
      auto at = arrays.find(to_std(e->lhs->text));
      if (at != arrays.end() && g_warp.thread_arrays.count(at->second.decl)) g_warp.thread_index[e] = k;
    }
    ranges.clear(); inner.clear();
    shared_code(p->body);
  }
  // Thread loops and Apple primitives belong in kernels.
  void helper(const Stmt *s) {
    if (!s) return;
    if (is_thread_loop(s)) { refuse(s->loc, CU_thread_loop(false, true, 0, 32, false)); return; }
    std::function<void(const Expr *)> ex = [&](const Expr *e) {
      if (!e) return;
      if (e->kind == ExprKind::call && apple_kind(e->proc)) { refuse(e->loc, metal ? 41 : 42); return; }
      ex(e->lhs); ex(e->rhs); ex(e->third);
      for (const Expr *a : e->args) ex(a);
    };
    ex(s->expr); ex(s->expr2); ex(s->target);
    for (const Stmt *c : s->body) helper(c);
    helper(s->then_); helper(s->else_);
  }
};

struct ComputeAdmission {
  const File *file;
  bool metal = false;
  std::vector<const ProcDecl *> done, active;
  bool ok = true;

  void refuse(Loc loc, uint32_t reason) {
    compute_refuse(file, loc, reason, metal);
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
      if (is_device_primitive(e->proc)) {  // a device primitive: its body is its CPU meaning, not device code
        // Its arguments are whole register arrays (the warp check sizes them).
        for (const Expr *a : e->args) expr(a->kind == ExprKind::slice && !a->rhs ? a->lhs : a);
        return;
      }
      if (e->byte_intrinsic != ByteIntrinsic::none) {
        r = metal ? MT_byte_intrinsic((uint32_t)e->byte_intrinsic) : CU_byte_intrinsic((uint32_t)e->byte_intrinsic);
        if (r) { refuse(e->loc, r); return; }
      }
      if (metal && e->intrinsic != Intrinsic::none) {
        r = MT_math((uint32_t)e->intrinsic, compute_code(e->type) == 3);
        if (r) { refuse(e->loc, r); return; }
      }
    }
    if (metal && e->kind == ExprKind::cast) {
      uint32_t r = MT_cast(compute_code(e->lhs->type), compute_code(e->type));
      if (r) { refuse(e->loc, r); return; }
    }
    if (metal && e->kind == ExprKind::slice) {
      uint32_t r = MT_slice(e->lhs->type->kind == TypeKind::array, false);
      if (r) { refuse(e->loc, r); return; }
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
bool compute_admit(const File *file, bool metal) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)file; (void)compute_code; (void)metal;
  fprintf(stderr, "coax: coax cuda and coax metal-compute require the final compiler, not build-only stage0\n");
  return false;
#else
  ComputeAdmission a{file};
  WarpAdmission warp{file};
  GroupAdmission group{file};
  a.metal = warp.metal = group.metal = metal;
  g_warp = WarpInfo{};
  bool any = false;
  for (const ProcDecl *p : file->procs) {
    if (!p->export_c) continue;
    any = true;
    a.proc(p);
    warp.kernel(p);
    group.admit(p);
  }
  for (const ProcDecl *p : a.done) if (!p->export_c) { warp.helper(p->body); group.helper(p->body); }
  a.ok = a.ok && warp.ok && group.ok;
  if (!any) {
    error_at(file->src, {1, 1, 0}, metal ? "E0703" : "E0701", metal ? "a Metal compute module needs at least one #export_c kernel" : "a CUDA module needs at least one #export_c kernel",
             "Mark each kernel #export_c; its first parameter is the int work item and it returns nothing.");
    return false;
  }
  return a.ok;
#endif
}

}  // namespace coax
