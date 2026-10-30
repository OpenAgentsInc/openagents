/* Native effects checks: deadlines, pending reads, close semantics and reuse.
   Uses only localhost and ephemeral fixture certificates. */
#include "tls-native.h"
#include "socket-native.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int64_t connect_to(int port, int timeout) {
  const char *host = "localhost";
  for (const char *p = host; *p; p++) assert(cxtls_host_push(*p) == 0);
  return cxtls_connect(port, timeout);
}
int main(int argc, char **argv) {
  assert(argc == 3);
  int port = atoi(argv[2]);
  if (!strcmp(argv[1], "stall")) {
    int64_t started = cxsock_now_ms();
    for (int i = 0; i < 18; i++) assert(connect_to(port, 100) == CXTLS_TIMEOUT);
    assert(cxsock_now_ms() - started < 8000);
    puts("PASS stalled handshakes time out and release all slots");
    return 0;
  }
  if (!strcmp(argv[1], "eof") || !strcmp(argv[1], "truncate")) {
    int64_t h = connect_to(port, 3000); assert(h >= 0);
    int64_t n = cxtls_read(h, 3000);
    fprintf(stderr, "fixture %s: stream end %lld\n", argv[1], (long long)n);
#ifdef __APPLE__
    assert(n == CXTLS_END_UNVERIFIED); /* Network.framework hides close_notify. */
#else
    assert(n == (!strcmp(argv[1], "eof") ? CXTLS_EOF : CXTLS_ERROR));
#endif
    assert(cxtls_close(h) == 0);
    puts("PASS stream-end evidence is reported without inventing authenticated completion");
    return 0;
  }
  assert(!strcmp(argv[1], "stream"));
  assert(cxtls_host_push(0) == CXTLS_FULL);
  assert(cxtls_connect(port, 100) == CXTLS_ERROR);
  assert(cxtls_connect(port, 100) == CXTLS_ERROR); /* host was cleared */
  /* TLS's host builder must be independent of a pending plain-TCP name. */
  assert(cxsock_host_push('!') == 0);
  int64_t h = connect_to(port, 3000); assert(h >= 0);
  assert(cxtls_version(h) == 0x0304);
  assert(cxtls_read(h, 0) == CXTLS_TIMEOUT);
  assert(cxtls_byte(h, 0) == CXTLS_ERROR);
  /* No bytes can arrive before we write: independent of host scheduling. */
  assert(cxtls_read(h, 1) == CXTLS_TIMEOUT);
  enum { BYTES = 131071 };
  for (int i = 0; i < BYTES; i++) assert(cxtls_write(h, i % 251) == 0);
  assert(cxtls_flush(h, 3000) == 0);
  /* A timed-out pending receive must not lose or duplicate the later bytes. */
  int received = 0;
  while (received < BYTES) {
    int64_t n = cxtls_read(h, 3000); assert(n > 0 && n <= CXTLS_MAX_IN);
    for (int64_t i = 0; i < n; i++) assert(cxtls_byte(h, i) == (received + i) % 251);
    assert(cxtls_byte(h, n) == CXTLS_ERROR);
    received += (int)n;
  }
  assert(received == BYTES && cxtls_read(h, 0) == CXTLS_TIMEOUT);
  assert(cxtls_close(h) == 0 && cxtls_read(h, 0) == CXTLS_ERROR);
  /* Each close can have a pending callback; it must not affect the reused slot. */
  for (int i = 0; i < 24; i++) {
    h = connect_to(port, 3000); assert(h >= 0);
    assert(cxtls_read(h, 0) == CXTLS_TIMEOUT);
    assert(cxtls_close(h) == 0);
  }
  h = connect_to(port, 3000); assert(h >= 0);
  for (int i = 0; i < CXTLS_MAX_OUT; i++) assert(cxtls_write(h, i % 251) == 0);
  assert(cxtls_write(h, 1) == CXTLS_FULL);
  assert(cxtls_close(h) == 0);
  puts("PASS exact stream bytes, nonblocking/pending reads, bounds and slot reuse");
  return 0;
}
