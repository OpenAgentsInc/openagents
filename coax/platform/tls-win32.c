/* Windows TLS stub for local daemon builds (`coder-host.exe serve`, GitHub #13880).
   Local AF_UNIX daemon IPC uses Winsock AF_UNIX (`socket-win32.c`) and does not use TLS.
   Remote Nostr relay TLS (`wss://`) via Windows Schannel is not wired yet, so every
   `cxtls_*` call returns CXTLS_UNSUPPORTED honestly. */
#include "tls-native.h"

int64_t cxtls_host_push(int64_t byte) { (void)byte; return CXTLS_UNSUPPORTED; }
int64_t cxtls_connect(int64_t port, int64_t timeout_ms) { (void)port; (void)timeout_ms; return CXTLS_UNSUPPORTED; }
int64_t cxtls_start(int64_t h, int64_t timeout_ms) { (void)h; (void)timeout_ms; return CXTLS_UNSUPPORTED; }
int64_t cxtls_write(int64_t h, int64_t byte) { (void)h; (void)byte; return CXTLS_UNSUPPORTED; }
int64_t cxtls_flush(int64_t h, int64_t timeout_ms) { (void)h; (void)timeout_ms; return CXTLS_UNSUPPORTED; }
int64_t cxtls_read(int64_t h, int64_t timeout_ms) { (void)h; (void)timeout_ms; return CXTLS_UNSUPPORTED; }
int64_t cxtls_byte(int64_t h, int64_t i) { (void)h; (void)i; return 0; }
int64_t cxtls_version(int64_t h) { (void)h; return 0; }
int64_t cxtls_close(int64_t h) { (void)h; return 0; }
