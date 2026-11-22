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
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <shellapi.h>
#include <direct.h>
#include <errno.h>
#include <fcntl.h>
#include <io.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include "vk-load.h"

#define OA_PLATFORM "windows"
#define OA_PLATFORM_NAME "Windows"
#define OA_LOCAL_OS 3
#define OA_LOCAL_SERVER "psionic-inkling-serve.exe"
#ifdef OA_MINIMAL
#define OA_WORKER "inky.exe"
#else
#define OA_WORKER "mcworker.exe"
#endif
#define OA_INKY_WORKER "inky.exe"
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
static void oa_rmdir(const char *path) {
  if (!path || !path[0]) return;
  wchar_t w[MAX_PATH * 4];
  oa_wide(path, w, sizeof w / sizeof w[0]);
  oa_back(w);
  RemoveDirectoryW(w);
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

// Windows taskbar attention flash (#13874) and SetThreadExecutionState keep-awake (#13883).
static void oa_notify_os(const char *title, const char *body) {
  (void)title; (void)body;
  HWND hw = GetActiveWindow();
  if (!hw) hw = GetForegroundWindow();
  if (hw) {
    FLASHWINFO fi;
    memset(&fi, 0, sizeof fi);
    fi.cbSize = sizeof fi;
    fi.hwnd = hw;
    fi.dwFlags = FLASHW_TRAY | FLASHW_TIMERNOFG;
    fi.uCount = 3;
    FlashWindowEx(&fi);
  }
}
static void oa_keep_awake(int active) {
  SetThreadExecutionState(active ? (ES_CONTINUOUS | ES_SYSTEM_REQUIRED) : ES_CONTINUOUS);
}

static void oa_sleep_ms(int ms) {
  if (ms > 0) Sleep((DWORD)ms);
}

static void oa_mkdir_all(const char *path) {
  if (!path || !path[0]) return;
  char tmp[4096];
  snprintf(tmp, sizeof tmp, "%s", path);
  oa_forward(tmp);
  char *start = tmp;
  if (((start[0] >= 'A' && start[0] <= 'Z') || (start[0] >= 'a' && start[0] <= 'z')) && start[1] == ':' && start[2] == '/') {
    start += 3;
  } else if (start[0] == '/') {
    start += 1;
  }
  for (char *p = start; *p; p++) {
    if (*p == '/') {
      *p = '\0';
      oa_mkdir(tmp);
      *p = '/';
    }
  }
  oa_mkdir(tmp);
}

static int oa_file_stat(const char *path, int64_t *out_bytes, int64_t *out_mtime) {
  if (!path || !path[0]) return 0;
  wchar_t w[MAX_PATH * 4];
  oa_wide(path, w, sizeof w / sizeof w[0]);
  oa_back(w);
  WIN32_FILE_ATTRIBUTE_DATA fad;
  if (!GetFileAttributesExW(w, GetFileExInfoStandard, &fad)) return 0;
  if (fad.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) return 0;
  if (out_bytes) {
    *out_bytes = ((int64_t)fad.nFileSizeHigh << 32) | (int64_t)fad.nFileSizeLow;
  }
  if (out_mtime) {
    uint64_t ft = ((uint64_t)fad.ftLastWriteTime.dwHighDateTime << 32) | (uint64_t)fad.ftLastWriteTime.dwLowDateTime;
    *out_mtime = (ft >= 116444736000000000ULL) ? (int64_t)((ft - 116444736000000000ULL) / 10000000ULL) : 0;
  }
  return 1;
}

static int oa_rename_replace(const char *src, const char *dst) {
  if (!src || !dst) return 0;
  wchar_t ws[MAX_PATH * 4], wd[MAX_PATH * 4];
  oa_wide(src, ws, sizeof ws / sizeof ws[0]);
  oa_wide(dst, wd, sizeof wd / sizeof wd[0]);
  oa_back(ws);
  oa_back(wd);
  return MoveFileExW(ws, wd, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) != 0;
}

static int64_t oa_avail_disk_bytes(const char *path) {
  char probe[4096];
  snprintf(probe, sizeof probe, "%s", (path && path[0]) ? path : ".");
  oa_forward(probe);
  for (int guard = 0; guard < 32; guard++) {
    wchar_t wp[MAX_PATH * 4];
    oa_wide(probe, wp, sizeof wp / sizeof wp[0]);
    oa_back(wp);
    DWORD attr = GetFileAttributesW(wp);
    if (attr != INVALID_FILE_ATTRIBUTES) {
      ULARGE_INTEGER avail;
      if (GetDiskFreeSpaceExW(wp, &avail, NULL, NULL)) return (int64_t)avail.QuadPart;
      break;
    }
    char *slash = strrchr(probe, '/');
    if (!slash) break;
    if (slash == probe + 2 && probe[1] == ':') { probe[3] = '\0'; break; }
    *slash = '\0';
  }
  ULARGE_INTEGER avail;
  if (GetDiskFreeSpaceExW(NULL, &avail, NULL, NULL)) return (int64_t)avail.QuadPart;
  return 0;
}

// Local model hardware probe: GlobalMemoryStatusEx RAM + runtime LoadLibraryA("nvcuda.dll") VRAM probe.
// Never links nvcuda.dll at build time so non-NVIDIA hosts boot cleanly to onboarding.
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
  MEMORYSTATUSEX ms;
  memset(&ms, 0, sizeof ms);
  ms.dwLength = sizeof ms;
  if (GlobalMemoryStatusEx(&ms)) {
    out->system_ram_gb = (int)((ms.ullTotalPhys + (512ULL << 20)) / (1024ULL << 20));
  }
  HMODULE lib = LoadLibraryA("nvcuda.dll");
  if (!lib) return;
  typedef int (WINAPI *cuInit_fn)(unsigned int);
  typedef int (WINAPI *cuDeviceGetCount_fn)(int *);
  typedef int (WINAPI *cuDeviceGet_fn)(int *, int);
  typedef int (WINAPI *cuDeviceTotalMem_fn)(size_t *, int);
  typedef int (WINAPI *cuDeviceGetName_fn)(char *, int, int);
  cuInit_fn p_init = (cuInit_fn)GetProcAddress(lib, "cuInit");
  cuDeviceGetCount_fn p_count = (cuDeviceGetCount_fn)GetProcAddress(lib, "cuDeviceGetCount");
  cuDeviceGet_fn p_get = (cuDeviceGet_fn)GetProcAddress(lib, "cuDeviceGet");
  cuDeviceTotalMem_fn p_mem = (cuDeviceTotalMem_fn)GetProcAddress(lib, "cuDeviceTotalMem_v2");
  if (!p_mem) p_mem = (cuDeviceTotalMem_fn)GetProcAddress(lib, "cuDeviceTotalMem");
  cuDeviceGetName_fn p_name = (cuDeviceGetName_fn)GetProcAddress(lib, "cuDeviceGetName");
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
  FreeLibrary(lib);
}

typedef struct {
  HANDLE proc;
  DWORD pid;
  int active;
} OaSubproc;

static int oa_dl_start(OaSubproc *p, const char *url, const char *part_path, const char *log_path) {
  if (!p || !url || !part_path || !log_path) return 0;
  memset(p, 0, sizeof *p);
  wchar_t wlog[MAX_PATH * 4];
  oa_wide(log_path, wlog, sizeof wlog / sizeof wlog[0]);
  oa_back(wlog);
  SECURITY_ATTRIBUTES sa;
  memset(&sa, 0, sizeof sa);
  sa.nLength = sizeof sa;
  sa.bInheritHandle = TRUE;
  HANDLE hlog = CreateFileW(wlog, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, &sa, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
  char cmd[8192];
  snprintf(cmd, sizeof cmd, "curl.exe -fSL -C - --retry 3 --retry-delay 2 -o \"%s\" \"%s\"", part_path, url);
  wchar_t wcmd[8192];
  oa_wide(cmd, wcmd, sizeof wcmd / sizeof wcmd[0]);
  STARTUPINFOW si;
  PROCESS_INFORMATION pi;
  memset(&si, 0, sizeof si); si.cb = sizeof si;
  memset(&pi, 0, sizeof pi);
  BOOL inherit = FALSE;
  if (hlog != INVALID_HANDLE_VALUE) {
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hlog;
    si.hStdError = hlog;
    si.hStdInput = INVALID_HANDLE_VALUE;
    inherit = TRUE;
  }
  BOOL ok = CreateProcessW(NULL, wcmd, NULL, NULL, inherit, CREATE_NO_WINDOW, NULL, NULL, &si, &pi);
  if (hlog != INVALID_HANDLE_VALUE) CloseHandle(hlog);
  if (!ok) return 0;
  CloseHandle(pi.hThread);
  p->proc = pi.hProcess;
  p->pid = pi.dwProcessId;
  p->active = 1;
  return 1;
}

static int oa_dl_poll(OaSubproc *p, int *out_code) {
  if (!p || !p->active) return 0;
  if (WaitForSingleObject(p->proc, 0) != WAIT_OBJECT_0) return 0;
  DWORD code = 1;
  GetExitCodeProcess(p->proc, &code);
  CloseHandle(p->proc);
  p->proc = NULL;
  p->pid = 0;
  p->active = 0;
  if (out_code) *out_code = (int)code;
  return 1;
}

static void oa_dl_stop(OaSubproc *p) {
  if (!p || !p->active) return;
  if (p->proc) {
    TerminateProcess(p->proc, 1);
    WaitForSingleObject(p->proc, 1000);
    CloseHandle(p->proc);
    p->proc = NULL;
  }
  p->pid = 0;
  p->active = 0;
}

#define OA_PROC_ENVS 4
// A program run in the background, no window, its stdout and stderr appended to log_path: argv[0]
// the program and argv NULL-ended; envs, when not NULL, up to OA_PROC_ENVS "NAME=value" entries
// (NULL-ended) set for it alone. Polled and stopped with oa_dl_poll / oa_dl_stop. 1 when it started.
static int oa_proc_logged(OaSubproc *p, const char *const *argv, const char *const *envs, const char *log_path) {
  if (!p || !argv || !argv[0] || !log_path) return 0;
  memset(p, 0, sizeof *p);
  char cmd[8192];
  size_t at = 0;
  for (int i = 0; argv[i]; i++) {
    int w = snprintf(cmd + at, sizeof cmd - at, "%s\"%s\"", i ? " " : "", argv[i]);
    if (w < 0 || (size_t)w >= sizeof cmd - at) return 0;
    at += (size_t)w;
  }
  wchar_t wcmd[8192];
  oa_wide(cmd, wcmd, sizeof wcmd / sizeof wcmd[0]);
  wchar_t wlog[MAX_PATH * 4];
  oa_wide(log_path, wlog, sizeof wlog / sizeof wlog[0]);
  oa_back(wlog);
  SECURITY_ATTRIBUTES sa;
  memset(&sa, 0, sizeof sa);
  sa.nLength = sizeof sa;
  sa.bInheritHandle = TRUE;
  HANDLE hlog = CreateFileW(wlog, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, &sa, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
  // the variables are set in this process around CreateProcess (the child copies the block) and put back after
  static wchar_t wname[OA_PROC_ENVS][256], wold[OA_PROC_ENVS][4096];
  wchar_t wval[4096];
  int had_old[OA_PROC_ENVS] = {0}, nset = 0;
  for (int i = 0; envs && envs[i] && nset < OA_PROC_ENVS; i++) {
    const char *env = envs[i];
    const char *eq = strchr(env, '=');
    if (!eq || eq == env || (size_t)(eq - env) >= 255) continue;
    char name[256];
    memcpy(name, env, (size_t)(eq - env)); name[eq - env] = '\0';
    oa_wide(name, wname[nset], 256);
    oa_wide(eq + 1, wval, 4096);
    had_old[nset] = GetEnvironmentVariableW(wname[nset], wold[nset], 4096) > 0;
    SetEnvironmentVariableW(wname[nset], wval);
    nset++;
  }
  STARTUPINFOW si;
  PROCESS_INFORMATION pi;
  memset(&si, 0, sizeof si); si.cb = sizeof si;
  memset(&pi, 0, sizeof pi);
  BOOL inherit = FALSE;
  if (hlog != INVALID_HANDLE_VALUE) {
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hlog;
    si.hStdError = hlog;
    si.hStdInput = INVALID_HANDLE_VALUE;
    inherit = TRUE;
  }
  BOOL ok = CreateProcessW(NULL, wcmd, NULL, NULL, inherit, CREATE_NO_WINDOW, NULL, NULL, &si, &pi);
  for (int i = 0; i < nset; i++) SetEnvironmentVariableW(wname[i], had_old[i] ? wold[i] : NULL);
  if (hlog != INVALID_HANDLE_VALUE) CloseHandle(hlog);
  if (!ok) return 0;
  CloseHandle(pi.hThread);
  p->proc = pi.hProcess;
  p->pid = pi.dwProcessId;
  p->active = 1;
  return 1;
}

typedef struct { void (*fn)(void *); void *arg; } OaThreadBox;
static DWORD WINAPI oa_thread_trampoline(LPVOID raw) {
  OaThreadBox *b = (OaThreadBox *)raw;
  void (*fn)(void *) = b->fn;
  void *arg = b->arg;
  free(b);
  fn(arg);
  return 0;
}
static int oa_spawn_thread(void (*fn)(void *), void *arg) {
  OaThreadBox *b = (OaThreadBox *)malloc(sizeof *b);
  if (!b) return 0;
  b->fn = fn; b->arg = arg;
  HANDLE h = CreateThread(NULL, 4 * 1024 * 1024, oa_thread_trampoline, b, 0, NULL);
  if (!h) { free(b); return 0; }
  CloseHandle(h);
  return 1;
}

// Detached psionic-inkling-serve.exe launch (DETACHED_PROCESS | CREATE_NO_WINDOW, logged to log_path).
static int64_t oa_server_spawn_detached(
    const char *bin, const char *weights, const char *host,
    int port, int ctx, int idle_exit_s, const char *log_path) {
  if (!bin || !bin[0] || !weights || !weights[0] || !log_path || !log_path[0]) return -1;
  SECURITY_ATTRIBUTES sa;
  memset(&sa, 0, sizeof sa);
  sa.nLength = sizeof sa;
  sa.bInheritHandle = TRUE;
  HANDLE hlog = CreateFileA(log_path, GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, &sa, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
  char cmd[8192];
  snprintf(cmd, sizeof cmd, "\"%s\" --model \"%s\" --host %s --port %d --ctx %d --idle-exit-s %d",
           bin, weights, host, port, ctx, idle_exit_s);
  STARTUPINFOA si;
  PROCESS_INFORMATION pi;
  memset(&si, 0, sizeof si); si.cb = sizeof si;
  memset(&pi, 0, sizeof pi);
  BOOL inherit = FALSE;
  if (hlog != INVALID_HANDLE_VALUE) {
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hlog;
    si.hStdError = hlog;
    si.hStdInput = INVALID_HANDLE_VALUE;
    inherit = TRUE;
  }
  BOOL ok = CreateProcessA(NULL, cmd, NULL, NULL, inherit, DETACHED_PROCESS | CREATE_NO_WINDOW, NULL, NULL, &si, &pi);
  if (hlog != INVALID_HANDLE_VALUE) CloseHandle(hlog);
  if (!ok) return -1;
  int64_t pid = (int64_t)pi.dwProcessId;
  CloseHandle(pi.hThread);
  CloseHandle(pi.hProcess);
  return pid;
}

static int oa_server_pid_alive(int64_t pid) {
  if (pid <= 0) return 0;
  HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, (DWORD)pid);
  if (!h) return 0;
  DWORD code = 0;
  int alive = GetExitCodeProcess(h, &code) && code == STILL_ACTIVE;
  CloseHandle(h);
  return alive;
}

static void oa_server_pid_stop(int64_t pid) {
  if (pid <= 0) return;
  HANDLE h = OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE, FALSE, (DWORD)pid);
  if (!h) return;
  TerminateProcess(h, 0);
  WaitForSingleObject(h, 2000);
  CloseHandle(h);
}

static void oa_win_wsa_once(void) {
  static int inited = 0;
  if (!inited) {
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) == 0) inited = 1;
  }
}

// Inspect listening TCP port via dynamic iphlpapi.dll GetExtendedTcpTable (fallback bind probe).
static int oa_port_conflict(int port, char *out, size_t cap) {
  if (out && cap > 0) out[0] = '\0';
  oa_win_wsa_once();
  typedef struct {
    DWORD dwState;
    DWORD dwLocalAddr;
    DWORD dwLocalPort;
    DWORD dwRemoteAddr;
    DWORD dwRemotePort;
    DWORD dwOwningPid;
  } OaTcpRowPid;
  typedef struct {
    DWORD dwNumEntries;
    OaTcpRowPid table[1];
  } OaTcpTablePid;
  typedef DWORD (WINAPI *GetExtTcp_fn)(PVOID, PDWORD, BOOL, ULONG, int, ULONG);
  HMODULE iph = LoadLibraryA("iphlpapi.dll");
  if (iph) {
    GetExtTcp_fn p_get = (GetExtTcp_fn)GetProcAddress(iph, "GetExtendedTcpTable");
    if (p_get) {
      DWORD sz = 0;
      p_get(NULL, &sz, FALSE, AF_INET, 3 /* TCP_TABLE_OWNER_PID_LISTENER */, 0);
      if (sz > 0) {
        OaTcpTablePid *tbl = (OaTcpTablePid *)malloc(sz);
        if (tbl && p_get(tbl, &sz, FALSE, AF_INET, 3, 0) == NO_ERROR) {
          for (DWORD i = 0; i < tbl->dwNumEntries; i++) {
            u_short lp = ntohs((u_short)tbl->table[i].dwLocalPort);
            if ((int)lp == port) {
              DWORD pid = tbl->table[i].dwOwningPid;
              char pname[260] = {0};
              HANDLE hp = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
              if (hp) {
                DWORD plen = (DWORD)sizeof pname;
                if (QueryFullProcessImageNameA(hp, 0, pname, &plen) && plen > 0) {
                  char *sl = strrchr(pname, '\\');
                  if (sl && sl[1]) memmove(pname, sl + 1, strlen(sl + 1) + 1);
                } else {
                  pname[0] = '\0';
                }
                CloseHandle(hp);
              }
              if (out && cap > 0) {
                if (pname[0]) snprintf(out, cap, "%s (PID %lu)", pname, (unsigned long)pid);
                else snprintf(out, cap, "PID %lu", (unsigned long)pid);
              }
              free(tbl);
              FreeLibrary(iph);
              return 1;
            }
          }
        }
        free(tbl);
      }
    }
    FreeLibrary(iph);
  }
  SOCKET s = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
  if (s != INVALID_SOCKET) {
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof addr);
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = htons((u_short)port);
    int in_use = (bind(s, (struct sockaddr *)&addr, sizeof addr) == SOCKET_ERROR && WSAGetLastError() == WSAEADDRINUSE);
    closesocket(s);
    if (in_use) {
      if (out && cap > 0) snprintf(out, cap, "another process");
      return 1;
    }
  }
  return 0;
}

// Localhost GET /health probe: 0=free, 1=healthy Psionic server, 2=occupied by non-Psionic service.
static int oa_http_probe_health(int port) {
  oa_win_wsa_once();
  SOCKET s = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
  if (s == INVALID_SOCKET) return 0;
  u_long nb = 1;
  ioctlsocket(s, FIONBIO, &nb);
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof addr);
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  addr.sin_port = htons((u_short)port);
  int cr = connect(s, (struct sockaddr *)&addr, sizeof addr);
  if (cr == SOCKET_ERROR) {
    int err = WSAGetLastError();
    if (err != WSAEWOULDBLOCK && err != WSAEINPROGRESS) { closesocket(s); return 0; }
    fd_set wfds;
    FD_ZERO(&wfds); FD_SET(s, &wfds);
    struct timeval tv = {0, 500000};
    if (select(0, NULL, &wfds, NULL, &tv) <= 0) { closesocket(s); return 0; }
    int so_err = 0; int so_len = (int)sizeof so_err;
    if (getsockopt(s, SOL_SOCKET, SO_ERROR, (char *)&so_err, &so_len) != 0 || so_err != 0) { closesocket(s); return 0; }
  }
  const char *req = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
  (void)send(s, req, (int)strlen(req), 0);
  char resp[2048];
  size_t total = 0;
  for (int i = 0; i < 8 && total + 1 < sizeof resp; i++) {
    fd_set rfds;
    FD_ZERO(&rfds); FD_SET(s, &rfds);
    struct timeval tv = {0, 250000};
    if (select(0, &rfds, NULL, NULL, &tv) <= 0) break;
    int r = recv(s, resp + total, (int)(sizeof resp - 1 - total), 0);
    if (r <= 0) break;
    total += (size_t)r;
  }
  closesocket(s);
  resp[total] = '\0';
  if (total > 0 && strncmp(resp, "HTTP/1.", 7) == 0 && strstr(resp, " 200") &&
      strstr(resp, "psionic-") && strstr(resp, "\"ok\"")) {
    return 1;
  }
  return 2;
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
