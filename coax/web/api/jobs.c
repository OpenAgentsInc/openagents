// Scheduled jobs (svc-schedules, docs/coaxial-services.md): Cloud Scheduler
// calls POST /api/jobs/<job> at a local time in America/Chicago (the scheduler
// keeps daylight saving), with a Google-signed OIDC token for the scheduler's
// service account. A job runs once per Chicago day; a run already done says so,
// and an admin can run a day again by hand (?day=YYYY-MM-DD&again=1).
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

#include <curl/curl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

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
  char *out = malloc(64); if (out) snprintf(out, 64, "{\"day\":\"%s\"}", day);
  return out;
}
static const struct { const char *name; char *(*run)(PGconn *db, const char *day); } JOBS[] = {
  {"heartbeat", job_heartbeat},   // proves the schedule: one run each morning
};

bool jobs_route(const request *q) {
  if (strncmp(q->path, "/api/jobs/", 10)) return false;
  const char *name = q->path + 10;
  int which = -1;
  for (size_t i = 0; i < sizeof JOBS / sizeof JOBS[0]; i++) if (!strcmp(name, JOBS[i].name)) which = (int)i;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST to run a job\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char who[65];
  bool by_admin = false;
  if (!scheduler(q, db)) {
    if (admin_who(q, db, who)) { api_json(q->fd, 401, "{\"error\":\"the scheduler or an admin only\"}"); return true; }
    by_admin = true;
  }
  if (which < 0) { api_json(q->fd, 404, "{\"error\":\"no such job\"}"); return true; }
  char day[16] = "", again[8] = "";
  api_query(q, "day", day, sizeof day);
  api_query(q, "again", again, sizeof again);
  if ((*day || *again) && !by_admin) { api_json(q->fd, 403, "{\"error\":\"only an admin runs a day by hand\"}"); return true; }
  // The Chicago day (today, or the admin's), and this job's claim on it.
  const char *v[3] = {name, day, !strcmp(again, "1") ? "t" : "f"};
  PGresult *r = PQexecParams(db, "SELECT x->>'state', x->>'day', x->>'result' FROM jobs.claim($1, nullif($2, ''), $3::boolean) x", 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); api_json(q->fd, 400, "{\"error\":\"day: YYYY-MM-DD\"}"); return true; }
  char state[32], d[16];
  snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(d, sizeof d, "%s", PQgetvalue(r, 0, 1));
  if (strcmp(state, "run")) {
    char out[1024]; snprintf(out, sizeof out, "{\"job\":\"%s\",\"day\":\"%s\",\"state\":\"%s\",\"result\":%s}", name, d, state, PQgetisnull(r, 0, 2) ? "null" : PQgetvalue(r, 0, 2));
    PQclear(r); api_json(q->fd, strcmp(state, "already ran") ? 409 : 200, out); return true;
  }
  PQclear(r);
  char *result = JOBS[which].run(db, d);
  const char *f[3] = {name, d, result};
  PQclear(PQexecParams(db, "SELECT jobs.finish($1, $2::date, $3::jsonb)", 3, NULL, f, NULL, NULL, 0));
  api_log("job %s %s: %s%s", name, d, result ? "ran" : "failed", by_admin ? " (by hand)" : "");
  if (!result) { api_json(q->fd, 500, "{\"error\":\"the job failed; its day can run again\"}"); return true; }
  size_t n = strlen(result) + 256; char *out = malloc(n);
  if (out) { snprintf(out, n, "{\"job\":\"%s\",\"day\":\"%s\",\"state\":\"ran\",\"result\":%s}", name, d, result); api_json(q->fd, 200, out); free(out); }
  else api_json(q->fd, 500, "{\"error\":\"out of memory\"}");
  free(result);
  return true;
}
