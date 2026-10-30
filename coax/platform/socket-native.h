#ifndef COAX_SOCKET_NATIVE_H
#define COAX_SOCKET_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* TCP client sockets for Coax on Linux and macOS, through #foreign scalars
   only. A handle is a small index; bytes cross one at a time. This file does
   effects only: name resolution, connect, read, write, close and a monotonic
   clock. The WebSocket protocol, the relay policy and every limit beyond
   these buffers are Coax (coax/lib/net/ws.coax, coax/lib/nostr/relay.coax).
   There is no TLS here: wss:// needs a separate owner decision.

   Connect: cxsock_host_push the host name's bytes (at most CXSOCK_MAX_HOST),
   then cxsock_connect(port, timeout_ms) resolves it (getaddrinfo, blocking),
   tries each address with a nonblocking connect bounded by timeout_ms, and
   returns a handle or CXSOCK_RESOLVE / CXSOCK_REFUSED / CXSOCK_TIMEOUT /
   CXSOCK_FULL / CXSOCK_ERROR. The pushed host is cleared either way.

   Write: cxsock_write buffers a byte (CXSOCK_FULL past CXSOCK_MAX_OUT);
   cxsock_flush(h, timeout_ms) sends every buffered byte or fails.

   Read: cxsock_read(h, timeout_ms) waits at most timeout_ms (0: only what is
   ready, negative: for ever) and returns the count of bytes now readable
   through cxsock_byte (valid until the next read), CXSOCK_TIMEOUT, CXSOCK_EOF
   or CXSOCK_ERROR. cxsock_close releases the handle. SIGPIPE is not raised:
   a write to a closed peer reports CXSOCK_ERROR. */
#define CXSOCK_EOF (-1)
#define CXSOCK_TIMEOUT (-2)
#define CXSOCK_ERROR (-3)
#define CXSOCK_RESOLVE (-4)
#define CXSOCK_REFUSED (-5)
#define CXSOCK_FULL (-6)
#define CXSOCK_MAX (16)
#define CXSOCK_MAX_HOST (253)
#define CXSOCK_MAX_IN (65536)
#define CXSOCK_MAX_OUT (1048576)
int64_t cxsock_host_push(int64_t byte);
int64_t cxsock_connect(int64_t port, int64_t timeout_ms);
int64_t cxsock_write(int64_t h, int64_t byte);
int64_t cxsock_flush(int64_t h, int64_t timeout_ms);
int64_t cxsock_read(int64_t h, int64_t timeout_ms);
int64_t cxsock_byte(int64_t h, int64_t i);
int64_t cxsock_close(int64_t h);
int64_t cxsock_now_ms(void);
#ifdef __cplusplus
}
#endif
#endif
