// Effects bridge for the authored Coax mesh-policy.coax. Authentication reuses
// private.c's existing NIP-98 verifier/site session. Native Coax psionic-fleet
// performs relay discovery, authenticated encrypted transport, failover and
// decision validation; this host only moves bounded bytes and database rows.
#include "api.h"
#include "mesh-policy.generated.h"
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
static void refuse(int fd, int status) {
  const char *text = status == 401 ? "authentication_required" : status == 403 ? "cross_site" :
    status == 405 ? "method_not_allowed" : status == 413 ? "limit_exceeded" : status == 415 ? "application_json_required" :
    status == 429 ? "demo_limit_exceeded" : status == 422 ? "unsupported_input" : status == 400 ? "invalid_request_or_model" :
    status == 504 ? "mesh_timeout" : "mesh_unavailable";
  char out[180]; snprintf(out, sizeof out, "{\"error\":{\"code\":\"%s\"}}", text);
  api_respond(fd, status, "application/json", status == 429 ? MESH_HEADERS "Retry-After: 60\r\n" : MESH_HEADERS, out, strlen(out));
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
  const char *end = strchr(url + 8, '/'); size_t n = end ? (size_t)(end - url) : strlen(url);
  char audience[1025]; if (n >= sizeof audience) return false;
  memcpy(audience, url, n); audience[n] = 0;
  CURL *h = curl_easy_init(); if (!h) return false;
  char *escaped = curl_easy_escape(h, audience, 0); char meta[4300];
  if (!escaped) { curl_easy_cleanup(h); return false; }
  snprintf(meta, sizeof meta, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity?audience=%s", escaped);
  curl_free(escaped); bytes b = {0};
  struct curl_slist *headers = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(h, CURLOPT_URL, meta); curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, receive); curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT_MS, 3000L); curl_easy_setopt(h, CURLOPT_PROXY, "");
  long status = 0; CURLcode rc = curl_easy_perform(h); curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  bool ok = rc == CURLE_OK && status == 200 && b.count > 0 && b.count < 4096;
  if (ok) memcpy(out, b.data, b.count + 1);
  free(b.data); curl_slist_free_all(headers); curl_easy_cleanup(h); return ok;
}
static void answer(const request *q, coax_t_MgRoute route, const char *id) {
  char url[1025], token[4096], auth[4120];
  snprintf(url, sizeof url, "%.*s", (int)route.url.count, route.url.data);
  bytes b = {0}; long http = 0; CURLcode rc = CURLE_FAILED_INIT;
  CURL *h = curl_easy_init(); struct curl_slist *headers = NULL;
  if (h && iam_token(url, token)) {
    headers = curl_slist_append(NULL, "Content-Type: application/json");
    if (token[0]) { snprintf(auth, sizeof auth, "Authorization: Bearer %s", token); headers = curl_slist_append(headers, auth); }
    curl_easy_setopt(h, CURLOPT_URL, url); curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, q->body); curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)q->body_len);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, receive); curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
    curl_easy_setopt(h, CURLOPT_TIMEOUT_MS, 30000L); curl_easy_setopt(h, CURLOPT_CONNECTTIMEOUT_MS, 3000L);
    curl_easy_setopt(h, CURLOPT_PROTOCOLS_STR, "http,https"); curl_easy_setopt(h, CURLOPT_FOLLOWLOCATION, 0L);
    curl_easy_setopt(h, CURLOPT_PROXY, ""); rc = curl_easy_perform(h); curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &http);
  }
  memset(token, 0, sizeof token); memset(auth, 0, sizeof auth);
  int status = rc == CURLE_OPERATION_TIMEDOUT ? 504 : 502;
  if (rc == CURLE_OK) {
    if (http == 200 && mg_response((coax_string){q->body, (int64_t)q->body_len}, (coax_string){b.data, (int64_t)b.count}, route)) status = 200;
    else if (http == 400 || http == 413 || http == 422 || http == 429 || http == 503) status = (int)http;
  }
  PGconn *db = api_db_new();
  if (db && PQstatus(db) == CONNECTION_OK) finish(db, id, status);
  if (db) PQfinish(db);
  if (status == 200) api_respond(q->fd, 200, "application/json", MESH_HEADERS, b.data, b.count);
  else refuse(q->fd, status);
  free(b.data); curl_slist_free_all(headers); if (h) curl_easy_cleanup(h);
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
  const char *routes = getenv("OA_MESH_ROUTES");
  if (!routes || !*routes) { refuse(q->fd, 503); return true; }
  coax_t_MgRoute route = mg_route((coax_string){q->body, (int64_t)q->body_len}, view(routes));
  if (route.status != 200) { refuse(q->fd, (int)route.status); return true; }
  char id[32]; status = reserve(db, who, route.identity, id);
  if (status != 200) { refuse(q->fd, status); return true; }
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { answer(q, route, id); _exit(0); }
  if (pid < 0) { finish(db, id, 503); refuse(q->fd, 503); }
  return true;
}
