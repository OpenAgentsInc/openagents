// Signing Nostr events and sending them to a relay over a WebSocket (RFC 6455),
// for publishing a project made from a drop (drops.c, rc-publish). BIP-340
// Schnorr via libsecp256k1, TLS via OpenSSL. The client is the small part of
// the protocol a publisher needs: one connection, text frames out (masked),
// text frames in, ping answered, NIP-42 AUTH answered with the project's key,
// and an OK awaited for every event. Nothing it sends or reads is logged.
#include "relay.h"
#include <netdb.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>
#include <poll.h>
#include <secp256k1.h>
#include <signal.h>
#include <secp256k1_extrakeys.h>
#include <secp256k1_schnorrsig.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define WAIT_MS 15000
#define FRAME_MAX (1 << 20)

static void hex(const unsigned char *in, size_t n, char *out) { for (size_t i = 0; i < n; i++) snprintf(out + 2 * i, 3, "%02x", in[i]); }
static int hexv(char c) { return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1; }
static bool unhex(const char *s, unsigned char *out, size_t n) {
  if (strlen(s) != 2 * n) return false;
  for (size_t i = 0; i < n; i++) {
    int a = hexv(s[2 * i]), b = hexv(s[2 * i + 1]);
    if (a < 0 || b < 0) return false;
    out[i] = (unsigned char)(a * 16 + b);
  }
  return true;
}
static void fail(char *err, size_t cap, const char *why) { if (err && cap) snprintf(err, cap, "%s", why); }

static secp256k1_context *ctx(void) {
  static secp256k1_context *c;
  if (!c) {
    unsigned char seed[32];
    c = secp256k1_context_create(SECP256K1_CONTEXT_NONE);
    if (RAND_bytes(seed, 32) == 1) (void)secp256k1_context_randomize(c, seed);
  }
  return c;
}

bool nostr_pubkey(const unsigned char sk[32], char pubhex[65]) {
  secp256k1_keypair pair; secp256k1_xonly_pubkey x; unsigned char pub[32];
  if (!secp256k1_keypair_create(ctx(), &pair, sk)) return false;
  secp256k1_keypair_xonly_pub(ctx(), &x, NULL, &pair);
  secp256k1_xonly_pubkey_serialize(ctx(), pub, &x);
  hex(pub, 32, pubhex);
  return true;
}

bool nostr_keygen(unsigned char sk[32], char pubhex[65]) {
  for (int tries = 0; tries < 8; tries++) {
    if (RAND_bytes(sk, 32) != 1) return false;
    if (nostr_pubkey(sk, pubhex)) return true;
  }
  return false;
}

char *nostr_sign(const unsigned char sk[32], int kind, long long created_at, const char *tags_json, const char *content_json, char id[65]) {
  char pk[65];
  secp256k1_keypair pair;
  if (!secp256k1_keypair_create(ctx(), &pair, sk) || !nostr_pubkey(sk, pk)) return NULL;
  size_t n = strlen(tags_json) + strlen(content_json) + 200;
  char *serial = malloc(n), *event = malloc(n + 300);
  if (!serial || !event) { free(serial); free(event); return NULL; }
  // NIP-01: the id is the hash of [0, pubkey, created_at, kind, tags, content]
  snprintf(serial, n, "[0,\"%s\",%lld,%d,%s,%s]", pk, created_at, kind, tags_json, content_json);
  unsigned char d[32], sig[64], aux[32];
  SHA256((const unsigned char *)serial, strlen(serial), d);
  free(serial);
  if (RAND_bytes(aux, 32) != 1 || !secp256k1_schnorrsig_sign32(ctx(), sig, d, &pair, aux)) { free(event); return NULL; }
  char sighex[129];
  hex(d, 32, id); hex(sig, 64, sighex);
  snprintf(event, n + 300, "{\"id\":\"%s\",\"pubkey\":\"%s\",\"created_at\":%lld,\"kind\":%d,\"tags\":%s,\"content\":%s,\"sig\":\"%s\"}",
           id, pk, created_at, kind, tags_json, content_json, sighex);
  return event;
}

// ---- sealing
bool seal(const char *kek_hex, const char *aad, const unsigned char *plain, size_t n, char *sealed, size_t cap) {
  unsigned char kek[32], nonce[12], tag[16];
  if (!kek_hex || !unhex(kek_hex, kek, 32) || cap < 2 * (12 + n + 16) + 1 || RAND_bytes(nonce, 12) != 1) return false;
  unsigned char *ct = malloc(n + 16);
  EVP_CIPHER_CTX *c = EVP_CIPHER_CTX_new();
  int len = 0, fin = 0;
  bool ok = ct && c && EVP_EncryptInit_ex(c, EVP_aes_256_gcm(), NULL, kek, nonce) == 1
    && EVP_EncryptUpdate(c, NULL, &len, (const unsigned char *)aad, (int)strlen(aad)) == 1
    && EVP_EncryptUpdate(c, ct, &len, plain, (int)n) == 1 && EVP_EncryptFinal_ex(c, ct + len, &fin) == 1
    && EVP_CIPHER_CTX_ctrl(c, EVP_CTRL_GCM_GET_TAG, 16, tag) == 1;
  if (ok) { hex(nonce, 12, sealed); hex(ct, n, sealed + 24); hex(tag, 16, sealed + 24 + 2 * n); }
  EVP_CIPHER_CTX_free(c); if (ct) { memset(ct, 0, n + 16); free(ct); }
  memset(kek, 0, sizeof kek);
  return ok;
}

bool unseal(const char *kek_hex, const char *aad, const char *sealed, unsigned char *plain, size_t n) {
  unsigned char kek[32], nonce[12], tag[16];
  if (!kek_hex || !sealed || strlen(sealed) != 2 * (12 + n + 16) || !unhex(kek_hex, kek, 32)) return false;
  unsigned char *ct = malloc(n + 1);
  char part[64];
  bool ok = ct != NULL;
  memcpy(part, sealed, 24); part[24] = 0; ok = ok && unhex(part, nonce, 12);
  for (size_t i = 0; ok && i < n; i++) { char b[3] = {sealed[24 + 2 * i], sealed[25 + 2 * i], 0}; ok = unhex(b, ct + i, 1); }
  memcpy(part, sealed + 24 + 2 * n, 32); part[32] = 0; ok = ok && unhex(part, tag, 16);
  EVP_CIPHER_CTX *c = EVP_CIPHER_CTX_new();
  int len = 0, fin = 0;
  ok = ok && c && EVP_DecryptInit_ex(c, EVP_aes_256_gcm(), NULL, kek, nonce) == 1
    && EVP_DecryptUpdate(c, NULL, &len, (const unsigned char *)aad, (int)strlen(aad)) == 1
    && EVP_DecryptUpdate(c, plain, &len, ct, (int)n) == 1
    && EVP_CIPHER_CTX_ctrl(c, EVP_CTRL_GCM_SET_TAG, 16, tag) == 1 && EVP_DecryptFinal_ex(c, plain + len, &fin) == 1;
  EVP_CIPHER_CTX_free(c); free(ct);
  memset(kek, 0, sizeof kek);
  if (!ok) memset(plain, 0, n);
  return ok;
}

// ---- the WebSocket
struct relay {
  int fd; SSL_CTX *tls_ctx; SSL *tls;
  unsigned char sk[32]; char url[512];
  char challenge[256]; bool authed;
  unsigned char *in; size_t in_n, in_cap;   // bytes read, not yet a whole frame
};

static long io(relay *r, void *buf, size_t n, bool out) {
  if (r->tls) return out ? SSL_write(r->tls, buf, (int)n) : SSL_read(r->tls, buf, (int)n);
  return out ? send(r->fd, buf, n, 0) : recv(r->fd, buf, n, 0);
}
static bool put_all(relay *r, const unsigned char *p, size_t n) {
  while (n) { long k = io(r, (void *)p, n, true); if (k <= 0) return false; p += k; n -= (size_t)k; }
  return true;
}
// Some bytes into r->in, waiting up to ms.
static bool fill(relay *r, int ms) {
  if (!(r->tls && SSL_pending(r->tls) > 0)) {
    struct pollfd p = {r->fd, POLLIN, 0};
    if (poll(&p, 1, ms) <= 0) return false;
  }
  if (r->in_n + 16384 > r->in_cap) {
    size_t cap = r->in_cap ? r->in_cap * 2 : 65536;
    if (cap > 4 * FRAME_MAX) return false;
    unsigned char *q = realloc(r->in, cap);
    if (!q) return false;
    r->in = q; r->in_cap = cap;
  }
  long k = io(r, r->in + r->in_n, r->in_cap - r->in_n, false);
  if (k <= 0) return false;
  r->in_n += (size_t)k;
  return true;
}
static long long now_ms(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (long long)t.tv_sec * 1000 + t.tv_nsec / 1000000; }

static bool frame_out(relay *r, int op, const char *data, size_t n) {
  unsigned char head[14], mask[4];
  size_t h = 0;
  head[h++] = (unsigned char)(0x80 | op);
  if (n < 126) head[h++] = (unsigned char)(0x80 | n);
  else if (n < 65536) { head[h++] = 0x80 | 126; head[h++] = (unsigned char)(n >> 8); head[h++] = (unsigned char)n; }
  else { head[h++] = 0x80 | 127; for (int i = 7; i >= 0; i--) head[h++] = (unsigned char)((uint64_t)n >> (8 * i)); }
  if (RAND_bytes(mask, 4) != 1) return false;
  memcpy(head + h, mask, 4); h += 4;
  unsigned char *body = malloc(n ? n : 1);
  if (!body) return false;
  for (size_t i = 0; i < n; i++) body[i] = (unsigned char)data[i] ^ mask[i & 3];
  bool ok = put_all(r, head, h) && put_all(r, body, n);
  free(body);
  return ok;
}

// The next text message (malloc'd), waiting until deadline; pings are answered on the way.
static char *message_in(relay *r, long long deadline) {
  char *msg = NULL; size_t msg_n = 0;
  for (;;) {
    if (r->in_n >= 2) {
      size_t len = r->in[1] & 0x7f, h = 2;
      if (len == 126) { if (r->in_n < 4) goto more; len = ((size_t)r->in[2] << 8) | r->in[3]; h = 4; }
      else if (len == 127) { if (r->in_n < 10) goto more; len = 0; for (int i = 0; i < 8; i++) len = (len << 8) | r->in[2 + i]; h = 10; }
      bool masked = r->in[1] & 0x80;
      if (len > FRAME_MAX) { free(msg); return NULL; }
      if (r->in_n < h + (masked ? 4 : 0) + len) goto more;
      unsigned char *p = r->in + h + (masked ? 4 : 0);
      if (masked) for (size_t i = 0; i < len; i++) p[i] ^= r->in[h + (i & 3)];
      int op = r->in[0] & 0x0f; bool fin = r->in[0] & 0x80;
      size_t used = h + (masked ? 4 : 0) + len;
      if (op == 0x9) frame_out(r, 0xA, (const char *)p, len);
      else if (op == 0x8) { free(msg); return NULL; }
      else if (op == 0x1 || op == 0x0) {
        char *m = realloc(msg, msg_n + len + 1);
        if (!m) { free(msg); return NULL; }
        msg = m; memcpy(msg + msg_n, p, len); msg_n += len; msg[msg_n] = 0;
        memmove(r->in, r->in + used, r->in_n - used); r->in_n -= used;
        if (fin) return msg;
        continue;
      }
      memmove(r->in, r->in + used, r->in_n - used); r->in_n -= used;
      continue;
    }
  more:;
    long long left = deadline - now_ms();
    if (left <= 0 || !fill(r, (int)left)) { free(msg); return NULL; }
  }
}

// A JSON string's raw text after the n-th comma of a top-level array message (no unescaping:
// the values read here are hex, words or the relay's own short text).
static bool field(const char *msg, int n, char *out, size_t cap) {
  const char *p = msg; int depth = 0, k = 0; bool in_str = false;
  out[0] = 0;
  for (; *p; p++) {
    if (in_str) { if (*p == '\\' && p[1]) p++; else if (*p == '"') in_str = false; continue; }
    if (*p == '"') { if (depth == 1 && k == n) break; in_str = true; }
    else if (*p == '[' || *p == '{') depth++;
    else if (*p == ']' || *p == '}') depth--;
    else if (*p == ',' && depth == 1) k++;
    else if (depth == 1 && k == n && (*p == 't' || *p == 'f')) { snprintf(out, cap, "%s", *p == 't' ? "true" : "false"); return true; }
  }
  if (*p != '"') return false;
  p++;
  size_t o = 0;
  for (; *p && *p != '"' && o + 1 < cap; p++) { if (*p == '\\' && p[1]) p++; out[o++] = *p; }
  out[o] = 0;
  return true;
}

static bool auth(relay *r, char *err, size_t cap) {
  char ue[600], ce[600], tags[1400], id[65];
  // the url and challenge as NIP-01 strings: they hold no quotes or backslashes we keep
  size_t o = 0;
  for (const char *s = r->url; *s && o + 2 < sizeof ue; s++) if (*s != '"' && *s != '\\' && (unsigned char)*s >= 0x20) ue[o++] = *s;
  ue[o] = 0; o = 0;
  for (const char *s = r->challenge; *s && o + 2 < sizeof ce; s++) if (*s != '"' && *s != '\\' && (unsigned char)*s >= 0x20) ce[o++] = *s;
  ce[o] = 0;
  snprintf(tags, sizeof tags, "[[\"relay\",\"%s\"],[\"challenge\",\"%s\"]]", ue, ce);
  char *ev = nostr_sign(r->sk, 22242, (long long)time(NULL), tags, "\"\"", id);
  if (!ev) { fail(err, cap, "auth: not signed"); return false; }
  size_t n = strlen(ev) + 16;
  char *msg = malloc(n);
  if (!msg) { free(ev); return false; }
  snprintf(msg, n, "[\"AUTH\",%s]", ev);
  free(ev);
  bool sent = frame_out(r, 0x1, msg, strlen(msg));
  free(msg);
  if (!sent) { fail(err, cap, "auth: cut off"); return false; }
  long long deadline = now_ms() + WAIT_MS;
  for (;;) {
    char *m = message_in(r, deadline), word[16], got[80], ok[8];
    if (!m) { fail(err, cap, "auth: no answer"); return false; }
    bool mine = field(m, 0, word, sizeof word) && !strcmp(word, "OK") && field(m, 1, got, sizeof got) && !strcmp(got, id);
    bool yes = mine && field(m, 2, ok, sizeof ok) && !strcmp(ok, "true");
    if (mine && !yes) { char why[200]; field(m, 3, why, sizeof why); snprintf(err, cap, "auth refused: %s", why); }
    free(m);
    if (mine) { r->authed = yes; return yes; }
  }
}

relay *relay_open(const char *url, const unsigned char sk[32], char *err, size_t cap) {
  bool tls = !strncmp(url, "wss://", 6);
  if (!tls && strncmp(url, "ws://", 5)) { fail(err, cap, "not a ws:// or wss:// url"); return NULL; }
  const char *hp = url + (tls ? 6 : 5);
  char host[256], port[8], path[256];
  size_t hn = strcspn(hp, ":/");
  if (!hn || hn >= sizeof host) { fail(err, cap, "no host"); return NULL; }
  memcpy(host, hp, hn); host[hn] = 0;
  const char *rest = hp + hn;
  snprintf(port, sizeof port, "%s", tls ? "443" : "80");
  if (*rest == ':') { size_t pn = strcspn(rest + 1, "/"); if (pn == 0 || pn >= sizeof port) { fail(err, cap, "bad port"); return NULL; } memcpy(port, rest + 1, pn); port[pn] = 0; rest += 1 + pn; }
  snprintf(path, sizeof path, "%s", *rest ? rest : "/");
  signal(SIGPIPE, SIG_IGN);   // a relay that hangs up is an error here, never the end of this process
  struct addrinfo hints = {0}, *ai = NULL;
  hints.ai_socktype = SOCK_STREAM;
  if (getaddrinfo(host, port, &hints, &ai) != 0 || !ai) { fail(err, cap, "relay not found"); return NULL; }
  int fd = -1;
  for (struct addrinfo *a = ai; a; a = a->ai_next) {
    fd = socket(a->ai_family, a->ai_socktype, a->ai_protocol);
    if (fd >= 0 && connect(fd, a->ai_addr, a->ai_addrlen) == 0) break;
    if (fd >= 0) close(fd);
    fd = -1;
  }
  freeaddrinfo(ai);
  if (fd < 0) { fail(err, cap, "relay unreachable"); return NULL; }
  struct timeval tv = {15, 0};
  setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv); setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof tv);
  relay *r = calloc(1, sizeof *r);
  if (!r) { close(fd); return NULL; }
  r->fd = fd; memcpy(r->sk, sk, 32); snprintf(r->url, sizeof r->url, "%s", url);
  if (tls) {
    r->tls_ctx = SSL_CTX_new(TLS_client_method());
    if (r->tls_ctx) { SSL_CTX_set_default_verify_paths(r->tls_ctx); SSL_CTX_set_verify(r->tls_ctx, SSL_VERIFY_PEER, NULL); }
    r->tls = r->tls_ctx ? SSL_new(r->tls_ctx) : NULL;
    if (!r->tls || !SSL_set_tlsext_host_name(r->tls, host) || !SSL_set1_host(r->tls, host) || !SSL_set_fd(r->tls, fd) || SSL_connect(r->tls) != 1) {
      fail(err, cap, "relay: TLS failed"); relay_close(r); return NULL;
    }
  }
  unsigned char nonce[16]; unsigned char key[32];
  RAND_bytes(nonce, 16);
  EVP_EncodeBlock(key, nonce, 16);
  char req[1024];
  int n = snprintf(req, sizeof req, "GET %s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                   "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\nUser-Agent: oa-api\r\n\r\n", path, host, key);
  if (!put_all(r, (const unsigned char *)req, (size_t)n)) { fail(err, cap, "relay: cut off"); relay_close(r); return NULL; }
  long long deadline = now_ms() + WAIT_MS;
  char *end = NULL;
  while (!end) {
    if (!fill(r, (int)(deadline - now_ms() > 0 ? deadline - now_ms() : 0))) { fail(err, cap, "relay: no handshake"); relay_close(r); return NULL; }
    r->in[r->in_n < r->in_cap ? r->in_n : r->in_cap - 1] = 0;
    end = strstr((char *)r->in, "\r\n\r\n");
  }
  if (strncmp((char *)r->in, "HTTP/1.1 101", 12)) { fail(err, cap, "relay: refused the upgrade"); relay_close(r); return NULL; }
  size_t used = (size_t)(end + 4 - (char *)r->in);
  memmove(r->in, r->in + used, r->in_n - used); r->in_n -= used;
  // A relay that wants AUTH asks first (relay.openagents.com does, on connect).
  char *m = message_in(r, now_ms() + 1500), word[16];
  if (m && field(m, 0, word, sizeof word) && !strcmp(word, "AUTH")) field(m, 1, r->challenge, sizeof r->challenge);
  free(m);
  if (r->challenge[0] && !auth(r, err, cap)) { relay_close(r); return NULL; }
  return r;
}

bool relay_send(relay *r, const char *event_json, const char *id, char *err, size_t cap) {
  for (int attempt = 0; attempt < 2; attempt++) {
    size_t n = strlen(event_json) + 16;
    char *msg = malloc(n);
    if (!msg) return false;
    snprintf(msg, n, "[\"EVENT\",%s]", event_json);
    bool sent = frame_out(r, 0x1, msg, strlen(msg));
    free(msg);
    if (!sent) { fail(err, cap, "relay: cut off"); return false; }
    long long deadline = now_ms() + WAIT_MS;
    for (;;) {
      char *m = message_in(r, deadline), word[16], got[80], ok[8], why[240] = "";
      if (!m) { fail(err, cap, "relay: no OK"); return false; }
      if (field(m, 0, word, sizeof word) && !strcmp(word, "AUTH")) { field(m, 1, r->challenge, sizeof r->challenge); free(m); continue; }
      bool mine = !strcmp(word, "OK") && field(m, 1, got, sizeof got) && !strcmp(got, id);
      bool yes = mine && field(m, 2, ok, sizeof ok) && !strcmp(ok, "true");
      if (mine) field(m, 3, why, sizeof why);
      free(m);
      if (!mine) continue;
      // a duplicate is the relay saying it has it already
      if (yes || !strncmp(why, "duplicate:", 10)) return true;
      if (!strncmp(why, "auth-required:", 14) && attempt == 0 && r->challenge[0] && auth(r, err, cap)) break;
      snprintf(err, cap, "relay refused: %s", why);
      return false;
    }
  }
  return false;
}

// The raw text of a top-level array message's n-th element (an object or array): where it starts and its length.
static bool element(const char *msg, int n, const char **at, size_t *len) {
  int depth = 0, k = 0; bool in_str = false; const char *start = NULL;
  for (const char *p = msg; *p; p++) {
    if (in_str) { if (*p == '\\' && p[1]) p++; else if (*p == '"') in_str = false; continue; }
    if (*p == '"') in_str = true;
    else if (*p == '[' || *p == '{') { if (depth == 1 && k == n && !start) start = p; depth++; }
    else if (*p == ']' || *p == '}') { depth--; if (depth == 1 && start) { *at = start; *len = (size_t)(p + 1 - start); return true; } }
    else if (*p == ',' && depth == 1) k++;
  }
  return false;
}

char *relay_query(relay *r, const char *filter_json, char *err, size_t cap) {
  size_t n = strlen(filter_json) + 32;
  char *msg = malloc(n);
  if (!msg) return NULL;
  snprintf(msg, n, "[\"REQ\",\"q\",%s]", filter_json);
  bool sent = frame_out(r, 0x1, msg, strlen(msg));
  free(msg);
  if (!sent) { fail(err, cap, "relay: cut off"); return NULL; }
  size_t out_n = 1, out_cap = 4096;
  char *out = malloc(out_cap);
  if (!out) return NULL;
  out[0] = '[';
  long long deadline = now_ms() + WAIT_MS;
  for (int guard = 0; guard < 20000; guard++) {
    char *m = message_in(r, deadline), word[16], sub[16];
    if (!m) { fail(err, cap, "relay: no EOSE"); free(out); return NULL; }
    field(m, 0, word, sizeof word);
    bool ours = field(m, 1, sub, sizeof sub) && !strcmp(sub, "q");
    const char *ev; size_t len;
    if (ours && !strcmp(word, "EVENT") && element(m, 2, &ev, &len)) {
      if (out_n + len + 3 > out_cap) {
        while (out_n + len + 3 > out_cap) out_cap *= 2;
        char *grown = out_cap <= (64u << 20) ? realloc(out, out_cap) : NULL;
        if (!grown) { free(m); free(out); fail(err, cap, "relay: too much"); return NULL; }
        out = grown;
      }
      if (out_n > 1) out[out_n++] = ',';
      memcpy(out + out_n, ev, len); out_n += len;
    }
    bool end = ours && (!strcmp(word, "EOSE") || !strcmp(word, "CLOSED"));
    if (!strcmp(word, "AUTH")) field(m, 1, r->challenge, sizeof r->challenge);
    free(m);
    if (end) break;
  }
  frame_out(r, 0x1, "[\"CLOSE\",\"q\"]", 13);
  out[out_n++] = ']'; out[out_n] = 0;
  return out;
}

void relay_close(relay *r) {
  if (!r) return;
  if (r->tls) { SSL_shutdown(r->tls); SSL_free(r->tls); }
  if (r->tls_ctx) SSL_CTX_free(r->tls_ctx);
  if (r->fd >= 0) close(r->fd);
  memset(r->sk, 0, 32);
  free(r->in); free(r);
}
