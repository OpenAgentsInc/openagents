// Firm reason: 6 (native Linux Wayland/Vulkan/D-Bus host entry point; UI, store, worker, worktree, daemon and update logic are Coax).
// OpenAgents Desktop for Linux (GitHub #13781): the Linux side of host-main.c, which holds the
// app's entry logic (page.coax via host.h on apps/shared/host/linux/ui-host.c, the portal's file
// chooser, and the desktop core through host-core.coax).
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <time.h>
#include <sys/wait.h>
#include <unistd.h>

#define OA_PLATFORM "linux"
#define OA_PLATFORM_NAME "Linux"
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
