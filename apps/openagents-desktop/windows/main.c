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
#define OA_CLAUDE_WORKER "claudeworker.exe"
#define OA_AGY_WORKER "agyworker.exe"
#define OA_DAEMON "coder-host.exe"
#define OA_DAEMON_ALT "openagents-daemon.exe"

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
static int oa_download_url(const char *url, const char *dest) {
  if (!url || !url[0] || !dest || !dest[0]) return 0;
  char cmd[8192];
  snprintf(cmd, sizeof cmd, "curl.exe -fsSL --max-time 120 -o \"%s\" \"%s\"", dest, url);
  wchar_t wcmd[8192];
  oa_wide(cmd, wcmd, sizeof wcmd / sizeof wcmd[0]);
  STARTUPINFOW si; PROCESS_INFORMATION pi;
  memset(&si, 0, sizeof si); si.cb = sizeof si;
  memset(&pi, 0, sizeof pi);
  if (!CreateProcessW(NULL, wcmd, NULL, NULL, FALSE, CREATE_NO_WINDOW, NULL, NULL, &si, &pi)) return 0;
  WaitForSingleObject(pi.hProcess, 130000);
  DWORD code = 1;
  GetExitCodeProcess(pi.hProcess, &code);
  CloseHandle(pi.hProcess);
  CloseHandle(pi.hThread);
  return code == 0;
}
static int oa_swap_self(const char *staged) {
  wchar_t self[MAX_PATH * 4], old[MAX_PATH * 4], next[MAX_PATH * 4];
  DWORD n = GetModuleFileNameW(NULL, self, (DWORD)(sizeof self / sizeof self[0]));
  if (n == 0 || n >= sizeof self / sizeof self[0]) return 0;
  oa_wide(staged, next, sizeof next / sizeof next[0]);
  oa_back(next);
  swprintf(old, sizeof old / sizeof old[0], L"%ls.old", self);
  DeleteFileW(old);
  if (!MoveFileExW(self, old, MOVEFILE_REPLACE_EXISTING)) return 0;
  if (!MoveFileExW(next, self, MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED)) {
    MoveFileExW(old, self, MOVEFILE_REPLACE_EXISTING);
    return 0;
  }
  return 1;
}

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

// Single-instance Named Pipe (\\.\pipe\openagents-desktop-activate) and HKCU openagents://
// protocol handler registration for Windows Desktop (#13889).
#define OA_WIN_PIPE L"\\\\.\\pipe\\openagents-desktop-activate"

static void oa_win_register_uri_scheme(void) {
  wchar_t self[MAX_PATH * 4];
  DWORD n = GetModuleFileNameW(NULL, self, (DWORD)(sizeof self / sizeof self[0]));
  if (n == 0 || n >= sizeof self / sizeof self[0]) return;
  HKEY hkey = NULL;
  if (RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\Classes\\openagents", 0, NULL, 0, KEY_WRITE, NULL, &hkey, NULL) == ERROR_SUCCESS) {
    const wchar_t *desc = L"URL:OpenAgents Protocol";
    const wchar_t *empty = L"";
    RegSetValueExW(hkey, NULL, 0, REG_SZ, (const BYTE *)desc, (DWORD)((wcslen(desc) + 1) * sizeof(wchar_t)));
    RegSetValueExW(hkey, L"URL Protocol", 0, REG_SZ, (const BYTE *)empty, (DWORD)sizeof(wchar_t));
    HKEY hcmd = NULL;
    if (RegCreateKeyExW(hkey, L"shell\\open\\command", 0, NULL, 0, KEY_WRITE, NULL, &hcmd, NULL) == ERROR_SUCCESS) {
      wchar_t cmd[MAX_PATH * 4 + 32];
      swprintf(cmd, sizeof cmd / sizeof cmd[0], L"\"%ls\" \"%%1\"", self);
      RegSetValueExW(hcmd, NULL, 0, REG_SZ, (const BYTE *)cmd, (DWORD)((wcslen(cmd) + 1) * sizeof(wchar_t)));
      RegCloseKey(hcmd);
    }
    RegCloseKey(hkey);
  }
}

static int oa_win_try_forward(const char *payload) {
  HANDLE h = CreateFileW(OA_WIN_PIPE, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING, 0, NULL);
  if (h == INVALID_HANDLE_VALUE) return 0;
  DWORD written = 0, readn = 0;
  BOOL ok = WriteFile(h, payload, (DWORD)strlen(payload), &written, NULL);
  char reply[64];
  if (ok) ok = ReadFile(h, reply, sizeof reply - 1, &readn, NULL) && readn > 0;
  CloseHandle(h);
  return ok ? 1 : 0;
}

static volatile LONG oa_win_pipe_running = 0;

static DWORD WINAPI oa_win_pipe_thread(LPVOID arg) {
  (void)arg;
  while (InterlockedCompareExchange(&oa_win_pipe_running, 1, 1) == 1) {
    HANDLE h = CreateNamedPipeW(
      OA_WIN_PIPE,
      PIPE_ACCESS_DUPLEX,
      PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
      4, 4096, 4096, 0, NULL
    );
    if (h == INVALID_HANDLE_VALUE) { Sleep(250); continue; }
    if (ConnectNamedPipe(h, NULL) || GetLastError() == ERROR_PIPE_CONNECTED) {
      char buf[4096];
      DWORD n = 0, w = 0;
      if (ReadFile(h, buf, sizeof buf - 1, &n, NULL) && n > 0) {
        buf[n] = '\0';
        WriteFile(h, "ok\n", 3, &w, NULL);
      }
      FlushFileBuffers(h);
      DisconnectNamedPipe(h);
    }
    CloseHandle(h);
  }
  return 0;
}

static void oa_win_start_activation_server(void) {
  oa_win_register_uri_scheme();
  InterlockedExchange(&oa_win_pipe_running, 1);
  HANDLE th = CreateThread(NULL, 0, oa_win_pipe_thread, NULL, 0, NULL);
  if (th) CloseHandle(th);
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE prev, PWSTR cmdline, int show) {
  (void)instance; (void)prev; (void)cmdline; (void)show;
  int argc = 0;
  wchar_t **wargv = CommandLineToArgvW(GetCommandLineW(), &argc);
  if (!wargv) return 1;
  char **argv = calloc((size_t)argc + 1, sizeof *argv);
  if (!argv) return 1;
  int console = 0;
  const char *uri = NULL;
  for (int i = 0; i < argc; i++) {
    int cap = WideCharToMultiByte(CP_UTF8, 0, wargv[i], -1, NULL, 0, NULL, NULL);
    argv[i] = malloc(cap > 0 ? (size_t)cap : 1);
    if (!argv[i]) return 1;
    oa_utf8(wargv[i], argv[i], cap > 0 ? (size_t)cap : 1);
    if (i > 0 && (strcmp(argv[i], "--smoke") == 0 || strcmp(argv[i], "--check-update") == 0 || strcmp(argv[i], "--shot") == 0)) console = 1;
    else if (i > 0 && strncmp(argv[i], "openagents://", 13) == 0) uri = argv[i];
  }
  LocalFree(wargv);
  if (!console && uri) {
    char msg[4096];
    snprintf(msg, sizeof msg, "{\"action\":\"uri\",\"uri\":\"%s\"}\n", uri);
    if (oa_win_try_forward(msg)) return 0;
  }
  if (console) oa_attach_console();
  else { oa_open_log(); atexit(oa_exit_note); oa_win_start_activation_server(); }

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
