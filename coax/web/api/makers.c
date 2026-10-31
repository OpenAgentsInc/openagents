// A realm's maker (ev-claim-realm, docs/verse/makers.md): who made a realm, and the Lightning
// address its tips go to. Every rule is SQL's (migrations/0072_realm_makers.sql); this file reads
// who asks.
//
//   GET  /api/playtest/makers/<realm>          -> 200 {realm, name, pubkey} (the maker also gets
//                                                 lightning and mine), 404 unclaimed
//   POST /api/playtest/makers/<realm>/claim {code, name, lightning}
//                                              -> 200 {realm, name, lightning}; 400, 401 not logged in,
//                                                 404 not this realm's code, 409 used already
//   POST /api/playtest/makers/<realm> {name?, lightning?}   the maker changes them; 403 anyone else
//   POST /api/admin/makers/<realm>/code        an admin (cookie or NIP-98): a new one-time code
//                                              -> 200 {realm, code, link}; the code is never logged or kept
//
// Who: the site session (oa_session, from /login) or a NIP-98 signature that binds the body and
// was never seen before. A POST from another site's page (Origin) is refused. The claim code
// travels in the link's fragment and the request body, never a path or query, so it never
// reaches a load balancer's logs.
#include "api.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PREFIX "/api/playtest/makers/"
#define ADMIN_PREFIX "/api/admin/makers/"

static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }

static bool slug_ok(const char *s, size_t n) {
  if (!n || n > 64) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9') || s[i] == '-')) return false;
  return true;
}

static char *body_of(const request *q) {
  char *b = malloc(q->body_len + 3);
  if (!b) return NULL;
  if (q->body_len) { memcpy(b, q->body, q->body_len); b[q->body_len] = 0; } else strcpy(b, "{}");
  return b;
}

// One SQL call answering {"status", ...}: sent as its status and the rest.
static void answer(const request *q, PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("makers: %s", PQerrorMessage(db));
    api_json(q->fd, 500, "{\"error\":\"not kept\"}");
  } else api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
}

// The logged-in key, or false (and the answer sent).
static bool who_asks(const request *q, PGconn *db, char who[65], bool post) {
  bool signed_req = false, payload = false, unseen = false;
  if (!site_identity(q, db, who, &signed_req, &payload, &unseen)) {
    api_json(q->fd, 401, "{\"error\":\"log in first\"}");
    return false;
  }
  if (signed_req && post && (!payload || !unseen)) {   // a signed write binds its body, once
    api_json(q->fd, 401, "{\"error\":\"log in first\"}");
    return false;
  }
  return true;
}

static void make_code(const request *q, PGconn *db, const char *realm) {
  char who[65];
  int s = admin_who(q, db, who);
  if (s) { api_json(q->fd, s, s == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return; }
  unsigned char raw[16]; char code[33];
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, raw, sizeof raw) != (ssize_t)sizeof raw) { if (fd >= 0) close(fd); api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return; }
  close(fd);
  for (int i = 0; i < 16; i++) snprintf(code + 2 * i, 3, "%02x", raw[i]);
  const char *v[3] = {realm, code, who};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x::text FROM makers.make_code($1, $2, $3) x", 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("makers: code: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not kept\"}"); }
  else if (atoi(PQgetvalue(r, 0, 0)) != 200) api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  else {
    char out[512];
    snprintf(out, sizeof out, "{\"realm\":\"%s\",\"code\":\"%s\",\"link\":\"%s/realms/%s/claim#code=%s\"}", realm, code, origin(), realm, code);
    api_json(q->fd, 200, out);
    api_log("makers: a claim code for %s", realm);
  }
  PQclear(r);
  memset(code, 0, sizeof code);
}

bool makers_route(const request *q) {
  bool admin = !strncmp(q->path, ADMIN_PREFIX, sizeof ADMIN_PREFIX - 1);
  if (!admin && strncmp(q->path, PREFIX, sizeof PREFIX - 1)) return false;
  const char *realm = q->path + (admin ? sizeof ADMIN_PREFIX : sizeof PREFIX) - 1;
  const char *slash = strchr(realm, '/');
  size_t n = slash ? (size_t)(slash - realm) : strlen(realm);
  const char *what = slash ? slash + 1 : "";
  if (!slug_ok(realm, n)) { api_json(q->fd, 404, "{\"error\":\"no such realm\"}"); return true; }
  char slug[65]; memcpy(slug, realm, n); slug[n] = 0;
  bool post = !strcmp(q->method, "POST");
  char from[256];
  if (post && api_header(q, "Origin", from, sizeof from) && strcmp(from, origin())) {
    api_json(q->fd, 403, "{\"error\":\"cross-site\"}");
    return true;
  }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  if (admin) {
    if (strcmp(what, "code")) { api_json(q->fd, 404, "{\"error\":\"no such route\"}"); return true; }
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST\"}"); return true; }
    make_code(q, db, slug);
    return true;
  }
  if (!*what && !post) {
    char who[65] = ""; bool a, b, c;
    if (!site_identity(q, db, who, &a, &b, &c)) who[0] = 0;
    const char *v[2] = {slug, who};
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.of($1, $2) x", 2, v);
    return true;
  }
  if (!post || (*what && strcmp(what, "claim"))) { api_json(q->fd, post ? 404 : 405, post ? "{\"error\":\"no such route\"}" : "{\"error\":\"POST\"}"); return true; }
  char who[65];
  if (!who_asks(q, db, who, true)) return true;
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  const char *v[3] = {slug, who, body};
  if (*what) {
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.claim($1, $2, $3) x", 3, v);
    api_log("makers: a claim for %s", slug);
  } else answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.edit($1, $2, $3) x", 3, v);
  memset(body, 0, q->body_len);
  free(body);
  return true;
}
