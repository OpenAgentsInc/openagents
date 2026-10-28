// Analytics capture: POST /api/analytics/events takes a page's batch
// (apps/openagents-web/analytics.coax: {"sid", "events": [...], "auto"}) and
// keeps it in analytics.events with what this request says about the visit:
// the address, the User-Agent, the host, the automation marks the playtest
// reads (header X-Verse-Automation, cookie verse-automation) and the load
// balancer's geography headers (coax/web/api/lb.sh). A request signed with
// NIP-98 also ties the session to that Nostr pubkey. The cleaning and the
// limits are the playtest's, in SQL (migrations/0002_events.sql
// analytics.ingest), so an import and a live page are kept by one rule.
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

static void ingest(const request *q) {
  if (q->body_len == 0 || q->body_len > 256 * 1024) { api_json(q->fd, 400, "{\"error\":\"no batch\"}"); return; }
  char ip[128], ua[512], host[256], automation[64], cookies[2048], auth[4096], tmp[256];
  char region[64], subdivision[64], city[128], latlong[64], rtt[32], tls[64], device[64], player[160];
  api_header(q, "X-Forwarded-For", ip, sizeof ip);
  char *comma = strchr(ip, ','); if (comma) *comma = 0;   // the first address is the visitor's
  api_header(q, "User-Agent", ua, sizeof ua);
  api_header(q, "Host", host, sizeof host);
  api_header(q, "X-Verse-Automation", automation, sizeof automation);
  if (!automation[0] && api_header(q, "Cookie", cookies, sizeof cookies)) cookie_auto(cookies, automation, sizeof automation);
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
  if (n + 2 < sizeof ctx) { ctx[n++] = '}'; ctx[n] = 0; }

  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  char *body = malloc(q->body_len + 1);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  const char *params[2] = {body, ctx};
  PGresult *r = PQexecParams(db, "SELECT analytics.ingest($1::jsonb, $2::jsonb)", 2, NULL, params, NULL, NULL, 0);
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
  api_json(q->fd, 200, out);
}

bool analytics_route(const request *q) {
  if (strcmp(q->path, "/api/analytics/events")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a batch\"}"); return true; }
  ingest(q);
  return true;
}
