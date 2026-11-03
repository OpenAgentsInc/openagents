// Players, guests, saves and published worlds for the Verse playtest
// (pt-player-endpoints): the demo's player and world routes (aethyr-cpp
// tools/serve.py, docs/verse/playtest-port.md section 3) under
// /api/playtest/. The rules live in SQL (migrations/0004_verse_players.sql,
// verse.api); this file reads who is asking and hands the request over.
//
//   POST /api/playtest/player                 a guest: mints a player key {key, id, account}, or
//                                             echoes the sent one. Pages send it as X-Verse-Player
//   GET  /api/playtest/player                 {id, account, worlds} for the sent key
//   POST /api/playtest/join                   {email, player, sid, how}: join by email, never
//                                             verified (a format check); kept as a playtester and
//                                             the key given an account of its own -> {joined, player}
//   POST /api/playtest/signin                 {sid}, NIP-98 signed: the key, its worlds, realm, saves
//                                             and sessions move to the pubkey's account -> {player, moved}
//   POST /api/playtest/worlds                 a new world owned by the player -> {id}
//   GET  /api/playtest/world/<id>             the published world {doc, version, publishedBy,
//                                             publishedAt}, or {doc: null, version: 0}: a link opens it
//   POST /api/playtest/world/<id>             publish {doc, summary}: the owner (for a hand-built
//                                             realm also its realm_rights owner and admins), or anyone
//                                             known on a world its owner opened; else 403 with offer: true
//   GET  /api/playtest/world/<id>/owner       {claimed, you, site, open, name}
//   POST /api/playtest/world/<id>/open        {open}: the owner opens or closes direct publishing
//   GET/POST /api/playtest/world/<id>/settings  {view: {view: third|first|top, allowToggle}}
//   POST /api/playtest/world/<id>/reset       the owner takes the published world down
//   GET  /api/playtest/log/<id>               the edit log, newest first
//   GET/POST /api/playtest/progress/<world>/<name>   an account's save {current, previous}, each
//                                             {bytes (base64 AESV), at (ms)}; 409 when a later one is there
//   POST /api/playtest/progress/<world>/<name>/erase
//   POST /api/playtest/change/<id>[/<cid>/accept|decline]   offered changes (0010_verse_offers.sql)
//   POST /api/playtest/award, GET/POST /api/playtest/planets[/mine|/<id>]   a player's realm (0011_verse_planets.sql)
//   POST /api/playtest/survey/answer, /api/playtest/survey   feedback (below)
//   POST /api/playtest/room-ticket/<room>     {name}: a signed seat ticket for that room's socket
//                                             (room_ticket below; coax/web/room/room-ticket.coax checks it)
//
// Who asks: the X-Verse-Player key; a NIP-98 signed request (Authorization:
// Nostr) adds that pubkey's account, and an admin's signature (OA_ADMIN_PUBKEYS)
// is the site, which owns the built-in worlds; so is an admin logged in on the
// site (oa_session, site_session below). A bad signature, or one seen before,
// is refused. The automation mark is the analytics one.
#include "api.h"
#include "nostr.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <openssl/hmac.h>
#include <openssl/evp.h>

#define PREFIX "/api/playtest/"

// A JSON string's contents, escaped, into out.
static size_t esc(char *out, size_t cap, const char *v) {
  size_t n = 0;
  for (const unsigned char *p = (const unsigned char *)v; *p && n + 8 < cap; p++) {
    if (*p == '"' || *p == '\\') { out[n++] = '\\'; out[n++] = (char)*p; }
    else if (*p < 0x20) n += (size_t)snprintf(out + n, cap - n, "\\u%04x", *p);
    else out[n++] = (char)*p;
  }
  out[n] = 0;
  return n;
}

static uint32_t crc32_of(const unsigned char *b, size_t n) {
  uint32_t c = 0xFFFFFFFFu;
  for (size_t i = 0; i < n; i++) {
    c ^= b[i];
    for (int k = 0; k < 8; k++) c = (c >> 1) ^ (0xEDB88320u & (0u - (c & 1u)));
  }
  return c ^ 0xFFFFFFFFu;
}

// An engine save (aethyr-cpp docs/save.md, tools/players.py container_ok):
// "AESV", container version 1 or 2, the progress layer, a good CRC-32 last.
static bool container_ok(const unsigned char *b, size_t n) {
  if (n < 18 || memcmp(b, "AESV", 4) || (b[4] != 1 && b[4] != 2) || b[5] != 2) return false;
  uint32_t want = (uint32_t)b[n - 4] | (uint32_t)b[n - 3] << 8 | (uint32_t)b[n - 2] << 16 | (uint32_t)b[n - 1] << 24;
  return crc32_of(b, n - 4) == want;
}

// The automation the playtest reads: header X-Verse-Automation, else cookie verse-automation.
static void automation(const request *q, char *out, size_t cap) {
  char cookies[2048];
  if (api_header(q, "X-Verse-Automation", out, cap) && *out) return;
  out[0] = 0;
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return;
  const char *c = strstr(cookies, "verse-automation=");
  if (!c) return;
  c += strlen("verse-automation=");
  size_t i = 0;
  while (c[i] && c[i] != ';' && c[i] != ' ' && i + 1 < cap) { out[i] = c[i]; i++; }
  out[i] = 0;
}

// A signed event used once only (verse.nostr_seen).
static bool first_use(PGconn *db, const nostr_event *ev) {
  const char *v[1] = {ev->id};
  PGresult *r = PQexecParams(db, "INSERT INTO verse.nostr_seen (id, ms) VALUES ($1, verse.now_ms()) ON CONFLICT DO NOTHING",
                             1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  return ok;
}

// An admin logged in on the site (/login's oa_session, admin.c admin_who) is
// the site too, so an admin edits the built-in worlds from the page itself
// (eg-edit: Everglade's edit mode). A write sent from another site's page
// never counts, whatever cookie it carries.
static bool site_session(const request *q, PGconn *db) {
  char from[256], who[65];
  const char *o = getenv("OA_API_ORIGIN");
  if (!strcmp(q->method, "POST") && api_header(q, "Origin", from, sizeof from) && strcmp(from, o && *o ? o : "https://openagents.com")) return false;
  return admin_who(q, db, who) == 0;
}

// Feedback (pt-feedback): POST /api/playtest/survey/answer, one answer
// {rid, q, a, skip, n, of, player, sid, user, joinedAs}, and POST
// /api/playtest/survey, the whole form; verse.feedback keeps them
// (migrations/0006_verse_feedback.sql) with the session they came from.
static void feedback(const request *q, PGconn *db, const char *parts) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST feedback\"}"); return; }
  char autom[64];
  automation(q, autom, sizeof autom);
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *v[3] = {parts, body, autom};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.feedback($1::text[], $2::jsonb, $3) x",
                             3, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("feedback: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
}

// The demo's paths for what moved under /api/playtest/: /api/change/...,
// /api/log/<id>, /api/award and /api/planets... answer as they did.
static const char *legacy(const char *path) {
  static const char *const kept[] = {"change/", "log/", "award", "planets"};
  if (strncmp(path, "/api/", 5)) return NULL;
  const char *rest = path + 5;
  for (size_t i = 0; i < sizeof kept / sizeof kept[0]; i++) {
    size_t n = strlen(kept[i]);
    if (!strncmp(rest, kept[i], n) && (kept[i][n - 1] == '/' || rest[n] == 0 || rest[n] == '/')) return rest;
  }
  return NULL;
}

// A seat ticket (#12712, spec 4.4): who the X-Verse-Player key is, for one room's socket, for
// TICKET_MS. Its text, which coax/web/room/room-ticket.coax reads and checks:
//   v1.PID.ACCOUNT.NAME.ROOM.EXPIRES.MAC
// PID the player's public id, ACCOUNT 1 when the key has an account (0 a guest), NAME the display name
// as base64url without padding ("-" none), ROOM the room's code, EXPIRES unix ms, MAC the lowercase hex
// of HMAC-SHA256 over everything before ".MAC", keyed with OA_ROOM_TICKET_KEY's bytes (Secret Manager
// oa-room-ticket-key, which oa-room holds too). The name is the account's when it has one, else the
// one asked for ({"name": ...}), kept to letters, digits, spaces, '-' and '_', 24 at most.
#define TICKET_MS 600000
static void b64url(char *out, size_t cap, const unsigned char *b, size_t n) {
  static const char a[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  size_t o = 0;
  for (size_t i = 0; i < n && o + 5 < cap; i += 3) {
    unsigned v = (unsigned)b[i] << 16 | (i + 1 < n ? (unsigned)b[i + 1] << 8 : 0) | (i + 2 < n ? b[i + 2] : 0);
    out[o++] = a[v >> 18 & 63]; out[o++] = a[v >> 12 & 63];
    if (i + 1 < n) out[o++] = a[v >> 6 & 63];
    if (i + 2 < n) out[o++] = a[v & 63];
  }
  out[o] = 0;
}
static size_t ticket_name(char *out, size_t cap, const char *in) {
  size_t m = 0;
  for (const unsigned char *p = (const unsigned char *)(in ? in : ""); *p && m + 1 < cap && m < 24; p++) {
    unsigned char ch = *p;
    bool ok = (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || (ch == ' ' && m > 0) || ch == '-' || ch == '_';
    if (ok) out[m++] = (char)ch;
  }
  while (m > 0 && out[m - 1] == ' ') m--;
  out[m] = 0;
  return m;
}
static void room_ticket(const request *q, PGconn *db, const char *key, const char *room, const char *body) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a ticket\"}"); return; }
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  if (!secret || strlen(secret) < 32) { api_json(q->fd, 503, "{\"error\":\"no ticket key\"}"); return; }
  size_t rn = strlen(room);
  if (!rn || rn > 24) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  for (size_t i = 0; i < rn; i++) {
    char ch = room[i];
    if (!((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '-')) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  }
  if (!key || !key[0]) { api_json(q->fd, 401, "{\"error\":\"a player key\"}"); return; }
  const char *v[2] = {key, body};
  PGresult *r = PQexecParams(db, "SELECT p.id, p.account IS NOT NULL, coalesce(nullif(a.name, ''), nullif($2, '')::jsonb->>'name', '') "
                                 "FROM verse.players p LEFT JOIN verse.accounts a ON a.id = p.account "
                                 "WHERE p.key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-ticket: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"no ticket\"}");
    return;
  }
  if (PQntuples(r) != 1) { PQclear(r); api_json(q->fd, 403, "{\"error\":\"unknown player key\"}"); return; }
  char pid[64], name[32], name64[48];
  snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0));
  bool account = PQgetvalue(r, 0, 1)[0] == 't';
  size_t nn = ticket_name(name, sizeof name, PQgetvalue(r, 0, 2));
  PQclear(r);
  for (char *c = pid; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-')) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  if (nn) b64url(name64, sizeof name64, (const unsigned char *)name, nn); else strcpy(name64, "-");
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long expires = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000 + TICKET_MS;
  char text[256];
  int tn = snprintf(text, sizeof text, "v1.%s.%d.%s.%s.%lld", pid, account ? 1 : 0, name64, room, expires);
  if (tn <= 0 || (size_t)tn + 66 >= sizeof text) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)text, (size_t)tn, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  text[tn++] = '.';
  for (unsigned i = 0; i < 32; i++) tn += snprintf(text + tn, sizeof text - (size_t)tn, "%02x", mac[i]);
  char out[512], en[48];
  esc(en, sizeof en, name);
  snprintf(out, sizeof out, "{\"ticket\":\"%s\",\"id\":\"%s\",\"account\":%s,\"name\":\"%s\",\"expires\":%lld}", text, pid, account ? "true" : "false", en, expires);
  api_json(q->fd, 200, out);
}

bool player_route(const request *q) {
  const char *rest = legacy(q->path);
  if (!rest && (strncmp(q->path, PREFIX, strlen(PREFIX)) || !strcmp(q->path, "/api/playtest/health"))) return false;
  if (!rest) rest = q->path + strlen(PREFIX);
  if (strcmp(q->method, "GET") && strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }

  // The path after the prefix, as a Postgres text[] literal: segments are
  // [A-Za-z0-9_-] only (anything else is not a route).
  char parts[600]; size_t pn = 0; int segs = 0;
  parts[pn++] = '{';
  for (const char *p = rest; *p; p++) {
    char ch = *p;
    if (ch == '/') { if (p[1] == 0) break; if (pn + 2 >= sizeof parts) break; parts[pn++] = ','; continue; }
    bool ok = (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '-';
    if (!ok || pn + 2 >= sizeof parts) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
    if (p == rest || p[-1] == '/') segs++;
    parts[pn++] = ch;
  }
  parts[pn++] = '}'; parts[pn] = 0;
  if (!segs || segs > 4) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  if (!strncmp(rest, "survey", 6) && (rest[6] == 0 || rest[6] == '/')) { feedback(q, db, parts); return true; }

  char key[160], autom[64], auth[4096], npub[65] = "";
  bool site = false;
  api_header(q, "X-Verse-Player", key, sizeof key);
  automation(q, autom, sizeof autom);
  if (api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6)) {
    const char *origin = getenv("OA_API_ORIGIN");
    char url[4200];
    snprintf(url, sizeof url, "%s%s", origin && *origin ? origin : "https://openagents.com", q->target);
    nostr_event ev; char err[160];
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) {
      char out[256]; snprintf(out, sizeof out, "{\"error\":\"nip98: %s\"}", err);
      api_json(q->fd, 401, out); return true;
    }
    if (!first_use(db, &ev)) { api_json(q->fd, 401, "{\"error\":\"nip98: that signature was used already\"}"); return true; }
    memcpy(npub, ev.pubkey, sizeof npub);
    site = admin_is(npub);
  } else if (site_session(q, db)) site = true;
  // A hand-built realm's owner or one of its admins (verse.realm_rights, docs/verse/realm-edit.md)
  // publishes that realm's world, and puts it back, as the site does: for that world's routes only.
  if (!site && key[0] && !strncmp(rest, "world/", 6)) {
    char id[80]; size_t i = 0;
    for (const char *p = rest + 6; *p && *p != '/' && i + 1 < sizeof id; p++) id[i++] = *p;
    id[i] = 0;
    const char *ev[2] = {id, key};
    PGresult *m = PQexecParams(db, "SELECT (verse.edit_hand($1)).slug IS NOT NULL AND verse.edit_may($1, $2, false)", 2, NULL, ev, NULL, NULL, 0);
    if (PQresultStatus(m) == PGRES_TUPLES_OK && PQntuples(m) == 1 && PQgetvalue(m, 0, 0)[0] == 't') site = true;
    PQclear(m);
  }

  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; }
  else strcpy(body, "{}");
  if (!strncmp(rest, "room-ticket/", 12) && segs == 2) { room_ticket(q, db, key, rest + 12, body); free(body); return true; }

  // A progress save: its bytes decoded and checked here, where CRC-32 is cheap.
  bool progress_post = !strcmp(q->method, "POST") && !strncmp(rest, "progress/", 9) && segs == 3;
  bool good = false;
  const char *b64 = NULL;
  PGresult *dec = NULL;
  if (progress_post) {
    const char *v[1] = {body};
    dec = PQexecParams(db, "SELECT decode($1::jsonb->>'bytes', 'base64'), $1::jsonb->>'bytes'", 1, NULL, v, NULL, NULL, 1);
    if (PQresultStatus(dec) == PGRES_TUPLES_OK && PQntuples(dec) == 1 && !PQgetisnull(dec, 0, 0)) {
      size_t n = (size_t)PQgetlength(dec, 0, 0);
      good = n <= 16u << 20 && container_ok((const unsigned char *)PQgetvalue(dec, 0, 0), n);
      b64 = PQgetvalue(dec, 0, 1);
    } else if (PQresultStatus(dec) != PGRES_TUPLES_OK) {
      PQclear(dec); free(body);
      api_json(q->fd, 400, "{\"error\":\"bytes must be base64 and at a time\"}");
      return true;
    }
  }

  // who: what verse.api needs to know about the asker.
  size_t wcap = 1024 + (b64 ? strlen(b64) : 0);
  char *who = malloc(wcap);
  char ek[400], ea[160];
  esc(ek, sizeof ek, key); esc(ea, sizeof ea, autom);
  char after[24] = "";
  if (api_query(q, "after", after, sizeof after))
    for (char *c = after; *c; c++) if (*c < '0' || *c > '9') { after[0] = 0; break; }
  size_t wn = (size_t)snprintf(who, wcap, "{\"key\":\"%s\",\"auto\":\"%s\",\"npub\":\"%s\",\"site\":%s,\"good\":%s,\"after\":\"%s\"", ek, ea, npub,
                               site ? "true" : "false", good ? "true" : "false", after);
  if (b64 && good) wn += (size_t)snprintf(who + wn, wcap - wn, ",\"bytes\":\"%s\"", b64);   // base64 needs no escaping
  snprintf(who + wn, wcap - wn, "}");
  if (dec) PQclear(dec);

  const char *v[4] = {q->method, parts, body, who};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.api($1, $2::text[], $3::jsonb, $4::jsonb) x",
                             4, NULL, v, NULL, NULL, 0);
  free(body); free(who);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("playtest: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  api_json(q->fd, status, PQgetvalue(r, 0, 1));
  PQclear(r);
  return true;
}
