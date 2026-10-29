// Drops for a new project (rc-drop-zone; docs/projects/new-project.md Part 2):
// the files someone drops on /projects (NEW PROJECT), kept in a private
// bucket until a project made from them is published. The rules, the limits
// and the scan are SQL's (migrations/0016_project_drops.sql); this file reads
// who is asking and does the bucket's part.
//
//   GET  /api/drops/me                       {pubkey, may, used, limits}, else 401
//   POST /api/drops/signin                   NIP-98 signed: the oa_drops session cookie (Path=/api/drops)
//   POST /api/drops/signout                  ends it
//   GET  /api/drops                          the signed-in person's drops
//   POST /api/drops                          a new drop -> {id}
//   GET  /api/drops/<d>                      its files {id, name, kind, bytes, status, note}
//   POST /api/drops/<d>/files {name, bytes}  admitted: an upload is opened in the bucket for exactly that
//                                            many bytes of our content type, and its session URL comes back
//                                            {file, upload}; the browser PUTs the file there (CORS for this
//                                            site's origin), then says it's done
//   POST /api/drops/<d>/files/<f>/done       the scan: the bucket's size and first 4 KiB, judged by
//                                            drops.scan; a refused file's object is deleted -> {file}
//   POST /api/drops/<d>/draft                an agent reads the drop's kept files and drafts the project
//                                            (rc-read-drop, 0017_project_drafts.sql): Gemini on Vertex AI
//                                            reads them from the bucket; the answer waits for the draft
//                                            -> {status: ready|failed, note, draft}
//   GET  /api/drops/<d>/draft                the last draft {status: none|reading|ready|failed, ...}
//   POST /api/drops/<d>/draft/edit           the owner changes one field {path, value} (rc-review,
//                                            0020_project_review.sql) -> the draft
//   POST /api/drops/<d>/publish              PUBLISH: the reviewed draft is frozen, then published (rc-publish,
//                                            0038_project_publish.sql): the project's key made and sealed, its
//                                            events on the relay, its files and page in the site's bucket, so it
//                                            is live at /projects/<slug> and listed on /projects with no deploy
//                                            -> the draft and {published: {slug, page} | note}
//   POST /api/drops/<d>/discard              DISCARD: the drop's files leave the bucket, its draft is gone
//
// Who asks: a NIP-98 signature (each event once), else the oa_drops cookie,
// else the admin sign-in's cookie (admin.c sets it for /api/drops too). A
// write comes from this site's pages (Origin) or is signed. Nothing anyone
// dropped is ever logged: no names, sessions or bytes.
//   OA_DROPS_BUCKET  the bucket (openagentsgemini-project-drops)
//   OA_DROPS_PREFIX  this environment's folder in it (staging, production; default dev)
//   OA_DROPS_GCS     the storage API's origin (tests point it at a fake)
//   OA_DROPS_TOKEN   a token to send instead of the metadata server's (tests)
//   OA_DRAFT_MODEL   the drafting model (gemini-3.8-flash, on the global endpoint)
//   OA_DRAFT_URL     its generateContent URL (tests point it at a fake)
#include "api.h"
#include "nostr.h"
#include "relay.h"
#include <ctype.h>
#include <curl/curl.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>
#include <unistd.h>

#define PREFIX "/api/drops"
#define COOKIE "oa_drops"
#define SESSION_DAYS 7

static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }
static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

typedef struct { char *p; size_t n, cap; } buf;
static size_t take(char *data, size_t size, size_t n, void *user) {
  buf *b = user; size_t k = size * n;
  if (b->n + k + 1 > b->cap) {
    size_t cap = (b->n + k + 1) * 2;
    if (cap > (1u << 22)) return 0;
    char *p = realloc(b->p, cap);
    if (!p) return 0;
    b->p = p; b->cap = cap;
  }
  memcpy(b->p + b->n, data, k); b->n += k; b->p[b->n] = 0;
  return k;
}

// ---- the bucket
static bool token(char *out, size_t cap) {
  const char *given = getenv("OA_DROPS_TOKEN");
  if (given && *given) { snprintf(out, cap, "%s", given); return true; }
  CURL *h = curl_easy_init();
  if (!h) return false;
  buf b = {0};
  struct curl_slist *hd = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(h, CURLOPT_URL, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token");
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 5L);
  bool ok = curl_easy_perform(h) == CURLE_OK && b.p;
  out[0] = 0;
  const char *t = ok ? strstr(b.p, "\"access_token\":\"") : NULL;
  if (t) {
    t += strlen("\"access_token\":\"");
    size_t n = 0;
    while (t[n] && t[n] != '"' && n + 1 < cap) { out[n] = t[n]; n++; }
    out[n] = 0;
  }
  curl_slist_free_all(hd); curl_easy_cleanup(h); free(b.p);
  return out[0] != 0;
}

// An object name for a URL path or query.
static void enc(char *out, size_t cap, const char *s) {
  size_t n = 0;
  for (; *s && n + 4 < cap; s++) {
    unsigned char c = (unsigned char)*s;
    if (isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~') out[n++] = (char)c;
    else n += (size_t)snprintf(out + n, cap - n, "%%%02X", c);
  }
  out[n] = 0;
}

typedef struct { char location[2048]; } heads;
static size_t take_head(char *data, size_t size, size_t n, void *user) {
  heads *h = user; size_t k = size * n;
  if (k > 9 && !strncasecmp(data, "Location:", 9)) {
    const char *v = data + 9; size_t len = k - 9;
    while (len && (*v == ' ' || *v == '\t')) { v++; len--; }
    while (len && (v[len - 1] == '\r' || v[len - 1] == '\n')) len--;
    if (len < sizeof h->location) { memcpy(h->location, v, len); h->location[len] = 0; }
  }
  return k;
}

// One call to the storage API; its status, and its body into b when given.
static long gcs(const char *method, const char *url, const char *tok, struct curl_slist *extra, buf *b, heads *hd) {
  CURL *h = curl_easy_init();
  if (!h) return 0;
  char auth[4200]; snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  struct curl_slist *list = curl_slist_append(extra, auth);
  buf sink = {0};
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_CUSTOMREQUEST, method);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, list);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, b ? b : &sink);
  if (hd) { curl_easy_setopt(h, CURLOPT_HEADERFUNCTION, take_head); curl_easy_setopt(h, CURLOPT_HEADERDATA, hd); }
  if (!strcmp(method, "POST")) { curl_easy_setopt(h, CURLOPT_POSTFIELDS, ""); curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, 0L); }
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 20L);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(list); curl_easy_cleanup(h); free(sink.p);
  return status;
}

// Opens a resumable upload for exactly bytes of type at object; its session URL into out.
static bool open_upload(const char *object, const char *type, const char *bytes, const char *from, char *out, size_t cap) {
  char tok[4096]; out[0] = 0;
  if (!token(tok, sizeof tok)) { api_log("drops: no token"); return false; }
  char name[1024], url[2048], h1[512], h2[128], h3[600];
  enc(name, sizeof name, object);
  snprintf(url, sizeof url, "%s/upload/storage/v1/b/%s/o?uploadType=resumable&ifGenerationMatch=0&name=%s",
           env_or("OA_DROPS_GCS", "https://storage.googleapis.com"), env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops"), name);
  snprintf(h1, sizeof h1, "X-Upload-Content-Type: %s", type);
  snprintf(h2, sizeof h2, "X-Upload-Content-Length: %s", bytes);
  snprintf(h3, sizeof h3, "Origin: %s", from);   // the session answers that origin's browser (CORS)
  struct curl_slist *x = curl_slist_append(curl_slist_append(curl_slist_append(NULL, h1), h2), h3);
  heads hd = {{0}};
  long status = gcs("POST", url, tok, x, NULL, &hd);
  if (status != 200 || !hd.location[0]) { api_log("drops: upload not opened (%ld)", status); return false; }
  snprintf(out, cap, "%s", hd.location);
  return true;
}

// What the bucket holds at object: its size (-1 none), MD5 and first 4 KiB as hex.
static void inspect(const char *object, long long *size, char *md5, size_t md5cap, char **head_hex) {
  *size = -1; md5[0] = 0; *head_hex = NULL;
  char tok[4096];
  if (!token(tok, sizeof tok)) return;
  char name[1024], url[2048];
  enc(name, sizeof name, object);
  const char *base = env_or("OA_DROPS_GCS", "https://storage.googleapis.com"), *bucket = env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops");
  snprintf(url, sizeof url, "%s/storage/v1/b/%s/o/%s?fields=size,md5Hash", base, bucket, name);
  buf b = {0};
  long status = gcs("GET", url, tok, NULL, &b, NULL);
  if (status == 200 && b.p) {
    const char *s = strstr(b.p, "\"size\"");
    if (s && (s = strchr(s + 6, '"'))) *size = strtoll(s + 1, NULL, 10);
    const char *m = strstr(b.p, "\"md5Hash\"");
    if (m && (m = strchr(m + 9, '"'))) { size_t n = 0; m++; while (m[n] && m[n] != '"' && n + 1 < md5cap) { md5[n] = m[n]; n++; } md5[n] = 0; }
  }
  free(b.p);
  if (*size <= 0) return;
  snprintf(url, sizeof url, "%s/storage/v1/b/%s/o/%s?alt=media", base, bucket, name);
  buf body = {0};
  status = gcs("GET", url, tok, curl_slist_append(NULL, "Range: bytes=0-4095"), &body, NULL);
  if ((status == 200 || status == 206) && body.p) {
    size_t n = body.n > 4096 ? 4096 : body.n;
    char *hex = malloc(n * 2 + 1);
    if (hex) { for (size_t i = 0; i < n; i++) snprintf(hex + 2 * i, 3, "%02x", (unsigned char)body.p[i]); hex[2 * n] = 0; }
    *head_hex = hex;
  } else *head_hex = strdup("");
  free(body.p);
}

static void delete_object(const char *object) {
  char tok[4096];
  if (!token(tok, sizeof tok)) return;
  char name[1024], url[2048];
  enc(name, sizeof name, object);
  snprintf(url, sizeof url, "%s/storage/v1/b/%s/o/%s", env_or("OA_DROPS_GCS", "https://storage.googleapis.com"),
           env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops"), name);
  long status = gcs("DELETE", url, tok, NULL, NULL, NULL);
  if (status != 204 && status != 200 && status != 404) api_log("drops: refused object not deleted (%ld)", status);
}

// ---- who asks
static bool first_use(PGconn *db, const nostr_event *ev) {
  const char *v[1] = {ev->id};
  PGresult *r = PQexecParams(db, "INSERT INTO drops.seen (id, ms) VALUES ($1, drops.now_ms()) ON CONFLICT DO NOTHING", 1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  return ok;
}

// A cookie's 64-hex-digit value, or false.
static bool cookie(const request *q, const char *name, char out[65]) {
  char cookies[4096]; out[0] = 0;
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return false;
  size_t len = strlen(name);
  for (const char *c = cookies; (c = strstr(c, name)); c += len) {
    if ((c != cookies && c[-1] != ' ' && c[-1] != ';') || c[len] != '=') continue;
    const char *v = c + len + 1; size_t n = 0;
    while (n < 64 && isxdigit((unsigned char)v[n])) { out[n] = v[n]; n++; }
    out[n] = 0;
    if (n == 64) return true;
  }
  return false;
}

// The signed-in pubkey (into pk) from a signature or a session; false: nobody.
static bool who(const request *q, PGconn *db, char pk[65], bool *signed_req) {
  char auth[4096], tok[65];
  pk[0] = 0; *signed_req = false;
  if (api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6)) {
    char url[4300]; snprintf(url, sizeof url, "%s%s", origin(), q->target);
    nostr_event ev; char err[160];
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err) || !first_use(db, &ev)) return false;
    memcpy(pk, ev.pubkey, 65);
    *signed_req = true;
    return true;
  }
  const char *sql[2] = {
    "SELECT pubkey FROM drops.sessions WHERE token_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex') AND expires_ms > drops.now_ms()",
    "SELECT pubkey FROM analytics.admin_sessions WHERE token_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex') AND expires_ms > drops.now_ms()"};
  const char *names[2] = {COOKIE, "oa_admin"};
  for (int k = 0; k < 2; k++) {
    if (!cookie(q, names[k], tok)) continue;
    const char *v[1] = {tok};
    PGresult *r = PQexecParams(db, sql[k], 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(pk, 65, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    if (pk[0] && (k == 0 || admin_is(pk))) return true;
    pk[0] = 0;
  }
  return false;
}

// Runs sql (params v) for a {status, body, ...} result; its status, body and the whole result.
typedef struct { int status; char *body; char *all; } answer;
static answer ask(PGconn *db, const char *sql, int n, const char *const *v) {
  answer a = {500, NULL, NULL};
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    a.status = atoi(PQgetvalue(r, 0, 0)); a.body = strdup(PQgetvalue(r, 0, 1)); a.all = strdup(PQgetvalue(r, 0, 2));
  } else {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    a.status = state && !strncmp(state, "22", 2) ? 400 : 500;
    api_log("drops: sql %s", state ? state : "?");
  }
  PQclear(r);
  return a;
}
static void reply(int fd, answer a) {
  if (a.body) api_json(fd, a.status, a.body);
  else api_json(fd, a.status, a.status == 400 ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
  free(a.body); free(a.all);
}
#define Q(fn) "SELECT (x->>'status')::int, (x->'body')::text, x::text FROM " fn " x"

// A field of a result's JSON (a string value), into out.
static bool str_of(const char *json, const char *key, char *out, size_t cap) {
  char pat[64]; snprintf(pat, sizeof pat, "\"%s\": \"", key);
  const char *s = json ? strstr(json, pat) : NULL;
  out[0] = 0;
  if (!s) return false;
  s += strlen(pat);
  size_t n = 0;
  while (s[n] && s[n] != '"' && n + 1 < cap) { out[n] = s[n]; n++; }
  out[n] = 0;
  return true;
}

static void signin(const request *q, PGconn *db, const char *pk) {
  unsigned char raw[32]; char tok[65];
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, raw, 32) != 32) { if (fd >= 0) close(fd); api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return; }
  close(fd);
  for (int i = 0; i < 32; i++) snprintf(tok + 2 * i, 3, "%02x", raw[i]);
  char days[8]; snprintf(days, sizeof days, "%d", SESSION_DAYS);
  const char *v[3] = {tok, pk, days};
  PGresult *r = PQexecParams(db, "INSERT INTO drops.sessions (token_sha256, pubkey, created_ms, expires_ms) "
                                 "VALUES (encode(sha256(convert_to($1, 'UTF8')), 'hex'), $2, drops.now_ms(), drops.now_ms() + $3::bigint * 86400000)",
                             3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  if (!ok) { api_json(q->fd, 503, "{\"error\":\"no session\"}"); return; }
  char headers[300], body[160];
  snprintf(headers, sizeof headers, "Set-Cookie: " COOKIE "=%s; Path=" PREFIX "; Max-Age=%d; HttpOnly; Secure; SameSite=Strict\r\n", tok, SESSION_DAYS * 86400);
  snprintf(body, sizeof body, "{\"pubkey\":\"%s\"}", pk);
  api_respond(q->fd, 200, "application/json", headers, body, strlen(body));
}

static void signout(const request *q, PGconn *db) {
  char tok[65];
  if (cookie(q, COOKIE, tok)) {
    const char *v[1] = {tok};
    PQclear(PQexecParams(db, "DELETE FROM drops.sessions WHERE token_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0));
  }
  const char *h = "Set-Cookie: " COOKIE "=; Path=" PREFIX "; Max-Age=0; HttpOnly; Secure; SameSite=Strict\r\n";
  api_respond(q->fd, 200, "application/json", h, "{}", 2);
}

// The bucket's part takes a second or more and this service answers one
// request at a time: a child does it, with its own connection, and answers.
static bool in_child(void) {
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid < 0) return false;
  return pid == 0;
}

static void add_file(const request *q, PGconn *db, const char *pk, bool admin, const char *drop) {
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *v[5] = {pk, admin ? "true" : "false", drop, body, env_or("OA_DROPS_PREFIX", "dev")};
  answer a = ask(db, Q("drops.admit($1, $2::bool, $3, $4::jsonb, $5)"), 5, v);
  free(body);
  if (a.status != 200) { reply(q->fd, a); return; }
  char object[600], type[200], file[40], bytes[24] = "", from[256];
  str_of(a.all, "object", object, sizeof object); str_of(a.all, "type", type, sizeof type); str_of(a.body, "file", file, sizeof file);
  const char *b = strstr(a.all, "\"bytes\": ");
  if (b) snprintf(bytes, sizeof bytes, "%lld", strtoll(b + 9, NULL, 10));
  if (!api_header(q, "Origin", from, sizeof from) || strcmp(from, origin())) snprintf(from, sizeof from, "%s", origin());
  free(a.body); free(a.all);
  if (!in_child()) return;   // the parent goes on; the child answers
  char session[2048];
  bool opened = open_upload(object, type, bytes, from, session, sizeof session);
  PGconn *c = api_db_new();
  if (!c) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); _exit(0); }
  const char *w[2] = {file, opened ? session : ""};
  reply(q->fd, ask(c, Q("drops.started($1, nullif($2, ''))"), 2, w));
  PQfinish(c);
  _exit(0);
}

static void file_done(const request *q, PGconn *db, const char *pk, const char *drop, const char *file) {
  const char *v[3] = {pk, drop, file};
  answer a = ask(db, Q("drops.finishing($1, $2, $3)"), 3, v);
  char object[600];
  if (a.status != 200 || !str_of(a.all, "object", object, sizeof object)) { reply(q->fd, a); return; }
  free(a.body); free(a.all);
  if (!in_child()) return;
  long long size; char md5[64], sz[24]; char *head = NULL;
  inspect(object, &size, md5, sizeof md5, &head);
  snprintf(sz, sizeof sz, "%lld", size);
  PGconn *c = api_db_new();
  if (!c) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); free(head); _exit(0); }
  const char *w[4] = {file, sz, md5, head ? head : ""};
  answer s = ask(c, Q("drops.scan($1, $2::bigint, nullif($3, ''), decode($4, 'hex'))"), 4, w);
  if (s.all && strstr(s.all, "\"delete\": true")) delete_object(object);
  reply(q->fd, s);
  PQfinish(c); free(head);
  _exit(0);
}

// In the child: asks the model for the draft, keeps it, answers.
static void draft_child(int fd, const char *drop, const char *request) {
  const char *model = env_or("OA_DRAFT_MODEL", "gemini-3.8-flash");
  char url[512], tok[4096], auth[4200], http[16], took[16];
  const char *given = getenv("OA_DRAFT_URL");
  if (given && *given) snprintf(url, sizeof url, "%s", given);
  else snprintf(url, sizeof url, "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/publishers/google/models/%s:generateContent", model);
  long status = 0;
  buf b = {0};
  double t0 = api_now_ms();
  if (token(tok, sizeof tok)) {
    CURL *h = curl_easy_init();
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, request);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 280L);
    if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
  } else api_log("drops: no token for the model");
  if (status != 200) api_log("drops: the model answered %ld", status);
  snprintf(http, sizeof http, "%ld", status);
  snprintf(took, sizeof took, "%d", (int)(api_now_ms() - t0));
  PGconn *c = api_db_new();
  if (!c) { api_json(fd, 503, "{\"error\":\"no database\"}"); free(b.p); return; }
  const char *v[5] = {drop, http, b.p ? b.p : "", model, took};
  reply(fd, ask(c, Q("drops.draft_done($1, $2::int, $3, $4, $5::int)"), 5, v));
  PQfinish(c); free(b.p);
}

static void draft(const request *q, PGconn *db, const char *pk, const char *drop) {
  const char *v[3] = {pk, drop, env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops")};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text, coalesce((x->'request')::text, '') FROM drops.draft_start($1, $2, $3) x",
                             3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("drops: draft sql"); PQclear(r); api_json(q->fd, 500, "{\"error\":\"not kept\"}"); return; }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) { api_json(q->fd, status, PQgetvalue(r, 0, 1)); PQclear(r); return; }
  char *request = strdup(PQgetvalue(r, 0, 2));
  PQclear(r);
  if (in_child()) { draft_child(q->fd, drop, request); free(request); _exit(0); }
  free(request);
}

// A call that takes the request's JSON body as $3.
static void body_call(const request *q, PGconn *db, const char *pk, const char *drop, const char *sql) {
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  reply(q->fd, ask(db, sql, 3, (const char *[]){pk, drop, body}));
  free(body);
}

// DISCARD: the rules forget the drop; a child takes its files out of the bucket, then answers.
static void discard(const request *q, PGconn *db, const char *pk, const char *drop) {
  answer a = ask(db, Q("drops.discard($1, $2)"), 2, (const char *[]){pk, drop});
  if (a.status != 200) { reply(q->fd, a); return; }
  PGresult *r = PQexecParams(db, "SELECT jsonb_array_elements_text(($1::jsonb)->'objects')", 1, NULL, (const char *[]){a.all}, NULL, NULL, 0);
  if (in_child()) {
    if (PQresultStatus(r) == PGRES_TUPLES_OK) for (int i = 0; i < PQntuples(r); i++) delete_object(PQgetvalue(r, i, 0));
    reply(q->fd, a);
    _exit(0);
  }
  PQclear(r); free(a.body); free(a.all);
}

// ---- publishing (rc-publish, 0038_project_publish.sql): after PUBLISH froze the draft, the project's
// key is made and sealed, its events signed and sent, its files copied and its page written.
//   OA_PROJECT_KEK         64 hex digits that seal project keys (Secret Manager); without it nothing is published
//   OA_PUBLISH_RELAY       where the events go (wss://relay.openagents.com); without it nothing is published
//   OA_SITE_BUCKET         the site's bucket the page, files and lists go into
//   OA_PUBLISH_VISIBILITY  public (production) or private (staging: on the relay, never listed)
static bool publisher_on(void) { const char *k = getenv("OA_PROJECT_KEK"), *r = getenv("OA_PUBLISH_RELAY"); return k && strlen(k) == 64 && r && *r; }

// A call to the storage API with a body (POST); its status, and its answer into b.
static long gcs_body(const char *url, const char *tok, const char *type, const char *body, size_t n, buf *b) {
  CURL *h = curl_easy_init();
  if (!h) return 0;
  char auth[4200], ct[200];
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok); snprintf(ct, sizeof ct, "Content-Type: %s", type);
  struct curl_slist *list = curl_slist_append(curl_slist_append(NULL, auth), ct);
  buf sink = {0};
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, list);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, body);
  curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)n);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, b ? b : &sink);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 60L);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(list); curl_easy_cleanup(h); free(sink.p);
  return status;
}

// Writes name in the site's bucket (no-cache: a page, a list).
static bool site_put(const char *tok, const char *name, const char *type, const char *data) {
  const char *bucket = getenv("OA_SITE_BUCKET");
  if (!bucket || !*bucket) return false;
  size_t n = strlen(data), cap = n + 1024;
  char *body = malloc(cap), url[1024];
  if (!body) return false;
  int k = snprintf(body, cap, "--oa-site\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n"
                   "{\"name\":\"%s\",\"contentType\":\"%s\",\"cacheControl\":\"no-cache\"}\r\n--oa-site\r\nContent-Type: %s\r\n\r\n", name, type, type);
  memcpy(body + k, data, n);
  k += (int)n;
  k += snprintf(body + k, cap - (size_t)k, "\r\n--oa-site--\r\n");
  snprintf(url, sizeof url, "%s/upload/storage/v1/b/%s/o?uploadType=multipart", env_or("OA_DROPS_GCS", "https://storage.googleapis.com"), bucket);
  long status = gcs_body(url, tok, "multipart/related; boundary=oa-site", body, (size_t)k, NULL);
  free(body);
  if (status != 200) api_log("drops: site write %ld", status);
  return status == 200;
}

// Copies a kept file from the drops bucket into the site's (a rewrite: the bytes never pass through here).
static bool site_copy(const char *tok, const char *object, const char *dest) {
  const char *bucket = getenv("OA_SITE_BUCKET");
  if (!bucket || !*bucket) return false;
  char from[1024], to[1024], url[4096], rt[2048] = "";
  enc(from, sizeof from, object); enc(to, sizeof to, dest);
  for (int i = 0; i < 20; i++) {
    snprintf(url, sizeof url, "%s/storage/v1/b/%s/o/%s/rewriteTo/b/%s/o/%s%s%s", env_or("OA_DROPS_GCS", "https://storage.googleapis.com"),
             env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops"), from, bucket, to, rt[0] ? "?rewriteToken=" : "", rt);
    buf b = {0};
    long status = gcs_body(url, tok, "application/json", "{}", 2, &b);
    bool done = status == 200 && b.p && (strstr(b.p, "\"done\": true") || strstr(b.p, "\"done\":true"));
    bool more = status == 200 && !done && b.p && str_of(b.p, "rewriteToken", rt, sizeof rt);
    free(b.p);
    if (done) return true;
    if (!more) { api_log("drops: copy %ld", status); return false; }
  }
  return false;
}

// One text from SQL (params v), malloc'd; NULL on an error.
static char *sql_text(PGconn *c, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(c, sql, n, NULL, v, NULL, NULL, 0);
  char *out = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) ? strdup(PQgetvalue(r, 0, 0)) : NULL;
  PQclear(r);
  return out;
}

// In the child: the publishing itself, then the answer. Every step is kept, so a second PUBLISH goes on
// where a first one stopped.
static const char *publish_steps(PGconn *c, const char *slug, bool has_key) {
  const char *kek = getenv("OA_PROJECT_KEK");
  unsigned char sk[32]; char pub[65], sealed[200];
  if (!has_key) {
    if (!nostr_keygen(sk, pub) || !seal(kek, slug, sk, 32, sealed, sizeof sealed)) return "no key";
    char *kept = sql_text(c, "SELECT drops.publish_key($1, $2, $3)::text", 3, (const char *[]){slug, pub, sealed});
    bool ok = kept && !strcmp(kept, "true");
    free(kept);
    if (!ok) { memset(sk, 0, 32); return "the key wasn't kept"; }
  } else {
    char *s = sql_text(c, "SELECT drops.publish_sealed($1)", 1, (const char *[]){slug});
    bool ok = s && unseal(kek, slug, s, sk, 32);
    free(s);
    if (!ok) return "the key can't be opened";
  }
  relay *r = NULL;
  char err[300] = "";
  const char *why = NULL;
  for (int guard = 0; guard < 2000 && !why; guard++) {
    PGresult *q = PQexecParams(c, "SELECT x->>'seq', x->>'kind', x->>'tags', x->>'content' FROM drops.publish_next($1) x", 1, NULL,
                               (const char *[]){slug}, NULL, NULL, 0);
    if (PQresultStatus(q) != PGRES_TUPLES_OK || PQntuples(q) != 1) { PQclear(q); why = "sql"; break; }
    if (PQgetisnull(q, 0, 0)) { PQclear(q); break; }   // every event is sent
    char id[65], seq[16];
    snprintf(seq, sizeof seq, "%s", PQgetvalue(q, 0, 0));
    char *ev = nostr_sign(sk, atoi(PQgetvalue(q, 0, 1)), (long long)time(NULL), PQgetvalue(q, 0, 2), PQgetvalue(q, 0, 3), id);
    PQclear(q);
    if (!ev) { why = "not signed"; break; }
    if (!r && !(r = relay_open(getenv("OA_PUBLISH_RELAY"), sk, err, sizeof err))) { free(ev); why = "the relay"; break; }
    if (!relay_send(r, ev, id, err, sizeof err)) { free(ev); why = "the relay"; break; }
    char *kept = sql_text(c, "SELECT drops.publish_sent($1, $2::int, $3, $4)::text", 4, (const char *[]){slug, seq, id, ev});
    free(ev);
    if (!kept) why = "sql";
    free(kept);
  }
  relay_close(r);
  memset(sk, 0, 32);
  if (why) { if (err[0]) api_log("drops: publish: %s", err); return why; }
  free(sql_text(c, "SELECT drops.publish_step($1, 'sent')::text", 1, (const char *[]){slug}));
  char tok[4096];
  if (!token(tok, sizeof tok)) return "no token";
  PGresult *f = PQexecParams(c, "SELECT object, dest FROM drops.publish_files($1)", 1, NULL, (const char *[]){slug}, NULL, NULL, 0);
  bool copied = PQresultStatus(f) == PGRES_TUPLES_OK;
  for (int i = 0; copied && i < PQntuples(f); i++) copied = site_copy(tok, PQgetvalue(f, i, 0), PQgetvalue(f, i, 1));
  PQclear(f);
  if (!copied) return "the files";
  free(sql_text(c, "SELECT drops.publish_step($1, 'files')::text", 1, (const char *[]){slug}));
  free(sql_text(c, "SELECT drops.publish_step($1, 'listed')::text", 1, (const char *[]){slug}));
  char name[200];
  const char *pages[3][3] = {{"SELECT drops.published_slugs()", "projects/published-slugs.txt", "text/plain; charset=utf-8"},
                             {"SELECT drops.publish_page($1)", "index.html", "text/html; charset=utf-8"},
                             {"SELECT drops.publish_md($1)", "index.md", "text/markdown; charset=utf-8"}};
  for (int i = 0; i < 3; i++) {
    char *text = sql_text(c, pages[i][0], i ? 1 : 0, (const char *[]){slug});
    if (i) snprintf(name, sizeof name, "projects/%s/%s", slug, pages[i][1]); else snprintf(name, sizeof name, "%s", pages[i][1]);
    bool ok = text && site_put(tok, name, pages[i][2], text);
    free(text);
    if (!ok) return "the page";
  }
  char *rows = sql_text(c, "SELECT drops.published_rows()", 0, NULL);
  bool listed = rows && site_put(tok, "projects/published.txt", "text/plain; charset=utf-8", rows);
  free(rows);
  if (!listed) return "the list";
  free(sql_text(c, "SELECT drops.publish_step($1, 'done')::text", 1, (const char *[]){slug}));
  return NULL;
}

// PUBLISH: the reviewed draft is frozen (0020), then, when this service publishes, made public.
static void publish(const request *q, PGconn *db, const char *pk, const char *drop) {
  answer ap = ask(db, Q("drops.draft_approve($1, $2)"), 2, (const char *[]){pk, drop});
  if (!publisher_on() || (ap.status != 200 && ap.status != 409)) { reply(q->fd, ap); return; }
  answer b = ask(db, Q("drops.publish_begin($1, $2, $3)"), 3, (const char *[]){pk, drop, env_or("OA_PUBLISH_VISIBILITY", "public")});
  if (b.status != 200) { free(b.body); free(b.all); reply(q->fd, ap); return; }
  char slug[80], state[24];
  str_of(b.all, "slug", slug, sizeof slug);
  bool has_key = b.all && strstr(b.all, "\"has_key\": true");
  bool done = str_of(b.body, "drop", state, sizeof state) && !strcmp(state, "published");
  free(b.body); free(b.all); free(ap.body); free(ap.all);
  if (done) { reply(q->fd, ask(db, Q("drops.publish_answer($1, $2, $3, null)"), 3, (const char *[]){pk, drop, slug})); return; }
  if (!in_child()) return;
  PGconn *c = api_db_new();
  if (!c) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); _exit(0); }
  const char *why = publish_steps(c, slug, has_key);
  if (why) api_log("drops: publish stopped at %s", why);
  reply(q->fd, ask(c, Q("drops.publish_answer($1, $2, $3, $4)"), 4, (const char *[]){pk, drop, slug, why}));
  PQfinish(c);
  _exit(0);
}

static bool id_ok(const char *s, char lead) {
  if (s[0] != lead || s[1] != '-' || strlen(s) != 18) return false;
  for (int i = 2; i < 18; i++) if (!isxdigit((unsigned char)s[i])) return false;
  return true;
}

bool drops_route(const request *q) {
  if (strncmp(q->path, PREFIX, strlen(PREFIX)) || (q->path[strlen(PREFIX)] && q->path[strlen(PREFIX)] != '/')) return false;
  const char *rest = q->path + strlen(PREFIX);
  if (*rest == '/') rest++;
  bool post = !strcmp(q->method, "POST");
  if (!post && strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char pk[65]; bool signed_req;
  bool in = who(q, db, pk, &signed_req);
  if (post) {   // CSRF: a write comes from this site's pages, or is signed
    char from[256];
    bool has_origin = api_header(q, "Origin", from, sizeof from) != NULL;
    if ((has_origin && strcmp(from, origin())) || (!has_origin && !signed_req)) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  }
  if (!strcmp(rest, "signout") && post) { signout(q, db); return true; }
  if (!in) { api_json(q->fd, 401, "{\"error\":\"log in to drop files\"}"); return true; }
  bool admin = admin_is(pk);
  if (!strcmp(rest, "signin")) {
    if (!post || !signed_req) { api_json(q->fd, 401, "{\"error\":\"sign the sign-in\"}"); return true; }
    PGresult *r = PQexecParams(db, "SELECT drops.may($1, $2::bool)", 2, NULL, (const char *[]){pk, admin ? "true" : "false"}, NULL, NULL, 0);
    bool may = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && PQgetvalue(r, 0, 0)[0] == 't';
    PQclear(r);
    if (!may) { api_json(q->fd, 403, "{\"error\":\"this key may not drop files yet\"}"); return true; }
    signin(q, db, pk);
    return true;
  }
  if (!strcmp(rest, "me") && !post) { reply(q->fd, ask(db, Q("drops.me($1, $2::bool)"), 2, (const char *[]){pk, admin ? "true" : "false"})); return true; }
  if (!*rest) {
    if (post) reply(q->fd, ask(db, Q("drops.new_drop($1, $2::bool)"), 2, (const char *[]){pk, admin ? "true" : "false"}));
    else reply(q->fd, ask(db, Q("drops.list($1)"), 1, (const char *[]){pk}));
    return true;
  }
  char drop[24] = "", file[24] = "", tail[16] = "";
  const char *slash = strchr(rest, '/');
  size_t dn = slash ? (size_t)(slash - rest) : strlen(rest);
  if (dn < sizeof drop) { memcpy(drop, rest, dn); drop[dn] = 0; }
  if (!id_ok(drop, 'd')) { api_json(q->fd, 404, "{\"error\":\"not found\"}"); return true; }
  if (!slash) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET a drop\"}"); return true; }
    reply(q->fd, ask(db, Q("drops.get($1, $2)"), 2, (const char *[]){pk, drop}));
    return true;
  }
  if (!strcmp(slash, "/files") && post) { add_file(q, db, pk, admin, drop); return true; }
  if (post && !strcmp(slash, "/draft/edit")) { body_call(q, db, pk, drop, Q("drops.draft_edit($1, $2, $3::jsonb)")); return true; }
  if (post && !strcmp(slash, "/publish")) { publish(q, db, pk, drop); return true; }
  if (post && !strcmp(slash, "/discard")) { discard(q, db, pk, drop); return true; }
  if (!strcmp(slash, "/draft")) {
    if (post) draft(q, db, pk, drop);
    else reply(q->fd, ask(db, Q("drops.draft_get($1, $2)"), 2, (const char *[]){pk, drop}));
    return true;
  }
  if (!strncmp(slash, "/files/", 7) && sscanf(slash + 7, "%19[^/]/%15s", file, tail) == 2 && id_ok(file, 'f') && !strcmp(tail, "done") && post) {
    file_done(q, db, pk, drop, file);
    return true;
  }
  api_json(q->fd, 404, "{\"error\":\"not found\"}");
  return true;
}

// The bucket's part for villa.c (Planetary Villa's pet files share this private bucket).
bool drops_token(char *out, size_t cap) { return token(out, cap); }
bool drops_open_upload(const char *object, const char *type, const char *bytes, const char *from, char *out, size_t cap) {
  return open_upload(object, type, bytes, from, out, cap);
}
void drops_inspect(const char *object, long long *size, char **head_hex) { char md5[64]; inspect(object, size, md5, sizeof md5, head_hex); }
void drops_delete_object(const char *object) { delete_object(object); }
