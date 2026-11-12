// Service levels and alerts (#13297, docs/deployment/service-levels.md). service-policy.coax
// (#authority) decides what a good minute, a good join and an alert are; this file moves the facts
// in and the answers out, and sends the mail. Every body it sends is written by service-wire.coax
// (proto/openagents/ops/service/v1alpha1/service.proto, Protocol E11 #13250).
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
// Crash reports (#13298, docs/deployment/crash-reports.md): the watch folds the pages' `crash`
// events into reports and raises the crash alerts; rooms post theirs to /api/playtest/room-crash,
// and GET /api/playtest/crashes lists the buckets (rooms.c). crash_keep stores one.
//
//   OA_ALERT_TO     where production's alert mail goes (deploy.sh sets the owner's address); none, or
//                   staging: no mail
//   OA_API_ORIGIN   https://staging.openagents.com names the staging environment in the mail
#include "api.h"
#include "service-policy.generated.h"
#include "service-wire.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// A buffer the codec (service-wire.coax) writes into. W makes room for a writer's bytes (at most
// `most` of them), runs it, and takes what it wrote; false when it did not fit or memory ran out
// (b->p is freed then).
typedef struct { char *p; size_t n, cap; } sbuf;
static bool room(sbuf *b, size_t most) {
  if (b->p && b->n + most + 1 <= b->cap) return true;
  size_t cap = b->cap * 2 + most + 1024;
  char *g = realloc(b->p, cap);
  if (!g) { free(b->p); b->p = NULL; b->n = b->cap = 0; return false; }
  b->p = g; b->cap = cap;
  return true;
}
static bool wrote(sbuf *b, int64_t k) {
  if (k < 0) { free(b->p); b->p = NULL; b->n = b->cap = 0; return false; }
  b->n = (size_t)k; b->p[b->n] = 0;
  return true;
}
#define ROOM(b) ((coax_v_u8){(uint8_t *)(b)->p, (int64_t)(b)->cap - 1})
#define W(b, most, call) (room((b), (most)) && wrote((b), (call)))
static coax_string cs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static void fail(const request *q, int status, const char *why) {
  char out[160];
  int64_t n = svw_error_write(cs(why), (coax_v_u8){(uint8_t *)out, sizeof out - 1});
  out[n < 0 ? 0 : n] = 0;
  api_json(q->fd, status, out);
}
static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }
static const char *env_name(void) { return strstr(origin(), "staging") ? "staging" : "production"; }

// ---- crash reports (#13298): room-crash.coax is the one codec (ticket-sign-unit.c)
int crash_report_check_c(const char *body, size_t n);
size_t crash_report_field_c(const char *body, size_t n, const char *key, char *out, size_t cap);
size_t crash_page_report_c(long long t, const char *page, const char *release, const char *msg, const char *trap, const char *frames, char *out, size_t cap);

// Keeps one report the codec takes (verse.crash_add): its bucket into bucket, false when it is not
// a report or could not be kept. automated: an automated browser sent it.
bool crash_keep(PGconn *db, const char *body, size_t n, bool automated, char bucket[33]) {
  char t[24], source[8], shard[8], release[256], build[80], sig[256], ns[24];
  bucket[0] = 0;
  if (crash_report_check_c(body, n) < 0) return false;
  char *b = malloc(n + 1);
  if (!b) return false;
  memcpy(b, body, n); b[n] = 0;
  bool room = crash_report_field_c(body, n, "shard", shard, sizeof shard) > 0;
  crash_report_field_c(body, n, "release", release, sizeof release);
  crash_report_field_c(body, n, "build", build, sizeof build);
  if (!crash_report_field_c(body, n, "t", t, sizeof t) || !crash_report_field_c(body, n, "source", source, sizeof source) ||
      !crash_report_field_c(body, n, "sig", sig, sizeof sig)) { free(b); return false; }
  snprintf(ns, sizeof ns, "%lld", (long long)api_now_ms());
  const char *v[9] = {ns, t, source, room ? shard : NULL, release, build, sig, automated ? "t" : "f", b};
  PGresult *r = PQexecParams(db, "SELECT verse.crash_add($1::bigint, $2::bigint, $3, $4::int, $5, $6, $7, $8::boolean, $9)", 9, NULL, v, NULL, NULL, 0);
  free(b);
  bool kept = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  if (kept) snprintf(bucket, 33, "%s", PQgetvalue(r, 0, 0)); else api_log("crash: %s", PQerrorMessage(db));
  PQclear(r);
  if (kept) api_log("crash report kept: %s %s (bucket %s)", source, sig, bucket);
  return kept;
}
// The pages' crash events since the last minute become reports (at most 50 a run).
static int crash_fold(PGconn *db) {
  PGresult *r = PQexec(db, "SELECT o_ms, o_page, o_auto, o_rel, o_msg, o_trap, o_frames FROM verse.crash_page_take(50)");
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("crash fold: %s", PQerrorMessage(db)); PQclear(r); return 0; }
  int kept = 0;
  static char body[8193];
  for (int i = 0; i < PQntuples(r); i++) {
    char bucket[33];
    size_t n = crash_page_report_c(atoll(PQgetvalue(r, i, 0)), PQgetvalue(r, i, 1), PQgetvalue(r, i, 3), PQgetvalue(r, i, 4), PQgetvalue(r, i, 5), PQgetvalue(r, i, 6), body, sizeof body);
    if (n && crash_keep(db, body, n, !strcmp(PQgetvalue(r, i, 2), "t"), bucket)) kept++;
  }
  PQclear(r);
  return kept;
}
// The crash alerts into the array service_sync reads (service-wire.coax writes each): one a room
// bucket, one for the pages. False when the array could not hold one.
static bool crash_alerts(PGconn *db, long long now, sbuf *want) {
  char ns[24], ws[24];
  snprintf(ns, sizeof ns, "%lld", now); snprintf(ws, sizeof ws, "%lld", (long long)sv_crash_window());
  const char *v[2] = {ns, ws};
  PGresult *r = PQexecParams(db, "SELECT bucket, source, sig, age_ms, coalesce(shard, 0), release, buckets FROM verse.crash_facts($1::bigint, $2::bigint)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("crash alerts: %s", PQerrorMessage(db)); PQclear(r); return true; }
  bool held = true;
  for (int i = 0; i < PQntuples(r); i++) {
    if (!sv_crash_alert(atoll(PQgetvalue(r, i, 3)))) continue;
    bool room_s = !strcmp(PQgetvalue(r, i, 1), "room");
    coax_string what = sv_crash_text(room_s ? 0 : 1), todo = sv_crash_do();
    char key[48], title[512];
    if (room_s) {
      snprintf(key, sizeof key, "crash:%.8s", PQgetvalue(r, i, 0));
      snprintf(title, sizeof title, "shard %d: %.*s: %.200s (release %.80s)", atoi(PQgetvalue(r, i, 4)), (int)what.count, what.data, PQgetvalue(r, i, 2), PQgetvalue(r, i, 5));
    } else {
      snprintf(key, sizeof key, "crash:page");
      snprintf(title, sizeof title, "%.*s: %.200s (%d bucket%s in the last hour)", (int)what.count, what.data, PQgetvalue(r, i, 2), atoi(PQgetvalue(r, i, 6)), atoi(PQgetvalue(r, i, 6)) == 1 ? "" : "s");
    }
    held = held && W(want, strlen(title) + (size_t)todo.count + 160, svw_wanted_write(cs(key), 0, cs(title), todo, (int64_t)want->n, ROOM(want)));
  }
  PQclear(r);
  return held;
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
  sbuf want = {0}; bool held = true;
  for (int i = 0; i < PQntuples(r); i++) {
    int shard = atoi(PQgetvalue(r, i, 0));
    int64_t bits = sv_shard_alerts(!strcmp(PQgetvalue(r, i, 1), "t"), !strcmp(PQgetvalue(r, i, 2), "t"), atoll(PQgetvalue(r, i, 3)),
                                   atoll(PQgetvalue(r, i, 4)), atoll(PQgetvalue(r, i, 5)), atoll(PQgetvalue(r, i, 6)), atoll(PQgetvalue(r, i, 7)));
    for (int64_t k = 0; k < sv_alert_bits(); k++) {
      int64_t bit = (int64_t)1 << k;
      if (!(bits & bit)) continue;
      coax_string key = sv_alert_key(bit);
      char ks[40]; snprintf(ks, sizeof ks, "%.*s", (int)key.count, key.data);
      coax_string what = sv_alert_text(bit), todo = sv_alert_do(bit);
      held = held && W(&want, (size_t)(what.count + todo.count) + 160, svw_wanted_write(cs(ks), shard, what, todo, (int64_t)want.n, ROOM(&want)));
    }
  }
  PQclear(r);
  r = PQexec(db, "SELECT count(*), current_setting('max_connections')::int FROM pg_stat_activity WHERE backend_type = 'client backend'");
  int conns = -1, most = -1;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { conns = atoi(PQgetvalue(r, 0, 0)); most = atoi(PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (sv_db_alert(conns, most)) {
    coax_string what = sv_db_text(), todo = sv_db_do();
    held = held && W(&want, (size_t)(what.count + todo.count) + 160, svw_wanted_write(cs("db-connections"), 0, what, todo, (int64_t)want.n, ROOM(&want)));
  }
  crash_fold(db);                                  // the pages' crash events of the last minute, as reports (#13298)
  held = held && crash_alerts(db, now, &want);     // a room's bucket, the pages' one
  if (!held || !W(&want, 2, svw_wanted_end((int64_t)want.n, ROOM(&want)))) return NULL;
  const char *sv[2] = {ns, want.p};
  r = PQexecParams(db, "SELECT verse.service_sync($1::bigint, $2::jsonb)", 2, NULL, sv, NULL, NULL, 0);
  free(want.p);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("service-watch: %s", PQerrorMessage(db)); PQclear(r); return NULL; }
  char *sync = strdup(PQgetvalue(r, 0, 0));
  PQclear(r);
  if (!sync) return NULL;
  // Each change: the log, and mail to the owner.
  // Only production's alerts reach a person: staging's stay in the log, /status and the agents' inbox.
  const char *to = strcmp(env_name(), "production") ? NULL : getenv("OA_ALERT_TO");
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
  sbuf out = {0};
  bool ok = W(&out, strlen(sync) + 64, svw_watch_write(changes, mailed, cs(sync), ROOM(&out)));
  free(sync);
  return ok ? out.p : NULL;
}

// GET /api/playtest/service-health: the service levels and the open alerts.
static void health(const request *q) {
  if (strcmp(q->method, "GET") && strcmp(q->method, "HEAD")) { fail(q, 405, "GET the service health"); return; }
  PGconn *db = api_db();
  if (!db) { fail(q, 503, "no database"); return; }
  long long now = (long long)api_now_ms();
  char ns[24], days[8], t0[24], t1[24], t2[24];
  snprintf(ns, sizeof ns, "%lld", now); snprintf(days, sizeof days, "%lld", (long long)sv_days());
  snprintf(t0, sizeof t0, "%lld", (long long)sv_target(0)); snprintf(t1, sizeof t1, "%lld", (long long)sv_target(1));
  snprintf(t2, sizeof t2, "%lld", (long long)sv_target(2));
  const char *v[5] = {ns, days, t0, t1, t2};
  PGresult *r = PQexecParams(db, "SELECT which, good, total, worst, daily FROM verse.service_levels($1::bigint, $2::int, $3::bigint, $4::bigint, $5::bigint)",
                             5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("service-health: %s", PQerrorMessage(db)); PQclear(r); fail(q, 500, "no service levels"); return; }
  sbuf b = {0};
  bool held = W(&b, 128, svw_health_open(cs(env_name()), now, sv_days(), ROOM(&b)));
  for (int i = 0; i < PQntuples(r); i++) {
    int64_t which = atoll(PQgetvalue(r, i, 0)), good = atoll(PQgetvalue(r, i, 1)), total = atoll(PQgetvalue(r, i, 2));
    if (which < 0 || which > 2) continue;
    const char *worst = PQgetisnull(r, i, 3) ? "" : PQgetvalue(r, i, 3), *daily = PQgetvalue(r, i, 4);
    held = held && W(&b, strlen(worst) + strlen(daily) + 512,
                     svw_level_write(i != 0, which, sv_name(which), sv_target(which), sv_objective_ppm(which), good, total, sv_share_ppm(good, total),
                                     sv_met(which, good, total), cs(worst), cs(daily), (int64_t)b.n, ROOM(&b)));
  }
  PQclear(r);
  r = PQexec(db, "SELECT coalesce(jsonb_agg(jsonb_build_object('key', key, 'title', title, 'opened_ms', opened_ms, 'resolved_ms', resolved_ms) "
                 "ORDER BY resolved_ms IS NOT NULL, opened_ms DESC) FILTER (WHERE resolved_ms IS NULL), '[]'), "
                 "coalesce(jsonb_agg(jsonb_build_object('key', key, 'title', title, 'opened_ms', opened_ms, 'resolved_ms', resolved_ms) "
                 "ORDER BY resolved_ms DESC) FILTER (WHERE resolved_ms IS NOT NULL), '[]') "
                 "FROM verse.service_alerts WHERE resolved_ms IS NULL OR resolved_ms > (extract(epoch FROM now()) * 1000)::bigint - 86400000");
  bool got = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  const char *live = got ? PQgetvalue(r, 0, 0) : "[]", *done = got ? PQgetvalue(r, 0, 1) : "[]";
  held = held && W(&b, strlen(live) + strlen(done) + 64, svw_health_close(cs(live), cs(done), (int64_t)b.n, ROOM(&b)));
  if (!got) api_log("service-health: %s", PQerrorMessage(db));
  PQclear(r);
  if (held) { api_json(q->fd, 200, b.p); free(b.p); } else fail(q, 500, "out of memory");
}

bool service_route(const request *q) {
  if (strcmp(q->path, "/api/playtest/service-health")) return false;
  health(q);
  return true;
}
