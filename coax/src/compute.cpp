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
  bool any = false;
  for (const ProcDecl *p : file->procs) {
    if (!p->export_c) continue;
    any = true;
    a.proc(p);
  }
  if (!any) {
    error_at(file->src, {1, 1, 0}, "E0701", "a CUDA module needs at least one #export_c kernel",
             "Mark each kernel #export_c; its first parameter is the int work item and it returns nothing.");
    return false;
  }
  return a.ok;
#endif
}

}  // namespace coax
