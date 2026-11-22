// Firm reason: 6 (native Linux Wayland/Vulkan/D-Bus host entry point; UI, store, worker, worktree, daemon and update logic are Coax).
// OpenAgents Desktop for Linux (GitHub #13781): the Linux side of host-main.c, which holds the
// app's entry logic (page.coax via host.h on apps/shared/host/linux/ui-host.c, the portal's file
// chooser, and the desktop core through host-core.coax).
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <netinet/in.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/un.h>
#include <time.h>
#include <sys/wait.h>
#include <unistd.h>

#define OA_PLATFORM "linux"
#define OA_PLATFORM_NAME "Linux"
#define OA_LOCAL_OS 2
#define OA_LOCAL_SERVER "psionic-inkling-serve"
#define OA_WORKER "mcworker"
#define OA_CLAUDE_WORKER "claudeworker"
#define OA_AGY_WORKER "agyworker"
#define OA_DAEMON "coder-host"
#define OA_DAEMON_ALT "openagents-daemon"

static void oa_exe_dir(const char *argv0, char *out, size_t cap) {
  char self[PATH_MAX];
  ssize_t n = readlink("/proc/self/exe", self, sizeof(self) - 1);
  snprintf(out, cap, "%s", n > 0 ? (self[n] = '\0', self) : argv0 ? argv0 : "./x");
  char *slash = strrchr(out, '/');
  if (slash) *slash = '\0';
  else snprintf(out, cap, ".");
}
static const char *oa_home(char *out, size_t cap) {
  const char *home = getenv("HOME");
  snprintf(out, cap, "%s", home ? home : "");
  return out;
}
static void oa_working_dir(char *out, size_t cap) { if (!getcwd(out, cap)) snprintf(out, cap, "."); }
static int oa_absolute(const char *path) { return path[0] == '/'; }
static int oa_temp_dir(char *out, size_t cap) {
  snprintf(out, cap, "/tmp/oa-desktop-linux-smoke-XXXXXX");
  return mkdtemp(out) != NULL;
}
static int oa_mkdir(const char *path) { return mkdir(path, 0700) == 0; }
static void oa_rmdir(const char *path) { if (path && path[0]) rmdir(path); }
static void oa_open_external(const char *target) {
  if (!target || !target[0]) return;
  pid_t pid = fork();
  if (pid == 0) {
    execlp("xdg-open", "xdg-open", target, (char *)NULL);
    _exit(127);
  }
}
static void oa_fail(const char *why) { fprintf(stderr, "openagents-desktop: %s\n", why); }
static void oa_started(void) {}
static int oa_download_url(const char *url, const char *dest) {
  if (!url || !url[0] || !dest || !dest[0]) return 0;
  pid_t pid = fork();
  if (pid < 0) return 0;
  if (pid == 0) {
    execlp("curl", "curl", "-fsSL", "--max-time", "120", "-o", dest, url, (char *)NULL);
    _exit(127);
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return 0;
  return WIFEXITED(status) && WEXITSTATUS(status) == 0;
}
static int oa_swap_self(const char *staged) {
  char self[PATH_MAX];
  ssize_t n = readlink("/proc/self/exe", self, sizeof(self) - 1);
  if (n <= 0) return 0;
  self[n] = '\0';
  if (chmod(staged, 0755) != 0) return 0;
  return rename(staged, self) == 0;
}

// Linux freedesktop OS notification (notify-send; #13874) and systemd-inhibit keep-awake (#13883).
static pid_t oa_inhibit_pid = -1;
static void oa_notify_os(const char *title, const char *body) {
  if (!title || !body) return;
  pid_t pid = fork();
  if (pid == 0) {
    execlp("notify-send", "notify-send", "-a", "OpenAgents", title, body, (char *)NULL);
    _exit(127);
  }
}
static void oa_keep_awake(int active) {
  if (active) {
    if (oa_inhibit_pid > 0) return;
    pid_t pid = fork();
    if (pid == 0) {
      execlp("systemd-inhibit", "systemd-inhibit", "--what=idle:sleep", "--who=OpenAgents",
             "--why=Active agent runs", "sleep", "infinity", (char *)NULL);
      _exit(127);
    }
    if (pid > 0) oa_inhibit_pid = pid;
  } else if (oa_inhibit_pid > 0) {
    kill(oa_inhibit_pid, SIGTERM);
    waitpid(oa_inhibit_pid, NULL, WNOHANG);
    oa_inhibit_pid = -1;
  }
}

static void oa_sleep_ms(int ms) {
  if (ms <= 0) return;
  struct timespec ts = {ms / 1000, (long)(ms % 1000) * 1000000L};
  nanosleep(&ts, NULL);
}

static void oa_mkdir_all(const char *path) {
  if (!path || !path[0]) return;
  char tmp[4096];
  snprintf(tmp, sizeof tmp, "%s", path);
  for (char *p = tmp + 1; *p; p++) {
    if (*p == '/') {
      *p = '\0';
      mkdir(tmp, 0700);
      *p = '/';
    }
  }
  mkdir(tmp, 0700);
}

static int oa_file_stat(const char *path, int64_t *out_bytes, int64_t *out_mtime) {
  struct stat st;
  if (!path || !path[0] || stat(path, &st) != 0 || !S_ISREG(st.st_mode)) return 0;
  if (out_bytes) *out_bytes = (int64_t)st.st_size;
  if (out_mtime) *out_mtime = (int64_t)st.st_mtime;
  return 1;
}

static int oa_rename_replace(const char *src, const char *dst) {
  if (!src || !dst) return 0;
  return rename(src, dst) == 0;
}

static int64_t oa_avail_disk_bytes(const char *path) {
  char probe[4096];
  snprintf(probe, sizeof probe, "%s", (path && path[0]) ? path : "/");
  struct stat st;
  while (stat(probe, &st) != 0) {
    char *slash = strrchr(probe, '/');
    if (!slash) { snprintf(probe, sizeof probe, "."); break; }
    if (slash == probe) { probe[1] = '\0'; break; }
    *slash = '\0';
  }
  struct statvfs sv;
  if (statvfs(probe, &sv) != 0) return 0;
  return (int64_t)sv.f_bavail * (int64_t)sv.f_frsize;
}

// Local model hardware probe: /proc/meminfo RAM + runtime dlopen("libcuda.so.1") VRAM probe.
// Never links libcuda.so at build time so non-NVIDIA hosts boot cleanly to onboarding.
typedef struct {
  int os;
  int is_apple_silicon;
  int system_ram_gb;
  int backend;
  int gpu_vram_gb;
  int quant;
  char gpu_name[128];
} OaLocalHw;

static void oa_probe_local_hw(OaLocalHw *out) {
  if (!out) return;
  memset(out, 0, sizeof *out);
  out->os = OA_LOCAL_OS;
  FILE *mf = fopen("/proc/meminfo", "r");
  if (mf) {
    char line[256];
    while (fgets(line, sizeof line, mf)) {
      unsigned long long kb = 0;
      if (sscanf(line, "MemTotal: %llu kB", &kb) == 1) {
        out->system_ram_gb = (int)((kb + 524288ULL) / 1048576ULL);
        break;
      }
    }
    fclose(mf);
  }
  const char *paths[] = {
    getenv("PSIONIC_LIBCUDA"),
    "libcuda.so.1",
    "/run/opengl-driver/lib/libcuda.so.1",
    "/usr/lib/x86_64-linux-gnu/libcuda.so.1",
    "/usr/lib64/libcuda.so.1",
  };
  void *lib = NULL;
  for (size_t i = 0; i < sizeof paths / sizeof *paths && !lib; i++) {
    if (paths[i] && paths[i][0]) lib = dlopen(paths[i], RTLD_LAZY | RTLD_LOCAL);
  }
  if (!lib) return;
  typedef int (*cuInit_fn)(unsigned int);
  typedef int (*cuDeviceGetCount_fn)(int *);
  typedef int (*cuDeviceGet_fn)(int *, int);
  typedef int (*cuDeviceTotalMem_fn)(size_t *, int);
  typedef int (*cuDeviceGetName_fn)(char *, int, int);
  cuInit_fn p_init = NULL;
  cuDeviceGetCount_fn p_count = NULL;
  cuDeviceGet_fn p_get = NULL;
  cuDeviceTotalMem_fn p_mem = NULL;
  cuDeviceGetName_fn p_name = NULL;
  *(void **)&p_init = dlsym(lib, "cuInit");
  *(void **)&p_count = dlsym(lib, "cuDeviceGetCount");
  *(void **)&p_get = dlsym(lib, "cuDeviceGet");
  *(void **)&p_mem = dlsym(lib, "cuDeviceTotalMem_v2");
  if (!p_mem) *(void **)&p_mem = dlsym(lib, "cuDeviceTotalMem");
  *(void **)&p_name = dlsym(lib, "cuDeviceGetName");
  if (p_init && p_get && p_mem && p_init(0) == 0) {
    int count = 1;
    if (p_count && (p_count(&count) != 0 || count < 1)) count = 0;
    size_t best_bytes = 0;
    int best_dev = -1;
    for (int i = 0; i < count; i++) {
      int dev = 0;
      size_t bytes = 0;
      if (p_get(&dev, i) == 0 && p_mem(&bytes, dev) == 0 && bytes > best_bytes) {
        best_bytes = bytes;
        best_dev = dev;
      }
    }
    if (best_dev >= 0 && best_bytes > 0) {
      out->backend = 2; /* LM_BACKEND_CUDA */
      out->gpu_vram_gb = (int)((best_bytes + (512ULL << 20)) / (1024ULL << 20));
      if (p_name) {
        p_name(out->gpu_name, (int)sizeof out->gpu_name, best_dev);
        out->gpu_name[sizeof out->gpu_name - 1] = '\0';
      }
    }
  }
  dlclose(lib);
}

typedef struct {
  pid_t pid;
  int active;
} OaSubproc;

static int oa_dl_start(OaSubproc *p, const char *url, const char *part_path, const char *log_path) {
  if (!p || !url || !part_path || !log_path) return 0;
  memset(p, 0, sizeof *p);
  pid_t pid = fork();
  if (pid < 0) return 0;
  if (pid == 0) {
    int devnull = open("/dev/null", O_RDONLY);
    if (devnull >= 0) { dup2(devnull, 0); if (devnull > 2) close(devnull); }
    int lfd = open(log_path, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (lfd >= 0) {
      dup2(lfd, 1);
      dup2(lfd, 2);
      if (lfd > 2) close(lfd);
    }
    execlp("curl", "curl", "-fSL", "-C", "-", "--retry", "3", "--retry-delay", "2",
           "-o", part_path, url, (char *)NULL);
    _exit(127);
  }
  p->pid = pid;
  p->active = 1;
  return 1;
}

static int oa_dl_poll(OaSubproc *p, int *out_code) {
  if (!p || !p->active) return 0;
  int st = 0;
  pid_t got = waitpid(p->pid, &st, WNOHANG);
  if (got == 0) return 0;
  p->active = 0;
  p->pid = -1;
  if (out_code) *out_code = (got > 0 && WIFEXITED(st)) ? WEXITSTATUS(st) : -1;
  return 1;
}

static void oa_dl_stop(OaSubproc *p) {
  if (!p || !p->active) return;
  if (p->pid > 0) {
    kill(p->pid, SIGTERM);
    for (int i = 0; i < 20; i++) {
      int st = 0;
      if (waitpid(p->pid, &st, WNOHANG) != 0) break;
      oa_sleep_ms(25);
    }
    waitpid(p->pid, NULL, WNOHANG);
  }
  p->active = 0;
  p->pid = -1;
}

// A program run in the background, its stdout and stderr appended to log_path: argv[0] the
// program and argv NULL-ended; envs, when not NULL, "NAME=value" entries (NULL-ended) set for it
// alone. Polled and stopped with oa_dl_poll / oa_dl_stop. 1 when it started.
static int oa_proc_logged(OaSubproc *p, const char *const *argv, const char *const *envs, const char *log_path) {
  if (!p || !argv || !argv[0] || !log_path) return 0;
  memset(p, 0, sizeof *p);
  // Build the child environment before fork() so the child calls only async-signal-safe syscalls.
  size_t base_n = 0, add_n = 0;
  if (environ) while (environ[base_n]) base_n++;
  if (envs) while (envs[add_n]) add_n++;
  char **envp = (char **)malloc((base_n + add_n + 1) * sizeof *envp);
  if (!envp) return 0;
  size_t out_n = 0;
  for (size_t i = 0; i < base_n; i++) {
    const char *eq = strchr(environ[i], '=');
    int replaced = 0;
    if (eq && envs) {
      size_t klen = (size_t)(eq - environ[i]) + 1;
      for (size_t j = 0; j < add_n; j++) {
        if (strncmp(envs[j], environ[i], klen) == 0) { replaced = 1; break; }
      }
    }
    if (!replaced) envp[out_n++] = environ[i];
  }
  for (size_t j = 0; j < add_n; j++) {
    if (strchr(envs[j], '=')) envp[out_n++] = (char *)envs[j];
  }
  envp[out_n] = NULL;
  pid_t pid = fork();
  if (pid < 0) { free(envp); return 0; }
  if (pid == 0) {
    int devnull = open("/dev/null", O_RDONLY);
    if (devnull >= 0) { dup2(devnull, 0); if (devnull > 2) close(devnull); }
    int lfd = open(log_path, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (lfd >= 0) {
      dup2(lfd, 1);
      dup2(lfd, 2);
      if (lfd > 2) close(lfd);
    }
    execve(argv[0], (char *const *)argv, envp);
    _exit(127);
  }
  free(envp);
  p->pid = pid;
  p->active = 1;
  return 1;
}

typedef struct { void (*fn)(void *); void *arg; } OaThreadBox;
static void *oa_thread_trampoline(void *raw) {
  OaThreadBox *b = (OaThreadBox *)raw;
  void (*fn)(void *) = b->fn;
  void *arg = b->arg;
  free(b);
  fn(arg);
  return NULL;
}
static int oa_spawn_thread(void (*fn)(void *), void *arg) {
  OaThreadBox *b = (OaThreadBox *)malloc(sizeof *b);
  if (!b) return 0;
  b->fn = fn; b->arg = arg;
  pthread_attr_t attr;
  pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, 4 * 1024 * 1024);
  pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
  pthread_t tid;
  int rc = pthread_create(&tid, &attr, oa_thread_trampoline, b);
  pthread_attr_destroy(&attr);
  if (rc != 0) { free(b); return 0; }
  return 1;
}

// Detached psionic-inkling-serve launch (setsid + stdout/stderr to log_path, never /dev/null).
static int64_t oa_server_spawn_detached(
    const char *bin, const char *weights, const char *host,
    int port, int ctx, int idle_exit_s, const char *log_path) {
  if (!bin || !bin[0] || !weights || !weights[0] || !log_path || !log_path[0]) return -1;
  char port_s[16], ctx_s[16], idle_s[16];
  snprintf(port_s, sizeof port_s, "%d", port);
  snprintf(ctx_s, sizeof ctx_s, "%d", ctx);
  snprintf(idle_s, sizeof idle_s, "%d", idle_exit_s);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    setsid();
    int devnull = open("/dev/null", O_RDONLY);
    if (devnull >= 0) { dup2(devnull, 0); if (devnull > 2) close(devnull); }
    int lfd = open(log_path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (lfd >= 0) {
      dup2(lfd, 1);
      dup2(lfd, 2);
      if (lfd > 2) close(lfd);
    }
    execl(bin, bin, "--model", weights, "--host", host, "--port", port_s,
          "--ctx", ctx_s, "--idle-exit-s", idle_s, (char *)NULL);
    _exit(127);
  }
  return (int64_t)pid;
}

static int oa_server_pid_alive(int64_t pid) {
  if (pid <= 0) return 0;
  int st = 0;
  pid_t w = waitpid((pid_t)pid, &st, WNOHANG);
  if (w == (pid_t)pid) return 0;
  return kill((pid_t)pid, 0) == 0;
}

static void oa_server_pid_stop(int64_t pid) {
  if (pid <= 0) return;
  kill((pid_t)pid, SIGTERM);
  for (int i = 0; i < 40; i++) {
    int st = 0;
    pid_t w = waitpid((pid_t)pid, &st, WNOHANG);
    if (w == (pid_t)pid || (w < 0 && errno == ECHILD)) {
      if (kill((pid_t)pid, 0) != 0) return;
    }
    oa_sleep_ms(50);
  }
  kill((pid_t)pid, SIGKILL);
  waitpid((pid_t)pid, NULL, WNOHANG);
}

static int oa_run_capture(const char *const argv[], char *out, size_t cap) {
  int fds[2];
  if (pipe2(fds, O_CLOEXEC) != 0) return 0;
  pid_t pid = fork();
  if (pid < 0) { close(fds[0]); close(fds[1]); return 0; }
  if (pid == 0) {
    close(fds[0]);
    dup2(fds[1], 1);
    int dn = open("/dev/null", O_WRONLY);
    if (dn >= 0) { dup2(dn, 2); if (dn > 2) close(dn); }
    close(fds[1]);
    execvp(argv[0], (char *const *)argv);
    _exit(127);
  }
  close(fds[1]);
  size_t total = 0;
  while (total + 1 < cap) {
    ssize_t r = read(fds[0], out + total, cap - 1 - total);
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0) break;
    total += (size_t)r;
  }
  out[total] = '\0';
  close(fds[0]);
  int st = 0;
  waitpid(pid, &st, 0);
  return WIFEXITED(st) && WEXITSTATUS(st) == 0 && total > 0;
}

// Inspect listening TCP port via ss / lsof (fallback bind probe) so conflicts surface immediately.
static int oa_port_conflict(int port, char *out, size_t cap) {
  if (out && cap > 0) out[0] = '\0';
  char buf[4096];
  char filter[64];
  snprintf(filter, sizeof filter, "sport = :%d", port);
  const char *ss_argv[] = {"ss", "-H", "-tlnp", filter, NULL};
  if (oa_run_capture(ss_argv, buf, sizeof buf)) {
    const char *u = strstr(buf, "users:((\"");
    if (u) {
      u += 9;
      char cmd[128] = {0};
      size_t ci = 0;
      while (*u && *u != '"' && ci + 1 < sizeof cmd) cmd[ci++] = *u++;
      const char *pp = strstr(u, "pid=");
      int pid = pp ? atoi(pp + 4) : 0;
      if (out && cap > 0) {
        if (cmd[0] && pid > 0) snprintf(out, cap, "%s (PID %d)", cmd, pid);
        else if (pid > 0) snprintf(out, cap, "PID %d", pid);
        else if (cmd[0]) snprintf(out, cap, "%s", cmd);
        else snprintf(out, cap, "another process");
      }
      return 1;
    }
    if (strstr(buf, "LISTEN")) {
      if (out && cap > 0) snprintf(out, cap, "another process");
      return 1;
    }
  }
  char spec[64];
  snprintf(spec, sizeof spec, "-iTCP:%d", port);
  const char *lsof_argv[] = {"lsof", "-nP", spec, "-sTCP:LISTEN", "-Fp", "-Fc", NULL};
  if (oa_run_capture(lsof_argv, buf, sizeof buf)) {
    char pid_s[32] = {0}, cmd[128] = {0};
    char *line = buf;
    while (line && *line) {
      char *nl = strchr(line, '\n');
      if (nl) *nl = '\0';
      if (line[0] == 'p' && !pid_s[0]) snprintf(pid_s, sizeof pid_s, "%s", line + 1);
      else if (line[0] == 'c' && !cmd[0]) snprintf(cmd, sizeof cmd, "%s", line + 1);
      line = nl ? nl + 1 : NULL;
    }
    if (pid_s[0]) {
      if (out && cap > 0) {
        if (cmd[0]) snprintf(out, cap, "%s (PID %s)", cmd, pid_s);
        else snprintf(out, cap, "PID %s", pid_s);
      }
      return 1;
    }
  }
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd >= 0) {
    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof addr);
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = htons((uint16_t)port);
    int in_use = (bind(fd, (struct sockaddr *)&addr, sizeof addr) != 0 && errno == EADDRINUSE);
    close(fd);
    if (in_use) {
      if (out && cap > 0) snprintf(out, cap, "another process");
      return 1;
    }
  }
  return 0;
}

// Localhost GET /health probe: 0=free, 1=healthy Psionic server, 2=occupied by non-Psionic service.
static int oa_http_probe_health(int port) {
  int fd = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
  if (fd < 0) return 0;
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof addr);
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  addr.sin_port = htons((uint16_t)port);
  int cr = connect(fd, (struct sockaddr *)&addr, sizeof addr);
  if (cr != 0 && errno != EINPROGRESS) { close(fd); return 0; }
  if (cr != 0) {
    struct pollfd pf = {fd, POLLOUT, 0};
    if (poll(&pf, 1, 500) <= 0) { close(fd); return 0; }
    int err = 0; socklen_t el = sizeof err;
    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &el) != 0 || err != 0) { close(fd); return 0; }
  }
  const char *req = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
  (void)send(fd, req, strlen(req), MSG_NOSIGNAL);
  char resp[2048];
  size_t total = 0;
  for (int i = 0; i < 8 && total + 1 < sizeof resp; i++) {
    struct pollfd pf = {fd, POLLIN, 0};
    if (poll(&pf, 1, 250) <= 0) break;
    ssize_t r = recv(fd, resp + total, sizeof resp - 1 - total, 0);
    if (r <= 0) break;
    total += (size_t)r;
  }
  close(fd);
  resp[total] = '\0';
  if (total > 0 && strncmp(resp, "HTTP/1.", 7) == 0 && strstr(resp, " 200") &&
      strstr(resp, "psionic-") && strstr(resp, "\"ok\"")) {
    return 1;
  }
  return 2;
}

#include "../host-main.c"

// Single-instance AF_UNIX activation socket (~/.openagents/desktop/activate.sock) and
// openagents:// URI forwarding for Linux Desktop (#13889).
static int oa_linux_sock_path(char *out, size_t cap) {
  const char *home = getenv("HOME");
  if (!home || !home[0]) return 0;
  char dir[PATH_MAX];
  snprintf(dir, sizeof dir, "%s/.openagents", home);
  mkdir(dir, 0700);
  snprintf(dir, sizeof dir, "%s/.openagents/desktop", home);
  mkdir(dir, 0700);
  int n = snprintf(out, cap, "%s/.openagents/desktop/activate.sock", home);
  return n > 0 && (size_t)n < cap;
}

static int oa_linux_try_forward(const char *sock_path, const char *payload) {
  struct sockaddr_un addr;
  memset(&addr, 0, sizeof addr);
  addr.sun_family = AF_UNIX;
  if (strlen(sock_path) >= sizeof addr.sun_path) return 0;
  snprintf(addr.sun_path, sizeof addr.sun_path, "%s", sock_path);
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0) return 0;
  if (connect(fd, (struct sockaddr *)&addr, sizeof addr) != 0) {
    close(fd);
    return 0;
  }
  size_t len = strlen(payload);
  ssize_t w = write(fd, payload, len);
  char reply[64];
  ssize_t r = w == (ssize_t)len ? read(fd, reply, sizeof reply - 1) : -1;
  close(fd);
  return r > 0;
}

static int oa_linux_listen_fd = -1;
static char oa_linux_bound_sock[PATH_MAX];

static void *oa_linux_activate_thread(void *arg) {
  (void)arg;
  while (oa_linux_listen_fd >= 0) {
    int cfd = accept(oa_linux_listen_fd, NULL, NULL);
    if (cfd < 0) {
      if (errno == EINTR) continue;
      break;
    }
    char buf[4096];
    ssize_t n = read(cfd, buf, sizeof buf - 1);
    if (n > 0) {
      buf[n] = '\0';
      (void)write(cfd, "ok\n", 3);
    }
    close(cfd);
  }
  return NULL;
}

static void oa_linux_start_activation_server(const char *sock_path) {
  struct sockaddr_un addr;
  memset(&addr, 0, sizeof addr);
  addr.sun_family = AF_UNIX;
  if (strlen(sock_path) >= sizeof addr.sun_path) return;
  snprintf(addr.sun_path, sizeof addr.sun_path, "%s", sock_path);
  unlink(sock_path);
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0) return;
  if (bind(fd, (struct sockaddr *)&addr, sizeof addr) != 0 || listen(fd, 8) != 0) {
    close(fd);
    return;
  }
  chmod(sock_path, 0600);
  oa_linux_listen_fd = fd;
  snprintf(oa_linux_bound_sock, sizeof oa_linux_bound_sock, "%s", sock_path);
  pthread_t tid;
  if (pthread_create(&tid, NULL, oa_linux_activate_thread, NULL) == 0) {
    pthread_detach(tid);
  }
}

static void oa_linux_stop_activation_server(void) {
  if (oa_linux_listen_fd >= 0) {
    int fd = oa_linux_listen_fd;
    oa_linux_listen_fd = -1;
    close(fd);
  }
  if (oa_linux_bound_sock[0]) {
    unlink(oa_linux_bound_sock);
    oa_linux_bound_sock[0] = '\0';
  }
}

int main(int argc, char **argv) {
  int headless = 0;
  const char *uri = NULL;
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--smoke") == 0 || strcmp(argv[i], "--flow-test") == 0 || strcmp(argv[i], "--input-test") == 0 ||
        strcmp(argv[i], "--shot") == 0 || strcmp(argv[i], "--check-update") == 0) {
      headless = 1;
    } else if (strncmp(argv[i], "openagents://", 13) == 0) {
      uri = argv[i];
    }
  }
  char sock_path[PATH_MAX];
  if (!headless && oa_linux_sock_path(sock_path, sizeof sock_path)) {
    if (uri) {
      char msg[4096];
      snprintf(msg, sizeof msg, "{\"action\":\"uri\",\"uri\":\"%s\"}\n", uri);
      if (oa_linux_try_forward(sock_path, msg)) return 0;
    }
    oa_linux_start_activation_server(sock_path);
    atexit(oa_linux_stop_activation_server);
  }
  return oa_main(argc, argv);
}
