// Anime Lab's director effects. Coax owns request/schema/answer admission,
// identity observations, quota decisions and error/status selection. This
// host moves bounded bytes, observes/records rows and calls the existing
// selector transport. It stores no words or scene state.
#include "api.h"
#include "anime-director-policy.generated.h"
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define ANIME_BYTES 65536
#define ANIME_HEADERS "Cache-Control: no-store\r\nVary: Cookie, Authorization\r\n"

static coax_string text(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_u8 bytes(const void *p, size_t n) { return (coax_v_u8){(uint8_t *)p, (int64_t)n}; }
static const char *configured(const char *key, const char *fallback) {
  const char *value = getenv(key); return value && *value ? value : fallback;
}
static void reply(int fd, int status, const unsigned char *body, int64_t n) {
  if (n < 0 || n > ANIME_BYTES) { api_json(fd, 503, "{\"error\":\"memory\"}"); return; }
  api_respond(fd, status, "application/json", ANIME_HEADERS, (const char *)body, (size_t)n);
}
static void refuse(int fd, int status) {
  unsigned char out[512]; int64_t n = aar_error(status, bytes(out, sizeof out));
  reply(fd, status, out, n);
}
static void policy_error(int fd, int64_t code) {
  unsigned char out[512]; int status = (int)aap_status(code);
  int64_t n = aap_error(code, bytes(out, sizeof out)); reply(fd, status, out, n);
}
static bool command(PGconn *db, const char *sql) {
  PGresult *r = PQexec(db, sql); bool ok = PQresultStatus(r) == PGRES_COMMAND_OK; PQclear(r); return ok;
}

// Serialize observations, the Coax decision and the reservation across every
// service instance. The transaction ends before any model/network work.
static int reserve(PGconn *db, const char *identity, char id[32]) {
  if (!command(db, "BEGIN")) return 503;
  PGresult *lock = PQexec(db, "SELECT pg_advisory_xact_lock(3073002)");
  bool ok = PQresultStatus(lock) == PGRES_TUPLES_OK; PQclear(lock);
  const char *v[1] = {identity};
  PGresult *r = ok ? PQexecParams(db,
    "WITH t AS (SELECT clock_timestamp() now_, date_trunc('day',clock_timestamp() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' day_) "
    "SELECT count(*) FILTER (WHERE identity=$1 AND started>t.now_-interval '1 minute'), "
    "count(*) FILTER (WHERE identity=$1 AND started>=t.day_), "
    "count(*) FILTER (WHERE started>=t.day_), "
    "count(*) FILTER (WHERE identity=$1 AND NOT finished AND lease_until>t.now_), "
    "count(*) FILTER (WHERE NOT finished AND lease_until>t.now_) "
    "FROM anime.calls CROSS JOIN t WHERE started>=t.day_ OR lease_until>t.now_ OR started>t.now_-interval '1 minute'",
    1, NULL, v, NULL, NULL, 0) : NULL;
  int status = 503;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    status = (int)aar_budget(atoll(PQgetvalue(r, 0, 0)), atoll(PQgetvalue(r, 0, 1)),
      atoll(PQgetvalue(r, 0, 2)), atoll(PQgetvalue(r, 0, 3)), atoll(PQgetvalue(r, 0, 4)));
  }
  if (r) PQclear(r);
  if (status == 200) {
    char lease[32]; snprintf(lease, sizeof lease, "%lld", (long long)aar_lease_ms());
    const char *values[2] = {identity, lease};
    r = PQexecParams(db,
      "INSERT INTO anime.calls(identity,lease_until) VALUES($1,clock_timestamp()+$2::bigint*interval '1 millisecond') RETURNING id",
      2, NULL, values, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(id, 32, "%s", PQgetvalue(r, 0, 0));
    else status = 503;
    PQclear(r);
  }
  if (status != 200) { command(db, "ROLLBACK"); return status; }
  return command(db, "COMMIT") ? 200 : 503;
}

static void finish(const char *id, int status) {
  PGconn *db = api_db_new();
  if (db && PQstatus(db) == CONNECTION_OK) {
    char http[16]; snprintf(http, sizeof http, "%d", status); const char *v[2] = {id, http};
    PGresult *r = PQexecParams(db, "UPDATE anime.calls SET finished=true,status=$2::int WHERE id=$1::bigint", 2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_COMMAND_OK) api_log("anime: completion not recorded");
    PQclear(r);
  } else api_log("anime: completion database unavailable");
  if (db) PQfinish(db);
}

static void answer(const request *q, const char *prepared, const char *id, long timeout_ms) {
  const char *model = configured("OA_ANIME_MODEL", configured("OA_NPC_MODEL", "gemini-3.8-flash"));
  char *got = NULL; size_t got_n = 0;
  long http = npc_ask_model_twice_n("OA_ANIME_URL", model, prepared, timeout_ms, &got, &got_n);
  int status = (int)aar_upstream(http);
  unsigned char *out = malloc(ANIME_BYTES);
  if (!out) { free(got); finish(id, 503); refuse(q->fd, 503); return; }
  int64_t n = 0;
  if (status == 200) {
    n = aap_finish(bytes(q->body, q->body_len), bytes(got, got_n), bytes(out, ANIME_BYTES));
    if (n < 0) { status = (int)aap_status(n); n = aap_error(n, bytes(out, ANIME_BYTES)); }
  } else n = aar_error(status, bytes(out, ANIME_BYTES));
  finish(id, status);
  reply(q->fd, status, out, n);
  free(out); free(got);
}

bool anime_route(const request *q) {
  if (strcmp(q->path, "/api/playtest/labs/anime/direct")) return false;
  char content[128] = ""; api_header(q, "Content-Type", content, sizeof content);
  int status = (int)aar_http(text(q->method), text(content), (int64_t)q->body_len);
  if (status != 200) { refuse(q->fd, status); return true; }
  char from[512] = "";
  bool has_origin = api_header(q, "Origin", from, sizeof from) != NULL;
  bool same_origin = !strcmp(from, configured("OA_API_ORIGIN", "https://openagents.com"));
  status = (int)aar_origin(has_origin, same_origin);
  if (status != 200) { refuse(q->fd, status); return true; }
  PGconn *db = api_db(); if (!db) { refuse(q->fd, 503); return true; }
  char identity[65]; bool signed_request, payload, unseen;
  bool authenticated = site_identity(q, db, identity, &signed_request, &payload, &unseen);
  status = (int)aar_auth(authenticated, signed_request, payload, unseen, has_origin,
    same_origin);
  if (status != 200) { refuse(q->fd, status); return true; }
  char *prepared = malloc(ANIME_BYTES + 1);
  if (!prepared) { refuse(q->fd, 503); return true; }
  int64_t n = aap_prepare(bytes(q->body, q->body_len), bytes(prepared, ANIME_BYTES));
  if (n < 0) { free(prepared); policy_error(q->fd, n); return true; }
  if (n == 0 || n > ANIME_BYTES) { free(prepared); refuse(q->fd, 503); return true; }
  prepared[n] = 0;
  long timeout_ms = (long)aar_timeout(text(configured("OA_ANIME_TIMEOUT_MS", "")));
  if (timeout_ms == 0) { free(prepared); refuse(q->fd, 503); return true; }
  char id[32]; status = reserve(db, identity, id);
  if (status != 200) { free(prepared); refuse(q->fd, status); return true; }
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { answer(q, prepared, id, timeout_ms); free(prepared); _exit(0); }
  if (pid < 0) { finish(id, 503); refuse(q->fd, 503); }
  free(prepared);
  return true;
}
