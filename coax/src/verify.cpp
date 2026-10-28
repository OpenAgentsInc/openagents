// coax test and coax mutate. Checks (#test, #forall) run in the evaluator,
// never in generated code, each with its own step budget. Mutation testing
// checks the checks: it changes the program one operator, literal, condition
// or statement at a time and requires some check to fail. A mutant that
// survives is code the checks do not pin down, unless it is equivalent: a
// change no input can observe, such as `x < lo` becoming `x <= lo` before
// `return lo`. A reviewer marks such a line with the comment
// `// mutant: equivalent`; mutants on it that survive are counted apart.
#include "coax.h"

#include <algorithm>
#include <functional>

namespace coax {

static constexpr uint64_t TEST_STEPS = 20000000;

// Runs every check in order. Failures print as diagnostics with the check's
// name and, for a #forall, the counterexample. verbose: one line per check.
static bool run_all_checks(File *file, uint64_t max_steps, bool verbose, std::vector<uint64_t> *steps = nullptr) {
  bool ok = true;
  for (CheckDecl *c : file->checks) {
    CheckOutcome r = run_check(file, c, max_steps);
    if (steps) steps->push_back(r.steps);
    const char *kind = c->forall ? "#forall" : "#test";
    std::string name = to_std(c->name);
    if (r.passed) {
      if (verbose)
        printf("ok   %s %s (%llu case%s, %llu steps)\n", kind, name.c_str(), (unsigned long long)r.cases,
               r.cases == 1 ? "" : "s", (unsigned long long)r.steps);
      continue;
    }
    ok = false;
    if (verbose) {
      printf("FAIL %s %s\n", kind, name.c_str());
      fflush(stdout);
    }
    error_at(file->src, r.loc, r.code.c_str(), r.message);
    if (!g_json_errors) {
      fprintf(stderr, "  note: in %s %s\n", kind, name.c_str());
      if (!r.counterexample.empty()) fprintf(stderr, "  counterexample: %s\n", r.counterexample.c_str());
    }
  }
  return ok;
}

static int test_command(File *file, uint64_t max_steps) {
  int32_t before = g_error_count;
  bool ok = run_all_checks(file, max_steps, true);
  int32_t failed = g_error_count - before;
  int32_t total = file->checks.count;
  printf("%d check%s, %d passed, %d failed\n", total, total == 1 ? "" : "s", total - failed, failed);
  return ok ? 0 : 1;
}

// ---- mutation ----

struct Mutant {
  Loc loc;
  std::string what;
  std::function<void()> apply, undo;
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
    if (!e) return;
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
  const std::string &t = src->text;
  size_t start = std::min((size_t)loc.offset, t.size());
  while (start > 0 && t[start - 1] != '\n') start--;
  size_t end = t.find('\n', start);
  return t.substr(start, end == std::string::npos ? std::string::npos : end - start).find("// mutant: equivalent") !=
         std::string::npos;
}

static int mutate_command(Arena *arena, File *file) {
  if (file->checks.count == 0) {
    fprintf(stderr, "coax: %s has no #test or #forall to mutate against\n", file->src->path.c_str());
    return 1;
  }
  std::vector<uint64_t> base;
  if (!run_all_checks(file, TEST_STEPS, false, &base)) {
    fprintf(stderr, "coax: the checks fail before any mutation; make them pass first\n");
    return 1;
  }
  Mutator m{arena};
  for (const CheckDecl *c : file->checks) m.calls(c->proc->body);
  // Mutate in source order, procedure by procedure.
  for (ProcDecl *p : file->procs)
    if (std::find(m.reached.begin(), m.reached.end(), p) != m.reached.end()) m.stmt(p->body);
  std::vector<const Mutant *> survivors;
  size_t equivalent = 0;
  for (const Mutant &mu : m.mutants) {
    mu.apply();
    bool killed = false;
    // A mutant that loops forever is killed by a budget well above what the
    // unmutated check needed.
    for (int32_t i = 0; i < file->checks.count && !killed; i++) {
      uint64_t budget = std::max<uint64_t>(10000, base[(size_t)i] * 10);
      killed = !run_check(file, file->checks[i], budget).passed;
    }
    mu.undo();
    if (killed) continue;
    if (marked_equivalent(file->src, mu.loc)) equivalent++;
    else survivors.push_back(&mu);
  }
  for (const Mutant *mu : survivors) {
    printf("%s:%d:%d: mutant survived: %s\n", file->src->path.c_str(), mu->loc.line, mu->loc.col, mu->what.c_str());
    show_source(stdout, file->src, mu->loc);
  }
  std::string unreached;
  for (const ProcDecl *p : file->procs) {
    if (p->foreign || str_eq(p->name, "main")) continue;
    if (std::find(m.reached.begin(), m.reached.end(), p) == m.reached.end())
      unreached += (unreached.empty() ? "" : ", ") + to_std(p->name);
  }
  if (!unreached.empty()) printf("not reached by any check: %s\n", unreached.c_str());
  size_t total = m.mutants.size();
  printf("%zu mutant%s, %zu killed, %zu survived", total, total == 1 ? "" : "s", total - survivors.size() - equivalent,
         survivors.size());
  if (equivalent) printf(", %zu marked equivalent", equivalent);
  printf("\n");
  return survivors.empty() ? 0 : 1;
}

}  // namespace coax
