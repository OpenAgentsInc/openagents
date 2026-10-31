/* The verified Nostr store's append-only file: effects only (see
   store-file-native.h). One writer per file, enforced with flock. */
#define _DEFAULT_SOURCE
#define _XOPEN_SOURCE 700
#if defined(__APPLE__) && !defined(_DARWIN_C_SOURCE)
#define _DARWIN_C_SOURCE
#endif
#include "store-file-native.h"
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <dirent.h>
#include <ftw.h>
#include <limits.h>
#include <stdio.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <unistd.h>

#define CXSTORE_PENDING (4 + 262144)
static char path_buf[4096];
static int fd = -1;
static unsigned char *loaded;
static int64_t loaded_n;
static unsigned char pending[CXSTORE_PENDING];
static int64_t pending_n;
static int broken;

int cxstore_set_path(const char *path) {
  size_t n = strlen(path);
  if (n == 0 || n >= sizeof path_buf) return -1;
  memcpy(path_buf, path, n + 1);
  return 0;
}

/* fsyncs the directory that holds path_buf: 0, or -1. */
static int sync_parent(void) {
  char dir[sizeof path_buf];
  memcpy(dir, path_buf, sizeof dir);
  char *slash = strrchr(dir, '/');
  if (!slash) { dir[0] = '.'; dir[1] = 0; }
  else if (slash == dir) dir[1] = 0;
  else *slash = 0;
  int d = open(dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (d < 0) return -1;
  int r = fsync(d);
  close(d);
  return r;
}

int64_t cxstore_load(void) {
  if (!path_buf[0]) return -1;
  if (fd < 0) {
    int created = 1;
    fd = open(path_buf, O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (fd < 0 && errno == EEXIST) { created = 0; fd = open(path_buf, O_RDWR | O_CLOEXEC); }
    if (fd < 0) return -1;
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); fd = -1; return -1; }
    /* A new file's name is durable only once its directory is synced. */
    if (created && sync_parent() != 0) { close(fd); fd = -1; return -1; }
  }
  struct stat st;
  if (fstat(fd, &st) != 0 || st.st_size < 0) return -1;
  free(loaded); loaded = NULL; loaded_n = 0;
  if (st.st_size > 0) {
    loaded = malloc((size_t)st.st_size);
    if (!loaded) return -1;
    int64_t got = 0;
    while (got < st.st_size) {
      ssize_t r = pread(fd, loaded + got, (size_t)(st.st_size - got), got);
      if (r < 0 && errno == EINTR) continue;
      if (r <= 0) { free(loaded); loaded = NULL; return -1; }
      got += r;
    }
  }
  loaded_n = st.st_size;
  return loaded_n;
}

int64_t cxstore_byte(int64_t i) {
  if (i < 0 || i >= loaded_n) return -1;
  return loaded[i];
}

int64_t cxstore_put(int64_t b) {
  if (b < 0 || b > 255 || pending_n >= CXSTORE_PENDING) return -1;
  pending[pending_n++] = (unsigned char)b;
  return 0;
}

int64_t cxstore_commit(void) {
  int64_t n = pending_n;
  pending_n = 0;
  if (fd < 0 || broken) return -1;
  off_t end = lseek(fd, 0, SEEK_END);
  if (end < 0) return -1;
  int64_t done = 0;
  while (done < n) {
    ssize_t w = pwrite(fd, pending + done, (size_t)(n - done), end + done);
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) break;
    done += w;
  }
  if (done == n && fsync(fd) == 0) return 0;
  /* Cut the failed append back. If even that fails, stop writing: a later
     append after a partial frame would put damage in the middle of the file,
     which replay refuses (store_recover.coax). */
  if (ftruncate(fd, end) != 0 || fsync(fd) != 0) broken = 1;
  return -1;
}

int64_t cxstore_truncate(int64_t n) {
  if (fd < 0 || n < 0) return -1;
  if (ftruncate(fd, (off_t)n) != 0 || fsync(fd) != 0) return -1;
  if (n < loaded_n) loaded_n = n;
  return 0;
}

/* ---- general file effects for Coax tools (store-file-native.h) ---- */

static char fs_path[CXFS_PATH_MAX + 1];
static size_t fs_path_n;
static int fs_path_bad;
static unsigned char *fs_data;
static int64_t fs_data_n;
static unsigned char *fs_out;
static size_t fs_out_n, fs_out_cap;
static char fs_name[CXFS_PATH_MAX + 1];
static char **fs_entries;
static int64_t fs_entries_n;

int64_t cxfs_path_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || fs_path_n >= CXFS_PATH_MAX) { fs_path_bad = 1; return -1; }
  fs_path[fs_path_n++] = (char)byte;
  return 0;
}
/* The pushed path, cleared for the next call; NULL when it was refused or empty. */
static const char *take_path(void) {
  int bad = fs_path_bad || fs_path_n == 0;
  fs_path[fs_path_n] = 0;
  fs_path_n = 0; fs_path_bad = 0;
  return bad ? NULL : fs_path;
}

int64_t cxfs_read(void) {
  const char *p = take_path();
  free(fs_data); fs_data = NULL; fs_data_n = 0;
  if (!p) return -1;
  int f = open(p, O_RDONLY | O_CLOEXEC);
  if (f < 0) return -1;
  struct stat st;
  if (fstat(f, &st) != 0 || !S_ISREG(st.st_mode)) { close(f); return -1; }
  fs_data = malloc(st.st_size > 0 ? (size_t)st.st_size : 1);
  if (!fs_data) { close(f); return -1; }
  int64_t got = 0;
  while (got < st.st_size) {
    ssize_t r = read(f, fs_data + got, (size_t)(st.st_size - got));
    if (r < 0 && errno == EINTR) continue;
    if (r < 0) { close(f); free(fs_data); fs_data = NULL; return -1; }
    if (r == 0) break;
    got += r;
  }
  close(f);
  fs_data_n = got;
  return got;
}
int64_t cxfs_byte(int64_t i) { return i >= 0 && i < fs_data_n ? fs_data[i] : -1; }

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
int64_t cxfs_write(int64_t append) {
  const char *p = take_path();
  size_t n = fs_out_n;
  fs_out_n = 0;
  if (!p) return -1;
  int f = open(p, O_WRONLY | O_CREAT | O_CLOEXEC | (append ? O_APPEND : O_TRUNC), 0644);
  if (f < 0) return -1;
  size_t done = 0;
  while (done < n) {
    ssize_t w = write(f, fs_out + done, n - done);
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) { close(f); return -1; }
    done += (size_t)w;
  }
  return close(f) == 0 ? 0 : -1;
}

int64_t cxfs_kind(void) {
  const char *p = take_path();
  struct stat st;
  if (!p) return -1;
  if (stat(p, &st) != 0) return errno == ENOENT || errno == ENOTDIR ? 0 : -1;
  return S_ISREG(st.st_mode) ? 1 : S_ISDIR(st.st_mode) ? 2 : 3;
}
int64_t cxfs_size(void) {
  const char *p = take_path();
  struct stat st;
  if (!p || stat(p, &st) != 0 || !S_ISREG(st.st_mode)) return -1;
  return (int64_t)st.st_size;
}

int64_t cxfs_mkdir(void) {
  const char *p = take_path();
  if (!p) return -1;
  char d[CXFS_PATH_MAX + 1];
  snprintf(d, sizeof d, "%s", p);
  for (char *s = d + 1; *s; s++) {
    if (*s != '/') continue;
    *s = 0;
    if (mkdir(d, 0755) != 0 && errno != EEXIST) return -1;
    *s = '/';
  }
  if (mkdir(d, 0755) != 0 && errno != EEXIST) return -1;
  struct stat st;
  return stat(d, &st) == 0 && S_ISDIR(st.st_mode) ? 0 : -1;
}

int64_t cxfs_temp(void) {
  const char *prefix = take_path();
  if (!prefix || strchr(prefix, '/')) return -1;
  const char *tmp = getenv("TMPDIR");
  if (!tmp || !*tmp) tmp = "/tmp";
  size_t t = strlen(tmp);
  while (t > 1 && tmp[t - 1] == '/') t--;
  if (snprintf(fs_name, sizeof fs_name, "%.*s/%sXXXXXX", (int)t, tmp, prefix) >= (int)sizeof fs_name) return -1;
  if (!mkdtemp(fs_name)) { fs_name[0] = 0; return -1; }
  return (int64_t)strlen(fs_name);
}
int64_t cxfs_name_byte(int64_t i) {
  return i >= 0 && (size_t)i < strlen(fs_name) ? (unsigned char)fs_name[i] : -1;
}

static int remove_one(const char *path, const struct stat *st, int flag, struct FTW *ftw) {
  (void)st; (void)flag; (void)ftw;
  return remove(path) == 0 || errno == ENOENT ? 0 : -1;
}
int64_t cxfs_remove(void) {
  const char *p = take_path();
  struct stat st;
  if (!p) return -1;
  if (lstat(p, &st) != 0) return errno == ENOENT ? 0 : -1;
  if (!S_ISDIR(st.st_mode)) return unlink(p) == 0 ? 0 : -1;
  return nftw(p, remove_one, 32, FTW_DEPTH | FTW_PHYS) == 0 ? 0 : -1;
}

static int by_bytes(const void *a, const void *b) { return strcmp(*(char *const *)a, *(char *const *)b); }
int64_t cxfs_list(void) {
  const char *p = take_path();
  for (int64_t i = 0; i < fs_entries_n; i++) free(fs_entries[i]);
  free(fs_entries); fs_entries = NULL; fs_entries_n = 0;
  if (!p) return -1;
  DIR *d = opendir(p);
  if (!d) return -1;
  size_t cap = 0;
  struct dirent *e;
  while ((e = readdir(d))) {
    if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
    if ((size_t)fs_entries_n == cap) {
      cap = cap ? cap * 2 : 64;
      char **g = realloc(fs_entries, cap * sizeof *g);
      if (!g) { closedir(d); return -1; }
      fs_entries = g;
    }
    fs_entries[fs_entries_n] = strdup(e->d_name);
    if (!fs_entries[fs_entries_n]) { closedir(d); return -1; }
    fs_entries_n++;
  }
  closedir(d);
  qsort(fs_entries, (size_t)fs_entries_n, sizeof *fs_entries, by_bytes);
  return fs_entries_n;
}
int64_t cxfs_entry_len(int64_t i) { return i >= 0 && i < fs_entries_n ? (int64_t)strlen(fs_entries[i]) : -1; }
int64_t cxfs_entry_byte(int64_t i, int64_t j) {
  if (i < 0 || i >= fs_entries_n || j < 0 || (size_t)j >= strlen(fs_entries[i])) return -1;
  return (unsigned char)fs_entries[i][j];
}

int64_t cxfs_cwd(void) {
  if (!getcwd(fs_name, sizeof fs_name)) { fs_name[0] = 0; return -1; }
  return (int64_t)strlen(fs_name);
}
