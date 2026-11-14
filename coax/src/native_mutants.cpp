// Native mutants (#13582, coax/docs/native-mutants.md): coax mutate --native.
// The checks and what they reach are emitted to C once, as the native check
// binary is (emit_check_binary_c), with every mutation site of this run's
// inventory behind a run-time switch. One C compile; then each mutant is a
// process that runs its scheduled checks as native code and stops at the
// first that fails.
//
// Native code never decides a verdict. A native kill names the check the
// evaluator runs first for that mutant, and only that check's failure in the
// evaluator is the kill. A native survivor gets no hint and runs the
// evaluator's whole schedule. So the report is the one coax mutate prints
// without --native, byte for byte, and a difference between native code and
// the evaluator is a failure of the native path, reported on stderr.
//
// The decisions are Coax (lib/compiler/mutation-schedule.coax: NM_tick_limit,
// NM_wall_ms, NM_killed, NM_compare, MS_run); this file moves processes.
//   COAX_NATIVE_MUTANTS_TRACE=1     one line on stderr: counts and times
//   COAX_NATIVE_MUTANTS_REPORT=FILE one line a mutant: index, native outcome,
//                                   the check it stopped in, the mutant's key;
//                                   the native survivors are its `survived` rows
//   COAX_NATIVE_MUTANTS_KEEP=1      keeps the C, the binary and the schedules
//   COAX_NATIVE_MUTANTS_STRICT=1    a disagreement makes the command exit 3
//   COAX_NATIVE_MUTANTS_JOBS=N      processes at once (default: the mutation workers)
// COAX_CC, COAX_NATIVE_CHECKS_OPT, COAX_NATIVE_CHECKS_PARTS and
// COAX_CHECK_SECONDS mean what they mean for native checks.
#include <deque>

namespace coax {

// A mutant's native outcome, as lib/compiler/mutation-schedule.coax numbers it.
enum : uint32_t { NM_NOT_RUN = 0, NM_SURVIVED = 1, NM_SURVIVED_PART = 2, NM_TRAP = 3, NM_REFUSAL = 4, NM_CRASH = 5, NM_TICKS = 6, NM_WALL = 7 };

struct NativeMutants {
  bool ran = false;               // native code judged this run's mutants
  std::string why;                // why not ("" when it did)
  std::vector<uint32_t> outcome;  // by mutant index: NM_NOT_RUN .. NM_WALL
  std::vector<int32_t> killer;    // by mutant index: the check it stopped in, or -1
  std::vector<uint8_t> judged;    // by check index: its unmutated native run passed, so native code runs it
  std::vector<int32_t> rows;      // by check index: its row in the binary's table, or -1
  size_t emitted = 0, checks = 0;
  double emit = 0, compile = 0, baseline = 0, run = 0;
  std::chrono::steady_clock::time_point began;  // when the native pass began: after the evaluator's unmutated run
};

static bool g_native_mutants_disagreed = false;  // native_mutants_compare found a disagreement in this run

static const char *native_mutant_outcome(uint32_t outcome) {
  static const char *names[] = {"not-run", "survived", "survived-part", "trap", "refusal", "crash", "ticks", "wall"};
  return outcome < 8 ? names[outcome] : "unknown";
}

// Is the C compiler there to run? A name is looked up on PATH, as posix_spawnp does.
static bool native_compiler_found() {
  const char *cc = getenv("COAX_CC");
  const std::string compiler = cc && *cc ? cc : "cc";
  if (compiler.find('/') != std::string::npos) return access(compiler.c_str(), X_OK) == 0;
  const char *path = getenv("PATH");
  std::istringstream dirs(path && *path ? path : "/usr/bin:/bin");
  std::string dir;
  while (std::getline(dirs, dir, ':'))
    if (access(((dir.empty() ? "." : dir) + "/" + compiler).c_str(), X_OK) == 0) return true;
  return false;
}

// Runs `tasks` processes of the check binary, at most `workers` at once, each
// worker one build worker from the machine-wide pool (JobSlots), as the
// evaluator's mutation workers are. A worker keeps its token from one process
// to the next while tasks are left, as an evaluator worker keeps its token
// from one mutant to the next: the pool's load limit only decides how many
// workers start, so the processes' own load doesn't shrink a running pass. start(task) gives a process's arguments
// (none: the task has nothing to run). line(task, text) gets each line the
// process writes to file descriptor 3 and returns the seconds its current
// check may still take (negative: no change). ended(task, status, timed_out)
// gets how it ended and says whether the task has more to run.
template <class Start, class Line, class Ended>
static void native_mutant_pool(size_t tasks, uint64_t workers, double first_limit, Start start, Line line, Ended ended) {
  struct Process {
    pid_t pid = -1;
    int fd = -1;
    size_t task = 0;
    std::string buffer;
    std::chrono::steady_clock::time_point began;
    double limit = 0;
  };
  std::vector<Process> live;
  std::deque<size_t> again;
  size_t next = 0, held = 0;  // held: the build workers this pass holds
  JobSlots slots(workers);
  auto finish = [&](size_t at, bool timed_out) {
    Process p = std::move(live[at]);
    live.erase(live.begin() + (std::ptrdiff_t)at);
    // What it wrote before it stopped still counts.
    for (;;) {
      char chunk[65536];
      const ssize_t got = read(p.fd, chunk, sizeof chunk);
      if (got < 0 && errno == EINTR) continue;
      if (got <= 0) break;
      p.buffer.append(chunk, (size_t)got);
    }
    for (size_t nl; (nl = p.buffer.find('\n')) != std::string::npos; p.buffer.erase(0, nl + 1)) line(p.task, p.buffer.substr(0, nl));
    close(p.fd);
    const int status = native_wait(p.pid);
    if (ended(p.task, status, timed_out)) again.push_back(p.task);
  };
  for (;;) {
    while ((next < tasks || !again.empty()) && live.size() < workers) {
      if (live.size() >= held) {
        if (!slots.start()) break;
        held++;
      }
      size_t task;
      if (!again.empty()) { task = again.front(); again.pop_front(); }
      else task = next++;
      const std::vector<std::string> args = start(task);
      int fds[2];
      pid_t pid = -1;
      if (args.empty() || pipe(fds) != 0) { if (!args.empty()) ended(task, -1, false); continue; }
      fcntl(fds[0], F_SETFD, FD_CLOEXEC);
      if (native_spawn(args, fds[1], &pid) != 0) { close(fds[0]); close(fds[1]); ended(task, -1, false); continue; }
      close(fds[1]);
      Process p;
      p.pid = pid;
      p.fd = fds[0];
      p.task = task;
      p.began = std::chrono::steady_clock::now();
      p.limit = first_limit;
      live.push_back(std::move(p));
    }
    // No task is left to start: the workers with nothing to run give their tokens back.
    if (next >= tasks && again.empty()) while (held > live.size()) { slots.stop(); held--; }
    if (live.empty()) {
      if (next >= tasks && again.empty()) break;
      continue;  // the first worker may always start
    }
    std::vector<pollfd> polls;
    // More could start when a build worker is free: look again in a quarter second.
    int wait_ms = (next < tasks || !again.empty()) && live.size() < workers ? 250 : -1;
    for (const Process &p : live) {
      polls.push_back(pollfd{p.fd, POLLIN, 0});
      const double left = p.limit - seconds_since(p.began);
      const int ms = left <= 0 ? 0 : left > 3600 ? 3600000 : (int)(left * 1000) + 1;
      if (wait_ms < 0 || ms < wait_ms) wait_ms = ms;
    }
    const int ready = poll(polls.data(), (nfds_t)polls.size(), wait_ms);
    if (ready < 0 && errno != EINTR) break;
    // Back to front, so finishing one leaves the earlier indexes in place.
    for (size_t at = live.size(); at-- > 0;) {
      Process &p = live[at];
      if (ready > 0 && polls[at].revents) {
        char chunk[65536];
        const ssize_t got = read(p.fd, chunk, sizeof chunk);
        if (got < 0 && errno == EINTR) continue;
        if (got <= 0) { finish(at, false); continue; }
        p.buffer.append(chunk, (size_t)got);
        for (size_t nl; (nl = p.buffer.find('\n')) != std::string::npos; p.buffer.erase(0, nl + 1)) {
          const double limit = line(p.task, p.buffer.substr(0, nl));
          if (limit >= 0) { p.began = std::chrono::steady_clock::now(); p.limit = limit; }
        }
        continue;
      }
      if (seconds_since(p.began) >= p.limit) {
        kill(p.pid, SIGKILL);
        finish(at, true);
      }
    }
  }
  for (const Process &p : live) { kill(p.pid, SIGKILL); close(p.fd); native_wait(p.pid); }
}

// The native pass. On return `native` holds each owned mutant's outcome, and
// `hints` names, for each mutant native code killed, the check it stopped in.
static void native_mutants(File *file, const std::vector<Mutant> &mutants, const std::vector<size_t> &owned,
                           const std::vector<uint64_t> &base, const std::vector<std::vector<const ProcDecl *>> &paths,
                           bool selective, std::vector<int32_t> *hints, const MutationOptions &opt, NativeMutants *native) {
  const size_t total = mutants.size(), checks = (size_t)file->checks.count;
  native->outcome.assign(total, NM_NOT_RUN);
  native->killer.assign(total, -1);
  native->judged.assign(checks, 0);
  native->rows.assign(checks, -1);
  native->checks = checks;
  native->began = std::chrono::steady_clock::now();
  // Said once, however the pass ends.
  struct Said {
    NativeMutants *native;
    const std::vector<size_t> *owned;
    ~Said() {
      if (g_json_errors) return;  // no text in a JSON stream
      if (!native->ran) {
        if (native->why.empty()) return;
        fprintf(stderr, "coax: mutate --native: %s; the evaluator runs every mutant's checks, as without --native\n", native->why.c_str());
        return;
      }
      const char *trace = getenv("COAX_NATIVE_MUTANTS_TRACE");
      if (!trace || !*trace) return;
      size_t counts[8] = {}, judged = 0;
      for (size_t i : *owned) counts[native->outcome[i] < 8 ? native->outcome[i] : 0]++;
      for (uint8_t j : native->judged) judged += j;
      fprintf(stderr, "coax: native mutants: %zu of %zu mutants have a switch, %zu of %zu checks run natively; %zu killed"
              " (%zu trap, %zu refusal, %zu crash, %zu ticks, %zu wall), %zu survived, %zu survived what ran, %zu not run;"
              " emit %.2f s, compile %.2f s, baseline %.2f s, mutants %.2f s\n",
              native->emitted, owned->size(), judged, native->checks, counts[3] + counts[4] + counts[5] + counts[6] + counts[7],
              counts[3], counts[4], counts[5], counts[6], counts[7], counts[1], counts[2], counts[0],
              native->emit, native->compile, native->baseline, native->run);
    }
  } said{native, &owned};
  if (owned.empty()) return;
  if (!native_compiler_found()) { native->why = "no C compiler ($COAX_CC, else cc, is not on PATH)"; return; }

  // The plan: every check, as coax test plans them.
  NativeCheckCPlan plan;
  const std::vector<bool> want(checks, true);
  if (!native_plan(file, want, TEST_STEPS, &plan, &native->why)) { native->why = "the checks can't be emitted (" + native->why + ")"; return; }
  if (plan.checks.empty()) { native->why = "the checks can't be emitted (no check native code can hold)"; return; }
  for (size_t row = 0; row < plan.checks.size(); row++) native->rows[(size_t)plan.checks[row].declaration_index] = (int32_t)row;

  NativeMutantPlan sites;
  for (size_t i : owned) {
    const Mutant &mu = mutants[i];
    if (!mu.site) continue;
    NativeMutantSite site;
    site.id = (uint64_t)i + 1;
    site.expr = mu.site_expr;
    site.stmt = mu.site_stmt;
    site.kind = mu.site;
    site.apply = mu.apply;
    site.undo = mu.undo;
    sites.sites.push_back(std::move(site));
  }
  EmitOptions opts = g_native_opts;
  opts.wrapping = false;  // checked arithmetic, as in the evaluator
  opts.library = false;
  opts.split = 0;
  opts.debug_info = false;
  opts.metal = opts.cuda = opts.metal_compute = false;
  auto t0 = std::chrono::steady_clock::now();
  const char *parts_env = getenv("COAX_NATIVE_CHECKS_PARTS");
  const size_t parts = parts_env && *parts_env ? (size_t)strtoull(parts_env, nullptr, 10) : (size_t)std::min<uint64_t>(available_cores(), 32);
  const std::vector<std::string> units = emit_check_binary_c(file, opts, plan, parts, &sites);
  native->emit = seconds_since(t0);
  std::vector<uint8_t> has_switch(total, 0);
  for (size_t k = 0; k < sites.sites.size(); k++)
    if (sites.emitted[k]) { has_switch[(size_t)sites.sites[k].id - 1] = 1; native->emitted++; }
  if (!native->emitted) { native->why = "no mutation site of this run is in code the check binary holds"; return; }

  const char *tmp = getenv("TMPDIR");
  std::string dir = std::string(tmp && *tmp ? tmp : "/tmp") + "/coax-native-mutants.XXXXXX";
  if (!mkdtemp(dir.data())) { native->why = "no temporary directory"; return; }
  struct Cleanup {
    std::string dir;
    ~Cleanup() {
      std::error_code ec;
      if (!getenv("COAX_NATIVE_MUTANTS_KEEP")) std::filesystem::remove_all(dir, ec);
      else fprintf(stderr, "coax: native mutants: kept in %s\n", dir.c_str());
    }
  } cleanup{dir};
  t0 = std::chrono::steady_clock::now();
  std::string exe, compile_why;
  if (!native_compile(units, dir, &exe, &compile_why)) {
    native->why = compile_why.empty() ? "the C compiler failed on the mutant binary (a coax bug; COAX_NATIVE_MUTANTS_KEEP=1 keeps it)" : compile_why;
    return;
  }
  native->compile = seconds_since(t0);

  // The most processes at once: the mutation workers' count.
  uint64_t jobs = opt.jobs ? opt.jobs : job_cap("COAX_MUTATE_JOBS", std::min<uint64_t>(available_cores(), 1024));
  if (const char *j = getenv("COAX_NATIVE_MUTANTS_JOBS")) { uint64_t v = 0; if (parse_count(j, &v) && v) jobs = v; }
  double check_seconds = 120;
  if (const char *s = getenv("COAX_CHECK_SECONDS")) { double v = strtod(s, nullptr); if (v > 0) check_seconds = v; }

  // The unmutated run: which checks pass as native code, and each one's ticks
  // and time. Process w of n runs rows w, w+n, ...; one a check stops starts
  // again at its next row.
  const size_t rows = plan.checks.size();
  std::vector<uint64_t> ticks(rows, 0);
  std::vector<double> seconds(rows, 0);
  std::vector<uint8_t> passed(rows, 0);
  {
    t0 = std::chrono::steady_clock::now();
    const size_t slices = (size_t)std::max<uint64_t>(1, std::min<uint64_t>(jobs, rows));
    struct Slice { size_t start; int64_t current = -1; std::chrono::steady_clock::time_point began; };
    std::vector<Slice> slice(slices);
    for (size_t w = 0; w < slices; w++) slice[w].start = w;
    native_mutant_pool(slices, slices, check_seconds,
      [&](size_t w) -> std::vector<std::string> {
        slice[w].current = -1;
        if (slice[w].start >= rows) return {};
        return {exe, std::to_string(slice[w].start), std::to_string(slices)};
      },
      [&](size_t w, const std::string &text) -> double {
        Slice &k = slice[w];
        if (text.size() < 2) return -1;
        if (text[0] == 'B') { k.current = strtoll(text.c_str() + 2, nullptr, 10); k.began = std::chrono::steady_clock::now(); return check_seconds; }
        if (k.current < 0 || (size_t)k.current >= rows) return -1;
        if (text[0] == 'K') { const char *at = text.c_str() + 2; char *end; strtoll(at, &end, 10); ticks[(size_t)k.current] = strtoull(end, nullptr, 10); }
        if (text[0] == 'E') {
          passed[(size_t)k.current] = 1;
          seconds[(size_t)k.current] = seconds_since(k.began);
          k.start = (size_t)k.current + slices;
          k.current = -1;
        }
        return -1;
      },
      [&](size_t w, int, bool) -> bool {
        Slice &k = slice[w];
        if (k.current < 0) return false;  // done, or stopped between checks: nothing more of this slice is judged
        k.start = (size_t)k.current + slices;  // the check that stopped it is the evaluator's
        return k.start < rows;
      });
    native->baseline = seconds_since(t0);
  }
  size_t judged = 0;
  for (size_t row = 0; row < rows; row++) {
    // A #forall's bounds are constants, folded before any mutant: the binary's cases are every mutant's.
    if (!passed[row]) continue;
    native->judged[(size_t)plan.checks[row].declaration_index] = 1;
    judged++;
  }
  if (!judged) { native->why = "no check passed as native code"; return; }

  // Each mutant's schedule is its procedure's: the checks the evaluator would
  // run for it (MS_run) that native code can run, those that enter the
  // procedure first, each group cheapest first by the evaluator's steps. So a
  // native kill names a cheap check for the evaluator to confirm.
  std::vector<int32_t> cheap;
  for (int32_t i = 0; i < (int32_t)checks; i++) cheap.push_back(i);
  std::stable_sort(cheap.begin(), cheap.end(), [&](int32_t a, int32_t b) { return base[(size_t)a] < base[(size_t)b]; });
  struct Schedule { std::string path; bool whole = true; std::vector<int32_t> order; double seconds = 0; };
  std::map<const ProcDecl *, Schedule> schedules;
  for (size_t i : owned) {
    if (!has_switch[i]) continue;
    const ProcDecl *owner = mutants[i].owner;
    if (schedules.count(owner)) continue;
    Schedule s;
    std::vector<int32_t> later;
    for (int32_t c : cheap) {
      const auto &path = paths[(size_t)c];
      const bool entered = !owner || std::find(path.begin(), path.end(), owner) != path.end();
      if (!MS_run(selective, entered)) continue;
      if (!native->judged[(size_t)c]) { s.whole = false; continue; }
      (entered ? s.order : later).push_back(c);
    }
    s.order.insert(s.order.end(), later.begin(), later.end());
    s.path = dir + "/schedule" + std::to_string(schedules.size());
    std::string text;
    for (int32_t c : s.order) {
      const size_t row = (size_t)native->rows[(size_t)c];
      text += std::to_string(row) + " " + std::to_string((unsigned long long)NM_tick_limit(ticks[row])) + "\n";
      s.seconds += seconds[row];
    }
    if (!native_write(s.path, text)) { native->why = "can't write a schedule"; return; }
    schedules.emplace(owner, std::move(s));
  }

  // The mutants: one process each. It stops at its first failing check.
  std::vector<size_t> work;
  for (size_t i : owned) if (has_switch[i] && !schedules[mutants[i].owner].order.empty()) work.push_back(i);
  struct Run { int64_t current = -1; uint32_t stop = 0; bool survived = false; };
  std::vector<Run> runs(work.size());
  auto row_seconds = [&](int64_t row) {
    return (double)NM_wall_ms((uint64_t)((row >= 0 && (size_t)row < rows ? seconds[(size_t)row] : 0) * 1000)) / 1000.0;
  };
  t0 = std::chrono::steady_clock::now();
  native_mutant_pool(work.size(), std::max<uint64_t>(1, std::min<uint64_t>(jobs, work.size())), check_seconds,
    [&](size_t t) -> std::vector<std::string> {
      const size_t i = work[t];
      std::vector<std::string> args = {exe, "m", std::to_string(i + 1), schedules[mutants[i].owner].path};
      // The check a --killers file names for it runs first, when native code runs it at all.
      const int32_t first = hints->empty() ? -1 : (*hints)[i];
      if (first >= 0 && (size_t)first < checks && native->judged[(size_t)first]) args.push_back(std::to_string(native->rows[(size_t)first]));
      return args;
    },
    [&](size_t t, const std::string &text) -> double {
      Run &r = runs[t];
      if (text.empty()) return -1;
      if (text[0] == 'B' && text.size() > 2) { r.current = strtoll(text.c_str() + 2, nullptr, 10); return row_seconds(r.current); }
      if (text[0] == 'T') r.stop = NM_TRAP;
      else if (text[0] == 'R') r.stop = NM_REFUSAL;
      else if (text[0] == 'L') r.stop = NM_TICKS;
      else if (text[0] == 'S') r.survived = true;
      return -1;
    },
    [&](size_t t, int status, bool timed_out) -> bool {
      Run &r = runs[t];
      const size_t i = work[t];
      const Schedule &s = schedules[mutants[i].owner];
      if (r.survived && status >= 0 && WIFEXITED(status) && WEXITSTATUS(status) == 0) {
        native->outcome[i] = s.whole ? NM_SURVIVED : NM_SURVIVED_PART;
        return false;
      }
      // Stopped before any check began: the binary, not the mutant. Not run.
      if (r.current < 0 || (size_t)r.current >= rows) return false;
      native->outcome[i] = timed_out ? NM_WALL : r.stop ? r.stop : NM_CRASH;
      native->killer[i] = (int32_t)plan.checks[(size_t)r.current].declaration_index;
      return false;
    });
  native->run = seconds_since(t0);
  native->ran = true;

  if (hints->empty()) hints->assign(total, -1);
  for (size_t i : owned)
    if (NM_killed(native->outcome[i])) (*hints)[i] = native->killer[i];

  if (const char *report = getenv("COAX_NATIVE_MUTANTS_REPORT")) {
    if (*report) {
      const std::vector<std::string> keys = mutant_keys(file, mutants);
      std::string text;
      for (size_t i : owned) {
        const int32_t k = native->killer[i];
        text += std::to_string(i) + "\t" + native_mutant_outcome(native->outcome[i]) + "\t" +
                (k >= 0 ? killer_field(to_std(file->checks[k]->name)) : "") + "\t" + keys[i] + "\n";
      }
      if (!native_write(report, text)) fprintf(stderr, "coax: native mutants: cannot write '%s'\n", report);
    }
  }
}

// After the evaluator: each mutant's native outcome against its verdict.
// The verdicts stand whatever this finds; it reports the native path.
static void native_mutants_compare(const File *file, const std::vector<Mutant> &mutants, const std::vector<size_t> &owned,
                                   const std::vector<MutantResult> &results, NativeMutants &native, const MutationOptions &opt) {
  if (!native.ran) return;
  size_t agree = 0, limits = 0, disagree = 0, apart = 0;
  for (size_t i : owned) {
    const MutantResult &r = results[i];
    const uint32_t evaluator = r.killer < 0 ? 0 : r.stop == 2 ? 2 : 1;
    const bool same = r.killer >= 0 && r.killer == native.killer[i];
    const bool ran = r.killer >= 0 && (size_t)r.killer < native.judged.size() && native.judged[(size_t)r.killer];
    const uint32_t verdict = NM_compare(native.outcome[i], same, ran, evaluator);
    if (verdict == 0) agree++;
    else if (verdict == 1) limits++;
    else if (verdict == 3) apart++;
    if (verdict == 2) disagree++;
    if (g_json_errors || (verdict != 2 && !(verdict == 1 && opt.stats))) continue;
    const Mutant &mu = mutants[i];
    auto point = source_point(file->src, diagnostic_loc(file->src, mu.loc));
    const std::string native_text = std::string(native_mutant_outcome(native.outcome[i])) +
        (native.killer[i] >= 0 ? " in '" + to_std(file->checks[native.killer[i]]->name) + "'" : "");
    const std::string evaluator_text = r.killer < 0 ? "survived" :
        std::string(r.stop == 2 ? "stopped at its own limit in '" : "killed by '") + to_std(file->checks[r.killer]->name) + "'";
    if (verdict == 2) {
      fprintf(stderr, "coax: native mutants disagree with the evaluator: mutant %zu (%s:%d:%d: %s): native code: %s; the evaluator: %s\n",
              i, point.path.c_str(), point.loc.line, point.loc.col, mu.what.c_str(), native_text.c_str(), evaluator_text.c_str());
    } else {
      fprintf(stderr, "native-mutant-limits %zu %s:%d:%d: %s: native code: %s; the evaluator: %s\n",
              i, point.path.c_str(), point.loc.line, point.loc.col, mu.what.c_str(), native_text.c_str(), evaluator_text.c_str());
    }
  }
  if (disagree) g_native_mutants_disagreed = true;
  if (disagree && !g_json_errors) {
    fprintf(stderr, "coax: native mutants: %zu disagreement%s with the evaluator: a coax bug in the native path. Every verdict in the report is the evaluator's.\n",
            disagree, disagree == 1 ? "" : "s");
  }
  if (opt.stats && !g_json_errors)
    fprintf(stderr, "native-mutants agree=%zu limits=%zu disagree=%zu not-compared=%zu emit=%.2f compile=%.2f baseline=%.2f run=%.2f after-baseline=%.2f\n",
            agree, limits, disagree, apart, native.emit, native.compile, native.baseline, native.run, seconds_since(native.began));
}

}  // namespace coax
