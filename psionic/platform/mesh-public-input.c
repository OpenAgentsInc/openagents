/* Public bundle bytes only. No private witness, parsing or trust logic. */
#if defined(__APPLE__)
#define _DARWIN_C_SOURCE
#endif
#define _POSIX_C_SOURCE 200809L
#include "mesh-public-input.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stddef.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static unsigned char mpub_input[MPUB_INPUT_MAX];
static size_t mpub_used;

static int64_t mpub_now_ms(void) {
  struct timespec t;
  if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) return -1;
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static int mpub_kind(int fd) {
  if (isatty(fd)) return 4;
  struct stat st;
  if (fstat(fd, &st) != 0) return 0;
  if (S_ISFIFO(st.st_mode)) return 1;
  if (S_ISREG(st.st_mode)) return 3;
  if (S_ISSOCK(st.st_mode)) {
    struct sockaddr_storage address;
    socklen_t size = sizeof address;
    if (getsockname(fd, (struct sockaddr *)&address, &size) == 0 && address.ss_family == AF_UNIX) return 2;
  }
  return 0;
}
int64_t mpub_stdin_clear(void) {
  volatile unsigned char *p = mpub_input;
  for (size_t i = 0; i < sizeof mpub_input; i++) p[i] = 0;
  mpub_used = 0;
  return 0;
}
int64_t mpub_stdin_byte(int64_t i) { return i >= 0 && (uint64_t)i < mpub_used ? mpub_input[i] : -1; }
int64_t mpub_stdin_read(int64_t timeout_ms) {
  mpub_stdin_clear();
  if (timeout_ms < 0 || timeout_ms > 60000) return -3;
  int fd = fcntl(STDIN_FILENO, F_DUPFD_CLOEXEC, 3);
  if (fd < 0) return -3;
  int kind = mpub_kind(fd);
  if (kind != 1 && kind != 2 && kind != 3) { close(fd); return -3; }
  int flags = fcntl(fd, F_GETFL);
  if (flags < 0 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) != 0) { close(fd); return -3; }
  int64_t start = mpub_now_ms(), result = -3;
  if (start < 0) goto done;
  for (;;) {
    int64_t clock = mpub_now_ms();
    if (clock < 0) break;
    if (timeout_ms > 0 && clock >= start + timeout_ms) { result = -2; break; }
    int64_t left = start + timeout_ms - clock;
    if (left < 0) left = 0;
    struct pollfd p = { fd, POLLIN, 0 };
    int ready = poll(&p, 1, (int)left);
    if (ready == 0) { result = -2; break; }
    if (ready < 0) {
      if (errno == EINTR && left > 0) continue;
      result = errno == EINTR ? -2 : -3;
      break;
    }
    if (p.revents & POLLNVAL) break;
    unsigned char extra;
    size_t room = sizeof mpub_input - mpub_used;
    unsigned char *out = room ? mpub_input + mpub_used : &extra;
    size_t count = room ? room : 1;
    ssize_t got = read(fd, out, count);
    if (got == 0) {
      int64_t end = mpub_now_ms();
      result = end < 0 ? -3 : timeout_ms > 0 && end > start + timeout_ms ? -2 : (int64_t)mpub_used;
      break;
    }
    if (got < 0) {
      if ((errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK) && left > 0) continue;
      result = (errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK) ? -2 : -3;
      break;
    }
    if (!room) { result = -4; break; }
    mpub_used += (size_t)got;
  }
done:
  if (fcntl(fd, F_SETFL, flags) != 0) result = -3;
  close(fd);
  if (result < 0) mpub_stdin_clear();
  return result;
}
