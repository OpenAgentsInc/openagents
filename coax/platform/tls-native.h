#ifndef COAX_TLS_NATIVE_H
#define COAX_TLS_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* TLS client streams for Coax over socket-native handles, through #foreign
   scalars only. Owner exception 2026-10-29 (AGENTS.md, Rules): this file
   does the TLS handshake, certificate verification and record encryption
   and nothing else. Whether a URL needs TLS, which host and port, and every
   decision above the byte stream (WebSocket, Nostr, NIP-42) are Coax:
   coax/lib/net/relay_uri.coax decides ws:// or wss://, and the driver calls
   these only when it says tls.

   Linux: OpenSSL 3 (headers and libraries from Nix: coax/platform/tls-env.sh).
   Apple: Network.framework owns TCP + TLS; Security evaluates the certificate
   chain and DNS name. Link the flags from tls-env.sh. Calls have one owner
   thread per process; Apple callbacks are internally serialized. Handles are
   opaque and only valid with the matching transport's functions.

   Policy, fixed here with no switch to weaken it: TLS 1.2 minimum (1.3
   preferred), SNI set to the host, the chain verified against the system
   trust store, and the DNS name checked against the leaf. Linux uses
   OpenSSL's default verify paths and SSL_set1_host (no partial wildcards);
   Apple uses SecPolicyCreateSSL and SecTrustEvaluateWithError, without
   certificate-controlled network fetches. There is no option to skip
   verification. A macOS/Linux test build compiled
   with -DCXTLS_TEST_TRUST trusts only the PEM file named by the environment
   variable CXTLS_TEST_CA instead of the system store; shipped builds never
   define that macro, and without it the variable is ignored.

   Portable start: cxtls_host_push the host name's bytes, then
   cxtls_connect(port, timeout_ms) connects TCP + TLS within one deadline.
   It returns an opaque nonnegative handle, or a negative TLS/socket error.
   Failure releases every connection effect. Always use this on Apple.
   Apple's deadline includes asynchronous DNS. Linux's system getaddrinfo
   remains blocking, as documented in socket-native.h.

   Legacy Linux-only upgrade: cxsock_connect a TCP handle h, cxtls_host_push the host name's
   bytes (at most CXTLS_MAX_HOST), then cxtls_start(h, timeout_ms) runs the
   handshake within timeout_ms. It returns 0, or CXTLS_UNTRUSTED (the chain
   does not reach a trusted root, or is expired or malformed), CXTLS_NAME
   (the certificate is not for this host), CXTLS_VERSION (the peer offers
   nothing at TLS 1.2 or above), CXTLS_HANDSHAKE (any other handshake
   failure, including a peer that is not TLS or closes early),
   CXTLS_TIMEOUT, CXTLS_FULL or CXTLS_ERROR. The pushed host is cleared
   either way. On failure no TLS state remains and the caller still owns h
   (cxsock_close it). Apple cannot adopt a POSIX fd and returns
   CXTLS_UNSUPPORTED for this legacy upgrade, consuming the pushed name.

   After a successful start, use these instead of cxsock_write/flush/read/
   byte on h: cxtls_write buffers a byte (CXTLS_FULL past CXTLS_MAX_OUT);
   cxtls_flush(h, timeout_ms) encrypts and sends every buffered byte or
   fails; cxtls_read(h, timeout_ms) waits at most timeout_ms (0: only what is
   ready, negative: for ever) and returns the count of plaintext bytes now
   readable through cxtls_byte (valid until the next read), CXTLS_TIMEOUT,
   CXTLS_EOF (the peer sent close_notify), CXTLS_END_UNVERIFIED or CXTLS_ERROR.
   Network.framework does not expose whether receive completion included a
   close_notify: Apple returns CXTLS_END_UNVERIFIED for either completion.
   This never authenticates stream completeness. A framed application must
   establish its own complete closing record before treating it as orderly.
   Linux reports a TCP close without close_notify as CXTLS_ERROR.
   A timed-out Apple read stays pending and later bytes remain available.
   Flush failure requires closing; partial writes must never be replayed.
   cxtls_version(h) is the negotiated protocol
   (0x0303 for TLS 1.2, 0x0304 for TLS 1.3). cxtls_close(h) sends
   close_notify without waiting, frees the TLS state and closes h. */
#define CXTLS_EOF (-1)
#define CXTLS_TIMEOUT (-2)
#define CXTLS_ERROR (-3)
#define CXTLS_FULL (-6)
#define CXTLS_UNSUPPORTED (-7)
#define CXTLS_HANDSHAKE (-8)
#define CXTLS_UNTRUSTED (-9)
#define CXTLS_NAME (-10)
#define CXTLS_VERSION (-11)
#define CXTLS_END_UNVERIFIED (-12)
#define CXTLS_MAX_HOST (253)
#define CXTLS_MAX_IN (16384)
#define CXTLS_MAX_OUT (1048576)
int64_t cxtls_host_push(int64_t byte);
/* Preferred portable entry: push the TLS host, then connect TCP + TLS under
   one deadline. A failed connect closes all effects; a success is an opaque
   TLS handle, used only with cxtls_* (Apple owns no socket-native fd). */
int64_t cxtls_connect(int64_t port, int64_t timeout_ms);
int64_t cxtls_start(int64_t h, int64_t timeout_ms);
int64_t cxtls_write(int64_t h, int64_t byte);
int64_t cxtls_flush(int64_t h, int64_t timeout_ms);
int64_t cxtls_read(int64_t h, int64_t timeout_ms);
int64_t cxtls_byte(int64_t h, int64_t i);
int64_t cxtls_version(int64_t h);
int64_t cxtls_close(int64_t h);
#ifdef __cplusplus
}
#endif
#endif
