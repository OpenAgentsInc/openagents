/* Crash capture for the room server (#13298, docs/deployment/crash-reports.md). Platform effects only:
   the signal handlers, the dump they leave and addr2line's run; room-crash.coax turns the dump into the
   report and room-host.c posts it.

   crash_install: handlers for SIGSEGV, SIGBUS, SIGFPE, SIGILL, SIGABRT, SIGTRAP and SIGSYS on a stack
   of their own. A handler writes STATE/crash.txt with async-signal-safe calls only (open, write,
   close; backtrace was called once at the start, so its unwinder is already loaded), prints the frames
   to stderr, and raises the signal again with its default action, so the process still dies of it
   and systemd starts the next one.

   The dump (room-crash.coax reads it):
     dump 1
     t MS                  wall clock, Unix ms
     signal N
     addr HEX              the address the signal names (si_addr)
     build HEX             this binary's build ID (its ELF note): the name its symbols are kept under
     exe PATH              this binary's file
     release NAME          the folder the binary is in (vm-deploy.sh: /opt/oa-room/releases/NAME/room)
     shard N
     err TEXT              the last line written to stderr (a Coax trap prints PATH:LINE: WHAT, then aborts)
     a HEX                 a frame, innermost first: its offset in the binary (a return address less one,
                           so it names the call's line), or "a -" for a frame outside the binary
     tick MS US PLAYERS    the last 16 ticks, oldest first

   crash_pending: at the next start, the dump the last process left (moved to crash.last.txt, so it is
   sent once) and addr2line's answer for each offset (addr2line -a -f -i: the offset's own "0x..." line,
   then a function and its FILE:LINE for the code there and for each function it was inlined into), from
   the binary the dump names.

   Without a state directory (Cloud Run, a local build) the handler only prints the frames. */
#define _GNU_SOURCE
#include "room-crash.h"
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#if defined(__linux__) && defined(__GLIBC__)
#include <execinfo.h>
#include <link.h>
#include <ucontext.h>

#define CRASH_FRAMES 40
#define CRASH_TICKS 16
static char crash_path[512], crash_last[512], crash_exe[512], crash_release[128], crash_build[65];
static int crash_shard;
static uintptr_t crash_base, crash_end;   /* this binary's addresses: [base, end) */
static char crash_err[1024];              /* stderr's buffer: its last line */
static struct { int64_t ms, us, players; } crash_ring[CRASH_TICKS];
static unsigned crash_ring_n;
static volatile sig_atomic_t crash_test_flag;
static char crash_stack[65536];

static int crash_phdr(struct dl_phdr_info *info, size_t size, void *data) {
  (void)size; (void)data;
  crash_base = (uintptr_t)info->dlpi_addr;
  for (int i = 0; i < info->dlpi_phnum; i++) {
    const ElfW(Phdr) *p = &info->dlpi_phdr[i];
    if (p->p_type == PT_LOAD && crash_base + p->p_vaddr + p->p_memsz > crash_end) crash_end = crash_base + p->p_vaddr + p->p_memsz;
    if (p->p_type != PT_NOTE) continue;
    const unsigned char *n = (const unsigned char *)(crash_base + p->p_vaddr), *end = n + p->p_memsz;
    while (n + sizeof(ElfW(Nhdr)) <= end) {
      const ElfW(Nhdr) *h = (const ElfW(Nhdr) *)n;
      const unsigned char *name = n + sizeof *h, *desc = name + ((h->n_namesz + 3u) & ~3u);
      if (desc + h->n_descsz > end) break;
      if (h->n_type == NT_GNU_BUILD_ID && h->n_namesz == 4 && !memcmp(name, "GNU", 4) && h->n_descsz <= 32)
        for (unsigned k = 0; k < h->n_descsz; k++) snprintf(crash_build + 2 * k, 3, "%02x", desc[k]);
      n = desc + ((h->n_descsz + 3u) & ~3u);
    }
  }
  return 1;   /* the first entry is the program itself */
}

/* async-signal-safe formatting into b */
static size_t cs_str(char *b, size_t n, size_t cap, const char *s) { while (*s && n + 1 < cap) b[n++] = *s++; return n; }
static size_t cs_num(char *b, size_t n, size_t cap, uint64_t v, unsigned base) {
  char d[24]; int k = 0;
  do { unsigned x = (unsigned)(v % base); d[k++] = (char)(x < 10 ? '0' + x : 'a' + x - 10); v /= base; } while (v && k < 24);
  while (k > 0 && n + 1 < cap) b[n++] = d[--k];
  return n;
}
static size_t cs_line(char *b, size_t n, size_t cap, const char *key, const char *value) {
  n = cs_str(b, n, cap, key); n = cs_str(b, n, cap, " "); n = cs_str(b, n, cap, value); return cs_str(b, n, cap, "\n");
}

static void crash_on(int sig, siginfo_t *si, void *ctx) {
  static volatile sig_atomic_t again;
  if (again) _exit(128 + sig);
  again = 1;
  static void *frames[CRASH_FRAMES]; static char b[8192];
  int count = backtrace(frames, CRASH_FRAMES);
  uintptr_t pc = 0;
#if defined(__x86_64__)
  pc = (uintptr_t)((ucontext_t *)ctx)->uc_mcontext.gregs[REG_RIP];
#elif defined(__aarch64__)
  pc = (uintptr_t)((ucontext_t *)ctx)->uc_mcontext.pc;
#else
  (void)ctx;
#endif
  /* past the handler and the kernel's trampoline: from the frame the signal interrupted */
  int first = count > 2 ? 2 : 0;
  for (int i = 0; i < count; i++) if (pc && (uintptr_t)frames[i] == pc) { first = i; break; }
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  size_t n = 0, cap = sizeof b;
  n = cs_str(b, n, cap, "dump 1\nt "); n = cs_num(b, n, cap, (uint64_t)ts.tv_sec * 1000u + (uint64_t)ts.tv_nsec / 1000000u, 10);
  n = cs_str(b, n, cap, "\nsignal "); n = cs_num(b, n, cap, (uint64_t)sig, 10);
  /* si_addr names an address only for a fault; for SIGABRT and SIGSYS the same bytes hold a pid or a call */
  bool fault = sig == SIGSEGV || sig == SIGBUS || sig == SIGFPE || sig == SIGILL || sig == SIGTRAP;
  n = cs_str(b, n, cap, "\naddr "); n = cs_num(b, n, cap, (uint64_t)(uintptr_t)(si && fault ? si->si_addr : 0), 16); n = cs_str(b, n, cap, "\n");
  if (crash_build[0]) n = cs_line(b, n, cap, "build", crash_build);
  if (crash_exe[0]) n = cs_line(b, n, cap, "exe", crash_exe);
  if (crash_release[0]) n = cs_line(b, n, cap, "release", crash_release);
  n = cs_str(b, n, cap, "shard "); n = cs_num(b, n, cap, (uint64_t)crash_shard, 10); n = cs_str(b, n, cap, "\n");
  size_t el = 0; while (el < sizeof crash_err && el < 200 && crash_err[el] >= 32 && crash_err[el] != 127) el++;
  if (el > 0 && n + el + 8 < cap) { n = cs_str(b, n, cap, "err "); memcpy(b + n, crash_err, el); n += el; n = cs_str(b, n, cap, "\n"); }
  for (int i = first; i < count; i++) {
    uintptr_t a = (uintptr_t)frames[i];
    if (a < crash_base || a >= crash_end) { n = cs_str(b, n, cap, "a -\n"); continue; }
    n = cs_str(b, n, cap, "a "); n = cs_num(b, n, cap, (uint64_t)(a - crash_base - (a == pc ? 0u : 1u)), 16); n = cs_str(b, n, cap, "\n");
  }
  unsigned have = crash_ring_n < CRASH_TICKS ? crash_ring_n : CRASH_TICKS;
  for (unsigned i = 0; i < have; i++) {
    unsigned k = (crash_ring_n - have + i) % CRASH_TICKS;
    n = cs_str(b, n, cap, "tick "); n = cs_num(b, n, cap, (uint64_t)crash_ring[k].ms, 10); n = cs_str(b, n, cap, " ");
    n = cs_num(b, n, cap, (uint64_t)crash_ring[k].us, 10); n = cs_str(b, n, cap, " "); n = cs_num(b, n, cap, (uint64_t)crash_ring[k].players, 10); n = cs_str(b, n, cap, "\n");
  }
  if (crash_path[0]) {
    int fd = open(crash_path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    if (fd >= 0) { size_t at = 0; while (at < n) { ssize_t w = write(fd, b + at, n - at); if (w <= 0) break; at += (size_t)w; } close(fd); }
  }
  static const char said[] = "room: crashed: the frames, innermost first\n";
  if (write(2, said, sizeof said - 1) < 0) {}
  backtrace_symbols_fd(frames + first, count - first, 2);
  signal(sig, SIG_DFL);
  raise(sig);   /* delivered when this handler returns: the process dies of the signal it took */
}
static void crash_usr2(int sig) { (void)sig; crash_test_flag = 1; }

void crash_install(int shard) {
  crash_shard = shard;
  dl_iterate_phdr(crash_phdr, NULL);
  ssize_t k = readlink("/proc/self/exe", crash_exe, sizeof crash_exe - 1);
  crash_exe[k > 0 ? k : 0] = 0;
  /* /opt/oa-room/releases/NAME/room: NAME */
  char *slash = strrchr(crash_exe, '/');
  if (slash && slash > crash_exe) {
    char *from = slash; while (from > crash_exe && from[-1] != '/') from--;
    size_t len = (size_t)(slash - from); if (len >= sizeof crash_release) len = sizeof crash_release - 1;
    memcpy(crash_release, from, len); crash_release[len] = 0;
  }
  const char *state = getenv("STATE_DIRECTORY");
  if (state && *state && !strchr(state, ':') && strlen(state) < sizeof crash_path - 32) {
    snprintf(crash_path, sizeof crash_path, "%s/crash.txt", state);
    snprintf(crash_last, sizeof crash_last, "%s/crash.last.txt", state);
  }
  setvbuf(stderr, crash_err, _IOLBF, sizeof crash_err);   /* a line at a time, its last one still here in the handler */
  void *warm[4]; (void)backtrace(warm, 4);                /* loads the unwinder now, never inside a handler */
  stack_t ss = {.ss_sp = crash_stack, .ss_size = sizeof crash_stack, .ss_flags = 0};
  sigaltstack(&ss, NULL);
  struct sigaction sa; memset(&sa, 0, sizeof sa);
  sa.sa_sigaction = crash_on; sa.sa_flags = SA_SIGINFO | SA_ONSTACK;
  sigemptyset(&sa.sa_mask);
  static const int sigs[] = {SIGSEGV, SIGBUS, SIGFPE, SIGILL, SIGABRT, SIGTRAP, SIGSYS};
  for (size_t i = 0; i < sizeof sigs / sizeof sigs[0]; i++) sigaction(sigs[i], &sa, NULL);
  signal(SIGUSR2, crash_usr2);   /* vm-deploy.sh crash: the crash test */
}
void crash_tick(int64_t wall_ms, int64_t us, int64_t players) {
  unsigned k = crash_ring_n % CRASH_TICKS;
  crash_ring[k].ms = wall_ms; crash_ring[k].us = us; crash_ring[k].players = players;
  crash_ring_n++;
}
bool crash_test_asked(void) { if (!crash_test_flag) return false; crash_test_flag = 0; return true; }

static bool crash_hex(const char *s, size_t n) {
  if (n < 1 || n > 16) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= '0' && s[i] <= '9') || (s[i] >= 'a' && s[i] <= 'f'))) return false;
  return true;
}
size_t crash_pending(char *dump, size_t dcap, char *names, size_t ncap, size_t *names_n) {
  *names_n = 0;
  if (!crash_path[0] || dcap < 2) return 0;
  FILE *f = fopen(crash_path, "r");
  if (!f) return 0;
  size_t n = fread(dump, 1, dcap - 1, f);
  fclose(f);
  dump[n] = 0;
  rename(crash_path, crash_last);   /* sent once; the last one stays on the disk to read */
  if (n == 0) return 0;
  /* the binary that crashed (its dump's exe line), while its release is still on the disk */
  char exe[512] = "", cmd[2048];
  const char *e = strstr(dump, "\nexe ");
  if (e) { e += 5; size_t len = strcspn(e, "\n"); if (len < sizeof exe) { memcpy(exe, e, len); exe[len] = 0; } }
  bool plain = exe[0] == '/';
  for (const char *c = exe; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= 'A' && *c <= 'Z') || (*c >= '0' && *c <= '9') || strchr("/._+-@", *c))) plain = false;
  if (!plain || access(exe, R_OK) || access("/usr/bin/addr2line", X_OK)) return n;
  /* -a: each offset's answer starts with its own "0x..." line; -i: the functions inlined there, innermost first */
  size_t k = (size_t)snprintf(cmd, sizeof cmd, "/usr/bin/addr2line -a -f -i -e '%s'", exe);
  int asked = 0;
  for (const char *l = dump; l && *l; l = strchr(l, '\n'), l = l ? l + 1 : NULL) {
    if (strncmp(l, "a ", 2)) continue;
    size_t len = strcspn(l + 2, "\n");
    if (!crash_hex(l + 2, len) || k + len + 8 >= sizeof cmd) continue;
    k += (size_t)snprintf(cmd + k, sizeof cmd - k, " 0x%.*s", (int)len, l + 2); asked++;
  }
  if (!asked) return n;
  snprintf(cmd + k, sizeof cmd - k, " 2>/dev/null");
  FILE *p = popen(cmd, "r");
  if (!p) return n;
  *names_n = fread(names, 1, ncap - 1, p);
  names[*names_n] = 0;
  pclose(p);
  return n;
}
#else
void crash_install(int shard) { (void)shard; }
void crash_tick(int64_t wall_ms, int64_t us, int64_t players) { (void)wall_ms; (void)us; (void)players; }
bool crash_test_asked(void) { return false; }
size_t crash_pending(char *dump, size_t dcap, char *names, size_t ncap, size_t *names_n) { (void)dump; (void)dcap; (void)names; (void)ncap; *names_n = 0; return 0; }
#endif
