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
#include <atomic>
#include <chrono>
#include <cerrno>
#include <functional>
#include <new>
#include <sstream>
#include <sched.h>
#include <csignal>
#include <sys/mman.h>
#ifdef __linux__
#include <sys/prctl.h>
#endif
#include <sys/wait.h>
#include <poll.h>
#include <fcntl.h>
#include <cstring>
#include <set>
#include <filesystem>
#include <map>
#include <unordered_map>
#include <sys/stat.h>
#include <unistd.h>

namespace coax {
// The Coax policy's closed type/expr observations use this ABI mapping.
static_assert((uint8_t)TypeKind::invalid == 0 && (uint8_t)TypeKind::string_ == 5 &&
              (uint8_t)TypeKind::struct_ == 8 && (uint8_t)TypeKind::array == 10 &&
              (uint8_t)TypeKind::view == 11 && (uint8_t)ExprKind::cast == 14 &&
              (uint8_t)ExprKind::ct_value == 17, "mutation scheduling kind ABI");

static constexpr uint64_t TEST_STEPS = 20000000;

// coax test/mutate --evidence FILE (coax.evidence.v1): the claims this run
// made and how each ended. Outcomes and the run's verdict come from the
// checked policy (EV_check, EV_add, EV_verdict, EV_mutation in
// lib/compiler/mutation-schedule.coax); main.cpp writes the record.
struct EvidenceClaim {
  std::string kind, name, code, message, counterexample;
  uint32_t outcome = 4;
  uint64_t cases = 0, steps = 0;
  Loc loc;
};
static std::string json_escape_basic(const std::string &s) {
  std::string out;
  for (unsigned char c : s) {
    if (c == '"' || c == '\\') { out += '\\'; out += (char)c; }
    else if (c < 0x20) { char b[8]; snprintf(b, sizeof b, "\\u%04x", c); out += b; }
    else out += (char)c;
  }
  return out;
}
struct Evidence {
  std::string path;
  std::vector<EvidenceClaim> claims;
  bool mutation = false;  // the run makes the mutation claim
  uint32_t mutation_outcome = 4;
  std::string mutation_detail;  // JSON object fields, without braces
  bool infrastructure = false;
} g_evidence;
static void evidence_claim(EvidenceClaim c) {
  if (!g_evidence.path.empty()) g_evidence.claims.push_back(std::move(c));
}

// Runs every check in order. Failures print as diagnostics with the check's
// name and, for a #forall, the counterexample. verbose: one line per check,
// written as each check ends, with its time. only: just the checks whose
// name contains it (coax test --only); the #authority coverage check needs
// every check, so a filtered run skips it.
// --profile steps: after a check's line, the procedures that used the most
// of its steps: own steps (charged while innermost), their share of the
// check's steps, total steps with the calls they make, and calls.
static constexpr size_t PROFILE_ROWS = 8;
static void print_step_profile(const File *file, const CheckOutcome &r) {
  const uint64_t all = r.steps ? r.steps : 1;
  printf("     %12s %6s %12s %9s  procedure\n", "own steps", "share", "with calls", "calls");
  for (size_t i = 0; i < r.profile.size() && i < PROFILE_ROWS; i++) {
    const StepProfileRow &row = r.profile[i];
    std::string where = "(the check's own body)";
    if (row.proc) {
      SourcePoint at = source_point(file->src, row.proc->loc);
      where = to_std(row.proc->name) + "  " + at.path + ":" + std::to_string(at.loc.line);
    }
    printf("     %12llu %5.1f%% %12llu %9llu  %s\n", (unsigned long long)row.own, 100.0 * (double)row.own / (double)all,
           (unsigned long long)row.total, (unsigned long long)row.calls, where.c_str());
  }
  if (r.profile.size() > PROFILE_ROWS) printf("     ... %zu more procedures\n", r.profile.size() - PROFILE_ROWS);
  fflush(stdout);
}

// A check that already passed in a child process (parallel_checks): its steps,
// cases, time and the procedures it entered, as run_check reported them there.
struct PassedCheck {
  bool passed = false;
  uint64_t steps = 0, cases = 0, budget = 0;
  double seconds = 0;
  bool reused = false;  // taken from the check cache (ax-check-reuse), not run
  std::vector<const ProcDecl *> executed;
};

// passed: the checks that already passed in a child process (parallel_checks);
// each is taken as its child reported it, and every other check runs here.
static bool run_all_checks(File *file, uint64_t max_steps, bool verbose, std::vector<uint64_t> *steps = nullptr, int32_t *failures = nullptr,
                           std::vector<std::vector<const ProcDecl *>> *paths = nullptr, bool *unstable = nullptr,
                           const std::string *only = nullptr, int32_t *ran = nullptr, bool profile = false,
                           const std::vector<PassedCheck> *passed = nullptr) {
  bool ok = true;
  std::vector<const ProcDecl *> executed;
  if (failures) *failures = 0;
  if (ran) *ran = 0;
  int32_t index = -1;
  for (CheckDecl *c : file->checks) {
    ++index;
    if (only && to_std(c->name).find(*only) == std::string::npos) {
      evidence_claim({c->forall ? "#forall" : "#test", to_std(c->name), "", "excluded by --only", "", EV_check(false, false, false), 0, 0, c->loc});
      continue;
    }
    if (ran) ++*ran;
    const auto slots = file->ct_memory->slots, containers = file->ct_memory->containers;
    const auto began = std::chrono::steady_clock::now();
    // A check's own #steps budget replaces the file's.
    const PassedCheck *child = passed && (size_t)index < passed->size() && (*passed)[(size_t)index].passed ? &(*passed)[(size_t)index] : nullptr;
    CheckOutcome r;
    if (child) {
      r.passed = true;
      r.steps = child->steps;
      r.cases = child->cases;
      r.budget = child->budget;
      r.executed = child->executed;
    } else r = run_check(file, c, c->steps ? c->steps : max_steps, profile);
    const double seconds = child ? child->seconds : seconds_since(began);
    const long long ms = (long long)(seconds * 1000);
    if (g_timings.on())
      g_timings.checks.push_back({c->forall ? "#forall" : "#test", to_std(c->name), g_timings.check_phase, seconds, r.steps, r.cases, r.passed, child && child->reused});
    if (paths) paths->push_back(r.executed);
    if (unstable && MS_storage_changed(slots, containers, file->ct_memory->slots, file->ct_memory->containers)) *unstable = true;
    if (steps) steps->push_back(r.steps);
    executed.insert(executed.end(), r.executed.begin(), r.executed.end());
    const char *kind = c->forall ? "#forall" : "#test";
    std::string name = to_std(c->name);
    evidence_claim({kind, name, r.passed ? "" : r.code, r.passed ? "" : r.message, r.counterexample,
                    EV_check(true, r.passed, !r.passed && r.code == "E0400"), r.cases, r.steps, r.passed ? c->loc : r.loc});
    if (r.passed) {
      if (verbose) {
        printf("ok   %s %s (%llu case%s, %llu steps, %lld ms)\n", kind, name.c_str(), (unsigned long long)r.cases,
               r.cases == 1 ? "" : "s", (unsigned long long)r.steps, ms);
        fflush(stdout);
        if (profile) print_step_profile(file, r);
      }
      continue;
    }
    ok = false;
    if (failures) ++*failures;
    if (verbose) {
      if (r.over_budget)
        printf("FAIL %s %s (out of steps: %llu used, its budget is %llu, %lld ms)\n", kind, name.c_str(), (unsigned long long)r.steps,
               (unsigned long long)r.budget, ms);
      else printf("FAIL %s %s (%lld ms)\n", kind, name.c_str(), ms);
      fflush(stdout);
      if (profile) print_step_profile(file, r);
    }
    std::string message = r.message;
    // An exhausted budget names the check's budget, where it comes from and how to raise it.
    if (r.over_budget)
      message = "compile-time evaluation exceeded " + std::to_string(r.budget) + " steps, the check's budget (" +
                (c->steps ? "its own #steps" : "the file's: coax test --steps N, or #steps N on the check for its own") + ")";
    diagnostic_note(file->src, c->loc, true, "in " + std::string(kind) + " " + name);
    if (!r.counterexample.empty()) diagnostic_note(file->src, c->loc, false, "counterexample: " + r.counterexample);
    if (g_json_errors && !g_diagnostics_json) {
      message += " [" + std::string(kind) + " " + name + "]";
      if (!r.counterexample.empty()) message += "; counterexample: " + r.counterexample;
    }
    error_at(file->src, r.loc, r.code.c_str(), message);
    if (!g_json_errors) {
      fprintf(stderr, "  note: in %s %s\n", kind, name.c_str());
      if (!r.counterexample.empty()) fprintf(stderr, "  counterexample: %s\n", r.counterexample.c_str());
    }
  }
  if (file->authority) {
    // Execution coverage is a claim of its own: it needs every check to pass.
    bool covered = true;
    std::string missing;
    if (ok && !only) {
      for (const ProcDecl *p : file->procs) {
        if (!SU_check_required(p->authority,p->root_source,false) || p->foreign || str_eq(p->name, "main")) continue;
        if (std::find(executed.begin(), executed.end(), p) == executed.end()) {
          error_at(file->src, p->loc, "E0380", "'" + to_std(p->name) + "' was never executed by a check",
                   "calls in dead branches or pre-folded #run results do not exercise the procedure");
          ok = false;
          covered = false;
          missing += (missing.empty() ? "" : ", ") + to_std(p->name);
        }
      }
    }
    bool ran_coverage = !only && (ok || !covered);
    evidence_claim({"#authority", "execution coverage", covered ? "" : "E0380",
                    !ran_coverage ? (only ? "needs every check (--only)" : "a check failed first") : covered ? "" : "never executed: " + missing, "",
                    EV_check(ran_coverage, covered, false), 0, 0, Loc{}});
  }
  return ok;
}

// coax test [--only SUBSTR] [--list]: every check, or those whose name
// contains SUBSTR; --list names them (kind and name, one a line) and runs none.
static int test_command(File *file, uint64_t max_steps, const char *only, bool list, bool profile = false) {
  if (!file->checks.count) {
    fprintf(stderr, "coax: %s has no #test or #forall to run\n", file->src->path.c_str());
    return 1;
  }
  const std::string filter = only ? only : "";
  if (list) {
    for (CheckDecl *c : file->checks) {
      std::string name = to_std(c->name);
      if (!only || name.find(filter) != std::string::npos) printf("%s %s\n", c->forall ? "#forall" : "#test", name.c_str());
    }
    return 0;
  }
  int32_t failed = 0, ran = 0;
  bool ok = run_all_checks(file, max_steps, true, nullptr, &failed, nullptr, nullptr, only ? &filter : nullptr, &ran, profile);
  int32_t total = file->checks.count;
  if (only) {
    if (!ran) {
      fprintf(stderr, "coax: no check in %s has '%s' in its name (coax test --list names them)\n", file->src->path.c_str(), only);
      return 1;
    }
    printf("%d of %d check%s (--only '%s'), %d passed, %d failed\n", ran, total, total == 1 ? "" : "s", only, ran - failed, failed);
    if (file->authority) printf("authority execution coverage not checked: it needs every check (run coax test without --only)\n");
  } else {
    printf("%d check%s, %d passed, %d failed\n", total, total == 1 ? "" : "s", total - failed, failed);
    if (!ok && !failed) printf("authority execution coverage failed\n");
  }
  fflush(stdout);
  return ok ? 0 : 1;
}

// ---- mutation ----

struct Mutant {
  Loc loc;
  std::string what;
  std::function<void()> apply, undo;
  const ProcDecl *owner = nullptr;
  bool spatial = false;
  Tok from = Tok::eof, to = Tok::eof;
};

struct Mutator {
  Arena *arena;
  std::vector<Mutant> mutants;
  std::vector<const ProcDecl *> reached;
  size_t spatial_eligible = 0, spatial_no_alternative = 0, spatial_unsupported = 0;
  bool invalid_spatial = false;

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
    for (const SwitchCase *c : s->cases) { calls(c->enum_pattern); calls(c->body); }
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
    if(e->lowering) {
      if(e->kind!=ExprKind::call || (e->lowering!=1 && e->lowering!=2)) invalid_spatial=true;
      else if(e->lowering==2 && e->mutation_status==3) ++spatial_unsupported;
      else if(e->lowering==1 && e->mutation_status==2) ++spatial_no_alternative;
      else if(e->lowering==1 && e->mutation_status==1) {
        auto *from=e->proc,*to=e->mutation_proc;
        bool shape=from && to && from!=to && to->ret==e->type && from->ret==e->type && to->params.count==e->args.count && from->params.count==e->args.count && e->mutation_op!=Tok::eof && e->mutation_op!=e->op;
        if(shape)for(int i=0;i<e->args.count;i++)shape=shape && e->args[i] && to->params[i].type==e->args[i]->type && from->params[i].type==e->args[i]->type;
        if(!shape)invalid_spatial=true;
        else {++spatial_eligible;mutants.push_back({e->loc,"spatial source '"+spell(e->op)+"' became '"+spell(e->mutation_op)+"'",[e,to]{e->proc=to;},[e,from]{e->proc=from;},nullptr,true,e->op,e->mutation_op});}
      }else invalid_spatial=true;
    }
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
    for (SwitchCase *c : s->cases) { expr(c->enum_pattern); stmt(c->body); }
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
  for (const SwitchCase *c : s->cases)
    if (mutation_allocates_string(c->enum_pattern) || mutation_allocates_string(c->body)) return true;
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

// ---- parallel and sharded mutation ----
//
// The mutant set, its order and each mutant's outcome do not depend on who
// evaluates it. Every check runs in a fresh evaluator, so a worker process
// forked after the baseline evaluates the same mutant against the same
// syntax tree and step budgets as the serial loop. Workers write outcomes
// into shared memory by global index; the parent prints the report in
// source order after all of them finish, so the bytes match a serial run.

struct MutationOptions {
  bool reference = false, stats = false, trace = false, root_only = false;
  uint64_t jobs = 0;               // 0: available cores (or $COAX_MUTATE_JOBS)
  uint64_t shard = 0, shards = 0;  // 0/0: the whole set
  std::vector<std::string> merge;  // shard records to combine instead of running
};

struct MutantResult {
  int32_t killer = -1;  // the first check that failed, -1 for a survivor
  int32_t done = 0;
  uint64_t invocations = 0, skipped = 0;
};

struct MutationRun {
  File *file;
  const std::vector<Mutant> *mutants;
  const std::vector<uint64_t> *base;
  const std::vector<std::vector<const ProcDecl *>> *paths;
  bool selective;
};

static MutantResult evaluate_mutant(const MutationRun &run, size_t index) {
  const Mutant &mu = (*run.mutants)[index];
  File *file = run.file;
  MutantResult r;
  mu.apply();
  bool killed = false;
  // A mutant that loops forever is killed by a budget well above what the
  // unmutated check needed.
  for (int32_t i = 0; i < file->checks.count && !killed; i++) {
    const auto &path = (*run.paths)[(size_t)i];
    bool entered = !mu.owner || std::find(path.begin(), path.end(), mu.owner) != path.end();
    if (!MS_run(run.selective, entered)) { ++r.skipped; continue; }
    uint64_t budget = std::max<uint64_t>(10000, (*run.base)[(size_t)i] * 10);
    // A mutant that needs more than the check's own budget fails that check.
    if (file->checks[i]->steps) budget = std::min(budget, file->checks[i]->steps);
    ++r.invocations;
    killed = !run_check(file, file->checks[i], budget).passed;
    if (killed) r.killer = i;
  }
  mu.undo();
  r.done = 1;
  return r;
}

static uint64_t available_cores() {
#ifdef __linux__
  cpu_set_t set;
  CPU_ZERO(&set);
  if (sched_getaffinity(0, sizeof set, &set) == 0 && CPU_COUNT(&set) > 0) return (uint64_t)CPU_COUNT(&set);
#endif
  long n = sysconf(_SC_NPROCESSORS_ONLN);
  return n > 0 ? (uint64_t)n : 1;
}

static bool parse_count(const char *text, uint64_t *out) {
  if (!text || !*text) return false;
  uint64_t v = 0;
  for (const char *p = text; *p; p++) {
    if (*p < '0' || *p > '9' || v > 100000000) return false;
    v = v * 10 + (uint64_t)(*p - '0');
  }
  *out = v;
  return true;
}

// How many checks run at once in forked children: the core count, or fewer
// when $COAX_CHECK_JOBS says so. Shared by parallel #authority checks and
// the standalone library checks (ax-library-check-cache); either way the
// result is the sequential one.
static uint64_t check_jobs() {
  uint64_t jobs = 0;
  const char *env = getenv("COAX_CHECK_JOBS");
  if (!env || !parse_count(env, &jobs) || !jobs) jobs = available_cores();
  return std::max<uint64_t>(1, std::min<uint64_t>(jobs, available_cores()));
}

// Reusing passing checks one at a time (ax-check-reuse). When an #authority
// compile's whole-program verdict isn't in the check cache (an edit since the
// last pass), parallel_checks still takes each check that passed before with
// the same inputs from the cache instead of running it. A check's inputs are
// the declarations its result can depend on, found by name from its own
// tokens:
//   - its own body, in full;
//   - every procedure it, or a procedure it ran, names: in full when the
//     check ran it, and only its signature (parameter and result types, up
//     to its body) when it didn't, since then only overload resolution
//     reads it;
//   - every other declaration so named (constants, structs, enums, unions,
//     and what their tokens name, with the procedures in full: they may run
//     at compile time), and everything a body holding #run names, in full;
//   - every token outside a named declaration (#load, #module, #import,
//     #authority, top-level #run and #assert ...), and the units' paths.
// A name is matched to every top-level declaration that spells it, in any
// unit, so a new declaration a check's code could bind to is a new input.
// The procedures a check ran come from the run that recorded it: if every
// input is the same, the evaluator takes the same steps and runs the same
// procedures. The key also holds the compiler's SHA-256, the options that
// change checking, the units left out as libraries, the step budget, the
// storage limits and the compile-time storage every check starts with.
// Content is the tokens (kind, spelling, value), so comments and moving a
// declaration change nothing. A failing check is never recorded. The record
// keeps the check's steps, cases, peak and final storage and the procedures it
// ran, so a reused check stands in for a child's report and the rest of
// parallel_checks (the sequential-equivalence rules, E0380 coverage) is
// unchanged. Declarations are found by splitting the tokens at depth-0 `;`
// and `}`, and that split is checked against the parser's: each declaration
// must start an item that spells its name and no item may hold two. When it
// isn't, or a procedure a check ran can't be named, nothing is reused or
// recorded for that check (or that program).
static bool read_record(const std::string &path, std::string *out);
struct CheckReuse {
  bool on = false;
  std::string dir, prefix;  // prefix: the compiler, the options and the left-out units, as a key's first lines
  const Source *src = nullptr;
  const std::vector<Token> *tokens = nullptr;
  size_t reused = 0, ran = 0;
};
static CheckReuse g_check_reuse;

// Marks a cache record as just used, so the cache evicts the least recently used first.
static void cache_touch(const std::string &path) { utimensat(AT_FDCWD, path.c_str(), nullptr, 0); }
static void check_cache_used(const std::string &dir);  // main.cpp: evicts old records when this process ends

struct ReuseIndex {
  enum : uint8_t { NONE, SIG, EXEC, FULL };
  struct Item {
    int32_t first = 0, last = -1, sig_last = -1;  // tokens [first, last]; a procedure's signature is [first, sig_last]
    int32_t unit = -1;
    std::string name, id;
    bool named = false, proc = false, check = false, run = false, sig_run = false;  // run: its tokens hold #run (sig_run: its signature's)
    std::vector<int32_t> uses, sig_uses;  // the named items its tokens (its signature's) spell
    std::string full, sig;                 // digests, when first needed
  };
  const Source *src = nullptr;
  const std::vector<Token> *tokens = nullptr;
  std::vector<Item> items;
  std::string global;
  std::map<const ProcDecl *, std::string> proc_id;
  std::map<std::string, const ProcDecl *> proc_by_id;
  std::map<const ProcDecl *, int32_t> proc_item;
  std::map<const CheckDecl *, int32_t> check_item;
  std::string why;  // why nothing is reused in this program ("" when the index is sound)

  static void put(std::string &out, const Token &t) {
    out += (char)t.kind;
    auto str = [&](Str s) { uint32_t n = (uint32_t)s.count; out.append((const char *)&n, sizeof n); out.append(s.data, (size_t)s.count); };
    str(t.text);
    if (t.kind == Tok::int_lit || t.kind == Tok::float_lit || t.kind == Tok::string_lit) {
      out.append((const char *)&t.int_value, sizeof t.int_value);
      out.append((const char *)&t.float_value, sizeof t.float_value);
      out += (char)t.float_bits;
      str(t.string_value);
    }
  }
  std::string digest(int32_t first, int32_t last) const {
    std::string bytes;
    for (int32_t k = first; k <= last; k++) put(bytes, (*tokens)[(size_t)k]);
    return debug_source_hash(bytes);
  }
  const std::string &full_digest(int32_t k) { Item &it = items[(size_t)k]; if (it.full.empty()) it.full = digest(it.first, it.last); return it.full; }
  const std::string &sig_digest(int32_t k) { Item &it = items[(size_t)k]; if (it.sig.empty()) it.sig = digest(it.first, it.sig_last); return it.sig; }

  // The item whose tokens span offset, or -1.
  int32_t item_at(int32_t offset) const {
    size_t lo = 0, hi = items.size();
    while (lo < hi) {
      size_t mid = (lo + hi) / 2;
      if ((*tokens)[(size_t)items[mid].first].loc.offset <= offset) lo = mid + 1; else hi = mid;
    }
    if (lo == 0) return -1;
    const Item &it = items[lo - 1];
    return offset <= (*tokens)[(size_t)it.last].loc.offset ? (int32_t)(lo - 1) : -1;
  }

  bool build(const File *file) {
    const std::vector<Token> &tk = *tokens;
    // Units by where they begin in the composed text.
    std::vector<std::pair<int32_t, int32_t>> starts;
    for (size_t u = 0; u < src->units.size(); u++) starts.push_back({src->units[u]->begin, (int32_t)u});
    std::sort(starts.begin(), starts.end());
    auto unit_of = [&](int32_t offset) -> int32_t {
      auto at = std::upper_bound(starts.begin(), starts.end(), std::make_pair(offset, INT32_MAX));
      if (at == starts.begin()) return -1;
      const int32_t u = (at - 1)->second;
      return offset < src->units[(size_t)u]->end ? u : -1;
    };
    // Items: a run of tokens that ends at a depth-0 `;`, or at the `}` that
    // returns to depth 0 (with a `;` straight after it), or where its unit ends.
    int32_t last_offset = -1;
    for (size_t i = 0; i < tk.size();) {
      if (tk[i].kind == Tok::eof) { i++; continue; }
      Item it;
      it.first = (int32_t)i;
      it.unit = unit_of(tk[i].loc.offset);
      int depth = 0;
      size_t j = i;
      for (; j < tk.size(); j++) {
        const Token &t = tk[j];
        if (t.kind == Tok::eof || (j > i && unit_of(t.loc.offset) != it.unit)) break;
        if (t.loc.offset <= last_offset) { why = "tokens out of order"; return false; }
        last_offset = t.loc.offset;
        if (t.kind == Tok::lparen || t.kind == Tok::lbracket || t.kind == Tok::lbrace) depth++;
        else if (t.kind == Tok::rparen || t.kind == Tok::rbracket || t.kind == Tok::rbrace) depth--;
        if (depth < 0) { why = "unbalanced tokens"; return false; }
        if (depth == 0 && t.kind == Tok::semicolon) { j++; break; }
        if (depth == 0 && t.kind == Tok::rbrace) {
          j++;
          if (j < tk.size() && tk[j].kind == Tok::semicolon && unit_of(tk[j].loc.offset) == it.unit) { last_offset = tk[j].loc.offset; j++; }
          break;
        }
      }
      it.last = (int32_t)j - 1;
      const Token &head = tk[i];
      if (head.kind == Tok::ident) { it.named = true; it.name = to_std(head.text); }
      if (head.kind == Tok::directive && (str_eq(head.text, "test") || str_eq(head.text, "forall"))) it.check = true;
      for (int32_t k = it.first; k <= it.last; k++) it.run = it.run || (tk[(size_t)k].kind == Tok::directive && str_eq(tk[(size_t)k].text, "run"));
      items.push_back(std::move(it));
      i = j;
    }
    // The parser's declarations must each start an item that spells their
    // name, one an item; a procedure with a body has its signature before it.
    std::vector<const void *> owner(items.size(), nullptr);
    auto claim = [&](const void *decl, Loc loc, const Str *name) -> int32_t {
      const int32_t k = item_at(loc.offset);
      if (k < 0) { why = "a declaration outside every item"; return -1; }
      if (owner[(size_t)k] && owner[(size_t)k] != decl) { why = "two declarations in one item"; return -1; }
      owner[(size_t)k] = decl;
      if (name && (!items[(size_t)k].named || items[(size_t)k].name != to_std(*name))) { why = "an item that doesn't spell its declaration's name"; return -1; }
      return k;
    };
    auto procedure = [&](const ProcDecl *p) -> bool {
      const int32_t k = claim(p, p->loc, &p->name);
      if (k < 0) return false;
      Item &it = items[(size_t)k];
      if (p->foreign || !p->body) return true;
      // The body is the brace group that ends the item, and holds the body's location.
      int32_t close = it.last;
      if (tk[(size_t)close].kind == Tok::semicolon) close--;
      if (close <= it.first || tk[(size_t)close].kind != Tok::rbrace) return true;
      int depth = 0;
      for (int32_t t = close; t > it.first; t--) {
        const Tok kind = tk[(size_t)t].kind;
        if (kind == Tok::rparen || kind == Tok::rbracket || kind == Tok::rbrace) depth++;
        else if (kind == Tok::lparen || kind == Tok::lbracket || kind == Tok::lbrace) depth--;
        if (depth == 0) {
          if (kind == Tok::lbrace && p->body->loc.offset >= tk[(size_t)t].loc.offset && p->body->loc.offset <= tk[(size_t)close].loc.offset) {
            it.proc = true;
            it.sig_last = t - 1;
          }
          break;
        }
      }
      return true;
    };
    for (const StructDecl *d : file->structs) if (!d->origin && claim(d, d->loc, &d->name) < 0) return false;
    for (const StructDecl *d : file->generic_structs) if (claim(d, d->loc, &d->name) < 0) return false;
    for (const EnumDecl *d : file->enums) if (claim(d, d->loc, &d->name) < 0) return false;
    for (const UnionDecl *d : file->unions) if (claim(d, d->loc, &d->name) < 0) return false;
    for (const ConstDecl *d : file->constants) if (claim(d, d->loc, &d->name) < 0) return false;
    for (const ProcDecl *p : file->procs) if (!p->origin && !procedure(p)) return false;
    for (const ProcDecl *p : file->generics) if (!procedure(p)) return false;
    for (const CheckDecl *c : file->checks) {
      const int32_t k = claim(c->proc ? (const void *)c->proc : (const void *)c, c->loc, nullptr);
      if (k < 0) return false;
      if (!items[(size_t)k].check) { why = "a check outside a check item"; return false; }
      check_item[c] = k;
    }
    // Identities: unit, name and which of the unit's items so named.
    std::map<std::pair<int32_t, std::string>, int32_t> seen;
    std::unordered_map<std::string, std::vector<int32_t>> by_name;
    for (size_t k = 0; k < items.size(); k++) {
      Item &it = items[k];
      if (!it.named && !it.check) continue;
      std::string name = it.named ? it.name : "#check";
      if (it.check) {
        for (const auto &[c, at] : check_item) if (at == (int32_t)k) name = std::string(c->forall ? "#forall " : "#test ") + to_std(c->name);
      }
      const int32_t ordinal = seen[{it.unit, name}]++;
      it.id = (it.unit < 0 ? std::string("?") : src->units[(size_t)it.unit]->canonical) + "\x1f" + name + "\x1f" + std::to_string(ordinal);
      if (it.id.find('\n') != std::string::npos) it.id.clear();  // no record can name it
      if (it.named) by_name[it.name].push_back((int32_t)k);
    }
    for (Item &it : items) {
      auto names = [&](int32_t last, std::vector<int32_t> *out) {
        for (int32_t t = it.first; t <= last; t++) {
          if (tk[(size_t)t].kind != Tok::ident) continue;
          auto at = by_name.find(to_std(tk[(size_t)t].text));
          if (at != by_name.end()) out->insert(out->end(), at->second.begin(), at->second.end());
        }
        std::sort(out->begin(), out->end());
        out->erase(std::unique(out->begin(), out->end()), out->end());
      };
      names(it.last, &it.uses);
      if (it.proc) names(it.sig_last, &it.sig_uses);
      for (int32_t t = it.first; it.proc && t <= it.sig_last; t++) it.sig_run = it.sig_run || (tk[(size_t)t].kind == Tok::directive && str_eq(tk[(size_t)t].text, "run"));
    }
    // Every procedure, by its template's item and its instance's bindings.
    for (const ProcDecl *p : file->procs) {
      const ProcDecl *base = p->origin ? p->origin : p;
      const int32_t k = item_at(base->loc.offset);
      if (k < 0 || items[(size_t)k].id.empty()) continue;
      std::string id = items[(size_t)k].id + "\x1f" + (p->origin ? to_std(p->key) : std::string());
      if (id.find('\n') != std::string::npos) continue;
      proc_item[p] = k;
      proc_id[p] = id;
      proc_by_id[id] = p;
    }
    // Everything outside a named item, by unit, and the units.
    std::string bytes;
    for (const auto &unit : src->units) bytes += unit->canonical + "\n";
    for (const Item &it : items) {
      if (it.named || it.check) continue;
      bytes += "\x1e" + std::to_string(it.unit) + "\n";
      for (int32_t t = it.first; t <= it.last; t++) put(bytes, tk[(size_t)t]);
    }
    global = debug_source_hash(bytes);
    if (global.empty()) { why = "no hash in this compiler"; return false; }
    return true;
  }

  // The key of a check's inputs, given the items of the procedures it ran.
  std::string key(int32_t check, const std::set<int32_t> &ran, const std::string &head) {
    std::vector<uint8_t> mode(items.size(), NONE);
    std::vector<int32_t> work{check};
    mode[(size_t)check] = EXEC;
    auto visit = [&](int32_t k, uint8_t m) {
      if (!items[(size_t)k].proc) m = FULL;
      if (m <= mode[(size_t)k]) return;
      mode[(size_t)k] = m;
      work.push_back(k);
    };
    while (!work.empty()) {
      const int32_t k = work.back();
      work.pop_back();
      const Item &it = items[(size_t)k];
      const uint8_t m = mode[(size_t)k];
      // A procedure a declaration, or tokens holding #run, name may run at
      // compile time: all of it. One a signature names is read for its own
      // signature (a default's type); one a body the check ran names, all of
      // it when the check ran it too, else its signature.
      const bool deep = m == FULL || (m == SIG ? it.sig_run : it.run);
      for (int32_t j : m == SIG ? it.sig_uses : it.uses)
        visit(j, !items[(size_t)j].proc || deep ? FULL : m == SIG ? SIG : ran.count(j) ? EXEC : SIG);
    }
    std::string text = "coax.check-reuse.v1\n" + head + "global\t" + global + "\ncheck\t" + items[(size_t)check].id + "\n";
    for (size_t k = 0; k < items.size(); k++) {
      if (!mode[k]) continue;
      if (items[k].id.empty()) return "";
      text += std::string(mode[k] == SIG ? "S\t" : mode[k] == EXEC ? "E\t" : "F\t") + items[k].id + "\t" +
              (mode[k] == SIG ? sig_digest((int32_t)k) : full_digest((int32_t)k)) + "\n";
    }
    return debug_source_hash(text);
  }
};

// The record of one program's reusable checks: one file in the check cache
// under the SHA-256 of its entry unit and the key's first lines, holding a
// table of procedure identities and, for each check that passed, its key and
// what its child reported.
struct ReuseRecord {
  enum { STEPS, CASES, BUDGET, SECONDS, PEAK_SLOTS, PEAK_CONTAINERS, PEAK_STRINGS, END_SLOTS, END_CONTAINERS, END_STRINGS, NUMBERS };
  struct Entry { std::string key; uint64_t numbers[NUMBERS] = {}; std::vector<std::string> ran; };
  std::map<std::string, Entry> entries;  // by the check's identity
  void load(const std::string &path) {
    std::string text;
    if (!read_record(path, &text) || text.compare(0, 20, "coax.check-reuse.v1\n") != 0) return;
    cache_touch(path);
    std::vector<std::string> table;
    for (size_t at = 20; at < text.size();) {
      const size_t end = text.find('\n', at);
      if (end == std::string::npos) break;
      const std::string line = text.substr(at, end - at);
      at = end + 1;
      if (line.compare(0, 2, "p\t") == 0) { table.push_back(line.substr(2)); continue; }
      if (line.compare(0, 2, "c\t") != 0) return;
      std::vector<std::string> f;
      for (size_t s = 2;;) {
        const size_t tab = line.find('\t', s);
        f.push_back(line.substr(s, tab == std::string::npos ? std::string::npos : tab - s));
        if (tab == std::string::npos) break;
        s = tab + 1;
      }
      if (f.size() != 3 + NUMBERS) return;
      Entry e;
      e.key = f[1];
      for (size_t i = 0; i < NUMBERS; i++) e.numbers[i] = strtoull(f[2 + i].c_str(), nullptr, 10);
      const std::string &list = f[2 + NUMBERS];
      for (size_t s = 0; s < list.size();) {
        size_t comma = list.find(',', s);
        if (comma == std::string::npos) comma = list.size();
        const size_t index = strtoull(list.c_str() + s, nullptr, 10);
        if (index >= table.size()) return;
        e.ran.push_back(table[index]);
        s = comma + 1;
      }
      entries[f[0]] = std::move(e);
    }
  }
  bool save(const std::string &dir, const std::string &path) const {
    std::map<std::string, size_t> index;
    std::string table, rows;
    for (const auto &[check, e] : entries) {
      rows += "c\t" + check + "\t" + e.key;
      for (uint64_t v : e.numbers) rows += "\t" + std::to_string(v);
      rows += "\t";
      for (size_t i = 0; i < e.ran.size(); i++) {
        auto at = index.find(e.ran[i]);
        if (at == index.end()) { at = index.emplace(e.ran[i], index.size()).first; table += "p\t" + e.ran[i] + "\n"; }
        rows += (i ? "," : "") + std::to_string(at->second);
      }
      rows += "\n";
    }
    std::error_code ec;
    std::filesystem::create_directories(dir, ec);
    const std::string temp = dir + "/." + path.substr(path.rfind('/') + 1) + "." + std::to_string(getpid());
    FILE *f = fopen(temp.c_str(), "wb");
    if (!f) return false;
    const std::string text = "coax.check-reuse.v1\n" + table + rows;
    const bool ok = fwrite(text.data(), 1, text.size(), f) == text.size();
    if (fclose(f) != 0 || !ok || std::rename(temp.c_str(), path.c_str()) != 0) { unlink(temp.c_str()); return false; }
    return true;
  }
};

// Parallel #authority checks (ax-parallel-checks). A compile of an #authority
// program runs every check it keeps, one after another, in the evaluator.
// parallel_checks runs each check instead in a forked child of this process,
// up to the core count at once ($COAX_CHECK_JOBS caps it), with the same
// evaluator, budgets and limits. A child reports whether its check passed,
// its steps and cases, the procedures it entered (for E0380), its peak
// compile-time storage and what it left behind; it prints nothing. The
// parent then runs run_all_checks as before, taking each passing check from
// its child and running every other check itself, in order, so a failure's
// diagnostics are the ones a sequential run prints.
//
// A sequential run starts each check with the storage the checks before it
// left behind; a child starts with none of it. So parallel_checks returns
// false, and the caller runs every check sequentially as if nothing had been
// tried, when its result might not be the sequential one: a child crashed or
// wrote no whole report; a check changed the evaluator's shared state (frozen
// values, struct defaults); a check's peak storage plus what the checks
// before it left behind passes a storage limit; or a check failed after some
// check left storage behind. It also returns false, without running
// anything, for fewer than two checks, one job, or an --evidence record.
// The struct defaults not yet evaluated: a check that evaluates one changes
// what later checks pay for.
static int64_t defaults_pending(const File *file) {
  int64_t pending = 0;
  for (const StructDecl *d : file->structs) pending += d->defaults != 2;
  return pending;
}

// times: each check's seconds in the last run of this file's checks, by
// name. Checks start longest first (one not timed yet first of all), so the
// slowest doesn't start last; the order changes nothing else. On return it
// holds the seconds this run measured.
static bool parallel_checks(File *file, uint64_t max_steps, std::vector<PassedCheck> *out, std::map<std::string, double> *times) {
  const size_t n = (size_t)file->checks.count;
  auto sequential = [](const char *why) { g_timings.parallel = std::string("sequential: ") + why; return false; };
  if (n < 2) return sequential("fewer than two checks");
  if (!g_evidence.path.empty()) return sequential("an --evidence record");
  const uint64_t jobs = std::min<uint64_t>(check_jobs(), n);
  if (jobs < 2) return sequential("one job");
  CtMemory &memory = *file->ct_memory;
  const uint64_t slots = memory.slots, containers = memory.containers, strings = memory.string_bytes;
  const size_t frozen = file->frozen_values.size();
  const int64_t pending = defaults_pending(file);
  enum { MAGIC, PASSED, STEPS, CASES, BUDGET, SECONDS, PEAK_SLOTS, PEAK_CONTAINERS, PEAK_STRINGS,
         END_SLOTS, END_CONTAINERS, END_STRINGS, SHARED, EXECUTED, FIELDS };
  static constexpr uint64_t REPORT = 0x636f61782d636b31ULL;  // "coax-ck1"
  struct Child { pid_t pid; int fd; size_t index; std::string bytes; };
  std::vector<Child> running;
  std::vector<std::vector<uint64_t>> reports(n);
  bool broken = false;
  std::vector<size_t> order(n);
  std::vector<double> known(n, 1e300);
  for (size_t i = 0; i < n; i++) {
    order[i] = i;
    auto at = times->find(to_std(file->checks[(int32_t)i]->name));
    if (at != times->end()) known[i] = at->second;
  }
  std::stable_sort(order.begin(), order.end(), [&](size_t a, size_t b) { return known[a] > known[b]; });
  // Checks that passed before with the same inputs (ax-check-reuse): their
  // recorded reports stand in for a child's.
  std::vector<bool> prefilled(n, false);
  ReuseIndex index;
  ReuseRecord record;
  std::string record_path, head;
  std::vector<std::string> check_ids(n);
  const bool reuse = g_check_reuse.on && g_check_reuse.tokens && !g_check_reuse.prefix.empty() &&
                     (index.src = g_check_reuse.src, index.tokens = g_check_reuse.tokens, index.build(file));
  if (getenv("COAX_CHECK_REUSE_DEBUG") && g_check_reuse.on && !index.why.empty()) fprintf(stderr, "check reuse off: %s\n", index.why.c_str());
  if (reuse) {
    head = g_check_reuse.prefix + "start\t" + std::to_string(slots) + " " + std::to_string(containers) + " " + std::to_string(strings) + " " +
           std::to_string(frozen) + " " + std::to_string(pending) + " limits " + std::to_string(memory.slot_limit) + " " +
           std::to_string(memory.container_limit) + " " + std::to_string(CB_string_live_limit()) + " steps " + std::to_string(max_steps) + "\n";
    record_path = g_check_reuse.dir + "/" + debug_source_hash("coax.check-reuse-record.v1\n" + g_check_reuse.src->units[0]->canonical + "\n" + g_check_reuse.prefix);
    record.load(record_path);
    for (size_t i = 0; i < n; i++) {
      const auto at = index.check_item.find(file->checks[(int32_t)i]);
      if (at == index.check_item.end() || index.items[(size_t)at->second].id.empty()) continue;
      check_ids[i] = debug_source_hash(index.items[(size_t)at->second].id);
      const auto e = record.entries.find(check_ids[i]);
      if (e == record.entries.end()) continue;
      std::set<int32_t> ran;
      std::vector<const ProcDecl *> procs;
      bool known_all = true;
      for (const std::string &id : e->second.ran) {
        const auto p = index.proc_by_id.find(id);
        if (p == index.proc_by_id.end()) { known_all = false; break; }
        procs.push_back(p->second);
        ran.insert(index.proc_item[p->second]);
      }
      if (!known_all || index.key(at->second, ran, head) != e->second.key) continue;
      const uint64_t *v = e->second.numbers;
      std::vector<uint64_t> report(FIELDS + procs.size());
      report[MAGIC] = REPORT;
      report[PASSED] = 1;
      report[STEPS] = v[ReuseRecord::STEPS];
      report[CASES] = v[ReuseRecord::CASES];
      report[BUDGET] = v[ReuseRecord::BUDGET];
      report[SECONDS] = 0;  // reused: no time spent
      report[PEAK_SLOTS] = v[ReuseRecord::PEAK_SLOTS];
      report[PEAK_CONTAINERS] = v[ReuseRecord::PEAK_CONTAINERS];
      report[PEAK_STRINGS] = v[ReuseRecord::PEAK_STRINGS];
      report[END_SLOTS] = v[ReuseRecord::END_SLOTS];
      report[END_CONTAINERS] = v[ReuseRecord::END_CONTAINERS];
      report[END_STRINGS] = v[ReuseRecord::END_STRINGS];
      report[SHARED] = 0;
      report[EXECUTED] = procs.size();
      for (size_t k = 0; k < procs.size(); k++) report[FIELDS + k] = (uint64_t)(uintptr_t)procs[k];
      reports[i] = std::move(report);
      prefilled[i] = true;
      g_check_reuse.reused++;
    }
  }
  if (g_check_reuse.on) g_check_reuse.ran += n - (size_t)std::count(prefilled.begin(), prefilled.end(), true);
  size_t next = 0;
  fflush(stdout);
  fflush(stderr);
  auto start = [&](size_t index) -> bool {
    if (prefilled[index]) return true;
    int pipe_fds[2];
    if (pipe(pipe_fds) != 0) return false;
    pid_t pid = fork();
    if (pid < 0) { close(pipe_fds[0]); close(pipe_fds[1]); return false; }
    if (pid == 0) {
      close(pipe_fds[0]);
      int null = open("/dev/null", O_WRONLY);
      if (null >= 0) { dup2(null, 1); dup2(null, 2); }
      memory.peak_slots = memory.slots;
      memory.peak_containers = memory.containers;
      memory.string_peak_bytes = memory.string_bytes;
      CheckDecl *c = file->checks[(int32_t)index];
      const auto began = std::chrono::steady_clock::now();
      CheckOutcome r = run_check(file, c, c->steps ? c->steps : max_steps, false);
      const double seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - began).count();
      std::vector<uint64_t> report(FIELDS + r.executed.size());
      uint64_t seconds_bits;
      memcpy(&seconds_bits, &seconds, sizeof seconds_bits);
      report[MAGIC] = REPORT;
      report[PASSED] = r.passed;
      report[STEPS] = r.steps;
      report[CASES] = r.cases;
      report[BUDGET] = r.budget;
      report[SECONDS] = seconds_bits;
      report[PEAK_SLOTS] = memory.peak_slots;
      report[PEAK_CONTAINERS] = memory.peak_containers;
      report[PEAK_STRINGS] = memory.string_peak_bytes;
      report[END_SLOTS] = memory.slots;
      report[END_CONTAINERS] = memory.containers;
      report[END_STRINGS] = memory.string_bytes;
      report[SHARED] = file->frozen_values.size() != frozen || defaults_pending(file) != pending;
      report[EXECUTED] = r.executed.size();
      for (size_t i = 0; i < r.executed.size(); i++) report[FIELDS + i] = (uint64_t)(uintptr_t)r.executed[i];
      const char *at = (const char *)report.data();
      size_t left = report.size() * sizeof(uint64_t);
      while (left) {
        ssize_t w = write(pipe_fds[1], at, left);
        if (w < 0 && errno == EINTR) continue;
        if (w <= 0) _exit(1);
        at += w;
        left -= (size_t)w;
      }
      _exit(0);
    }
    close(pipe_fds[1]);
    running.push_back({pid, pipe_fds[0], index, {}});
    return true;
  };
  while (!broken && (next < n || !running.empty())) {
    while (running.size() < jobs && next < n)
      if (!start(order[next++])) { broken = true; break; }
    if (broken || running.empty()) break;
    std::vector<pollfd> fds;
    for (const Child &c : running) fds.push_back({c.fd, POLLIN, 0});
    if (poll(fds.data(), fds.size(), -1) < 0) {
      if (errno == EINTR) continue;
      broken = true;
      break;
    }
    for (size_t k = running.size(); k-- > 0;) {
      if (!(fds[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      Child &c = running[k];
      char buffer[65536];
      ssize_t got = read(c.fd, buffer, sizeof buffer);
      if (got < 0 && errno == EINTR) continue;
      if (got > 0) { c.bytes.append(buffer, (size_t)got); continue; }
      close(c.fd);
      int status = 0;
      while (waitpid(c.pid, &status, 0) < 0 && errno == EINTR) {}
      if (WIFEXITED(status) && WEXITSTATUS(status) == 0 && c.bytes.size() % sizeof(uint64_t) == 0 &&
          c.bytes.size() >= FIELDS * sizeof(uint64_t)) {
        std::vector<uint64_t> report(c.bytes.size() / sizeof(uint64_t));
        memcpy(report.data(), c.bytes.data(), c.bytes.size());
        if (report[MAGIC] == REPORT && report[EXECUTED] == report.size() - FIELDS) reports[c.index] = std::move(report);
      }
      running.erase(running.begin() + (std::ptrdiff_t)k);
    }
  }
  if (broken) {
    for (Child &c : running) { kill(c.pid, SIGKILL); close(c.fd); int status; waitpid(c.pid, &status, 0); }
    return sequential("a child process could not start");
  }
  // Record each check that passed in a child, and every procedure it ran, for
  // the next compile; drop the records of checks this program no longer has.
  if (reuse) {
    ReuseRecord kept;
    bool changed = false;
    for (size_t i = 0; i < n; i++) {
      if (check_ids[i].empty()) continue;
      const auto old = record.entries.find(check_ids[i]);
      const std::vector<uint64_t> &report = reports[i];
      if (prefilled[i] || report.empty() || !report[PASSED] || report[SHARED]) {
        if (old != record.entries.end()) kept.entries[check_ids[i]] = old->second;
        continue;
      }
      ReuseRecord::Entry e;
      std::set<int32_t> ran;
      bool named = true;
      for (size_t k = FIELDS; k < report.size(); k++) {
        const ProcDecl *p = (const ProcDecl *)(uintptr_t)report[k];
        const auto id = index.proc_id.find(p);
        if (id == index.proc_id.end()) { named = false; break; }
        e.ran.push_back(id->second);
        ran.insert(index.proc_item[p]);
      }
      if (named) e.key = index.key(index.check_item[file->checks[(int32_t)i]], ran, head);
      if (e.key.empty()) {
        if (old != record.entries.end()) kept.entries[check_ids[i]] = old->second;
        continue;
      }
      const size_t from[ReuseRecord::NUMBERS] = {STEPS, CASES, BUDGET, SECONDS, PEAK_SLOTS, PEAK_CONTAINERS, PEAK_STRINGS,
                                                 END_SLOTS, END_CONTAINERS, END_STRINGS};
      for (size_t k = 0; k < ReuseRecord::NUMBERS; k++) e.numbers[k] = report[from[k]];
      kept.entries[check_ids[i]] = std::move(e);
      changed = true;
    }
    if (changed || kept.entries.size() != record.entries.size()) kept.save(g_check_reuse.dir, record_path);
  }
  times->clear();
  for (size_t i = 0; i < n; i++)
    if (!reports[i].empty()) {
      double seconds;
      memcpy(&seconds, &reports[i][SECONDS], sizeof seconds);
      (*times)[to_std(file->checks[(int32_t)i]->name)] = seconds;
    }
  // Is this the sequential result? Each check, run sequentially, would start
  // with what every check before it left behind.
  std::set<const ProcDecl *> procedures(file->procs.begin(), file->procs.end());
  int64_t extra_slots = 0, extra_containers = 0, extra_strings = 0;
  bool left = false, failed = false;
  out->assign(n, PassedCheck{});
  for (size_t i = 0; i < n; i++) {
    const std::vector<uint64_t> &report = reports[i];
    // A child that crashed or wrote no whole report: run them all here.
    if (report.empty()) return sequential("a child crashed or wrote no whole report");
    if (report[SHARED]) return sequential("a check changed the evaluator's shared state");
    if ((int64_t)report[PEAK_SLOTS] + extra_slots > (int64_t)memory.slot_limit ||
        (int64_t)report[PEAK_CONTAINERS] + extra_containers > (int64_t)memory.container_limit ||
        (int64_t)report[PEAK_STRINGS] + extra_strings > (int64_t)CB_string_live_limit())
      return sequential("a check would pass a storage limit after the storage earlier checks left behind");
    const int64_t ds = (int64_t)report[END_SLOTS] - (int64_t)slots, dc = (int64_t)report[END_CONTAINERS] - (int64_t)containers,
                  db = (int64_t)report[END_STRINGS] - (int64_t)strings;
    left = left || ds || dc || db;
    extra_slots += ds;
    extra_containers += dc;
    extra_strings += db;
    if (!report[PASSED]) { failed = true; continue; }
    PassedCheck &passed = (*out)[i];
    passed.passed = true;
    passed.steps = report[STEPS];
    passed.cases = report[CASES];
    passed.budget = report[BUDGET];
    memcpy(&passed.seconds, &report[SECONDS], sizeof passed.seconds);
    passed.reused = prefilled[i];
    for (size_t k = FIELDS; k < report.size(); k++) {
      const ProcDecl *p = (const ProcDecl *)(uintptr_t)report[k];
      if (procedures.count(p)) passed.executed.push_back(p);
    }
  }
  // A failing check runs again here, after none of the others; that is where
  // a sequential run would start it only when no check left anything behind.
  if (failed && left) return sequential("a check failed after checks left storage behind");
  g_timings.parallel = "parallel: " + std::to_string(n) + " checks, " + std::to_string(jobs) + " jobs";
  return true;
}

static_assert(std::atomic<uint64_t>::is_always_lock_free, "mutation workers share a lock-free counter");

// Evaluates `owned` (global mutant indices) with up to `workers` forked
// processes pulling the next index from a shared counter. Returns false, with
// a message, if a worker could not finish.
static bool evaluate_parallel(const MutationRun &run, const std::vector<size_t> &owned, uint64_t workers,
                              std::vector<MutantResult> *results) {
  size_t bytes = 64 + owned.size() * sizeof(MutantResult);
  void *shared = mmap(nullptr, bytes, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
  if (shared == MAP_FAILED) {
    for (size_t k : owned) (*results)[k] = evaluate_mutant(run, k);
    return true;
  }
  auto *next = new (shared) std::atomic<uint64_t>(0);
  auto *slots = (MutantResult *)((char *)shared + 64);
  for (size_t k = 0; k < owned.size(); k++) new (&slots[k]) MutantResult();
  fflush(stdout);
  fflush(stderr);
  std::vector<pid_t> pids;
  pid_t parent = getpid();
  for (uint64_t w = 0; w < workers; w++) {
    pid_t pid = fork();
    if (pid == 0) {
      // A worker never outlives the run that started it.
#ifdef __linux__
      prctl(PR_SET_PDEATHSIG, SIGKILL);
#endif
      for (;;) {
        if (getppid() != parent) _exit(1);
        uint64_t k = next->fetch_add(1);
        if (k >= owned.size()) break;
        slots[k] = evaluate_mutant(run, owned[k]);
      }
      _exit(0);
    }
    if (pid < 0) break;  // fewer workers; the ones started take all the work
    pids.push_back(pid);
  }
  std::string failure;
  for (pid_t pid : pids) {
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
    if (failure.empty() && !(WIFEXITED(status) && WEXITSTATUS(status) == 0))
      failure = WIFSIGNALED(status) ? "terminated by signal " + std::to_string(WTERMSIG(status))
                                    : "exited with status " + std::to_string(WEXITSTATUS(status));
  }
  // No worker could start: evaluate in this process.
  if (pids.empty())
    for (size_t k = 0; k < owned.size(); k++) slots[k] = evaluate_mutant(run, owned[k]);
  bool ok = true;
  for (size_t k = 0; k < owned.size(); k++) {
    if (!slots[k].done) {
      if (ok) {
        const Mutant &mu = (*run.mutants)[owned[k]];
        auto point = source_point(run.file->src, diagnostic_loc(run.file->src, mu.loc));
        fprintf(stderr, "coax: a mutation worker %s; mutant %zu (%s:%d:%d: %s) did not finish\n",
                failure.empty() ? "stopped early" : failure.c_str(), owned[k], point.path.c_str(), point.loc.line,
                point.loc.col, mu.what.c_str());
      }
      ok = false;
      continue;
    }
    (*results)[owned[k]] = slots[k];
  }
  munmap(shared, bytes);
  return ok;
}

// Identifies the mutant set a shard record describes: the compiler, the root
// source and every mutant's location and change, in order.
static std::string mutation_fingerprint(const File *file, const std::vector<Mutant> &mutants, bool root_only) {
  uint64_t h = 1469598103934665603ull;
  auto mix = [&](const std::string &s) {
    for (unsigned char c : s) { h ^= c; h *= 1099511628211ull; }
    h ^= 0xff; h *= 1099511628211ull;
  };
  // Keep default receipts unchanged; a selected scope must never merge with
  // an unfiltered receipt, even if that graph has the same mutation sites.
  if (root_only) mix("scope:root-only:v1");
  for(const auto &mu:mutants)if(mu.spatial){mix("lowered-spatial-source-token:v1");break;}
  mix(COAX_RELEASE_VERSION);
  mix("execution-profile:v1:" + file->execution_profile);
  mix(file->src->text);
  for (const Mutant &mu : mutants) {
    auto point = source_point(file->src, diagnostic_loc(file->src, mu.loc));
    mix(point.path + ":" + std::to_string(point.loc.line) + ":" + std::to_string(point.loc.col) + " " + mu.what);
  }
  char out[17];
  snprintf(out, sizeof out, "%016llx", (unsigned long long)h);
  return out;
}

static bool read_record(const std::string &path, std::string *out) {
  FILE *f = fopen(path.c_str(), "rb");
  if (!f) return false;
  char buf[65536];
  size_t n;
  while ((n = fread(buf, 1, sizeof buf, f)) > 0) out->append(buf, n);
  bool ok = !ferror(f);
  fclose(f);
  return ok;
}

struct MutationFacts {
  bool selective = false, borrowed = false, strings = false, unstable = false;
};

static std::string facts_line(const std::string &fingerprint, size_t total, uint64_t shards, const MutationFacts &f, bool root_only) {
  return "file " + fingerprint + " mutants " + std::to_string(total) + " shards " + std::to_string(shards) +
         " scheduling " + (f.selective ? "selective" : "full") + " borrowed " + std::to_string(f.borrowed) +
         " strings " + std::to_string(f.strings) + " unstable " + std::to_string(f.unstable) +
         (root_only ? " scope root-only" : "");
}

// Reads shard records and fills every result. Each record must describe the
// same mutant set; together they must cover shards 1..n exactly once.
static bool merge_records(const MutationOptions &opt, const std::string &fingerprint, size_t total,
                          std::vector<MutantResult> *results, MutationFacts *facts) {
  std::string expected_prefix = "file " + fingerprint + " mutants " + std::to_string(total) + " shards ";
  std::string shared_facts;
  uint64_t shards = 0;
  std::vector<bool> seen_shard;
  for (const std::string &path : opt.merge) {
    std::string text;
    if (!read_record(path, &text)) { fprintf(stderr, "coax: cannot read shard record '%s'\n", path.c_str()); return false; }
    std::istringstream in(text);
    std::string line;
    auto bad = [&](const std::string &why) {
      fprintf(stderr, "coax: shard record '%s': %s\n", path.c_str(), why.c_str());
      return false;
    };
    if (!std::getline(in, line) || line != "coax-mutation-shard v1") return bad("not a coax mutation shard record");
    if (!std::getline(in, line) || line.rfind("shard ", 0) != 0) return bad("missing shard line");
    uint64_t s = 0, n = 0;
    {
      std::string spec = line.substr(6);
      size_t slash = spec.find('/');
      if (slash == std::string::npos || !parse_count(spec.substr(0, slash).c_str(), &s) ||
          !parse_count(spec.substr(slash + 1).c_str(), &n) || !MS_shard_valid(s, n))
        return bad("invalid shard " + spec);
    }
    if (!std::getline(in, line)) return bad("missing facts line");
    if (line.rfind(expected_prefix, 0) != 0) return bad("it describes a different file, compiler or mutant set");
    if (shared_facts.empty()) {
      shared_facts = line;
      shards = n;
      seen_shard.assign((size_t)n + 1, false);
    } else if (line != shared_facts) return bad("its shard count or scheduling facts differ from the other records");
    if (n != shards) return bad("shard counts differ");
    if (seen_shard[(size_t)s]) return bad("shard " + std::to_string(s) + "/" + std::to_string(n) + " appears twice");
    seen_shard[(size_t)s] = true;
    size_t count = 0;
    bool ended = false;
    while (std::getline(in, line)) {
      std::istringstream fields(line);
      std::string tag;
      fields >> tag;
      if (tag == "end") {
        size_t declared = 0;
        if (!(fields >> declared) || declared != count) return bad("truncated record");
        ended = true;
        break;
      }
      size_t index = 0;
      long long killer = 0;
      unsigned long long invocations = 0, skipped = 0;
      if (tag != "outcome" || !(fields >> index >> killer >> invocations >> skipped) || index >= total ||
          !MS_shard_owns(index, s, n) || killer < -1 || (*results)[index].done)
        return bad("invalid outcome line '" + line + "'");
      (*results)[index] = {(int32_t)killer, 1, invocations, skipped};
      count++;
    }
    if (!ended) return bad("truncated record");
  }
  for (uint64_t s = 1; s <= shards; s++)
    if (!seen_shard[(size_t)s]) {
      fprintf(stderr, "coax: shard %llu/%llu is missing\n", (unsigned long long)s, (unsigned long long)shards);
      return false;
    }
  for (size_t i = 0; i < total; i++)
    if (!(*results)[i].done) { fprintf(stderr, "coax: the shard records do not cover mutant %zu\n", i); return false; }
  std::istringstream fields(shared_facts);
  std::string word, scheduling;
  int b = 0, st = 0, u = 0;
  // file F mutants T shards N scheduling S borrowed B strings S unstable U
  for (int i = 0; i < 6; i++) fields >> word;
  fields >> word >> scheduling >> word >> b >> word >> st >> word >> u;
  facts->selective = scheduling == "selective";
  facts->borrowed = b, facts->strings = st, facts->unstable = u;
  return true;
}

static int mutate_command(Arena *arena, File *file, const MutationOptions &opt) {
  if (file->checks.count == 0) {
    fprintf(stderr, "coax: %s has no #test or #forall to mutate against\n", file->src->path.c_str());
    return 1;
  }
  bool merging = !opt.merge.empty();
  std::vector<uint64_t> base;
  std::vector<std::vector<const ProcDecl *>> paths;
  bool unstable = false;
  g_evidence.mutation = true;
  if (!merging && !run_all_checks(file, TEST_STEPS, false, &base, nullptr, &paths, &unstable)) {
    g_evidence.mutation_detail = "\"reason\":\"the checks fail before any mutation\"";
    fprintf(stderr, "coax: the checks fail before any mutation; make them pass first\n");
    return 1;
  }
  Mutator m{arena};
  for (const CheckDecl *c : file->checks) m.calls(c->proc->body);
  // Mutate in source order, procedure by procedure.
  for (ProcDecl *p : file->procs) {
    if (std::find(m.reached.begin(), m.reached.end(), p) == m.reached.end()) continue;
    // root_source was mapped from the declaration through the source-unit
    // provenance, not from a flattened line or the caller's source location.
    if (!MS_include(opt.root_only, p->root_source)) continue;
    size_t first = m.mutants.size();
    m.stmt(p->body);
    for (size_t i = first; i < m.mutants.size(); ++i) m.mutants[i].owner = p;
  }
  if(m.invalid_spatial) {g_evidence.infrastructure=true;fprintf(stderr,"coax: incomplete or inconsistent spatial mutation metadata\n");return 1;}
  if(m.spatial_eligible || m.spatial_no_alternative || m.spatial_unsupported)
    fprintf(stderr,"spatial operators: %zu eligible token mutations, %zu no admitted alternate, %zu unsupported unary operations\n",m.spatial_eligible,m.spatial_no_alternative,m.spatial_unsupported);
  size_t total = m.mutants.size();
  std::string fingerprint = merging || opt.shards ? mutation_fingerprint(file, m.mutants, opt.root_only) : "";
  MutationFacts facts;
  std::vector<MutantResult> results(total);
  bool sharded = opt.shards != 0;
  std::vector<size_t> owned;
  if (merging) {
    if (!merge_records(opt, fingerprint, total, &results, &facts)) {
      g_evidence.infrastructure = true;
      return 1;
    }
    for (size_t i = 0; i < total; i++) owned.push_back(i);
  } else {
    facts.borrowed = mutation_borrowed_aggregate(file);
    facts.strings = mutation_allocates_string(file);
    facts.unstable = unstable;
    facts.selective = MS_selective(opt.reference, facts.borrowed, facts.strings, facts.unstable);
    for (size_t i = 0; i < total; i++)
      if (!sharded || MS_shard_owns(i, opt.shard, opt.shards)) owned.push_back(i);
    MutationRun run{file, &m.mutants, &base, &paths, facts.selective};
    uint64_t jobs = opt.jobs;
    if (!jobs) {
      const char *env = getenv("COAX_MUTATE_JOBS");
      if (!env || !parse_count(env, &jobs) || !MS_jobs_valid(jobs)) jobs = std::min<uint64_t>(available_cores(), 1024);
    }
    uint64_t workers = std::min<uint64_t>(jobs, owned.size());
    if (workers <= 1) {
      for (size_t i : owned) {
        results[i] = evaluate_mutant(run, i);
        if (opt.trace) fprintf(stderr, "mutation-outcome %zu %d\n", i, results[i].killer);
      }
    } else if (!evaluate_parallel(run, owned, workers, &results)) {
      g_evidence.infrastructure = true;
      return 1;
    }
    if (workers > 1 && opt.trace)
      for (size_t i : owned) fprintf(stderr, "mutation-outcome %zu %d\n", i, results[i].killer);
  }
  if (merging && opt.trace)
    for (size_t i : owned) fprintf(stderr, "mutation-outcome %zu %d\n", i, results[i].killer);
  uint64_t invocations = 0, skipped = 0;
  std::vector<const Mutant *> survivors;
  size_t equivalent = 0;
  for (size_t i : owned) {
    invocations += results[i].invocations;
    skipped += results[i].skipped;
    if (results[i].killer >= 0) continue;
    if (marked_equivalent(file->src, m.mutants[i].loc)) equivalent++;
    else survivors.push_back(&m.mutants[i]);
  }
  std::string unreached;
  std::vector<const ProcDecl *> unreached_procs;
  for (const ProcDecl *p : file->procs) {
    if (!SU_check_required(p->authority,p->root_source,true) || p->foreign || str_eq(p->name, "main")) continue;
    if (std::find(m.reached.begin(), m.reached.end(), p) == m.reached.end()) {
      unreached += (unreached.empty() ? "" : ", ") + to_std(p->name);
      unreached_procs.push_back(p);
    }
  }
  // A shard prints a record for `--merge` on stdout and its own survivors on
  // stderr; the full report comes from the merge.
  FILE *report = sharded ? stderr : stdout;
  if (sharded) {
    printf("coax-mutation-shard v1\nshard %llu/%llu\n%s\n", (unsigned long long)opt.shard, (unsigned long long)opt.shards,
           facts_line(fingerprint, total, opt.shards, facts, opt.root_only).c_str());
    for (size_t i : owned)
      printf("outcome %zu %d %llu %llu\n", i, results[i].killer, (unsigned long long)results[i].invocations,
             (unsigned long long)results[i].skipped);
    printf("end %zu\n", owned.size());
    fflush(stdout);
  }
  if (opt.root_only) fprintf(report, "mutation scope: root-only (entry-source procedures; dependency checks retained)\n");
  if (g_diagnostics_json) {
    // One coax.diagnostic.v1 object per survivor and per unreached procedure, on stderr.
    for (const Mutant *mu : survivors)
      error_at(file->src, mu->loc, "E0414", "mutant survived: " + mu->what, "add a check that fails for this change, or end the line with '// mutant: equivalent' if it can't change behavior");
    for (const ProcDecl *p : unreached_procs)
      error_at(file->src, p->loc, "E0415", "'" + to_std(p->name) + "' is not reached by any check", "call it from a #test or #forall");
  } else {
    for (const Mutant *mu : survivors) {
      Loc loc = diagnostic_loc(file->src, mu->loc);
      auto point = source_point(file->src, loc);
      fprintf(report, "%s:%d:%d: mutant survived: %s\n", point.path.c_str(), point.loc.line, point.loc.col, mu->what.c_str());
      show_source(report, file->src, mu->loc);
    }
    if (!unreached.empty()) fprintf(report, "not reached by any check: %s\n", unreached.c_str());
  }
  if (sharded) {
    fprintf(report, "shard %llu/%llu: %zu of %zu mutant%s, %zu killed, %zu survived", (unsigned long long)opt.shard,
            (unsigned long long)opt.shards, owned.size(), total, total == 1 ? "" : "s",
            owned.size() - survivors.size() - equivalent, survivors.size());
  } else {
    fprintf(report, "%zu mutant%s, %zu killed, %zu survived", total, total == 1 ? "" : "s",
            total - survivors.size() - equivalent, survivors.size());
  }
  if (equivalent) fprintf(report, ", %zu marked equivalent", equivalent);
  fprintf(report, "\n");
  if (m.mutants.empty()) fprintf(report, "no reachable mutation sites\n");
  g_evidence.mutation_outcome = EV_mutation(!m.mutants.empty(), !survivors.empty(), !unreached.empty(), sharded);
  {
    std::string rows;
    for (const Mutant *mu : survivors) {
      auto point = source_point(file->src, diagnostic_loc(file->src, mu->loc));
      rows += std::string(rows.empty() ? "" : ",") + "{\"file\":\"" + json_escape_basic(point.path) + "\",\"line\":" + std::to_string(point.loc.line) +
              ",\"col\":" + std::to_string(point.loc.col) + ",\"mutant\":\"" + json_escape_basic(mu->what) + "\"}";
    }
    g_evidence.mutation_detail = "\"mutants\":" + std::to_string(total) + ",\"evaluated\":" + std::to_string(owned.size()) +
        ",\"killed\":" + std::to_string(owned.size() - survivors.size() - equivalent) + ",\"survived\":[" + rows + "]" +
        ",\"marked_equivalent\":" + std::to_string(equivalent) + ",\"unreached\":\"" + json_escape_basic(unreached) + "\"" +
        ",\"scope\":\"" + (opt.root_only ? "root-only" : "all") + "\",\"shard\":" +
        (sharded ? "[" + std::to_string(opt.shard) + "," + std::to_string(opt.shards) + "]" : std::string("null")) +
        ",\"merged_records\":" + std::to_string(opt.merge.size()) + ",\"scheduling\":\"" + (facts.selective ? "selective" : "full") +
        "\",\"per_check_budget\":\"max(10000, 10 x the unmutated check's steps)\"";
  }
  if (opt.stats) fprintf(stderr, "mutation-scheduling %s checks=%llu skipped=%llu borrowed=%d strings=%d unstable=%d\n",
                         facts.selective ? "selective" : "full", (unsigned long long)invocations,
                         (unsigned long long)skipped, facts.borrowed, facts.strings, facts.unstable);
  return survivors.empty() && unreached.empty() && !m.mutants.empty() ? 0 : 1;
}

}  // namespace coax
