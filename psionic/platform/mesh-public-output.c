#define _POSIX_C_SOURCE 200809L
#include "mesh-public-output.h"
#include <errno.h>
#include <unistd.h>
int64_t mpub_stdout_tty(void) { return isatty(STDOUT_FILENO) ? 1 : 0; }
int64_t mpub_stderr_byte(int64_t byte) {
  if (byte < 0 || byte > 255) return -3;
  unsigned char value = (unsigned char)byte;
  ssize_t written;
  do { written = write(STDERR_FILENO, &value, 1); } while (written < 0 && errno == EINTR);
  return written == 1 ? 0 : -3;
}
