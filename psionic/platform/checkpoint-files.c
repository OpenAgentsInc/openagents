/* Atomic checkpoint publication: write a temporary file in the same
   directory, fsync it, rename it over the name, fsync the directory. A reader
   sees the old file or the new one, never a partial one. No admission here. */
#include "checkpoint-files.h"
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
static int path(char *out, size_t cap, const char *dir, const char *name) {
    if (!dir || !name || !*name || strchr(name, '/')) return -1;
    int n = snprintf(out, cap, "%s/%s", dir, name);
    return n < 0 || (size_t)n >= cap ? -1 : 0;
}
int64_t psionic_ck_publish(const char *dir, const char *name, const uint8_t *bytes, int64_t n) {
    char final[1024], tmp[1100];
    if (n < 0 || path(final, sizeof final, dir, name) || snprintf(tmp, sizeof tmp, "%s.tmp.XXXXXX", final) >= (int)sizeof tmp) return -1;
    int directory = open(dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if (directory < 0) return -1;
    int fd = mkstemp(tmp);
    if (fd < 0) { close(directory); return -1; }
    int64_t done = 0;
    while (done < n) { ssize_t w = write(fd, bytes + done, (size_t)(n - done)); if (w <= 0) break; done += w; }
    int ok = done == n && fsync(fd) == 0;
    ok = close(fd) == 0 && ok;
    if (!ok || rename(tmp, final) != 0) { unlink(tmp); close(directory); return -1; }
    ok = fsync(directory) == 0; close(directory);
    return ok ? 0 : -1;
}
/* The file's length (at most cap bytes are read), or -1. */
int64_t psionic_ck_load(const char *dir, const char *name, uint8_t *buf, int64_t cap) {
    char p[1024];
    if (cap < 0 || path(p, sizeof p, dir, name)) return -1;
    int fd = open(p, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return -1;
    int64_t n = 0; ssize_t r;
    while (n < cap && (r = read(fd, buf + n, (size_t)(cap - n))) > 0) n += r;
    uint8_t extra; ssize_t more = read(fd, &extra, 1); close(fd);
    return more == 0 ? n : -1;
}
