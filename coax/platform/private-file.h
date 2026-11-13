/* One descriptor-based private-file boundary for TLS and local signing keys.
   Opening nonblocking prevents a substituted FIFO from blocking validation. */
#ifndef COAX_PRIVATE_FILE_H
#define COAX_PRIVATE_FILE_H
#include <errno.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
static inline int cx_private_open(const char *path) {
  int fd = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NOCTTY | O_NONBLOCK);
  if (fd < 0) return -1;
  struct stat st;
  if (fstat(fd, &st) != 0 || !S_ISREG(st.st_mode) || st.st_uid != geteuid() ||
      ((st.st_mode & 0777) != 0400 && (st.st_mode & 0777) != 0600)) {
    close(fd); errno = EPERM; return -1;
  }
  return fd;
}
#endif
