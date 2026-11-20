// Firm reason: 6 (native Win32 host entry point; UI, store, worker, worktree, daemon and update logic are Coax).
// OpenAgents Desktop for Windows (GitHub #13782): the Windows side of host-main.c, which holds the
// app's entry logic (page.coax via host.h on apps/shared/host/windows/ui-host.c, the system's file
// chooser through portal.c, and the desktop core through host-core.coax). The program is a GUI
// program: what it writes to stderr goes to %LOCALAPPDATA%/OpenAgents/logs/openagents.log, and a
// startup failure is told in a message box that names that file. --smoke and --check-update write
// to the console they were started from.
#define WIN32_LEAN_AND_MEAN
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#include <windows.h>
#include <shellapi.h>
#include <direct.h>
#include <errno.h>
#include <fcntl.h>
#include <io.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include "vk-load.h"

#define OA_PLATFORM "windows"
#define OA_PLATFORM_NAME "Windows"
#define OA_WORKER "mcworker.exe"
#define OA_DAEMON "openagents-daemon.exe"

static char oa_log_path[4096];
static int oa_up;

// UTF-16 to UTF-8 and back, NUL-ended; "" when it does not fit.
static void oa_utf8(const wchar_t *w, char *out, size_t cap) {
  if (!cap) return;
  if (!w || WideCharToMultiByte(CP_UTF8, 0, w, -1, out, (int)cap, NULL, NULL) <= 0) out[0] = 0;
}
static void oa_wide(const char *s, wchar_t *out, size_t cap) {
  if (!cap) return;
  if (!s || MultiByteToWideChar(CP_UTF8, 0, s, -1, out, (int)cap) <= 0) out[0] = 0;
}
static void oa_forward(char *p) { for (; *p; p++) if (*p == '\\') *p = '/'; }
static void oa_back(wchar_t *p) { for (; *p; p++) if (*p == L'/') *p = L'\\'; }
static void oa_trim_slash(char *p) {
  size_t n = strlen(p);
  while (n > 3 && p[n - 1] == '/') p[--n] = 0;
}

static void oa_exe_dir(const char *argv0, char *out, size_t cap) {
  (void)argv0;
  wchar_t self[MAX_PATH * 4];
  DWORD n = GetModuleFileNameW(NULL, self, (DWORD)(sizeof self / sizeof self[0]));
  if (n == 0 || n >= sizeof self / sizeof self[0]) { snprintf(out, cap, "."); return; }
  oa_utf8(self, out, cap);
  oa_forward(out);
  char *slash = strrchr(out, '/');
  if (slash) *slash = 0;
}
static const char *oa_home(char *out, size_t cap) {
  wchar_t w[MAX_PATH * 4];
  DWORD n = GetEnvironmentVariableW(L"USERPROFILE", w, (DWORD)(sizeof w / sizeof w[0]));
  if (n == 0 || n >= sizeof w / sizeof w[0]) { out[0] = 0; return out; }
  oa_utf8(w, out, cap);
  oa_forward(out);
  oa_trim_slash(out);
  return out;
}
// The folder the app starts in: the current folder, unless that is the program's own (a double
// click in Explorer starts it there), then the home folder.
static void oa_working_dir(char *out, size_t cap) {
  wchar_t w[MAX_PATH * 4];
  DWORD n = GetCurrentDirectoryW((DWORD)(sizeof w / sizeof w[0]), w);
  if (n == 0 || n >= sizeof w / sizeof w[0]) out[0] = 0;
  else { oa_utf8(w, out, cap); oa_forward(out); oa_trim_slash(out); }
  char exe[4096];
  oa_exe_dir(NULL, exe, sizeof exe);
  if (!out[0] || _stricmp(out, exe) == 0) {
    char home[4096];
    oa_home(home, sizeof home);
    if (home[0]) snprintf(out, cap, "%s", home);
  }
}
static int oa_absolute(const char *p) {
  return (((p[0] >= 'A' && p[0] <= 'Z') || (p[0] >= 'a' && p[0] <= 'z')) && p[1] == ':' && (p[2] == '/' || p[2] == '\\')) || p[0] == '/' || p[0] == '\\';
}
static int oa_mkdir(const char *path) {
  wchar_t w[MAX_PATH * 4];
  oa_wide(path, w, sizeof w / sizeof w[0]);
  oa_back(w);
  return CreateDirectoryW(w, NULL) != 0;
}
static int oa_temp_dir(char *out, size_t cap) {
  wchar_t w[MAX_PATH + 1];
  DWORD n = GetTempPathW(MAX_PATH + 1, w);
  if (n == 0 || n > MAX_PATH) return 0;
  char base[4096];
  oa_utf8(w, base, sizeof base);
  oa_forward(base);
  oa_trim_slash(base);
  for (int i = 0; i < 16; i++) {
    snprintf(out, cap, "%s/oa-desktop-windows-smoke-%lu-%d", base, (unsigned long)GetCurrentProcessId(), i);
    if (oa_mkdir(out)) return 1;
  }
  return 0;
}
// A link in the default browser, or a file in the app Windows opens it with.
static void oa_open_external(const char *target) {
  if (!target || !target[0]) return;
  wchar_t w[4096];
  oa_wide(target, w, sizeof w / sizeof w[0]);
  if (oa_absolute(target)) oa_back(w);
  ShellExecuteW(NULL, L"open", w, NULL, NULL, SW_SHOWNORMAL);
}
static void oa_fail(const char *why) {
  fprintf(stderr, "openagents-desktop: %s\n", why);
  fflush(stderr);
  char text[8192];
  if (oa_log_path[0]) snprintf(text, sizeof text, "%s\n\nDetails are in %s", why, oa_log_path);
  else snprintf(text, sizeof text, "%s", why);
  wchar_t w[8192];
  oa_wide(text, w, sizeof w / sizeof w[0]);
  MessageBoxW(NULL, w, L"OpenAgents", MB_OK | MB_ICONERROR);
  oa_up = 1;   // told: no second note at exit
}
static void oa_started(void) { oa_up = 1; }

#include "../host-main.c"

// stderr (and stdout) into the log file beside the app's other data:
// %LOCALAPPDATA%/OpenAgents/logs/openagents.log, a new one each launch (the last kept as .old).
static void oa_open_log(void) {
  wchar_t w[MAX_PATH * 4];
  DWORD n = GetEnvironmentVariableW(L"LOCALAPPDATA", w, (DWORD)(sizeof w / sizeof w[0]));
  if (n == 0 || n >= sizeof w / sizeof w[0]) return;
  char dir[4096];
  oa_utf8(w, dir, sizeof dir);
  oa_forward(dir);
  oa_trim_slash(dir);
  char path[4096];
  snprintf(path, sizeof path, "%s/OpenAgents", dir); oa_mkdir(path);
  snprintf(path, sizeof path, "%s/OpenAgents/logs", dir); oa_mkdir(path);
  snprintf(oa_log_path, sizeof oa_log_path, "%s/OpenAgents/logs/openagents.log", dir);
  char old[4200];
  snprintf(old, sizeof old, "%s.old", oa_log_path);
  remove(old);
  rename(oa_log_path, old);
  if (!freopen(oa_log_path, "w", stderr)) { oa_log_path[0] = 0; return; }
  setvbuf(stderr, NULL, _IONBF, 0);
  if (freopen(oa_log_path, "a", stdout)) setvbuf(stdout, NULL, _IOLBF, 1024);
  for (char *p = oa_log_path; *p; p++) if (*p == '/') *p = '\\';   // as the person reads it
  time_t now = time(NULL);
  fprintf(stderr, "OpenAgents %s (build %d) for Windows started %s", OA_APP_VERSION, OA_APP_BUILD, ctime(&now));
}

// A console run (--smoke, --check-update, --shot): output goes to the console it came from.
static void oa_attach_console(void) {
  if (!AttachConsole(ATTACH_PARENT_PROCESS)) return;
  freopen("CONOUT$", "w", stdout);
  freopen("CONOUT$", "w", stderr);
}

// Leaving before the window came up with no message box said why (an exit in a library): the
// person still learns where the log is.
static void oa_exit_note(void) {
  if (!oa_up && oa_log_path[0]) {
    fflush(stderr);
    char text[4600];
    snprintf(text, sizeof text, "OpenAgents stopped before its window opened.\n\nDetails are in %s", oa_log_path);
    wchar_t w[4800];
    oa_wide(text, w, sizeof w / sizeof w[0]);
    MessageBoxW(NULL, w, L"OpenAgents", MB_OK | MB_ICONERROR);
  }
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE prev, PWSTR cmdline, int show) {
  (void)instance; (void)prev; (void)cmdline; (void)show;
  int argc = 0;
  wchar_t **wargv = CommandLineToArgvW(GetCommandLineW(), &argc);
  if (!wargv) return 1;
  char **argv = calloc((size_t)argc + 1, sizeof *argv);
  if (!argv) return 1;
  int console = 0;
  for (int i = 0; i < argc; i++) {
    int cap = WideCharToMultiByte(CP_UTF8, 0, wargv[i], -1, NULL, 0, NULL, NULL);
    argv[i] = malloc(cap > 0 ? (size_t)cap : 1);
    if (!argv[i]) return 1;
    oa_utf8(wargv[i], argv[i], cap > 0 ? (size_t)cap : 1);
    if (i > 0 && (strcmp(argv[i], "--smoke") == 0 || strcmp(argv[i], "--check-update") == 0 || strcmp(argv[i], "--shot") == 0)) console = 1;
  }
  LocalFree(wargv);
  if (console) oa_attach_console();
  else { oa_open_log(); atexit(oa_exit_note); }

  // Vulkan comes from the graphics driver (vulkan-1.dll): without it the window cannot draw, so
  // the person is told what to install before anything else runs.
  char why[512];
  if (!ui_vk_load(why, sizeof why)) {
    if (console) fprintf(stderr, "openagents-desktop: %s\n", why);
    else oa_fail(why);
    oa_up = 1;
    return 1;
  }
  int rc = oa_main(argc, argv);
  oa_up = 1;
  return rc;
}
