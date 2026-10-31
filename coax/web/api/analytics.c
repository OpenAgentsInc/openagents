// Analytics capture: POST /api/analytics/events takes a page's batch
// (apps/openagents-web/analytics.coax: {"sid", "events": [...], "auto"}) and
// keeps it in analytics.events with what this request says about the visit:
// the address, the User-Agent, the host, the automation marks the playtest
// reads (header X-Verse-Automation, cookie verse-automation) and the load
// balancer's geography headers (coax/web/api/lb.sh). A request signed with
// NIP-98 also ties the session to that Nostr pubkey. The cleaning and the
// limits are the playtest's, in SQL (migrations/0002_events.sql
// analytics.ingest), so an import and a live page are kept by one rule.
//
// Our own testing is not usage (migrations/0037_analytics_team.sql): a batch
// from the team's browser (cookie oa_team=1, set when an admin logs in), from
// an admin's site session (cookie oa_session) or key, a team player or a
// team device is kept with auto = 'team'. A team session's answer sets
// oa_team, so that browser stays the team's after it logs out. On start the
// service writes the team (OA_TEAM from team.conf, and OA_ADMIN_PUBKEYS) into
// analytics.team and runs analytics.team_tag(), which backfills.
#include "api.h"
#include "nostr.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

// Appends `v` as a JSON string field to `out` (n so far): `,"k":"v"`.
static size_t field(char *out, size_t n, size_t cap, const char *k, const char *v) {
  if (!v || !*v || n + 8 + strlen(k) >= cap) return n;
  n += (size_t)snprintf(out + n, cap - n, "%s\"%s\":\"", n > 1 ? "," : "", k);
  for (const unsigned char *p = (const unsigned char *)v; *p && n + 8 < cap; p++) {
    if (*p == '"' || *p == '\\') { out[n++] = '\\'; out[n++] = (char)*p; }
    else if (*p < 0x20) n += (size_t)snprintf(out + n, cap - n, "\\u%04x", *p);
    else out[n++] = (char)*p;
  }
  if (n + 2 < cap) out[n++] = '"';
  out[n] = 0;
  return n;
}

// The automation the playtest reads from a cookie: verse-automation=<tool>.
static void cookie_auto(const char *cookies, char *out, size_t cap) {
  out[0] = 0;
  const char *c = strstr(cookies, "verse-automation=");
  if (!c) return;
  c += strlen("verse-automation=");
  size_t i = 0;
  while (c[i] && c[i] != ';' && c[i] != ' ' && i + 1 < cap) { out[i] = c[i]; i++; }
  out[i] = 0;
}

// A cookie's value: name=<value> up to ';' or a space, at most cap - 1 of [A-Za-z0-9_-].
static void cookie_value(const char *cookies, const char *name, char *out, size_t cap) {
  out[0] = 0;
  size_t w = strlen(name);
  for (const char *c = strstr(cookies, name); c; c = strstr(c + 1, name)) {
    if ((c != cookies && c[-1] != ' ' && c[-1] != ';') || c[w] != '=') continue;
    c += w + 1;
    size_t i = 0;
    while (i + 1 < cap && ((c[i] >= 'a' && c[i] <= 'z') || (c[i] >= 'A' && c[i] <= 'Z') || (c[i] >= '0' && c[i] <= '9') || c[i] == '_' || c[i] == '-')) {
      out[i] = c[i]; i++;
    }
    out[i] = 0;
    return;
  }
}

// Appends the tokens of `list` (split at commas and spaces) to a JSON array of
// `kind` in out: kind:value tokens of OA_TEAM when `kind` is set, else the
// 64-hex keys of OA_ADMIN_PUBKEYS. Only [A-Za-z0-9_-] gets through.
static size_t team_list(char *out, size_t n, size_t cap, const char *list, const char *kind) {
  bool first = true;
  for (const char *p = list ? list : ""; *p;) {
    while (*p == ',' || *p == ' ' || *p == '\n' || *p == '\t') p++;
    const char *t = p;
    while (*p && *p != ',' && *p != ' ' && *p != '\n' && *p != '\t') p++;
    size_t len = (size_t)(p - t);
    const char *v = t;
    if (kind) {
      size_t k = strlen(kind);
      if (len <= k + 1 || strncmp(t, kind, k) || t[k] != ':') continue;
      v = t + k + 1; len -= k + 1;
    } else if (len != 64) continue;
    bool ok = len > 0 && len < 100;
    for (size_t i = 0; ok && i < len; i++)
      ok = (v[i] >= 'a' && v[i] <= 'z') || (v[i] >= 'A' && v[i] <= 'Z') || (v[i] >= '0' && v[i] <= '9') || v[i] == '_' || v[i] == '-';
    if (!ok || n + len + 8 >= cap) continue;
    n += (size_t)snprintf(out + n, cap - n, "%s\"%.*s\"", first ? "" : ",", (int)len, v);
    first = false;
  }
  return n;
}

void analytics_team_init(PGconn *db) {
  const char *team = getenv("OA_TEAM");
  static char cfg[16384];
  size_t n = (size_t)snprintf(cfg, sizeof cfg, "{\"npub\":[");
  n = team_list(cfg, n, sizeof cfg, getenv("OA_ADMIN_PUBKEYS"), NULL);
  n += (size_t)snprintf(cfg + n, sizeof cfg - n, "],\"player\":[");
  n = team_list(cfg, n, sizeof cfg, team, "player");
  n += (size_t)snprintf(cfg + n, sizeof cfg - n, "],\"email_sha256\":[");
  n = team_list(cfg, n, sizeof cfg, team, "email_sha256");
  snprintf(cfg + n, sizeof cfg - n, "]}");
  const char *v[1] = {cfg};
  PGresult *r = PQexecParams(db, "SELECT analytics.team_set($1::jsonb), analytics.team_tag()::text", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_log("team: %s kept; marked %s", PQgetvalue(r, 0, 0), PQgetvalue(r, 0, 1));
  else api_log("team: %s", PQerrorMessage(db));
  PQclear(r);
}

static void ingest(const request *q) {
  if (q->body_len == 0 || q->body_len > 256 * 1024) { api_json(q->fd, 400, "{\"error\":\"no batch\"}"); return; }
  char ip[128], ua[512], host[256], automation[64], cookies[2048], auth[4096], tmp[256];
  char region[64], subdivision[64], city[128], latlong[64], rtt[32], tls[64], device[64], player[160];
  api_header(q, "X-Forwarded-For", ip, sizeof ip);
  char *comma = strchr(ip, ','); if (comma) *comma = 0;   // the first address is the visitor's
  api_header(q, "User-Agent", ua, sizeof ua);
  api_header(q, "Host", host, sizeof host);
  api_header(q, "X-Verse-Automation", automation, sizeof automation);
  char team[4] = "", site[72] = "";
  if (api_header(q, "Cookie", cookies, sizeof cookies)) {
    if (!automation[0]) cookie_auto(cookies, automation, sizeof automation);
    cookie_value(cookies, "oa_team", team, sizeof team);
    cookie_value(cookies, "oa_session", site, sizeof site);
  }
  api_header(q, "X-Client-Region", region, sizeof region);
  api_header(q, "X-Client-Subdivision", subdivision, sizeof subdivision);
  api_header(q, "X-Client-City", city, sizeof city);
  api_header(q, "X-Client-Latlong", latlong, sizeof latlong);
  api_header(q, "X-Client-Rtt", rtt, sizeof rtt);
  api_header(q, "X-Client-Tls", tls, sizeof tls);
  api_header(q, "X-Client-Device", device, sizeof device);
  api_header(q, "X-Verse-Player", player, sizeof player);

  // A NIP-98 signed batch: the session is that pubkey's. A bad signature is refused.
  char npub[65] = "";
  if (api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6)) {
    const char *origin = getenv("OA_API_ORIGIN");
    char url[4096];
    snprintf(url, sizeof url, "%s%s", origin && *origin ? origin : "https://openagents.com", q->target);
    nostr_event ev; char err[160];
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) {
      char out[256]; snprintf(out, sizeof out, "{\"error\":\"nip98: %s\"}", err);
      api_json(q->fd, 401, out); return;
    }
    memcpy(npub, ev.pubkey, sizeof npub);
  }

  char ctx[4096]; size_t n = 0;
  ctx[n++] = '{'; ctx[n] = 0;
  snprintf(tmp, sizeof tmp, "%lld", (long long)api_now_ms());
  n += (size_t)snprintf(ctx + n, sizeof ctx - n, "\"ms\":%s", tmp);
  n = field(ctx, n, sizeof ctx, "ip", ip);
  n = field(ctx, n, sizeof ctx, "ua", ua);
  n = field(ctx, n, sizeof ctx, "host", host);
  n = field(ctx, n, sizeof ctx, "auto", automation);
  n = field(ctx, n, sizeof ctx, "npub", npub);
  n = field(ctx, n, sizeof ctx, "region", region);
  n = field(ctx, n, sizeof ctx, "subdivision", subdivision);
  n = field(ctx, n, sizeof ctx, "city", city);
  n = field(ctx, n, sizeof ctx, "latlong", latlong);
  n = field(ctx, n, sizeof ctx, "rtt", rtt);
  n = field(ctx, n, sizeof ctx, "tls", tls);
  n = field(ctx, n, sizeof ctx, "device", device);
  n = field(ctx, n, sizeof ctx, "key", player);
  n = field(ctx, n, sizeof ctx, "team", !strcmp(team, "1") ? "1" : "");
  n = field(ctx, n, sizeof ctx, "site", site);
  if (n + 2 < sizeof ctx) { ctx[n++] = '}'; ctx[n] = 0; }

  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  char *body = malloc(q->body_len + 1);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  const char *params[2] = {body, ctx};
  PGresult *r = PQexecParams(db, "SELECT analytics.ingest($1::jsonb, $2::jsonb)", 2, NULL, params, NULL, NULL, 0);
  // A team session marks its browser (oa_team) for the visits after it.
  bool mark = false;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && atoi(PQgetvalue(r, 0, 0)) >= 0 && strcmp(team, "1")) {
    PGresult *t = PQexecParams(db, "SELECT 1 FROM analytics.sessions WHERE sid = ($1::jsonb)->>'sid' AND team", 1, NULL, params, NULL, NULL, 0);
    mark = PQresultStatus(t) == PGRES_TUPLES_OK && PQntuples(t) == 1;
    PQclear(t);
  }
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    // bad JSON is the page's fault; anything else is ours
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("ingest: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  int kept = atoi(PQgetvalue(r, 0, 0));
  PQclear(r);
  if (kept < 0) { api_json(q->fd, 400, "{\"error\":\"not a batch\"}"); return; }
  char out[64]; snprintf(out, sizeof out, "{\"kept\":%d}", kept);
  api_respond(q->fd, 200, "application/json", mark ? TEAM_COOKIE : NULL, out, strlen(out));
}

// GET /api/analytics/realms: how much each realm is played, for /realms (realmlist.coax), a line a
// realm: "slug|visitors|visits|minutes". A visit is a page-view of /realms/<slug> (or its old
// /realm/<slug>); visitors are its distinct devices (pid); minutes run from each visit to the
// session's next page-view or its last event, at most three hours a visit. The filter is
// /admin/analytics' (admin.c KEPT): a session with an automation mark, an agent's session-start or
// our team's mark (auto = 'team', migrations/0037_analytics_team.sql) counts for nothing.
// Counts only, no visitor's data; kept a minute so a busy page costs one query.
#define REALM_VISITS \
  "WITH pv AS (SELECT e.sid, e.ms, e.p->>'pid' AS pid, " \
  "    substring(coalesce(e.p->>'path', '') from '^/realms?/([a-z0-9-]+)(/.*)?$') AS slug, " \
  "    lead(e.ms) OVER (PARTITION BY e.sid ORDER BY e.ms, e.id) AS next_ms " \
  "  FROM analytics.events e WHERE e.ev = 'page-view' AND e.space = 'web'), " \
  "k AS (SELECT pv.* FROM pv WHERE pv.slug IS NOT NULL AND NOT EXISTS (SELECT 1 FROM analytics.events x " \
  "  WHERE x.sid = pv.sid AND (x.auto IS NOT NULL OR (x.ev = 'session-start' AND x.p ? 'agent')))) " \
  "SELECT k.slug, count(DISTINCT coalesce(nullif(k.pid, ''), k.sid))::text, count(*)::text, " \
  "  round(sum(least(greatest(coalesce(k.next_ms, s.last_ms, k.ms) - k.ms, 0), 10800000)) / 60000.0)::bigint::text " \
  "FROM k LEFT JOIN analytics.sessions s USING (sid) GROUP BY k.slug ORDER BY count(*) DESC, k.slug LIMIT 200"

static void realm_visits(const request *q) {
  static char cached[16384]; static size_t cached_n; static double cached_at = -1e18;
  double now = api_now_ms();
  if (now - cached_at > 60000) {
    PGconn *db = api_db();
    if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
    PGresult *r = PQexec(db, REALM_VISITS);
    if (PQresultStatus(r) != PGRES_TUPLES_OK) {
      api_log("realm visits: %s", PQerrorMessage(db));
      PQclear(r);
      api_json(q->fd, 500, "{\"error\":\"not now\"}");
      return;
    }
    size_t n = 0;
    for (int i = 0; i < PQntuples(r); i++) {
      int w = snprintf(cached + n, sizeof cached - n, "%s|%s|%s|%s\n", PQgetvalue(r, i, 0), PQgetvalue(r, i, 1), PQgetvalue(r, i, 2), PQgetvalue(r, i, 3));
      if (w < 0 || (size_t)w >= sizeof cached - n) break;
      n += (size_t)w;
    }
    PQclear(r);
    cached_n = n; cached_at = now;
  }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, cached, cached_n);
}

bool analytics_route(const request *q) {
  if (!strcmp(q->path, "/api/analytics/realms")) {
    if (strcmp(q->method, "GET")) api_json(q->fd, 405, "{\"error\":\"GET\"}");
    else realm_visits(q);
    return true;
  }
  if (strcmp(q->path, "/api/analytics/events")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a batch\"}"); return true; }
  ingest(q);
  return true;
}
