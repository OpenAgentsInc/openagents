#ifndef COAX_SOCKET_SERVER_NATIVE_H
#define COAX_SOCKET_SERVER_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* A TCP listener and its accepted connections for Coax on Linux and macOS,
   through #foreign scalars only (the relay host, coax/relay/). Effects only:
   bind, listen, accept, poll, read, write, close and the clocks. Every
   decision (HTTP, WebSocket, Nostr, limits beyond these buffers) is Coax.

   cxsrv_listen(port) binds the address set by cxsrv_set_address (an IPv4 or
   IPv6 literal such as 127.0.0.1 or a tailnet address; no name lookup) and
   returns 0 or CXSRV_ERROR. cxsrv_bound_port() is the port actually bound
   (useful with port 0). cxsrv_poll(timeout_ms) waits until the listener or
   a connection is readable or a write can drain; it returns the number of
   ready descriptors, 0 on timeout. cxsrv_accept() returns a new handle or
   CXSRV_NONE / CXSRV_FULL (the connection is refused and closed then).
   cxsrv_read(h) reads what is ready without blocking: the count readable
   through cxsrv_byte(h, i) until the next read, 0 when nothing is ready,
   CXSRV_EOF or CXSRV_ERROR. cxsrv_write buffers a byte (CXSRV_FULL past
   CXSRV_MAX_OUT); cxsrv_flush(h, timeout_ms) sends every buffered byte or
   fails. cxsrv_close releases the handle. SIGPIPE is ignored.

   cxsrv_set_tls(1), after tls-native.c's cxtls_server_load, makes every
   accepted connection TLS (wss:// and https://). Nothing above changes for
   the caller: reads and writes are plaintext. The handshake runs inside
   cxsrv_read without blocking (0 while it waits, so a stalled client never
   holds up the others); a client that is not TLS, offers nothing at TLS 1.2
   or above, or does not finish within CXSRV_TLS_HANDSHAKE_MS gets
   CXSRV_ERROR on that handle only. cxsrv_poll returns at once while
   decrypted bytes wait inside TLS. Link tls-native.c and socket-native.c
   with the flags from coax/platform/tls-env.sh. */
#define CXSRV_NONE (0)
#define CXSRV_EOF (-1)
#define CXSRV_ERROR (-3)
#define CXSRV_FULL (-6)
#define CXSRV_MAX (32)
#define CXSRV_MAX_IN (65536)
#define CXSRV_MAX_OUT (4194304)
#define CXSRV_TLS_HANDSHAKE_MS (10000)
void cxsrv_set_address(const char *address);
void cxsrv_set_tls(int on);
int64_t cxsrv_listen(int64_t port);
int64_t cxsrv_bound_port(void);
int64_t cxsrv_poll(int64_t timeout_ms);
int64_t cxsrv_accept(void);
int64_t cxsrv_read(int64_t h);
int64_t cxsrv_byte(int64_t h, int64_t i);
int64_t cxsrv_write(int64_t h, int64_t byte);
int64_t cxsrv_flush(int64_t h, int64_t timeout_ms);
int64_t cxsrv_close(int64_t h);
int64_t cxsrv_now_s(void);
int64_t cxsrv_log(int64_t byte);
#ifdef __cplusplus
}
#endif
#endif
