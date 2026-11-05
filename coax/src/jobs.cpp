// The machine-wide build-worker budget (ax-job-budget). Every coax process
// that runs work in parallel (#authority checks, standalone library checks,
// mutation workers) draws its workers from one pool of N tokens shared by
// every coax process on the machine, so several builds at once share the
// cores instead of each forking one worker per core.
//
// A token is a file, $COAX_JOBS_DIR/worker-K (default ~/.cache/coax/jobs/),
// held with an exclusive flock(2). The kernel drops a flock when the last
// descriptor on it closes, so a process that dies or is killed frees its
// tokens at once: no token is ever leaked, and nothing needs cleaning up.
// A holder writes its pid and command into the file, for `coax status`.
// This is GNU make's jobserver idea with lock files in place of a pipe, so
// unrelated processes share it without a common parent.
//
//   N        $COAX_JOBS_BUDGET, or the performance cores less the reserve.
//   reserve  $COAX_JOBS_RESERVE, or a third of the performance cores (at
//            least one): left for the person using the machine.
//   load     no worker past a process's first starts while the 1-minute
//            load average is at or above $COAX_JOBS_LOAD (default: the
//            budget, so outside load eats into coax, not the reserve; 0
//            turns this off).
//
// A process always has one worker: its first takes a token when one is
// free and runs without one when none is (shown by `coax status` as over
// the budget, through a locked over-PID file), so no build ever waits for
// another to finish. Each further worker needs a token; a process tries
// again for one while it has work waiting, and gives each back as soon as
// that worker ends. A forked worker that fans out again (a library check
// running its own #authority checks) has its parent's token as its first.
//
// A process's own cap is --jobs N, else $COAX_CHECK_JOBS (checks) or
// $COAX_MUTATE_JOBS (mutants), else $COAX_JOBS, else N. The pool decides
// only how many workers run at once, never what they compute, so every
// result is the same for every count. A build that can't get the workers
// it could use says so once on stderr, but only when stderr is a terminal:
// the notice is for the person watching, and output a program captures
// (a test comparing stderr, a log) must not depend on what else runs on
// the machine (GitHub #12939). $COAX_JOBS_QUIET=1 silences it everywhere;
// $COAX_JOBS_QUIET=0 prints it off a terminal too. Never under --json or
// --diagnostics json: there stderr is the machine-readable stream, one JSON
// object per line, and a text notice would break it (GitHub #12937).
// $COAX_JOBS_LOAD_NOW stands in for the load average, so a test can put a
// build over $COAX_JOBS_LOAD at will.
#include "coax.h"

#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <set>
#include <string>
#include <vector>
#include <dirent.h>
#include <fcntl.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <unistd.h>
#include <csignal>
#include <thread>
#ifdef __APPLE__
#include <sys/event.h>
#include <sys/sysctl.h>
#endif
#ifdef __linux__
#include <sched.h>
#include <sys/prctl.h>
#endif

namespace coax {

static std::string json_escape_basic(const std::string &s);  // verify.cpp

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

// The cores that are not efficiency cores: on Apple silicon, every
// performance level not named "Efficiency" (the M5 Max's Super and
// Performance cores); elsewhere every available core.
static uint64_t performance_cores() {
  uint64_t cores = 0;
#ifdef __APPLE__
  int levels = 0;
  size_t size = sizeof levels;
  if (sysctlbyname("hw.nperflevels", &levels, &size, nullptr, 0) == 0)
    for (int i = 0; i < levels; i++) {
      char key[64], name[64] = {0};
      int count = 0;
      size = sizeof name - 1;
      snprintf(key, sizeof key, "hw.perflevel%d.name", i);
      const bool named = sysctlbyname(key, name, &size, nullptr, 0) == 0;
      if (named && strcmp(name, "Efficiency") == 0) continue;
      size = sizeof count;
      snprintf(key, sizeof key, "hw.perflevel%d.logicalcpu", i);
      if (sysctlbyname(key, &count, &size, nullptr, 0) == 0 && count > 0) cores += (uint64_t)count;
    }
#endif
  const uint64_t available = available_cores();
  if (!cores || cores > available) cores = available;
  return cores;
}

struct JobPool {
  bool configured = false;
  bool shared = false;           // the token directory is usable
  uint64_t cores = 1, reserve = 0, budget = 1;
  double load_limit = 0;         // 0: no load backoff
  std::string dir, why;          // why: the reason there is no shared pool
  std::string command;           // this process's command, written into its tokens
  uint64_t override_jobs = 0;    // --jobs N (0: none)
  bool inherited = false;        // a forked worker: its first worker is its parent's token
  std::vector<int> held;         // descriptors of the tokens this process holds
  bool told_wait = false, told_load = false;
};
static JobPool g_jobs;

static uint64_t env_count(const char *name, bool *set) {
  uint64_t v = 0;
  const char *text = getenv(name);
  *set = text && parse_count(text, &v);
  return v;
}

static JobPool &job_pool() {
  JobPool &p = g_jobs;
  if (p.configured) return p;
  p.configured = true;
  p.cores = performance_cores();
  bool set = false;
  uint64_t reserve = env_count("COAX_JOBS_RESERVE", &set);
  p.reserve = set ? std::min<uint64_t>(reserve, p.cores - 1) : std::max<uint64_t>(1, p.cores / 3);
  if (p.reserve >= p.cores) p.reserve = p.cores - 1;
  uint64_t budget = env_count("COAX_JOBS_BUDGET", &set);
  p.budget = set && budget ? std::min<uint64_t>(budget, 1024) : p.cores - p.reserve;
  uint64_t load = env_count("COAX_JOBS_LOAD", &set);
  p.load_limit = set ? (double)load : (double)p.budget;
  // The waiting and load notices go to a terminal only; $COAX_JOBS_QUIET=1
  // drops them, $COAX_JOBS_QUIET=0 prints them to any stderr. `coax status`
  // still shows the budget.
  const char *quiet = getenv("COAX_JOBS_QUIET");
  const bool always = quiet && strcmp(quiet, "0") == 0;
  if (!always && ((quiet && *quiet) || !isatty(STDERR_FILENO))) p.told_wait = p.told_load = true;
  const char *dir = getenv("COAX_JOBS_DIR");
  if (dir && *dir) p.dir = dir;
  else if (const char *home = getenv("HOME"); home && *home) p.dir = std::string(home) + "/.cache/coax/jobs";
  if (p.dir.empty()) { p.why = "no $HOME or $COAX_JOBS_DIR"; return p; }
  // mkdir -p, then check it is a writable directory.
  for (size_t at = 1; at <= p.dir.size(); at++)
    if (at == p.dir.size() || p.dir[at] == '/') mkdir(p.dir.substr(0, at).c_str(), 0755);
  struct stat st;
  if (stat(p.dir.c_str(), &st) != 0 || !S_ISDIR(st.st_mode) || access(p.dir.c_str(), W_OK | X_OK) != 0) {
    p.why = "can't write " + p.dir;
    return p;
  }
  p.shared = true;
  return p;
}

static std::string job_token_path(const JobPool &p, uint64_t k) {
  char name[32];
  snprintf(name, sizeof name, "/worker-%03llu", (unsigned long long)k);
  return p.dir + name;
}

// Takes a free token without waiting: its descriptor, or -1 when every one
// is held (or the pool is not shared).
static int job_take() {
  JobPool &p = job_pool();
  if (!p.shared) return -1;
  for (uint64_t k = 0; k < p.budget; k++) {
    const std::string path = job_token_path(p, k);
    int fd = open(path.c_str(), O_RDWR | O_CREAT | O_CLOEXEC, 0644);
    if (fd < 0) continue;
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); continue; }
    char line[512];
    int n = snprintf(line, sizeof line, "%lld\t%lld\t%s\n", (long long)getpid(), (long long)getppid(), p.command.c_str());
    if (n < 0) n = 0;
    if (n >= (int)sizeof line) n = (int)sizeof line - 1;
    if (ftruncate(fd, 0) == 0) { ssize_t w = pwrite(fd, line, (size_t)n, 0); (void)w; }
    p.held.push_back(fd);
    return fd;
  }
  return -1;
}

// A first worker that runs without a token (none was free) is still shown:
// $COAX_JOBS_DIR/over-PID, held with a flock like a token, so `coax status`
// lists it as over the budget. A dead process's file is unlocked and ignored.
static int job_over() {
  JobPool &p = job_pool();
  if (!p.shared) return -1;
  const std::string path = p.dir + "/over-" + std::to_string((long long)getpid());
  int fd = open(path.c_str(), O_RDWR | O_CREAT | O_CLOEXEC, 0644);
  if (fd < 0) return -1;
  if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); return -1; }
  char line[512];
  int n = snprintf(line, sizeof line, "%lld\t%lld\t%s\n", (long long)getpid(), (long long)getppid(), p.command.c_str());
  if (n < 0) n = 0;
  if (n >= (int)sizeof line) n = (int)sizeof line - 1;
  if (ftruncate(fd, 0) == 0) { ssize_t w = pwrite(fd, line, (size_t)n, 0); (void)w; }
  p.held.push_back(fd);
  return fd;
}

static void job_give(int fd) {
  if (fd < 0) return;
  JobPool &p = g_jobs;
  for (size_t i = 0; i < p.held.size(); i++)
    if (p.held[i] == fd) { p.held.erase(p.held.begin() + (std::ptrdiff_t)i); break; }
  close(fd);  // the last descriptor on the file: the flock goes with it
}

// A forked worker never outlives the process that forked it (`parent`, its
// pid from before the fork): a killed coax frees its tokens at once, and its
// workers stop within a moment instead of finishing their current check.
// Linux asks the kernel for SIGKILL on the parent's death. macOS has no such
// call, so a thread waits on a kqueue for the parent's exit (EVFILT_PROC
// NOTE_EXIT), or, where that can't be set up, polls getppid() every 100 ms.
// Each then ends the worker; its own workers watch it in turn.
static void job_die_with_parent(pid_t parent) {
#ifdef __linux__
  prctl(PR_SET_PDEATHSIG, SIGKILL);
#else
#ifdef __APPLE__
  int kq = kqueue();
  if (kq >= 0) {
    struct kevent watch;
    EV_SET(&watch, parent, EVFILT_PROC, EV_ADD | EV_ONESHOT, NOTE_EXIT, 0, nullptr);
    if (kevent(kq, &watch, 1, nullptr, 0, nullptr) == 0) {
      std::thread([kq] {
        struct kevent fired;
        while (kevent(kq, nullptr, 0, &fired, 1, nullptr) < 0 && errno == EINTR) {}
        _exit(1);
      }).detach();
    } else if (errno == ESRCH) {
      _exit(1);  // the parent is already gone
    } else {
      close(kq);
      kq = -1;
    }
  }
  if (kq < 0)
#endif
    std::thread([parent] {
      while (getppid() == parent) usleep(100000);
      _exit(1);
    }).detach();
#endif
  // The parent may have died before the watch was set.
  if (getppid() != parent) _exit(1);
}

// In a forked worker: end with `parent` (above), drop the copies of this
// process's token descriptors (the parent gives each back when its worker
// ends), and run any fan-out of its own under the token its parent holds
// for it.
static void job_child(pid_t parent) {
  job_die_with_parent(parent);
  for (int fd : g_jobs.held) close(fd);
  g_jobs.held.clear();
  g_jobs.inherited = true;
  g_jobs.told_wait = g_jobs.told_load = true;  // a worker's stderr is not the build's
}

struct JobHolder { long long pid = 0, parent = 0; std::string command; uint64_t tokens = 0, over = 0; };

// The holder written in a held token or over- file, or false when it is free.
static bool job_read_holder(const std::string &path, long long *pid, long long *parent, std::string *command) {
  int fd = open(path.c_str(), O_RDONLY | O_CLOEXEC);
  if (fd < 0) return false;
  if (flock(fd, LOCK_SH | LOCK_NB) == 0) { close(fd); return false; }  // free
  char text[512];
  ssize_t n = pread(fd, text, sizeof text - 1, 0);
  close(fd);
  text[n > 0 ? n : 0] = 0;
  // "pid<TAB>parent pid<TAB>command"
  char *tab = strchr(text, '\t');
  char *tab2 = tab ? strchr(tab + 1, '\t') : nullptr;
  *pid = atoll(text);
  *parent = tab ? atoll(tab + 1) : 0;
  *command = tab2 ? std::string(tab2 + 1) : std::string();
  while (!command->empty() && command->back() == '\n') command->pop_back();
  return true;
}

// Who holds the pool's tokens now, and who runs a first worker over the
// budget: one row per process, most tokens first. *held_total: the tokens held.
static std::vector<JobHolder> job_holders(uint64_t *held_total, uint64_t *over_total = nullptr) {
  JobPool &p = job_pool();
  std::map<long long, JobHolder> by_pid;
  *held_total = 0;
  if (over_total) *over_total = 0;
  if (!p.shared) return {};
  auto note = [&](long long pid, long long parent, const std::string &command, bool over) {
    JobHolder &h = by_pid[pid];
    h.pid = pid;
    h.parent = parent;
    if (h.command.empty()) h.command = command;
    (over ? h.over : h.tokens)++;
  };
  long long pid = 0, parent = 0;
  std::string command;
  for (uint64_t k = 0; k < p.budget; k++)
    if (job_read_holder(job_token_path(p, k), &pid, &parent, &command)) { ++*held_total; note(pid, parent, command, false); }
  if (DIR *d = opendir(p.dir.c_str())) {
    while (dirent *e = readdir(d)) {
      if (strncmp(e->d_name, "over-", 5) != 0) continue;
      const std::string path = p.dir + "/" + e->d_name;
      if (job_read_holder(path, &pid, &parent, &command)) { if (over_total) ++*over_total; note(pid, parent, command, true); }
      else if (atoll(e->d_name + 5) != (long long)getpid()) unlink(path.c_str());  // its process is gone
    }
    closedir(d);
  }
  std::vector<JobHolder> rows;
  for (auto &[id, h] : by_pid) rows.push_back(h);
  std::stable_sort(rows.begin(), rows.end(), [](const JobHolder &a, const JobHolder &b) { return a.tokens > b.tokens; });
  return rows;
}

static double job_load() {
  if (const char *now = getenv("COAX_JOBS_LOAD_NOW"); now && *now) {
    char *end = nullptr;
    const double v = strtod(now, &end);
    if (end && !*end && v >= 0) return v;
  }
  double load[1] = {0};
  return getloadavg(load, 1) == 1 ? load[0] : 0;
}

// The most workers one fan-out in this process runs at once: --jobs N, else
// the first of the named variables that holds a count, else the budget.
static uint64_t job_cap(const char *specific, uint64_t most) {
  JobPool &p = job_pool();
  uint64_t jobs = p.override_jobs;
  bool set = false;
  if (!jobs && specific) { jobs = env_count(specific, &set); if (!set) jobs = 0; }
  if (!jobs) { jobs = env_count("COAX_JOBS", &set); if (!set) jobs = 0; }
  if (!jobs) jobs = p.budget;
  return std::max<uint64_t>(1, std::min<uint64_t>(jobs, most));
}

// One fan-out's workers. start() says whether one more may start now; each
// started worker ends with stop(). The first always may.
struct JobSlots {
  uint64_t cap = 1;
  std::vector<int> slots;  // a token per running worker; -1 for a first worker without one
  int over = -1;           // job_over()'s file while a first worker runs without a token

  explicit JobSlots(uint64_t most) : cap(std::max<uint64_t>(1, most)) {}
  ~JobSlots() { for (int fd : slots) job_give(fd); drop_over(); }
  void drop_over() {
    if (over < 0) return;
    unlink((g_jobs.dir + "/over-" + std::to_string((long long)getpid())).c_str());
    job_give(over);
    over = -1;
  }
  size_t running() const { return slots.size(); }

  bool start() {
    if (slots.size() >= cap) return false;
    JobPool &p = job_pool();
    if (slots.empty()) {
      // The first worker: the parent's token in a forked worker, else a
      // token when one is free, else none (a build always progresses).
      slots.push_back(p.inherited ? -1 : job_take());
      if (slots.back() < 0 && !p.inherited) { over = job_over(); tell_wait(); }
      return true;
    }
    if (p.load_limit > 0 && slots.size() >= 1) {
      const double load = job_load();
      if (load >= p.load_limit) { tell_load(load); return false; }
    }
    if (!p.shared) { slots.push_back(-1); return true; }  // no pool: up to the cap, unshared (coax status says why)
    const int fd = job_take();
    if (fd < 0) { tell_wait(); return false; }
    slots.push_back(fd);
    return true;
  }

  void stop() {
    if (slots.empty()) return;
    // Give back a token first; a first worker without one is the last to go.
    for (size_t i = slots.size(); i-- > 0;)
      if (slots[i] >= 0) { job_give(slots[i]); slots.erase(slots.begin() + (std::ptrdiff_t)i); return; }
    slots.pop_back();
    drop_over();
  }

  void tell_wait() {
    JobPool &p = job_pool();
    if (p.told_wait || !p.shared || g_json_errors) return;  // no text in a JSON stream
    p.told_wait = true;
    uint64_t held = 0;
    std::vector<JobHolder> rows = job_holders(&held);
    uint64_t others = 0;
    // Not this process, nor its own forked workers fanning out in turn.
    for (const JobHolder &h : rows) others += h.pid != (long long)getpid() && h.parent != (long long)getpid();
    const uint64_t want = std::min<uint64_t>(cap - std::min<uint64_t>(cap, slots.size()), p.budget);
    if (!want || !others) return;
    fprintf(stderr, "coax jobs: waiting for %llu of %llu build workers held by %llu other coax process%s; running %zu now (coax status shows them)\n",
            (unsigned long long)want, (unsigned long long)p.budget, (unsigned long long)others, others == 1 ? "" : "es",
            std::max<size_t>(1, slots.size()));
  }

  void tell_load(double load) {
    JobPool &p = job_pool();
    if (p.told_load || g_json_errors) return;  // no text in a JSON stream
    p.told_load = true;
    fprintf(stderr, "coax jobs: the load average is %.1f, at or above %.0f ($COAX_JOBS_LOAD); running %zu build worker%s, not up to %llu, until it drops\n",
            load, p.load_limit, slots.size(), slots.size() == 1 ? "" : "s", (unsigned long long)cap);
  }
};

static bool by_holder(const std::vector<JobHolder> &rows, long long pid) {
  for (const JobHolder &h : rows) if (h.pid == pid) return true;
  return false;
}

// `coax status`: the budget, the reserve and who holds tokens now.
static void job_status(FILE *out, bool json) {
  JobPool &p = job_pool();
  uint64_t held = 0, over = 0;
  std::vector<JobHolder> rows = job_holders(&held, &over);
  const double load = job_load();
  bool budget_set = false;
  env_count("COAX_JOBS_BUDGET", &budget_set);
  budget_set = budget_set && p.budget != p.cores - p.reserve;
  if (json) {
    std::string s = "{\"budget\":" + std::to_string(p.budget) + ",\"performance_cores\":" + std::to_string(p.cores) +
                    ",\"reserve\":" + std::to_string(p.reserve) + ",\"budget_from_environment\":" + (budget_set ? "true" : "false") +
                    ",\"held\":" + std::to_string(held) + ",\"over_budget\":" + std::to_string(over) +
                    ",\"shared\":" + (p.shared ? "true" : "false") + ",\"directory\":\"" + json_escape_basic(p.dir) + "\"" +
                    ",\"load\":" + std::to_string(load) + ",\"load_limit\":" + std::to_string((uint64_t)p.load_limit) + ",\"holders\":[";
    for (size_t i = 0; i < rows.size(); i++)
      s += std::string(i ? "," : "") + "{\"pid\":" + std::to_string(rows[i].pid) + ",\"parent\":" + std::to_string(rows[i].parent) +
           ",\"tokens\":" + std::to_string(rows[i].tokens) + ",\"over_budget\":" + std::to_string(rows[i].over) +
           ",\"command\":\"" + json_escape_basic(rows[i].command) + "\"}";
    s += "]}\n";
    fputs(s.c_str(), out);
    return;
  }
  fprintf(out, "Build workers: %llu of %llu in use by %zu coax process%s", (unsigned long long)held, (unsigned long long)p.budget,
          rows.size(), rows.size() == 1 ? "" : "es");
  if (over) fprintf(out, ", and %llu over the budget (a build's first worker when none was free)", (unsigned long long)over);
  fprintf(out, "\n");
  if (budget_set)
    fprintf(out, "  budget %llu from $COAX_JOBS_BUDGET (%llu performance cores; the default leaves %llu for you, $COAX_JOBS_RESERVE)\n",
            (unsigned long long)p.budget, (unsigned long long)p.cores, (unsigned long long)p.reserve);
  else
    fprintf(out, "  budget %llu = %llu performance cores - %llu reserved for you ($COAX_JOBS_RESERVE; $COAX_JOBS_BUDGET sets it)\n",
            (unsigned long long)p.budget, (unsigned long long)p.cores, (unsigned long long)p.reserve);
  if (p.load_limit > 0)
    fprintf(out, "  load %.1f; no new workers past each build's first at %.0f or above ($COAX_JOBS_LOAD)\n", load, p.load_limit);
  else
    fprintf(out, "  load %.1f; load backoff off ($COAX_JOBS_LOAD=0)\n", load);
  fprintf(out, "  each build's own cap: --jobs N, $COAX_CHECK_JOBS, $COAX_MUTATE_JOBS, $COAX_JOBS\n");
  if (p.shared) fprintf(out, "  tokens in %s\n", p.dir.c_str());
  else fprintf(out, "  no shared pool (%s): each process runs up to %llu workers\n", p.why.c_str(), (unsigned long long)p.budget);
  for (const JobHolder &h : rows) {
    fprintf(out, "  pid %lld holds %llu", h.pid, (unsigned long long)h.tokens);
    if (h.over) fprintf(out, " and runs 1 over the budget");
    fprintf(out, ": coax %s%s\n", h.command.c_str(), by_holder(rows, h.parent) ? " (a worker of the build above)" : "");
  }
}

}  // namespace coax
