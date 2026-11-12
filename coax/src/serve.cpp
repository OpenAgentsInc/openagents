// The resident compiler (`coax serve`, #13138). Included by main.cpp after
// main_impl, finish_profile and report_stale_compiler.
//
// A resident is one long-lived process per compiler binary (by content) and
// user. `coax c`, `coax test` and the other compile commands connect to it,
// pass their arguments, directory, environment and standard streams, and the
// resident forks a child that runs the command exactly as a new process
// would. The child differs from a cold process in one way: it inherits the
// resident's memo, so work that is a pure function of a file's bytes is not
// done again:
//
//   - a unit's lex, its unit-syntax check and the SHA-256 of its bytes and of
//     its tokens (UnitLex, keyed by the bytes);
//   - the compiler binary's SHA-256.
//
// Each entry is found by comparing the whole key, so a child computes the
// same values a cold compile computes, and its output is the same bytes. A
// child sends what it computed back to the resident when its command ends;
// the resident never runs compiler phases itself.
//
// Failure is safe. A client that finds no resident starts one and compiles
// cold. A resident that refuses (another binary), does not answer, or whose
// request dies makes the client compile cold. COAX_SERVE=0 turns it off.
//
// Bounded (#13323, #13310). One resident serves every checkout whose compiler
// has its bytes, so it must not grow with them: at most COAX_SERVE_JOBS
// requests run at once (default the machine's cores); the next ones wait in
// its queue (their clients told so) and start as running ones end. A request
// runs at most COAX_SERVE_TIMEOUT seconds (default 3600); then it is killed
// and its client exits 124. A request ends with its client (the resident
// kills it when the client's socket closes), with its resident (it dies with
// its parent, as a build worker does: jobs.cpp), and within 30 s of its reply
// (its reports to the resident are the resident's memo, never worth waiting
// on). A resident whose binary is gone or replaced takes no new request and
// leaves once its running ones end, so a resident from a live binary can
// start beside it. The log of the previous resident is kept as log.1.
#include <cstdarg>
#include <ctime>
#include <poll.h>
#include <signal.h>
#include <sys/file.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unordered_map>

namespace coax {
namespace {

constexpr uint32_t SERVE_MAGIC = 0x53584f43;  // "COXS"
constexpr uint32_t SERVE_PROTOCOL = 1;
enum : uint32_t { SERVE_RUN = 1, SERVE_STATUS = 2, SERVE_STOP = 3 };
// A request's reports to its resident.
enum : uint32_t { REPORT_RUNNING = 1, REPORT_REPLIED = 2, REPORT_UNIT = 3, REPORT_COMPILER = 4, REPORT_USED = 5, REPORT_END = 6, REPORT_PASSED = 7, REPORT_ANSWERED = 8 };

struct ServeEntry {
  std::shared_ptr<const UnitLex> unit;
  uint64_t used = 0;  // the resident's request count when a request last used it
};
struct ServeState {
  bool request = false;  // this process is a request a resident forked: lookups are on
  std::unordered_map<uint64_t, std::vector<ServeEntry>> units;  // by serve_hash of the bytes
  uint64_t bytes = 0;
  std::string compiler;    // the binary's SHA-256, once a request computed it
  // The last compiles whose type check passed, by entry unit, newest first
  // (skeleton.cpp): at most PASSED_EACH an entry (one for each set of
  // options) and PASSED_ENTRIES entries, the least recently used out first.
  struct Passed { std::vector<std::shared_ptr<const PassedSkeleton>> last; uint64_t used = 0; };
  std::map<std::string, Passed> passed;
  std::vector<std::pair<std::string, std::shared_ptr<const PassedSkeleton>>> passed_fresh;  // a request's own, reported when it ends
  bool answered = false;   // a request: its errors came from the changed bodies' check
  std::string executable;  // a request: the client's path to the same binary
  // A request's own additions and uses, reported when it ends.
  std::vector<std::shared_ptr<const UnitLex>> fresh;
  std::vector<uint64_t> used;
  bool compiler_fresh = false;
  uint64_t hits = 0, misses = 0;
  int report = -1, client = -1;
};
// Never destroyed: a request leaves through exit() and must not spend its
// last moments freeing the resident's memo.
ServeState &serve_state() { static ServeState *state = new ServeState; return *state; }

constexpr size_t PASSED_EACH = 2, PASSED_ENTRIES = 6;  // a skeleton is some 14 bytes a token (fivelab: 30 MB)
uint64_t serve_hash(const char *data, size_t count) {
  uint64_t h = 0x9e3779b97f4a7c15ull ^ (uint64_t)count;
  auto mix = [&](uint64_t k) { h = (h ^ k) * 0xff51afd7ed558ccdull; h ^= h >> 29; };
  for (; count >= 8; data += 8, count -= 8) { uint64_t k; memcpy(&k, data, 8); mix(k); }
  uint64_t k = 0;
  memcpy(&k, data, count);
  mix(k);
  h = (h ^ (h >> 33)) * 0xc4ceb9fe1a85ec53ull;
  return h ^ (h >> 33);
}
uint64_t unit_bytes(const UnitLex &unit) {
  return unit.text.size() + unit.tokens.size() * sizeof(UnitToken) + unit.decoded.size() + 256;
}

// ---- Bytes on a descriptor ----

bool write_all(int fd, const void *data, size_t count) {
  const char *at = (const char *)data;
  while (count) {
#ifdef MSG_NOSIGNAL
    ssize_t n = send(fd, at, count, MSG_NOSIGNAL);
    if (n < 0 && errno == ENOTSOCK) n = write(fd, at, count);
#else
    ssize_t n = write(fd, at, count);
#endif
    if (n < 0) { if (errno == EINTR) continue; return false; }
    at += n;
    count -= (size_t)n;
  }
  return true;
}
bool read_all(int fd, void *data, size_t count) {
  char *at = (char *)data;
  while (count) {
    ssize_t n = read(fd, at, count);
    if (n < 0) { if (errno == EINTR) continue; return false; }
    if (n == 0) return false;
    at += n;
    count -= (size_t)n;
  }
  return true;
}
void no_sigpipe(int fd) {
#ifdef SO_NOSIGPIPE
  int on = 1;
  setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, sizeof on);
#else
  (void)fd;
#endif
}
void put_u32(std::string &out, uint32_t v) { out.append((const char *)&v, sizeof v); }
void put_u64(std::string &out, uint64_t v) { out.append((const char *)&v, sizeof v); }
void put_str(std::string &out, const std::string &s) { put_u64(out, s.size()); out += s; }
struct Reader {
  const char *at, *end;
  bool ok = true;
  bool take(void *out, size_t n) {
    if (!ok || (size_t)(end - at) < n) return ok = false;
    memcpy(out, at, n);
    at += n;
    return true;
  }
  uint32_t u32() { uint32_t v = 0; take(&v, sizeof v); return v; }
  uint64_t u64() { uint64_t v = 0; take(&v, sizeof v); return v; }
  std::string str() {
    const uint64_t n = u64();
    if (!ok || (uint64_t)(end - at) < n) { ok = false; return ""; }
    std::string s(at, (size_t)n);
    at += n;
    return s;
  }
};
// One framed message: magic, kind, payload length, payload.
bool send_frame(int fd, uint32_t kind, const std::string &payload) {
  std::string head;
  put_u32(head, SERVE_MAGIC);
  put_u32(head, kind);
  put_u64(head, payload.size());
  return write_all(fd, head.data(), head.size()) && write_all(fd, payload.data(), payload.size());
}

// ---- Where a binary's resident lives ----

std::string read_file_bytes(const std::string &path) {
  std::string out;
  FILE *f = fopen(path.c_str(), "rb");
  if (!f) return out;
  char buffer[1 << 16];
  size_t n;
  while ((n = fread(buffer, 1, sizeof buffer, f)) > 0) out.append(buffer, n);
  fclose(f);
  return out;
}
// $COAX_SERVE_DIR, else ~/.cache/coax-serve; then one directory per binary
// content. "" when there is nowhere to keep one, or the path is too long for
// a socket address.
std::string serve_dir(const std::string &image) {
  const char *env = getenv("COAX_SERVE_DIR"), *home = getenv("HOME");
  std::string base = env && *env ? env : home && *home ? std::string(home) + "/.cache/coax-serve" : "";
  if (base.empty() || image.empty()) return "";
  char name[64];
  snprintf(name, sizeof name, "/%016llx-%zu", (unsigned long long)serve_hash(image.data(), image.size()), image.size());
  base += name;
  return base.size() + 3 < sizeof(sockaddr_un{}.sun_path) ? base : "";
}
bool socket_address(const std::string &dir, sockaddr_un *out) {
  const std::string path = dir + "/s";
  if (path.size() >= sizeof out->sun_path) return false;
  memset(out, 0, sizeof *out);
  out->sun_family = AF_UNIX;
  memcpy(out->sun_path, path.c_str(), path.size() + 1);
  return true;
}
bool serve_off() {
#ifdef COAX_BOOTSTRAP_IMPORTER
  return true;  // the build-only stage0 neither starts nor uses a resident
#else
  const char *env = getenv("COAX_SERVE");
  return env && (!strcmp(env, "0") || !strcmp(env, "off"));
#endif
}
uint64_t env_count(const char *name, uint64_t fallback) {
  const char *env = getenv(name);
  if (!env || !*env) return fallback;
  char *end = nullptr;
  errno = 0;
  const unsigned long long v = strtoull(env, &end, 10);
  return errno || !end || *end ? fallback : (uint64_t)v;
}

// ---- A request: the command, run in a child of the resident ----

void report_send(uint32_t kind, const std::string &payload) {
  ServeState &s = serve_state();
  if (s.report >= 0 && !send_frame(s.report, kind, payload)) { close(s.report); s.report = -1; }
}
std::string unit_record(const UnitLex &unit) {
  std::string out;
  out.reserve(unit.text.size() + unit.tokens.size() * sizeof(UnitToken) + unit.decoded.size() + 64);
  put_u32(out, unit.syntax ? 1 : 0);
  put_str(out, unit.text);
  put_u64(out, unit.tokens.size());
  out.append((const char *)unit.tokens.data(), unit.tokens.size() * sizeof(UnitToken));
  put_str(out, unit.decoded);
  put_str(out, unit.text_sha);
  put_str(out, unit.token_sum);
  return out;
}
// The request's reply to its client, then what it learned to its resident.
void request_finish(int status) {
  ServeState &s = serve_state();
  fflush(stdout);
  fflush(stderr);
  report_send(REPORT_REPLIED, "");
  if (s.client >= 0) {
    const char reply[5] = {'X', (char)(status & 0xff), 0, 0, 0};
    write_all(s.client, reply, sizeof reply);
    close(s.client);
    s.client = -1;
  }
  // The command is over for its caller: let go of its streams, so a reader waiting for their end is not kept by the report below.
  const int null = open("/dev/null", O_RDWR);
  if (null >= 0) { dup2(null, 0); dup2(null, 1); dup2(null, 2); if (null > 2) close(null); }
  // The report only feeds the resident's memo: a resident too busy to take it
  // within 10 s a send loses it, and the request ends (#13323).
  if (s.report >= 0) { timeval limit{10, 0}; setsockopt(s.report, SOL_SOCKET, SO_SNDTIMEO, &limit, sizeof limit); }
  if (s.compiler_fresh) report_send(REPORT_COMPILER, s.compiler);
  for (const auto &unit : s.fresh) report_send(REPORT_UNIT, unit_record(*unit));
  for (const auto &[entry, passed] : s.passed_fresh) report_send(REPORT_PASSED, passed_record(entry, *passed));
  if (s.answered) report_send(REPORT_ANSWERED, "");
  std::string used;
  put_u64(used, s.hits);
  put_u64(used, s.misses);
  for (uint64_t h : s.used) put_u64(used, h);
  report_send(REPORT_USED, used);
  report_send(REPORT_END, "");
}
struct RunRequest {
  std::string executable, directory;
  std::vector<std::string> args, env;
  uint32_t mask = 022;
  int32_t priority = 0;
};
bool parse_run(const std::string &payload, RunRequest *out) {
  Reader r{payload.data(), payload.data() + payload.size()};
  out->executable = r.str();
  out->directory = r.str();
  out->mask = r.u32();
  out->priority = (int32_t)r.u32();
  const uint32_t argc = r.u32();
  for (uint32_t i = 0; r.ok && i < argc; i++) out->args.push_back(r.str());
  const uint32_t envc = r.u32();
  for (uint32_t i = 0; r.ok && i < envc; i++) out->env.push_back(r.str());
  return r.ok && r.at == r.end && !out->args.empty();
}
// In the forked child. Never returns.
[[noreturn]] void request_run(const RunRequest &run, int streams[3], int client, int report, pid_t resident) {
  ServeState &s = serve_state();
  s.request = true;
  s.report = report;
  s.client = client;
  s.executable = run.executable;
  setpgid(0, 0);
  // It never outlives its resident (a killed resident's requests were left
  // running with no one to stop them, #13310); its client then compiles cold.
  job_die_with_parent(resident);
  signal(SIGPIPE, SIG_DFL);
  signal(SIGCHLD, SIG_DFL);
  signal(SIGTERM, SIG_DFL);
  signal(SIGINT, SIG_DFL);
  signal(SIGHUP, SIG_DFL);
  bool ok = chdir(run.directory.c_str()) == 0;
  for (int i = 0; ok && i < 3; i++) ok = dup2(streams[i], i) == i;
  for (int i = 0; i < 3; i++) if (streams[i] > 2) close(streams[i]);
  if (!ok) _exit(125);  // before REPORT_RUNNING: the resident refuses and the client compiles cold
  umask((mode_t)run.mask);
  errno = 0;
  const int priority = getpriority(PRIO_PROCESS, 0);
  if (errno == 0 && run.priority > priority) setpriority(PRIO_PROCESS, 0, run.priority);  // a client under nice stays niced
  g_timings.start = std::chrono::steady_clock::now();
  std::vector<std::string> names;
  for (char **e = environ; e && *e; e++) {
    const char *eq = strchr(*e, '=');
    names.push_back(eq ? std::string(*e, (size_t)(eq - *e)) : std::string(*e));
  }
  for (const auto &name : names) unsetenv(name.c_str());
  for (const auto &entry : run.env) {
    const size_t eq = entry.find('=');
    if (eq != std::string::npos && eq > 0) setenv(entry.substr(0, eq).c_str(), entry.c_str() + eq + 1, 1);
  }
  std::vector<char *> argv;
  for (const auto &arg : run.args) argv.push_back((char *)arg.c_str());
  argv.push_back(nullptr);
  // The command runs only once its client says it is still waiting: a client
  // that gave up on a slow start has gone on to run the command itself.
  const char started = 'A';
  char go = 0;
  if (!write_all(client, &started, 1) || !read_all(client, &go, 1) || go != 'G') _exit(125);
  report_send(REPORT_RUNNING, "");
  // For coax/tests/serve.coax: a request that takes this long before its command.
  if (const char *hold = getenv("COAX_SERVE_TEST_HOLD_MS")) {
    const unsigned long long ms = std::min(60000ull, strtoull(hold, nullptr, 10));
    timespec wait{(time_t)(ms / 1000), (long)(ms % 1000) * 1000000L};
    while (nanosleep(&wait, &wait) != 0 && errno == EINTR) {}
  }
  int status = finish_profile(main_impl((int)run.args.size(), argv.data()));
  if (status != 0) report_stale_compiler();
  request_finish(status);
  exit(status);
}

// ---- The resident ----

struct Resident {
  std::string dir, image;
  int log = -1, listener = -1, lock = -1;
  uint64_t requests = 0, hits = 0, misses = 0, evicted = 0, limit = 0, idle = 0, answered = 0;
  uint64_t cap = 1, timeout = 0, queued_total = 0, timed_out = 0, lingered = 0, log_lines = 0;
  bool draining = false;  // its binary is gone or replaced: no new requests, and it leaves once the running ones end
  std::chrono::steady_clock::time_point began = std::chrono::steady_clock::now(), active = began, binary_checked = began;
  struct Live {
    pid_t pid = -1;
    int client = -1, report = -1;
    std::string in;
    bool running = false, replied = false, ended = false, killed = false;
    std::string what;
    std::chrono::steady_clock::time_point began, replied_at;
  };
  std::vector<Live> live;
  // A request that came while cap requests ran: its client is told 'Q' and waits.
  struct Waiting {
    int client = -1;
    int streams[3] = {-1, -1, -1};
    RunRequest run;
    std::string what;
  };
  std::vector<Waiting> waiting;
  std::map<std::string, std::string> verified;  // a client's binary path -> its stat identity, once its bytes matched
  bool stop = false;
};
int g_resident_wake[2] = {-1, -1};
void resident_signal(int sig) {
  const char byte = sig == SIGCHLD ? 'c' : 't';
  if (g_resident_wake[1] >= 0) { ssize_t n = write(g_resident_wake[1], &byte, 1); (void)n; }
}
// The log goes to log.1 when it passes 4 MiB, and at a resident's start, so
// the last one's lines are there to read after it is gone (#13310).
void resident_log_open(Resident &r, bool keep_last) {
  const std::string path = r.dir + "/log";
  struct stat st;
  if (keep_last && stat(path.c_str(), &st) == 0 && st.st_size > 0) rename(path.c_str(), (path + ".1").c_str());
  if (r.log >= 0) close(r.log);
  r.log = open(path.c_str(), O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
}
void resident_log(Resident &r, const char *format, ...) {
  if (r.log < 0) return;
  struct stat st;
  if (++r.log_lines % 256 == 0 && fstat(r.log, &st) == 0 && st.st_size > (4 << 20)) resident_log_open(r, true);
  char line[1024];
  time_t now = time(nullptr);
  struct tm tm;
  localtime_r(&now, &tm);
  size_t n = strftime(line, sizeof line, "%Y-%m-%d %H:%M:%S ", &tm);
  va_list args;
  va_start(args, format);
  vsnprintf(line + n, sizeof line - n - 1, format, args);
  va_end(args);
  n = strlen(line);
  line[n++] = '\n';
  ssize_t w = write(r.log, line, n);
  (void)w;
}
std::string stat_identity(const std::string &path) {
  struct stat st;
  if (stat(path.c_str(), &st) != 0 || !S_ISREG(st.st_mode)) return "";
#ifdef __APPLE__
  const long long ns = (long long)st.st_mtimespec.tv_sec * 1000000000 + st.st_mtimespec.tv_nsec;
#else
  const long long ns = (long long)st.st_mtim.tv_sec * 1000000000 + st.st_mtim.tv_nsec;
#endif
  return std::to_string((long long)st.st_dev) + ":" + std::to_string((long long)st.st_ino) + ":" + std::to_string((long long)st.st_size) + ":" + std::to_string(ns);
}
// The client runs this resident's binary: the file at its path has the same bytes.
bool same_binary(Resident &r, const std::string &path) {
  const std::string identity = stat_identity(path);
  if (identity.empty()) return false;
  auto known = r.verified.find(path);
  if (known != r.verified.end() && known->second == identity) return true;
  if (read_file_bytes(path) != r.image || stat_identity(path) != identity) return false;
  r.verified[path] = identity;
  return true;
}
void resident_evict(Resident &r) {
  ServeState &s = serve_state();
  if (s.bytes <= r.limit) return;
  struct Old { uint64_t used, hash; const UnitLex *unit; };
  std::vector<Old> all;
  for (const auto &[hash, bucket] : s.units) for (const auto &e : bucket) all.push_back({e.used, hash, e.unit.get()});
  std::sort(all.begin(), all.end(), [](const Old &a, const Old &b) { return a.used < b.used; });
  const uint64_t target = r.limit - r.limit / 5;
  for (const Old &old : all) {
    if (s.bytes <= target) break;
    auto &bucket = s.units[old.hash];
    for (size_t i = 0; i < bucket.size(); i++) if (bucket[i].unit.get() == old.unit) {
      s.bytes -= unit_bytes(*old.unit);
      bucket.erase(bucket.begin() + (std::ptrdiff_t)i);
      r.evicted++;
      break;
    }
    if (bucket.empty()) s.units.erase(old.hash);
  }
}
void resident_keep(Resident &r, const std::string &payload) {
  ServeState &s = serve_state();
  Reader in{payload.data(), payload.data() + payload.size()};
  auto unit = std::make_shared<UnitLex>();
  unit->syntax = in.u32() != 0;
  unit->text = in.str();
  const uint64_t count = in.u64();
  if (!in.ok || count > (uint64_t)(in.end - in.at) / sizeof(UnitToken)) return;
  unit->tokens.resize((size_t)count);
  in.take(unit->tokens.data(), (size_t)count * sizeof(UnitToken));
  unit->decoded = in.str();
  unit->text_sha = in.str();
  unit->token_sum = in.str();
  if (!in.ok || in.at != in.end || unit->tokens.empty()) return;
  // The record indexes its own text and decoded bytes: refuse one that does not.
  for (const UnitToken &t : unit->tokens) {
    if (t.text < 0 || t.count < 0 || (uint64_t)t.text + (uint64_t)t.count > unit->text.size()) return;
    if (t.decoded >= 0 && (t.decoded_count < 0 || (uint64_t)t.decoded + (uint64_t)t.decoded_count >= unit->decoded.size())) return;
  }
  const uint64_t hash = serve_hash(unit->text.data(), unit->text.size());
  auto &bucket = s.units[hash];
  for (auto &e : bucket) if (e.unit->text == unit->text) {
    if (unit->syntax) e.unit->syntax = true;
    if (e.unit->text_sha.empty()) e.unit->text_sha = unit->text_sha;
    if (e.unit->token_sum.empty()) e.unit->token_sum = unit->token_sum;
    e.used = r.requests;
    return;
  }
  s.bytes += unit_bytes(*unit);
  bucket.push_back({unit, r.requests});
  resident_evict(r);
}
void resident_report(Resident &r, Resident::Live &live, uint32_t kind, const std::string &payload) {
  ServeState &s = serve_state();
  switch (kind) {
    case REPORT_RUNNING: live.running = true; break;
    case REPORT_REPLIED:
      live.replied = true;
      if (live.client >= 0) { close(live.client); live.client = -1; }
      break;
    case REPORT_UNIT: resident_keep(r, payload); break;
    case REPORT_COMPILER: if (payload.size() == 64 && s.compiler.empty()) s.compiler = payload; break;
    case REPORT_ANSWERED: r.answered++; break;
    case REPORT_PASSED: {
      std::string entry;
      const auto passed = passed_parse(payload, &entry);
      if (!passed || entry.empty()) break;
      ServeState::Passed &kept = s.passed[entry];
      kept.used = r.requests;
      // One of the same options replaces the older one of them.
      for (size_t i = 0; i < kept.last.size(); i++) if (passed_same_options(*kept.last[i], *passed)) { kept.last.erase(kept.last.begin() + (std::ptrdiff_t)i); break; }
      kept.last.insert(kept.last.begin(), passed);
      if (kept.last.size() > PASSED_EACH) kept.last.resize(PASSED_EACH);
      while (s.passed.size() > PASSED_ENTRIES) {
        auto oldest = s.passed.begin();
        for (auto at = s.passed.begin(); at != s.passed.end(); ++at) if (at->second.used < oldest->second.used) oldest = at;
        s.passed.erase(oldest);
      }
      break;
    }
    case REPORT_USED: {
      Reader in{payload.data(), payload.data() + payload.size()};
      const uint64_t hits = in.u64(), misses = in.u64();
      if (!in.ok) break;
      r.hits += hits;
      r.misses += misses;
      while (in.ok && in.at != in.end) {
        const uint64_t hash = in.u64();
        auto found = s.units.find(hash);
        if (in.ok && found != s.units.end()) for (auto &e : found->second) e.used = r.requests;
      }
      resident_log(r, "pid %lld: %llu unit lexes reused, %llu made; memo %llu MiB", (long long)live.pid, (unsigned long long)hits,
                   (unsigned long long)misses, (unsigned long long)(s.bytes >> 20));
      break;
    }
    case REPORT_END: live.ended = true; break;
  }
}
// Reads what a request has sent. False once its report stream is finished.
bool resident_read(Resident &r, Resident::Live &live) {
  char buffer[1 << 16];
  for (;;) {
    ssize_t n = recv(live.report, buffer, sizeof buffer, MSG_DONTWAIT);
    if (n < 0 && errno == EINTR) continue;
    if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
    if (n <= 0) { live.ended = true; break; }
    live.in.append(buffer, (size_t)n);
    if ((size_t)n < sizeof buffer) break;
  }
  size_t at = 0;
  while (live.in.size() - at >= 16) {
    uint32_t magic, kind;
    uint64_t length;
    memcpy(&magic, live.in.data() + at, 4);
    memcpy(&kind, live.in.data() + at + 4, 4);
    memcpy(&length, live.in.data() + at + 8, 8);
    if (magic != SERVE_MAGIC || length > (1ull << 34)) { live.ended = true; break; }
    if (live.in.size() - at - 16 < length) break;
    resident_report(r, live, kind, live.in.substr(at + 16, (size_t)length));
    at += 16 + (size_t)length;
  }
  live.in.erase(0, at);
  if (live.ended && live.report >= 0) { close(live.report); live.report = -1; }
  return !live.ended;
}
// Receives the first 16 bytes of a client's message and the descriptors sent with them.
bool receive_head(int fd, uint32_t *kind, uint64_t *length, int streams[3]) {
  char head[16];
  char control[CMSG_SPACE(3 * sizeof(int))];
  iovec io{head, sizeof head};
  msghdr msg{};
  msg.msg_iov = &io;
  msg.msg_iovlen = 1;
  msg.msg_control = control;
  msg.msg_controllen = sizeof control;
  ssize_t n;
  do n = recvmsg(fd, &msg, MSG_WAITALL); while (n < 0 && errno == EINTR);
  streams[0] = streams[1] = streams[2] = -1;
  for (cmsghdr *c = CMSG_FIRSTHDR(&msg); c; c = CMSG_NXTHDR(&msg, c))
    if (c->cmsg_level == SOL_SOCKET && c->cmsg_type == SCM_RIGHTS && c->cmsg_len == CMSG_LEN(3 * sizeof(int))) memcpy(streams, CMSG_DATA(c), 3 * sizeof(int));
  uint32_t magic = 0;
  if (n == (ssize_t)sizeof head) { memcpy(&magic, head, 4); memcpy(kind, head + 4, 4); memcpy(length, head + 8, 8); }
  return n == (ssize_t)sizeof head && magic == SERVE_MAGIC && *length < (64u << 20);
}
void close_streams(int streams[3]) { for (int i = 0; i < 3; i++) if (streams[i] >= 0) close(streams[i]); }
std::string resident_status(const Resident &r) {
  const ServeState &s = serve_state();
  size_t units = 0;
  for (const auto &[hash, bucket] : s.units) units += bucket.size();
  size_t passed = 0;
  for (const auto &[entry, kept] : s.passed) passed += kept.last.size();
  char text[1024];
  snprintf(text, sizeof text,
           "coax serve: pid %lld, up %lld s, %llu requests (%zu running), %zu waiting, at most %llu at once (COAX_SERVE_JOBS), "
           "each at most %llu s (COAX_SERVE_TIMEOUT; %llu stopped there), %llu queued in all, %llu stopped after their reply, %s"
           "%zu unit lexes kept in %llu MiB (limit %llu MiB, %llu evicted), "
           "%llu reused and %llu made, %zu checked programs remembered, %llu errors from changed bodies alone, idle exit after %llu s\n  %s\n",
           (long long)getpid(), (long long)std::chrono::duration_cast<std::chrono::seconds>(std::chrono::steady_clock::now() - r.began).count(),
           (unsigned long long)r.requests, r.live.size(), r.waiting.size(), (unsigned long long)r.cap, (unsigned long long)r.timeout,
           (unsigned long long)r.timed_out, (unsigned long long)r.queued_total, (unsigned long long)r.lingered,
           r.draining ? "draining (its binary is gone: no new requests), " : "", units, (unsigned long long)(s.bytes >> 20), (unsigned long long)(r.limit >> 20),
           (unsigned long long)r.evicted, (unsigned long long)r.hits, (unsigned long long)r.misses, passed, (unsigned long long)r.answered,
           (unsigned long long)r.idle, r.dir.c_str());
  return text;
}
// Forks the request's child. The client's socket and streams go to it; the
// resident keeps the client's socket to see it leave, and the report pair's end.
void resident_start(Resident &r, int fd, int streams[3], const RunRequest &run, const std::string &what) {
  auto refuse = [&](const char *why) {
    const char reply = 'R';
    write_all(fd, &reply, 1);
    close(fd);
    close_streams(streams);
    resident_log(r, "refused: %s", why);
  };
  int report[2];
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, report) != 0) return refuse("no socket pair");
  no_sigpipe(report[0]);
  no_sigpipe(report[1]);
  // Room for a request's whole report (a checked program's skeleton is tens of
  // MB): macOS gives a socket pair 8 KB, and a child then waited on the
  // resident for every 8 KB of it while the resident served everyone else (#13323).
  for (int end : report) {
    int size = 4 << 20;
    setsockopt(end, SOL_SOCKET, SO_SNDBUF, &size, sizeof size);
    setsockopt(end, SOL_SOCKET, SO_RCVBUF, &size, sizeof size);
  }
  r.requests++;
  const pid_t resident = getpid();
  const pid_t pid = fork();
  if (pid < 0) { close(report[0]); close(report[1]); return refuse("fork failed"); }
  if (pid == 0) {
    close(report[0]);
    close(r.listener);
    close(r.lock);
    if (r.log >= 0) close(r.log);
    close(g_resident_wake[0]);
    close(g_resident_wake[1]);
    for (const auto &other : r.live) { if (other.client >= 0) close(other.client); if (other.report >= 0) close(other.report); }
    for (const auto &other : r.waiting) { close(other.client); for (int f : other.streams) if (f >= 0) close(f); }
    // The programs the command starts (cc, a check's binary) never hold the report's end.
    fcntl(report[1], F_SETFD, FD_CLOEXEC);
    timeval none{0, 0};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &none, sizeof none);
    request_run(run, streams, fd, report[1], resident);
  }
  close(report[1]);
  close_streams(streams);
  fcntl(report[0], F_SETFD, FD_CLOEXEC);
  Resident::Live live;
  live.pid = pid;
  live.client = fd;
  live.report = report[0];
  live.began = std::chrono::steady_clock::now();
  live.what = what;
  resident_log(r, "pid %lld: %s (in %s)", (long long)pid, live.what.c_str(), run.directory.c_str());
  r.live.push_back(std::move(live));
}
void resident_accept(Resident &r) {
  int fd = accept(r.listener, nullptr, nullptr);
  if (fd < 0) return;
  fcntl(fd, F_SETFD, FD_CLOEXEC);
  no_sigpipe(fd);
  timeval wait{2, 0};
  setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &wait, sizeof wait);
  uint32_t kind = 0;
  uint64_t length = 0;
  int streams[3];
  std::string payload;
  bool ok = receive_head(fd, &kind, &length, streams);
  if (ok) { payload.resize((size_t)length); ok = read_all(fd, payload.data(), payload.size()); }
  auto refuse = [&](const char *why) {
    const char reply = 'R';
    write_all(fd, &reply, 1);
    close(fd);
    close_streams(streams);
    if (why) resident_log(r, "refused: %s", why);
  };
  if (!ok) return refuse("unreadable request");
  if (kind == SERVE_STATUS || kind == SERVE_STOP) {
    const std::string text = resident_status(r);
    write_all(fd, text.data(), text.size());
    close(fd);
    close_streams(streams);
    if (kind == SERVE_STOP) r.stop = true;
    return;
  }
  RunRequest run;
  if (kind != SERVE_RUN || !parse_run(payload, &run) || streams[0] < 0) return refuse("malformed request");
  if (!same_binary(r, run.executable)) return refuse("the client is another compiler binary");
  if (r.draining) return refuse("its binary is gone; a new resident takes new requests");
  std::string what;
  for (size_t i = 1; i < run.args.size() && what.size() < 160; i++) what += (i > 1 ? " " : "") + run.args[i];
  if (r.live.size() >= r.cap) {
    // At most 16 waiting for each running slot: past that the client compiles cold.
    if (r.waiting.size() >= 16 * r.cap) return refuse("too many requests waiting");
    const char queued = 'Q';
    if (!write_all(fd, &queued, 1)) { close(fd); close_streams(streams); return; }
    Resident::Waiting w;
    w.client = fd;
    for (int i = 0; i < 3; i++) w.streams[i] = streams[i];
    w.run = std::move(run);
    w.what = what;
    r.waiting.push_back(std::move(w));
    r.queued_total++;
    resident_log(r, "waiting (%zu running, %zu waiting): %s", r.live.size(), r.waiting.size(), what.c_str());
    return;
  }
  resident_start(r, fd, streams, run, what);
}
void resident_reap(Resident &r) {
  for (;;) {
    int status = 0;
    const pid_t pid = waitpid(-1, &status, WNOHANG);
    if (pid <= 0) break;
    for (size_t i = 0; i < r.live.size(); i++) {
      Resident::Live &live = r.live[i];
      if (live.pid != pid) continue;
      // What it sent before it ended. A bounded drain: a child that died
      // without its end report while another process still held the report's
      // end left this loop spinning, and the resident served no one (#13323).
      for (int round = 0; live.report >= 0 && round < 64 && resident_read(r, live); round++) {}
      if (live.report >= 0) { close(live.report); live.report = -1; }
      if (live.client >= 0) {
        // It left without replying: exit() inside the command, or a crash.
        if (!live.running) { const char reply = 'R'; write_all(live.client, &reply, 1); }
        else if (WIFEXITED(status)) { const char reply[5] = {'X', (char)WEXITSTATUS(status), 0, 0, 0}; write_all(live.client, reply, sizeof reply); }
        else { const char reply[5] = {'S', (char)(WIFSIGNALED(status) ? WTERMSIG(status) : 0), 0, 0, 0}; write_all(live.client, reply, sizeof reply); }
        close(live.client);
      }
      resident_log(r, "pid %lld: %s %d after %.2f s", (long long)pid, WIFEXITED(status) ? "exit" : "signal",
                   WIFEXITED(status) ? WEXITSTATUS(status) : WIFSIGNALED(status) ? WTERMSIG(status) : -1,
                   std::chrono::duration<double>(std::chrono::steady_clock::now() - live.began).count());
      r.live.erase(r.live.begin() + (std::ptrdiff_t)i);
      break;
    }
  }
}
int resident_main(const std::string &dir) {
  Resident r;
  r.dir = dir;
  r.image = read_file_bytes(executable_path());
  r.limit = env_count("COAX_SERVE_MEMORY_MB", 4096) << 20;
  r.idle = env_count("COAX_SERVE_IDLE", 900);
  r.cap = std::max<uint64_t>(1, env_count("COAX_SERVE_JOBS", available_cores()));
  r.timeout = env_count("COAX_SERVE_TIMEOUT", 3600);
  std::error_code ec;
  std::filesystem::create_directories(dir, ec);
  chmod(dir.c_str(), 0700);
  sockaddr_un address;
  if (r.image.empty() || serve_dir(r.image) != dir || !socket_address(dir, &address)) {
    fprintf(stderr, "coax serve: '%s' is not this compiler's resident directory\n", dir.c_str());
    return 1;
  }
  r.lock = open((dir + "/lock").c_str(), O_RDWR | O_CREAT | O_CLOEXEC, 0600);
  if (r.lock < 0 || flock(r.lock, LOCK_EX | LOCK_NB) != 0) return 0;  // another resident serves this binary
  resident_log_open(r, true);
  unlink(address.sun_path);
  r.listener = socket(AF_UNIX, SOCK_STREAM, 0);
  if (r.listener < 0) return 1;
  fcntl(r.listener, F_SETFD, FD_CLOEXEC);
  const mode_t old = umask(0177);
  const bool bound = bind(r.listener, (sockaddr *)&address, sizeof address) == 0 && listen(r.listener, 64) == 0;
  umask(old);
  if (!bound) { resident_log(r, "cannot listen at %s: %s", address.sun_path, strerror(errno)); return 1; }
  if (pipe(g_resident_wake) != 0) return 1;
  for (int fd : g_resident_wake) { fcntl(fd, F_SETFD, FD_CLOEXEC); fcntl(fd, F_SETFL, O_NONBLOCK); }
  signal(SIGPIPE, SIG_IGN);
  signal(SIGHUP, SIG_IGN);
  signal(SIGCHLD, resident_signal);
  signal(SIGTERM, resident_signal);
  signal(SIGINT, resident_signal);
  { std::string pid = std::to_string((long long)getpid()) + "\n"; write_file(dir + "/pid", pid); }
  resident_log(r, "resident %lld of %s: memory limit %llu MiB, idle exit after %llu s, at most %llu requests at once, each at most %llu s",
               (long long)getpid(), executable_path().c_str(), (unsigned long long)(r.limit >> 20), (unsigned long long)r.idle,
               (unsigned long long)r.cap, (unsigned long long)r.timeout);
  const std::string own = executable_path();
  while (!r.stop) {
    std::vector<pollfd> fds;
    fds.push_back({r.listener, (short)(r.listener >= 0 ? POLLIN : 0), 0});
    fds.push_back({g_resident_wake[0], POLLIN, 0});
    const size_t first = fds.size();
    for (const auto &live : r.live) {
      fds.push_back({live.report, (short)(live.report >= 0 ? POLLIN : 0), 0});
      fds.push_back({live.client, (short)(live.client >= 0 ? POLLIN : 0), 0});
    }
    const size_t waiters = fds.size();
    for (const auto &w : r.waiting) fds.push_back({w.client, POLLIN, 0});
    const int ready = poll(fds.data(), fds.size(), 1000);
    if (ready < 0 && errno != EINTR) break;
    if (ready > 0 && (fds[1].revents & POLLIN)) {
      char bytes[64];
      ssize_t n;
      while ((n = read(g_resident_wake[0], bytes, sizeof bytes)) > 0)
        for (ssize_t i = 0; i < n; i++) if (bytes[i] == 't') r.stop = true;
    }
    // Reports before hang-ups: a request says it replied before its client can leave.
    for (size_t i = 0; ready > 0 && i < r.live.size(); i++)
      if (r.live[i].report >= 0 && (fds[first + 2 * i].revents & (POLLIN | POLLHUP | POLLERR))) resident_read(r, r.live[i]);
    const auto now = std::chrono::steady_clock::now();
    for (size_t i = 0; i < r.live.size(); i++) {
      Resident::Live &live = r.live[i];
      if (live.replied && live.replied_at == std::chrono::steady_clock::time_point()) live.replied_at = now;
      if (live.killed) continue;
      const double age = std::chrono::duration<double>(now - live.began).count();
      // Its time is up: the client exits 124 with a note, and the command stops.
      if (!live.replied && r.timeout > 0 && age >= (double)r.timeout) {
        resident_log(r, "pid %lld: still running after %llu s (COAX_SERVE_TIMEOUT); stopping it", (long long)live.pid, (unsigned long long)r.timeout);
        if (live.client >= 0) {
          char reply[5] = {'T', 0, 0, 0, 0};
          const uint32_t limit = (uint32_t)r.timeout;
          memcpy(reply + 1, &limit, 4);
          write_all(live.client, reply, sizeof reply);
          close(live.client);
          live.client = -1;
        }
        r.timed_out++;
        live.killed = true;
        kill(-live.pid, SIGKILL);
        kill(live.pid, SIGKILL);
        continue;
      }
      // Replied 30 s ago and still sending its report: the memo can do without it.
      if (live.replied && std::chrono::duration<double>(now - live.replied_at).count() >= 30) {
        resident_log(r, "pid %lld: replied 30 s ago and has not ended; stopping it", (long long)live.pid);
        r.lingered++;
        live.killed = true;
        kill(-live.pid, SIGKILL);
        kill(live.pid, SIGKILL);
        continue;
      }
      if (ready <= 0 || live.client < 0 || !(fds[first + 2 * i + 1].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (live.report >= 0) resident_read(r, live);
      if (live.client < 0) continue;
      // Its end of file means the client is gone, and its command stops with
      // it. A byte is the client's go-ahead, which the request is about to read.
      char byte;
      const ssize_t peeked = recv(live.client, &byte, 1, MSG_PEEK | MSG_DONTWAIT);
      if (peeked > 0) { usleep(200); continue; }
      if (peeked < 0 && (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR)) continue;
      resident_log(r, "pid %lld: its client left; stopping it", (long long)live.pid);
      close(live.client);
      live.client = -1;
      live.killed = true;
      kill(-live.pid, SIGKILL);
      kill(live.pid, SIGKILL);
    }
    // A waiting client that left (it sends nothing until its request starts).
    for (size_t i = r.waiting.size(); ready > 0 && i-- > 0;) {
      if (!(fds[waiters + i].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      Resident::Waiting &w = r.waiting[i];
      resident_log(r, "a waiting client left: %s", w.what.c_str());
      close(w.client);
      close_streams(w.streams);
      r.waiting.erase(r.waiting.begin() + (std::ptrdiff_t)i);
    }
    resident_reap(r);
    // The waiting ones, in the order they came, as running ones end.
    while (!r.waiting.empty() && r.live.size() < r.cap) {
      Resident::Waiting w = std::move(r.waiting.front());
      r.waiting.erase(r.waiting.begin());
      resident_start(r, w.client, w.streams, w.run, w.what);
    }
    if (ready > 0 && r.listener >= 0 && (fds[0].revents & POLLIN)) resident_accept(r);
    if (!r.live.empty() || !r.waiting.empty()) r.active = now;
    else if (r.draining || (uint64_t)std::chrono::duration_cast<std::chrono::seconds>(now - r.active).count() >= r.idle) break;
    // A binary that is gone or replaced (a temporary build, a new install, a
    // removed worktree) has no more clients here: take no new request, let a
    // resident of a live binary start in this directory, and leave once the
    // running and waiting ones end. Checked every 20 s (or every COAX_SERVE_IDLE
    // seconds, when that is less), busy or not (#13310).
    if (!r.draining && (uint64_t)std::chrono::duration_cast<std::chrono::seconds>(now - r.binary_checked).count() >= std::min<uint64_t>(20, std::max<uint64_t>(1, r.idle))) {
      r.binary_checked = now;
      if (!same_binary(r, own)) {
        if (r.live.empty() && r.waiting.empty()) break;
        resident_log(r, "its binary is gone or replaced: no new requests; leaving after %zu running and %zu waiting", r.live.size(), r.waiting.size());
        r.draining = true;
        close(r.listener);
        r.listener = -1;
        unlink(address.sun_path);
        unlink((dir + "/pid").c_str());
        close(r.lock);
        r.lock = -1;
      }
    }
  }
  for (const auto &live : r.live) { kill(-live.pid, SIGKILL); kill(live.pid, SIGKILL); }
  for (auto &w : r.waiting) { const char reply = 'R'; write_all(w.client, &reply, 1); close(w.client); close_streams(w.streams); }
  if (!r.draining) {
    unlink(address.sun_path);
    unlink((dir + "/pid").c_str());
  }
  resident_log(r, "resident %lld leaves after %llu requests", (long long)getpid(), (unsigned long long)r.requests);
  return 0;
}

// ---- The client ----

int serve_connect(const std::string &dir) {
  sockaddr_un address;
  if (!socket_address(dir, &address)) return -1;
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0) return -1;
  fcntl(fd, F_SETFD, FD_CLOEXEC);
  no_sigpipe(fd);
  if (connect(fd, (sockaddr *)&address, sizeof address) != 0) { close(fd); return -1; }
  return fd;
}
// Starts this binary's resident, detached, and does not wait for it.
void serve_start(const std::string &exe, const std::string &dir) {
  fflush(stdout);
  fflush(stderr);
  const pid_t pid = fork();
  if (pid < 0) return;
  if (pid == 0) {
    setsid();
    if (fork() != 0) _exit(0);
    int null = open("/dev/null", O_RDWR);
    if (null >= 0) { dup2(null, 0); dup2(null, 1); dup2(null, 2); }
    for (int fd = 3; fd < 256; fd++) close(fd);
    execl(exe.c_str(), exe.c_str(), "serve", "--resident", dir.c_str(), (char *)nullptr);
    _exit(127);
  }
  int status;
  while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
}
bool send_head(int fd, uint32_t kind, uint64_t length, bool streams) {
  std::string head;
  put_u32(head, SERVE_MAGIC);
  put_u32(head, kind);
  put_u64(head, length);
  char control[CMSG_SPACE(3 * sizeof(int))];
  memset(control, 0, sizeof control);
  iovec io{head.data(), head.size()};
  msghdr msg{};
  msg.msg_iov = &io;
  msg.msg_iovlen = 1;
  if (streams) {
    msg.msg_control = control;
    msg.msg_controllen = sizeof control;
    cmsghdr *c = CMSG_FIRSTHDR(&msg);
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    c->cmsg_len = CMSG_LEN(3 * sizeof(int));
    const int fds[3] = {0, 1, 2};
    memcpy(CMSG_DATA(c), fds, sizeof fds);
  }
  ssize_t n;
#ifdef MSG_NOSIGNAL
  do n = sendmsg(fd, &msg, MSG_NOSIGNAL); while (n < 0 && errno == EINTR);
#else
  do n = sendmsg(fd, &msg, 0); while (n < 0 && errno == EINTR);
#endif
  return n == (ssize_t)head.size();
}
bool serve_command(const std::string &cmd) {
  for (const char *name : {"c", "check", "test", "lib", "header", "build", "deps", "layout", "checks-c"}) if (cmd == name) return true;
  return false;
}
// Runs the command in this binary's resident. False when it must run here
// instead: there is no resident (one is started for the next command), it
// refused, or the request died before it replied.
bool serve_client(int argc, char **argv, int *status) {
  if (argc < 2 || !serve_command(argv[1]) || serve_off() || serve_state().request) return false;
  for (int fd = 0; fd < 3; fd++) if (fcntl(fd, F_GETFD) < 0) return false;
  const std::string exe = executable_path();
  const std::string dir = serve_dir(read_file_bytes(exe));
  if (dir.empty()) return false;
  int fd = serve_connect(dir);
  if (fd < 0) { serve_start(exe, dir); return false; }
  std::string payload;
  put_str(payload, exe);
  std::error_code ec;
  const std::string cwd = std::filesystem::current_path(ec).string();
  if (ec) { close(fd); return false; }
  put_str(payload, cwd);
  const mode_t mask = umask(0);
  umask(mask);
  put_u32(payload, (uint32_t)mask);
  errno = 0;
  const int priority = getpriority(PRIO_PROCESS, 0);
  put_u32(payload, (uint32_t)(errno == 0 ? priority : 0));
  put_u32(payload, (uint32_t)argc);
  for (int i = 0; i < argc; i++) put_str(payload, argv[i]);
  uint32_t envc = 0;
  for (char **e = environ; e && *e; e++) envc++;
  put_u32(payload, envc);
  for (char **e = environ; e && *e; e++) put_str(payload, *e);
  // Where standard output stands, so a request that dies midway leaves no part of its output in a file.
  struct stat out_stat;
  const bool out_file = fstat(1, &out_stat) == 0 && S_ISREG(out_stat.st_mode);
  const off_t out_at = out_file ? lseek(1, 0, SEEK_CUR) : -1;
  fflush(stdout);
  fflush(stderr);
  timeval wait{5, 0};
  setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &wait, sizeof wait);
  char reply = 0;
  bool sent = send_head(fd, SERVE_RUN, payload.size(), true) && write_all(fd, payload.data(), payload.size()) && read_all(fd, &reply, 1);
  if (sent && reply == 'Q') {
    // Every running slot is taken: wait for one (COAX_SERVE_WAIT seconds,
    // default 900), else compile here.
    timeval queued{(time_t)env_count("COAX_SERVE_WAIT", 900), 0};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &queued, sizeof queued);
    sent = read_all(fd, &reply, 1);
  }
  if (!sent || reply != 'A') {
    close(fd);
    return false;  // nothing ran
  }
  timeval none{0, 0};
  setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &none, sizeof none);
  const char go = 'G';
  char done[5] = {0, 0, 0, 0, 0};
  const bool answered = write_all(fd, &go, 1) && read_all(fd, done, sizeof done);
  close(fd);
  if (answered && done[0] == 'X') { *status = (unsigned char)done[1]; return true; }
  if (answered && done[0] == 'T') {
    uint32_t limit = 0;
    memcpy(&limit, done + 1, 4);
    fprintf(stderr, "coax: stopped after %u s, the resident compiler's limit for one command (COAX_SERVE_TIMEOUT)\n", limit);
    *status = 124;
    return true;
  }
  // A signal this process would have taken itself (its output's reader left, an interrupt): leave by it.
  const int sig = answered && done[0] == 'S' ? (unsigned char)done[1] : 0;
  if (sig == SIGPIPE || sig == SIGINT || sig == SIGTERM || sig == SIGHUP) { signal(sig, SIG_DFL); raise(sig); _exit(128 + sig); }
  // The request died (a signal, or its resident and it both went away). Run the command here, from where its output began.
  if (out_file && out_at >= 0) { if (ftruncate(1, out_at) == 0) lseek(1, out_at, SEEK_SET); }
  fprintf(stderr, "coax: the resident compiler's request ended without a result; running the command again without it\n");
  return false;
}
int serve_ask(uint32_t kind) {
  const std::string dir = serve_dir(read_file_bytes(executable_path()));
  int fd = dir.empty() ? -1 : serve_connect(dir);
  if (fd < 0) { puts("coax serve: no resident is running for this compiler"); return kind == SERVE_STATUS ? 1 : 0; }
  if (!send_head(fd, kind, 0, false)) { close(fd); return 1; }
  char buffer[1024];
  ssize_t n;
  while ((n = read(fd, buffer, sizeof buffer)) > 0) fwrite(buffer, 1, (size_t)n, stdout);
  close(fd);
  if (kind == SERVE_STOP) puts("coax serve: stopped");
  return 0;
}

}  // namespace

std::shared_ptr<const UnitLex> serve_find_unit(const std::string &text) {
  ServeState &s = serve_state();
  if (!s.request) return nullptr;
  const uint64_t hash = serve_hash(text.data(), text.size());
  auto found = s.units.find(hash);
  if (found != s.units.end())
    for (const auto &e : found->second) if (e.unit->text == text) {
      s.hits++;
      s.used.push_back(hash);
      return e.unit;
    }
  s.misses++;
  return nullptr;
}
std::vector<std::shared_ptr<const PassedSkeleton>> serve_passed(const std::string &entry) {
  ServeState &s = serve_state();
  if (!s.request) return {};
  const auto found = s.passed.find(entry);
  return found == s.passed.end() ? std::vector<std::shared_ptr<const PassedSkeleton>>() : found->second.last;
}
void serve_keep_passed(const std::string &entry, const std::shared_ptr<const PassedSkeleton> &passed) {
  ServeState &s = serve_state();
  if (s.request && passed) s.passed_fresh.push_back({entry, passed});
}
bool serve_request() { return serve_state().request; }
void serve_answered() { serve_state().answered = true; }
void serve_detach() {
  ServeState &s = serve_state();
  s.request = false;
  s.report = -1;
  s.client = -1;
}
void serve_keep_unit(const std::shared_ptr<const UnitLex> &unit) {
  ServeState &s = serve_state();
  if (!s.request || !unit || unit->queued) return;
  unit->queued = true;
  s.fresh.push_back(unit);
}
// A request's view of the binary: the client's path to it, and its SHA-256 when a request already computed it.
static const std::string &serve_executable() { return serve_state().executable; }
static const std::string &serve_compiler() { return serve_state().compiler; }
static void serve_compiler_known(const std::string &sha) {
  ServeState &s = serve_state();
  if (s.request && s.compiler.empty() && sha.size() == 64) { s.compiler = sha; s.compiler_fresh = true; }
}

// `coax serve [status|stop]`, the resident itself (`coax serve --resident DIR`,
// started by a compile command), or a compile command a resident runs. False:
// the command runs in this process.
static bool serve_main(int argc, char **argv, int *status) {
  if (argc >= 2 && !strcmp(argv[1], "serve")) {
    const std::string what = argc >= 3 ? argv[2] : "status";
    if (serve_off()) { fprintf(stderr, "coax serve: off (COAX_SERVE=0, or the build-only stage0)\n"); *status = 1; }
    else if (what == "--resident" && argc == 4) *status = resident_main(argv[3]);
    else if (what == "status" && argc <= 3) *status = serve_ask(SERVE_STATUS);
    else if (what == "stop" && argc == 3) *status = serve_ask(SERVE_STOP);
    else if (what == "start" && argc == 3) {
      const std::string exe = executable_path(), dir = serve_dir(read_file_bytes(exe));
      if (dir.empty()) { fprintf(stderr, "coax serve: nowhere to keep a resident (set HOME or COAX_SERVE_DIR)\n"); *status = 1; }
      else { serve_start(exe, dir); printf("coax serve: started in %s\n", dir.c_str()); *status = 0; }
    }
    else { fprintf(stderr, "usage: coax serve [status|start|stop]\n"); *status = 2; }
    return true;
  }
  return serve_client(argc, argv, status);
}

}  // namespace coax
