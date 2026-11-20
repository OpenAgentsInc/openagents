/* General file effects on Windows (GitHub #13782): the cxfs_* and cxjournal_* half of
   store-file-native.h over Win32 calls, for coax/lib/sys/fs.coax. Paths and every decision are
   Coax; these only touch the file system, as store-file-native.c does on POSIX (which stays the
   POSIX build's). Paths arrive as UTF-8 with forward or back slashes; a path given back
   (cxfs_temp, cxfs_cwd, cxfs_realpath) uses forward slashes. "Private" means what Windows gives
   a file under the user's profile: only that user, SYSTEM and administrators reach it; there
   are no POSIX modes. The relay store's append-only file (cxstore_*) is not provided here. */
#include "win32-native.h"
#include "store-file-native.h"
#include <stdio.h>

static char fs_path[CXFS_PATH_MAX + 1];
static size_t fs_path_n;
static int fs_path_bad;
static unsigned char *fs_data;
static int64_t fs_data_n;
static unsigned char *fs_out;
static size_t fs_out_n, fs_out_cap;
static char fs_name[CXFS_PATH_MAX * 3 + 1];
static char **fs_entries;
static int64_t fs_entries_n;

int64_t cxfs_path_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || fs_path_n >= CXFS_PATH_MAX) { fs_path_bad = 1; return -1; }
  fs_path[fs_path_n++] = (char)byte;
  return 0;
}
/* The pushed path as UTF-16 (free it), cleared for the next call; NULL when refused or empty. */
static wchar_t *take_path(void) {
  int bad = fs_path_bad || fs_path_n == 0;
  fs_path[fs_path_n] = 0;
  fs_path_n = 0; fs_path_bad = 0;
  return bad ? NULL : cxwin_wide(fs_path);
}
static HANDLE open_read(const wchar_t *p) {
  return CreateFileW(p, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
}
static int read_all(HANDLE f, unsigned char *d, int64_t want, int64_t *got) {
  *got = 0;
  while (*got < want) {
    DWORD n = 0, ask = want - *got > (1 << 30) ? (1 << 30) : (DWORD)(want - *got);
    if (!ReadFile(f, d + *got, ask, &n, NULL)) return 0;
    if (!n) break;
    *got += n;
  }
  return 1;
}

int64_t cxfs_read(void) {
  wchar_t *p = take_path();
  free(fs_data); fs_data = NULL; fs_data_n = 0;
  if (!p) return -1;
  HANDLE f = open_read(p); free(p);
  if (f == INVALID_HANDLE_VALUE) return -1;
  LARGE_INTEGER size;
  if (GetFileType(f) != FILE_TYPE_DISK || !GetFileSizeEx(f, &size) || size.QuadPart < 0) { CloseHandle(f); return -1; }
  fs_data = malloc(size.QuadPart > 0 ? (size_t)size.QuadPart : 1);
  int64_t got = 0;
  if (!fs_data || !read_all(f, fs_data, size.QuadPart, &got)) { CloseHandle(f); free(fs_data); fs_data = NULL; return -1; }
  CloseHandle(f);
  fs_data_n = got;
  return got;
}
int64_t cxfs_byte(int64_t i) { return i >= 0 && i < fs_data_n ? fs_data[i] : -1; }
int64_t cxfs_read_at(int64_t offset, int64_t count) {
  wchar_t *p = take_path();
  free(fs_data); fs_data = NULL; fs_data_n = 0;
  if (!p || offset < 0 || count < 0) { free(p); return -1; }
  HANDLE f = open_read(p); free(p);
  if (f == INVALID_HANDLE_VALUE) return -1;
  LARGE_INTEGER size, at;
  if (!GetFileSizeEx(f, &size)) { CloseHandle(f); return -1; }
  int64_t available = offset >= size.QuadPart ? 0 : size.QuadPart - offset;
  int64_t wanted = count < available ? count : available;
  at.QuadPart = offset;
  fs_data = malloc(wanted > 0 ? (size_t)wanted : 1);
  int64_t got = 0;
  if (!fs_data || !SetFilePointerEx(f, at, NULL, FILE_BEGIN) || !read_all(f, fs_data, wanted, &got)) { CloseHandle(f); free(fs_data); fs_data = NULL; return -1; }
  CloseHandle(f);
  fs_data_n = got;
  return got;
}
int64_t cxfs_put(int64_t byte) {
  if (byte < 0 || byte > 255) return -1;
  if (fs_out_n == fs_out_cap) {
    size_t cap = fs_out_cap ? fs_out_cap * 2 : 65536;
    unsigned char *d = realloc(fs_out, cap);
    if (!d) return -1;
    fs_out = d; fs_out_cap = cap;
  }
  fs_out[fs_out_n++] = (unsigned char)byte;
  return 0;
}
static int write_all(HANDLE f, const unsigned char *d, size_t n) {
  size_t done = 0;
  while (done < n) {
    DWORD w = 0, ask = n - done > (1u << 30) ? (1u << 30) : (DWORD)(n - done);
    if (!WriteFile(f, d + done, ask, &w, NULL) || !w) return 0;
    done += w;
  }
  return 1;
}
int64_t cxfs_truncate(int64_t size) {
  wchar_t *p = take_path();
  if (!p || size < 0) { free(p); return -1; }
  HANDLE f = CreateFileW(p, GENERIC_WRITE, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT, NULL);
  free(p);
  if (f == INVALID_HANDLE_VALUE) return -1;
  LARGE_INTEGER now, to; to.QuadPart = size;
  int ok = GetFileSizeEx(f, &now) && size <= now.QuadPart && SetFilePointerEx(f, to, NULL, FILE_BEGIN) && SetEndOfFile(f) && FlushFileBuffers(f);
  CloseHandle(f);
  return ok ? 0 : -1;
}
/* mode 0 replaces, 1 appends; 2 and 3 the same, flushed to storage; 4 replaces through a
   temporary file renamed over it. */
int64_t cxfs_write(int64_t mode) {
  wchar_t *p = take_path();
  size_t n = fs_out_n;
  fs_out_n = 0;
  if (!p) return -1;
  if (mode == 4) {
    size_t k = wcslen(p);
    wchar_t *temp = malloc((k + 32) * sizeof *temp);
    if (!temp) { free(p); return -1; }
    swprintf(temp, k + 32, L"%ls.tmp.%lu", p, (unsigned long)GetCurrentProcessId());
    HANDLE f = CreateFileW(temp, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    int ok = f != INVALID_HANDLE_VALUE && write_all(f, fs_out, n) && FlushFileBuffers(f);
    if (f != INVALID_HANDLE_VALUE) CloseHandle(f);
    if (ok) ok = MoveFileExW(temp, p, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
    if (!ok) DeleteFileW(temp);
    free(temp); free(p);
    return ok ? 0 : -1;
  }
  int append = mode == 1 || mode == 3, durable = mode >= 2;
  HANDLE f = CreateFileW(p, append ? FILE_APPEND_DATA : GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_DELETE, NULL, append ? OPEN_ALWAYS : CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
  free(p);
  if (f == INVALID_HANDLE_VALUE) return -1;
  int ok = write_all(f, fs_out, n) && (!durable || FlushFileBuffers(f));
  CloseHandle(f);
  return ok ? 0 : -1;
}
static wchar_t *fs_held;
int64_t cxfs_hold(void) {
  free(fs_held);
  fs_held = take_path();
  return fs_held ? 0 : -1;
}
int64_t cxfs_rename(void) {
  wchar_t *p = take_path();
  int ok = fs_held && p && MoveFileExW(fs_held, p, MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH);
  free(fs_held); fs_held = NULL; free(p);
  return ok ? 0 : -1;
}
static int attributes(const wchar_t *p, WIN32_FILE_ATTRIBUTE_DATA *a) { return GetFileAttributesExW(p, GetFileExInfoStandard, a); }
int64_t cxfs_mtime_ns(void) {
  wchar_t *p = take_path();
  WIN32_FILE_ATTRIBUTE_DATA a;
  int ok = p && attributes(p, &a);
  free(p);
  if (!ok) return -1;
  uint64_t t = ((uint64_t)a.ftLastWriteTime.dwHighDateTime << 32) | a.ftLastWriteTime.dwLowDateTime;
  return (int64_t)((t - 116444736000000000ull) * 100);
}
int64_t cxfs_kind(void) {
  wchar_t *p = take_path();
  if (!p) return -1;
  WIN32_FILE_ATTRIBUTE_DATA a;
  int ok = attributes(p, &a);
  DWORD e = ok ? 0 : GetLastError();
  free(p);
  if (!ok) return e == ERROR_FILE_NOT_FOUND || e == ERROR_PATH_NOT_FOUND || e == ERROR_INVALID_NAME || e == ERROR_BAD_NETPATH || e == ERROR_INVALID_DRIVE ? 0 : -1;
  return (a.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) ? 2 : (a.dwFileAttributes & FILE_ATTRIBUTE_DEVICE) ? 3 : 1;
}
int64_t cxfs_size(void) {
  wchar_t *p = take_path();
  WIN32_FILE_ATTRIBUTE_DATA a;
  int ok = p && attributes(p, &a) && !(a.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY);
  free(p);
  return ok ? (int64_t)(((uint64_t)a.nFileSizeHigh << 32) | a.nFileSizeLow) : -1;
}
static HANDLE fs_lock = INVALID_HANDLE_VALUE;
int64_t cxfs_try_lock(void) {
  wchar_t *p = take_path();
  if (!p || fs_lock != INVALID_HANDLE_VALUE) { free(p); return -1; }
  HANDLE f = CreateFileW(p, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
  free(p);
  if (f == INVALID_HANDLE_VALUE) return -1;
  OVERLAPPED o; memset(&o, 0, sizeof o);
  if (!LockFileEx(f, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, 1, 0, &o)) { CloseHandle(f); return -1; }
  fs_lock = f;
  return 0;
}
int64_t cxfs_unlock(void) {
  if (fs_lock == INVALID_HANDLE_VALUE) return -1;
  CloseHandle(fs_lock); fs_lock = INVALID_HANDLE_VALUE;
  return 0;
}
/* The folder and its parents. */
static int make_dirs(wchar_t *d) {
  size_t n = wcslen(d);
  for (size_t i = 1; i < n; i++) {
    if (d[i] != L'/' && d[i] != L'\\') continue;
    if (i == 2 && d[1] == L':') continue;   /* "C:\" is the drive */
    wchar_t c = d[i]; d[i] = 0;
    if (!CreateDirectoryW(d, NULL) && GetLastError() != ERROR_ALREADY_EXISTS && GetLastError() != ERROR_ACCESS_DENIED) { d[i] = c; return 0; }
    d[i] = c;
  }
  if (!CreateDirectoryW(d, NULL) && GetLastError() != ERROR_ALREADY_EXISTS) return 0;
  DWORD a = GetFileAttributesW(d);
  return a != INVALID_FILE_ATTRIBUTES && (a & FILE_ATTRIBUTE_DIRECTORY);
}
int64_t cxfs_private_directory(const char *path) {
  wchar_t *w = cxwin_wide(path);
  int ok = w && make_dirs(w);
  free(w);
  return ok ? 0 : -1;
}
int64_t cxfs_mkdir_private(void) { wchar_t *p = take_path(); int ok = p && make_dirs(p); free(p); return ok ? 0 : -1; }
int64_t cxfs_mkdir(void) { wchar_t *p = take_path(); int ok = p && make_dirs(p); free(p); return ok ? 0 : -1; }

int64_t cxfs_temp(void) {
  wchar_t *prefix = take_path();
  if (!prefix || wcschr(prefix, L'/') || wcschr(prefix, L'\\')) { free(prefix); return -1; }
  wchar_t tmp[MAX_PATH + 1], made[MAX_PATH * 2];
  DWORD k = GetTempPathW(MAX_PATH + 1, tmp);
  if (!k || k > MAX_PATH) { free(prefix); return -1; }
  int ok = 0;
  for (int attempt = 0; attempt < 64 && !ok; attempt++) {
    unsigned char r[6];
    if (cxwin_random(r, sizeof r) != 0) break;
    swprintf(made, MAX_PATH * 2, L"%ls%ls%02x%02x%02x%02x%02x%02x", tmp, prefix, r[0], r[1], r[2], r[3], r[4], r[5]);
    ok = CreateDirectoryW(made, NULL);
  }
  free(prefix);
  if (!ok || cxwin_utf8_into(made, fs_name, sizeof fs_name) < 0) { fs_name[0] = 0; return -1; }
  cxwin_forward(fs_name);
  return (int64_t)strlen(fs_name);
}
int64_t cxfs_name_byte(int64_t i) { return i >= 0 && (size_t)i < strlen(fs_name) ? (unsigned char)fs_name[i] : -1; }

/* A file, or a folder and everything in it. Read-only files (Git's objects) are made writable
   first; a link is removed, never what it points at. */
static int remove_tree(const wchar_t *p) {
  DWORD a = GetFileAttributesW(p);
  if (a == INVALID_FILE_ATTRIBUTES) return GetLastError() == ERROR_FILE_NOT_FOUND || GetLastError() == ERROR_PATH_NOT_FOUND;
  if (a & FILE_ATTRIBUTE_READONLY) SetFileAttributesW(p, a & ~(DWORD)FILE_ATTRIBUTE_READONLY);
  if (!(a & FILE_ATTRIBUTE_DIRECTORY)) return DeleteFileW(p) != 0;
  if (!(a & FILE_ATTRIBUTE_REPARSE_POINT)) {
    size_t n = wcslen(p);
    wchar_t *pattern = malloc((n + 3) * sizeof *pattern);
    if (!pattern) return 0;
    wcscpy(pattern, p); wcscat(pattern, L"\\*");
    WIN32_FIND_DATAW e;
    HANDLE find = FindFirstFileW(pattern, &e);
    free(pattern);
    if (find != INVALID_HANDLE_VALUE) {
      do {
        if (!wcscmp(e.cFileName, L".") || !wcscmp(e.cFileName, L"..")) continue;
        size_t k = n + wcslen(e.cFileName) + 2;
        wchar_t *child = malloc(k * sizeof *child);
        if (!child) { FindClose(find); return 0; }
        swprintf(child, k, L"%ls\\%ls", p, e.cFileName);
        int ok = remove_tree(child);
        free(child);
        if (!ok) { FindClose(find); return 0; }
      } while (FindNextFileW(find, &e));
      FindClose(find);
    }
  }
  return RemoveDirectoryW(p) != 0;
}
int64_t cxfs_remove(void) { wchar_t *p = take_path(); int ok = p && remove_tree(p); free(p); return ok ? 0 : -1; }

static int by_bytes(const void *a, const void *b) { return strcmp(*(char *const *)a, *(char *const *)b); }
int64_t cxfs_list(void) {
  wchar_t *p = take_path();
  for (int64_t i = 0; i < fs_entries_n; i++) free(fs_entries[i]);
  free(fs_entries); fs_entries = NULL; fs_entries_n = 0;
  if (!p) return -1;
  size_t n = wcslen(p);
  wchar_t *pattern = malloc((n + 3) * sizeof *pattern);
  if (!pattern) { free(p); return -1; }
  wcscpy(pattern, p); wcscat(pattern, L"\\*");
  free(p);
  WIN32_FIND_DATAW e;
  HANDLE find = FindFirstFileW(pattern, &e);
  free(pattern);
  if (find == INVALID_HANDLE_VALUE) return GetLastError() == ERROR_FILE_NOT_FOUND ? 0 : -1;
  size_t cap = 0;
  do {
    if (!wcscmp(e.cFileName, L".") || !wcscmp(e.cFileName, L"..")) continue;
    char u[MAX_PATH * 4];
    if (cxwin_utf8_into(e.cFileName, u, sizeof u) < 0) continue;
    if ((size_t)fs_entries_n == cap) {
      cap = cap ? cap * 2 : 64;
      char **g = realloc(fs_entries, cap * sizeof *g);
      if (!g) { FindClose(find); return -1; }
      fs_entries = g;
    }
    fs_entries[fs_entries_n] = _strdup(u);
    if (!fs_entries[fs_entries_n]) { FindClose(find); return -1; }
    fs_entries_n++;
  } while (FindNextFileW(find, &e));
  FindClose(find);
  if (fs_entries_n > 1) qsort(fs_entries, (size_t)fs_entries_n, sizeof *fs_entries, by_bytes);
  return fs_entries_n;
}
int64_t cxfs_entry_len(int64_t i) { return i >= 0 && i < fs_entries_n ? (int64_t)strlen(fs_entries[i]) : -1; }
int64_t cxfs_entry_byte(int64_t i, int64_t j) {
  if (i < 0 || i >= fs_entries_n || j < 0 || (size_t)j >= strlen(fs_entries[i])) return -1;
  return (unsigned char)fs_entries[i][j];
}
int64_t cxfs_cwd(void) {
  wchar_t w[32768];
  DWORD n = GetCurrentDirectoryW(32768, w);
  if (!n || n >= 32768 || cxwin_utf8_into(w, fs_name, sizeof fs_name) < 0) { fs_name[0] = 0; return -1; }
  cxwin_forward(fs_name);
  return (int64_t)strlen(fs_name);
}
/* The path with every link resolved (the final path of the file it opens), or -1 when it does
   not exist. */
int64_t cxfs_realpath(void) {
  wchar_t *p = take_path();
  if (!p) return -1;
  HANDLE f = CreateFileW(p, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
  free(p);
  if (f == INVALID_HANDLE_VALUE) return -1;
  wchar_t w[32768];
  DWORD n = GetFinalPathNameByHandleW(f, w, 32768, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
  CloseHandle(f);
  if (!n || n >= 32768 || cxwin_utf8_into(w, fs_name, sizeof fs_name) < 0) return -1;
  cxwin_forward(fs_name);
  size_t k = strlen(fs_name);
  if (k > CXFS_PATH_MAX) return -1;
  return (int64_t)k;
}

/* ---- the private, single-writer journal */

static char journal_path[4097];
static size_t journal_path_n;
static int journal_path_bad;
static HANDLE journal = INVALID_HANDLE_VALUE;
static int journal_broken;
static unsigned char journal_pending[32857];
static size_t journal_pending_n;
static void journal_discard(void) {
  volatile unsigned char *p = journal_pending;
  for (size_t i = 0; i < journal_pending_n; ++i) p[i] = 0;
  journal_pending_n = 0;
}
int64_t cxjournal_path_push(int64_t byte) {
  if (journal != INVALID_HANDLE_VALUE) return -1;
  if (byte < 1 || byte > 255 || journal_path_n >= sizeof journal_path - 1) { journal_path_bad = 1; return -1; }
  journal_path[journal_path_n++] = (char)byte;
  return 0;
}
int64_t cxjournal_open(void) {
  if (journal != INVALID_HANDLE_VALUE) return -1;
  int valid = !journal_path_bad && journal_path_n > 0;
  journal_path[journal_path_n] = 0;
  journal_path_n = 0; journal_path_bad = 0;
  if (!valid) return -1;
  wchar_t *w = cxwin_wide(journal_path);
  if (!w) return -1;
  HANDLE f = CreateFileW(w, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, NULL);
  free(w);
  if (f == INVALID_HANDLE_VALUE) return -1;
  if (!FlushFileBuffers(f)) { CloseHandle(f); return -1; }
  journal = f; journal_broken = 0; journal_discard();
  return 0;
}
int64_t cxjournal_put(int64_t byte) {
  if (journal == INVALID_HANDLE_VALUE || journal_broken) return -1;
  if (byte < 0 || byte > 255 || journal_pending_n >= sizeof journal_pending) { journal_broken = 1; return -1; }
  journal_pending[journal_pending_n++] = (unsigned char)byte;
  return 0;
}
int64_t cxjournal_commit(void) {
  if (journal == INVALID_HANDLE_VALUE || journal_broken) { journal_discard(); return -1; }
  LARGE_INTEGER zero, before; zero.QuadPart = 0;
  if (SetFilePointerEx(journal, zero, &before, FILE_END)) {
    if (write_all(journal, journal_pending, journal_pending_n) && FlushFileBuffers(journal)) { journal_discard(); return 0; }
    if (SetFilePointerEx(journal, before, NULL, FILE_BEGIN) && SetEndOfFile(journal)) FlushFileBuffers(journal);
  }
  journal_broken = 1; journal_discard();
  return -1;
}
int64_t cxjournal_close(void) {
  int ok = journal == INVALID_HANDLE_VALUE || CloseHandle(journal);
  journal = INVALID_HANDLE_VALUE; journal_broken = 0; journal_discard();
  return ok ? 0 : -1;
}
