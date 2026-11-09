// Native checks (#13122, coax/docs/native-checks.md). With COAX_NATIVE_CHECKS
// set, a compile's checks (#authority, library and coax test) run as native
// code first: the program's procedures and the checks' bodies, emitted by the
// ordinary C emitter (emit_check_binary_c), compiled by the host C compiler
// and run by a driver main. A check that passes there is taken as passed, with
// the procedures it ran; every other check (a trap, a refusal for output or a
// host effect, a crash, a time limit, a form the binary can't hold) runs in
// the evaluator as before, so its diagnostics are the evaluator's.
//   COAX_NATIVE_CHECKS=1        native first, the evaluator for the rest
//   COAX_NATIVE_CHECKS=compare  both, for every check; disagreements are
//                               reported (COAX_NATIVE_CHECKS_REPORT FILE
//                               appends one JSON line per check)
//   COAX_NATIVE_CHECKS_OPT=N    the C optimization level (default 0)
//   COAX_CHECK_SECONDS=N        one native check's wall-time limit (default 120)
#include <poll.h>
#include <signal.h>
#include <spawn.h>
#include <sys/wait.h>
#include <fcntl.h>
#include <unistd.h>
#include <filesystem>

extern char **environ;

namespace coax {

enum class NativeMode { off, on, compare };
static bool g_native_blocked = false;  // a mutation-lowered compile: the evaluator alone
static NativeMode native_mode() {
  if (g_native_blocked) return NativeMode::off;
  const char *v = getenv("COAX_NATIVE_CHECKS");
  if (!v || !*v || !strcmp(v, "0") || !strcmp(v, "off")) return NativeMode::off;
  if (!strcmp(v, "compare")) return NativeMode::compare;
  return NativeMode::on;
}

struct NativeResult {
  enum State { not_run, passed, trapped, refused, timed_out, crashed } state = not_run;
  uint64_t cases = 0, fail_case = 0;
  double seconds = 0;
  int64_t line = 0;
  std::string what, path;
  std::vector<const ProcDecl *> executed;
};
// The check cache keys' mark of native checks ("" when off), so a verdict
// native code gave is never taken by an evaluator-only compile, or back.
static std::string native_key() {
  const NativeMode m = native_mode();
  return m == NativeMode::off ? "" : m == NativeMode::on ? " native=1" : " native=compare";
}
static const char *native_state_name(NativeResult::State s) {
  static const char *names[] = {"not-run", "passed", "trapped", "refused", "timed-out", "crashed"};
  return names[s];
}

struct NativeRun {
  const File *file = nullptr;
  std::vector<NativeResult> results;  // by check index
  size_t eligible = 0, passed = 0;
  double emit = 0, compile = 0, run = 0;
  std::string why;  // why no check ran natively ("" when they did)
};
static NativeRun g_native;
static EmitOptions g_native_opts;  // the compile's options (set by front_end)

// The procedures a body can call, directly or not (a constant's expression
// and a generic instance included), and whether it reaches an unevaluated
// #run, which the binary can't hold.
struct NativeWalk {
  std::set<const ProcDecl *> *have;  // procedures already in the binary
  std::set<const void *> seen;
  std::vector<const ProcDecl *> added;
  bool bad = false;
  void proc(const ProcDecl *p) {
    if (!p || have->count(p) || !seen.insert(p).second) return;
    added.push_back(p);
    if (p->body) stmt(p->body);
  }
  void expr(const Expr *e) {
    if (!e || bad || !seen.insert(e).second) return;
    if (e->kind == ExprKind::run) { bad = true; return; }
    proc(e->proc);
    proc(e->mutation_proc);
    if (e->kind == ExprKind::ident && e->constant) expr(e->constant->expr);
    expr(e->lhs); expr(e->rhs); expr(e->third);
    for (const Expr *a : e->args) expr(a);
    for (const FieldInit &f : e->fields) expr(f.value);
  }
  void stmt(const Stmt *s) {
    if (!s || bad) return;
    expr(s->expr); expr(s->expr2); expr(s->target);
    for (const Stmt *c : s->body) stmt(c);
    stmt(s->then_); stmt(s->else_);
    for (const SwitchCase *arm : s->cases) { expr(arm->enum_pattern); stmt(arm->body); }
  }
};

static bool native_write(const std::string &path, const std::string &text) {
  FILE *f = fopen(path.c_str(), "wb");
  if (!f) return false;
  const bool ok = fwrite(text.data(), 1, text.size(), f) == text.size();
  return fclose(f) == 0 && ok;
}

static int native_spawn(const std::vector<std::string> &args, int out_fd, pid_t *pid_out) {
  std::vector<char *> argv;
  for (const std::string &a : args) argv.push_back(const_cast<char *>(a.c_str()));
  argv.push_back(nullptr);
  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0);
  posix_spawn_file_actions_addopen(&actions, 1, "/dev/null", O_WRONLY, 0);
  if (out_fd >= 0) {
    posix_spawn_file_actions_adddup2(&actions, out_fd, 3);
    posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_WRONLY, 0);
  }
  pid_t pid = 0;
  const int rc = posix_spawnp(&pid, argv[0], &actions, nullptr, argv.data(), environ);
  posix_spawn_file_actions_destroy(&actions);
  if (rc != 0) return -1;
  *pid_out = pid;
  return 0;
}

static int native_wait(pid_t pid) {
  int status = 0;
  while (waitpid(pid, &status, 0) < 0) if (errno != EINTR) return -1;
  return status;
}

// Runs the checks whose want[i] is set natively; fills g_native. Never prints.
static void native_run(File *file, const std::vector<bool> &want, uint64_t steps) {
  g_native = NativeRun{};
  g_native.file = file;
  const size_t n = (size_t)file->checks.count;
  g_native.results.assign(n, NativeResult{});
  const auto began = std::chrono::steady_clock::now();
  TimedLaps laps;  // --timings: native.plan, native.emit, native.compile, native.run
  // The procedures every struct default calls (the binary's init helpers
  // hold them all), then each eligible check's body and what it calls.
  std::set<const ProcDecl *> have;
  NativeCheckCPlan plan;
  plan.verification_abi = 2;
  {
    NativeWalk walk{&have};
    std::vector<Type> types;
    collect_types(file, &types);
    for (Type t : types)
      if (t->kind == TypeKind::struct_ && t->struct_decl)
        for (const Field &f : t->struct_decl->fields) walk.expr(f.init);
    if (walk.bad) { g_native.why = "a struct default holds #run"; return; }
    for (const ProcDecl *p : walk.added) { have.insert(p); plan.procedures.push_back(p); }
  }
  for (size_t i = 0; i < n; i++) {
    CheckDecl *c = file->checks[(int32_t)i];
    if (!want[i] || !c->proc || !c->proc->body || c->proc->params.count > 64) continue;
    NativeWalk walk{&have};
    walk.proc(c->proc);
    if (walk.bad) continue;
    NativeCheckCEntry entry;
    entry.declaration_index = (int64_t)i;
    entry.check = c;
    if (c->forall && !native_check_domains_quiet(file, &entry, steps)) continue;
    for (const ProcDecl *p : walk.added) { have.insert(p); plan.procedures.push_back(p); }
    plan.checks.push_back(std::move(entry));
  }
  g_native.eligible = plan.checks.size();
  laps.lap("native.plan");
  if (plan.checks.empty()) { g_native.why = "no eligible check"; return; }
  EmitOptions opts = g_native_opts;
  opts.wrapping = false;  // checks are checked arithmetic, as in the evaluator
  opts.library = false;
  opts.split = 0;
  opts.debug_info = false;
  opts.metal = opts.cuda = opts.metal_compute = false;
  auto t0 = std::chrono::steady_clock::now();
  const char *parts_env = getenv("COAX_NATIVE_CHECKS_PARTS");
  const size_t parts = parts_env && *parts_env ? (size_t)strtoull(parts_env, nullptr, 10) : (size_t)std::min<uint64_t>(available_cores(), 32);
  const std::vector<std::string> units = emit_check_binary_c(file, opts, plan, parts);
  g_native.emit = seconds_since(t0);
  laps.lap("native.emit");
  const char *tmp = getenv("TMPDIR");
  std::string dir = std::string(tmp && *tmp ? tmp : "/tmp") + "/coax-native-checks.XXXXXX";
  if (!mkdtemp(dir.data())) { g_native.why = "no temporary directory"; return; }
  struct Cleanup { std::string dir; ~Cleanup() { std::error_code ec; if (!getenv("COAX_NATIVE_CHECKS_KEEP")) std::filesystem::remove_all(dir, ec); } } cleanup{dir};
  const std::string exe = dir + "/checks";
  const char *cc = getenv("COAX_CC");
  const char *opt = getenv("COAX_NATIVE_CHECKS_OPT");
  const std::string compiler = cc && *cc ? cc : "cc";
  // Each translation unit compiles in its own process, all at once; then one link.
  t0 = std::chrono::steady_clock::now();
  std::vector<pid_t> compiling;
  std::vector<std::string> objects;
  bool failed = false;
  for (size_t u = 0; u < units.size(); u++) {
    const std::string source = dir + "/checks" + std::to_string(u) + ".c", object = dir + "/checks" + std::to_string(u) + ".o";
    if (!native_write(source, units[u])) { g_native.why = "can't write the C"; failed = true; break; }
    pid_t pid;
    if (native_spawn({compiler, "-std=c11", "-w", std::string("-O") + (opt && *opt ? opt : "0"), "-ffp-contract=off", "-fno-builtin-sin",
                      "-fno-builtin-cos", "-pthread", "-c", "-o", object, source}, -1, &pid) != 0) { g_native.why = "no C compiler"; failed = true; break; }
    compiling.push_back(pid);
    objects.push_back(object);
  }
  for (pid_t pid : compiling) {
    const int status = native_wait(pid);
    if (status < 0 || !WIFEXITED(status) || WEXITSTATUS(status) != 0) failed = true;
  }
  if (!failed) {
    std::vector<std::string> link = {compiler, "-pthread", "-o", exe};
    link.insert(link.end(), objects.begin(), objects.end());
    link.push_back("-lm");
    pid_t pid;
    const int status = native_spawn(link, -1, &pid) == 0 ? native_wait(pid) : -1;
    failed = status < 0 || !WIFEXITED(status) || WEXITSTATUS(status) != 0;
  }
  g_native.compile = seconds_since(t0);
  laps.lap("native.compile");
  if (failed) {
    if (g_native.why.empty()) g_native.why = "the C compiler failed on the check binary (a coax bug; COAX_NATIVE_CHECKS_KEEP=1 keeps it)";
    if (getenv("COAX_NATIVE_CHECKS_KEEP")) fprintf(stderr, "coax: native checks: %s in %s\n", g_native.why.c_str(), dir.c_str());
    return;
  }
  // Run the binary, from the first check on, and again after each check that stops it.
  double limit = 120;
  if (const char *s = getenv("COAX_CHECK_SECONDS")) { double v = strtod(s, nullptr); if (v > 0) limit = v; }
  t0 = std::chrono::steady_clock::now();
  const size_t rows = plan.checks.size();
  size_t start = 0;
  while (start < rows) {
    int fds[2];
    if (pipe(fds) != 0) break;
    fcntl(fds[0], F_SETFD, FD_CLOEXEC);
    pid_t pid;
    if (native_spawn({exe, std::to_string(start)}, fds[1], &pid) != 0) { close(fds[0]); close(fds[1]); break; }
    close(fds[1]);
    int64_t current = -1;
    auto check_began = std::chrono::steady_clock::now();
    std::string buffer;
    bool killed = false;
    NativeResult pending;  // what the current check reported before it stopped
    for (;;) {
      int wait_ms = -1;
      if (current >= 0) {
        const double left = limit - seconds_since(check_began);
        wait_ms = left <= 0 ? 0 : (int)(left * 1000) + 1;
      }
      pollfd p{fds[0], POLLIN, 0};
      const int r = poll(&p, 1, wait_ms);
      if (r < 0 && errno == EINTR) continue;
      if (r == 0) {  // the check ran past its limit
        kill(pid, SIGKILL);
        killed = true;
        break;
      }
      char chunk[65536];
      const ssize_t got = read(fds[0], chunk, sizeof chunk);
      if (got < 0 && errno == EINTR) continue;
      if (got <= 0) break;
      buffer.append(chunk, (size_t)got);
      for (size_t nl; (nl = buffer.find('\n')) != std::string::npos; buffer.erase(0, nl + 1)) {
        const std::string line = buffer.substr(0, nl);
        if (line.size() < 2) continue;
        if (line[0] == 'B') {
          current = strtoll(line.c_str() + 2, nullptr, 10);
          check_began = std::chrono::steady_clock::now();
          pending = NativeResult{};
        } else if (line[0] == 'E' && current >= 0 && (size_t)current < rows) {
          NativeResult &out = g_native.results[(size_t)plan.checks[(size_t)current].declaration_index];
          out.state = NativeResult::passed;
          out.seconds = seconds_since(check_began);
          const char *at = line.c_str() + 2;
          char *end;
          strtoll(at, &end, 10);
          out.cases = strtoull(end, &end, 10);
          for (;;) {
            while (*end == ' ') end++;
            if (!*end) break;
            const size_t id = strtoull(end, &end, 10);
            if (id < plan.procedures.size()) out.executed.push_back(plan.procedures[id]);
          }
          g_native.passed++;
          start = (size_t)current + 1;
          current = -1;
        } else if (line[0] == 'T') {
          pending.state = NativeResult::trapped;
          const char *at = line.c_str() + 2;
          char *end;
          pending.fail_case = strtoull(at, &end, 10);
          if (*end == '\t') end++;
          pending.line = strtoll(end, &end, 10);
          if (*end == '\t') end++;
          const std::string rest = end;
          const size_t tab = rest.find('\t');
          pending.path = rest.substr(0, tab);
          pending.what = tab == std::string::npos ? "" : rest.substr(tab + 1);
        } else if (line[0] == 'R') {
          pending.state = NativeResult::refused;
          pending.what = line.substr(2);
        }
      }
    }
    close(fds[0]);
    const int st = native_wait(pid);
    if (current < 0) {
      // Stopped between checks: done when it exited normally, else a broken binary.
      if (!(st >= 0 && WIFEXITED(st) && WEXITSTATUS(st) == 0) && start < rows) g_native.why = "the check binary stopped between checks";
      break;
    }
    NativeResult &out = g_native.results[(size_t)plan.checks[(size_t)current].declaration_index];
    out = pending;
    out.seconds = seconds_since(check_began);
    if (killed) out.state = NativeResult::timed_out;
    else if (out.state == NativeResult::not_run) out.state = NativeResult::crashed;
    start = (size_t)current + 1;
  }
  g_native.run = seconds_since(t0);
  laps.lap("native.run");
  if (g_timings.on()) {
    g_timings.detail["native.checks"] += (double)g_native.passed;
    g_timings.detail["native.eligible"] += (double)g_native.eligible;
  }
  (void)began;
}

static bool native_passed(const File *file, size_t i) {
  return g_native.file == file && i < g_native.results.size() && g_native.results[i].state == NativeResult::passed;
}

// compare: one check's native and evaluator verdicts. A disagreement is a
// native pass where the evaluator fails for a reason other than its budgets
// (E0400), or a native trap where the evaluator passes.
static void native_compare(const File *file, size_t i, const CheckDecl *c, const CheckOutcome &r) {
  if (native_mode() != NativeMode::compare || g_native.file != file || i >= g_native.results.size()) return;
  const NativeResult &nr = g_native.results[i];
  std::string verdict = "agree";
  if (nr.state == NativeResult::not_run) verdict = "evaluator-only";
  else if (nr.state == NativeResult::passed && !r.passed) verdict = r.code == "E0400" ? "native-only-budget" : "disagree";
  else if (nr.state == NativeResult::trapped && r.passed) verdict = "disagree";
  else if (nr.state != NativeResult::passed && nr.state != NativeResult::trapped) verdict = "fallback";
  if (verdict == "disagree")
    fprintf(stderr, "coax: native check disagrees: %s %s: native %s%s%s, evaluator %s%s\n", c->forall ? "#forall" : "#test", to_std(c->name).c_str(),
            native_state_name(nr.state), nr.what.empty() ? "" : " ", nr.what.c_str(), r.passed ? "passed" : "failed ", r.passed ? "" : r.code.c_str());
  const char *report = getenv("COAX_NATIVE_CHECKS_REPORT");
  if (!report || !*report) return;
  const std::string row = "{\"file\":" + json_string(tree_path(file->src->units[0]->canonical)) + ",\"check\":" + json_string(to_std(c->name)) +
                          ",\"verdict\":\"" + verdict + "\",\"native\":\"" + native_state_name(nr.state) + "\",\"native_what\":" + json_string(nr.what) +
                          ",\"evaluator\":" + (r.passed ? "\"passed\"" : json_string("failed " + r.code)) + ",\"native_seconds\":" + std::to_string(nr.seconds) + "}\n";
  const int fd = open(report, O_WRONLY | O_CREAT | O_APPEND, 0644);
  if (fd < 0) return;
  ssize_t w = write(fd, row.data(), row.size());
  (void)w;
  close(fd);
}

}  // namespace coax
