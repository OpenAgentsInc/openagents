// coax test and coax mutate. Checks (#test, #forall) run in the evaluator,
// never in generated code, each with its own step budget. Mutation testing
// checks the checks: it changes the program one operator, literal, condition
// or statement at a time and requires some check to fail. A mutant that
// survives is code the checks do not pin down, unless it is equivalent: a
// change no input can observe, such as `x < lo` becoming `x <= lo` before
// `return lo`. A reviewer marks such a line with the comment
// `// mutant: equivalent`; mutants on it that survive are counted apart.
#include "coax.h"
#ifdef COAX_BOOTSTRAP_IMPORTER
#include "sources.h"
#include "mutation-schedule.h"
#endif

#include <algorithm>
#include <functional>

namespace coax {
// The Coax policy's closed type/expr observations use this ABI mapping.
static_assert((uint8_t)TypeKind::invalid == 0 && (uint8_t)TypeKind::string_ == 5 &&
              (uint8_t)TypeKind::struct_ == 8 && (uint8_t)TypeKind::array == 10 &&
              (uint8_t)TypeKind::view == 11 && (uint8_t)ExprKind::cast == 14 &&
              (uint8_t)ExprKind::ct_value == 17, "mutation scheduling kind ABI");

static constexpr uint64_t TEST_STEPS = 20000000;

// Runs every check in order. Failures print as diagnostics with the check's
// name and, for a #forall, the counterexample. verbose: one line per check.
static bool run_all_checks(File *file, uint64_t max_steps, bool verbose, std::vector<uint64_t> *steps = nullptr, int32_t *failures = nullptr,
                           std::vector<std::vector<const ProcDecl *>> *paths = nullptr, bool *unstable = nullptr) {
  bool ok = true;
  std::vector<const ProcDecl *> executed;
  if (failures) *failures = 0;
  for (CheckDecl *c : file->checks) {
    const auto slots = file->ct_memory->slots, containers = file->ct_memory->containers;
    CheckOutcome r = run_check(file, c, max_steps);
    if (paths) paths->push_back(r.executed);
    if (unstable && MS_storage_changed(slots, containers, file->ct_memory->slots, file->ct_memory->containers)) *unstable = true;
    if (steps) steps->push_back(r.steps);
    executed.insert(executed.end(), r.executed.begin(), r.executed.end());
    const char *kind = c->forall ? "#forall" : "#test";
    std::string name = to_std(c->name);
    if (r.passed) {
      if (verbose)
        printf("ok   %s %s (%llu case%s, %llu steps)\n", kind, name.c_str(), (unsigned long long)r.cases,
               r.cases == 1 ? "" : "s", (unsigned long long)r.steps);
      continue;
    }
    ok = false;
    if (failures) ++*failures;
    if (verbose) {
      printf("FAIL %s %s\n", kind, name.c_str());
      fflush(stdout);
    }
    std::string message = r.message;
    if (g_json_errors) {
      message += " [" + std::string(kind) + " " + name + "]";
      if (!r.counterexample.empty()) message += "; counterexample: " + r.counterexample;
    }
    error_at(file->src, r.loc, r.code.c_str(), message);
    if (!g_json_errors) {
      fprintf(stderr, "  note: in %s %s\n", kind, name.c_str());
      if (!r.counterexample.empty()) fprintf(stderr, "  counterexample: %s\n", r.counterexample.c_str());
    }
  }
  if (ok && file->authority) {
    for (const ProcDecl *p : file->procs) {
      if (!SU_check_required(p->authority,p->root_source,false) || p->foreign || str_eq(p->name, "main")) continue;
      if (std::find(executed.begin(), executed.end(), p) == executed.end()) {
        error_at(file->src, p->loc, "E0380", "'" + to_std(p->name) + "' was never executed by a check",
                 "calls in dead branches or pre-folded #run results do not exercise the procedure");
        ok = false;
      }
    }
  }
  return ok;
}

static int test_command(File *file, uint64_t max_steps) {
  if (!file->checks.count) {
    fprintf(stderr, "coax: %s has no #test or #forall to run\n", file->src->path.c_str());
    return 1;
  }
  int32_t failed = 0;
  bool ok = run_all_checks(file, max_steps, true, nullptr, &failed);
  int32_t total = file->checks.count;
  printf("%d check%s, %d passed, %d failed\n", total, total == 1 ? "" : "s", total - failed, failed);
  if (!ok && !failed) printf("authority execution coverage failed\n");
  return ok ? 0 : 1;
}

// ---- mutation ----

struct Mutant {
  Loc loc;
  std::string what;
  std::function<void()> apply, undo;
  const ProcDecl *owner = nullptr;
};

struct Mutator {
  Arena *arena;
  std::vector<Mutant> mutants;
  std::vector<const ProcDecl *> reached;

  static Tok swap_of(Tok op) {
    switch (op) {
      case Tok::lt: return Tok::le;
      case Tok::le: return Tok::lt;
      case Tok::gt: return Tok::ge;
      case Tok::ge: return Tok::gt;
      case Tok::eqeq: return Tok::noteq;
      case Tok::noteq: return Tok::eqeq;
      case Tok::plus: return Tok::minus;
      case Tok::minus: return Tok::plus;
      case Tok::star: return Tok::plus;
      case Tok::slash: return Tok::star;
      case Tok::percent: return Tok::slash;
      case Tok::andand: return Tok::oror;
      case Tok::oror: return Tok::andand;
      case Tok::amp: return Tok::pipe;
      case Tok::pipe: return Tok::amp;
      case Tok::caret: return Tok::amp;
      case Tok::shl: return Tok::shr;
      case Tok::shr: return Tok::shl;
      case Tok::plus_equal: return Tok::minus_equal;
      case Tok::minus_equal: return Tok::plus_equal;
      default: return Tok::eof;
    }
  }

  static std::string spell(Tok op) {
    switch (op) {
      case Tok::plus_equal: return "+=";
      case Tok::minus_equal: return "-=";
      default: {
        std::string name = tok_name(op);  // "'<'"
        return name.size() > 2 && name.front() == '\'' ? name.substr(1, name.size() - 2) : name;
      }
    }
  }

  void reach(const ProcDecl *p) {
    if (!p || p->foreign) return;
    for (const ProcDecl *q : reached)
      if (q == p) return;
    reached.push_back(p);
    calls(p->body);
  }

  void calls(const Stmt *s) {
    if (!s) return;
    for (const Expr *e : {s->expr, s->expr2, s->target}) calls(e);
    for (const Stmt *c : s->body) calls(c);
    calls(s->then_);
    calls(s->else_);
  }

  void calls(const Expr *e) {
    if (!e) return;
    if (e->kind == ExprKind::call) reach(e->proc);
    calls(e->lhs);
    calls(e->rhs);
    calls(e->third);
    for (const Expr *a : e->args) calls(a);
    for (const FieldInit &f : e->fields) calls(f.value);
  }

  void expr(Expr *e) {
    // The Coax checker identifies synthetic metadata. Mutation targets source
    // behavior; dynamic selector expressions remain outside these subtrees.
    if (!e || e->compiler_generated) return;
    if (e->kind == ExprKind::binary) {
      Tok from = e->op, to = swap_of(from);
      bool ints_only = from == Tok::percent;
      if (to != Tok::eof && (!ints_only || is_int(e->type)) && e->lhs->type != &T_STRING)
        mutants.push_back({e->loc, "'" + spell(from) + "' became '" + spell(to) + "'",
                           [e, to] { e->op = to; }, [e, from] { e->op = from; }});
    } else if (e->kind == ExprKind::int_lit && is_int(e->type)) {
      Type t = e->type;
      __int128 v = t->is_signed ? (__int128)(int64_t)e->int_value : (__int128)e->int_value;
      __int128 hi = t->is_signed ? ((__int128)1 << (t->bits - 1)) - 1 : ((__int128)1 << t->bits) - 1;
      __int128 w = v < hi ? v + 1 : v - 1;
      uint64_t from = e->int_value, to = (uint64_t)w;
      std::string a = t->is_signed ? std::to_string((long long)(int64_t)from) : std::to_string((unsigned long long)from);
      std::string b = t->is_signed ? std::to_string((long long)(int64_t)to) : std::to_string((unsigned long long)to);
      mutants.push_back({e->loc, a + " became " + b, [e, to] { e->int_value = to; }, [e, from] { e->int_value = from; }});
    } else if (e->kind == ExprKind::bool_lit) {
      uint64_t from = e->int_value;
      mutants.push_back({e->loc, std::string(from ? "true" : "false") + " became " + (from ? "false" : "true"),
                         [e, from] { e->int_value = !from; }, [e, from] { e->int_value = from; }});
    }
    expr(e->lhs);
    expr(e->rhs);
    expr(e->third);
    for (Expr *a : e->args) expr(a);
    for (FieldInit &f : e->fields) expr(f.value);
  }

  void stmt(Stmt *s) {
    if (!s) return;
    if (s->kind == StmtKind::if_ || s->kind == StmtKind::while_) {
      Expr *cond = s->expr;
      Expr *neg = make<Expr>(arena);
      neg->kind = ExprKind::unary;
      neg->op = Tok::bang;
      neg->loc = cond->loc;
      neg->type = &T_BOOL;
      neg->lhs = cond;
      mutants.push_back({cond->loc, std::string("the ") + (s->kind == StmtKind::if_ ? "if" : "while") + " condition was negated",
                         [s, neg] { s->expr = neg; }, [s, cond] { s->expr = cond; }});
    }
    bool removable = s->kind == StmtKind::assign || (s->kind == StmtKind::expr && s->expr->kind == ExprKind::call);
    if (removable) {
      StmtKind kind = s->kind;
      mutants.push_back({s->loc, s->kind == StmtKind::assign ? "the assignment was removed" : "the call was removed",
                         [s] { s->kind = StmtKind::block; }, [s, kind] { s->kind = kind; }});
    }
    if (s->kind == StmtKind::assign) {
      Tok from = s->op, to = swap_of(from);
      if (to != Tok::eof)
        mutants.push_back({s->loc, "'" + spell(from) + "' became '" + spell(to) + "'",
                           [s, to] { s->op = to; }, [s, from] { s->op = from; }});
    }
    expr(s->expr);
    expr(s->expr2);
    expr(s->target);
    for (Stmt *c : s->body) stmt(c);
    stmt(s->then_);
    stmt(s->else_);
  }
};

static bool marked_equivalent(const Source *src, Loc loc) {
  const std::string &text = src->text;
  // A mutant location is a real code token. An actual line comment on the
  // same line can only follow it; do not mistake strings or block comments
  // containing the marker for a reviewed exemption.
  size_t at = std::min((size_t)loc.offset, text.size());
  size_t end = text.find('\n', at);
  if (end == std::string::npos) end = text.size();
  int block = 0;
  while (at < end) {
    if (block) {
      if (text.compare(at, 2, "/*") == 0) { block++; at += 2; }
      else if (text.compare(at, 2, "*/") == 0) { block--; at += 2; }
      else at++;
      continue;
    }
    if (text[at] == '\"' || text[at] == '\'') {
      char quote = text[at++];
      while (at < end) {
        if (text[at] == '\\') at = std::min(at + 2, end);
        else if (text[at++] == quote) break;
      }
      continue;
    }
    if (text.compare(at, 2, "/*") == 0) { block++; at += 2; continue; }
    if (text.compare(at, 2, "//") == 0) {
      const std::string marker = "// mutant: equivalent";
      size_t after = at + marker.size();
      return text.compare(at, marker.size(), marker) == 0 &&
             (after == end || text[after] == ' ' || text[after] == '\t' || text[after] == '\r');
    }
    at++;
  }
  return false;
}

// Conservative facts for scheduling. Views held inside aggregate storage can
// form cycles that survive a check; byte-view-to-string casts allocate storage
// whose lifetime currently reaches beyond the check. Either requires the full
// reference runner, regardless of whether this particular baseline leaked.
static bool mutation_borrowed_type(Type t, std::vector<Type> &seen) {
  if (!t) return true;
  uint32_t action = MS_type_action((uint32_t)t->kind);
  if (action == 1 || action == 4) return true;
  if (action == 0) return false;
  if (std::find(seen.begin(), seen.end(), t) != seen.end()) return false;
  seen.push_back(t);
  if (action == 2) return mutation_borrowed_type(t->elem, seen);
  if (action == 3) {
    if (!t->struct_decl) return true;
    for (const Field &field : t->struct_decl->fields)
      if (mutation_borrowed_type(field.type, seen)) return true;
  }
  return false;
}
static bool mutation_borrowed_aggregate(const File *file) {
  std::vector<Type> seen;
  for (const StructDecl *decl : file->structs)
    if (mutation_borrowed_type(decl->type, seen)) return true;
  for (Type t : all_composite_types()) {
    if (!t) return true;
    uint32_t action = MS_type_action((uint32_t)t->kind);
    if (action == 4 || (action == 2 && mutation_borrowed_type(t, seen))) return true;
  }
  return false;
}
static bool mutation_allocates_string(const Expr *e) {
  if (!e) return false;
  if (!e->type || MS_string_cast((uint32_t)e->kind, (uint32_t)e->type->kind)) return true;
  if (mutation_allocates_string(e->lhs) || mutation_allocates_string(e->rhs) || mutation_allocates_string(e->third)) return true;
  for (const Expr *arg : e->args) if (mutation_allocates_string(arg)) return true;
  for (const FieldInit &f : e->fields) if (mutation_allocates_string(f.value)) return true;
  return false;
}
static bool mutation_allocates_string(const Stmt *s) {
  if (!s) return false;
  if (mutation_allocates_string(s->expr) || mutation_allocates_string(s->expr2) || mutation_allocates_string(s->target)) return true;
  for (const Stmt *child : s->body) if (mutation_allocates_string(child)) return true;
  return mutation_allocates_string(s->then_) || mutation_allocates_string(s->else_);
}
static bool mutation_allocates_string(const File *file) {
  for (const ProcDecl *p : file->procs) if (mutation_allocates_string(p->body)) return true;
  for (const CheckDecl *c : file->checks) {
    if (mutation_allocates_string(c->proc->body)) return true;
    for (const Binding &b : c->domains)
      if (mutation_allocates_string(b.lo) || mutation_allocates_string(b.hi)) return true;
  }
  return false;
}

static int mutate_command(Arena *arena, File *file, bool reference, bool stats, bool trace) {
  if (file->checks.count == 0) {
    fprintf(stderr, "coax: %s has no #test or #forall to mutate against\n", file->src->path.c_str());
    return 1;
  }
  std::vector<uint64_t> base;
  std::vector<std::vector<const ProcDecl *>> paths;
  bool unstable = false;
  if (!run_all_checks(file, TEST_STEPS, false, &base, nullptr, &paths, &unstable)) {
    fprintf(stderr, "coax: the checks fail before any mutation; make them pass first\n");
    return 1;
  }
  Mutator m{arena};
  for (const CheckDecl *c : file->checks) m.calls(c->proc->body);
  // Mutate in source order, procedure by procedure.
  for (ProcDecl *p : file->procs) {
    if (std::find(m.reached.begin(), m.reached.end(), p) == m.reached.end()) continue;
    size_t first = m.mutants.size();
    m.stmt(p->body);
    for (size_t i = first; i < m.mutants.size(); ++i) m.mutants[i].owner = p;
  }
  bool borrowed = mutation_borrowed_aggregate(file);
  bool strings = mutation_allocates_string(file);
  bool selective = MS_selective(reference, borrowed, strings, unstable);
  uint64_t invocations = 0, skipped = 0;
  size_t mutant_index = 0;
  std::vector<const Mutant *> survivors;
  size_t equivalent = 0;
  for (const Mutant &mu : m.mutants) {
    mu.apply();
    bool killed = false;
    // A mutant that loops forever is killed by a budget well above what the
    // unmutated check needed.
    int32_t killer = -1;
    for (int32_t i = 0; i < file->checks.count && !killed; i++) {
      const auto &path = paths[(size_t)i];
      bool entered = !mu.owner || std::find(path.begin(), path.end(), mu.owner) != path.end();
      if (!MS_run(selective, entered)) { ++skipped; continue; }
      uint64_t budget = std::max<uint64_t>(10000, base[(size_t)i] * 10);
      ++invocations;
      killed = !run_check(file, file->checks[i], budget).passed;
      if (killed) killer = i;
    }
    if (trace) fprintf(stderr, "mutation-outcome %zu %d\n", mutant_index, killer);
    ++mutant_index;
    mu.undo();
    if (killed) continue;
    if (marked_equivalent(file->src, mu.loc)) equivalent++;
    else survivors.push_back(&mu);
  }
  for (const Mutant *mu : survivors) {
    Loc loc = diagnostic_loc(file->src, mu->loc);
    auto point = source_point(file->src, loc);
    printf("%s:%d:%d: mutant survived: %s\n", point.path.c_str(), point.loc.line, point.loc.col, mu->what.c_str());
    show_source(stdout, file->src, mu->loc);
  }
  std::string unreached;
  for (const ProcDecl *p : file->procs) {
    if (!SU_check_required(p->authority,p->root_source,true) || p->foreign || str_eq(p->name, "main")) continue;
    if (std::find(m.reached.begin(), m.reached.end(), p) == m.reached.end())
      unreached += (unreached.empty() ? "" : ", ") + to_std(p->name);
  }
  if (!unreached.empty()) printf("not reached by any check: %s\n", unreached.c_str());
  size_t total = m.mutants.size();
  printf("%zu mutant%s, %zu killed, %zu survived", total, total == 1 ? "" : "s", total - survivors.size() - equivalent,
         survivors.size());
  if (equivalent) printf(", %zu marked equivalent", equivalent);
  printf("\n");
  if (m.mutants.empty()) printf("no reachable mutation sites\n");
  if (stats) fprintf(stderr, "mutation-scheduling %s checks=%llu skipped=%llu borrowed=%d strings=%d unstable=%d\n",
                     selective ? "selective" : "full", (unsigned long long)invocations,
                     (unsigned long long)skipped, borrowed, strings, unstable);
  return survivors.empty() && unreached.empty() && !m.mutants.empty() ? 0 : 1;
}

}  // namespace coax
