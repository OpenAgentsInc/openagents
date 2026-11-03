// Private human packs for every visitor (docs/verse/metahuman-support.md, "Sending packs to
// browsers"): MetaHuman Lab's figure is shown on openagents.com, and its pack is never a file
// anyone can save and pass on. The pack is a compiled runtime format (VPK1, pack_human.py), never
// DNA or source meshes. verse/tools/humans/upload_packs.sh gzips it, encrypts it with a key of its
// own (AES-256-CTR) and puts it in the private bucket by its digest, packs/<sha256>; the keys and
// the names live in Secret Manager (verse-private-packs), never in git.
//
//   POST /api/verse/packs/<name>        a session: {url, key, iv, sha256, bytes, size, expires}
//                                       (404 no such pack, 429 too many for this address or the day's
//                                       budget spent, 503 the manifest or the database unreachable)
//   GET  /api/verse/packs/fetch/<sid>   the session's bytes, once, within EXPIRES seconds of being
//                                       asked for: the gzipped pack under the session's own key
//                                       (AES-256-CTR, the key and iv the session gave), chunked,
//                                       Cache-Control no-store. 410 used, expired or unknown.
//
// A child streams the bytes (this service answers one request at a time): it reads the stored
// object from the bucket, takes off the stored key and puts on the session's, a chunk at a time,
// so the plain pack is never on disk here and never leaves as itself. The session key is
// HMAC-SHA256(master, "key:" SID) (iv: the first 8 bytes of HMAC(master, "iv:" SID), then 8 zero
// bytes, so WebCrypto's 64-bit counter and OpenSSL's agree), so no key is stored per session.
//
// Limits (shown to the page as the 429's reason): OA_VERSE_PACK_PER_HOUR sessions an address an
// hour (default 12), OA_VERSE_PACK_DAILY_GB gigabytes fetched a UTC day across everyone (default
// 40, about $4.80 of egress at most). The manifest (Secret Manager, read again after 60 s):
//   master HEX64
//   pack NAME SHA256 PLAIN_BYTES STORED_BYTES OBJECT KEY_HEX64 IV_HEX32
// OA_VERSE_PACKS gives it inline (the tests); OA_VERSE_PACKS_SECRET names another secret;
// OA_VERSE_PACKS_GCS another storage endpoint; OA_VERSE_PACKS_BUCKET another bucket.
#include "api.h"
#include <ctype.h>
#include <curl/curl.h>
#include <errno.h>
#include <openssl/evp.h>
#include <openssl/hmac.h>
#include <openssl/rand.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define PACKS_MAX 32
#define SESSION_SECONDS 120
typedef struct { char name[65], sha256[65], object[200]; long long plain, stored; unsigned char key[32], iv[16]; } pack_entry;
static struct { unsigned char master[32]; bool has_master; pack_entry packs[PACKS_MAX]; int n; double at; } manifest;

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static bool unhex(const char *s, unsigned char *out, size_t n) {
  if (strlen(s) != 2 * n) return false;
  for (size_t i = 0; i < n; i++) {
    unsigned v = 0;
    if (sscanf(s + 2 * i, "%2x", &v) != 1 || !isxdigit((unsigned char)s[2 * i]) || !isxdigit((unsigned char)s[2 * i + 1])) return false;
    out[i] = (unsigned char)v;
  }
  return true;
}
static void tohex(const unsigned char *in, size_t n, char *out) { for (size_t i = 0; i < n; i++) snprintf(out + 2 * i, 3, "%02x", in[i]); }
static bool name_ok(const char *s) {
  size_t n = strlen(s);
  if (!n || n > 64) return false;
  for (; *s; s++) if (!((*s >= 'a' && *s <= 'z') || (*s >= '0' && *s <= '9') || *s == '-')) return false;
  return true;
}

typedef struct { char *p; size_t n, cap; } buf;
static size_t take(char *data, size_t size, size_t n, void *user) {
  buf *b = user; size_t k = size * n;
  if (b->n + k + 1 > b->cap) {
    size_t cap = (b->n + k + 1) * 2;
    if (cap > (1u << 20)) return 0;
    char *p = realloc(b->p, cap);
    if (!p) return 0;
    b->p = p; b->cap = cap;
  }
  memcpy(b->p + b->n, data, k); b->n += k; b->p[b->n] = 0;
  return k;
}

// ---- the manifest
static void parse(const char *text) {
  manifest.n = 0; manifest.has_master = false;
  for (const char *line = text; line && *line; ) {
    const char *end = strchr(line, '\n');
    size_t len = end ? (size_t)(end - line) : strlen(line);
    char l[1024];
    if (len < sizeof l) {
      memcpy(l, line, len); l[len] = 0;
      char a[16], b[600], c[600], d[600], e[600], f[600], g[600], h[600];
      if (sscanf(l, "%15s %599s", a, b) == 2 && !strcmp(a, "master")) manifest.has_master = unhex(b, manifest.master, 32);
      else if (sscanf(l, "%15s %599s %599s %599s %599s %599s %599s %599s", a, b, c, d, e, f, g, h) == 8 && !strcmp(a, "pack") && manifest.n < PACKS_MAX) {
        pack_entry *p = &manifest.packs[manifest.n];
        if (name_ok(b) && strlen(c) == 64 && strlen(f) < sizeof p->object && unhex(g, p->key, 32) && unhex(h, p->iv, 16)) {
          snprintf(p->name, sizeof p->name, "%s", b); snprintf(p->sha256, sizeof p->sha256, "%s", c);
          p->plain = atoll(d); p->stored = atoll(e); snprintf(p->object, sizeof p->object, "%s", f);
          if (p->plain > 0 && p->stored > 0) manifest.n++;
        }
      }
    }
    line = end ? end + 1 : NULL;
  }
}

// Secret Manager's latest version of the manifest, read with the service's own token.
static bool load(void) {
  const char *inline_text = getenv("OA_VERSE_PACKS");
  if (inline_text && *inline_text) { if (!manifest.at) { parse(inline_text); manifest.at = api_now_ms(); } return manifest.has_master; }
  if (manifest.at && api_now_ms() - manifest.at < 60000) return manifest.has_master;
  char tok[4096];
  if (!drops_token(tok, sizeof tok)) { api_log("verse packs: no token"); return manifest.has_master; }
  char url[512], auth[4200];
  snprintf(url, sizeof url, "https://secretmanager.googleapis.com/v1/%s/versions/latest:access",
           env_or("OA_VERSE_PACKS_SECRET", "projects/openagentsgemini/secrets/verse-private-packs"));
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  CURL *h = curl_easy_init();
  if (!h) return manifest.has_master;
  buf b = {0};
  struct curl_slist *hd = curl_slist_append(NULL, auth);
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 5L);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h);
  const char *d = status == 200 && b.p ? strstr(b.p, "\"data\": \"") : NULL;
  if (!d && status == 200 && b.p) d = strstr(b.p, "\"data\":\"");
  if (d) {
    d = strchr(d + 6, '"') + 1;
    const char *q = strchr(d, '"');
    size_t n = q ? (size_t)(q - d) : 0;
    unsigned char *text = n ? malloc(n + 4) : NULL;
    int k = text ? EVP_DecodeBlock(text, (const unsigned char *)d, (int)n) : -1;
    if (k > 0) {
      while (k && text[k - 1] == 0) k--;   // EVP_DecodeBlock counts the padding's bytes as zeros
      text[k] = 0; parse((const char *)text); manifest.at = api_now_ms();
      OPENSSL_cleanse(text, (size_t)k);
    }
    free(text);
  } else api_log("verse packs: manifest unread (%ld)", status);
  if (b.p) { OPENSSL_cleanse(b.p, b.n); free(b.p); }
  return manifest.has_master;
}

static const pack_entry *find(const char *name) {
  for (int i = 0; i < manifest.n; i++) if (!strcmp(manifest.packs[i].name, name)) return &manifest.packs[i];
  return NULL;
}

// The session's key and iv, from the master and its id.
static void session_key(const char *sid, unsigned char key[32], unsigned char iv[16]) {
  char msg[80]; unsigned int n = 32; unsigned char mac[32];
  snprintf(msg, sizeof msg, "key:%s", sid);
  HMAC(EVP_sha256(), manifest.master, 32, (const unsigned char *)msg, strlen(msg), key, &n);
  snprintf(msg, sizeof msg, "iv:%s", sid);
  HMAC(EVP_sha256(), manifest.master, 32, (const unsigned char *)msg, strlen(msg), mac, &n);
  memcpy(iv, mac, 8); memset(iv + 8, 0, 8);
}

// The visitor's address: the load balancer's next to last (access.c's rule).
static void visitor(const request *q, char ip[128]) {
  char xff[512];
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  snprintf(ip, 128, "%s", np ? parts[np >= 2 ? np - 2 : 0] : "local");
}

static void issue(const request *q, const char *name) {
  if (!load()) { api_json(q->fd, 503, "{\"error\":\"packs unavailable\"}"); return; }
  const pack_entry *p = name_ok(name) ? find(name) : NULL;
  if (!p) { api_json(q->fd, 404, "{\"error\":\"no such pack\"}"); return; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  char ip[128], per_hour[24], daily[32], stored[24];
  visitor(q, ip);
  snprintf(per_hour, sizeof per_hour, "%d", atoi(env_or("OA_VERSE_PACK_PER_HOUR", "12")));
  snprintf(daily, sizeof daily, "%lld", (long long)(atof(env_or("OA_VERSE_PACK_DAILY_GB", "40")) * 1e9));
  snprintf(stored, sizeof stored, "%lld", p->stored);
  unsigned char raw[16]; char sid[33];
  if (RAND_bytes(raw, sizeof raw) != 1) { api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return; }
  tohex(raw, sizeof raw, sid);
  const char *v[6] = {sid, ip, name, stored, per_hour, daily};
  PGresult *r = PQexecParams(db, "SELECT verse.pack_issue($1, $2, $3, $4::bigint, $5::int, $6::bigint)", 6, NULL, v, NULL, NULL, 0);
  const char *said = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? PQgetvalue(r, 0, 0) : NULL;
  if (!said) { api_log("verse packs: %s", PQerrorMessage(db)); PQclear(r); api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  if (strcmp(said, "ok")) {
    char json[200];
    snprintf(json, sizeof json, "{\"error\":\"%s\"}", !strcmp(said, "hour") ? "too many for this address this hour" : "the day's pack budget is spent");
    PQclear(r);
    api_json(q->fd, 429, json);
    return;
  }
  PQclear(r);
  unsigned char key[32], iv[16]; char kh[65], ivh[33], json[700];
  session_key(sid, key, iv);
  tohex(key, 32, kh); tohex(iv, 16, ivh);
  snprintf(json, sizeof json, "{\"url\":\"/api/verse/packs/fetch/%s\",\"key\":\"%s\",\"iv\":\"%s\",\"sha256\":\"%s\",\"bytes\":%lld,\"size\":%lld,\"expires\":%d}",
           sid, kh, ivh, p->sha256, p->plain, p->stored, SESSION_SECONDS);
  OPENSSL_cleanse(key, sizeof key); OPENSSL_cleanse(kh, sizeof kh);
  api_json(q->fd, 200, json);
}

// ---- the bytes, in a child
typedef struct { int fd; EVP_CIPHER_CTX *rest, *session; bool started, broken; long long sent; CURL *h; } stream;
static void send_all(int fd, const void *data, size_t n, bool *broken) {
  const char *p = data;
  while (n && !*broken) {
    ssize_t k = send(fd, p, n, 0);
    if (k < 0) { if (errno == EINTR) continue; *broken = true; return; }
    p += k; n -= (size_t)k;
  }
}
static size_t relay(char *data, size_t size, size_t n, void *user) {
  stream *s = user; size_t k = size * n;
  if (!s->started) {
    long status = 0; curl_easy_getinfo(s->h, CURLINFO_RESPONSE_CODE, &status);
    if (status != 200) return 0;
    const char *head = "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\n"
                       "Cache-Control: no-store, private\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n";
    send_all(s->fd, head, strlen(head), &s->broken);
    s->started = true;
  }
  unsigned char *a = malloc(k + 32), *b = malloc(k + 32);
  int la = 0, lb = 0;
  bool ok = a && b && EVP_EncryptUpdate(s->rest, a, &la, (const unsigned char *)data, (int)k) == 1 &&
            EVP_EncryptUpdate(s->session, b, &lb, a, la) == 1;
  if (ok && lb > 0) {
    char size_line[24]; int m = snprintf(size_line, sizeof size_line, "%x\r\n", lb);
    send_all(s->fd, size_line, (size_t)m, &s->broken);
    send_all(s->fd, b, (size_t)lb, &s->broken);
    send_all(s->fd, "\r\n", 2, &s->broken);
    s->sent += lb;
  }
  if (a) { OPENSSL_cleanse(a, k + 32); free(a); }
  free(b);
  return ok && !s->broken ? k : 0;
}

static void stream_pack(int fd, const pack_entry *p, const char *sid) {
  unsigned char key[32], iv[16];
  session_key(sid, key, iv);
  stream s = {.fd = fd, .rest = EVP_CIPHER_CTX_new(), .session = EVP_CIPHER_CTX_new()};
  EVP_EncryptInit_ex(s.rest, EVP_aes_256_ctr(), NULL, p->key, p->iv);
  EVP_EncryptInit_ex(s.session, EVP_aes_256_ctr(), NULL, key, iv);
  OPENSSL_cleanse(key, sizeof key);
  char tok[4096], url[1024], auth[4200];
  bool have = drops_token(tok, sizeof tok);
  char object[600]; size_t o = 0;   // the object's name, percent-encoded as the JSON API wants (its / too)
  for (const char *c = p->object; *c && o + 4 < sizeof object; c++)
    if (isalnum((unsigned char)*c) || *c == '-' || *c == '_' || *c == '.') object[o++] = *c;
    else o += (size_t)snprintf(object + o, sizeof object - o, "%%%02X", (unsigned char)*c);
  object[o] = 0;
  snprintf(url, sizeof url, "%s/storage/v1/b/%s/o/%s?alt=media", env_or("OA_VERSE_PACKS_GCS", "https://storage.googleapis.com"),
           env_or("OA_VERSE_PACKS_BUCKET", "openagentsgemini-verse-private-assets"), object);
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  CURL *h = have ? curl_easy_init() : NULL;
  long status = 0;
  if (h) {
    s.h = h;
    struct curl_slist *hd = curl_slist_append(NULL, auth);
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, relay);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, &s);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 600L);
    CURLcode rc = curl_easy_perform(h);
    curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    if (rc != CURLE_OK && !s.started) api_log("verse packs: %s: %s (%ld)", p->name, curl_easy_strerror(rc), status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
  }
  if (!s.started) api_json(fd, 502, "{\"error\":\"the pack could not be read\"}");
  else if (s.sent == p->stored && !s.broken) send_all(fd, "0\r\n\r\n", 5, &s.broken);
  else api_log("verse packs: %s cut off after %lld of %lld bytes", p->name, s.sent, p->stored);   // no final chunk: the page sees a broken body
  EVP_CIPHER_CTX_free(s.rest); EVP_CIPHER_CTX_free(s.session);
}

static void fetch(const request *q, const char *sid) {
  bool hex = strlen(sid) == 32;
  for (const char *c = sid; hex && *c; c++) hex = (*c >= '0' && *c <= '9') || (*c >= 'a' && *c <= 'f');
  if (!hex) { api_json(q->fd, 410, "{\"error\":\"no such session\"}"); return; }
  if (!load()) { api_json(q->fd, 503, "{\"error\":\"packs unavailable\"}"); return; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  char secs[12]; snprintf(secs, sizeof secs, "%d", SESSION_SECONDS);
  const char *v[2] = {sid, secs};
  PGresult *r = PQexecParams(db, "SELECT verse.pack_fetch($1, $2::int)", 2, NULL, v, NULL, NULL, 0);
  const char *name = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) ? PQgetvalue(r, 0, 0) : NULL;
  const pack_entry *p = name ? find(name) : NULL;
  PQclear(r);
  if (!p) { api_json(q->fd, 410, "{\"error\":\"no such session\"}"); return; }
  signal(SIGCHLD, SIG_IGN);   // children are never waited for
  pid_t pid = fork();
  if (pid < 0) { api_json(q->fd, 503, "{\"error\":\"busy\"}"); return; }
  if (pid == 0) { stream_pack(q->fd, p, sid); _exit(0); }
}

bool verse_packs_route(const request *q) {
  static const char base[] = "/api/verse/packs/", fetch_at[] = "/api/verse/packs/fetch/";
  if (strncmp(q->path, base, sizeof base - 1)) return false;
  if (!strncmp(q->path, fetch_at, sizeof fetch_at - 1)) {
    if (strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET\"}"); return true; }
    fetch(q, q->path + sizeof fetch_at - 1);
    return true;
  }
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST\"}"); return true; }
  issue(q, q->path + sizeof base - 1);
  return true;
}
