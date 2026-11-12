// Realm making behind invites (docs/auth/2026-10-31-realm-invites-and-auth.md;
// migrations/0090_realm_invites.sql holds every rule). A flag, a row of access.flags, off until an
// admin turns it on (no row is off): with realm_invites off, GET /api/playtest/access answers
// "make 1" and the POSTs here are 404, so every page and route behaves as before.
//
//   GET  /api/playtest/access                 text/plain: invites 0|1, make 0|1, in 0|1, creator 0|1, asked none|waiting|...
// (Off, those POSTs fall through to the routes that answered them before: 404.)
//   POST /api/playtest/access/redeem {code}   -> creator 1; 401 log in first; 404 that code isn't good; 429
//   POST /api/playtest/access/request {note, contact?}   -> asked waiting; 400, 401, 429
//        (?as=lines: the lines, or the error's words, as text/plain; else JSON)
//   GET  /api/admin/access                    text/plain rows: F|flag|0|1, I|invite..., R|request..., C|creator...
//   POST /api/admin/access/flag {name, on}
//   POST /api/admin/access/invites {uses?, days?, note?}   -> {id, code, link}: the code once, only its hash kept
//   POST /api/admin/access/invites/<id>/revoke
//   POST /api/admin/access/requests/<id>/approve | decline
//   POST /api/admin/access/creators {who}     a pubkey (hex) or an account id; .../creators/<who>/remove
//   POST /api/admin/access/grandfather        everyone who owns a made realm becomes a creator
//   GET  /api/admin/access/realms[?visibility=V][&as=lines]   every made realm: realm SLUG|VISIBILITY|REV|PUBLISHED|OWNER|NAME (0099)
//   GET  /api/admin/access/admins             A|npub|hex|root and A|npub|hex|db|added by|at|note (0100)
//   POST /api/admin/access/admins {pubkey, note?}   a root admin adds a database admin; .../admins/<hex>/remove
// Who: a logged-in session's key or a NIP-98 signature (site_identity, private.c); the player
// key (X-Verse-Player) for creators grandfathered as guests; a site admin (admin_who) always may.
// A POST from another site's page (Origin) is refused. Codes are never logged.
//   OA_ACCESS_WAITING       waiting requests the waitlist holds (2000)
#include "api.h"
#include "nostr.h"
#include "caller-policy.generated.h"
#include <ctype.h>
#include <signal.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static const char *origin(void) { return env_or("OA_API_ORIGIN", "https://openagents.com"); }

static bool as_lines(const request *q) { char v[16]; return api_query(q, "as", v, sizeof v) && !strcmp(v, "lines"); }
#define ADMIN_ROOTS_TEXT (32 * 65 + 8)   // admin_roots: admins.conf's keys, at most 32

static char *body_of(const request *q) {
  char *b = malloc(q->body_len + 3);
  if (!b) return NULL;
  if (q->body_len) { memcpy(b, q->body, q->body_len); b[q->body_len] = 0; } else strcpy(b, "{}");
  return b;
}

// ---- how fast: a fixed window a minute per address, per instance (as private.c's)
#define LIMITS 256
typedef struct { char key[96]; double since; int n; } limit;
static limit limits[LIMITS];
static bool allowed(const char *kind, const char *who, int per_minute) {
  char key[96]; snprintf(key, sizeof key, "%s:%.80s", kind, who);
  double now = api_now_ms();
  int free_slot = -1, oldest = 0;
  for (int i = 0; i < LIMITS; i++) {
    if (limits[i].key[0] && !strcmp(limits[i].key, key)) {
      if (now - limits[i].since > 60000) { limits[i].since = now; limits[i].n = 0; }
      return ++limits[i].n <= per_minute;
    }
    if (!limits[i].key[0] || now - limits[i].since > 60000) { if (free_slot < 0) free_slot = i; }
    else if (limits[i].since < limits[oldest].since) oldest = i;
  }
  int k = free_slot >= 0 ? free_slot : oldest;
  snprintf(limits[k].key, sizeof limits[k].key, "%s", key);
  limits[k].since = now; limits[k].n = 1;
  return true;
}
// The visitor's address: the load balancer's next to last.
static void visitor(const request *q, char ip[128]) {
  char xff[512];
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  ip[0] = 0;
  snprintf(ip, 128, "%s", np ? parts[np >= 2 ? np - 2 : 0] : "local");
}

static bool flag(PGconn *db, const char *name) {
  const char *v[1] = {name};
  PGresult *r = PQexecParams(db, "SELECT access.flag($1)", 1, NULL, v, NULL, NULL, 0);
  // Not there (the migration not applied) is off: the routes answer as before.
  bool on = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  return on;
}

// The logged-in key: a site session, or a signature that binds this body and is new. "" for nobody.
static void logged_in(const request *q, PGconn *db, char pk[65]) {
  bool signed_req = false, payload = false, unseen = false;
  if (!site_identity(q, db, pk, &signed_req, &payload, &unseen)) { pk[0] = 0; return; }
  if (signed_req && strcmp(q->method, "GET") && !(payload && unseen)) pk[0] = 0;
}

static void player_key(const request *q, char key[160]) {
  api_header(q, "X-Verse-Player", key, 160);
}

// ---- a NIP-98-signed caller (caller-policy.coax decides; this moves the facts in and out)
static bool first_seen(PGconn *db, const char *id) {
  const char *v[1] = {id};
  PGresult *r = PQexecParams(db, "INSERT INTO projects.seen (id, ms) VALUES ($1, projects.now_ms()) ON CONFLICT DO NOTHING", 1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  return ok;
}
int access_signed(const request *q, PGconn *db, char pk[65]) {
  char auth[4096];
  pk[0] = 0;
  bool signed_req = api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6);
  bool verified = false, payload = false, unseen = false;
  bool safe = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD");
  nostr_event ev;
  memset(&ev, 0, sizeof ev);
  if (signed_req) {
    char url[4300], err[160];
    snprintf(url, sizeof url, "%s%s", origin(), q->target);
    verified = nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err);
    payload = verified && ev.payload[0] != 0;
    if (cp_wants_seen(signed_req, verified, safe, payload)) unseen = first_seen(db, ev.id);
  }
  int64_t who = cp_caller(signed_req, verified, safe, payload, unseen);
  if (who == 0) return 0;
  if (who == 1) { memcpy(pk, ev.pubkey, 65); return 1; }
  coax_string why = cp_refusal_text(who);
  char out[160];
  if (as_lines(q)) api_respond(q->fd, 401, "text/plain; charset=utf-8", NULL, why.data, (size_t)why.count);
  else { snprintf(out, sizeof out, "{\"error\":\"%.*s\"}", (int)why.count, why.data); api_json(q->fd, 401, out); }
  return -1;
}
// The player key the signer's account acts with (auth.caller_key, migrations/0098): derived in SQL, never kept or shown.
bool access_caller_key(PGconn *db, const char *pk, char key[160]) {
  const char *v[1] = {pk};
  PGresult *r = PQexecParams(db, "SELECT auth.caller_key($1)", 1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) && PQgetlength(r, 0, 0) < 160;
  if (ok) snprintf(key, 160, "%s", PQgetvalue(r, 0, 0));
  else { key[0] = 0; api_log("access: caller key: %s", PQerrorMessage(db)); }
  PQclear(r);
  return ok;
}

// One answer from SQL: {status, body, lines}. Lines (as text/plain) when asked for and there.
static void answer(const request *q, PGconn *db, const char *sql, int n, const char *const *v, const char *what) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("access: %s: %s", what, PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (as_lines(q) && !PQgetisnull(r, 0, 2)) {
    const char *t = PQgetvalue(r, 0, 2);
    api_respond(q->fd, status, "text/plain; charset=utf-8", NULL, t, strlen(t));
  } else api_json(q->fd, status, PQgetvalue(r, 0, 1));
  PQclear(r);
}
#define RESULT "SELECT (x->>'status')::int, (x->'body')::text, x->>'lines' FROM "

static bool same_site(const request *q) {
  char from[256];
  if (!api_header(q, "Origin", from, sizeof from)) {
    char auth[16];
    return api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6);   // a signed tool
  }
  return !strcmp(from, origin());
}

// ---- realm creation's gate (realms.c): true when it answered (403 invite only), false to go on.
bool access_refused(const request *q, PGconn *db, const char *key, const char *signer) {
  if (!flag(db, "realm_invites")) return false;
  char who[65], pk[65];
  bool admin = signer && *signer ? admin_is(signer) : admin_who(q, db, who) == 0;
  if (admin) return false;
  if (signer && *signer) snprintf(pk, sizeof pk, "%s", signer); else logged_in(q, db, pk);
  const char *v[2] = {key ? key : "", pk};
  PGresult *r = PQexecParams(db, "SELECT access.gate($1, nullif($2, ''), false)", 2, NULL, v, NULL, NULL, 0);
  int status = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? atoi(PQgetvalue(r, 0, 0)) : 500;
  PQclear(r);
  if (status == 200) return false;
  if (status != 403) { api_json(q->fd, 500, "{\"error\":\"not read\"}"); return true; }
  if (as_lines(q)) api_respond(q->fd, 403, "text/plain; charset=utf-8", NULL, "invite only", 11);
  else api_json(q->fd, 403, "{\"error\":\"invite only\",\"need\":\"invite\"}");
  return true;
}

// ---- /api/playtest/access
static bool playtest(const request *q, PGconn *db, const char *rest) {
  bool post = !strcmp(q->method, "POST");
  char key[160], pk[65], who[65];
  player_key(q, key);
  if (!*rest) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET it\"}"); return true; }
    if (!flag(db, "realm_invites")) {   // off: today's answer, and nothing else is asked
      static const char off[] = "invites 0\nmake 1\nin 0\ncreator 0\nasked none\n";
      api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, off, sizeof off - 1);
      return true;
    }
    logged_in(q, db, pk);
    const char *admin = admin_who(q, db, who) == 0 ? "true" : "false";
    const char *v[3] = {key, pk, admin};
    PGresult *r = PQexecParams(db, "SELECT access.state($1, nullif($2, ''), $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else { api_log("access: state: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return true;
  }
  bool redeem = !strcmp(rest, "redeem"), ask = !strcmp(rest, "request");
  if (!redeem && !ask) { api_json(q->fd, 404, "{\"error\":\"not found\"}"); return true; }
  if (!flag(db, "realm_invites")) return false;   // off: whatever answered this path before answers it
  if (!post) { api_json(q->fd, 405, "{\"error\":\"POST it\"}"); return true; }
  if (!same_site(q)) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  char ip[128]; visitor(q, ip);
  if (!allowed(redeem ? "redeem" : "ask", ip, redeem ? 10 : 5)) {
    if (as_lines(q)) api_respond(q->fd, 429, "text/plain; charset=utf-8", "Retry-After: 60\r\n", "slow down", 9);
    else api_respond(q->fd, 429, "application/json", "Retry-After: 60\r\n", "{\"error\":\"slow down\"}", 21);
    return true;
  }
  if (q->body_len > 4096) { api_json(q->fd, 413, "{\"error\":\"too large\"}"); return true; }
  logged_in(q, db, pk);
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  char cap[16]; snprintf(cap, sizeof cap, "%d", atoi(env_or("OA_ACCESS_WAITING", "2000")));
  const char *v[3] = {body, pk, cap};
  if (redeem) answer(q, db, RESULT "access.redeem($1::jsonb, nullif($2, '')) x", 2, v, "redeem");
  else answer(q, db, RESULT "access.ask($1::jsonb, nullif($2, ''), $3::int) x", 3, v, "request");
  free(body);
  if (pk[0]) api_log("access %s by %.8s", rest, pk);
  return true;
}

// ---- /api/admin/access
// /worlds' text copy (realms/index.md, written by coax/web/realms.ts) links to /worlds/new. With
// realm_invites on it doesn't: the API writes the marker realms/invite-only.txt ("on" or "off") and
// the copy with or without the link into the site's bucket, in a child; coax/web/upload.py keeps the
// marker and drops the link from each release's copy while it says "on". Without a bucket (a dev
// server, the tests) nothing is written.
#define BUILD_LINK ", [build your own world](https://openagents.com/worlds/new)"
static char *replaced(const char *text, const char *from, const char *to) {
  const char *at = strstr(text, from);
  if (!at) return NULL;
  size_t a = (size_t)(at - text), f = strlen(from), t = strlen(to), n = strlen(text);
  char *out = malloc(n - f + t + 1);
  if (!out) return NULL;
  memcpy(out, text, a); memcpy(out + a, to, t); memcpy(out + a + t, at + f, n - a - f + 1);
  return out;
}
static void realms_twin(PGconn *db) {
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) return;
  bool on = flag(db, "realm_invites");
  signal(SIGCHLD, SIG_IGN);
  if (fork() != 0) return;
  bool kept = drops_site_write("realms/invite-only.txt", "text/plain; charset=utf-8", on ? "on\n" : "off\n");
  char *md = drops_site_read("realms/index.md"), *to = NULL;
  if (md) to = on ? replaced(md, BUILD_LINK, "") : (strstr(md, BUILD_LINK) ? NULL : replaced(md, "(https://openagents.com/verse).", "(https://openagents.com/verse)" BUILD_LINK "."));
  bool wrote = to && drops_site_write("realms/index.md", "text/markdown; charset=utf-8", to);
  api_log("access: realms twin %s: marker %s, copy %s", on ? "on" : "off", kept ? "kept" : "not kept", wrote ? "written" : "as it was");
  free(md); free(to);
  _exit(0);
}

static bool digits(const char *s, size_t n) {
  if (!n || n > 18) return false;
  for (size_t i = 0; i < n; i++) if (!isdigit((unsigned char)s[i])) return false;
  return true;
}
static bool admin(const request *q, PGconn *db, const char *rest) {
  char who[65];
  int s = admin_who(q, db, who);
  if (s) { api_json(q->fd, s == 403 ? 403 : 401, s == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return true; }
  bool post = !strcmp(q->method, "POST");
  if (!*rest) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET it\"}"); return true; }
    PGresult *r = PQexec(db, "SELECT access.admin_lines()");
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else { api_log("access: admin lines: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return true;
  }
  char roots[ADMIN_ROOTS_TEXT];
  admin_roots(roots, sizeof roots);
  if (!post && !strcmp(rest, "realms")) {   // every made realm and who sees it (0099), ?visibility= one of them
    char vis[16] = "";
    api_query(q, "visibility", vis, sizeof vis);
    const char *v[1] = {vis};
    PGresult *r = PQexecParams(db, "SELECT x->>'lines', (x->'body')::text FROM verse.realm_admin_rows(nullif($1, '')) x", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("access: realms: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    else if (as_lines(q)) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else api_json(q->fd, 200, PQgetvalue(r, 0, 1));
    PQclear(r);
    return true;
  }
  if (!post && !strcmp(rest, "admins")) {   // the roots (admins.conf) and the database's admins (0100)
    const char *v[1] = {roots};
    PGresult *r = PQexecParams(db, "SELECT access.admin_list($1::text[])", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else { api_log("access: admins: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return true;
  }
  if (!post) { api_json(q->fd, 405, "{\"error\":\"POST it\"}"); return true; }
  if (!same_site(q)) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  if (q->body_len > 4096) { api_json(q->fd, 413, "{\"error\":\"too large\"}"); return true; }
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  const char *v[3] = {body, who, origin()};
  const char *rv[3] = {body, who, roots};   // a change to the admins: only a root may (0100)
  if (!strcmp(rest, "flag")) { answer(q, db, RESULT "access.set_flag($1::jsonb, $2) x", 2, v, "flag"); realms_twin(db); }
  else if (!strcmp(rest, "invites")) answer(q, db, RESULT "access.mint($1::jsonb, $2, $3) x", 3, v, "mint");
  else if (!strcmp(rest, "creators")) answer(q, db, RESULT "access.add($1::jsonb, $2) x", 2, v, "add");
  else if (!strcmp(rest, "grandfather")) answer(q, db, RESULT "access.grandfather($2) x WHERE $1::text IS NOT NULL", 2, v, "grandfather");
  else if (!strcmp(rest, "admins")) answer(q, db, RESULT "access.admin_add($1::jsonb, $2, $3::text[]) x", 3, rv, "admin add");
  else {
    // invites/<id>/revoke, requests/<id>/approve|decline, creators/<who>/remove
    const char *slash = strchr(rest, '/'), *end = slash ? strchr(slash + 1, '/') : NULL;
    size_t k = end ? (size_t)(end - slash - 1) : 0;
    char id[80] = "";
    if (end && k < sizeof id) { memcpy(id, slash + 1, k); id[k] = 0; }
    const char *w[3] = {id, who, "false"};
    if (end && !strncmp(rest, "invites/", 8) && !strcmp(end, "/revoke") && digits(id, k))
      answer(q, db, RESULT "access.revoke($1::bigint) x", 1, w, "revoke");
    else if (end && !strncmp(rest, "requests/", 9) && (!strcmp(end, "/approve") || !strcmp(end, "/decline")) && digits(id, k)) {
      w[2] = !strcmp(end, "/approve") ? "true" : "false";
      answer(q, db, RESULT "access.decide($1::bigint, $3::boolean, $2) x", 3, w, "decide");
    } else if (end && !strncmp(rest, "creators/", 9) && !strcmp(end, "/remove") && k > 0)
      answer(q, db, RESULT "access.remove($1) x", 1, w, "remove");
    else if (end && !strncmp(rest, "admins/", 7) && !strcmp(end, "/remove") && k == 64) {
      const char *x[3] = {id, who, roots};
      answer(q, db, RESULT "access.admin_remove($1, $2, $3::text[]) x", 3, x, "admin remove");
    }
    else api_json(q->fd, 404, "{\"error\":\"not found\"}");
  }
  api_log("access admin %.24s by %.8s", rest, who);
  free(body);
  return true;
}

bool access_route(const request *q) {
  static const char play[] = "/api/playtest/access", adm[] = "/api/admin/access";
  const char *p = q->path, *rest;
  int which;
  if (!strncmp(p, play, sizeof play - 1) && (!p[sizeof play - 1] || p[sizeof play - 1] == '/')) { which = 0; rest = p + sizeof play - 1; }
  else if (!strncmp(p, adm, sizeof adm - 1) && (!p[sizeof adm - 1] || p[sizeof adm - 1] == '/')) { which = 1; rest = p + sizeof adm - 1; }
  else return false;
  if (*rest == '/') rest++;
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  return which == 0 ? playtest(q, db, rest) : admin(q, db, rest);
}
