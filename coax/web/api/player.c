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
//                                             (room_ticket below; coax/web/room/room-ticket.coax checks it);
//                                             ?realm=R: the instance and shard from the directory (rooms.c),
//                                             answered as "room" and "shard" (#12717)
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
// A look or class word as kept: [a-z0-9.], at most cap - 1 ("" when anything else is in it).
static void ticket_word(char *out, size_t cap, const char *in) {
  size_t m = 0;
  for (const char *p = in; *p; p++) {
    if (m + 1 >= cap || !((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || *p == '.')) { out[0] = 0; return; }
    out[m++] = *p;
  }
  out[m] = 0;
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
  // the saved state (verse.room_vitals, #12715): HP for 10 minutes, WANTED for 5, the look and class always
  PGresult *r = PQexecParams(db, "SELECT p.id, p.account IS NOT NULL, CASE WHEN verse.name_ok(a.name) THEN a.name WHEN verse.name_ok(nullif($2, '')::jsonb->>'name') THEN nullif($2, '')::jsonb->>'name' ELSE verse.guest_name(p.id) END, "
                                 "CASE WHEN rv.hp_at > extract(epoch FROM clock_timestamp()) * 1000 - 600000 THEN rv.hp END, "
                                 "CASE WHEN rv.wanted_at > extract(epoch FROM clock_timestamp()) * 1000 - 300000 THEN rv.wanted END, rv.look, rv.class, "
                                 "EXISTS (SELECT 1 FROM verse.room_kicks k WHERE k.player = p.id AND k.until_ms > extract(epoch FROM clock_timestamp()) * 1000) "
                                 "FROM verse.players p LEFT JOIN verse.accounts a ON a.id = p.account "
                                 "LEFT JOIN verse.room_vitals rv ON rv.who = coalesce(p.account, p.id) "
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
  if (PQgetvalue(r, 0, 7)[0] == 't') { PQclear(r); api_json(q->fd, 403, "{\"error\":\"kicked\"}"); return; }   /* an admin's kick, 24 hours (#12714) */
  int hp = PQgetisnull(r, 0, 3) ? -1 : atoi(PQgetvalue(r, 0, 3));
  int wanted = PQgetisnull(r, 0, 4) ? -1 : atoi(PQgetvalue(r, 0, 4));
  char look[40] = "", klass[24] = "";
  if (!PQgetisnull(r, 0, 5)) ticket_word(look, sizeof look, PQgetvalue(r, 0, 5));
  if (!PQgetisnull(r, 0, 6)) ticket_word(klass, sizeof klass, PQgetvalue(r, 0, 6));
  PQclear(r);
  for (char *c = pid; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-')) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  if (nn) b64url(name64, sizeof name64, (const unsigned char *)name, nn); else strcpy(name64, "-");
  // ?realm=REALM (#12717): the directory (rooms.c) gives the instance and the shard that holds it,
  // and the ticket names that shard (v3), which seats it and no other. Without it, a v2 ticket for the
  // room asked, which shard 1 seats (a page from before the directory).
  char realm[32] = "", placed[32] = "";
  int shard = 0; bool full = false;
  if (api_query(q, "realm", realm, sizeof realm) && realm[0]) {
    if (!rooms_pick(db, realm, room, pid, placed, sizeof placed, &shard, &full)) { api_json(q->fd, 503, "{\"error\":\"every room is full: try again in a minute\"}"); return; }
    room = placed;
  }
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long expires = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000 + TICKET_MS;
  char text[256];
  char hps[16]; if (hp >= 0 && hp < 100000) snprintf(hps, sizeof hps, "%d", hp); else strcpy(hps, "-");
  int tn = shard > 0 ? snprintf(text, sizeof text, "v3.%s.%d.%s.%s.%d.%lld.%s", pid, account ? 1 : 0, name64, room, shard, expires, hps)
                     : snprintf(text, sizeof text, "v2.%s.%d.%s.%s.%lld.%s", pid, account ? 1 : 0, name64, room, expires, hps);
  if (tn <= 0 || (size_t)tn + 66 >= sizeof text) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)text, (size_t)tn, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  text[tn++] = '.';
  for (unsigned i = 0; i < 32; i++) tn += snprintf(text + tn, sizeof text - (size_t)tn, "%02x", mac[i]);
  char out[768], en[48], st[160] = "";
  esc(en, sizeof en, name);
  int sn = 0;
  if (hp >= 0) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"hp\":%d", hp);
  if (wanted >= 0) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"wanted\":%d", wanted);
  if (look[0]) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"look\":\"%s\"", look);
  if (klass[0]) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"class\":\"%s\"", klass);
  if (shard > 0) snprintf(st + sn, sizeof st - (size_t)sn, ",\"room\":\"%s\",\"shard\":%d%s", room, shard, full ? ",\"full\":true" : "");
  snprintf(out, sizeof out, "{\"ticket\":\"%s\",\"id\":\"%s\",\"account\":%s,\"name\":\"%s\",\"expires\":%lld%s}", text, pid, account ? "true" : "false", en, expires, st);
  api_json(q->fd, 200, out);
}

// The page's part of what a player brings back into a room (#12715): {"wanted":N,"look":"...","class":"..."},
// any of them, kept by the account when logged in, else the player.
static void room_state(const request *q, PGconn *db, const char *key, const char *body) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST the state\"}"); return; }
  if (!key || !key[0]) { api_json(q->fd, 401, "{\"error\":\"a player key\"}"); return; }
  const char *v[2] = {key, body};
  PGresult *r = PQexecParams(db,
    "WITH b AS (SELECT $2::jsonb AS j), p AS (SELECT coalesce(account, id) AS who FROM verse.players "
    "  WHERE key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')), "
    "s AS (SELECT CASE WHEN jsonb_typeof(j->'wanted') = 'number' AND (j->>'wanted')::numeric BETWEEN 0 AND 5 THEN (j->>'wanted')::int END AS wanted, "
    "  CASE WHEN j->>'look' ~ '^[a-z0-9.]{1,32}$' THEN j->>'look' END AS look, "
    "  CASE WHEN j->>'class' ~ '^[a-z]{1,16}$' THEN j->>'class' END AS klass, "
    "  (extract(epoch FROM clock_timestamp()) * 1000)::bigint AS now FROM b) "
    "INSERT INTO verse.room_vitals (who, wanted, wanted_at, look, class, at) "
    "SELECT p.who, s.wanted, CASE WHEN s.wanted IS NOT NULL THEN s.now END, s.look, s.klass, s.now FROM p, s "
    "ON CONFLICT (who) DO UPDATE SET wanted = coalesce(EXCLUDED.wanted, verse.room_vitals.wanted), "
    "  wanted_at = coalesce(EXCLUDED.wanted_at, verse.room_vitals.wanted_at), look = coalesce(EXCLUDED.look, verse.room_vitals.look), "
    "  class = coalesce(EXCLUDED.class, verse.room_vitals.class), at = EXCLUDED.at RETURNING who",
    2, NULL, v, NULL, NULL, 0);
  ExecStatusType st = PQresultStatus(r);
  int n = st == PGRES_TUPLES_OK ? PQntuples(r) : -1;
  if (n < 0) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-state: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  PQclear(r);
  if (n != 1) { api_json(q->fd, 403, "{\"error\":\"unknown player key\"}"); return; }
  api_json(q->fd, 200, "{\"kept\":true}");
}
// oa-room's part (#12715): "t MS\nPID HP MAX\n..." signed with the seat-ticket key (X-Room-Mac: the
// hex HMAC-SHA256 of the body), within two minutes of now. Each seat's HP is kept by who the
// player is, unless a newer one is already kept.
static void room_vitals(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST vitals\"}"); return; }
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  if (!secret || strlen(secret) < 32) { api_json(q->fd, 503, "{\"error\":\"no ticket key\"}"); return; }
  char given[80];
  if (!api_header(q, "X-Room-Mac", given, sizeof given) || strlen(given) != 64 || q->body_len == 0 || q->body_len > 8192) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)q->body, q->body_len, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no mac\"}"); return; }
  char want[65]; for (unsigned i = 0; i < 32; i++) snprintf(want + 2 * i, 3, "%02x", mac[i]);
  unsigned diff = 0; for (int i = 0; i < 64; i++) diff |= (unsigned)(want[i] ^ given[i]);
  if (diff) { api_json(q->fd, 401, "{\"error\":\"bad mac\"}"); return; }
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  long long at = 0;
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  if (sscanf(b, "t %lld", &at) != 1 || at < now - 120000 || at > now + 120000) { free(b); api_json(q->fd, 400, "{\"error\":\"stale\"}"); return; }
  int kept = 0;
  char *line = strchr(b, '\n');
  while (line && *++line) {
    char pid[48]; int hp, max;
    char *next = strchr(line, '\n');
    if (sscanf(line, "%47s %d %d", pid, &hp, &max) == 3 && hp >= 0 && max > 0 && hp <= max && max < 100000) {
      char hs[16], ms[16], as[24];
      snprintf(hs, sizeof hs, "%d", hp); snprintf(ms, sizeof ms, "%d", max); snprintf(as, sizeof as, "%lld", at);
      const char *v[4] = {pid, hs, ms, as};
      PGresult *r = PQexecParams(db,
        "INSERT INTO verse.room_vitals (who, hp, hp_max, hp_at, at) "
        "SELECT coalesce(account, id), $2::int, $3::int, $4::bigint, $4::bigint FROM verse.players WHERE id = $1 "
        "ON CONFLICT (who) DO UPDATE SET hp = EXCLUDED.hp, hp_max = EXCLUDED.hp_max, hp_at = EXCLUDED.hp_at, at = EXCLUDED.at "
        "WHERE verse.room_vitals.hp_at IS NULL OR verse.room_vitals.hp_at <= EXCLUDED.hp_at", 4, NULL, v, NULL, NULL, 0);
      if (PQresultStatus(r) == PGRES_COMMAND_OK) kept += atoi(PQcmdTuples(r));
      else api_log("room-vitals: %s", PQerrorMessage(db));
      PQclear(r);
    }
    line = next;
  }
  free(b);
  char out[64]; snprintf(out, sizeof out, "{\"kept\":%d}", kept);
  api_json(q->fd, 200, out);
}

// What oa-room asks every two seconds (#12714): "pvp 0|1\nedit STAMP\nkick PID\n..." for Everglade: its
// pvp row, when its world or board last changed (a publish or a switch; the room says "e edit" to
// every page then) and who is kicked. Signed like room-vitals: X-Room-Mac over "t MS" (X-Room-At).
static void room_control(const request *q, PGconn *db) {
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  if (!secret || strlen(secret) < 32) { api_json(q->fd, 503, "{\"error\":\"no ticket key\"}"); return; }
  char given[80], at[32];
  if (!api_header(q, "X-Room-Mac", given, sizeof given) || strlen(given) != 64 || !api_header(q, "X-Room-At", at, sizeof at)) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  char text[48]; int tn = snprintf(text, sizeof text, "t %s", at);
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (tn <= 0 || !HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)text, (size_t)tn, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no mac\"}"); return; }
  char want[65]; for (unsigned i = 0; i < 32; i++) snprintf(want + 2 * i, 3, "%02x", mac[i]);
  unsigned diff = 0; for (int i = 0; i < 64; i++) diff |= (unsigned)(want[i] ^ given[i]);
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000, when = atoll(at);
  if (diff || when < now - 120000 || when > now + 120000) { api_json(q->fd, 401, "{\"error\":\"bad mac\"}"); return; }
  PGresult *r = PQexecParams(db,
    "SELECT 'pvp ' || CASE WHEN coalesce((SELECT (switched->>'pvp')::boolean FROM verse.realm_edit WHERE slug = 'everglade'), false) THEN '1' ELSE '0' END || chr(10) || "
    "'edit ' || greatest(coalesce((SELECT ms FROM verse.realm_edit WHERE slug = 'everglade'), 0), "
    "  coalesce((SELECT (published_at * 1000)::bigint FROM verse.published WHERE world = 'everglade'), 0)) || chr(10) || "
    "coalesce((SELECT string_agg('kick ' || player, chr(10)) || chr(10) FROM verse.room_kicks WHERE until_ms > verse.now_ms()), '')",
    0, NULL, NULL, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("room-control: %s", PQerrorMessage(db)); PQclear(r); api_json(q->fd, 500, "{\"error\":\"no control\"}"); return; }
  const char *t = PQgetvalue(r, 0, 0);
  api_respond(q->fd, 200, "text/plain", NULL, t, strlen(t));
  PQclear(r);
}
// An Everglade admin's kick (#12714): {"player": "p-...", "on": false to let them back}. 24 hours, every room.
static void room_kick(const request *q, PGconn *db, const char *key, bool site, const char *body) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a kick\"}"); return; }
  const char *v[3] = {key ? key : "", body, site ? "t" : "f"};
  PGresult *r = PQexecParams(db,
    "WITH b AS (SELECT $2::jsonb AS j), may AS (SELECT verse.edit_may('everglade', $1, $3::boolean) AS ok), "
    "them AS (SELECT id FROM verse.players, b WHERE id = j->>'player'), "
    "del AS (DELETE FROM verse.room_kicks WHERE (SELECT ok FROM may) AND (SELECT j->'on' = 'false'::jsonb FROM b) AND player = (SELECT id FROM them) RETURNING 1), "
    "ins AS (INSERT INTO verse.room_kicks (player, by_who, until_ms, at_ms) SELECT id, coalesce(verse.player_of($1), 'site'), verse.now_ms() + 86400000, verse.now_ms() "
    "  FROM them, b WHERE (SELECT ok FROM may) AND j->'on' IS DISTINCT FROM 'false'::jsonb "
    "  ON CONFLICT (player) DO UPDATE SET by_who = EXCLUDED.by_who, until_ms = EXCLUDED.until_ms, at_ms = EXCLUDED.at_ms RETURNING 1) "
    "SELECT (SELECT ok FROM may), EXISTS (SELECT 1 FROM them), (SELECT count(*) FROM ins), (SELECT count(*) FROM del)",
    3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-kick: %s", PQerrorMessage(db));
    PQclear(r); api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"no kick\"}"); return;
  }
  bool may = PQgetvalue(r, 0, 0)[0] == 't', them = PQgetvalue(r, 0, 1)[0] == 't';
  bool in = atoi(PQgetvalue(r, 0, 2)) > 0;
  PQclear(r);
  if (!may) { api_json(q->fd, 403, "{\"error\":\"Everglade's admins only\"}"); return; }
  if (!them) { api_json(q->fd, 404, "{\"error\":\"no such player\"}"); return; }
  api_json(q->fd, 200, in ? "{\"kicked\":true,\"hours\":24}" : "{\"kicked\":false}");
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
  // oa-room's HP report (#12715): signed with the seat-ticket key, not a player's
  if (!strcmp(rest, "room-vitals")) { room_vitals(q, db); return true; }
  if (!strcmp(rest, "room-control")) { room_control(q, db); return true; }
  // the Everglade instance directory (rooms.c, #12717): a shard's report, and the instances' list
  if (rooms_route(q, db, rest)) return true;

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
  if (!strcmp(rest, "room-state")) { room_state(q, db, key, body); free(body); return true; }
  if (!strcmp(rest, "room-kick")) { room_kick(q, db, key, site, body); free(body); return true; }

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
