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
//   GET/POST /api/playtest/progress/<world>/<name>   an account's save {rev, current, previous}, each
//                                             {bytes (base64 AESV), at (ms)}; a POST {bytes, rev} is kept
//                                             only from the revision there now, else 409 with what is
//                                             (progress-policy.coax, migrations/0119_progress_revisions.sql)
//   POST /api/playtest/progress/<world>/<name>/erase
//   POST /api/playtest/change/<id>[/<cid>/accept|decline]   offered changes (0010_verse_offers.sql)
//   POST /api/playtest/award, GET/POST /api/playtest/planets[/mine|/<id>]   a player's realm (0011_verse_planets.sql)
//   POST /api/playtest/survey/answer, /api/playtest/survey   feedback (below)
//   POST /api/playtest/room-ticket/<room>     a signed seat ticket for that room's socket
//                                             (room_ticket below; coax/web/room/room-ticket.coax checks it);
//                                             ?realm=R: the instance and shard from the directory (rooms.c),
//                                             answered as "room" and "shard" (#12717), and the
//                                             shard's "endpoint" when a room VM names one (#13179)
//                                             ?with=P: the friend whose invite holds them a seat (#12795);
//                                             a full asked room adds "asked" and the friend's name "by"
//                                             ?party=P (with ?realm, no room named): the room party member P
//                                             sits in (#12911); "party_full" when full, "party_away" when P
//                                             is in no room of yours to join
//                                             at most 12 asks a minute a player key, 120 an address: 429
//                                             {retry_ms}; every room full: 503 {waiting, last_wait_ms, retry_ms} (#12796)
//   POST /api/playtest/room-invite/<room>     INVITE: a seat in your room held two minutes for a friend
//                                             (rooms_invite, migrations/0125_room_invites.sql, #12795)
//   POST /api/playtest/room-items            the item ledger's one mutation, signed with the room key
//                                             (room_items below, migrations/0130_item_ledger.sql, #12849)
//   GET  /api/playtest/party, POST /api/playtest/party/invite|join|leave|hide   durable parties, the
//                                             roster health only (party below, migrations/0129_parties.sql, #12852)
//   POST /api/playtest/player/name            {name}: a guest's own name ({"name": ""} the generated one)
//
// Who asks: the X-Verse-Player key; a NIP-98 signed request (Authorization:
// Nostr) adds that pubkey's account, and an admin's signature (OA_ADMIN_PUBKEYS)
// is the site, which owns the built-in worlds; so is an admin logged in on the
// site (a session of ours, site_session below). A bad signature, or one seen before,
// is refused. The automation mark is the analytics one.
#include "api.h"
#include "nostr.h"
#include "progress-policy.generated.h"
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

// An admin logged in on the site (/login, admin.c admin_who) is
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

// A progress save (#12719): the slot's facts under its lock (verse.progress_facts), the decision
// progress-policy.coax makes on them, then SQL keeps the save or refuses it, in one transaction.
static bool progress_run(PGconn *db, const char *sql) {
  PGresult *r = PQexec(db, sql);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  if (!ok) api_log("progress: %s", PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static void progress_save(const request *q, PGconn *db, const char *parts, const char *body, const char *who) {
  if (!progress_run(db, "BEGIN")) { api_json(q->fd, 500, "{\"error\":\"not kept\"}"); return; }
  const char *fv[3] = {parts, body, who};
  PGresult *f = PQexecParams(db, "SELECT (f->>'status')::int, coalesce(f->'body', 'null')::text, f->>'acct', f->>'slot', "
                                 "coalesce(f->>'rev', '0'), coalesce(f->>'marked', 'false'), coalesce(f->>'sent_rev', '-1') "
                                 "FROM verse.progress_facts($1::text[], $2::jsonb, $3::jsonb) f", 3, NULL, fv, NULL, NULL, 0);
  if (PQresultStatus(f) != PGRES_TUPLES_OK || PQntuples(f) != 1) {
    const char *state = PQresultErrorField(f, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("progress: %s", PQerrorMessage(db));
    PQclear(f); progress_run(db, "ROLLBACK");
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  int status = atoi(PQgetvalue(f, 0, 0));
  if (status != 0) {
    char *out = strdup(PQgetvalue(f, 0, 1));
    PQclear(f); progress_run(db, "ROLLBACK");
    api_json(q->fd, status, out ? out : "{}"); free(out);
    return;
  }
  int64_t rev = atoll(PQgetvalue(f, 0, 4)), sent = atoll(PQgetvalue(f, 0, 6));
  bool marked = !strcmp(PQgetvalue(f, 0, 5), "true");
  int64_t code = pp_save(rev, marked, sent);
  char next[24], marks[8], st[8], words[200];
  snprintf(next, sizeof next, "%lld", (long long)pp_next(rev));
  snprintf(marks, sizeof marks, "%s", pp_marks(sent, marked) ? "true" : "false");
  snprintf(st, sizeof st, "%lld", (long long)pp_status(code));
  coax_string why = pp_text(code);
  snprintf(words, sizeof words, "%.*s", (int)why.count, why.data);
  PGresult *r;
  if (code == 0) {   /* PP_KEEP */
    const char *kv[5] = {PQgetvalue(f, 0, 2), PQgetvalue(f, 0, 3), who, next, marks};
    r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.progress_keep($1, $2, $3::jsonb->>'bytes', $4::bigint, $5::boolean) x",
                     5, NULL, kv, NULL, NULL, 0);
  } else {
    const char *rv[4] = {PQgetvalue(f, 0, 2), PQgetvalue(f, 0, 3), st, words};
    r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.progress_refuse($1, $2, $3::int, $4) x",
                     4, NULL, rv, NULL, NULL, 0);
  }
  PQclear(f);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || !progress_run(db, "COMMIT")) {
    if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("progress: %s", PQerrorMessage(db));
    PQclear(r); progress_run(db, "ROLLBACK");
    api_json(q->fd, 500, "{\"error\":\"not kept\"}");
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
//   v2.PID.ACCOUNT.NAME.ROOM.EXPIRES.HP.MAC
// PID the player's public id, ACCOUNT 1 when the key has an account (0 a guest), NAME the display name
// as base64url without padding ("-" none), ROOM the room's code, EXPIRES unix ms, HP the saved hit points
// ("-" none, #12715), MAC the lowercase hex
// of HMAC-SHA256 over everything before ".MAC", keyed with OA_ROOM_TICKET_KEY's bytes (Secret Manager
// oa-room-ticket-key, which oa-room holds too). The name is verse.player_name's, the one chat, presence
// and the inbox show too (migrations/0116_one_player_name.sql), kept to letters, digits, spaces, '-'
// and '_', 24 at most.
int room_write_signed(const request *q, const char *purpose, const char *body, size_t n);   /* rooms.c (#13260) */
#define TICKET_MS 600000
/* seat ticket v5's one writer (ticket-sign-unit.c, aethyr/session/seat-ticket.coax; #13260) */
bool stk_secret_c(const char *text, unsigned char out[32]);
size_t stk_write_c(const unsigned char secret[32], const char *pid, int acct, const char *name, const char *room, int shard,
                   long long expires, int hp, int xp, int wanted, long long owner, const char *ext, int profile, const char *saved,
                   char *out, size_t cap);
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
// Admission (#12796, migrations/0126_room_admission.sql): the ticket asks a player key and an address
// may make in a minute, in this process. Over, the answer is 429 with the ms until the window ends
// (and Retry-After); the 429s are counted as 'rate' with the next write (room_rate_n).
#define ROOM_ASKS_KEY 12
#define ROOM_ASKS_ADDR 120
#define ROOM_LIMITS 512
static struct { char key[96]; double since; int n; } room_limits[ROOM_LIMITS];
static long long room_rate_n;
// 0 when allowed, else the ms until who's window ends.
static long long room_admit(const char *kind, const char *who, int per_minute) {
  char key[96]; snprintf(key, sizeof key, "%s:%.80s", kind, who);
  double now = api_now_ms();
  int free_slot = -1, oldest = 0;
  for (int i = 0; i < ROOM_LIMITS; i++) {
    if (room_limits[i].key[0] && !strcmp(room_limits[i].key, key)) {
      if (now - room_limits[i].since > 60000) { room_limits[i].since = now; room_limits[i].n = 0; }
      if (++room_limits[i].n <= per_minute) return 0;
      long long left = (long long)(room_limits[i].since + 60000 - now);
      return left > 0 ? left : 1;
    }
    if (!room_limits[i].key[0] || now - room_limits[i].since > 60000) { if (free_slot < 0) free_slot = i; }
    else if (room_limits[i].since < room_limits[oldest].since) oldest = i;
  }
  int k = free_slot >= 0 ? free_slot : oldest;
  snprintf(room_limits[k].key, sizeof room_limits[k].key, "%s", key);
  room_limits[k].since = now; room_limits[k].n = 1;
  return 0;
}
// The visitor's address: the load balancer's next to last (as access.c's).
static void room_visitor(const request *q, char ip[128]) {
  char xff[512] = "";
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  snprintf(ip, 128, "%s", np ? parts[np >= 2 ? np - 2 : 0] : "local");
}
// A refusal counted by reason (and this process's 429s since the last write).
static void room_count(PGconn *db, const char *reason) {
  char n[24]; snprintf(n, sizeof n, "%lld", room_rate_n);
  const char *v[2] = {reason, n};
  PGresult *r = PQexecParams(db, "SELECT verse.room_admit_count($1, 1), verse.room_admit_count('rate', $2::bigint)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK) room_rate_n = 0;
  PQclear(r);
}
// SDK transaction adapter: choose the checked profile, lock identity/owner,
// read the complete accepted capture, and issue its successor before COMMIT.
static int ticket_resume(const request *q, PGconn *db, const char *pid, int seat, char saved[2049], int *xp, long long *owner) {
  char seat_s[12]; snprintf(seat_s,sizeof seat_s,"%d",seat);
  const char *pv[1]={pid}, *sv[2]={pid,seat_s}; PGresult *r=NULL; char who[96], asked[8];
  const char *enabled=getenv("OA_ROOM_CHECKPOINTS");
  bool wants=api_query(q,"checkpoint",asked,sizeof asked)&&!strcmp(asked,"2");
  // rest=1 (character-checkpoint-v3): a profile 3 capture as it is; otherwise its profile 2 part
  // (without rest and the optional uses)
  char rest_q[8]; const char *three=api_query(q,"rest",rest_q,sizeof rest_q)&&!strcmp(rest_q,"1")?"t":"f";
  if (!progress_run(db,"BEGIN")) return -1;
  r=PQexecParams(db,"SELECT coalesce(account,id) FROM verse.players WHERE id=$1 FOR SHARE",1,NULL,pv,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK||PQntuples(r)!=1) goto fail;
  snprintf(who,sizeof who,"%s",PQgetvalue(r,0,0));PQclear(r);r=NULL;
  const char *wv[1]={who};
  r=PQexecParams(db,"SELECT gen FROM verse.room_owners WHERE who=$1 FOR UPDATE",1,NULL,wv,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK) goto fail;PQclear(r);r=NULL;
  r=PQexecParams(db,"SELECT EXISTS(SELECT 1 FROM verse.character_profiles_v2 WHERE who=$1)",1,NULL,wv,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK||PQntuples(r)!=1) goto fail;
  int profile=(int)checkpoint2_profile(wants,enabled&&!strcmp(enabled,"1"),PQgetvalue(r,0,0)[0]=='t');PQclear(r);r=NULL;
  if (profile==2) goto fail;
  if (profile==1) {
    if (!progress_run(db,"SET LOCAL synchronous_commit=on")) goto fail;
    r=PQexecParams(db,"SELECT verse.room_owner_next($1,$2::int),current_setting('fsync')='on'",2,NULL,sv,NULL,NULL,0);   /* the generation and its shard (0156) */
    if (PQresultStatus(r)!=PGRES_TUPLES_OK||PQntuples(r)!=1||PQgetisnull(r,0,0)||PQgetvalue(r,0,1)[0]!='t') goto fail;
    *owner=atoll(PQgetvalue(r,0,0));PQclear(r);r=NULL;
    r=PQexecParams(db,"INSERT INTO verse.character_profiles_v2(who) VALUES($1) ON CONFLICT DO NOTHING",1,NULL,wv,NULL,NULL,0);
    if(PQresultStatus(r)!=PGRES_COMMAND_OK)goto fail;PQclear(r);r=NULL;
    const char *rv[2]={who,three};
    r=PQexecParams(db,"SELECT coalesce((SELECT (CASE WHEN $2::boolean OR body->>'v'<>'3' THEN body ELSE (body-'rest'-'uses')||'{\"v\":2}'::jsonb END)::text FROM verse.character_checkpoints_v2 WHERE who=$1),'-'),greatest(coalesce((SELECT xp FROM verse.room_vitals WHERE who=$1),0),coalesce((SELECT (body->>'xp')::bigint FROM verse.character_checkpoints_v1 WHERE who=$1),0),coalesce((SELECT (body->>'xp')::bigint FROM verse.character_checkpoints_v2 WHERE who=$1),0))",2,NULL,rv,NULL,NULL,0);
    if (PQresultStatus(r)!=PGRES_TUPLES_OK||PQntuples(r)!=1||strlen(PQgetvalue(r,0,0))>2048) goto fail;
    snprintf(saved,2049,"%s",PQgetvalue(r,0,0));*xp=atoi(PQgetvalue(r,0,1));PQclear(r);r=NULL;
  }
  if (!progress_run(db,"COMMIT")) goto fail;
  return profile;
fail:
  if(r)PQclear(r);progress_run(db,"ROLLBACK");return -1;
}
static void room_ticket(const request *q, PGconn *db, const char *key, bool site, const char *room, const char *body) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a ticket\"}"); return; }
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  /* v5 (#13260, aethyr/session/seat-ticket.coax; docs/deployment/room-keys.md): set, the API signs with this
     private key (64 hex) and rooms check with its public key only. Unset, v2/v3/v4 under OA_ROOM_TICKET_KEY. */
  const char *signing = getenv("OA_ROOM_TICKET_SIGNING_KEY");
  unsigned char v5_key[32]; bool v5 = signing && signing[0];
  if (v5 ? !stk_secret_c(signing, v5_key) : (!secret || strlen(secret) < 32)) { api_json(q->fd, 503, "{\"error\":\"no ticket key\"}"); return; }
  {
    char ip[128]; room_visitor(q, ip);
    long long wait = room_admit("room-key", key && key[0] ? key : "-", ROOM_ASKS_KEY);
    long long wait_ip = room_admit("room-ip", ip, ROOM_ASKS_ADDR);
    if (wait_ip > wait) wait = wait_ip;
    if (wait > 0) {
      room_rate_n++;
      char hdr[48], out[96];
      snprintf(hdr, sizeof hdr, "Retry-After: %lld\r\n", (wait + 999) / 1000);
      int on = snprintf(out, sizeof out, "{\"error\":\"slow down\",\"retry_ms\":%lld}", wait);
      api_respond(q->fd, 429, "application/json", hdr, out, (size_t)on);
      return;
    }
  }
  size_t rn = strlen(room);
  if (!rn || rn > 24) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  for (size_t i = 0; i < rn; i++) {
    char ch = room[i];
    if (!((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '-')) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  }
  if (!key || !key[0]) { api_json(q->fd, 401, "{\"error\":\"a player key\"}"); return; }
  const char *v[1] = {key};
  // the saved state (verse.room_vitals, #12715): HP for 10 minutes, WANTED for 5 (only a room's: 0124,
  // #12794), the look and class always
  PGresult *r = PQexecParams(db, "SELECT p.id, p.account IS NOT NULL, verse.player_name(p.id), "
                                 "CASE WHEN rv.hp_at > extract(epoch FROM clock_timestamp()) * 1000 - 600000 THEN rv.hp END, "
                                 "CASE WHEN rv.wanted_by IS NOT NULL AND rv.wanted_at > extract(epoch FROM clock_timestamp()) * 1000 - 300000 THEN rv.wanted END, rv.look, rv.class, "
                                 "EXISTS (SELECT 1 FROM verse.room_kicks k WHERE k.player = p.id AND k.until_ms > extract(epoch FROM clock_timestamp()) * 1000), "
                                 "(SELECT string_agg(b.blocked, ',' ORDER BY b.at_ms DESC) FROM (SELECT * FROM verse.player_blocks WHERE player = p.id ORDER BY at_ms DESC LIMIT 32) b), "
                                 "coalesce(rv.xp, 0) "
                                 "FROM verse.players p LEFT JOIN verse.accounts a ON a.id = p.account "
                                 "LEFT JOIN verse.room_vitals rv ON rv.who = coalesce(p.account, p.id) "
                                 "WHERE p.key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0);
  (void)body;
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
  if (PQgetvalue(r, 0, 7)[0] == 't') { PQclear(r); room_count(db, "kicked"); api_json(q->fd, 403, "{\"error\":\"kicked\"}"); return; }   /* an admin's kick, 24 hours (#12714) */
  int hp = PQgetisnull(r, 0, 3) ? -1 : atoi(PQgetvalue(r, 0, 3));
  int wanted = PQgetisnull(r, 0, 4) ? -1 : atoi(PQgetvalue(r, 0, 4));
  int xp = atoi(PQgetvalue(r, 0, 9));   /* XP, which only rises (#12739); the ticket's HP field carries it as HPxXP */
  if (xp < 0 || xp > 9999999) xp = 0;
  char look[40] = "", klass[24] = "";
  if (!PQgetisnull(r, 0, 5)) ticket_word(look, sizeof look, PQgetvalue(r, 0, 5));
  if (!PQgetisnull(r, 0, 6)) ticket_word(klass, sizeof klass, PQgetvalue(r, 0, 6));
  // who this player blocks (verse.player_blocks, #12714), 32 at most: the page hides their bodies
  char blocked[32 * 24 + 8] = "";
  if (!PQgetisnull(r, 0, 8)) {
    size_t bn = 0;
    for (const char *c = PQgetvalue(r, 0, 8); *c && bn + 1 < sizeof blocked; c++)
      if ((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-' || *c == ',') blocked[bn++] = *c;
    blocked[bn] = 0;
  }
  PQclear(r);
  for (char *c = pid; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-')) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  if (nn) b64url(name64, sizeof name64, (const unsigned char *)name, nn); else strcpy(name64, "-");
  // ?realm=REALM (#12717): the directory (rooms.c) gives the instance and the shard that holds it,
  // and the ticket names that shard (v3), which seats it and no other. Without it, a v2 ticket for the
  // room asked, which shard 1 seats (a page from before the directory).
  // ?with=PLAYER (#12795): the friend whose invite brought them, whose held seat they take
  // ?build=ENGINE.RULES.CONTENT (#12839): the page's build, placed on a shard that runs it
  // ?party=MEMBER (#12911, migrations/0134_party_follow.sql): a member of your party, whose room you join
  char realm[32] = "", placed[32] = "", asked[32] = "", via[48] = "", build[40] = "", mate[48] = "";
  char party_room[32] = ""; bool party_full = false, party_away = false;
  int shard = 0; bool full = false;
  snprintf(asked, sizeof asked, "%s", room);
  if (!api_query(q, "with", via, sizeof via)) via[0] = 0;
  if (!api_query(q, "build", build, sizeof build)) build[0] = 0;
  if (!api_query(q, "party", mate, sizeof mate)) mate[0] = 0;
  for (char *c = mate; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-')) { mate[0] = 0; break; }
  if (api_query(q, "realm", realm, sizeof realm) && realm[0]) {
    char rn_s[24]; snprintf(rn_s, sizeof rn_s, "%lld", room_rate_n);
    const char *wv[2] = {pid, rn_s};
    // a page that asks for the realm, not a room, goes where its party sits while that room has a seat
    // (#12852, migrations/0129_parties.sql verse.party_room); every such room full: "party_full" says which
    char seats_s[8]; snprintf(seats_s, sizeof seats_s, "%d", ROOM_SEATS);
    const char *gv[3] = {realm, pid, seats_s};
    const char *goes = room;
    const char *mv[4] = {realm, pid, mate, seats_s};
    if (!via[0] && !strcmp(room, realm)) {
      // the member you asked to join (?party=), else where your party sits
      PGresult *gr = mate[0] ? PQexecParams(db, "SELECT code, is_full FROM verse.party_member_room($1, $2, $3, $4::int)", 4, NULL, mv, NULL, NULL, 0)
                             : PQexecParams(db, "SELECT code, is_full FROM verse.party_room($1, $2, $3::int)", 3, NULL, gv, NULL, NULL, 0);
      if (PQresultStatus(gr) == PGRES_TUPLES_OK && PQntuples(gr) == 1 && !PQgetisnull(gr, 0, 0) && strlen(PQgetvalue(gr, 0, 0)) < sizeof party_room) {
        snprintf(party_room, sizeof party_room, "%s", PQgetvalue(gr, 0, 0));
        party_full = PQgetvalue(gr, 0, 1)[0] == 't';
        if (!party_full) goes = party_room;
      } else if (PQresultStatus(gr) != PGRES_TUPLES_OK) api_log("party-room: %s", PQerrorMessage(db));
      else if (mate[0]) party_away = true;
      PQclear(gr);
    }
    if (!rooms_pick(db, realm, goes, pid, via, build, placed, sizeof placed, &shard, &full)) {
      // every room full: this player waits; the answer says how many wait, the last wait, and when to ask again
      PGresult *w = PQexecParams(db, "SELECT waiting, last_wait_ms FROM verse.room_wait($1, $2::bigint)", 2, NULL, wv, NULL, NULL, 0);
      long long waiting = 0, last = -1;
      if (PQresultStatus(w) == PGRES_TUPLES_OK && PQntuples(w) == 1) {
        room_rate_n = 0;
        waiting = atoll(PQgetvalue(w, 0, 0));
        if (!PQgetisnull(w, 0, 1)) last = atoll(PQgetvalue(w, 0, 1));
      } else api_log("room-wait: %s", PQerrorMessage(db));
      PQclear(w);
      /* more waiting, longer apart: W waiting pages ask W / (5 + 0.2 W) times a second, under 5, up to 1475 waiting */
      long long retry = 5000 + (waiting * 200 < 295000 ? waiting * 200 : 295000);
      char out[192], hdr[48];
      int on = snprintf(out, sizeof out, "{\"error\":\"every room is full: try again in a minute\",\"waiting\":%lld,\"retry_ms\":%lld%s", waiting, retry, last >= 0 ? ",\"last_wait_ms\":" : "");
      if (last >= 0) on += snprintf(out + on, sizeof out - (size_t)on, "%lld", last);
      on += snprintf(out + on, sizeof out - (size_t)on, "}");
      snprintf(hdr, sizeof hdr, "Retry-After: %lld\r\n", (retry + 999) / 1000);
      api_respond(q->fd, 503, "application/json", hdr, out, (size_t)on);
      return;
    }
    PGresult *sr = PQexecParams(db, "SELECT verse.room_seated($1, $2::bigint)", 2, NULL, wv, NULL, NULL, 0);
    if (PQresultStatus(sr) == PGRES_TUPLES_OK) room_rate_n = 0;
    PQclear(sr);
    room = placed;
  }
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long expires = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000 + TICKET_MS;
  char saved[2049]="-"; long long checkpoint_owner=0;
  int seat_shard = shard > 0 ? shard : 1;   /* the shard this ticket seats (a v2 ticket: shard 1); room_owners notes it (0156) */
  int resume=ticket_resume(q,db,pid,seat_shard,saved,&xp,&checkpoint_owner);
  if (resume<0) {api_json(q->fd,503,"{\"error\":\"character recovery unavailable\"}");return;}
  if (resume==1) {
    coax_string cls=checkpoint2_class((coax_string){saved,(int64_t)strlen(saved)});
    if(cls.count)snprintf(klass,sizeof klass,"%.*s",(int)cls.count,cls.data);
  }
  char text[256];
  char hps[32]; if (hp >= 0 && hp < 100000) snprintf(hps, sizeof hps, "%d", hp); else strcpy(hps, "-");
  if (xp > 0) snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "x%d", xp);
  /* the WANTED level the room saved, "w1" to "w5" (#12794; room-ticket.coax): the room seats it wanted */
  if (wanted >= 1 && wanted <= 5) { if (xp <= 0) snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "x0"); snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "w%d", wanted); }
  /* the character's next owner generation (#12834, migrations/0127): "g" and it, last in the HP field, once
     every shard reads it (OA_ROOM_OWNER_GEN=1; a room from before refuses a ticket with it). The room
     sends it with its vitals lines, and a line below the character's newest generation is not kept. */
  const char *og = getenv("OA_ROOM_OWNER_GEN");
  long long owner_gen = 0;   /* the same generation, for the v5 writer */
  if (resume==1) { snprintf(hps+strlen(hps),sizeof hps-strlen(hps),"g%lld",checkpoint_owner); owner_gen = checkpoint_owner; }
  else if (og && !strcmp(og, "1")) {
    char ss[12]; snprintf(ss, sizeof ss, "%d", seat_shard);
    const char *gv[2] = {pid, ss};
    PGresult *gr = PQexecParams(db, "SELECT verse.room_owner_next($1, $2::int)", 2, NULL, gv, NULL, NULL, 0);   /* and its shard (0156) */
    if (PQresultStatus(gr) == PGRES_TUPLES_OK && PQntuples(gr) == 1 && !PQgetisnull(gr, 0, 0)) {
      snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "g%s", PQgetvalue(gr, 0, 0));
      owner_gen = atoll(PQgetvalue(gr, 0, 0));
    } else if (PQresultStatus(gr) != PGRES_TUPLES_OK) api_log("room-owner: %s", PQerrorMessage(db));
    PQclear(gr);
  }
  /* ACCT 2: an account who is one of Everglade's admins, who may play its demo character (#12739) */
  int acct = account ? 1 : 0;
  if (account) {
    const char *av[2] = {key, site ? "t" : "f"};
    PGresult *ar = PQexecParams(db, "SELECT verse.edit_may('everglade', $1, $2::boolean)", 2, NULL, av, NULL, NULL, 0);
    if (PQresultStatus(ar) == PGRES_TUPLES_OK && PQntuples(ar) == 1 && PQgetvalue(ar, 0, 0)[0] == 't') acct = 2;
    PQclear(ar);
  }
  char resumed[4097]; const char *ticket=text;
  if (v5) {
    /* the one v5 writer (ticket-sign-unit.c): shard 0 (no directory) is shard 1's, as a v2 ticket is */
    if (!stk_write_c(v5_key, pid, acct, name, room, seat_shard, expires, hp, xp, wanted, owner_gen, "-", resume == 1 ? 2 : 1, saved, resumed, sizeof resumed)) {
      api_json(q->fd, resume == 1 ? 503 : 500, resume == 1 ? "{\"error\":\"character recovery unavailable\"}" : "{\"error\":\"no ticket\"}"); return;
    }
    ticket = resumed;
  } else {
  int tn = shard > 0 ? snprintf(text, sizeof text, "v3.%s.%d.%s.%s.%d.%lld.%s", pid, acct, name64, room, shard, expires, hps)
                     : snprintf(text, sizeof text, "v2.%s.%d.%s.%s.%lld.%s", pid, acct, name64, room, expires, hps);
  if (tn <= 0 || (size_t)tn + 66 >= sizeof text) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)text, (size_t)tn, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no ticket\"}"); return; }
  text[tn++] = '.';
  for (unsigned i = 0; i < 32; i++) tn += snprintf(text + tn, sizeof text - (size_t)tn, "%02x", mac[i]);
  if (resume==1) {
    coax_string prefix=resume_ticket_prefix((coax_string){text,tn},(coax_string){saved,(int64_t)strlen(saved)},(coax_v_u8){(uint8_t *)resumed,4031});
    if (!prefix.count || !HMAC(EVP_sha256(),secret,(int)strlen(secret),(const unsigned char *)resumed,(size_t)prefix.count,mac,&ml) || ml!=32) {api_json(q->fd,503,"{\"error\":\"character recovery unavailable\"}");return;}
    int pos=(int)prefix.count;resumed[pos++]='.';for(unsigned i=0;i<32;i++)pos+=snprintf(resumed+pos,sizeof resumed-(size_t)pos,"%02x",mac[i]);ticket=resumed;
  }
  }
  char out[6144], en[48], st[960] = "";
  esc(en, sizeof en, name);
  int sn = 0;
  if (hp >= 0) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"hp\":%d", hp);
  if (wanted >= 0) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"wanted\":%d", wanted);
  if (xp > 0) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"xp\":%d", xp);
  if (look[0]) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"look\":\"%s\"", look);
  if (klass[0]) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"class\":\"%s\"", klass);
  if (shard > 0) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"room\":\"%s\",\"shard\":%d%s", room, shard, full ? ",\"full\":true" : "");
  /* the shard's own endpoint when it names one (a room VM, #13179): the page opens it in place of the
     load balancer's /api/room/sK/ */
  { char ep[200]; if (shard > 0 && rooms_endpoint(db, shard, ep, sizeof ep) && sn + (int)strlen(ep) + 20 < (int)sizeof st) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"endpoint\":\"%s\"", ep); }
  if (shard > 0 && party_full) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"party_full\":\"%s\"", party_room);
  if (shard > 0 && party_away) sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"party_away\":true");
  /* the room asked was full: which, and whose it was when an invite named them (the page says so, #12795) */
  if (shard > 0 && full) {
    char by[32] = "", eb[48] = "";
    if (via[0]) {
      const char *bv[1] = {via};
      PGresult *br = PQexecParams(db, "SELECT verse.player_name($1)", 1, NULL, bv, NULL, NULL, 0);
      if (PQresultStatus(br) == PGRES_TUPLES_OK && PQntuples(br) == 1 && !PQgetisnull(br, 0, 0)) ticket_name(by, sizeof by, PQgetvalue(br, 0, 0));
      PQclear(br);
    }
    esc(eb, sizeof eb, by);
    sn += snprintf(st + sn, sizeof st - (size_t)sn, ",\"asked\":\"%s\"%s%s%s", asked, by[0] ? ",\"by\":\"" : "", eb, by[0] ? "\"" : "");
  }
  if (blocked[0]) snprintf(st + sn, sizeof st - (size_t)sn, ",\"blocked\":\"%s\"", blocked);
  snprintf(out, sizeof out, "{\"ticket\":\"%s\",\"id\":\"%s\",\"account\":%s,\"admin\":%s,\"name\":\"%s\",\"expires\":%lld%s}", ticket, pid, account ? "true" : "false", acct == 2 ? "true" : "false", en, expires, st);
  api_json(q->fd, 200, out);
}

// INVITE (#12795): one seat in the player's room held for their friends ROOMS_INVITE_MS (two minutes,
// migrations/0125_room_invites.sql). {"room":R,"until":MS}; 403 not in it, 409 full.
static void room_invite(const request *q, PGconn *db, const char *key, const char *room) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST an invite\"}"); return; }
  if (!key || !key[0]) { api_json(q->fd, 401, "{\"error\":\"a player key\"}"); return; }
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, "SELECT id FROM verse.players WHERE key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); api_json(q->fd, 403, "{\"error\":\"unknown player key\"}"); return; }
  char pid[64]; snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  long long until = 0;
  int got = rooms_invite(db, room, pid, &until);
  if (got == 1) { api_json(q->fd, 403, "{\"error\":\"not in it\"}"); return; }
  if (got == 2) { api_json(q->fd, 409, "{\"error\":\"full\"}"); return; }
  if (got < 0) { api_json(q->fd, 500, "{\"error\":\"no invite\"}"); return; }
  char out[96]; snprintf(out, sizeof out, "{\"room\":\"%s\",\"until\":%lld}", room, until);
  api_json(q->fd, 200, out);
}

// Durable parties (#12852, migrations/0129_parties.sql): invite, join, leave, hide, and the roster,
// health only. sub is "" (GET the roster) or invite, join, leave, hide (POST).
static void party(const request *q, PGconn *db, const char *key, const char *sub, const char *body) {
  if (!key || !key[0]) { api_json(q->fd, 401, "{\"error\":\"a player key\"}"); return; }
  bool get = !strcmp(q->method, "GET");
  if (sub[0] ? get : !get) { api_json(q->fd, 405, sub[0] ? "{\"error\":\"POST it\"}" : "{\"error\":\"GET the party\"}"); return; }
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, "SELECT id FROM verse.players WHERE key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); api_json(q->fd, 403, "{\"error\":\"unknown player key\"}"); return; }
  char pid[64]; snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  const char *pv[2] = {pid, body};
  const char *sql =
    !sub[0] ? "SELECT 200, verse.party_view($1)" :
    !strcmp(sub, "invite") ? "SELECT 200, format('{\"party\":%s,\"code\":%s,\"until\":%s}', x.party, to_json(x.code), x.until_ms) FROM verse.party_invite($1) x" :
    !strcmp(sub, "join") ? "SELECT CASE x.why WHEN 'gone' THEN 404 WHEN 'blocked' THEN 403 WHEN 'full' THEN 409 ELSE 200 END, "
                           "CASE WHEN x.why IS NULL THEN format('{\"party\":%s,\"version\":%s}', x.party, x.version) ELSE format('{\"error\":%s}', to_json(x.why)) END "
                           "FROM verse.party_join($1, coalesce($2::jsonb->>'code', '')) x" :
    !strcmp(sub, "leave") ? "SELECT 200, '{\"party\":null}' FROM verse.party_leave($1)" :
    !strcmp(sub, "hide") ? "SELECT CASE WHEN h THEN 200 ELSE 404 END, CASE WHEN h THEN verse.party_view($1) ELSE '{\"error\":\"no party\"}' END "
                           "FROM verse.party_hide($1, coalesce(($2::jsonb->>'on')::boolean, true)) h" : NULL;
  if (!sql) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  r = PQexecParams(db, sql, strstr(sql, "$2") ? 2 : 1, NULL, pv, NULL, NULL, 0);   /* (an unused $2 has no type) */
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("party: %s", PQerrorMessage(db));
    PQclear(r); api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"no party\"}"); return;
  }
  api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
}

// The page's part of what a player brings back into a room (#12715): {"look":"...","class":"..."},
// either, kept by the account when logged in, else the player. WANTED is not the page's (#12794): the
// room reports it signed (room_vitals). A body that names "wanted" keeps the rest and says
// "refused":"wanted"; one with only "wanted" is a 403.
static void room_state(const request *q, PGconn *db, const char *key, const char *body) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST the state\"}"); return; }
  if (!key || !key[0]) { api_json(q->fd, 401, "{\"error\":\"a player key\"}"); return; }
  const char *v[2] = {key, body};
  PGresult *r = PQexecParams(db,
    "WITH b AS (SELECT $2::jsonb AS j), p AS (SELECT coalesce(account, id) AS who FROM verse.players "
    "  WHERE key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')), "
    "s AS (SELECT CASE WHEN j->>'look' ~ '^[a-z0-9.]{1,32}$' THEN j->>'look' END AS look, "
    "  CASE WHEN j->>'class' ~ '^[a-z]{1,16}$' THEN j->>'class' END AS klass, j ? 'wanted' AS asked_wanted, "
    "  (extract(epoch FROM clock_timestamp()) * 1000)::bigint AS now FROM b), "
    "ins AS (INSERT INTO verse.room_vitals (who, look, class, at) "
    "  SELECT p.who, s.look, s.klass, s.now FROM p, s WHERE s.look IS NOT NULL OR s.klass IS NOT NULL "
    "  ON CONFLICT (who) DO UPDATE SET look = coalesce(EXCLUDED.look, verse.room_vitals.look), "
    "  class = coalesce(EXCLUDED.class, verse.room_vitals.class), at = EXCLUDED.at RETURNING who) "
    "SELECT EXISTS (SELECT 1 FROM p), (SELECT count(*) FROM ins), (SELECT asked_wanted FROM s)",
    2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-state: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  bool known = PQgetvalue(r, 0, 0)[0] == 't', wanted = PQgetvalue(r, 0, 2)[0] == 't';
  int kept = atoi(PQgetvalue(r, 0, 1));
  PQclear(r);
  if (!known) { api_json(q->fd, 403, "{\"error\":\"unknown player key\"}"); return; }
  if (!kept && wanted) { api_json(q->fd, 403, "{\"error\":\"WANTED is the room server's\",\"refused\":\"wanted\"}"); return; }
  api_json(q->fd, 200, wanted ? "{\"kept\":true,\"refused\":\"wanted\"}" : "{\"kept\":true}");
}
// A vitals post with operation IDs (#12793; room-vitals.coax rm_vitals_due, migrations/0124): each line
// "PID HP MAX XP WANTED REV AT" applied once by (SHARD, GEN, PID, REV) in verse.room_vital_apply, its
// own statement, so a line kept stays kept whatever happens to the next. The answer, text/plain:
//   receipt GEN BATCH CODES      one letter a line, in order: a d s u (0124), f an older owner's (0127), x a line it couldn't read
// A database failure is a 500 with no receipt: the room sends the post again, and the lines already
// applied answer d. AT is when the room changed the values; it orders HP and WANTED, and is taken
// within the outbox's 15 minutes before the post's time.
static void room_vitals_receipt(const request *q, PGconn *db, const char *o, int shard, long long gen, long long batch, long long at) {
  char codes[128]; size_t cn = 0;
  if (shard < 1 || shard > 99 || gen <= 0) { api_json(q->fd, 400, "{\"error\":\"bad owner\"}"); return; }
  char *line = strchr(o + 1, '\n');
  while (line && *++line && cn + 1 < sizeof codes) {
    char *next = strchr(line, '\n');
    char one[160]; size_t ol = next ? (size_t)(next - line) : strlen(line);
    if (ol >= sizeof one) ol = sizeof one - 1;
    memcpy(one, line, ol); one[ol] = 0;
    char pid[48]; int hp, max, xp, wanted; long long rev, when, owner = -1;
    char code = 'x';
    /* an 8th field: the seat ticket's owner generation (#12834, migrations/0127) */
    int got = sscanf(one, "%47s %d %d %d %d %lld %lld %lld", pid, &hp, &max, &xp, &wanted, &rev, &when, &owner);
    if ((got == 7 || (got == 8 && owner > 0)) && hp >= 0 && max > 0 && hp <= max && max < 100000 &&
        xp >= 0 && xp <= 9999999 && wanted >= 0 && wanted <= 5 && rev > 0 && when <= at && when >= at - 900000) {
      char ss[8], gs[24], hs[16], ms[16], xs[16], ws[8], rs[24], as[24], os[24];
      snprintf(ss, sizeof ss, "%d", shard); snprintf(gs, sizeof gs, "%lld", gen); snprintf(hs, sizeof hs, "%d", hp); snprintf(ms, sizeof ms, "%d", max);
      snprintf(xs, sizeof xs, "%d", xp); snprintf(ws, sizeof ws, "%d", wanted); snprintf(rs, sizeof rs, "%lld", rev); snprintf(as, sizeof as, "%lld", when);
      snprintf(os, sizeof os, "%lld", owner);
      const char *v[10] = {ss, gs, pid, hs, ms, xs, ws, rs, as, got == 8 ? os : NULL};
      PGresult *r = PQexecParams(db, "SELECT verse.room_vital_apply($1::int, $2::bigint, $3, $4::int, $5::int, $6::int, $7::int, $8::bigint, $9::bigint, $10::bigint)", 10, NULL, v, NULL, NULL, 0);
      if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
        api_log("room-vitals: %s", PQerrorMessage(db));
        PQclear(r);
        api_json(q->fd, 500, "{\"error\":\"not kept\"}");
        return;
      }
      code = PQgetvalue(r, 0, 0)[0];
      PQclear(r);
    }
    codes[cn++] = code;
    line = next;
  }
  codes[cn] = 0;
  char out[192];
  int n = snprintf(out, sizeof out, "receipt %lld %lld %s\n", gen, batch, codes);
  api_respond(q->fd, 200, "text/plain", NULL, out, (size_t)n);
}
// oa-room's part (#12715): "t MS\nPID HP MAX XP\n..." (XP since #12739; a line without it keeps the XP) signed with the seat-ticket key (X-Room-Mac: the
// hex HMAC-SHA256 of the body), within two minutes of now. Each seat's HP is kept by who the
// player is, unless a newer one is already kept.
// The inventory and reward ledger (#12849, migrations/0130_item_ledger.sql): one mutation, signed with
// the seat-ticket key as vitals are, as JSON {id, op, pid, gen, ver, item, to, kind, sent}; the answer
// is verse.item_apply's: {"status":..., "item":..., "replay":...}. Game items only.
// An item or checkpoint write signed with shard SIGNER's own key (#13260): refused (403, and true)
// when the player's newest owner generation is another shard's and the write's generation (the
// JSON field GEN) is not older (verse.room_owner_shard_ok, 0156). An older one is left to the fence
// (stale, fenced), as before. A shared-key write (SIGNER 0) is not bound.
static bool room_write_owner(const request *q, PGconn *db, int signer, const char *gen) {
  if (signer <= 0) return false;
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  char sh[12]; snprintf(sh, sizeof sh, "%d", signer);
  const char *v[3] = {b, gen, sh};
  PGresult *r = PQexecParams(db, "SELECT verse.room_owner_shard_ok(j->>'pid', (j->>$2)::bigint, $3::int) FROM (SELECT $1::jsonb j) s", 3, NULL, v, NULL, NULL, 0);
  free(b);
  bool ok = PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetvalue(r, 0, 0)[0] != 'f';   /* not JSON: the write's own check answers */
  PQclear(r);
  if (!ok) api_json(q->fd, 403, "{\"error\":\"another shard's player\"}");
  return !ok;
}
static void room_items(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a mutation\"}"); return; }
  if (q->body_len == 0 || q->body_len > 4096) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  int signer = room_write_signed(q, "items", q->body, q->body_len);   /* this shard's write key, or the shared key (#13260) */
  if (signer < 0) return;
  if (room_write_owner(q, db, signer, "gen")) return;   /* the shard that holds the player's seat (0156) */
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  const char *v[1] = {b};
  PGresult *r = PQexecParams(db,
    "WITH j AS (SELECT $1::jsonb AS j) SELECT format('{\"status\":%s,\"item\":%s,\"replay\":%s}', to_json(x.status), "
    "coalesce(x.item::text, 'null'), CASE WHEN x.replay THEN 'true' ELSE 'false' END) FROM j, "
    "verse.item_apply(j->>'id', j->>'op', j->>'pid', (j->>'gen')::bigint, (j->>'ver')::int, (j->>'item')::bigint, "
    "j->>'to', (j->>'kind')::int, (j->>'sent')::bigint) x", 1, NULL, v, NULL, NULL, 0);
  free(b);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-items: %s", PQerrorMessage(db));
    PQclear(r); api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}"); return;
  }
  api_json(q->fd, 200, PQgetvalue(r, 0, 0));
  PQclear(r);
}
// character-checkpoint-v1: SDK authentication, locked database facts, then the
// Coax policy's transaction effects. A receipt follows a WAL-flushed commit.
// This receiver is additive: no room producer or ticket restore uses it yet.
static void room_checkpoint(const request *q, PGconn *db, int version) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a checkpoint\"}"); return; }
  if (q->body_len == 0 || q->body_len > 4096) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  int signer = room_write_signed(q, "checkpoint", q->body, q->body_len);   /* this shard's write key, or the shared key (#13260) */
  if (signer < 0) return;
  if (room_write_owner(q, db, signer, "owner")) return;   /* the shard that holds the player's seat (0156) */
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  int64_t now = (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  if (!(version == 2 ? checkpoint2_input((coax_string){(const char *)q->body, q->body_len}, now) : room_checkpoint_input((coax_string){(const char *)q->body, q->body_len}, now))) {
    api_json(q->fd, 400, "{\"error\":\"bad checkpoint\"}"); return;
  }
  char body[4097]; memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  const char *v[2] = {body, NULL};
  PGresult *r = NULL;
  if (!progress_run(db, "BEGIN")) goto failed;
  if (!progress_run(db, "SET LOCAL synchronous_commit = on")) goto failed;
  // Lock the identity first; account linking cannot move it halfway through.
  r = PQexecParams(db, "SELECT coalesce(account,id) FROM verse.players WHERE id = ($1::jsonb->>'pid') FOR SHARE", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) goto failed;
  bool known = PQntuples(r) == 1;
  char *who = known ? strdup(PQgetvalue(r, 0, 0)) : strdup("");
  PQclear(r); r = NULL;
  if (!who) goto failed;
  v[1] = who;
  // The same row is updated by room_owner_next. Holding it until commit closes
  // the check/write race with a newly issued ticket, including another shard.
  const char *wv[1] = {who};
  r = PQexecParams(db, "SELECT gen FROM verse.room_owners WHERE who=$1 FOR UPDATE", 1, NULL, wv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { free(who); goto failed; }
  int64_t directory = PQntuples(r) ? atoll(PQgetvalue(r, 0, 0)) : 0;
  PQclear(r);
  r = PQexecParams(db, "SELECT (j->>'owner')::bigint,(j->>'process')::bigint,(j->>'rev')::bigint,(j->>'xp')::bigint,current_setting('fsync')='on' FROM (SELECT $1::jsonb j) s", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { free(who); goto failed; }
  int64_t owner = atoll(PQgetvalue(r,0,0)), process = atoll(PQgetvalue(r,0,1)), rev = atoll(PQgetvalue(r,0,2)), xp = atoll(PQgetvalue(r,0,3));
  bool durable = PQgetvalue(r,0,4)[0] == 't';
  PQclear(r);
  r = PQexecParams(db, version == 2 ? "SELECT owner,process,rev,(body->>'xp')::bigint,body=($1::jsonb-'sent'),accepted_at FROM verse.character_checkpoints_v2 WHERE who=$2 FOR UPDATE" : "SELECT owner,process,rev,(body->>'xp')::bigint,body=($1::jsonb-'sent'),accepted_at FROM verse.character_checkpoints_v1 WHERE who=$2 FOR UPDATE", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { free(who); goto failed; }
  bool old = PQntuples(r) == 1;
  int64_t old_owner = old ? atoll(PQgetvalue(r,0,0)) : 0, old_process = old ? atoll(PQgetvalue(r,0,1)) : 0;
  int64_t old_rev = old ? atoll(PQgetvalue(r,0,2)) : 0, old_xp = old ? atoll(PQgetvalue(r,0,3)) : 0;
  bool same = old && PQgetvalue(r,0,4)[0] == 't';
  int64_t accepted = old ? atoll(PQgetvalue(r,0,5)) : 0;
  PQclear(r); r = NULL;
  if (version == 2 && old_owner != owner) {
    r = PQexecParams(db, "SELECT greatest(coalesce((SELECT xp FROM verse.room_vitals WHERE who=$1),0),coalesce((SELECT (body->>'xp')::bigint FROM verse.character_checkpoints_v1 WHERE who=$1),0),coalesce((SELECT (body->>'xp')::bigint FROM verse.character_checkpoints_v2 WHERE who=$1),0))", 1,NULL,wv,NULL,NULL,0);
    if (PQresultStatus(r)!=PGRES_TUPLES_OK || PQntuples(r)!=1) { free(who); goto failed; }
    old_xp=atoll(PQgetvalue(r,0,0)); PQclear(r); r=NULL;
  }
  int64_t code = room_checkpoint_decide(durable,known,(coax_t_OwnerGeneration){owner},(coax_t_OwnerGeneration){directory},process,rev,xp,(coax_t_OwnerGeneration){old_owner},old_process,old_rev,old_xp,same);
  if (code == 0) {
    r = PQexecParams(db, version == 2 ? "INSERT INTO verse.character_checkpoints_v2 AS c (who,owner,process,rev,body,accepted_at) SELECT $2,(j->>'owner')::bigint,(j->>'process')::bigint,(j->>'rev')::bigint,j-'sent',verse.now_ms() FROM (SELECT $1::jsonb j) s ON CONFLICT (who) DO UPDATE SET owner=EXCLUDED.owner,process=EXCLUDED.process,rev=EXCLUDED.rev,body=EXCLUDED.body,accepted_at=EXCLUDED.accepted_at RETURNING accepted_at" : "INSERT INTO verse.character_checkpoints_v1 AS c (who,owner,process,rev,body,accepted_at) SELECT $2,(j->>'owner')::bigint,(j->>'process')::bigint,(j->>'rev')::bigint,j-'sent',verse.now_ms() FROM (SELECT $1::jsonb j) s ON CONFLICT (who) DO UPDATE SET owner=EXCLUDED.owner,process=EXCLUDED.process,rev=EXCLUDED.rev,body=EXCLUDED.body,accepted_at=EXCLUDED.accepted_at RETURNING accepted_at", 2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { free(who); goto failed; }
    accepted = atoll(PQgetvalue(r,0,0)); PQclear(r); r = NULL;
    if (version==2) {
      r=PQexecParams(db,"INSERT INTO verse.room_vitals AS x(who,hp,hp_max,hp_at,xp,wanted,wanted_at,at) SELECT $2,(j->>'hp')::int,(j->>'max')::int,(j->>'at')::bigint,(j->>'xp')::int,(j->>'wanted')::int,(j->>'at')::bigint,(j->>'at')::bigint FROM (SELECT $1::jsonb j) s ON CONFLICT(who) DO UPDATE SET hp=EXCLUDED.hp,hp_max=EXCLUDED.hp_max,hp_at=EXCLUDED.hp_at,xp=greatest(x.xp,EXCLUDED.xp),wanted=EXCLUDED.wanted,wanted_at=EXCLUDED.wanted_at,at=greatest(x.at,EXCLUDED.at)",2,NULL,v,NULL,NULL,0);
      if (PQresultStatus(r)!=PGRES_COMMAND_OK) {free(who);goto failed;}PQclear(r);r=NULL;
    }
  }
  free(who);
  if (!progress_run(db, "COMMIT")) goto failed;
  coax_string status = room_checkpoint_status(code);
  char out[256];
  snprintf(out,sizeof out,"{\"v\":%d,\"status\":\"%.*s\",\"owner\":%lld,\"process\":%lld,\"rev\":%lld,\"accepted_at\":%lld}", version,(int)status.count,status.data,(long long)owner,(long long)process,(long long)rev,(long long)((code==0||code==1)?accepted:0));
  api_json(q->fd, 200, out); return;
failed:
  if (r) PQclear(r);
  api_log("room-checkpoint: %s", PQerrorMessage(db));
  progress_run(db, "ROLLBACK");
  api_json(q->fd, 500, "{\"error\":\"not kept\"}");
}
static void room_vitals(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST vitals\"}"); return; }
  if (q->body_len == 0 || q->body_len > 8192) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  int signer = room_write_signed(q, "vitals", q->body, q->body_len);   /* this shard's write key, or the shared key (#13260) */
  if (signer < 0) return;
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  long long at = 0;
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  if (sscanf(b, "t %lld", &at) != 1 || at < now - 120000 || at > now + 120000) { free(b); api_json(q->fd, 400, "{\"error\":\"stale\"}"); return; }
  /* "o SHARD GEN BATCH" (#12793): every line has an operation ID, and the answer is a receipt */
  char *o = strchr(b, '\n');
  int oshard = 0; long long ogen = 0, obatch = 0;
  bool owned = o && sscanf(o + 1, "o %d %lld %lld", &oshard, &ogen, &obatch) == 3;
  /* a shard's own key posts its own vitals only, with their operation IDs (#13260) */
  if (signer > 0 && (!owned || oshard != signer)) { free(b); api_json(q->fd, 403, "{\"error\":\"another shard's vitals\"}"); return; }
  if (owned) {
    room_vitals_receipt(q, db, o, oshard, ogen, obatch, at);
    free(b);
    return;
  }
  int kept = 0;
  char *line = strchr(b, '\n');
  while (line && *++line) {
    char pid[48]; int hp, max, xp = -1;
    char *next = strchr(line, '\n');
    char one[128]; size_t ol = next ? (size_t)(next - line) : strlen(line);
    if (ol >= sizeof one) ol = sizeof one - 1;
    memcpy(one, line, ol); one[ol] = 0;
    int got = sscanf(one, "%47s %d %d %d", pid, &hp, &max, &xp);
    if (got < 4 || xp < 0 || xp > 9999999) xp = -1;
    if (got >= 3 && hp >= 0 && max > 0 && hp <= max && max < 100000) {
      char hs[16], ms[16], as[24], xs[16];
      snprintf(hs, sizeof hs, "%d", hp); snprintf(ms, sizeof ms, "%d", max); snprintf(as, sizeof as, "%lld", at);
      snprintf(xs, sizeof xs, "%d", xp);
      const char *v[5] = {pid, hs, ms, as, xs};
      /* XP only rises: the larger of the kept and the reported (a -1 reports none) */
      PGresult *r=PQexecParams(db,"SELECT verse.room_vital_legacy($1,$2::int,$3::int,$5::int,$4::bigint)",5,NULL,v,NULL,NULL,0);
      if(PQresultStatus(r)==PGRES_TUPLES_OK&&PQntuples(r)==1)kept+=atoi(PQgetvalue(r,0,0));
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
  char at[32];
  if (!api_header(q, "X-Room-At", at, sizeof at)) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  char text[48]; int tn = snprintf(text, sizeof text, "t %s", at);
  if (tn <= 0 || tn >= (int)sizeof text) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  /* this shard's write key, or the shared key (#13260); the control answer is the realm's, not a shard's */
  if (room_write_signed(q, "control", text, (size_t)tn) < 0) return;
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000, when = atoll(at);
  if (when < now - 120000 || when > now + 120000) { api_json(q->fd, 401, "{\"error\":\"bad mac\"}"); return; }
  PGresult *r = PQexecParams(db,
    "SELECT 'pvp ' || CASE WHEN coalesce((SELECT (switched->>'pvp')::boolean FROM verse.realm_edit WHERE slug = 'everglade'), false) THEN '1' ELSE '0' END || chr(10) || "
    "'edit ' || greatest(coalesce((SELECT ms FROM verse.realm_edit WHERE slug = 'everglade'), 0), "
    "  coalesce((SELECT (published_at * 1000)::bigint FROM verse.published WHERE world = 'everglade'), 0)) || chr(10) || "
    "coalesce((SELECT string_agg('kick ' || player, chr(10)) || chr(10) FROM verse.room_kicks WHERE until_ms > verse.now_ms()), '') || "
    /* the world event's dials and the levels switch (#12738, #12739; room-events.coax ev_control_line) */
    "coalesce((SELECT 'event ' || concat_ws(' ', (d->>'on')::boolean::int, d->>'every', d->>'waves', d->>'size', d->>'cultists', d->>'fanatics', "
    "  d->>'warlocks', d->>'warlock_level', d->>'gates', (d->>'leader')::boolean::int, d->>'aggression', (d->>'town_damage')::boolean::int, "
    "  d->>'reward', d->>'minutes', e.start_n, e.stop_n) || chr(10) || 'levels ' || (d->>'levels')::boolean::int || ' ' || (d->>'level_cap') || chr(10) "
    "  FROM verse.room_events e, LATERAL (SELECT verse.room_event_dials(e.dials) AS d) x WHERE e.slug = 'everglade'), '')",
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

// Everglade's world event (#12738; migrations/0118_room_events.sql; spec section 8):
//   GET  /api/playtest/room-event   -> {dials, start_n, stop_n, started_ms, admin}: the dials, each clamped
//   POST /api/playtest/room-event {dials?, start?: true, stop?: true}   a realm admin's change -> the same; 403
// The dials POSTed are merged into what is kept (one not named keeps its value). START counts a press
// (every live instance starts within the 2 s poll, when the event is on), STOP the same.
static void room_event(const request *q, PGconn *db, const char *key, bool site, const char *body) {
  bool post = !strcmp(q->method, "POST");
  const char *v[4] = {key ? key : "", post ? body : "{}", site ? "t" : "f", post ? "t" : "f"};
  PGresult *r = PQexecParams(db,
    "WITH b AS (SELECT $2::jsonb AS j), may AS (SELECT verse.edit_may('everglade', $1, $3::boolean) AS ok), "
    "up AS (UPDATE verse.room_events e SET dials = verse.room_event_dials(e.dials || coalesce(CASE WHEN jsonb_typeof(j->'dials') = 'object' THEN j->'dials' END, '{}'::jsonb)), "
    "  start_n = e.start_n + CASE WHEN j->'start' = 'true'::jsonb THEN 1 ELSE 0 END, "
    "  stop_n = e.stop_n + CASE WHEN j->'stop' = 'true'::jsonb THEN 1 ELSE 0 END, "
    "  started_ms = CASE WHEN j->'start' = 'true'::jsonb THEN verse.now_ms() ELSE e.started_ms END, "
    "  ms = verse.now_ms(), by_who = coalesce(verse.player_of($1), 'site') "
    "  FROM b WHERE e.slug = 'everglade' AND $4::boolean AND (SELECT ok FROM may) RETURNING e.*) "
    "SELECT (SELECT ok FROM may), json_build_object('dials', verse.room_event_dials(x.dials), 'start_n', x.start_n, 'stop_n', x.stop_n, "
    "  'started_ms', x.started_ms, 'admin', (SELECT ok FROM may))::text "
    "FROM (SELECT * FROM up UNION ALL SELECT * FROM verse.room_events WHERE slug = 'everglade' AND NOT EXISTS (SELECT 1 FROM up)) x",
    4, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-event: %s", PQerrorMessage(db));
    PQclear(r); api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"no event\"}"); return;
  }
  if (PQntuples(r) != 1) { PQclear(r); api_json(q->fd, 404, "{\"error\":\"no event\"}"); return; }
  bool may = PQgetvalue(r, 0, 0)[0] == 't';
  if (post && !may) { PQclear(r); api_json(q->fd, 403, "{\"error\":\"Everglade's admins only\"}"); return; }
  api_json(q->fd, 200, PQgetvalue(r, 0, 1));
  PQclear(r);
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
  if (!strcmp(rest, "room-items")) { room_items(q, db); return true; }   // the item ledger (#12849)
  if (!strcmp(rest, "room-checkpoint-v2")) { room_checkpoint(q,db,2); return true; }
  if (!strcmp(rest, "room-checkpoint")) { room_checkpoint(q, db, 1); return true; } // character-checkpoint-v1 (#12838)
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
  if (!strncmp(rest, "room-ticket/", 12) && segs == 2) { room_ticket(q, db, key, site, rest + 12, body); free(body); return true; }
  if (!strncmp(rest, "room-invite/", 12) && segs == 2) { room_invite(q, db, key, rest + 12); free(body); return true; }
  if (!strcmp(rest, "room-state")) { room_state(q, db, key, body); free(body); return true; }
  if (!strcmp(rest, "party") || !strncmp(rest, "party/", 6)) { party(q, db, key, rest[5] == '/' ? rest + 6 : "", body); free(body); return true; }
  if (!strcmp(rest, "player/name")) {   // a guest's own name, through the same filter (migrations/0116)
    if (strcmp(q->method, "POST")) { free(body); api_json(q->fd, 405, "{\"error\":\"POST a name\"}"); return true; }
    const char *nv[2] = {key, body};
    PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.player_set_name($1, $2::jsonb) x", 2, NULL, nv, NULL, NULL, 0);
    free(body);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
      const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      bool theirs = state && !strncmp(state, "22", 2);
      if (!theirs) api_log("player/name: %s", PQerrorMessage(db));
      PQclear(r); api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"no name\"}"); return true;
    }
    api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
    PQclear(r);
    return true;
  }
  if (!strcmp(rest, "room-kick")) { room_kick(q, db, key, site, body); free(body); return true; }
  if (!strcmp(rest, "room-event")) { room_event(q, db, key, site, body); free(body); return true; }

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

  if (progress_post && good) { progress_save(q, db, parts, body, who); free(body); free(who); return true; }
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
