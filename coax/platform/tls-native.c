/* TLS client streams for Coax, and the server mode for C hosts; see
   tls-native.h for the contract. */
#define _POSIX_C_SOURCE 200809L
#define _DARWIN_C_SOURCE 1
#include "tls-native.h"
#include "socket-native.h"
#include <stdlib.h>
#include <string.h>

#if defined(__APPLE__)
#include "tls-apple.inc"
/* Server mode is not written for Network.framework yet (nw_listener). */
int64_t cxtls_server_load(const char *cert_path, const char *key_path) { (void)cert_path; (void)key_path; return CXTLS_UNSUPPORTED; }
cxtls_server *cxtls_server_new(int fd) { (void)fd; return NULL; }
int64_t cxtls_server_handshake(cxtls_server *s) { (void)s; return CXTLS_UNSUPPORTED; }
int cxtls_server_want(cxtls_server *s) { (void)s; return 0; }
int64_t cxtls_server_read(cxtls_server *s, unsigned char *buf, int64_t cap) { (void)s; (void)buf; (void)cap; return CXTLS_UNSUPPORTED; }
int64_t cxtls_server_pending(cxtls_server *s) { (void)s; return 0; }
int64_t cxtls_server_write(cxtls_server *s, const unsigned char *buf, int64_t n, int64_t deadline_ms) { (void)s; (void)buf; (void)n; (void)deadline_ms; return CXTLS_UNSUPPORTED; }
void cxtls_server_free(cxtls_server *s) { (void)s; }
#else

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <sys/stat.h>
#include <unistd.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>

#if OPENSSL_VERSION_NUMBER < 0x30000000L
#error "tls-native.c needs OpenSSL 3"
#endif

typedef struct {
  SSL *ssl;               /* NULL when no TLS stream is on this handle */
  int fd;
  int64_t in_count;
  unsigned char in[CXTLS_MAX_IN];
  int64_t out_count;
  unsigned char *out;     /* CXTLS_MAX_OUT bytes, allocated on start */
} cxtls_slot;

static cxtls_slot cxtls_slots[CXSOCK_MAX];
static SSL_CTX *cxtls_ctx = NULL;
static char cxtls_host[CXTLS_MAX_HOST + 1];
static int cxtls_host_len = 0;
static int cxtls_host_bad = 0;

static SSL_CTX *cxtls_context(void) {
  if (cxtls_ctx != NULL) return cxtls_ctx;
  SSL_CTX *c = SSL_CTX_new(TLS_client_method());
  if (c == NULL) return NULL;
  int ok = SSL_CTX_set_min_proto_version(c, TLS1_2_VERSION);
  SSL_CTX_set_options(c, SSL_OP_NO_COMPRESSION | SSL_OP_NO_RENEGOTIATION);
  SSL_CTX_set_mode(c, SSL_MODE_ENABLE_PARTIAL_WRITE | SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER);
  SSL_CTX_set_verify(c, SSL_VERIFY_PEER, NULL);
#ifdef CXTLS_TEST_TRUST
  /* Test builds only: trust exactly the named CA file, never the system. */
  const char *test_ca = getenv("CXTLS_TEST_CA");
  if (test_ca != NULL && *test_ca) ok = ok && SSL_CTX_load_verify_locations(c, test_ca, NULL) == 1;
  else ok = ok && SSL_CTX_set_default_verify_paths(c) == 1;
#else
  ok = ok && SSL_CTX_set_default_verify_paths(c) == 1;
#endif
  if (!ok) { SSL_CTX_free(c); return NULL; }
  cxtls_ctx = c;
  return c;
}

static cxtls_slot *cxtls_get(int64_t h) {
  if (h < 0 || h >= CXSOCK_MAX || cxtls_slots[h].ssl == NULL) return NULL;
  return &cxtls_slots[h];
}

int64_t cxtls_host_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || cxtls_host_len >= CXTLS_MAX_HOST) { cxtls_host_bad = 1; return CXTLS_FULL; }
  cxtls_host[cxtls_host_len++] = (char)byte;
  return 0;
}

/* Wait for what OpenSSL asked for, bounded by deadline (negative: none). */
static int64_t cxtls_wait(int fd, int want, int64_t deadline) {
  short events = want == SSL_ERROR_WANT_WRITE ? POLLOUT : POLLIN;
  for (;;) {
    int64_t left = deadline < 0 ? -1 : deadline - cxsock_now_ms();
    if (deadline >= 0 && left < 0) left = 0;
    struct pollfd p = { fd, events, 0 };
    int r = poll(&p, 1, left > 2147483647 ? 2147483647 : (int)left);
    if (r < 0 && errno == EINTR) continue;
    if (r < 0) return CXTLS_ERROR;
    if (r == 0) return CXTLS_TIMEOUT;
    return 0;
  }
}

static int64_t cxtls_handshake_error(SSL *ssl, int err) {
  long verify = SSL_get_verify_result(ssl);
  if (verify == X509_V_ERR_HOSTNAME_MISMATCH) return CXTLS_NAME;
  if (verify != X509_V_OK) return CXTLS_UNTRUSTED;
  if (err == SSL_ERROR_SSL) {
    unsigned long e;
    while ((e = ERR_get_error()) != 0) {
      if (ERR_GET_LIB(e) != ERR_LIB_SSL) continue;
      int reason = ERR_GET_REASON(e);
      if (reason == SSL_R_UNSUPPORTED_PROTOCOL || reason == SSL_R_TLSV1_ALERT_PROTOCOL_VERSION ||
          reason == SSL_R_NO_PROTOCOLS_AVAILABLE || reason == SSL_R_VERSION_TOO_LOW) return CXTLS_VERSION;
    }
  }
  return CXTLS_HANDSHAKE;
}

int64_t cxtls_start(int64_t h, int64_t timeout_ms) {
  int bad = cxtls_host_bad || cxtls_host_len == 0 || timeout_ms < 0;
  cxtls_host[cxtls_host_len] = 0;
  char host[CXTLS_MAX_HOST + 1];
  memcpy(host, cxtls_host, sizeof host);
  cxtls_host_len = 0; cxtls_host_bad = 0;
  int fd = cxsock_fd(h);
  if (bad || fd < 0 || cxtls_slots[h].ssl != NULL) return CXTLS_ERROR;
  SSL_CTX *ctx = cxtls_context();
  if (ctx == NULL) return CXTLS_ERROR;
  ERR_clear_error();
  SSL *ssl = SSL_new(ctx);
  if (ssl == NULL) return CXTLS_ERROR;
  SSL_set_hostflags(ssl, X509_CHECK_FLAG_NO_PARTIAL_WILDCARDS);
  if (SSL_set_fd(ssl, fd) != 1 || SSL_set_tlsext_host_name(ssl, host) != 1 || SSL_set1_host(ssl, host) != 1) {
    SSL_free(ssl); ERR_clear_error(); return CXTLS_ERROR;
  }
  int64_t deadline = cxsock_now_ms() + timeout_ms;
  int64_t result = 0;
  for (;;) {
    int r = SSL_connect(ssl);
    if (r == 1) break;
    int err = SSL_get_error(ssl, r);
    if (err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
      int64_t w = cxtls_wait(fd, err, deadline);
      if (w != 0) { result = w; break; }
      continue;
    }
    result = cxtls_handshake_error(ssl, err);
    break;
  }
  /* SSL_connect verified the chain and the name (SSL_VERIFY_PEER aborts the
     handshake otherwise); check again so no path admits an unverified peer. */
  if (result == 0 && (SSL_get_verify_result(ssl) != X509_V_OK || SSL_get0_peer_certificate(ssl) == NULL)) result = CXTLS_UNTRUSTED;
  if (result == 0 && SSL_version(ssl) < TLS1_2_VERSION) result = CXTLS_VERSION;
  cxtls_slot *s = &cxtls_slots[h];
  if (result == 0 && s->out == NULL) s->out = (unsigned char *)malloc(CXTLS_MAX_OUT);
  if (result == 0 && s->out == NULL) result = CXTLS_ERROR;
  ERR_clear_error();
  if (result != 0) { SSL_free(ssl); return result; }
  s->ssl = ssl; s->fd = fd; s->in_count = 0; s->out_count = 0;
  return 0;
}

/* Portable entry: one deadline covers TCP plus the TLS handshake. */
int64_t cxtls_connect(int64_t port, int64_t timeout_ms) {
  if (cxtls_host_bad || cxtls_host_len == 0 || timeout_ms < 0 ||
      timeout_ms > INT64_MAX - cxsock_now_ms()) {
    cxtls_host_len = 0; cxtls_host_bad = 0; return CXTLS_ERROR;
  }
  int64_t deadline = cxsock_now_ms() + timeout_ms;
  cxtls_host[cxtls_host_len] = 0;
  int64_t h = cxsock_connect_host(cxtls_host, port, timeout_ms);
  if (h < 0) { cxtls_host_len = 0; cxtls_host_bad = 0; return h; }
  int64_t left = deadline - cxsock_now_ms();
  int64_t result = cxtls_start(h, left < 0 ? 0 : left);
  if (result != 0) { cxsock_close(h); return result; }
  return h;
}

int64_t cxtls_write(int64_t h, int64_t byte) {
  cxtls_slot *s = cxtls_get(h);
  if (s == NULL || byte < 0 || byte > 255) return CXTLS_ERROR;
  if (s->out_count >= CXTLS_MAX_OUT) return CXTLS_FULL;
  s->out[s->out_count++] = (unsigned char)byte;
  return 0;
}

int64_t cxtls_flush(int64_t h, int64_t timeout_ms) {
  cxtls_slot *s = cxtls_get(h);
  if (s == NULL) return CXTLS_ERROR;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms;
  int64_t at = 0, result = 0;
  ERR_clear_error();
  while (at < s->out_count) {
    size_t n = 0;
    int r = SSL_write_ex(s->ssl, s->out + at, (size_t)(s->out_count - at), &n);
    if (r == 1) { at += (int64_t)n; continue; }
    int err = SSL_get_error(s->ssl, r);
    if (err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
      int64_t w = cxtls_wait(s->fd, err, deadline);
      if (w != 0) { result = w; break; }
      continue;
    }
    result = CXTLS_ERROR;
    break;
  }
  ERR_clear_error();
  s->out_count = 0;
  return result;
}

int64_t cxtls_read(int64_t h, int64_t timeout_ms) {
  cxtls_slot *s = cxtls_get(h);
  if (s == NULL) return CXTLS_ERROR;
  s->in_count = 0;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms;
  ERR_clear_error();
  for (;;) {
    size_t n = 0;
    int r = SSL_read_ex(s->ssl, s->in, CXTLS_MAX_IN, &n);
    if (r == 1) { s->in_count = (int64_t)n; return (int64_t)n; }
    int err = SSL_get_error(s->ssl, r);
    if (err == SSL_ERROR_ZERO_RETURN) return CXTLS_EOF;
    if (err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
      int64_t w = cxtls_wait(s->fd, err, deadline);
      if (w != 0) return w;
      continue;
    }
    /* A TCP close without close_notify is SSL_ERROR_SSL (unexpected EOF) in
       OpenSSL 3: reported as an error, since the stream may be truncated. */
    ERR_clear_error();
    return CXTLS_ERROR;
  }
}

int64_t cxtls_byte(int64_t h, int64_t i) {
  cxtls_slot *s = cxtls_get(h);
  if (s == NULL || i < 0 || i >= s->in_count) return CXTLS_ERROR;
  return s->in[i];
}

int64_t cxtls_version(int64_t h) {
  cxtls_slot *s = cxtls_get(h);
  if (s == NULL) return CXTLS_ERROR;
  return SSL_version(s->ssl);
}

int64_t cxtls_close(int64_t h) {
  cxtls_slot *s = cxtls_get(h);
  if (s == NULL) return CXTLS_ERROR;
  SSL_shutdown(s->ssl);   /* one nonblocking close_notify; we don't wait for the peer's */
  SSL_free(s->ssl);
  ERR_clear_error();
  s->ssl = NULL; s->fd = -1; s->in_count = 0; s->out_count = 0;
  free(s->out); s->out = NULL;
  return cxsock_close(h);
}
/* ---- Server mode (C hosts only; see tls-native.h). ---- */

struct cxtls_server {
  SSL *ssl;
  int want;               /* POLLIN or POLLOUT: what the last call waits for */
};
static SSL_CTX *cxtls_server_ctx = NULL;

/* The private key, read only from a regular file this user owns with no
   group or other bits, opened without following a link and checked on the
   open descriptor (no window between the check and the read). */
static EVP_PKEY *cxtls_server_key(const char *path, int64_t *why) {
  *why = CXTLS_KEY;
  int fd = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NOCTTY);
  if (fd < 0) { if (errno == ELOOP) *why = CXTLS_KEY_MODE; return NULL; }
  struct stat st;
  if (fstat(fd, &st) != 0 || !S_ISREG(st.st_mode) || st.st_uid != geteuid() || (st.st_mode & 077) != 0) {
    close(fd); *why = CXTLS_KEY_MODE; return NULL;
  }
  BIO *b = BIO_new_fd(fd, BIO_CLOSE);
  if (b == NULL) { close(fd); return NULL; }
  EVP_PKEY *k = PEM_read_bio_PrivateKey(b, NULL, NULL, NULL);
  BIO_free(b);
  return k;
}

int64_t cxtls_server_load(const char *cert_path, const char *key_path) {
  if (cxtls_server_ctx != NULL || cert_path == NULL || key_path == NULL) return CXTLS_ERROR;
  ERR_clear_error();
  int64_t why = 0;
  EVP_PKEY *key = cxtls_server_key(key_path, &why);
  if (key == NULL) { ERR_clear_error(); return why; }
  SSL_CTX *c = SSL_CTX_new(TLS_server_method());
  if (c == NULL) { EVP_PKEY_free(key); ERR_clear_error(); return CXTLS_ERROR; }
  int64_t result = 0;
  if (SSL_CTX_set_min_proto_version(c, TLS1_2_VERSION) != 1) result = CXTLS_ERROR;
  SSL_CTX_set_options(c, SSL_OP_NO_COMPRESSION | SSL_OP_NO_RENEGOTIATION | SSL_OP_CIPHER_SERVER_PREFERENCE);
  SSL_CTX_set_mode(c, SSL_MODE_ENABLE_PARTIAL_WRITE | SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER);
  SSL_CTX_set_verify(c, SSL_VERIFY_NONE, NULL);
  if (result == 0 && SSL_CTX_use_certificate_chain_file(c, cert_path) != 1) result = CXTLS_CERT;
  if (result == 0 && SSL_CTX_use_PrivateKey(c, key) != 1) result = CXTLS_KEY;
  if (result == 0 && SSL_CTX_check_private_key(c) != 1) result = CXTLS_KEY;
  EVP_PKEY_free(key);   /* the context holds its own reference */
  ERR_clear_error();
  if (result != 0) { SSL_CTX_free(c); return result; }
  cxtls_server_ctx = c;
  return 0;
}

cxtls_server *cxtls_server_new(int fd) {
  if (cxtls_server_ctx == NULL || fd < 0) return NULL;
  cxtls_server *s = (cxtls_server *)calloc(1, sizeof *s);
  if (s == NULL) return NULL;
  s->ssl = SSL_new(cxtls_server_ctx);
  if (s->ssl == NULL || SSL_set_fd(s->ssl, fd) != 1) {
    SSL_free(s->ssl); free(s); ERR_clear_error(); return NULL;
  }
  SSL_set_accept_state(s->ssl);
  s->want = POLLIN;
  return s;
}

int64_t cxtls_server_handshake(cxtls_server *s) {
  if (s == NULL) return CXTLS_ERROR;
  if (SSL_is_init_finished(s->ssl)) return 1;
  ERR_clear_error();
  int r = SSL_do_handshake(s->ssl);
  if (r == 1) {
    s->want = POLLIN;
    ERR_clear_error();
    return SSL_version(s->ssl) < TLS1_2_VERSION ? CXTLS_VERSION : 1;
  }
  int err = SSL_get_error(s->ssl, r);
  if (err == SSL_ERROR_WANT_READ) { s->want = POLLIN; return 0; }
  if (err == SSL_ERROR_WANT_WRITE) { s->want = POLLOUT; return 0; }
  int64_t result = CXTLS_HANDSHAKE;
  if (err == SSL_ERROR_SSL) {
    unsigned long e;
    while ((e = ERR_get_error()) != 0) {
      if (ERR_GET_LIB(e) != ERR_LIB_SSL) continue;
      int reason = ERR_GET_REASON(e);
      if (reason == SSL_R_UNSUPPORTED_PROTOCOL || reason == SSL_R_NO_PROTOCOLS_AVAILABLE ||
          reason == SSL_R_VERSION_TOO_LOW) result = CXTLS_VERSION;
    }
  }
  ERR_clear_error();
  return result;
}

int cxtls_server_want(cxtls_server *s) { return s == NULL ? 0 : s->want; }

int64_t cxtls_server_read(cxtls_server *s, unsigned char *buf, int64_t cap) {
  if (s == NULL || buf == NULL || cap <= 0 || !SSL_is_init_finished(s->ssl)) return CXTLS_ERROR;
  ERR_clear_error();
  size_t n = 0;
  int r = SSL_read_ex(s->ssl, buf, (size_t)cap, &n);
  if (r == 1) { s->want = POLLIN; return (int64_t)n; }
  int err = SSL_get_error(s->ssl, r);
  ERR_clear_error();
  if (err == SSL_ERROR_WANT_READ) { s->want = POLLIN; return 0; }
  if (err == SSL_ERROR_WANT_WRITE) { s->want = POLLOUT; return 0; }
  if (err == SSL_ERROR_ZERO_RETURN) return CXTLS_EOF;
  return CXTLS_ERROR;
}

int64_t cxtls_server_pending(cxtls_server *s) {
  return s != NULL && SSL_is_init_finished(s->ssl) && SSL_has_pending(s->ssl) ? 1 : 0;
}

int64_t cxtls_server_write(cxtls_server *s, const unsigned char *buf, int64_t n, int64_t deadline_ms) {
  if (s == NULL || n < 0 || (n > 0 && buf == NULL) || !SSL_is_init_finished(s->ssl)) return CXTLS_ERROR;
  int fd = SSL_get_fd(s->ssl);
  int64_t at = 0;
  ERR_clear_error();
  while (at < n) {
    size_t w = 0;
    int r = SSL_write_ex(s->ssl, buf + at, (size_t)(n - at), &w);
    if (r == 1) { at += (int64_t)w; continue; }
    int err = SSL_get_error(s->ssl, r);
    if (err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
      int64_t wait = cxtls_wait(fd, err, deadline_ms < 0 ? 0 : deadline_ms);
      if (wait != 0) { ERR_clear_error(); return wait; }
      continue;
    }
    ERR_clear_error();
    return CXTLS_ERROR;
  }
  return 0;
}

void cxtls_server_free(cxtls_server *s) {
  if (s == NULL) return;
  if (SSL_is_init_finished(s->ssl)) SSL_shutdown(s->ssl);   /* one nonblocking close_notify */
  SSL_free(s->ssl);
  ERR_clear_error();
  free(s);
}
#endif
