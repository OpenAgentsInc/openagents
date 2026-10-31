// `coax spirv`: native code observes the checked tree (already held to the
// Metal scalar profile by the checker) and hands it to lib/compiler/spirv.coax
// as tree code; the SV_ procedures decide every instruction, and this file
// writes the words they return. Build-only stage0 has no lowering and
// refuses the command.
#include "coax.h"

namespace coax {

#ifndef COAX_BOOTSTRAP_IMPORTER
// Tree code, version 1 (the comment at the top of spirv.coax).
struct SpirvTree {
  const File *file;
  std::vector<uint32_t> words;
  std::vector<std::pair<std::string, uint32_t>> scope;  // local names, innermost last
  uint32_t locals = 0;
  std::vector<uint32_t> local_types;
  bool ok = true;

  void refuse(Loc loc, const std::string &what) {
    if (ok) error_at(file->src, loc, "E0704", what + " is outside the SPIR-V lowering",
                     "coax spirv lowers what coax metal accepts: f32 and bool, if/else and calls");
    ok = false;
  }
  static uint32_t type_code(Type t) { return t == &T_VOID ? 0 : t == &T_BOOL ? 1 : t == &T_F32 ? 2 : 99; }
  uint32_t proc_index(const ProcDecl *p) {
    for (uint32_t i = 0; i < (uint32_t)file->procs.count; i++) if (file->procs[i] == p) return i;
    return 0;
  }
  uint32_t declare(Str name, Type t) {
    scope.emplace_back(std::string(name.data, name.count), locals);
    local_types.push_back(type_code(t));
    return locals++;
  }
  bool find(Str name, uint32_t *id) {
    std::string n(name.data, name.count);
    for (size_t i = scope.size(); i-- > 0;) if (scope[i].first == n) { *id = scope[i].second; return true; }
    return false;
  }
  void f32(double v) {
    float f = (float)v;
    uint32_t bits;
    memcpy(&bits, &f, 4);
    words.push_back(16); words.push_back(bits);
  }

  // A literal at the type its use gives it.
  void literal(const Expr *e, Type t) {
    if (e->kind == ExprKind::bool_lit) { words.push_back(17); words.push_back(e->int_value ? 1 : 0); }
    else if (e->kind == ExprKind::float_lit && t == &T_F32) f32(e->float_value);
    else if (e->kind == ExprKind::int_lit && t == &T_F32) f32(e->negative ? (double)(int64_t)e->int_value : (double)e->int_value);
    else refuse(e->loc, "this constant");
  }

  void expr(const Expr *e) {
    static const Tok ops[] = {Tok::eof, Tok::plus, Tok::minus, Tok::star, Tok::slash, Tok::percent, Tok::lt, Tok::le,
                              Tok::gt, Tok::ge, Tok::eqeq, Tok::noteq, Tok::andand, Tok::oror};
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::float_lit: case ExprKind::bool_lit: literal(e, e->type); return;
      case ExprKind::ident: {
        uint32_t id;
        if (e->constant && e->constant->expr) { literal(e->constant->expr, e->type); return; }
        if (!find(e->text, &id)) { refuse(e->loc, "this name"); return; }
        words.push_back(18); words.push_back(id);
        return;
      }
      case ExprKind::unary:
        if (e->op == Tok::plus) { expr(e->lhs); return; }
        if (e->op != Tok::minus && e->op != Tok::bang) break;
        words.push_back(e->op == Tok::minus ? 20 : 21);
        expr(e->lhs);
        return;
      case ExprKind::binary:
        for (uint32_t op = 1; op < 14; op++)
          if (ops[op] == e->op) { words.push_back(19); words.push_back(op); expr(e->lhs); expr(e->rhs); return; }
        break;
      case ExprKind::call:
        if (e->intrinsic != Intrinsic::none) { words.push_back(23); words.push_back((uint32_t)e->intrinsic); }
        else if (e->proc && !e->builtin_print && e->byte_intrinsic == ByteIntrinsic::none) { words.push_back(22); words.push_back(proc_index(e->proc)); }
        else break;
        words.push_back((uint32_t)e->args.count);
        for (const Expr *a : e->args) expr(a);
        return;
      case ExprKind::ifx:
        words.push_back(24); expr(e->lhs); expr(e->rhs); expr(e->third);
        return;
      case ExprKind::cast:
        if (e->type == e->lhs->type) { expr(e->lhs); return; }
        break;
      default: break;
    }
    refuse(e->loc, "this expression");
  }

  void stmt(const Stmt *s) {
    static const Tok assigns[] = {Tok::equal, Tok::plus_equal, Tok::minus_equal, Tok::star_equal, Tok::slash_equal, Tok::percent_equal};
    switch (s->kind) {
      case StmtKind::block: {
        size_t depth = scope.size();
        words.push_back(1); words.push_back((uint32_t)s->body.count);
        for (const Stmt *c : s->body) stmt(c);
        scope.resize(depth);
        return;
      }
      case StmtKind::expr: words.push_back(2); expr(s->expr); return;
      case StmtKind::var: {
        // A declaration without a value stores zero, as C does; `= ---` stores nothing.
        if (s->uninit) { declare(s->name, s->type); words.push_back(1); words.push_back(0); return; }
        words.push_back(3);
        size_t at = words.size();
        words.push_back(0); words.push_back(0);
        if (s->expr) expr(s->expr);
        else if (s->type == &T_BOOL) { words.push_back(17); words.push_back(0); }
        else f32(0);
        words[at] = declare(s->name, s->type);  // after the value: it cannot see itself
        return;
      }
      case StmtKind::assign: {
        uint32_t id, op = 6;
        for (uint32_t i = 0; i < 6; i++) if (assigns[i] == s->op) op = i;
        if (op == 6 || s->target->kind != ExprKind::ident || !find(s->target->text, &id)) break;
        words.push_back(3); words.push_back(id); words.push_back(op); expr(s->expr);
        return;
      }
      case StmtKind::ret:
        if (s->expr) { words.push_back(4); expr(s->expr); }
        else words.push_back(5);
        return;
      case StmtKind::if_:
        words.push_back(6); words.push_back(s->else_ ? 1 : 0); expr(s->expr);
        stmt(s->then_);
        if (s->else_) stmt(s->else_);
        return;
      default: break;
    }
    refuse(s->loc, "this statement");
  }
};
#endif

// kernel names one procedure for a compute kernel; fill names one to four
// (comma-separated) for a full-screen pass; stroke names one for a stroke
// pass. At most one of them is set; none makes a library.
bool emit_spirv(const File *file, const std::string &kernel, const std::string &fill, const std::string &stroke, std::string *out) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)file; (void)kernel; (void)fill; (void)stroke; (void)out;
  fprintf(stderr, "coax: coax spirv requires the final compiler, not build-only stage0\n");
  return false;
#else
  SpirvTree t{file};
  auto index = [&](const std::string &name, const char *flag, uint32_t *at) {
    for (uint32_t i = 0; i < (uint32_t)file->procs.count; i++)
      if (std::string(file->procs[i]->name.data, file->procs[i]->name.count) == name) { *at = i; return true; }
    error_at(file->src, {1, 1, 0}, "E0704", "no procedure named '" + name + "' for " + flag,
             std::string(flag) + " names procedures of the file");
    return false;
  };
  t.words = {(uint32_t)file->procs.count, 0xFFFFFFFFu};
  if (!kernel.empty() && !index(kernel, "--kernel", &t.words[1])) return false;
  if (!stroke.empty()) {
    uint32_t p = 0;
    if (!index(stroke, "--stroke", &p)) return false;
    t.words[1] = 0xFFFFFFFDu;
    t.words.push_back(p);
  }
  if (!fill.empty()) {
    std::vector<uint32_t> ps;
    size_t at = 0;
    for (;;) {
      size_t end = fill.find(',', at);
      uint32_t p = 0;
      if (!index(fill.substr(at, end == std::string::npos ? std::string::npos : end - at), "--fill", &p)) return false;
      ps.push_back(p);
      if (end == std::string::npos) break;
      at = end + 1;
    }
    t.words[1] = 0xFFFFFFFEu;
    t.words.push_back((uint32_t)ps.size());
    t.words.insert(t.words.end(), ps.begin(), ps.end());
  }
  for (const ProcDecl *p : file->procs) {
    t.words.push_back(SpirvTree::type_code(p->ret));
    t.words.push_back((uint32_t)p->params.count);
    for (const Param &prm : p->params) t.words.push_back(SpirvTree::type_code(prm.type));
  }
  for (const ProcDecl *p : file->procs) {
    t.scope.clear(); t.locals = 0; t.local_types.clear();
    for (const Param &prm : p->params) t.declare(prm.name, prm.type);
    std::vector<uint32_t> body;
    std::swap(body, t.words);
    t.stmt(p->body);
    std::swap(body, t.words);
    t.words.push_back(t.locals);
    t.words.insert(t.words.end(), t.local_types.begin(), t.local_types.end());
    t.words.insert(t.words.end(), body.begin(), body.end());
  }
  if (!t.ok) return false;
  std::vector<uint32_t> words(1u << 20), work(1u << 20);
  int64_t n = SV_lower({t.words.data(), (int64_t)t.words.size()}, {words.data(), (int64_t)words.size()}, {work.data(), (int64_t)work.size()});
  if (n <= 0) {
    coax_string text = SV_reason_text((uint32_t)-n);
    error_at(file->src, {1, 1, 0}, "E0704", std::string(text.data, (size_t)text.count) + " is outside the SPIR-V lowering",
             "coax spirv lowers what coax metal accepts: f32 and bool, if/else and calls");
    return false;
  }
  out->assign((const char *)words.data(), (size_t)n * 4);
  return true;
#endif
}

}  // namespace coax
