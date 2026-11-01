// coax query: where a name is defined, who uses it and what a module exports,
// answered from the checked graph through #load (never from text search, so a
// comment or a string never matches). The index is every top-level
// declaration and every resolved reference between them, each with its unit,
// line and column. It is cached under ~/.cache/coax-query ($COAX_QUERY_CACHE)
// keyed by the compiler binary's hash and the entry unit; a cached index is
// used only when every source unit and descriptor it was built from still has
// the recorded SHA-256, so a warm query runs no front end.
//
// Included by main.cpp after executable_path(), profile_file_hash() and
// write_file(), inside namespace coax. Build-only stage0 has no hash and
// refuses queries.

struct QueryUnit {
  std::string path, canonical, sha256, module;  // path: absolute, before symlinks resolve
  bool claims = false, sealed = false;
};
struct QueryDecl {
  std::string kind, name, id;
  int32_t unit = -1, line = 0, col = 0, end_line = 0;
  bool generated = false, exported = false;
};
struct QueryRef {
  int32_t to = -1;  // index in decls
  int32_t unit = -1, line = 0, col = 0;
  std::string from;  // "proc NAME", "struct NAME", "#test \"NAME\"", "#assert"
  bool generated = false;
};
struct QueryIndex {
  std::string compiler, entry;
  std::vector<QueryUnit> units;
  std::vector<std::pair<std::string, std::string>> descriptors;  // path, sha256
  std::vector<QueryDecl> decls;
  std::vector<QueryRef> refs;
};

static const char *QUERY_SCHEMA = "coax.query-index.v1";

// Fields are tab-separated; a field never holds a raw tab, newline or backslash.
static std::string query_escape(const std::string &s) {
  std::string out;
  for (char c : s) {
    if (c == '\\') out += "\\\\";
    else if (c == '\t') out += "\\t";
    else if (c == '\n') out += "\\n";
    else out += c;
  }
  return out;
}
static std::string query_unescape(const std::string &s) {
  std::string out;
  for (size_t i = 0; i < s.size(); i++) {
    if (s[i] != '\\' || i + 1 == s.size()) { out += s[i]; continue; }
    char c = s[++i];
    out += c == 't' ? '\t' : c == 'n' ? '\n' : c;
  }
  return out;
}

// The expression tree as parsed, before the checker runs. The checker
// replaces a use of an untyped constant (`N :: 4;`) with a copy of its
// literal in place, and folds constant arithmetic (`N * 4`) into one literal
// whose operands it no longer links. This snapshot keeps each node's
// written identifier and operands, so those uses are still found.
struct QueryParsed {
  struct Node { std::string ident; std::vector<const Expr *> kids; };
  std::map<const Expr *, Node> nodes;
  void expr(const Expr *e) {
    if (!e || nodes.count(e)) return;
    Node &n = nodes[e];
    if (e->kind == ExprKind::ident) n.ident = to_std(e->text);
    for (const Expr *k : {e->lhs, e->rhs, e->third}) if (k) n.kids.push_back(k);
    for (const Expr *k : e->args) n.kids.push_back(k);
    for (const FieldInit &f : e->fields) if (f.value) n.kids.push_back(f.value);
    for (const Expr *k : std::vector<const Expr *>(n.kids)) expr(k);
    type(e->type_expr);
  }
  void type(const TypeExpr *t) {
    for (; t; t = t->elem) expr(t->length);
  }
  void stmt(const Stmt *s) {
    if (!s) return;
    expr(s->expr); expr(s->expr2); expr(s->target);
    type(s->type_expr);
    for (const Stmt *b : s->body) stmt(b);
    stmt(s->then_); stmt(s->else_);
  }
  void proc(const ProcDecl *p) {
    for (const Param &x : p->params) type(x.type_expr);
    type(p->ret_expr);
    stmt(p->body);
  }
  void file(const File *f) {
    for (const ProcDecl *p : f->procs) proc(p);
    for (const ProcDecl *p : f->generics) proc(p);
    for (const ConstDecl *c : f->constants) expr(c->expr);
    for (const StructDecl *d : f->structs) for (const Field &x : d->fields) { type(x.type_expr); expr(x.init); }
    for (const StructDecl *d : f->generic_structs) for (const Field &x : d->fields) { type(x.type_expr); expr(x.init); }
    for (const EnumDecl *d : f->enums) for (const EnumMember &m : d->members) expr(m.value);
    for (const CheckDecl *c : f->checks) if (c->proc) proc(c->proc);
    for (const Expr *a : f->assertions) expr(a);
  }
};

// Builds the index from a checked program, before evaluation freezes
// constants and field defaults to literals. A reference is a place where a
// declaration's name is written and resolves to it: calls, constants (typed,
// and untyped ones through QueryParsed), and struct and enum names in types,
// literals and casts, in declarations, checks, top-level asserts and generic
// bodies (through their instances). Its column is the name itself: past an
// alias (`hud.Slot`), not the parameter or field that has the type. A type
// that is inferred, not written (`for s: all`), is not a reference.
struct QueryBuilder {
  const Source *src;
  const File *file;
  const QueryParsed *parsed;
  QueryIndex *index;
  std::map<int32_t, int32_t> decl_at;  // declaration offset -> decls index
  std::map<std::string, std::vector<const ConstDecl *>> consts;  // by name
  std::set<std::tuple<int32_t, int32_t, std::string>> seen;  // (offset, to, from): instances share their template's sites

  int32_t unit_of(Loc loc) const {
    const SourceUnit *u = source_unit_at(src, loc);
    for (size_t i = 0; u && i < src->units.size(); i++) if (src->units[i].get() == u) return (int32_t)i;
    return -1;
  }
  bool generated_at(Loc loc) const {
    for (const GeneratedRegion &region : src->generated) if (loc.offset >= region.start && loc.offset < region.end) return true;
    return false;
  }
  std::string module_at(Loc loc) const {
    const SourceUnit *u = source_unit_at(src, loc);
    return u ? u->module : "";
  }
  void decl(const char *kind, Str name, Loc loc, int32_t end, uint8_t id_kind) {
    QueryDecl d;
    d.kind = kind; d.name = to_std(name); d.id = declaration_id(src, id_kind, name, loc);
    d.unit = unit_of(loc); d.generated = generated_at(loc);
    SourcePoint at = source_point(src, loc);
    d.line = at.loc.line; d.col = at.loc.col;
    d.end_line = d.line;
    if (end > loc.offset && !d.generated) {
      const SourceUnit *u = source_unit_at(src, loc);
      if (u && end <= u->end) {
        int32_t lines = 0;
        for (int32_t i = loc.offset - u->begin; i < end - u->begin && i < (int32_t)u->text.size(); i++) lines += u->text[(size_t)i] == '\n';
        d.end_line = d.line + lines;
      }
    }
    decl_at[loc.offset] = (int32_t)index->decls.size();
    index->decls.push_back(d);
  }
  // The offset change from a node's location to the name as written, or
  // false when the name is not written there. A node sits at its name, at an
  // alias before it (`shapes.area`, `a.b.V`) or, for `Kind.circle`, at the
  // dot just after it.
  static bool is_word(char c) { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_'; }
  bool anchor(Loc at, const std::string &name, int32_t *delta) const {
    const SourceUnit *u = source_unit_at(src, at);
    if (!u || at.offset < u->begin || at.offset > u->end) return false;
    const std::string &t = u->text;
    const int64_t start = at.offset - u->begin;
    auto written = [&](int64_t i) {
      return i >= 0 && i + (int64_t)name.size() <= (int64_t)t.size() && t.compare((size_t)i, name.size(), name) == 0 &&
             (i == 0 || !is_word(t[(size_t)i - 1])) && (i + (int64_t)name.size() == (int64_t)t.size() || !is_word(t[(size_t)i + name.size()]));
    };
    for (int64_t i = start, hops = 0; hops < 4 && i < (int64_t)t.size(); hops++) {
      if (written(i)) { *delta = (int32_t)(i - start); return true; }
      int64_t j = i;
      while (j < (int64_t)t.size() && is_word(t[(size_t)j])) j++;
      if (j == i || j >= (int64_t)t.size() || t[(size_t)j] != '.') break;
      i = j + 1;
    }
    if (written(start - (int64_t)name.size())) { *delta = -(int32_t)name.size(); return true; }
    return false;
  }
  void reference(const std::string &from, Loc decl, Loc at) {
    auto found = decl_at.find(decl.offset);
    if (found == decl_at.end()) return;
    QueryRef r;
    r.to = found->second; r.from = from; r.unit = unit_of(at); r.generated = generated_at(at);
    SourcePoint point = source_point(src, at);
    r.line = point.loc.line; r.col = point.loc.col;
    int32_t delta = 0;
    if (!r.generated && !anchor(at, index->decls[(size_t)r.to].name, &delta)) return;  // not written here
    r.col += delta;
    if (!seen.insert({at.offset + delta, r.to, from}).second) return;
    index->refs.push_back(r);
  }
  // A resolved type and the type expression that wrote it: the reference is
  // at the named type inside any array or view brackets.
  void type(const std::string &from, Type t, const TypeExpr *te) {
    lengths(from, te);
    while (t && (t->kind == TypeKind::array || t->kind == TypeKind::view)) t = t->elem;
    while (te && te->kind != TypeExpr::named) te = te->elem;
    if (!t || !te) return;
    if (t->kind == TypeKind::struct_ && t->struct_decl) {
      const StructDecl *d = t->struct_decl->origin ? t->struct_decl->origin : t->struct_decl;
      reference(from, d->loc, te->loc);
    }
    if (t->kind == TypeKind::enum_ && t->enum_decl) reference(from, t->enum_decl->loc, te->loc);
  }
  void expr(const std::string &from, const Expr *e) {
    if (!e) return;
    if (e->kind == ExprKind::call && e->proc) reference(from, (e->proc->origin ? e->proc->origin : e->proc)->loc, e->loc);
    if (e->kind == ExprKind::ident && e->constant) reference(from, e->constant->loc, e->loc);
    const QueryParsed::Node *was = nullptr;
    if (parsed) {
      auto found = parsed->nodes.find(e);
      if (found != parsed->nodes.end()) was = &found->second;
    }
    if (e->kind != ExprKind::ident && was && !was->ident.empty()) {
      auto named = consts.find(was->ident);
      if (named != consts.end()) {
        // An inlined untyped constant: the one of that name in the use's
        // module, or the only one.
        const ConstDecl *pick = named->second.size() == 1 ? named->second[0] : nullptr;
        for (const ConstDecl *c : named->second) if (named->second.size() > 1 && module_at(c->loc) == module_at(e->loc)) pick = c;
        if (pick) reference(from, pick->loc, e->loc);
      }
    }
    if (e->kind == ExprKind::struct_lit || e->kind == ExprKind::cast || e->kind == ExprKind::array_lit) type(from, e->type, e->type_expr);
    if (e->kind == ExprKind::enum_value && e->type && e->type->kind == TypeKind::enum_ && e->type->enum_decl) reference(from, e->type->enum_decl->loc, e->loc);
    expr(from, e->lhs); expr(from, e->rhs); expr(from, e->third);
    for (const Expr *a : e->args) expr(from, a);
    for (const FieldInit &f : e->fields) expr(from, f.value);
    lengths(from, e->type_expr);
    // Operands a fold unlinked (sites already seen are not repeated).
    if (was) for (const Expr *k : was->kids) expr(from, k);
  }
  // Array lengths written in a type: `[OBJECTS * 4] f32`.
  void lengths(const std::string &from, const TypeExpr *t) {
    for (; t; t = t->elem) expr(from, t->length);
  }
  void stmt(const std::string &from, const Stmt *s) {
    if (!s) return;
    if (s->kind == StmtKind::var) type(from, s->type, s->type_expr);
    expr(from, s->expr); expr(from, s->expr2); expr(from, s->target);
    for (const Stmt *b : s->body) stmt(from, b);
    stmt(from, s->then_); stmt(from, s->else_);
  }
  void proc(const std::string &from, const ProcDecl *d) {
    for (const Param &p : d->params) type(from, p.type, p.type_expr);
    type(from, d->ret, d->ret_expr);
    stmt(from, d->body);
  }

  void build() {
    for (const auto &u : src->units) {
      QueryUnit q;
      std::error_code ec;
      auto absolute = std::filesystem::absolute(u->path, ec);
      q.path = ec ? u->path : absolute.lexically_normal().string();
      q.canonical = u->canonical; q.sha256 = debug_source_hash(u->text);
      q.module = u->module; q.claims = u->claims;
      index->units.push_back(q);
    }
    for (const auto &u : src->units) if (!u->exports.empty())
      for (QueryUnit &q : index->units) if (!u->module.empty() && q.module == u->module) q.sealed = true;
    for (const std::string &path : src->descriptor_inputs) {
      std::string bytes;
      index->descriptors.push_back({path, read_record(path, &bytes) ? debug_source_hash(bytes) : ""});
    }
    for (const StructDecl *d : file->structs) if (!d->origin) decl("struct", d->name, d->loc, d->end, 1);
    for (const StructDecl *d : file->generic_structs) decl("struct", d->name, d->loc, d->end, 1);
    for (const EnumDecl *d : file->enums) decl("enum", d->name, d->loc, d->end, 2);
    for (const ConstDecl *d : file->constants) { decl("const", d->name, d->loc, d->end, 3); consts[to_std(d->name)].push_back(d); }
    for (const ProcDecl *d : file->procs) if (!d->origin) decl("proc", d->name, d->loc, d->end, 0);
    for (const ProcDecl *d : file->generics) decl("proc", d->name, d->loc, d->end, 0);
    // Exported: listed by a sealed module, or every declaration of a module
    // with no list (and of the unowned program).
    std::map<std::string, std::set<std::string>> listed;
    for (const auto &u : src->units) for (const auto &e : u->exports) listed[u->module].insert(e.name);
    for (QueryDecl &d : index->decls) {
      const std::string module = d.unit >= 0 ? index->units[(size_t)d.unit].module : "";
      auto list = listed.find(module);
      d.exported = module.empty() || list == listed.end() || list->second.count(d.name);
    }
    for (const StructDecl *d : file->generic_structs) {
      const std::string from = "struct " + to_std(d->name);
      for (const Field &f : d->fields) { type(from, f.type, f.type_expr); expr(from, f.init); }
    }
    for (const StructDecl *d : file->structs) {
      if (d->origin) continue;
      const std::string from = "struct " + to_std(d->name);
      for (const Field &f : d->fields) { type(from, f.type, f.type_expr); expr(from, f.init); }
    }
    for (const EnumDecl *d : file->enums) {
      const std::string from = "enum " + to_std(d->name);
      for (const EnumMember &m : d->members) expr(from, m.value);
    }
    for (const ConstDecl *d : file->constants) {
      const std::string from = "const " + to_std(d->name);
      expr(from, d->expr);
    }
    // Procedures, and generic bodies through every instance (deduplicated by site).
    for (const ProcDecl *d : file->procs) proc("proc " + to_std((d->origin ? d->origin : d)->name), d);
    for (const ProcDecl *d : file->generics) proc("proc " + to_std(d->name), d);
    for (const CheckDecl *c : file->checks) if (c->proc) {
      const std::string from = std::string(c->forall ? "#forall" : "#test") + " \"" + to_std(c->name) + "\"";
      for (const Param &p : c->proc->params) type(from, p.type, p.type_expr);
      stmt(from, c->proc->body);
    }
    for (const Expr *a : file->assertions) expr("#assert", a);
    // Declarations and references in source order: by unit, line and column.
    std::vector<int32_t> order(index->decls.size()), moved(index->decls.size());
    for (size_t i = 0; i < order.size(); i++) order[i] = (int32_t)i;
    const auto &decls = index->decls;
    std::stable_sort(order.begin(), order.end(), [&](int32_t a, int32_t b) {
      return std::tie(decls[(size_t)a].unit, decls[(size_t)a].line, decls[(size_t)a].col) < std::tie(decls[(size_t)b].unit, decls[(size_t)b].line, decls[(size_t)b].col);
    });
    std::vector<QueryDecl> sorted;
    for (size_t i = 0; i < order.size(); i++) { moved[(size_t)order[i]] = (int32_t)i; sorted.push_back(decls[(size_t)order[i]]); }
    index->decls = sorted;
    for (QueryRef &r : index->refs) r.to = moved[(size_t)r.to];
    std::stable_sort(index->refs.begin(), index->refs.end(), [](const QueryRef &a, const QueryRef &b) {
      return std::tie(a.unit, a.line, a.col, a.to) < std::tie(b.unit, b.line, b.col, b.to);
    });
  }
};

static std::string query_serialize(const QueryIndex &q) {
  std::string out = std::string(QUERY_SCHEMA) + "\ncompiler\t" + q.compiler + "\nentry\t" + query_escape(q.entry) + "\n";
  for (const QueryUnit &u : q.units)
    out += "unit\t" + query_escape(u.path) + "\t" + query_escape(u.canonical) + "\t" + u.sha256 + "\t" + query_escape(u.module) + "\t" +
           (u.claims ? "1" : "0") + "\t" + (u.sealed ? "1" : "0") + "\n";
  for (const auto &[path, sha] : q.descriptors) out += "descriptor\t" + query_escape(path) + "\t" + sha + "\n";
  for (const QueryDecl &d : q.decls)
    out += "decl\t" + d.kind + "\t" + query_escape(d.name) + "\t" + query_escape(d.id) + "\t" + std::to_string(d.unit) + "\t" +
           std::to_string(d.line) + "\t" + std::to_string(d.col) + "\t" + std::to_string(d.end_line) + "\t" + (d.generated ? "1" : "0") + "\t" +
           (d.exported ? "1" : "0") + "\n";
  for (const QueryRef &r : q.refs)
    out += "ref\t" + std::to_string(r.to) + "\t" + std::to_string(r.unit) + "\t" + std::to_string(r.line) + "\t" + std::to_string(r.col) + "\t" +
           query_escape(r.from) + "\t" + (r.generated ? "1" : "0") + "\n";
  return out + "end\n";
}

static bool query_number(const std::string &s, int32_t *out) {
  if (s.empty() || s.size() > 10) return false;
  int64_t v = 0;
  size_t i = s[0] == '-' ? 1 : 0;
  if (i == s.size()) return false;
  for (; i < s.size(); i++) { if (s[i] < '0' || s[i] > '9') return false; v = v * 10 + (s[i] - '0'); }
  if (s[0] == '-') v = -v;
  if (v < INT32_MIN || v > INT32_MAX) return false;
  *out = (int32_t)v;
  return true;
}

// Parses a cached index. Any malformed or truncated record rejects the whole file.
static bool query_parse(const std::string &text, QueryIndex *q) {
  size_t at = 0;
  bool header = false, ended = false;
  while (at < text.size()) {
    size_t nl = text.find('\n', at);
    if (nl == std::string::npos) return false;
    std::string line = text.substr(at, nl - at);
    at = nl + 1;
    if (!header) { if (line != QUERY_SCHEMA) return false; header = true; continue; }
    if (ended) return false;
    std::vector<std::string> f;
    for (size_t s = 0;;) {
      size_t t = line.find('\t', s);
      f.push_back(line.substr(s, t == std::string::npos ? std::string::npos : t - s));
      if (t == std::string::npos) break;
      s = t + 1;
    }
    const std::string &kind = f[0];
    if (kind == "end" && f.size() == 1) ended = true;
    else if (kind == "compiler" && f.size() == 2) q->compiler = f[1];
    else if (kind == "entry" && f.size() == 2) q->entry = query_unescape(f[1]);
    else if (kind == "unit" && f.size() == 7) q->units.push_back({query_unescape(f[1]), query_unescape(f[2]), f[3], query_unescape(f[4]), f[5] == "1", f[6] == "1"});
    else if (kind == "descriptor" && f.size() == 3) q->descriptors.push_back({query_unescape(f[1]), f[2]});
    else if (kind == "decl" && f.size() == 10) {
      QueryDecl d;
      d.kind = f[1]; d.name = query_unescape(f[2]); d.id = query_unescape(f[3]);
      if (!query_number(f[4], &d.unit) || !query_number(f[5], &d.line) || !query_number(f[6], &d.col) || !query_number(f[7], &d.end_line)) return false;
      if (d.unit < -1 || d.unit >= (int32_t)q->units.size()) return false;
      d.generated = f[8] == "1"; d.exported = f[9] == "1";
      q->decls.push_back(d);
    } else if (kind == "ref" && f.size() == 7) {
      QueryRef r;
      if (!query_number(f[1], &r.to) || !query_number(f[2], &r.unit) || !query_number(f[3], &r.line) || !query_number(f[4], &r.col)) return false;
      if (r.to < 0 || r.to >= (int32_t)q->decls.size() || r.unit < -1 || r.unit >= (int32_t)q->units.size()) return false;
      r.from = query_unescape(f[5]); r.generated = f[6] == "1";
      q->refs.push_back(r);
    } else return false;
  }
  return header && ended;
}

static std::string query_cache_dir() {
  const char *dir = getenv("COAX_QUERY_CACHE");
  if (dir && *dir) return dir;
  const char *home = getenv("HOME");
  return std::string(home && *home ? home : "/tmp") + "/.cache/coax-query";
}

// A cached index answers only when it was written by this compiler for this
// entry, and every unit it read is still the same file with the same bytes.
static bool query_current(const QueryIndex &q, const std::string &compiler, const std::string &entry) {
  if (q.compiler != compiler || q.entry != entry || q.units.empty()) return false;
  for (const QueryUnit &u : q.units) {
    std::error_code ec;
    auto canonical = std::filesystem::canonical(u.path, ec);
    if (ec || canonical.string() != u.canonical) return false;
    if (profile_file_hash(u.canonical) != u.sha256) return false;
  }
  for (const auto &[path, sha] : q.descriptors) if (sha.empty() || profile_file_hash(path) != sha) return false;
  return true;
}

// Text paths are relative to the working directory when they are under it.
static std::string query_display(const std::string &canonical) {
  std::error_code ec;
  auto cwd = std::filesystem::current_path(ec);
  if (ec) return canonical;
  std::string base = cwd.string();
  if (!base.empty() && base.back() != '/') base += '/';
  return canonical.compare(0, base.size(), base) == 0 ? canonical.substr(base.size()) : canonical;
}

struct QueryRequest {
  std::string form, name;  // form: defines, uses, exports
  bool json = false, fresh = false;
};

static std::string query_location_json(const QueryIndex &q, int32_t unit, int32_t line, int32_t col) {
  return "\"file\":" + (unit >= 0 ? json_string(q.units[(size_t)unit].canonical) : std::string("null")) + ",\"line\":" + std::to_string(line) +
         ",\"column\":" + std::to_string(col);
}
static std::string query_decl_json(const QueryIndex &q, const QueryDecl &d) {
  const std::string module = d.unit >= 0 ? q.units[(size_t)d.unit].module : "";
  return "{\"kind\":" + json_string(d.kind) + ",\"name\":" + json_string(d.name) + ",\"id\":" + (d.id.empty() ? "null" : json_string(d.id)) +
         ",\"module\":" + (module.empty() ? "null" : json_string(module)) + "," + query_location_json(q, d.unit, d.line, d.col) +
         ",\"end_line\":" + std::to_string(d.end_line) + ",\"generated\":" + (d.generated ? "true" : "false") + ",\"exported\":" +
         (d.exported ? "true" : "false") + "}";
}
static std::string query_where(const QueryIndex &q, int32_t unit, int32_t line, int32_t col) {
  return (unit >= 0 ? query_display(q.units[(size_t)unit].canonical) : std::string("?")) + ":" + std::to_string(line) + ":" + std::to_string(col);
}

// Answers one question from an index; false (with a message) for an unknown module.
static bool query_answer(const QueryIndex &q, const QueryRequest &r, std::string *out) {
  std::vector<int32_t> targets;
  if (r.form == "defines" || r.form == "uses")
    for (size_t i = 0; i < q.decls.size(); i++) if (q.decls[i].name == r.name) targets.push_back((int32_t)i);
  if (r.form == "defines") {
    if (r.json) {
      *out = "{\"schema\":\"coax.query.v1\",\"query\":\"defines\",\"name\":" + json_string(r.name) + ",\"definitions\":[";
      for (size_t i = 0; i < targets.size(); i++) *out += (i ? "," : "") + query_decl_json(q, q.decls[(size_t)targets[i]]);
      *out += "]}\n";
    } else
      for (int32_t t : targets) {
        const QueryDecl &d = q.decls[(size_t)t];
        *out += query_where(q, d.unit, d.line, d.col) + ": " + d.kind + " " + d.name + (d.generated ? " (generated)" : "") + "\n";
      }
    return true;
  }
  if (r.form == "uses") {
    std::set<int32_t> wanted(targets.begin(), targets.end());
    if (r.json) {
      *out = "{\"schema\":\"coax.query.v1\",\"query\":\"uses\",\"name\":" + json_string(r.name) + ",\"definitions\":[";
      for (size_t i = 0; i < targets.size(); i++) *out += (i ? "," : "") + query_decl_json(q, q.decls[(size_t)targets[i]]);
      *out += "],\"uses\":[";
      bool first = true;
      for (const QueryRef &ref : q.refs) {
        if (!wanted.count(ref.to)) continue;
        const QueryDecl &d = q.decls[(size_t)ref.to];
        *out += std::string(first ? "" : ",") + "{" + query_location_json(q, ref.unit, ref.line, ref.col) + ",\"from\":" + json_string(ref.from) +
                ",\"kind\":" + json_string(d.kind) + ",\"to\":" + (d.id.empty() ? "null" : json_string(d.id)) + ",\"generated\":" +
                (ref.generated ? "true" : "false") + "}";
        first = false;
      }
      *out += "]}\n";
    } else
      for (const QueryRef &ref : q.refs) if (wanted.count(ref.to))
        *out += query_where(q, ref.unit, ref.line, ref.col) + ": " + ref.from + (ref.generated ? " (generated)" : "") + "\n";
    return true;
  }
  // exports [MODULE]: a module's public declarations; with no module, the
  // entry unit's module, or the entry unit's own declarations when it has none.
  std::string module = r.name;
  int32_t entry_unit = -1;
  if (module.empty()) {
    for (size_t i = 0; i < q.units.size(); i++) if (q.units[i].canonical == q.entry) entry_unit = (int32_t)i;
    if (entry_unit >= 0) module = q.units[(size_t)entry_unit].module;
  } else {
    bool known = false;
    for (const QueryUnit &u : q.units) known = known || u.module == module;
    if (!known) { fprintf(stderr, "coax: query: no module '%s' in this program\n", module.c_str()); return false; }
  }
  bool sealed = false;
  for (const QueryUnit &u : q.units) sealed = sealed || (!module.empty() && u.module == module && u.sealed);
  std::vector<int32_t> rows;
  for (size_t i = 0; i < q.decls.size(); i++) {
    const QueryDecl &d = q.decls[i];
    bool member = module.empty() ? d.unit == entry_unit : d.unit >= 0 && q.units[(size_t)d.unit].module == module;
    if (member && d.exported) rows.push_back((int32_t)i);
  }
  if (r.json) {
    *out = "{\"schema\":\"coax.query.v1\",\"query\":\"exports\",\"module\":" + (module.empty() ? std::string("null") : json_string(module)) +
           ",\"sealed\":" + (sealed ? "true" : "false") + ",\"exports\":[";
    for (size_t i = 0; i < rows.size(); i++) *out += (i ? "," : "") + query_decl_json(q, q.decls[(size_t)rows[i]]);
    *out += "]}\n";
  } else
    for (int32_t i : rows) {
      const QueryDecl &d = q.decls[(size_t)i];
      *out += query_where(q, d.unit, d.line, d.col) + ": " + d.kind + " " + d.name + "\n";
    }
  return true;
}

