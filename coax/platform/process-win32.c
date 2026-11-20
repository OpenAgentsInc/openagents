/* Child processes on Windows (GitHub #13782): process-native.h's contract over CreateProcess,
   anonymous pipes and job objects. OS effects only; what to run and how to talk to it is Coax,
   exactly as with process-native.c, which stays the POSIX build's.

   Differences the contract allows, by Windows' nature:
   - A child gets one command line (coax/platform/win32-args.h quotes each argument).
   - A program named by a POSIX path ("/bin/sh") is looked for by its last part, and a bare name
     on PATH and then in Git for Windows' folders (sh, bash and git come from there), with .exe
     added. A named file is taken as given.
   - Each child runs in a job object of its own, so a kill takes its helpers too (POSIX's
     process group) and the job ends them when this process goes (POSIX's guard_children).
     Signals do not exist: a kill ends the job with exit status 128 + sig, which is what
     cxproc_wait reports for a signalled POSIX child.
   - No console window opens for a child of a windowed program (CREATE_NO_WINDOW).
   - Pseudo-terminals, /proc and exec are not Windows': cxproc_pty, cxproc_cmdline return -1,
     and cxproc_exec runs the program and exits with its status. */
#include "win32-native.h"
#include "win32-args.h"
#include "process-native.h"
#include <stdio.h>

typedef struct { char *data; size_t count, cap; } Bytes;
typedef struct { Bytes *items; size_t count, cap; Bytes open; } List;

typedef struct {
  int used, started, reaped, quiet, detach, outnull, errours, errtoout;
  DWORD status, pid;
  HANDLE process, job, in, out, err;
  List args, env, unset;
  Bytes cwd;
  Bytes buf;
  size_t line_start, line_len, scan;
  int eof, skipping;
  Bytes wbuf;
  char keep[CXPROC_STDERR_KEEP];
  int64_t kept;
} Proc;

static Proc procs[CXPROC_MAX];

static int put(Bytes *b, char c) {
  if (b->count == b->cap) {
    size_t cap = b->cap ? b->cap * 2 : 64;
    char *d = realloc(b->data, cap);
    if (!d) return -1;
    b->data = d; b->cap = cap;
  }
  b->data[b->count++] = c;
  return 0;
}
static char *cstr(Bytes *b) { if (put(b, 0)) return NULL; b->count--; return b->data; }
static void drop(Bytes *b) { free(b->data); b->data = NULL; b->count = b->cap = 0; }
static int close_entry(List *l) {
  if (l->count == l->cap) {
    size_t cap = l->cap ? l->cap * 2 : 8;
    Bytes *d = realloc(l->items, cap * sizeof(Bytes));
    if (!d) return -1;
    l->items = d; l->cap = cap;
  }
  if (!cstr(&l->open)) return -1;
  l->items[l->count++] = l->open;
  memset(&l->open, 0, sizeof l->open);
  return 0;
}
static void drop_list(List *l) {
  for (size_t i = 0; i < l->count; i++) drop(&l->items[i]);
  free(l->items); drop(&l->open);
  memset(l, 0, sizeof *l);
}
static Proc *get(int64_t h) {
  if (h < 0 || h >= CXPROC_MAX || !procs[h].used) return NULL;
  return &procs[h];
}
static void shut(HANDLE *h) { if (*h && *h != INVALID_HANDLE_VALUE) CloseHandle(*h); *h = NULL; }

int64_t cxproc_now_ms(void) { return (int64_t)GetTickCount64(); }
int64_t cxproc_now_ns(void) {
  LARGE_INTEGER f, c;
  if (!QueryPerformanceFrequency(&f) || !QueryPerformanceCounter(&c)) return -1;
  return (int64_t)((double)c.QuadPart * 1e9 / (double)f.QuadPart);
}
int64_t cxproc_wall_us(void) {
  FILETIME ft; GetSystemTimePreciseAsFileTime(&ft);
  uint64_t t = ((uint64_t)ft.dwHighDateTime << 32) | ft.dwLowDateTime;   /* 100 ns since 1601 */
  return (int64_t)(t / 10 - 11644473600000000ull);
}
int64_t cxproc_username(coax_v_u8 out) {
  wchar_t w[257]; DWORD n = 257; char u[1024];
  if (!GetUserNameW(w, &n)) return -1;
  int64_t k = cxwin_utf8_into(w, u, sizeof u);
  if (k < 1 || k > out.count) return -1;
  memcpy(out.data, u, (size_t)k);
  return k;
}
int64_t cxproc_self_path(coax_v_u8 out) {
  wchar_t w[32768]; char u[4 * 4096];
  DWORD n = GetModuleFileNameW(NULL, w, 32768);
  if (!n || n >= 32768) return -1;
  int64_t k = cxwin_utf8_into(w, u, sizeof u);
  if (k < 0 || k > out.count) return -1;
  cxwin_forward(u);
  memcpy(out.data, u, (size_t)k);
  return k;
}
int64_t cxproc_self_pid(void) { return (int64_t)GetCurrentProcessId(); }
int64_t cxproc_uid(void) { return 0; }   /* Windows has SIDs, not numeric user ids */
int64_t cxproc_local_stamp(coax_v_u8 out) {
  SYSTEMTIME t; GetLocalTime(&t);
  if (out.count < 16) return -1;
  char b[32];
  snprintf(b, sizeof b, "%04u%02u%02u-%02u%02u%02u", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
  memcpy(out.data, b, 15);
  return 15;
}

int64_t cxproc_new(void) {
  for (int i = 0; i < CXPROC_MAX; i++) if (!procs[i].used) {
    memset(&procs[i], 0, sizeof procs[i]);
    procs[i].used = 1;
    return i;
  }
  return -1;
}
#define FLAG(name, field) int64_t name(int64_t h) { Proc *p = get(h); if (!p || p->started) return -1; p->field = 1; return 0; }
FLAG(cxproc_quiet, quiet)
FLAG(cxproc_detach, detach)
FLAG(cxproc_out_null, outnull)
FLAG(cxproc_err_ours, errours)
FLAG(cxproc_err_to_out, errtoout)
#undef FLAG
int64_t cxproc_gentle(int64_t h) { Proc *p = get(h); return p && !p->started ? 0 : -1; }
int64_t cxproc_sleep_ms(int64_t ms) { if (ms < 0 || ms > 86400000) return -1; Sleep((DWORD)ms); return 0; }

int64_t cxproc_push(int64_t h, int64_t field, int64_t byte) {
  Proc *p = get(h);
  if (!p || p->started || byte <= 0 || byte > 255) return -1;
  Bytes *b = field == 0 ? &p->args.open : field == 1 ? &p->env.open : field == 2 ? &p->unset.open : field == 3 ? &p->cwd : NULL;
  return b ? put(b, (char)byte) : -1;
}
int64_t cxproc_end(int64_t h, int64_t field) {
  Proc *p = get(h);
  if (!p || p->started) return -1;
  if (field == 0) return close_entry(&p->args);
  if (field == 1) return close_entry(&p->env);
  if (field == 2) return close_entry(&p->unset);
  return field == 3 ? 0 : -1;
}
int64_t cxproc_pty(int64_t h, int64_t cols, int64_t rows) { (void)h; (void)cols; (void)rows; return -1; }
int64_t cxproc_pty_resize(int64_t h, int64_t cols, int64_t rows) { (void)h; (void)cols; (void)rows; return -1; }

/* ---- finding the program */

static int is_file(const wchar_t *w) { DWORD a = GetFileAttributesW(w); return a != INVALID_FILE_ATTRIBUTES && !(a & FILE_ATTRIBUTE_DIRECTORY); }
/* dir\name, then dir\name.exe, into out (32768 wide characters): 1 when one is a file. */
static int try_dir(const wchar_t *dir, size_t dir_n, const wchar_t *name, wchar_t *out) {
  if (!dir_n || dir_n + wcslen(name) + 6 >= 32768) return 0;
  memcpy(out, dir, dir_n * sizeof *out);
  size_t n = dir_n;
  if (out[n - 1] != L'\\' && out[n - 1] != L'/') out[n++] = L'\\';
  wcscpy(out + n, name);
  if (is_file(out)) return 1;
  wcscat(out, L".exe");
  return is_file(out);
}
/* The program for argv[0] (UTF-8), with path the PATH to search (the child's when the core
   gave it one, else this process's): 1 with its full path in out. */
static int resolve(const char *name, const wchar_t *path, wchar_t *out) {
  if (!cxwin_is_posix_root(name) && (strchr(name, '/') || strchr(name, '\\'))) {
    wchar_t *w = cxwin_wide(name);
    if (!w || wcslen(w) + 5 >= 32768) { free(w); return 0; }
    wcscpy(out, w); free(w);
    if (is_file(out)) return 1;
    wcscat(out, L".exe");
    return is_file(out);
  }
  wchar_t *leaf = cxwin_wide(cxwin_leaf(name));
  if (!leaf) return 0;
  int found = 0;
  for (const wchar_t *at = path; at && *at && !found;) {
    const wchar_t *end = wcschr(at, L';');
    size_t n = end ? (size_t)(end - at) : wcslen(at);
    if (n >= 2 && at[0] == L'"' && at[n - 1] == L'"') found = try_dir(at + 1, n - 2, leaf, out);
    else found = try_dir(at, n, leaf, out);
    at = end ? end + 1 : NULL;
  }
  /* Git for Windows: git.exe in cmd, sh.exe, bash.exe and the POSIX tools in usr\bin */
  static const wchar_t *const roots[] = {L"ProgramFiles", L"ProgramW6432", L"ProgramFiles(x86)", L"LOCALAPPDATA"};
  static const wchar_t *const subs[] = {L"\\Git\\cmd", L"\\Git\\usr\\bin", L"\\Git\\bin", L"\\Programs\\Git\\cmd", L"\\Programs\\Git\\usr\\bin"};
  for (int r = 0; r < 4 && !found; r++) {
    wchar_t base[1024];
    DWORD k = GetEnvironmentVariableW(roots[r], base, 1024);
    if (!k || k >= 900) continue;
    for (int s = 0; s < 5 && !found; s++) {
      wchar_t dir[1100];
      wcscpy(dir, base); wcscat(dir, subs[s]);
      found = try_dir(dir, wcslen(dir), leaf, out);
    }
  }
  if (!found && !strcmp(cxwin_leaf(name), "claude")) {
    wchar_t home[1024];
    DWORD k = GetEnvironmentVariableW(L"USERPROFILE", home, 1024);
    if (k && k < 900) { wcscat(home, L"\\.local\\bin"); found = try_dir(home, wcslen(home), leaf, out); }
  }
  free(leaf);
  return found;
}

/* ---- the child's environment: this process's, less unset and overridden names, plus env */

static int env_has(List *l, const char *entry) {
  for (size_t i = 0; i < l->count; i++) if (cxwin_env_cmp(l->items[i].data, entry) == 0) return 1;
  return 0;
}
static int env_by_name(const void *a, const void *b) { return cxwin_env_cmp(*(char *const *)a, *(char *const *)b); }
/* A UTF-16 environment block (free it), and the PATH it sets in *path (inside the block), or NULL. */
static wchar_t *env_block(Proc *p, const wchar_t **path) {
  wchar_t *mine = GetEnvironmentStringsW();
  if (!mine) return NULL;
  size_t count = 0, cap = 64;
  char **all = malloc(cap * sizeof *all);
  int ok = all != NULL;
  for (wchar_t *at = mine; ok && *at; at += wcslen(at) + 1) {
    char u[65536];
    if (cxwin_utf8_into(at, u, sizeof u) < 0) continue;
    /* "=C:=C:\x" entries (each drive's working folder) are kept as they are */
    if (u[0] != '=' && (env_has(&p->unset, u) || env_has(&p->env, u))) continue;
    if (count == cap) { char **g = realloc(all, (cap *= 2) * sizeof *g); if (!g) { ok = 0; break; } all = g; }
    all[count] = _strdup(u);
    ok = all[count++] != NULL;
  }
  FreeEnvironmentStringsW(mine);
  for (size_t i = 0; ok && i < p->env.count; i++) {
    const char *kv = p->env.items[i].data, *eq = strchr(kv, '=');
    if (!eq || eq == kv || env_has(&p->unset, kv)) continue;
    if (count == cap) { char **g = realloc(all, (cap *= 2) * sizeof *g); if (!g) { ok = 0; break; } all = g; }
    all[count] = _strdup(kv);
    ok = all[count++] != NULL;
  }
  wchar_t *block = NULL;
  if (ok) {
    qsort(all, count, sizeof *all, env_by_name);
    size_t total = 1;
    for (size_t i = 0; i < count; i++) total += (size_t)MultiByteToWideChar(CP_UTF8, 0, all[i], -1, NULL, 0);
    block = malloc(total * sizeof *block);
    size_t at = 0;
    *path = NULL;
    for (size_t i = 0; block && i < count; i++) {
      int n = MultiByteToWideChar(CP_UTF8, 0, all[i], -1, block + at, (int)(total - at));
      if (n <= 0) continue;
      if (cxwin_env_cmp(all[i], "PATH=") == 0) *path = block + at + 5;
      at += (size_t)n;
    }
    if (block) block[at] = 0;
  }
  for (size_t i = 0; all && i < count; i++) free(all[i]);
  free(all);
  return block;
}

/* ---- starting it */

static HANDLE null_handle(DWORD access) {
  SECURITY_ATTRIBUTES sa = {sizeof sa, NULL, TRUE};
  return CreateFileW(L"NUL", access, FILE_SHARE_READ | FILE_SHARE_WRITE, &sa, OPEN_EXISTING, 0, NULL);
}
/* An anonymous pipe whose child end inherits and whose own end does not. */
static int pipe_pair(HANDLE *ours, HANDLE *theirs, int child_reads) {
  SECURITY_ATTRIBUTES sa = {sizeof sa, NULL, TRUE};
  HANDLE r, w;
  if (!CreatePipe(&r, &w, &sa, 1 << 16)) return 0;
  *ours = child_reads ? w : r;
  *theirs = child_reads ? r : w;
  SetHandleInformation(*ours, HANDLE_FLAG_INHERIT, 0);
  return 1;
}

int64_t cxproc_spawn(int64_t h) {
  Proc *p = get(h);
  if (!p || p->started || p->args.count == 0) return 22;   /* EINVAL */
  const wchar_t *path_env = NULL;
  wchar_t *env = env_block(p, &path_env);
  if (!env) return 12;   /* ENOMEM */
  wchar_t *exe = malloc(32768 * sizeof *exe);
  wchar_t own_path[32768];
  if (!path_env && GetEnvironmentVariableW(L"PATH", own_path, 32768)) path_env = own_path;
  if (!exe || !resolve(p->args.items[0].data, path_env, exe)) { free(env); free(exe); return 2; }   /* ENOENT */
  char **argv = calloc(p->args.count + 1, sizeof *argv);
  char *line = malloc(32768 * 3);
  if (!argv || !line) { free(env); free(exe); free(argv); free(line); return 12; }
  for (size_t i = 0; i < p->args.count; i++) argv[i] = p->args.items[i].data;
  long n = cxwin_command_line(argv, p->args.count, line, 32768 * 3);
  free(argv);
  wchar_t *wline = n >= 0 ? cxwin_wide(line) : NULL;
  free(line);
  if (!wline || wcslen(wline) >= 32767) { free(env); free(exe); free(wline); return 7; }   /* E2BIG */
  wchar_t *cwd = p->cwd.count ? cxwin_wide(cstr(&p->cwd)) : NULL;

  HANDLE child_in = NULL, child_out = NULL, child_err = NULL;
  int ok = 1;
  if (p->detach) {
    child_in = null_handle(GENERIC_READ); child_out = null_handle(GENERIC_WRITE); child_err = null_handle(GENERIC_WRITE);
  } else {
    ok = pipe_pair(&p->in, &child_in, 1);
    if (ok && (p->quiet || p->outnull)) child_out = null_handle(GENERIC_WRITE);
    else if (ok) ok = pipe_pair(&p->out, &child_out, 0);
    if (ok && p->quiet) child_err = null_handle(GENERIC_WRITE);
    else if (ok && p->errtoout && !p->outnull) {
      ok = DuplicateHandle(GetCurrentProcess(), child_out, GetCurrentProcess(), &child_err, 0, TRUE, DUPLICATE_SAME_ACCESS);
    } else if (ok && p->errours) {
      HANDLE mine = GetStdHandle(STD_ERROR_HANDLE);
      if (!mine || mine == INVALID_HANDLE_VALUE || !DuplicateHandle(GetCurrentProcess(), mine, GetCurrentProcess(), &child_err, 0, TRUE, DUPLICATE_SAME_ACCESS)) child_err = null_handle(GENERIC_WRITE);
    } else if (ok) ok = pipe_pair(&p->err, &child_err, 0);
  }
  if (child_in == INVALID_HANDLE_VALUE || child_out == INVALID_HANDLE_VALUE || child_err == INVALID_HANDLE_VALUE) ok = 0;

  /* only the three handles reach the child, never another child's pipes */
  HANDLE list[3] = {child_in, child_out, child_err};
  SIZE_T attr_size = 0;
  InitializeProcThreadAttributeList(NULL, 1, 0, &attr_size);
  LPPROC_THREAD_ATTRIBUTE_LIST attrs = ok ? malloc(attr_size) : NULL;
  if (attrs && InitializeProcThreadAttributeList(attrs, 1, 0, &attr_size))
    ok = UpdateProcThreadAttribute(attrs, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, list, sizeof list, NULL, NULL);
  else ok = 0;

  STARTUPINFOEXW si; memset(&si, 0, sizeof si);
  si.StartupInfo.cb = sizeof si;
  si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
  si.StartupInfo.hStdInput = child_in; si.StartupInfo.hStdOutput = child_out; si.StartupInfo.hStdError = child_err;
  si.lpAttributeList = attrs;
  PROCESS_INFORMATION pi; memset(&pi, 0, sizeof pi);
  DWORD flags = CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT;
  flags |= p->detach ? (DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB) : (CREATE_NO_WINDOW | CREATE_SUSPENDED);
  DWORD error = 0;
  if (ok) {
    ok = CreateProcessW(exe, wline, NULL, NULL, TRUE, flags, env, cwd, &si.StartupInfo, &pi);
    if (!ok && p->detach && GetLastError() == ERROR_ACCESS_DENIED) {   /* this process's job forbids breaking away */
      flags &= ~(DWORD)CREATE_BREAKAWAY_FROM_JOB;
      ok = CreateProcessW(exe, wline, NULL, NULL, TRUE, flags, env, cwd, &si.StartupInfo, &pi);
    }
    if (!ok) error = GetLastError();
  }
  if (attrs) { DeleteProcThreadAttributeList(attrs); free(attrs); }
  shut(&child_in); shut(&child_out); shut(&child_err);
  free(env); free(exe); free(wline); free(cwd);
  if (!ok) {
    shut(&p->in); shut(&p->out); shut(&p->err);
    return error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND ? 2 : error == ERROR_DIRECTORY ? 20 : 13;
  }
  if (!p->detach) {
    p->job = CreateJobObjectW(NULL, NULL);
    if (p->job) {
      JOBOBJECT_EXTENDED_LIMIT_INFORMATION limit; memset(&limit, 0, sizeof limit);
      limit.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
      SetInformationJobObject(p->job, JobObjectExtendedLimitInformation, &limit, sizeof limit);
      if (!AssignProcessToJobObject(p->job, pi.hProcess)) shut(&p->job);
    }
    ResumeThread(pi.hThread);
  }
  CloseHandle(pi.hThread);
  p->started = 1; p->process = pi.hProcess; p->pid = pi.dwProcessId;
  return 0;
}

/* ---- talking to it */

static void keep_stderr(Proc *p, const char *d, DWORD n) {
  for (DWORD i = 0; i < n; i++) p->keep[(p->kept + i) % CXPROC_STDERR_KEEP] = d[i];
  p->kept += n;
}
static void drain_stderr(Proc *p) {
  while (p->err) {
    DWORD avail = 0, got = 0;
    char d[4096];
    if (!PeekNamedPipe(p->err, NULL, 0, NULL, &avail, NULL)) { shut(&p->err); return; }
    if (!avail) return;
    if (!ReadFile(p->err, d, avail < sizeof d ? avail : sizeof d, &got, NULL) || !got) { shut(&p->err); return; }
    keep_stderr(p, d, got);
  }
}
/* What stdout holds now, without waiting: bytes appended to buf (1), nothing (0), its end (-1). */
static int take_stdout(Proc *p) {
  DWORD avail = 0, got = 0;
  if (!PeekNamedPipe(p->out, NULL, 0, NULL, &avail, NULL)) return -1;
  if (!avail) return 0;
  char d[65536];
  if (!ReadFile(p->out, d, avail < sizeof d ? avail : sizeof d, &got, NULL)) return -1;
  for (DWORD i = 0; i < got; i++) if (put(&p->buf, d[i])) return -2;
  return got ? 1 : 0;
}
static int find_line(Proc *p) {
  char *nl = p->scan < p->buf.count ? memchr(p->buf.data + p->scan, '\n', p->buf.count - p->scan) : NULL;
  if (!nl) return 0;
  size_t end = (size_t)(nl - p->buf.data);
  p->line_start = 0; p->line_len = end;
  if (p->line_len && p->buf.data[p->line_len - 1] == '\r') p->line_len--;
  p->scan = end + 1;
  return 1;
}
/* A short nap while waiting on pipes: anonymous pipes cannot be waited on with a timeout. */
static void nap(int64_t left) { Sleep(left < 0 || left > 2 ? 2 : (DWORD)(left > 0 ? left : 0)); }

int64_t cxproc_read_line(int64_t h, int64_t timeout_ms) {
  Proc *p = get(h);
  if (!p || !p->started || !p->out) return CXPROC_ERROR;
  if (p->scan) {
    memmove(p->buf.data, p->buf.data + p->scan, p->buf.count - p->scan);
    p->buf.count -= p->scan; p->scan = 0;
  }
  int64_t deadline = timeout_ms < 0 ? -1 : cxproc_now_ms() + timeout_ms;
  int polled = 0;
  for (;;) {
    if (find_line(p)) {
      if (p->skipping) {
        p->skipping = 0;
        memmove(p->buf.data, p->buf.data + p->scan, p->buf.count - p->scan);
        p->buf.count -= p->scan; p->scan = 0;
        return CXPROC_TOO_LONG;
      }
      return (int64_t)p->line_len;
    }
    if (p->buf.count > CXPROC_MAX_LINE) { p->skipping = 1; p->buf.count = 0; p->scan = 0; }
    if (p->eof) {
      if (p->buf.count && !p->skipping) { p->line_start = 0; p->line_len = p->buf.count; p->scan = p->buf.count; return (int64_t)p->line_len; }
      return CXPROC_EOF;
    }
    drain_stderr(p);
    int got = take_stdout(p);
    if (got == -2) return CXPROC_ERROR;
    if (got < 0) { p->eof = 1; continue; }
    if (got > 0) continue;
    int64_t left = deadline < 0 ? -1 : deadline - cxproc_now_ms();
    if (deadline >= 0 && left <= 0 && polled) return CXPROC_TIMEOUT;
    polled = 1;
    if (deadline >= 0 && left <= 0) return CXPROC_TIMEOUT;
    nap(left);
  }
}
int64_t cxproc_line_byte(int64_t h, int64_t i) {
  Proc *p = get(h);
  if (!p || i < 0 || (size_t)i >= p->line_len) return -1;
  return (unsigned char)p->buf.data[p->line_start + (size_t)i];
}
int64_t cxproc_read_bytes(int64_t h, int64_t timeout_ms, coax_v_u8 out) {
  Proc *p = get(h);
  if (!p || !p->started || !p->out || out.count <= 0) return CXPROC_ERROR;
  if (p->scan) { memmove(p->buf.data, p->buf.data + p->scan, p->buf.count - p->scan); p->buf.count -= p->scan; p->scan = 0; }
  int64_t deadline = timeout_ms < 0 ? -1 : cxproc_now_ms() + timeout_ms;
  for (;;) {
    if (p->buf.count) {
      size_t n = p->buf.count < (size_t)out.count ? p->buf.count : (size_t)out.count;
      memcpy(out.data, p->buf.data, n);
      memmove(p->buf.data, p->buf.data + n, p->buf.count - n); p->buf.count -= n;
      return (int64_t)n;
    }
    if (p->eof) return CXPROC_EOF;
    drain_stderr(p);
    int got = take_stdout(p);
    if (got == -2) return CXPROC_ERROR;
    if (got < 0) { p->eof = 1; continue; }
    if (got > 0) continue;
    int64_t left = deadline < 0 ? -1 : deadline - cxproc_now_ms();
    if (deadline >= 0 && left <= 0) return 0;
    nap(left);
  }
}

int64_t cxproc_write(int64_t h, int64_t byte) {
  Proc *p = get(h);
  if (!p || !p->in || byte < 0 || byte > 255) return -1;
  return put(&p->wbuf, (char)byte);
}
int64_t cxproc_flush(int64_t h) {
  Proc *p = get(h);
  if (!p || !p->in) return -1;
  size_t done = 0;
  while (done < p->wbuf.count) {
    DWORD n = 0;
    if (!WriteFile(p->in, p->wbuf.data + done, (DWORD)(p->wbuf.count - done), &n, NULL)) { p->wbuf.count = 0; return -1; }
    done += n;
  }
  p->wbuf.count = 0;
  return 0;
}
/* As much as the pipe takes now (PIPE_NOWAIT for the one call): the count, -1 when broken. */
static int64_t write_now(Proc *p, const char *d, size_t n) {
  DWORD mode = PIPE_READMODE_BYTE | PIPE_NOWAIT, back = PIPE_READMODE_BYTE | PIPE_WAIT, wrote = 0;
  SetNamedPipeHandleState(p->in, &mode, NULL, NULL);
  BOOL ok = WriteFile(p->in, d, (DWORD)n, &wrote, NULL);
  SetNamedPipeHandleState(p->in, &back, NULL, NULL);
  return ok ? (int64_t)wrote : -1;
}
int64_t cxproc_flush_timeout(int64_t h, int64_t timeout_ms) {
  Proc *p = get(h);
  if (!p || !p->started || !p->in || timeout_ms < 0) return CXPROC_ERROR;
  int64_t deadline = cxproc_now_ms() + timeout_ms, result = 0;
  size_t done = 0;
  int attempted = 0;
  while (done < p->wbuf.count) {
    if (attempted && cxproc_now_ms() >= deadline) { result = CXPROC_TIMEOUT; break; }
    attempted = 1;
    size_t count = p->wbuf.count - done;
    if (count > 65536) count = 65536;
    int64_t n = write_now(p, p->wbuf.data + done, count);
    if (n < 0) { result = CXPROC_ERROR; break; }
    if (n > 0) { done += (size_t)n; continue; }
    drain_stderr(p);
    nap(deadline - cxproc_now_ms());
  }
  if (done) {
    size_t remain = p->wbuf.count - done;
    memmove(p->wbuf.data, p->wbuf.data + done, remain);
    volatile unsigned char *retired = (volatile unsigned char *)p->wbuf.data;
    for (size_t i = remain; i < p->wbuf.count; i++) retired[i] = 0;
    p->wbuf.count = remain;
  }
  return result;
}
int64_t cxproc_write_some(int64_t h, coax_v_u8 s) {
  Proc *p = get(h);
  if (!p || !p->in) return -1;
  return write_now(p, (const char *)s.data, (size_t)s.count);
}
int64_t cxproc_close_stdin(int64_t h) {
  Proc *p = get(h);
  if (!p) return -1;
  shut(&p->in);
  return 0;
}

/* ---- ending it */

static void ended(Proc *p) {
  if (p->reaped) return;
  DWORD code = 0;
  GetExitCodeProcess(p->process, &code);
  p->status = code; p->reaped = 1;
  drain_stderr(p);
}
int64_t cxproc_kill(int64_t h, int64_t sig) {
  Proc *p = get(h);
  if (!p || !p->started) return -1;
  if (p->reaped && !p->job) return 0;
  UINT code = (UINT)(128 + (sig > 0 && sig < 64 ? sig : 9));
  if (p->job) return TerminateJobObject(p->job, code) ? 0 : -1;
  return TerminateProcess(p->process, code) || WaitForSingleObject(p->process, 0) == WAIT_OBJECT_0 ? 0 : -1;
}
int64_t cxproc_wait(int64_t h, int64_t timeout_ms) {
  Proc *p = get(h);
  if (!p || !p->started) return -1;
  int64_t deadline = timeout_ms < 0 ? -1 : cxproc_now_ms() + timeout_ms;
  while (!p->reaped) {
    DWORD r = WaitForSingleObject(p->process, 0);
    if (r == WAIT_OBJECT_0) { ended(p); break; }
    if (r == WAIT_FAILED) return -1;
    drain_stderr(p);
    if (deadline >= 0 && cxproc_now_ms() >= deadline) return CXPROC_RUNNING;
    WaitForSingleObject(p->process, 5);
  }
  /* a crash's NTSTATUS (0xC0000005 and the like) does not fit a POSIX status */
  return p->status <= 255 ? (int64_t)p->status : 255;
}
int64_t cxproc_pid(int64_t h) { Proc *p = get(h); return p && p->started ? (int64_t)p->pid : -1; }

static int64_t pid_alive(int64_t pid) {
  if (pid <= 0) return 0;
  HANDLE ph = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, (DWORD)pid);
  if (!ph) return GetLastError() == ERROR_ACCESS_DENIED;
  int alive = WaitForSingleObject(ph, 0) == WAIT_TIMEOUT;
  CloseHandle(ph);
  return alive;
}
static int64_t pid_end(int64_t pid, int64_t sig) {
  if (pid <= 0) return -1;
  HANDLE ph = OpenProcess(PROCESS_TERMINATE, FALSE, (DWORD)pid);
  if (!ph) return -1;
  int ok = TerminateProcess(ph, (UINT)(128 + sig));
  CloseHandle(ph);
  return ok ? 0 : -1;
}
int64_t cxproc_group_alive(int64_t pgid) { return pid_alive(pgid); }
int64_t cxproc_group_signal(int64_t pgid, int64_t sig) { return sig == 0 ? (pid_alive(pgid) ? 0 : -1) : pid_end(pgid, sig); }
int64_t cxproc_pid_alive(int64_t pid) { return pid_alive(pid); }
int64_t cxproc_pid_signal(int64_t pid, int64_t sig) { return sig == 0 ? (pid_alive(pid) ? 0 : -1) : pid_end(pid, sig); }

int64_t cxproc_stderr_count(int64_t h) {
  Proc *p = get(h);
  if (!p) return 0;
  return p->kept < CXPROC_STDERR_KEEP ? p->kept : CXPROC_STDERR_KEEP;
}
int64_t cxproc_stderr_byte(int64_t h, int64_t i) {
  Proc *p = get(h);
  if (!p) return -1;
  int64_t n = cxproc_stderr_count(h);
  if (i < 0 || i >= n) return -1;
  return (unsigned char)p->keep[(p->kept - n + i) % CXPROC_STDERR_KEEP];
}
int64_t cxproc_release(int64_t h) {
  Proc *p = get(h);
  if (!p) return -1;
  if (p->started && !p->detach) {
    if (p->job) TerminateJobObject(p->job, 137);
    else if (!p->reaped) TerminateProcess(p->process, 137);
    if (!p->reaped) { WaitForSingleObject(p->process, 2000); ended(p); }
  }
  shut(&p->in); shut(&p->out); shut(&p->err); shut(&p->job); shut(&p->process);
  drop_list(&p->args); drop_list(&p->env); drop_list(&p->unset);
  drop(&p->cwd); drop(&p->buf); drop(&p->wbuf);
  memset(p, 0, sizeof *p);
  return 0;
}

/* ---- this process's environment */

static Bytes env_name;
static char env_value[32768 * 3];
static int64_t env_n = -1;
int64_t cxproc_getenv_push(int64_t byte) {
  if (byte <= 0 || byte > 255) return -1;
  return put(&env_name, (char)byte);
}
int64_t cxproc_getenv(void) {
  char *name = cstr(&env_name);
  env_name.count = 0; env_n = -1;
  wchar_t *w = name ? cxwin_wide(name) : NULL;
  if (!w) return -1;
  wchar_t value[32768];
  DWORD n = GetEnvironmentVariableW(w, value, 32768);
  free(w);
  if (n == 0 && GetLastError() == ERROR_ENVVAR_NOT_FOUND) return -1;
  if (n >= 32768) return -1;
  value[n] = 0;
  env_n = cxwin_utf8_into(value, env_value, sizeof env_value);
  return env_n;
}
int64_t cxproc_getenv_byte(int64_t i) {
  if (env_n < 0 || i < 0 || i >= env_n) return -1;
  return (unsigned char)env_value[i];
}

/* As the shell's exec, which Windows lacks: the program runs with this process's stdio, and this
   process ends with its status. Returns only when it could not start (the error). */
int64_t cxproc_exec(int64_t h) {
  Proc *p = get(h);
  if (!p || p->started) return 22;
  p->errours = 1;
  int64_t e = cxproc_spawn(h);
  if (e) return e;
  fflush(stdout); fflush(stderr);
  WaitForSingleObject(p->process, INFINITE);
  ended(p);
  ExitProcess(p->status);
}
int64_t cxproc_cmdline(int64_t pid, coax_v_u8 out) { (void)pid; (void)out; return -1; }

int64_t cxproc_platform(void) {
#if defined(__aarch64__)
  return 6;
#else
  return 5;
#endif
}
