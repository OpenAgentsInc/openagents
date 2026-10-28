/* The engine's own stdin and stdout, for coax/web/agent/turn.coax through
   #foreign scalars: Coax's Basic prints but cannot read. eio_read_line
   returns the next stdin line's length (its bytes through eio_line_byte,
   valid until the next read), -1 at the end, -2 after timeout_ms (-1 waits).
   eio_put buffers a byte for stdout and eio_flush writes them. */
#include <errno.h>
#include <poll.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static char *in_buf, *line;
static size_t in_n, in_cap, line_n;
static char *out_buf;
static size_t out_n, out_cap;

int64_t eio_read_line(int64_t timeout_ms) {
  for (;;) {
    char *nl = in_n ? memchr(in_buf, '\n', in_n) : NULL;
    if (nl) {
      size_t n = (size_t)(nl - in_buf);
      free(line);
      line = malloc(n + 1);
      if (!line) return -1;
      memcpy(line, in_buf, n);
      line_n = n;
      memmove(in_buf, nl + 1, in_n - n - 1);
      in_n -= n + 1;
      return (int64_t)n;
    }
    struct pollfd p = {0, POLLIN, 0};
    int r = poll(&p, 1, (int)timeout_ms);
    if (r == 0) return -2;
    if (r < 0) { if (errno == EINTR) continue; return -1; }
    if (in_cap - in_n < 65536) {
      size_t cap = in_cap ? in_cap * 2 : 131072;
      char *b = realloc(in_buf, cap);
      if (!b) return -1;
      in_buf = b; in_cap = cap;
    }
    ssize_t k = read(0, in_buf + in_n, in_cap - in_n);
    if (k <= 0) return -1;
    in_n += (size_t)k;
  }
}
int64_t eio_line_byte(int64_t i) { return i >= 0 && (size_t)i < line_n ? (unsigned char)line[i] : 0; }
int64_t eio_flush(void) {
  size_t at = 0;
  while (at < out_n) {
    ssize_t k = write(1, out_buf + at, out_n - at);
    if (k < 0 && errno == EINTR) continue;
    if (k <= 0) { out_n = 0; return 0; }
    at += (size_t)k;
  }
  out_n = 0;
  return 1;
}
int64_t eio_put(int64_t byte) {
  if (out_n == out_cap) {
    size_t cap = out_cap ? out_cap * 2 : 65536;
    char *b = realloc(out_buf, cap);
    if (!b) return 0;
    out_buf = b; out_cap = cap;
  }
  out_buf[out_n++] = (char)byte;
  return 1;
}
