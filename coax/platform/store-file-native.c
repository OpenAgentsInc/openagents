/* The verified Nostr store's append-only file: effects only (see
   store-file-native.h). One writer per file, enforced with flock. */
#define _DEFAULT_SOURCE
#include "store-file-native.h"
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
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

int cxstore_set_path(const char *path) {
  size_t n = strlen(path);
  if (n == 0 || n >= sizeof path_buf) return -1;
  memcpy(path_buf, path, n + 1);
  return 0;
}

int64_t cxstore_load(void) {
  if (!path_buf[0]) return -1;
  if (fd < 0) {
    fd = open(path_buf, O_RDWR | O_CREAT | O_CLOEXEC, 0600);
    if (fd < 0) return -1;
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); fd = -1; return -1; }
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
  if (fd < 0) return -1;
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
  if (ftruncate(fd, end) == 0) (void)fsync(fd);
  return -1;
}

int64_t cxstore_truncate(int64_t n) {
  if (fd < 0 || n < 0) return -1;
  if (ftruncate(fd, (off_t)n) != 0 || fsync(fd) != 0) return -1;
  if (n < loaded_n) loaded_n = n;
  return 0;
}
