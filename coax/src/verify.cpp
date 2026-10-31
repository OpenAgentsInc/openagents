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
#include <unistd.h>

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
  if (!merging && !run_all_checks(file, TEST_STEPS, false, &base, nullptr, &paths, &unstable)) {
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
  size_t total = m.mutants.size();
  std::string fingerprint = merging || opt.shards ? mutation_fingerprint(file, m.mutants, opt.root_only) : "";
  MutationFacts facts;
  std::vector<MutantResult> results(total);
  bool sharded = opt.shards != 0;
  std::vector<size_t> owned;
  if (merging) {
    if (!merge_records(opt, fingerprint, total, &results, &facts)) return 1;
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
  for (const ProcDecl *p : file->procs) {
    if (!SU_check_required(p->authority,p->root_source,true) || p->foreign || str_eq(p->name, "main")) continue;
    if (std::find(m.reached.begin(), m.reached.end(), p) == m.reached.end())
      unreached += (unreached.empty() ? "" : ", ") + to_std(p->name);
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
  for (const Mutant *mu : survivors) {
    Loc loc = diagnostic_loc(file->src, mu->loc);
    auto point = source_point(file->src, loc);
    fprintf(report, "%s:%d:%d: mutant survived: %s\n", point.path.c_str(), point.loc.line, point.loc.col, mu->what.c_str());
    show_source(report, file->src, mu->loc);
  }
  if (!unreached.empty()) fprintf(report, "not reached by any check: %s\n", unreached.c_str());
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
  if (opt.stats) fprintf(stderr, "mutation-scheduling %s checks=%llu skipped=%llu borrowed=%d strings=%d unstable=%d\n",
                         facts.selective ? "selective" : "full", (unsigned long long)invocations,
                         (unsigned long long)skipped, facts.borrowed, facts.strings, facts.unstable);
  return survivors.empty() && unreached.empty() && !m.mutants.empty() ? 0 : 1;
}

}  // namespace coax
