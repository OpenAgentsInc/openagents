// Effects bridge for the authored Coax mesh-policy.coax. Authentication reuses
// private.c's existing NIP-98 verifier/site session. Native Coax psionic-fleet
// performs relay discovery, authenticated encrypted transport, failover and
// decision validation; this host only moves bounded bytes and database rows.
// A Gemini route goes to Google's Decisions API with OA_GEMINI_DECISIONS_KEY;
// mesh-gemini.coax maps the body and the answer. The model "auto" walks the
// door order in OA_DECISION_ORDER (mesh-policy.coax decides each step).
#include "api.h"
#include "mesh-policy.generated.h"
#include "mesh-gemini.generated.h"
#include "mesh-wire.generated.h"
#include <curl/curl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define MESH_HEADERS "Vary: Cookie, Authorization\r\n"
#define MESH_MAX 262144
static coax_string view(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static const char *origin(void) { const char *s = getenv("OA_API_ORIGIN"); return s && *s ? s : "https://openagents.com"; }
// A refusal is System One's error body, written by coax/lib/systemone/refusal.coax through
// mesh-wire.coax (Protocol, #13250): the status names its code there.
static void refuse(int fd, int status) {
  char out[180]; int64_t n = mgw_refusal_write((int64_t)status, (coax_v_u8){(uint8_t *)out, sizeof out});
  if (n < 0) n = 0;
  api_respond(fd, status, "application/json", status == 429 ? MESH_HEADERS "Retry-After: 60\r\n" : MESH_HEADERS, out, (size_t)n);
}
static bool command(PGconn *db, const char *sql) {
  PGresult *r = PQexec(db, sql); bool ok = PQresultStatus(r) == PGRES_COMMAND_OK; PQclear(r); return ok;
}

// Serialize only the observation/read/decision/lease transaction, never model
// execution. One advisory lock protects quotas across every Cloud Run instance.
static int reserve(PGconn *db, const char *pubkey, coax_string identity, char id[32]) {
  if (!command(db, "BEGIN")) return 503;
  PGresult *lock = PQexec(db, "SELECT pg_advisory_xact_lock(3073001)");
  bool ok = PQresultStatus(lock) == PGRES_TUPLES_OK; PQclear(lock);
  const char *v[1] = {pubkey};
  PGresult *r = ok ? PQexecParams(db,
    "SELECT count(*) FILTER (WHERE pubkey=$1 AND started>clock_timestamp()-interval '1 minute'), "
    "count(*) FILTER (WHERE pubkey=$1 AND started>=date_trunc('day',clock_timestamp())), "
    "count(*) FILTER (WHERE started>=date_trunc('day',clock_timestamp())), "
    "count(*) FILTER (WHERE pubkey=$1 AND NOT finished AND lease_until>clock_timestamp()), "
    "count(*) FILTER (WHERE NOT finished AND lease_until>clock_timestamp()) FROM mesh.calls "
    "WHERE started>=date_trunc('day',clock_timestamp()) OR lease_until>clock_timestamp() OR started>clock_timestamp()-interval '1 minute'",
    1, NULL, v, NULL, NULL, 0) : NULL;
  int status = 503;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    status = (int)mg_budget(atoll(PQgetvalue(r, 0, 0)), atoll(PQgetvalue(r, 0, 1)), atoll(PQgetvalue(r, 0, 2)), atoll(PQgetvalue(r, 0, 3)), atoll(PQgetvalue(r, 0, 4)));
  }
  if (r) PQclear(r);
  if (status == 200) {
    char exact[256]; snprintf(exact, sizeof exact, "%.*s", (int)identity.count, identity.data);
    const char *values[2] = {pubkey, exact};
    r = PQexecParams(db, "INSERT INTO mesh.calls(pubkey,identity) VALUES($1,$2) RETURNING id", 2, NULL, values, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(id, 32, "%s", PQgetvalue(r, 0, 0));
    else status = 503;
    PQclear(r);
  }
  if (status != 200) { command(db, "ROLLBACK"); return status; }
  return command(db, "COMMIT") ? 200 : 503;
}
static void finish(PGconn *db, const char *id, int status) {
  char http[16]; snprintf(http, sizeof http, "%d", status); const char *v[2] = {id, http};
  PGresult *r = PQexecParams(db, "UPDATE mesh.calls SET finished=true,status=$2::int WHERE id=$1::bigint", 2, NULL, v, NULL, NULL, 0); PQclear(r);
}

typedef struct { char *data; size_t count; } bytes;
static size_t receive(void *p, size_t size, size_t count, void *ctx) {
  bytes *b = ctx;
  if (size && count > MESH_MAX / size) return 0;
  size_t n = size * count;
  if (n > MESH_MAX - b->count) return 0;
  char *next = realloc(b->data, b->count + n + 1); if (!next) return 0;
  memcpy(next + b->count, p, n); b->data = next; b->count += n; b->data[b->count] = 0; return n;
}
// HTTPS fleet URLs are private Cloud Run services. Request their IAM audience
// token from the runtime's metadata service; neither user credentials nor the
// incoming cookie/signature are ever sent to a Pylon or upstream service.
static bool iam_token(const char *url, char out[4096]) {
  out[0] = 0;
  if (strncmp(url, "https://", 8)) return true;
  return api_id_token_for_url(url, NULL, out, 4096);
}
// One POST of n bytes to url with an optional extra header; the reply goes into b.
static CURLcode post(const char *url, const char *body, size_t n, const char *header, bytes *b, long *http) {
  CURL *h = curl_easy_init(); if (!h) return CURLE_FAILED_INIT;
  struct curl_slist *headers = curl_slist_append(NULL, "Content-Type: application/json");
  if (header && *header) headers = curl_slist_append(headers, header);
  curl_easy_setopt(h, CURLOPT_URL, url); curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, body); curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)n);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, receive); curl_easy_setopt(h, CURLOPT_WRITEDATA, b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT_MS, 30000L); curl_easy_setopt(h, CURLOPT_CONNECTTIMEOUT_MS, 3000L);
  curl_easy_setopt(h, CURLOPT_PROTOCOLS_STR, "http,https"); curl_easy_setopt(h, CURLOPT_FOLLOWLOCATION, 0L);
  curl_easy_setopt(h, CURLOPT_PROXY, "");
  CURLcode rc = curl_easy_perform(h); curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, http);
  curl_slist_free_all(headers); curl_easy_cleanup(h); return rc;
}
// A fleet door: the body as it is, the answer checked against the route's identity.
static int fleet(coax_t_MgRoute route, const char *body, size_t n, bytes *b) {
  char url[1025], token[4096], auth[4120];
  snprintf(url, sizeof url, "%.*s", (int)route.url.count, route.url.data);
  long http = 0; CURLcode rc = CURLE_FAILED_INIT;
  if (iam_token(url, token)) {
    auth[0] = 0; if (token[0]) snprintf(auth, sizeof auth, "Authorization: Bearer %s", token);
    rc = post(url, body, n, auth, b, &http);
  }
  memset(token, 0, sizeof token); memset(auth, 0, sizeof auth);
  if (rc != CURLE_OK) return rc == CURLE_OPERATION_TIMEDOUT ? 504 : 502;
  if (http == 200 && mg_response((coax_string){body, (int64_t)n}, (coax_string){b->data, (int64_t)b->count}, route)) return 200;
  return http == 400 || http == 413 || http == 422 || http == 429 || http == 503 ? (int)http : 502;
}
// Google's Decisions API: the body mapped to Google's shape, the answer mapped back into out.
// A body Gemini Decision 1 can't take is 422; a refusal with our error body leaves it in out.
static int gemini(coax_t_MgRoute route, const char *body, size_t n, bytes *out) {
  const char *key = getenv("OA_GEMINI_DECISIONS_KEY");
  if (!key || !*key || strlen(key) > 256) return 503;
  char url[1025]; snprintf(url, sizeof url, "%.*s", (int)route.url.count, route.url.data);
  char *g = malloc(MESH_MAX); if (!g) return 503;
  int64_t m = mgg_request((coax_string){body, (int64_t)n}, (coax_v_u8){(uint8_t *)g, MESH_MAX});
  if (m < 0) { free(g); return 422; }
  char header[300]; snprintf(header, sizeof header, "x-goog-api-key: %s", key);
  bytes b = {0}; long http = 0;
  CURLcode rc = post(url, g, (size_t)m, header, &b, &http);
  memset(header, 0, sizeof header); free(g);
  if (rc == CURLE_OPERATION_TIMEDOUT) { free(b.data); return 504; }
  char *o = malloc(MESH_MAX + 1); if (!o) { free(b.data); return 503; }
  coax_t_MggAnswer a = mgg_response((coax_string){body, (int64_t)n}, rc == CURLE_OK ? (int64_t)http : 0,
    (coax_string){b.data ? b.data : "", (int64_t)b.count}, (coax_v_u8){(uint8_t *)o, MESH_MAX});
  free(b.data);
  if (a.count > 0) { o[a.count] = 0; out->data = o; out->count = (size_t)a.count; } else free(o);
  return (int)a.status;
}
static void answer(const request *q, coax_t_MgRoute route, const char *id, const char *routes, const char *order, bool automatic) {
  bytes b = {0}; int status = 503;
  char *renamed = automatic ? malloc(MESH_MAX + 256) : NULL;
  while (true) {
    const char *body = q->body; size_t n = q->body_len;
    if (automatic) {
      int64_t m = renamed ? mg_rename((coax_string){q->body, (int64_t)q->body_len}, route.model, (coax_v_u8){(uint8_t *)renamed, MESH_MAX + 256}) : -1;
      if (m < 0) { status = 503; break; }
      body = renamed; n = (size_t)m;
    }
    free(b.data); b = (bytes){0};
    status = route.backend == 1 ? gemini(route, body, n, &b) : fleet(route, body, n, &b);
    if (!automatic || !mg_retry(status)) break;
    coax_t_MgRoute next = mg_order(view(routes), view(order), route.next);
    if (next.status != 200) break;
    route = next;
  }
  free(renamed);
  PGconn *db = api_db_new();
  if (db && PQstatus(db) == CONNECTION_OK) finish(db, id, status);
  if (db) PQfinish(db);
  // A fleet door's refusal body never reaches the caller; the Gemini door's is ours (mesh-gemini.coax).
  if (status == 200 || (route.backend == 1 && (status == 400 || status == 422) && b.count > 0)) api_respond(q->fd, status, "application/json", MESH_HEADERS, b.data, b.count);
  else refuse(q->fd, status);
  free(b.data);
}

bool mesh_route(const request *q) {
  if (strcmp(q->path, "/api/v1/systemone")) return false;
  char content[128]; api_header(q, "Content-Type", content, sizeof content);
  int status = (int)mg_http(view(q->method), view(content), (int64_t)q->body_len);
  if (status != 200) { refuse(q->fd, status); return true; }
  PGconn *db = api_db(); if (!db) { refuse(q->fd, 503); return true; }
  char who[65], from[512]; bool signed_request, payload, unseen;
  bool authenticated = site_identity(q, db, who, &signed_request, &payload, &unseen);
  bool has_origin = api_header(q, "Origin", from, sizeof from) != NULL;
  status = (int)mg_auth(authenticated, signed_request, payload, unseen, has_origin, !strcmp(from, origin()));
  if (status != 200) { refuse(q->fd, status); return true; }
  const char *routes = getenv("OA_MESH_ROUTES"), *order = getenv("OA_DECISION_ORDER");
  if (!routes || !*routes) { refuse(q->fd, 503); return true; }
  if (!order) order = "";
  bool automatic = mg_auto((coax_string){q->body, (int64_t)q->body_len});
  coax_t_MgRoute route = automatic ? mg_order(view(routes), view(order), 0) : mg_route((coax_string){q->body, (int64_t)q->body_len}, view(routes));
  if (route.status != 200) { refuse(q->fd, (int)route.status); return true; }
  char id[32]; status = reserve(db, who, route.identity, id);
  if (status != 200) { refuse(q->fd, status); return true; }
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { answer(q, route, id, routes, order, automatic); _exit(0); }
  if (pid < 0) { finish(db, id, 503); refuse(q->fd, 503); }
  return true;
}
