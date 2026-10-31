// `coax spirv --vertex V --fragment F`: the shader subset. Native code
// observes the checked tree (checked by the ordinary rules, not the Metal
// profile) and hands it to lib/compiler/shader.coax as tree code v2; the SH_
// procedures decide every instruction and refuse what they cannot lower, and
// this file writes the words they return. Build-only stage0 has no lowering.
#include "coax.h"

namespace coax {

#ifndef COAX_BOOTSTRAP_IMPORTER
// Tree code, version 2 (the comment at the top of shader.coax).
struct ShaderTree {
  const File *file;
  std::vector<uint32_t> words, types;   // types: the type table, encoded
  std::vector<Type> type_of;            // type index -> checked type
  std::vector<uint32_t> type_at;        // type index -> its offset in `types`
  std::vector<const ProcDecl *> procs;  // the procedures, in order of first need
  std::vector<std::pair<std::string, uint32_t>> scope;
  std::vector<uint32_t> local_types;
  std::vector<std::string> local_names;         // the procedure being encoded's, by local
  std::vector<std::vector<std::string>> names;  // each procedure's name, then its locals' (coax wgsl)
  uint32_t locals = 0;
  bool ok = true;

  void refuse(Loc loc, const std::string &what) {
    if (ok) error_at(file->src, loc, "E0704", what + " is outside the SPIR-V lowering",
                     "coax spirv --vertex/--fragment lowers f32, bool, s32, u32 and int, structs and fixed arrays of them, "
                     "views of them as an entry's storage, the textures and samplers of verse/gpu/shader.coax, if/else, "
                     "for and while loops with break and continue, and acyclic calls");
    ok = false;
  }
  static std::string name(Str s) { return std::string(s.data, s.count); }

  // The type table: each type once, its parts before it.
  uint32_t type(Type t, Loc loc) {
    for (uint32_t i = 0; i < type_of.size(); i++) if (type_of[i] == t) return i;
    std::vector<uint32_t> code;
    if (t == &T_VOID) code = {0};
    else if (t == &T_BOOL) code = {1};
    else if (t == &T_F32) code = {2};
    else if (t == &T_S32) code = {3};
    else if (t == &T_U32) code = {4};
    else if (t == &T_S64) code = {5};
    else if (t == &T_U64) code = {6};
    else if (t->kind == TypeKind::struct_ && t->struct_decl) {
      std::string n = name(t->struct_decl->name);
      if (n == "Tex2D") code = {10, 0};
      else if (n == "Tex2DArray") code = {10, 1};
      else if (n == "TexDepth") code = {10, 2};
      else if (n == "Sampler") code = {11, 0};
      else if (n == "SamplerCompare") code = {11, 1};
      else {
        code = {7, (uint32_t)t->struct_decl->fields.count};
        // a #flat field's offset carries bit 31 (an interface's fields have locations, not offsets)
        for (const Field &f : t->struct_decl->fields) { code.push_back(type(f.type, f.loc)); code.push_back((uint32_t)f.offset | (f.flat ? 0x80000000u : 0u)); }
        code.push_back((uint32_t)t->size);
      }
    } else if (t->kind == TypeKind::array && t->elem) code = {8, type(t->elem, loc), (uint32_t)t->length, (uint32_t)t->elem->size};
    else if (t->kind == TypeKind::view && t->elem) code = {9, type(t->elem, loc)};
    else { refuse(loc, std::string("a ") + type_name(t) + " value"); return 0; }
    type_of.push_back(t);
    type_at.push_back((uint32_t)types.size());
    types.insert(types.end(), code.begin(), code.end());
    return (uint32_t)type_of.size() - 1;
  }

  uint32_t proc_index(const ProcDecl *p) {
    for (uint32_t i = 0; i < procs.size(); i++) if (procs[i] == p) return i;
    procs.push_back(p);
    return (uint32_t)procs.size() - 1;
  }
  uint32_t declare(Str n, Type t, Loc loc) {
    scope.emplace_back(name(n), locals);
    local_types.push_back(type(t, loc));
    local_names.push_back(name(n));
    return locals++;
  }
  bool find(Str n, uint32_t *id) {
    std::string s = name(n);
    for (size_t i = scope.size(); i-- > 0;) if (scope[i].first == s) { *id = scope[i].second; return true; }
    return false;
  }

  // A constant of a scalar type: its type and its bits (two words for 64 bits).
  void constant(Type t, uint64_t bits, Loc loc) {
    words.push_back(16); words.push_back(type(t, loc));
    words.push_back((uint32_t)bits);
    if (t == &T_S64 || t == &T_U64) words.push_back((uint32_t)(bits >> 32));
  }
  void literal(const Expr *e, Type t) {
    if (e->kind == ExprKind::bool_lit && t == &T_BOOL) { constant(t, e->int_value ? 1 : 0, e->loc); return; }
    if (e->kind == ExprKind::float_lit || e->kind == ExprKind::int_lit) {
      double v = e->kind == ExprKind::float_lit ? e->float_value : e->negative ? (double)(int64_t)e->int_value : (double)e->int_value;
      if (t == &T_F32) { float f = (float)v; uint32_t b; memcpy(&b, &f, 4); constant(t, b, e->loc); return; }
      if (e->kind == ExprKind::int_lit && is_int(t) && t->bits <= 64) { constant(t, e->int_value, e->loc); return; }
    }
    refuse(e->loc, "this constant");
  }

  // The GPU's procedures in verse/gpu/shader.coax: their CPU bodies serve checks.
  static uint32_t gpu_intrinsic(const std::string &n) {
    static const char *names[] = {"", "gpu_discard", "gpu_sample", "gpu_sample_level", "gpu_sample_layer", "gpu_sample_layer_level",
                                  "gpu_sample_layer_bias", "gpu_load", "gpu_load_depth", "gpu_compare", "gpu_width", "gpu_height"};
    for (uint32_t i = 1; i < sizeof names / sizeof names[0]; i++) if (n == names[i]) return i;
    // A depth texture's size is the same instruction as a 2D texture's.
    if (n == "gpu_depth_width") return 10;
    if (n == "gpu_depth_height") return 11;
    return 0;
  }

  // A place: a local, a field of a place, an element of a place's array or a view.
  bool is_place(const Expr *e) {
    uint32_t id;
    if (e->kind == ExprKind::ident) return !e->constant && find(e->text, &id);
    if (e->kind == ExprKind::field) return e->field_index >= 0 && is_place(e->lhs);
    if (e->kind == ExprKind::index) return (e->lhs->type->kind == TypeKind::array || e->lhs->type->kind == TypeKind::view) && is_place(e->lhs);
    return false;
  }
  void place(const Expr *e) {
    uint32_t id = 0;
    if (e->kind == ExprKind::ident) { find(e->text, &id); words.push_back(40); words.push_back(id); return; }
    if (e->kind == ExprKind::field) { words.push_back(41); place(e->lhs); words.push_back((uint32_t)e->field_index); return; }
    words.push_back(42); place(e->lhs); expr(e->rhs);
  }

  void args(const Slice<Expr *> &a) { words.push_back((uint32_t)a.count); for (const Expr *x : a) expr(x); }

  // A struct literal names some fields; the others take their defaults, or zero.
  void struct_lit(const Expr *e) {
    const StructDecl *d = e->type->struct_decl;
    if (!d) { refuse(e->loc, "this literal"); return; }
    words.push_back(31); words.push_back(type(e->type, e->loc)); words.push_back((uint32_t)d->fields.count);
    for (int32_t i = 0; i < (int32_t)d->fields.count; i++) {
      const Expr *v = nullptr;
      for (const FieldInit &f : e->fields) if (f.field == i) v = f.value;
      const Field &f = d->fields[i];
      if (v) expr(v);
      else if (f.init) literal(f.init, f.type);
      else { words.push_back(17); words.push_back(type(f.type, f.loc)); }
    }
  }

  void expr(const Expr *e) {
    static const Tok ops[] = {Tok::eof, Tok::plus, Tok::minus, Tok::star, Tok::slash, Tok::percent, Tok::lt, Tok::le, Tok::gt, Tok::ge,
                              Tok::eqeq, Tok::noteq, Tok::andand, Tok::oror, Tok::amp, Tok::pipe, Tok::caret, Tok::shl, Tok::shr};
    if (!ok) return;
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::float_lit: case ExprKind::bool_lit: literal(e, e->type); return;
      case ExprKind::ident:
        if (e->constant && e->constant->expr) { literal(e->constant->expr, e->type); return; }
        if (!is_place(e)) break;
        { uint32_t id; find(e->text, &id); words.push_back(18); words.push_back(id); }
        return;
      case ExprKind::field:
        if (e->field_index < 0 && name(e->text) == "count" && e->lhs->type->kind == TypeKind::view && is_place(e->lhs)) {
          words.push_back(32); place(e->lhs); return;
        }
        if (e->field_index < 0) break;
        if (is_place(e)) { words.push_back(27); place(e); return; }
        words.push_back(25); expr(e->lhs); words.push_back((uint32_t)e->field_index);
        return;
      case ExprKind::index:
        if (!is_place(e)) break;
        words.push_back(27); place(e);
        return;
      case ExprKind::unary:
        if (e->op == Tok::plus) { expr(e->lhs); return; }
        if (e->op != Tok::minus && e->op != Tok::bang) break;
        words.push_back(e->op == Tok::minus ? 20 : 21); expr(e->lhs);
        return;
      case ExprKind::binary:
        for (uint32_t op = 1; op < 19; op++)
          if (ops[op] == e->op) { words.push_back(19); words.push_back(op); expr(e->lhs); expr(e->rhs); return; }
        break;
      case ExprKind::call:
        if (e->intrinsic != Intrinsic::none) { words.push_back(23); words.push_back((uint32_t)e->intrinsic); args(e->args); return; }
        // Wrapping u32 arithmetic is the GPU's own: an ordinary add, subtract or multiply.
        if (e->byte_intrinsic == ByteIntrinsic::add_wrap || e->byte_intrinsic == ByteIntrinsic::sub_wrap || e->byte_intrinsic == ByteIntrinsic::mul_wrap) {
          if (e->args[0]->type != &T_U32) break;
          words.push_back(19); words.push_back(e->byte_intrinsic == ByteIntrinsic::add_wrap ? 1 : e->byte_intrinsic == ByteIntrinsic::sub_wrap ? 2 : 3);
          expr(e->args[0]); expr(e->args[1]); return;
        }
        if (e->byte_intrinsic == ByteIntrinsic::f32_bits || e->byte_intrinsic == ByteIntrinsic::f32_from_bits) {
          words.push_back(30); words.push_back(type(e->type, e->loc)); expr(e->args[0]); return;
        }
        if (!e->proc || e->builtin_print || e->byte_intrinsic != ByteIntrinsic::none || e->proc->foreign) break;
        if (uint32_t g = gpu_intrinsic(name(e->proc->name))) { words.push_back(33); words.push_back(g); words.push_back(type(e->type, e->loc)); args(e->args); return; }
        words.push_back(22); words.push_back(proc_index(e->proc)); args(e->args);
        return;
      case ExprKind::ifx: words.push_back(24); expr(e->lhs); expr(e->rhs); expr(e->third); return;
      case ExprKind::cast:
        if (e->type == e->lhs->type) { expr(e->lhs); return; }
        if (!is_number(e->type) || !is_number(e->lhs->type)) break;
        words.push_back(29); words.push_back(type(e->type, e->loc)); expr(e->lhs);
        return;
      case ExprKind::struct_lit: struct_lit(e); return;
      case ExprKind::array_lit:
        if (e->type->kind != TypeKind::array || (int64_t)e->args.count != e->type->length) break;
        words.push_back(31); words.push_back(type(e->type, e->loc)); args(e->args);
        return;
      default: break;
    }
    refuse(e->loc, "this expression");
  }

  void stmt(const Stmt *s) {
    static const Tok assigns[] = {Tok::equal, Tok::plus_equal, Tok::minus_equal, Tok::star_equal, Tok::slash_equal, Tok::percent_equal,
                                  Tok::amp_equal, Tok::pipe_equal, Tok::caret_equal, Tok::shl_equal, Tok::shr_equal};
    static const uint32_t binops[] = {0, 1, 2, 3, 4, 5, 14, 15, 16, 17, 18};
    if (!ok) return;
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
        // A view declared to the whole of a local fixed array (`v := a[..]`): the array is in
        // Private storage and the view writes it.
        uint32_t array;
        if (s->expr && !s->uninit && s->expr->kind == ExprKind::slice && !s->expr->rhs && !s->expr->third &&
            s->expr->lhs->kind == ExprKind::ident && s->expr->lhs->type->kind == TypeKind::array && find(s->expr->lhs->text, &array)) {
          words.push_back(11);
          words.push_back(declare(s->name, s->type, s->loc)); words.push_back(array);
          return;
        }
        // A declaration stores its value, or zero as C does; `= ---` stores zero too.
        words.push_back(3);
        size_t at = words.size();
        words.push_back(40); words.push_back(0);
        if (s->expr && !s->uninit) expr(s->expr);
        else { words.push_back(17); words.push_back(type(s->type, s->loc)); }
        words[at + 1] = declare(s->name, s->type, s->loc);  // after the value: it cannot see itself
        return;
      }
      case StmtKind::assign: {
        int op = -1;
        for (int i = 0; i < 11; i++) if (assigns[i] == s->op) op = i;
        if (op < 0 || !is_place(s->target)) break;
        words.push_back(3); place(s->target);
        // x op= e stores x op e; the place's indices are evaluated twice, and they are pure.
        if (op > 0) { words.push_back(19); words.push_back(binops[op]); words.push_back(27); place(s->target); }
        expr(s->expr);
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
      case StmtKind::while_: words.push_back(7); expr(s->expr); stmt(s->then_); return;
      case StmtKind::for_range: {
        size_t depth = scope.size();
        words.push_back(8);
        size_t at = words.size();
        words.push_back(0);
        expr(s->expr); expr(s->expr2);
        words[at] = declare(s->name, s->type, s->loc);
        stmt(s->then_);
        scope.resize(depth);
        return;
      }
      case StmtKind::break_: words.push_back(9); return;
      case StmtKind::continue_: words.push_back(10); return;
      default: break;
    }
    refuse(s->loc, "this statement");
  }

  // One procedure: its signature, its locals and its body.
  std::vector<uint32_t> procedure(const ProcDecl *p) {
    words.clear(); scope.clear(); local_types.clear(); local_names.clear(); locals = 0;
    for (const Param &prm : p->params) declare(prm.name, prm.type, prm.loc);
    if (p->body) stmt(p->body);
    else refuse(p->loc, "a procedure without a body");
    names.push_back({name(p->name)});
    names.back().insert(names.back().end(), local_names.begin(), local_names.end());
    std::vector<uint32_t> out = {type(p->ret, p->loc), (uint32_t)p->params.count};
    for (uint32_t i = 0; i < (uint32_t)p->params.count; i++) out.push_back(local_types[i]);
    out.push_back(locals);
    out.insert(out.end(), local_types.begin(), local_types.end());
    out.push_back((uint32_t)words.size());  // the body's length, so a reader can pass over it
    out.insert(out.end(), words.begin(), words.end());
    return out;
  }
};
#endif

// Entry points by name: vertex then fragment, either may be empty.
// With `tree`, *out is the tree code as decimal words instead (for checks of the lowering).
bool emit_shader(const File *file, const std::string &vertex, const std::string &fragment, bool tree_text, std::string *out, std::string *report) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)file; (void)vertex; (void)fragment; (void)tree_text; (void)out; (void)report;
  fprintf(stderr, "coax: coax spirv requires the final compiler, not build-only stage0\n");
  return false;
#else
  ShaderTree t{file};
  // The scalars at fixed places: 0 void, 1 bool, 2 f32, 3 s32, 4 u32, 5 int.
  for (Type s : {(Type)&T_VOID, (Type)&T_BOOL, (Type)&T_F32, (Type)&T_S32, (Type)&T_U32, (Type)&T_S64}) t.type(s, {1, 1, 0});
  std::vector<std::pair<uint32_t, uint32_t>> entries;
  for (int stage = 0; stage < 2; stage++) {
    const std::string &want = stage == 0 ? vertex : fragment;
    if (want.empty()) continue;
    const ProcDecl *found = nullptr;
    for (const ProcDecl *p : file->procs) if (ShaderTree::name(p->name) == want) found = p;
    if (!found) {
      error_at(file->src, {1, 1, 0}, "E0705", "there is no procedure " + want + " to make an entry point",
               "--vertex and --fragment name procedures of the file or what it loads");
      return false;
    }
    entries.emplace_back(stage, t.proc_index(found));
  }
  // The procedures each entry reaches, encoded as they are first called.
  std::vector<std::vector<uint32_t>> bodies;
  for (size_t i = 0; i < t.procs.size() && t.ok; i++) bodies.push_back(t.procedure(t.procs[i]));
  if (!t.ok) return false;
  std::vector<uint32_t> tree = {2, (uint32_t)t.type_of.size()};
  tree.insert(tree.end(), t.types.begin(), t.types.end());
  tree.push_back((uint32_t)bodies.size());
  for (const auto &b : bodies) tree.insert(tree.end(), b.begin(), b.end());
  // Each entry: its stage, its procedure and a name id per parameter (the same name, the same id),
  // so a vertex and a fragment resource of one name share a binding.
  std::vector<std::string> names;
  tree.push_back((uint32_t)entries.size());
  for (auto [stage, proc] : entries) {
    tree.push_back(stage); tree.push_back(proc);
    for (const Param &prm : t.procs[proc]->params) {
      std::string n = ShaderTree::name(prm.name);
      uint32_t id = 0;
      while (id < names.size() && names[id] != n) id++;
      if (id == names.size()) names.push_back(n);
      tree.push_back(id);
    }
  }
  if (tree_text) {
    for (size_t i = 0; i < tree.size(); i++) *out += (i ? " " : "") + std::to_string(tree[i]);
    *out += "\n";
    return true;
  }
  std::vector<uint32_t> words((size_t)SH_out_words()), work((size_t)SH_work_words());
  int64_t n = SH_lower({tree.data(), (int64_t)tree.size()}, {words.data(), (int64_t)words.size()}, {work.data(), (int64_t)work.size()});
  if (n < 0) {
    coax_string why = SH_reason_text((uint32_t)-n);
    error_at(file->src, {1, 1, 0}, "E0704", std::string(why.data, (size_t)why.count) + " is outside the SPIR-V lowering",
             "coax spirv --vertex/--fragment lowers the shader subset of lib/compiler/shader.coax");
    return false;
  }
  out->assign((const char *)words.data(), (size_t)n * 4);
  // The bindings the module declares (set 0), as the lowering left them at the
  // start of the work view: a count, then name id, binding and kind for each.
  static const char *kinds[] = {"", "storage", "image", "sampler"};
  for (uint32_t k = 0; k < work[0]; k++) {
    uint32_t id = work[1 + 3 * k], binding = work[2 + 3 * k], kind = work[3 + 3 * k];
    *report += "binding " + std::to_string(binding) + " " + (kind < 4 ? kinds[kind] : "?") + " " + (id < names.size() ? names[id] : "?") + "\n";
  }
  return true;
#endif
}

// `coax wgsl FILE --functions A,B`: the procedures named, and what they call, as WGSL
// functions (lib/compiler/wgsl.coax), each a stage-2 entry of the same tree code. With
// `tree`, *out is the tree as decimal words, then the names' lines.
// `coax wgsl FILE --vertices A,B --fragments C,D --bindings F [--strip P]`: a whole module
// (WG_module), each vertex and fragment entry with its resources bound as F's lines
// (`NAME GROUP BINDING [uniform COUNT]`) say; an entry's WGSL name is its procedure's
// without the prefix P.
#ifndef COAX_BOOTSTRAP_IMPORTER
static std::vector<std::string> wgsl_list(const std::string &list) {
  std::vector<std::string> r;
  size_t from = 0;
  while (from <= list.size()) {
    size_t comma = list.find(',', from);
    std::string want = list.substr(from, comma == std::string::npos ? std::string::npos : comma - from);
    from = comma == std::string::npos ? list.size() + 1 : comma + 1;
    if (!want.empty()) r.push_back(want);
  }
  return r;
}
#endif
bool emit_wgsl_module(const File *file, const std::string &vertices, const std::string &fragments, const std::string &bindings,
                      const std::string &strip, bool tree_text, std::string *out) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)file; (void)vertices; (void)fragments; (void)bindings; (void)strip; (void)tree_text; (void)out;
  fprintf(stderr, "coax: coax wgsl requires the final compiler, not build-only stage0\n");
  return false;
#else
  ShaderTree t{file};
  for (Type s : {(Type)&T_VOID, (Type)&T_BOOL, (Type)&T_F32, (Type)&T_S32, (Type)&T_U32, (Type)&T_S64}) t.type(s, {1, 1, 0});
  std::vector<std::pair<uint32_t, uint32_t>> entries;
  for (int stage = 0; stage < 2; stage++) {
    for (const std::string &want : wgsl_list(stage == 0 ? vertices : fragments)) {
      const ProcDecl *found = nullptr;
      for (const ProcDecl *p : file->procs) if (ShaderTree::name(p->name) == want) found = p;
      if (!found) {
        error_at(file->src, {1, 1, 0}, "E0705", "there is no procedure " + want + " to make an entry point",
                 "--vertices and --fragments name procedures of the file or what it loads");
        return false;
      }
      entries.emplace_back(stage, t.proc_index(found));
    }
  }
  if (entries.empty()) {
    error_at(file->src, {1, 1, 0}, "E0705", "coax wgsl needs --functions, or --vertices and --fragments", "name the procedures to write as WGSL");
    return false;
  }
  std::vector<std::vector<uint32_t>> bodies;
  for (size_t i = 0; i < t.procs.size() && t.ok; i++) bodies.push_back(t.procedure(t.procs[i]));
  if (!t.ok) return false;
  std::vector<uint32_t> tree = {2, (uint32_t)t.type_of.size()};
  tree.insert(tree.end(), t.types.begin(), t.types.end());
  tree.push_back((uint32_t)bodies.size());
  for (const auto &b : bodies) tree.insert(tree.end(), b.begin(), b.end());
  std::vector<std::string> ids;
  tree.push_back((uint32_t)entries.size());
  for (auto [stage, proc] : entries) {
    tree.push_back(stage); tree.push_back(proc);
    for (const Param &prm : t.procs[proc]->params) {
      std::string n = ShaderTree::name(prm.name);
      uint32_t id = 0;
      while (id < ids.size() && ids[id] != n) id++;
      if (id == ids.size()) ids.push_back(n);
      tree.push_back(id);
    }
  }
  // the bindings file: a line a resource
  std::vector<std::string> lines;
  if (!bindings.empty()) {
    FILE *f = fopen(bindings.c_str(), "rb");
    if (!f) {
      error_at(file->src, {1, 1, 0}, "E0705", "cannot read the bindings " + bindings, "--bindings names a file of `NAME GROUP BINDING` lines");
      return false;
    }
    std::string text;
    char buf[4096];
    size_t got;
    while ((got = fread(buf, 1, sizeof buf, f)) > 0) text.append(buf, got);
    fclose(f);
    size_t at = 0;
    while (at < text.size()) {
      size_t e = text.find('\n', at);
      if (e == std::string::npos) e = text.size();
      std::string line = text.substr(at, e - at);
      at = e + 1;
      if (!line.empty() && line[0] != '#') lines.push_back(line);
    }
  }
  std::string names;
  for (size_t p = 0; p < t.names.size(); p++) {
    const auto &line = t.names[p];
    bool root = false;
    for (auto [stage, proc] : entries) root = root || proc == p;
    for (size_t i = 0; i < line.size(); i++) {
      std::string w = line[i];
      if (i == 0 && root && !strip.empty() && w.compare(0, strip.size(), strip) == 0 && w.size() > strip.size()) w = w.substr(strip.size());
      names += (i ? " " : "") + w;
    }
    names += "\n";
  }
  for (const std::string &id : ids) {
    std::string line = id;
    for (const std::string &b : lines) if (b.compare(0, id.size() + 1, id + " ") == 0) line = b;
    names += line + "\n";
  }
  if (tree_text) {
    for (size_t i = 0; i < tree.size(); i++) *out += (i ? " " : "") + std::to_string(tree[i]);
    *out += "\n" + names;
    return true;
  }
  std::vector<uint8_t> text((size_t)WG_out_bytes());
  std::vector<uint32_t> work((size_t)WG_work_words());
  int64_t n = WG_module({tree.data(), (int64_t)tree.size()}, {names.data(), (int64_t)names.size()}, {text.data(), (int64_t)text.size()},
                        {work.data(), (int64_t)work.size()});
  if (n < 0) {
    coax_string why = WG_reason_text((uint32_t)-n);
    error_at(file->src, {1, 1, 0}, "E0706", std::string(why.data, (size_t)why.count) + " is outside the WGSL writer",
             "coax wgsl writes modules as lib/compiler/wgsl.coax does");
    return false;
  }
  out->assign((const char *)text.data(), (size_t)n);
  return true;
#endif
}

bool emit_wgsl(const File *file, const std::string &functions, bool tree_text, std::string *out) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)file; (void)functions; (void)tree_text; (void)out;
  fprintf(stderr, "coax: coax wgsl requires the final compiler, not build-only stage0\n");
  return false;
#else
  ShaderTree t{file};
  for (Type s : {(Type)&T_VOID, (Type)&T_BOOL, (Type)&T_F32, (Type)&T_S32, (Type)&T_U32, (Type)&T_S64}) t.type(s, {1, 1, 0});
  std::vector<uint32_t> roots;
  for (const std::string &want : wgsl_list(functions)) {
    const ProcDecl *found = nullptr;
    for (const ProcDecl *p : file->procs) if (ShaderTree::name(p->name) == want) found = p;
    if (!found) {
      error_at(file->src, {1, 1, 0}, "E0705", "there is no procedure " + want + " to write as WGSL", "--functions names procedures of the file or what it loads");
      return false;
    }
    roots.push_back(t.proc_index(found));
  }
  if (roots.empty()) {
    error_at(file->src, {1, 1, 0}, "E0705", "coax wgsl needs --functions A,B", "name the procedures to write as WGSL");
    return false;
  }
  std::vector<std::vector<uint32_t>> bodies;
  for (size_t i = 0; i < t.procs.size() && t.ok; i++) bodies.push_back(t.procedure(t.procs[i]));
  if (!t.ok) return false;
  std::vector<uint32_t> tree = {2, (uint32_t)t.type_of.size()};
  tree.insert(tree.end(), t.types.begin(), t.types.end());
  tree.push_back((uint32_t)bodies.size());
  for (const auto &b : bodies) tree.insert(tree.end(), b.begin(), b.end());
  // each root a function entry; its parameters' name ids, which functions do not use, are 0
  tree.push_back((uint32_t)roots.size());
  for (uint32_t proc : roots) {
    tree.push_back(2); tree.push_back(proc);
    for (int32_t i = 0; i < t.procs[proc]->params.count; i++) tree.push_back(0);
  }
  std::string names;
  for (const auto &line : t.names) {
    for (size_t i = 0; i < line.size(); i++) names += (i ? " " : "") + line[i];
    names += "\n";
  }
  if (tree_text) {
    for (size_t i = 0; i < tree.size(); i++) *out += (i ? " " : "") + std::to_string(tree[i]);
    *out += "\n" + names;
    return true;
  }
  std::vector<uint8_t> text((size_t)WG_out_bytes());
  std::vector<uint32_t> work((size_t)WG_work_words());
  int64_t n = WG_emit({tree.data(), (int64_t)tree.size()}, {names.data(), (int64_t)names.size()}, {text.data(), (int64_t)text.size()},
                      {work.data(), (int64_t)work.size()});
  if (n < 0) {
    coax_string why = WG_reason_text((uint32_t)-n);
    error_at(file->src, {1, 1, 0}, "E0706", std::string(why.data, (size_t)why.count) + " is outside the WGSL writer",
             "coax wgsl writes the scalar functions of lib/compiler/wgsl.coax");
    return false;
  }
  out->assign((const char *)text.data(), (size_t)n);
  return true;
#endif
}

}  // namespace coax
