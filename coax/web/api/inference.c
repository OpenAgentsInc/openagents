// Our inference route (gw-ours, docs/coder/microcoder.md): the Mac app's Lab
// asks models through us, so the app ships with no provider key.
//
//   POST /api/v1/inference   a Chat Completions request (model, messages, stream, response_format ...),
//                        signed NIP-98 ("Authorization: Nostr <base64>") by the user's Nostr key
//                        -> Vertex AI's answer as it comes: its status, then its JSON or its
//                           server-sent events; 400 a bad request or a model we don't serve,
//                           401 no valid signature, 403 a key that may not use the route,
//                           429 the account's day used up, 502 Vertex not reached
//
// Who: a key in OA_INFERENCE_PUBKEYS (comma separated) or an admin's
// (OA_ADMIN_PUBKEYS). The account is the signing key: the request, its caps
// and its metering are SQL's (migrations/0059_inference.sql inference.request,
// inference.finish), at the card's rates. Payment for use past the caps is
// not here yet; when it comes it is Bitcoin only.
//
// The call to Vertex takes seconds, and this service answers one request at
// a time, so a child process makes it: it relays Vertex's status and bytes to
// the caller as they arrive, then keeps the call, with the usage Vertex
// reported, on a database connection of its own.
//   OA_INFERENCE_URL          the Chat Completions URL (tests point it at a fake)
//   OA_INFERENCE_TOKEN        a token to send instead of the metadata server's (tests)
//   OA_INFERENCE_PUBKEYS      the keys that may use the route, besides the admins'
//   OA_INFERENCE_DAILY_REQUESTS  requests a day per account (500)
//   OA_INFERENCE_DAILY_USD_MICROS  spend a day per account, in millionths of a dollar (2000000)
#include "api.h"
#include "nostr.h"
#include <curl/curl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define VERTEX_CHAT "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/endpoints/openapi/chat/completions"

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

// A key in the comma-separated list OA_INFERENCE_PUBKEYS.
static bool listed(const char *pubkey) {
  const char *l = getenv("OA_INFERENCE_PUBKEYS");
  size_t n = strlen(pubkey);
  while (l && *l) {
    const char *end = strchr(l, ',');
    size_t k = end ? (size_t)(end - l) : strlen(l);
    while (k && l[0] == ' ') { l++; k--; }
    while (k && l[k - 1] == ' ') k--;
    if (k == n && !strncmp(l, pubkey, n)) return true;
    if (!end) break;
    l = end + 1;
  }
  return false;
}

// The service account's token, from the metadata server (or OA_INFERENCE_TOKEN).
typedef struct { char *p; size_t n, cap; } buf;
static size_t keep(buf *b, const char *data, size_t k) {
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
static size_t take(char *data, size_t size, size_t n, void *user) { return keep(user, data, size * n); }
static bool token(char *out, size_t cap) {
  const char *given = getenv("OA_INFERENCE_TOKEN");
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

// The relay: Vertex's status line once its first bytes come, then its bytes
// as they come; everything also kept (to 4 MB) for the usage at the end.
typedef struct { int fd; CURL *h; bool stream, started, gone; buf all; } relay_t;
static bool send_all(int fd, const char *p, size_t n) {
  while (n) { ssize_t k = write(fd, p, n); if (k <= 0) return false; p += k; n -= (size_t)k; }
  return true;
}
static void start(relay_t *r, long status) {
  char head[256];
  const char *type = r->stream && status == 200 ? "text/event-stream" : "application/json";
  int n = snprintf(head, sizeof head, "HTTP/1.1 %ld %s\r\nContent-Type: %s\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                   status, status == 200 ? "OK" : "Upstream", type);
  r->started = true;
  if (!send_all(r->fd, head, (size_t)n)) r->gone = true;
}
static size_t relay(char *data, size_t size, size_t n, void *user) {
  relay_t *r = user; size_t k = size * n;
  if (!r->started) { long status = 0; curl_easy_getinfo(r->h, CURLINFO_RESPONSE_CODE, &status); start(r, status ? status : 502); }
  if (!r->gone && !send_all(r->fd, data, k)) r->gone = true;   // the caller left: stop asking Vertex
  if (r->all.n < (4u << 20)) keep(&r->all, data, k);
  return r->gone ? 0 : k;
}

// The last "name": N in text (a stream reports its usage in its last chunk), or 0.
static int last_int(const char *text, const char *name) {
  int v = 0; size_t nl = strlen(name);
  for (const char *p = text ? strstr(text, name) : NULL; p; p = strstr(p + nl, name)) {
    const char *q = p + nl;
    while (*q == ' ' || *q == ':') q++;
    v = atoi(q);
  }
  return v;
}

// In the child: asks Vertex, relays its answer, keeps the call.
static void answer(int fd, const char *who, const char *model, const char *request, bool stream) {
  relay_t r = {.fd = fd, .stream = stream};
  char tok[4096], auth[4200];
  long status = 0;
  double t0 = api_now_ms();
  if (token(tok, sizeof tok)) {
    r.h = curl_easy_init();
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    curl_easy_setopt(r.h, CURLOPT_URL, env_or("OA_INFERENCE_URL", VERTEX_CHAT));
    curl_easy_setopt(r.h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(r.h, CURLOPT_POSTFIELDS, request);
    curl_easy_setopt(r.h, CURLOPT_WRITEFUNCTION, relay);
    curl_easy_setopt(r.h, CURLOPT_WRITEDATA, &r);
    curl_easy_setopt(r.h, CURLOPT_TIMEOUT, 300L);
    if (curl_easy_perform(r.h) == CURLE_OK || r.started) curl_easy_getinfo(r.h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(r.h);
    memset(tok, 0, sizeof tok); memset(auth, 0, sizeof auth);
  } else api_log("inference: no token");
  if (!r.started) api_json(fd, 502, "{\"error\":\"the model was not reached\"}");
  char http[16], in[16], out[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(in, sizeof in, "%d", last_int(r.all.p, "\"prompt_tokens\""));
  snprintf(out, sizeof out, "%d", last_int(r.all.p, "\"completion_tokens\""));
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[6] = {who, model, http, in, out, taken};
  PGresult *q = db ? PQexecParams(db, "SELECT inference.finish($1, $2, $3::int, $4::int, $5::int, $6::int)", 6, NULL, v, NULL, NULL, 0) : NULL;
  if (!q || PQresultStatus(q) != PGRES_TUPLES_OK) api_log("inference: not kept: %s", db ? PQerrorMessage(db) : "no database");
  if (status != 200) api_log("inference: the model answered %ld", status);
  if (q) PQclear(q);
  if (db) PQfinish(db);
  free(r.all.p);
}

bool inference_route(const request *q) {
  if (strcmp(q->path, "/api/v1/inference")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a Chat Completions request\"}"); return true; }
  char auth[4096];
  if (!api_header(q, "Authorization", auth, sizeof auth) || strncmp(auth, "Nostr ", 6)) {
    api_json(q->fd, 401, "{\"error\":\"sign the request (NIP-98)\"}"); return true;
  }
  char url[4200]; snprintf(url, sizeof url, "%s%s", env_or("OA_API_ORIGIN", "https://openagents.com"), q->target);
  nostr_event ev; char err[160];
  if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) {
    char out[240]; snprintf(out, sizeof out, "{\"error\":\"%s\"}", err);
    api_json(q->fd, 401, out); return true;
  }
  if (!admin_is(ev.pubkey) && !listed(ev.pubkey)) { api_json(q->fd, 403, "{\"error\":\"this key may not use the route yet\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char *body = malloc(q->body_len + 1);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  const char *v[4] = {ev.pubkey, body, env_or("OA_INFERENCE_DAILY_REQUESTS", "500"), env_or("OA_INFERENCE_DAILY_USD_MICROS", "2000000")};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, x->>'model', x->>'stream' "
                                 "FROM inference.request($1, $2, $3::int, $4::bigint) x", 4, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("inference: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, 500, "{\"error\":\"not asked\"}");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) {
    char out[200]; snprintf(out, sizeof out, "{\"error\":\"%s\"}", PQgetvalue(r, 0, 1));
    PQclear(r);
    api_json(q->fd, status, out);
    return true;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *model = strdup(PQgetvalue(r, 0, 3));
  bool stream = !strcmp(PQgetvalue(r, 0, 4), "true");
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);   // children are never waited for
  pid_t pid = fork();
  if (pid == 0) {             // the child answers; the parent's database connection is never touched here
    answer(q->fd, ev.pubkey, model, request, stream);
    _exit(0);
  }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(model);
  return true;   // the parent closes its copy of the connection
}
