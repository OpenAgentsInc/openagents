// Scheduled jobs (svc-schedules, docs/coaxial-services.md): Cloud Scheduler
// calls POST /api/jobs/<job> at a local time in America/Chicago (the scheduler
// keeps daylight saving), with a Google-signed OIDC token for the scheduler's
// service account. A job runs once per Chicago day; a run already done says so,
// and an admin can run a day again by hand (?day=YYYY-MM-DD&again=1).
//
// The every-minute jobs (EVERY: service-watch) run on each call, with no claim on the day.
//
//   POST /api/jobs/<job>[?day=...&again=1]
//     the scheduler (OIDC, OA_JOBS_CALLER, audience OA_API_ORIGIN/api/jobs) or an admin
//     200 {"job","day","state":"ran"|"already ran","result"}; 401 anyone else; 404 no such job
//
//   OA_JOBS_CALLER    the scheduler's service account email
//   OA_TOKENINFO_URL  Google's ID token check (tests point it at a fake)
//
// The token is checked by Google's tokeninfo endpoint (signature, expiry), and
// here for its issuer, audience, caller and a verified email: a few calls a
// day, so no key cache.
#include "api.h"
#include "jobs-wire.generated.h"

#include <curl/curl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

// Every body this file sends is written by jobs-wire.coax (jobs-wire.generated.c; Protocol E11,
// #13250): a refusal (the API's ErrorReply), a run's answer, and the two jobs' own results.
static coax_string cs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static void fail(int fd, int status, const char *why) {
  char out[200];
  int64_t n = jbw_error_write(cs(why), (coax_v_u8){(uint8_t *)out, sizeof out - 1});
  out[n < 0 ? 0 : n] = 0; api_json(fd, status, out);
}
// A run's answer; day "" for a job with no day, result NULL for null.
static void ran(int fd, int status, const char *name, const char *day, const char *state, const char *result) {
  size_t cap = strlen(result ? result : "") + 512; char *out = malloc(cap);
  if (!out) { fail(fd, 500, "out of memory"); return; }
  int64_t n = jbw_run_write(cs(name), cs(day), cs(state), cs(result ? result : ""), (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  if (n < 0) { free(out); fail(fd, 500, "out of memory"); return; }
  out[n] = 0; api_json(fd, status, out); free(out);
}

typedef struct { char *data; size_t n; } buf;
static size_t keep(char *p, size_t size, size_t count, void *u) {
  buf *b = u; size_t k = size * count;
  if (b->n + k > 65536) return 0;
  char *g = realloc(b->data, b->n + k + 1); if (!g) return 0;
  b->data = g; memcpy(b->data + b->n, p, k); b->n += k; b->data[b->n] = 0;
  return k;
}

// True when Authorization is a Google ID token for the scheduler, for us.
static bool scheduler(const request *q, PGconn *db) {
  const char *caller = getenv("OA_JOBS_CALLER");
  char auth[4096];
  if (!caller || !*caller || !api_header(q, "Authorization", auth, sizeof auth) || strncmp(auth, "Bearer ", 7)) return false;
  for (const char *p = auth + 7; *p; p++)
    if (!((*p >= 'A' && *p <= 'Z') || (*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || *p == '-' || *p == '_' || *p == '.')) return false;
  CURL *h = curl_easy_init(); if (!h) return false;
  buf b = {0};
  char url[4600]; snprintf(url, sizeof url, "%s?id_token=%s", env_or("OA_TOKENINFO_URL", "https://oauth2.googleapis.com/tokeninfo"), auth + 7);
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, keep); curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 10L); curl_easy_setopt(h, CURLOPT_NOSIGNAL, 1L);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_easy_cleanup(h);
  bool ok = false;
  if (status == 200 && b.data) {
    char aud[600], now[32];
    snprintf(aud, sizeof aud, "%s/api/jobs", env_or("OA_API_ORIGIN", "https://openagents.com"));
    snprintf(now, sizeof now, "%lld", (long long)(api_now_ms() / 1000));
    const char *v[4] = {b.data, aud, caller, now};
    PGresult *r = PQexecParams(db,
        "SELECT t->>'iss' IN ('https://accounts.google.com', 'accounts.google.com') AND t->>'aud' = $2 AND t->>'email' = $3 "
        "AND t->>'email_verified' = 'true' AND (t->>'exp')::bigint > $4::bigint FROM (SELECT $1::jsonb t) x",
        4, NULL, v, NULL, NULL, 0);
    ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
    PQclear(r);
  }
  free(b.data);
  return ok;
}

// The jobs. Each one does its day's work and answers its result as JSON; a
// failed job answers NULL and its day can run again.
static char *job_heartbeat(PGconn *db, const char *day) {
  (void)db;
  char *out = malloc(64); if (!out) return NULL;
  int64_t n = jbw_heartbeat_write(cs(day), (coax_v_u8){(uint8_t *)out, 63});
  if (n < 0) { free(out); return NULL; }
  out[n] = 0;
  return out;
}
// Our own login's housekeeping (auth.c, #13202, #13203): old rate-limit windows, ended
// sessions and spent logins go; audit rows older than two years lose their account.
static char *job_auth_sweep(PGconn *db, const char *day) {
  // The two deletes are WITH items of the statement itself: Postgres refuses a data-modifying WITH
  // inside a sub-select ("must be at the top level"), which made every run of this job fail (#13250).
  PGresult *r = PQexec(db, "WITH p AS (DELETE FROM auth.pending WHERE expires_at < now() - interval '1 day' RETURNING 1), "
                           "s AS (DELETE FROM auth.nip98_seen WHERE expires_at < now() RETURNING 1) "
                           "SELECT auth.limit_sweep(), auth.audit_age(), (SELECT count(*) FROM p), (SELECT count(*) FROM s), "
                           // Accounts whose 30 days are up are purged to tombstones; ended merge asks go (#13214).
                           "coalesce(cardinality(auth.purge_due()), 0), auth.life_sweep()");
  char *out = NULL;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && (out = malloc(260))) {
    int64_t n = jbw_sweep_write(cs(day), cs(PQgetvalue(r, 0, 0)), cs(PQgetvalue(r, 0, 1)), cs(PQgetvalue(r, 0, 2)), cs(PQgetvalue(r, 0, 3)),
                                cs(PQgetvalue(r, 0, 4)), cs(PQgetvalue(r, 0, 5)), (coax_v_u8){(uint8_t *)out, 259});
    if (n < 0) { free(out); out = NULL; api_log("job auth-sweep: a count is not a number"); } else out[n] = 0;
  }
  else api_log("job auth-sweep: %s", PQerrorMessage(db));
  PQclear(r);
  return out;
}
static const struct { const char *name; char *(*run)(PGconn *db, const char *day); } JOBS[] = {
  {"heartbeat", job_heartbeat},   // proves the schedule: one run each morning
  {"auth-sweep", job_auth_sweep},
};
// Jobs that run on every call, not once a day: the minute's service watch (service.c, #13297).
static const struct { const char *name; char *(*run)(PGconn *db, const char *day); } EVERY[] = {
  {"service-watch", service_watch},
};

bool jobs_route(const request *q) {
  if (strncmp(q->path, "/api/jobs/", 10)) return false;
  const char *name = q->path + 10;
  int which = -1;
  for (size_t i = 0; i < sizeof JOBS / sizeof JOBS[0]; i++) if (!strcmp(name, JOBS[i].name)) which = (int)i;
  if (strcmp(q->method, "POST")) { fail(q->fd, 405, "POST to run a job"); return true; }
  PGconn *db = api_db();
  if (!db) { fail(q->fd, 503, "no database"); return true; }
  char who[65];
  bool by_admin = false;
  if (!scheduler(q, db)) {
    if (admin_who(q, db, who)) { fail(q->fd, 401, "the scheduler or an admin only"); return true; }
    by_admin = true;
  }
  for (size_t i = 0; which < 0 && i < sizeof EVERY / sizeof EVERY[0]; i++) {
    if (strcmp(name, EVERY[i].name)) continue;
    char *result = EVERY[i].run(db, "");
    if (!result) { api_log("job %s: failed", name); fail(q->fd, 500, "the job failed"); return true; }
    ran(q->fd, 200, name, "", "ran", result);
    free(result);
    return true;
  }
  if (which < 0) { fail(q->fd, 404, "no such job"); return true; }
  char day[16] = "", again[8] = "";
  api_query(q, "day", day, sizeof day);
  api_query(q, "again", again, sizeof again);
  if ((*day || *again) && !by_admin) { fail(q->fd, 403, "only an admin runs a day by hand"); return true; }
  // The Chicago day (today, or the admin's), and this job's claim on it.
  const char *v[3] = {name, day, !strcmp(again, "1") ? "t" : "f"};
  PGresult *r = PQexecParams(db, "SELECT x->>'state', x->>'day', x->>'result' FROM jobs.claim($1, nullif($2, ''), $3::boolean) x", 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); fail(q->fd, 400, "day: YYYY-MM-DD"); return true; }
  char state[32], d[16];
  snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(d, sizeof d, "%s", PQgetvalue(r, 0, 1));
  if (strcmp(state, "run")) {
    ran(q->fd, strcmp(state, "already ran") ? 409 : 200, name, d, state, PQgetisnull(r, 0, 2) ? NULL : PQgetvalue(r, 0, 2));
    PQclear(r); return true;
  }
  PQclear(r);
  char *result = JOBS[which].run(db, d);
  const char *f[3] = {name, d, result};
  PQclear(PQexecParams(db, "SELECT jobs.finish($1, $2::date, $3::jsonb)", 3, NULL, f, NULL, NULL, 0));
  api_log("job %s %s: %s%s", name, d, result ? "ran" : "failed", by_admin ? " (by hand)" : "");
  if (!result) { fail(q->fd, 500, "the job failed; its day can run again"); return true; }
  ran(q->fd, 200, name, d, "ran", result);
  free(result);
  return true;
}
