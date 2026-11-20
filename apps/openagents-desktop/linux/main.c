// Firm reason: 6 (native Linux Wayland/Vulkan/D-Bus host entry point; UI, store, worker, worktree, daemon and update logic are Coax).
// OpenAgents Desktop for Linux (GitHub #13781): the Linux side of host-main.c, which holds the
// app's entry logic (page.coax via host.h on apps/shared/host/linux/ui-host.c, the portal's file
// chooser, and the desktop core through host-core.coax).
#define _GNU_SOURCE
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <sys/wait.h>
#include <unistd.h>

#define OA_PLATFORM "linux"
#define OA_PLATFORM_NAME "Linux"
#define OA_WORKER "mcworker"
#define OA_DAEMON "openagents-daemon"

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

#include "../host-main.c"

int main(int argc, char **argv) { return oa_main(argc, argv); }
