// Service levels and alerts (#13297, docs/deployment/service-levels.md). service-policy.coax
// (#authority) decides what a good minute, a good join and an alert are; this file moves the facts
// in and the answers out, and sends the mail.
//
//   POST /api/jobs/service-watch      every minute (Cloud Scheduler, jobs.c): each room shard's
//                                     facts and the database's connections go through the policy;
//                                     what holds opens an alert, what stopped resolves one
//                                     (verse.service_sync), and each change is mailed to OA_ALERT_TO
//                                     (the owner's phone) and logged
//   GET  /api/playtest/service-health the three service levels over 30 days, each day's, and the open
//                                     alerts (the status page, the green-main watcher's forwarding to
//                                     the agents' inboxes: coax/remote/pipeline-lib.sh pl_service_watch)
//
//   OA_ALERT_TO     where alert mail goes (deploy.sh sets the owner's address); none: no mail
//   OA_API_ORIGIN   https://staging.openagents.com names the staging environment in the mail
#include "api.h"
#include "service-policy.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { char *p; size_t n, cap; } sbuf;
static void put(sbuf *b, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
#include <stdarg.h>
static void put(sbuf *b, const char *fmt, ...) {
  for (;;) {
    va_list a; va_start(a, fmt);
    int k = vsnprintf(b->p ? b->p + b->n : NULL, b->p ? b->cap - b->n : 0, fmt, a);
    va_end(a);
    if (k < 0) return;
    if (b->p && b->n + (size_t)k < b->cap) { b->n += (size_t)k; return; }
    size_t cap = (b->cap ? b->cap * 2 : 1024) + (size_t)k;
    char *g = realloc(b->p, cap);
    if (!g) return;
    b->p = g; b->cap = cap;
  }
}
// A policy string's characters inside a JSON string (its words hold no quote or backslash; any is dropped).
static void put_raw(sbuf *b, coax_string s) {
  for (int64_t i = 0; i < s.count; i++) { char c = s.data[i]; if (c != '"' && c != '\\' && (unsigned char)c >= 0x20) put(b, "%c", c); }
}
static void put_str(sbuf *b, coax_string s) { put(b, "\""); put_raw(b, s); put(b, "\""); }
static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }
static const char *env_name(void) { return strstr(origin(), "staging") ? "staging" : "production"; }

// One alert into the JSON array service_sync reads: {"key", "title", "do"}.
static void alert(sbuf *b, bool *first, const char *key, int shard, coax_string what, coax_string todo) {
  put(b, "%s{\"key\":\"%s", *first ? "" : ",", key);
  if (shard > 0) put(b, ":%d\",\"title\":\"shard %d: ", shard, shard); else put(b, "\",\"title\":\"");
  put_raw(b, what);
  put(b, "\",\"do\":");
  put_str(b, todo);
  put(b, "}");
  *first = false;
}

// The job (jobs.c, every minute): its result as JSON, or NULL when it could not run.
char *service_watch(PGconn *db, const char *day) {
  (void)day;
  long long now = (long long)api_now_ms();
  char ns[24], slow[24], w0[24], w1[24], w2[24];
  snprintf(ns, sizeof ns, "%lld", now); snprintf(slow, sizeof slow, "%lld", (long long)sv_slow_p95_us());
  snprintf(w0, sizeof w0, "%lld", (long long)sv_window(0)); snprintf(w1, sizeof w1, "%lld", (long long)sv_window(1));
  snprintf(w2, sizeof w2, "%lld", (long long)sv_window(2));
  const char *fv[5] = {ns, slow, w0, w1, w2};
  PGresult *r = PQexecParams(db, "SELECT shard, addressed, draining, silent_ms, slow, failures, fenced, unsaved_ms "
                                 "FROM verse.service_shard_facts($1::bigint, $2::bigint, $3::bigint, $4::bigint, $5::bigint)", 5, NULL, fv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("service-watch: %s", PQerrorMessage(db)); PQclear(r); return NULL; }
  sbuf want = {0}; bool first = true;
  put(&want, "[");
  for (int i = 0; i < PQntuples(r); i++) {
    int shard = atoi(PQgetvalue(r, i, 0));
    int64_t bits = sv_shard_alerts(!strcmp(PQgetvalue(r, i, 1), "t"), !strcmp(PQgetvalue(r, i, 2), "t"), atoll(PQgetvalue(r, i, 3)),
                                   atoll(PQgetvalue(r, i, 4)), atoll(PQgetvalue(r, i, 5)), atoll(PQgetvalue(r, i, 6)), atoll(PQgetvalue(r, i, 7)));
    for (int64_t k = 0; k < sv_alert_bits(); k++) {
      int64_t bit = (int64_t)1 << k;
      if (!(bits & bit)) continue;
      coax_string key = sv_alert_key(bit);
      char ks[40]; snprintf(ks, sizeof ks, "%.*s", (int)key.count, key.data);
      alert(&want, &first, ks, shard, sv_alert_text(bit), sv_alert_do(bit));
    }
  }
  PQclear(r);
  r = PQexec(db, "SELECT count(*), current_setting('max_connections')::int FROM pg_stat_activity WHERE backend_type = 'client backend'");
  int conns = -1, most = -1;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { conns = atoi(PQgetvalue(r, 0, 0)); most = atoi(PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (sv_db_alert(conns, most)) alert(&want, &first, "db-connections", 0, sv_db_text(), sv_db_do());
  put(&want, "]");
  if (!want.p) return NULL;
  const char *sv[2] = {ns, want.p};
  r = PQexecParams(db, "SELECT verse.service_sync($1::bigint, $2::jsonb)", 2, NULL, sv, NULL, NULL, 0);
  free(want.p);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("service-watch: %s", PQerrorMessage(db)); PQclear(r); return NULL; }
  char *sync = strdup(PQgetvalue(r, 0, 0));
  PQclear(r);
  if (!sync) return NULL;
  // Each change: the log, and mail to the owner.
  const char *to = getenv("OA_ALERT_TO");
  const char *mv[5] = {sync, to && *to ? to : "", env_name(), origin(), ns};
  r = PQexecParams(db, "SELECT m::text, m->>'subject' FROM verse.service_mail($1::jsonb, $2, $3, $4, $5::bigint) m", 5, NULL, mv, NULL, NULL, 0);
  int mailed = 0, changes = 0;
  if (PQresultStatus(r) == PGRES_TUPLES_OK) {
    changes = PQntuples(r);
    for (int i = 0; i < PQntuples(r); i++) {
      api_log("service alert: %s", PQgetvalue(r, i, 1));
      if (!to || !*to) continue;
      char id[128], err[200];
      int s = email_send(db, PQgetvalue(r, i, 0), id, err, sizeof err);
      if (s == 0 || s == 1) mailed++; else api_log("service alert: not mailed (%d): %s", s, err);
    }
  } else api_log("service-watch: %s", PQerrorMessage(db));
  PQclear(r);
  size_t n = strlen(sync) + 64;
  char *out = malloc(n);
  if (out) snprintf(out, n, "{\"changes\":%d,\"mailed\":%d,\"sync\":%s}", changes, mailed, sync);
  free(sync);
  return out;
}

// GET /api/playtest/service-health: the service levels and the open alerts.
static void health(const request *q) {
  if (strcmp(q->method, "GET") && strcmp(q->method, "HEAD")) { api_json(q->fd, 405, "{\"error\":\"GET the service health\"}"); return; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  long long now = (long long)api_now_ms();
  char ns[24], days[8], t0[24], t1[24], t2[24];
  snprintf(ns, sizeof ns, "%lld", now); snprintf(days, sizeof days, "%lld", (long long)sv_days());
  snprintf(t0, sizeof t0, "%lld", (long long)sv_target(0)); snprintf(t1, sizeof t1, "%lld", (long long)sv_target(1));
  snprintf(t2, sizeof t2, "%lld", (long long)sv_target(2));
  const char *v[5] = {ns, days, t0, t1, t2};
  PGresult *r = PQexecParams(db, "SELECT which, good, total, worst, daily FROM verse.service_levels($1::bigint, $2::int, $3::bigint, $4::bigint, $5::bigint)",
                             5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("service-health: %s", PQerrorMessage(db)); PQclear(r); api_json(q->fd, 500, "{\"error\":\"no service levels\"}"); return; }
  static const char *units[3] = {"us", "ms", "ms"};
  sbuf b = {0};
  put(&b, "{\"env\":\"%s\",\"now_ms\":%lld,\"days\":%lld,\"levels\":[", env_name(), now, (long long)sv_days());
  for (int i = 0; i < PQntuples(r); i++) {
    int64_t which = atoll(PQgetvalue(r, i, 0)), good = atoll(PQgetvalue(r, i, 1)), total = atoll(PQgetvalue(r, i, 2));
    if (which < 0 || which > 2) continue;
    put(&b, "%s{\"name\":", i ? "," : ""); put_str(&b, sv_name(which));
    put(&b, ",\"target\":%lld,\"unit\":\"%s\",\"objective_ppm\":%lld,\"good\":%lld,\"total\":%lld,\"share_ppm\":%lld,\"met\":%lld,\"worst\":%s,\"daily\":%s}",
        (long long)sv_target(which), units[which], (long long)sv_objective_ppm(which), (long long)good, (long long)total,
        (long long)sv_share_ppm(good, total), (long long)sv_met(which, good, total), PQgetisnull(r, i, 3) ? "null" : PQgetvalue(r, i, 3), PQgetvalue(r, i, 4));
  }
  PQclear(r);
  r = PQexec(db, "SELECT coalesce(jsonb_agg(jsonb_build_object('key', key, 'title', title, 'opened_ms', opened_ms, 'resolved_ms', resolved_ms) "
                 "ORDER BY resolved_ms IS NOT NULL, opened_ms DESC) FILTER (WHERE resolved_ms IS NULL), '[]'), "
                 "coalesce(jsonb_agg(jsonb_build_object('key', key, 'title', title, 'opened_ms', opened_ms, 'resolved_ms', resolved_ms) "
                 "ORDER BY resolved_ms DESC) FILTER (WHERE resolved_ms IS NOT NULL), '[]') "
                 "FROM verse.service_alerts WHERE resolved_ms IS NULL OR resolved_ms > (extract(epoch FROM now()) * 1000)::bigint - 86400000");
  bool got = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  put(&b, "],\"alerts\":%s,\"resolved_day\":%s}", got ? PQgetvalue(r, 0, 0) : "[]", got ? PQgetvalue(r, 0, 1) : "[]");
  if (!got) api_log("service-health: %s", PQerrorMessage(db));
  PQclear(r);
  if (b.p) { api_json(q->fd, 200, b.p); free(b.p); } else api_json(q->fd, 500, "{\"error\":\"out of memory\"}");
}

bool service_route(const request *q) {
  if (strcmp(q->path, "/api/playtest/service-health")) return false;
  health(q);
  return true;
}
