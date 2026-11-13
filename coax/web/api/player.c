// Players, guests, saves and published worlds for the Verse playtest
// (pt-player-endpoints): the demo's player and world routes (aethyr-cpp
// tools/serve.py, docs/metaverse/playtest-port.md section 3) under
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
#include "item-policy.generated.h"
#include "moderation-policy.generated.h"
#include "character-policy.generated.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

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
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST feedback"); return; }
  char autom[64];
  automation(q, autom, sizeof autom);
  char *body = malloc(q->body_len + 3);
  if (!body) { api_error(q->fd, 503, "memory"); return; }
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
    api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "not kept");
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
  if (!progress_run(db, "BEGIN")) { api_error(q->fd, 500, "not kept"); return; }
  const char *fv[3] = {parts, body, who};
  PGresult *f = PQexecParams(db, "SELECT (f->>'status')::int, coalesce(f->'body', 'null')::text, f->>'acct', f->>'slot', "
                                 "coalesce(f->>'rev', '0'), coalesce(f->>'marked', 'false'), coalesce(f->>'sent_rev', '-1') "
                                 "FROM verse.progress_facts($1::text[], $2::jsonb, $3::jsonb) f", 3, NULL, fv, NULL, NULL, 0);
  if (PQresultStatus(f) != PGRES_TUPLES_OK || PQntuples(f) != 1) {
    const char *state = PQresultErrorField(f, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("progress: %s", PQerrorMessage(db));
    PQclear(f); progress_run(db, "ROLLBACK");
    api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "not kept");
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
    api_error(q->fd, 500, "not kept");
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

/* metaverse-engine/session/moderation.coax's answers (#13302) */
#define MOD_OK 0
#define MOD_SELF 2
#define MOD_KICK 3
#define MOD_UNKICK 6
#define MOD_KIND_KICK 1
#define MOD_KIND_MUTE 2
#define MOD_KIND_BAN 3
// A seat ticket (#12712, spec 4.4): who the X-Verse-Player key is, for one room's socket, for
// TICKET_MS. metaverse-engine/session/seat-ticket.coax writes every byte of it (docs/protocol/everglade-session-v7.md
// "The seat ticket"): v5 signed with OA_ROOM_TICKET_SIGNING_KEY (#13260), else v2/v3, or the v4 envelope
// around one, under OA_ROOM_TICKET_KEY's HMAC (#13248). coax/web/room/room-ticket.coax reads and checks it.
// The name is verse.player_name's, the one chat, presence and the inbox show too
// (migrations/0116_one_player_name.sql), kept to letters, digits, spaces, '-' and '_', 24 at most.
int room_write_signed(const request *q, const char *purpose, const char *body, size_t n);   /* rooms.c (#13260) */
#define TICKET_MS 600000
/* seat ticket v5's one writer (ticket-sign-unit.c, metaverse-engine/session/seat-ticket.coax; #13260) */
bool stk_secret_c(const char *text, unsigned char out[32]);
size_t stk_write_c(const unsigned char secret[32], const char *pid, int acct, const char *name, const char *room, int shard,
                   long long expires, int hp, int xp, int wanted, long long owner, const char *ext, int profile, const char *saved,
                   char *out, size_t cap);
/* seat tickets v2 to v4's one writer (ticket-sign-unit.c, seat_ticket_hmac_write; #13248) */
size_t stk_hmac_write_c(const char *key, const char *pid, int acct, const char *name, const char *room, int shard,
                        long long expires, int hp, int xp, int wanted, long long owner, int envelope, const char *saved,
                        char *out, size_t cap);
/* the EXT's class and level (metaverse-engine/session/seat-class.coax, #13259) */
size_t stk_class_ext_c(const char *klass, int xp, int start, char *out, size_t cap);
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
// may make in a minute, counted in Postgres (api_limit) so the limit is one for every worker and
// instance (#13300). Over, the answer is 429 with the ms until the window ends (and Retry-After), and
// the refusal is counted as 'rate' at once.
#define ROOM_ASKS_KEY 12
#define ROOM_ASKS_ADDR 120
// 0 when allowed, else the ms until who's window ends.
static long long room_admit(PGconn *db, const char *kind, const char *who, int per_minute) {
  long long left = 0;
  return api_limit(db, kind, who, per_minute, &left) ? 0 : left;
}
// The visitor's address: the load balancer's next to last (as access.c's).
static void room_visitor(const request *q, char ip[128]) {
  char xff[512] = "";
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  snprintf(ip, 128, "%s", np ? parts[np >= 2 ? np - 2 : 0] : "local");
}
// A refusal counted by reason.
static void room_count(PGconn *db, const char *reason) {
  const char *v[1] = {reason};
  PGresult *r = PQexecParams(db, "SELECT verse.room_admit_count($1, 1)", 1, NULL, v, NULL, NULL, 0);
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
// The character record (#13290, migrations/0169_characters.sql, character-policy.coax): kept at every
// seat ticket, under its row's lock, from the record stored and the facts the server holds: the class
// kept once, the greatest XP (xp), the realm's starting level (start) and the item ledger's items. The
// ticket's class and level are then the record's. 1 kept (rec holds it), 0 none for this player (its
// identity is not a record's: the ticket signs as before), -1 the database failed.
/* The characters' levelled event (#13319, migrations/0176): when a stored record's level rose, its row in
   the characters' outbox, in the caller's transaction (everglade/model/events.coax ev_levelled decides).
   before: the record as it was stored ("" none). False when the database failed. */
static bool character_outbox(PGconn *db, const char *who, const char *before, const char *after) {
  int64_t level = ev_levelled((coax_string){before, (int64_t)strlen(before)}, (coax_string){after, (int64_t)strlen(after)});
  if (level <= 0) return true;
  char ls[24], xs[24];
  snprintf(ls, sizeof ls, "%lld", (long long)level);
  snprintf(xs, sizeof xs, "%lld", (long long)character_xp((coax_string){after, (int64_t)strlen(after)}));
  coax_string id = ev_id(1);   /* events.coax EV_LEVELLED: the id is the codec's */
  char ids[64]; snprintf(ids, sizeof ids, "%.*s", (int)id.count, id.data);
  const char *ov[4] = {who, ls, xs, ids};
  PGresult *r = PQexecParams(db, "INSERT INTO verse.character_outbox (event, who, level, xp, at_ms) VALUES ($4, $1, $2::int, $3::bigint, verse.now_ms())", 4, NULL, ov, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  return ok;
}
static int ticket_character(PGconn *db, const char *pid, const char *klass, int xp, int start, char *rec, size_t cap) {
  const char *pv[1] = {pid}; PGresult *r = NULL; char who[96], kept[4097] = "", items[1024] = "[]"; long long inv = 0;
  if (!progress_run(db, "BEGIN")) return -1;
  r = PQexecParams(db, "SELECT coalesce(account,id) FROM verse.players WHERE id=$1 FOR SHARE", 1, NULL, pv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || strlen(PQgetvalue(r, 0, 0)) >= sizeof who) goto fail;
  snprintf(who, sizeof who, "%s", PQgetvalue(r, 0, 0)); PQclear(r); r = NULL;
  const char *wv[1] = {who};
  r = PQexecParams(db, "SELECT coalesce((SELECT body::text FROM verse.characters WHERE who=$1 FOR UPDATE),''),"
                       "coalesce((SELECT json_agg(id ORDER BY id)::text FROM (SELECT id FROM verse.items WHERE owner=$1 ORDER BY id LIMIT 33) i),'[]'),"
                       "coalesce((SELECT version FROM verse.inventories WHERE who=$1),0)", 1, NULL, wv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || strlen(PQgetvalue(r, 0, 0)) >= sizeof kept || strlen(PQgetvalue(r, 0, 1)) >= sizeof items) goto fail;
  snprintf(kept, sizeof kept, "%s", PQgetvalue(r, 0, 0)); snprintf(items, sizeof items, "%s", PQgetvalue(r, 0, 1)); inv = atoll(PQgetvalue(r, 0, 2));
  PQclear(r); r = NULL;
  coax_string made = character_ticket((coax_string){kept, (int64_t)strlen(kept)}, (coax_string){who, (int64_t)strlen(who)}, (coax_string){klass, (int64_t)strlen(klass)},
                                      xp, start, (coax_string){items, (int64_t)strlen(items)}, inv, (coax_v_u8){(uint8_t *)rec, (int64_t)cap - 1});
  if (made.count <= 0) { api_log("room-ticket: no character record for %s", who); progress_run(db, "ROLLBACK"); rec[0] = 0; return 0; }
  rec[made.count] = 0;
  const char *bv[2] = {who, rec};
  r = PQexecParams(db, "INSERT INTO verse.characters AS c (who, body, at_ms) VALUES ($1, $2::jsonb, verse.now_ms()) "
                       "ON CONFLICT (who) DO UPDATE SET body = EXCLUDED.body, at_ms = EXCLUDED.at_ms WHERE c.body IS DISTINCT FROM EXCLUDED.body", 2, NULL, bv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_COMMAND_OK) goto fail;
  PQclear(r); r = NULL;
  if (!character_outbox(db, who, kept, rec)) goto fail;
  if (!progress_run(db, "COMMIT")) goto fail;
  return 1;
fail:
  if (r) PQclear(r);
  api_log("room-ticket: character: %s", PQerrorMessage(db));
  progress_run(db, "ROLLBACK"); rec[0] = 0; return -1;
}
static void room_ticket(const request *q, PGconn *db, const char *key, bool site, const char *room, const char *body) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a ticket"); return; }
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  /* v5 (#13260, metaverse-engine/session/seat-ticket.coax; docs/deployment/room-keys.md): set, the API signs with this
     private key (64 hex) and rooms check with its public key only. Unset, v2/v3/v4 under OA_ROOM_TICKET_KEY. */
  const char *signing = getenv("OA_ROOM_TICKET_SIGNING_KEY");
  unsigned char v5_key[32]; bool v5 = signing && signing[0];
  if (v5 ? !stk_secret_c(signing, v5_key) : (!secret || strlen(secret) < 32)) { api_error(q->fd, 503, "no ticket key"); return; }
  {
    char ip[128]; room_visitor(q, ip);
    long long wait = room_admit(db, "room-key", key && key[0] ? key : "-", ROOM_ASKS_KEY);
    long long wait_ip = room_admit(db, "room-ip", ip, ROOM_ASKS_ADDR);
    if (wait_ip > wait) wait = wait_ip;
    if (wait > 0) {
      room_count(db, "rate");
      char hdr[48], out[96];
      snprintf(hdr, sizeof hdr, "Retry-After: %lld\r\n", (wait + 999) / 1000);
      int on = (int)api_error_num_text(out, sizeof out, "slow down", "retry_ms", wait);
      api_respond(q->fd, 429, "application/json", hdr, out, (size_t)on);
      return;
    }
  }
  size_t rn = strlen(room);
  if (!rn || rn > 24) { api_error(q->fd, 404, "unknown"); return; }
  for (size_t i = 0; i < rn; i++) {
    char ch = room[i];
    if (!((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '-')) { api_error(q->fd, 404, "unknown"); return; }
  }
  if (!key || !key[0]) { api_error(q->fd, 401, "a player key"); return; }
  const char *v[1] = {key};
  // the saved state (verse.room_vitals, #12715): HP for 10 minutes, WANTED for 5 (only a room's: 0124,
  // #12794), the look and class always
  PGresult *r = PQexecParams(db, "SELECT p.id, p.account IS NOT NULL, verse.player_name(p.id), "
                                 "CASE WHEN rv.hp_at > extract(epoch FROM clock_timestamp()) * 1000 - 600000 THEN rv.hp END, "
                                 "CASE WHEN rv.wanted_by IS NOT NULL AND rv.wanted_at > extract(epoch FROM clock_timestamp()) * 1000 - 300000 THEN rv.wanted END, rv.look, rv.class, "
                                 "(SELECT k.until_ms FROM verse.room_kicks k WHERE k.player = p.id), "
                                 "(SELECT string_agg(b.blocked, ',' ORDER BY b.at_ms DESC) FROM (SELECT * FROM verse.player_blocks WHERE player = p.id ORDER BY at_ms DESC LIMIT 32) b), "
                                 "coalesce(rv.xp, 0), rv.character_class, "
                                 /* the kept ban, the clock and whether they are one of the team's (#13302, migrations/0170) */
                                 "(SELECT s.until_ms FROM verse.mod_sanctions s WHERE s.who = coalesce(p.account, p.id) AND s.kind = 'ban'), verse.now_ms(), "
                                 "EXISTS (SELECT 1 FROM analytics.team t WHERE t.kind = 'player' AND t.value = p.id) "
                                 "FROM verse.players p LEFT JOIN verse.accounts a ON a.id = p.account "
                                 "LEFT JOIN verse.room_vitals rv ON rv.who = coalesce(p.account, p.id) "
                                 "WHERE p.key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0);
  (void)body;
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-ticket: %s", PQerrorMessage(db));
    PQclear(r);
    api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "no ticket");
    return;
  }
  if (PQntuples(r) != 1) { PQclear(r); api_error(q->fd, 403, "unknown player key"); return; }
  char pid[64], name[32];
  snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0));
  bool account = PQgetvalue(r, 0, 1)[0] == 't';
  ticket_name(name, sizeof name, PQgetvalue(r, 0, 2));
  {
    /* no ticket for a banned or kicked player (#13302, #12714): moderation-policy.coax decides from the kept
       rows; a refusal is counted unless they are one of the team's */
    bool ban = !PQgetisnull(r, 0, 11), kick = !PQgetisnull(r, 0, 7);
    long long ban_until = ban ? atoll(PQgetvalue(r, 0, 11)) : 0, kick_until = kick ? atoll(PQgetvalue(r, 0, 7)) : 0;
    int stop = (int)mod_ticket(ban, ban_until, kick, kick_until, atoll(PQgetvalue(r, 0, 12)));
    if (stop) {
      bool team = PQgetvalue(r, 0, 13)[0] == 't';
      coax_string w = mod_kind_word(stop);
      char word[16];
      snprintf(word, sizeof word, "%.*s", (int)w.count, w.data);
      PQclear(r);
      if (!team) room_count(db, word);
      api_error_num(q->fd, 403, word, "until", stop == MOD_KIND_BAN ? ban_until : kick_until);
      return;
    }
  }
  int hp = PQgetisnull(r, 0, 3) ? -1 : atoi(PQgetvalue(r, 0, 3));
  int wanted = PQgetisnull(r, 0, 4) ? -1 : atoi(PQgetvalue(r, 0, 4));
  int xp = atoi(PQgetvalue(r, 0, 9));   /* XP, which only rises (#12739); the ticket's HP field carries it as HPxXP */
  if (xp < 0 || xp > 9999999) xp = 0;
  char look[40] = "", klass[24] = "";
  if (!PQgetisnull(r, 0, 5)) ticket_word(look, sizeof look, PQgetvalue(r, 0, 5));
  if (!PQgetisnull(r, 0, 6)) ticket_word(klass, sizeof klass, PQgetvalue(r, 0, 6));
  char kept_class[24] = "";   /* Everglade II's class, kept once (0160, #13259) */
  if (!PQgetisnull(r, 0, 10)) snprintf(kept_class, sizeof kept_class, "%s", PQgetvalue(r, 0, 10));
  // who this player blocks (verse.player_blocks, #12714), 32 at most: the page hides their bodies
  char blocked[32 * 24 + 8] = "";
  if (!PQgetisnull(r, 0, 8)) {
    size_t bn = 0;
    for (const char *c = PQgetvalue(r, 0, 8); *c && bn + 1 < sizeof blocked; c++)
      if ((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-' || *c == ',') blocked[bn++] = *c;
    blocked[bn] = 0;
  }
  /* maintenance (#13299, verse.flags; service-policy.coax sv_maintenance): nobody new is seated, and the
     answer says why and until when; the page shows the words and asks again */
  {
    char why[161]; long long until = 0;
    if (flags_maintenance(db, why, sizeof why, &until)) {
      char out[512];
      int on = snprintf(out, sizeof out, "{\"error\":\"maintenance%s%s\",\"maintenance\":\"%s\",\"until_ms\":%lld,\"retry_ms\":30000}", why[0] ? ": " : "", why, why, until);
      api_respond(q->fd, 503, "application/json", "Retry-After: 30\r\n", out, (size_t)on);
      return;
    }
  }
  PQclear(r);
  for (char *c = pid; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-')) { api_error(q->fd, 500, "no ticket"); return; }
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
    const char *rn_s = "0";   /* the 429s are counted as they happen (room_admit) */
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
    PQclear(sr);
    room = placed;
  }
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long expires = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000 + TICKET_MS;
  char saved[2049]="-"; long long checkpoint_owner=0;
  int seat_shard = shard > 0 ? shard : 1;   /* the shard this ticket seats (a v2 ticket: shard 1); room_owners notes it (0156) */
  int resume=ticket_resume(q,db,pid,seat_shard,saved,&xp,&checkpoint_owner);
  if (resume<0) {api_error(q->fd,503,"character recovery unavailable");return;}
  if (resume==1) {
    coax_string cls=checkpoint2_class((coax_string){saved,(int64_t)strlen(saved)});
    if(cls.count)snprintf(klass,sizeof klass,"%.*s",(int)cls.count,cls.data);
  }
  /* the character's next owner generation (#12834, migrations/0127), last in the ticket's HP field, once
     every shard reads it (OA_ROOM_OWNER_GEN=1; a room from before refuses a ticket with it). The room
     sends it with its vitals lines, and a line below the character's newest generation is not kept. */
  const char *og = getenv("OA_ROOM_OWNER_GEN");
  long long owner_gen = 0;   /* 0: none */
  if (resume==1) owner_gen = checkpoint_owner;
  else if (og && !strcmp(og, "1")) {
    char ss[12]; snprintf(ss, sizeof ss, "%d", seat_shard);
    const char *gv[2] = {pid, ss};
    PGresult *gr = PQexecParams(db, "SELECT verse.room_owner_next($1, $2::int)", 2, NULL, gv, NULL, NULL, 0);   /* and its shard (0156) */
    if (PQresultStatus(gr) == PGRES_TUPLES_OK && PQntuples(gr) == 1 && !PQgetisnull(gr, 0, 0)) owner_gen = atoll(PQgetvalue(gr, 0, 0));
    else if (PQresultStatus(gr) != PGRES_TUPLES_OK) api_log("room-owner: %s", PQerrorMessage(db));
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
  /* the character record (#13290): kept at every ticket, and the class and level the ticket signs are its
     own (the realm's starting level, OA_ROOM_START_LEVEL, 1 to 6, default 1, is its floor) */
  const char *sl = getenv("OA_ROOM_START_LEVEL");
  int start = sl && sl[0] ? atoi(sl) : 1;
  if (start < 1 || start > 6) start = 1;
  {
    char rec[4097];
    int kept_rec = ticket_character(db, pid, kept_class, xp, start, rec, sizeof rec);
    if (kept_rec < 0) { api_error(q->fd, 503, "character recovery unavailable"); return; }
    if (kept_rec == 1) {
      coax_string rc = (coax_string){rec, (int64_t)strlen(rec)};
      char cw[24]; coax_string w = character_class_word(rc, (coax_v_u8){(uint8_t *)cw, (int64_t)sizeof cw - 1});
      cw[w.count > 0 ? w.count : 0] = 0;
      snprintf(kept_class, sizeof kept_class, "%s", cw);
      xp = (int)character_xp(rc); start = (int)character_floor(rc);
    }
  }
  /* the ticket, every byte of it metaverse-engine/session/seat-ticket.coax's (ticket-sign-unit.c). HP, XP, WANTED and the
     owner generation ride in its HP field (the WANTED level the room saved, #12794: the room seats it wanted). */
  char ticket[4097];
  bool unclassed = false;
  if (v5) {
    /* v5: shard 0 (no directory) is shard 1's, as a v2 ticket is */
    /* the character's class, kept once (0160), and the level its XP gives, at least the realm's starting
       level (OA_ROOM_START_LEVEL, 1 to 6, default 1: the server's, never the page's): the room seats it so (#13259) */
    char ext[40] = "-";
    if (!stk_class_ext_c(kept_class, xp, start, ext, sizeof ext)) snprintf(ext, sizeof ext, "-");
    unclassed = strncmp(ext, "c=", 2) != 0;   /* no class kept for this character yet: the page opens Character Create before its hello (#13268) */
    if (!stk_write_c(v5_key, pid, acct, name, room, seat_shard, expires, hp, xp, wanted, owner_gen, ext, resume == 1 ? 2 : 1, saved, ticket, sizeof ticket)) {
      api_error(q->fd, resume == 1 ? 503 : 500, resume == 1 ? "character recovery unavailable" : "no ticket"); return;
    }
  } else if (!stk_hmac_write_c(secret, pid, acct, name, room, shard, expires, hp, xp, wanted, owner_gen, resume == 1 ? 1 : 0, saved, ticket, sizeof ticket)) {
    /* v2 (shard 0: no directory) or v3, wrapped in the v4 envelope with the character's capture when it resumes one */
    api_error(q->fd, resume == 1 ? 503 : 500, resume == 1 ? "character recovery unavailable" : "no ticket"); return;
  }
  /* the answer's tail: every part is added whole or the answer is refused, never cut short (#13300) */
  char out[8192], en[48], st[2048] = "";
  bool st_whole = true;
#define ST_ADD(...) do { int k_ = snprintf(st + sn, sizeof st - (size_t)sn, __VA_ARGS__); if (k_ < 0 || k_ >= (int)sizeof st - sn) { st_whole = false; st[sn] = 0; } else sn += k_; } while (0)
  esc(en, sizeof en, name);
  int sn = 0;
  if (hp >= 0) ST_ADD(",\"hp\":%d", hp);
  if (wanted >= 0) ST_ADD(",\"wanted\":%d", wanted);
  if (xp > 0) ST_ADD(",\"xp\":%d", xp);
  if (look[0]) ST_ADD(",\"look\":\"%s\"", look);
  if (klass[0]) ST_ADD(",\"class\":\"%s\"", klass);
  if (unclassed) ST_ADD(",\"unclassed\":true");
  if (shard > 0) ST_ADD(",\"room\":\"%s\",\"shard\":%d%s", room, shard, full ? ",\"full\":true" : "");
  /* the shard's own endpoint when it names one (a room VM, #13179): the page opens it in place of the
     load balancer's /api/room/sK/ */
  { char ep[200]; if (shard > 0 && rooms_endpoint(db, shard, ep, sizeof ep)) ST_ADD(",\"endpoint\":\"%s\"", ep); }
  if (shard > 0 && party_full) ST_ADD(",\"party_full\":\"%s\"", party_room);
  if (shard > 0 && party_away) ST_ADD(",\"party_away\":true");
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
    ST_ADD(",\"asked\":\"%s\"%s%s%s", asked, by[0] ? ",\"by\":\"" : "", eb, by[0] ? "\"" : "");
  }
  if (blocked[0]) ST_ADD(",\"blocked\":\"%s\"", blocked);
#undef ST_ADD
  int on = snprintf(out, sizeof out, "{\"ticket\":\"%s\",\"id\":\"%s\",\"account\":%s,\"admin\":%s,\"name\":\"%s\",\"expires\":%lld%s}", ticket, pid, account ? "true" : "false", acct == 2 ? "true" : "false", en, expires, st);
  if (!st_whole || on < 0 || on >= (int)sizeof out) { api_log("room-ticket: the answer did not fit (%d)", on); api_error(q->fd, 500, "no ticket"); return; }
  api_json(q->fd, 200, out);
}

// INVITE (#12795): one seat in the player's room held for their friends ROOMS_INVITE_MS (two minutes,
// migrations/0125_room_invites.sql). {"room":R,"until":MS}; 403 not in it, 409 full.
static void room_invite(const request *q, PGconn *db, const char *key, const char *room) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST an invite"); return; }
  if (!key || !key[0]) { api_error(q->fd, 401, "a player key"); return; }
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, "SELECT id FROM verse.players WHERE key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); api_error(q->fd, 403, "unknown player key"); return; }
  char pid[64]; snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  long long until = 0;
  int got = rooms_invite(db, room, pid, &until);
  if (got == 1) { api_error(q->fd, 403, "not in it"); return; }
  if (got == 2) { api_error(q->fd, 409, "full"); return; }
  if (got < 0) { api_error(q->fd, 500, "no invite"); return; }
  char out[96]; snprintf(out, sizeof out, "{\"room\":\"%s\",\"until\":%lld}", room, until);
  api_json(q->fd, 200, out);
}

// Durable parties (#12852, migrations/0129_parties.sql): invite, join, leave, hide, and the roster,
// health only. sub is "" (GET the roster) or invite, join, leave, hide (POST).
static void party(const request *q, PGconn *db, const char *key, const char *sub, const char *body) {
  if (!key || !key[0]) { api_error(q->fd, 401, "a player key"); return; }
  bool get = !strcmp(q->method, "GET");
  if (sub[0] ? get : !get) { api_error(q->fd, 405, sub[0] ? "POST it" : "GET the party"); return; }
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, "SELECT id FROM verse.players WHERE key_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); api_error(q->fd, 403, "unknown player key"); return; }
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
  if (!sql) { api_error(q->fd, 404, "unknown"); return; }
  r = PQexecParams(db, sql, strstr(sql, "$2") ? 2 : 1, NULL, pv, NULL, NULL, 0);   /* (an unused $2 has no type) */
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("party: %s", PQerrorMessage(db));
    PQclear(r); api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "no party"); return;
  }
  api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
}

// The page's part of what a player brings back into a room (#12715): {"look":"...","class":"..."},
// either, kept by the account when logged in, else the player. WANTED is not the page's (#12794): the
// room reports it signed (room_vitals). A body that names "wanted" keeps the rest and says
// "refused":"wanted"; one with only "wanted" is a 403.
static void room_state(const request *q, PGconn *db, const char *key, const char *body) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST the state"); return; }
  if (!key || !key[0]) { api_error(q->fd, 401, "a player key"); return; }
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
    api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "not kept");
    return;
  }
  bool known = PQgetvalue(r, 0, 0)[0] == 't', wanted = PQgetvalue(r, 0, 2)[0] == 't';
  int kept = atoi(PQgetvalue(r, 0, 1));
  PQclear(r);
  if (!known) { api_error(q->fd, 403, "unknown player key"); return; }
  if (!kept && wanted) { api_error_word(q->fd, 403, "WANTED is the room server's", "refused", "wanted"); return; }
  api_json(q->fd, 200, wanted ? "{\"kept\":true,\"refused\":\"wanted\"}" : "{\"kept\":true}");
}
// A vitals post with operation IDs (#12793; room-vitals.coax rm_vitals_due, migrations/0124): each line
// "PID HP MAX XP WANTED REV AT [OWNER] [c=CLASS]" applied once by (SHARD, GEN, PID, REV) in verse.room_vital_apply, its
// own statement, so a line kept stays kept whatever happens to the next. The answer, text/plain:
//   receipt GEN BATCH CODES      one letter a line, in order: a d s u (0124), f an older owner's (0127), x a line it couldn't read
// A database failure is a 500 with no receipt: the room sends the post again, and the lines already
// applied answer d. AT is when the room changed the values; it orders HP and WANTED, and is taken
// within the outbox's 15 minutes before the post's time.
// The room reports' codec (metaverse-engine/session/room-report.coax, through ticket-sign-unit.c): the receipts
// this file answers a room's vitals post and checkpoint post with (Protocol E7 and E8, #13250).
int room_report_receipt_c(long long gen, long long batch, const char *codes, char *out, size_t cap);
int room_report_checkpoint_receipt_c(int v, const char *status, size_t status_n, long long owner, long long process, long long rev, long long accepted_at, char *out, size_t cap);
static void room_vitals_receipt(const request *q, PGconn *db, const char *o, int shard, long long gen, long long batch, long long at) {
  char codes[128]; size_t cn = 0;
  if (shard < 1 || shard > 99 || gen <= 0) { api_error(q->fd, 400, "bad owner"); return; }
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
        api_error(q->fd, 500, "not kept");
        return;
      }
      code = PQgetvalue(r, 0, 0)[0];
      PQclear(r);
      /* " c=CLASS": the class the character's first hello chose, its ticket naming none (#13259): kept
         once (0160), so a later one changes nothing; not for an older owner's line */
      const char *cl = strstr(one, " c=");
      if (cl && code != 'f') {
        const char *cv[2] = {pid, cl + 3};
        PGresult *cr = PQexecParams(db, "SELECT verse.room_class_once($1, $2)", 2, NULL, cv, NULL, NULL, 0);
        if (PQresultStatus(cr) != PGRES_TUPLES_OK) api_log("room-class: %s", PQerrorMessage(db));
        PQclear(cr);
      }
    }
    codes[cn++] = code;
    line = next;
  }
  codes[cn] = 0;
  char out[192];
  int n = room_report_receipt_c(gen, batch, codes, out, sizeof out);   /* the receipt is room-report.coax's (Protocol E8, #13250) */
  if (n < 0) { api_error(q->fd, 500, "not kept"); return; }
  api_respond(q->fd, 200, "text/plain", NULL, out, (size_t)n);
}
// oa-room's part (#12715): "t MS\nPID HP MAX XP\n..." (XP since #12739; a line without it keeps the XP) signed with the seat-ticket key (X-Room-Mac: the
// hex HMAC-SHA256 of the body), within two minutes of now. Each seat's HP is kept by who the
// player is, unless a newer one is already kept.
// The inventory and reward ledger (#12849, migrations/0130_item_ledger.sql; #13291): one operation, signed
// with the shard's write key, as everglade.item-op.v1 (everglade/model/item-op.coax,
// docs/protocol/everglade-item-op-v1.md): {id, op, pid, gen, ver, item, to, kind, sent} and a claim on the
// character's record; the answer is the ledger's status and the character as the store now has it.
// Game items only.
// An item or checkpoint write signed with shard SIGNER's own key (#13260): refused (403, and true)
// when the player's newest owner generation is another shard's and the write's generation (the
// JSON field GEN) is not older (verse.room_owner_shard_ok, 0156). An older one is left to the fence
// (stale, fenced), as before. A shared-key write (SIGNER 0) is not bound.
static bool room_write_owner(const request *q, PGconn *db, int signer, const char *gen) {
  if (signer <= 0) return false;
  char *b = malloc(q->body_len + 1);
  if (!b) { api_error(q->fd, 503, "memory"); return true; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  char sh[12]; snprintf(sh, sizeof sh, "%d", signer);
  const char *v[3] = {b, gen, sh};
  PGresult *r = PQexecParams(db, "SELECT verse.room_owner_shard_ok(j->>'pid', (j->>$2)::bigint, $3::int) FROM (SELECT $1::jsonb j) s", 3, NULL, v, NULL, NULL, 0);
  free(b);
  bool ok = PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetvalue(r, 0, 0)[0] != 'f';   /* not JSON: the write's own check answers */
  PQclear(r);
  if (!ok) api_error(q->fd, 403, "another shard's player");
  return !ok;
}
// The item ledger's route (#13318, docs/decisions/0006-item-ledger-rules-in-coax.md): SQL keeps the
// rows, locks (one writer an operation ID, the item, then the inventories in one order), keeps each
// ID once and writes the outbox; item-policy.coax (metaverse-engine/session/ledger.coax's rules) decides.
#define ITEM_GO (-1)       /* ledger.coax LEDGER_GO: a new ID, in time */
#define ITEM_REPLAY (-2)   /* ledger.coax LEDGER_REPLAY: answer the ID's record */
#define ITEM_OK 0          /* ledger.coax LEDGER_OK */
static PGresult *item_q(PGconn *db, const char *sql, int n, const char *const *v, ExecStatusType want) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == want) return r;
  PQclear(r);
  return NULL;
}
static char *item_dup(const PGresult *r, int row, int col) {   /* a cell, or NULL for SQL NULL */
  return PQgetisnull(r, row, col) ? NULL : strdup(PQgetvalue(r, row, col));
}
static bool item_same(const char *a, const char *b) { return a && b && !strcmp(a, b); }
static int64_t item_int(const char *s, int64_t none) { return s ? atoll(s) : none; }
/* The character as the store has it now, for an answer (everglade.item-op.v1): the stored record with
   the ledger's bag and version (character_op with no claim; rec "" when who has no record and can't
   have one), the content kind of each item in the bag, and the inventory version. False when the
   database failed. */
static bool item_view(PGconn *db, const char *who, char *rec, size_t rec_cap, char *kinds, size_t kinds_cap, long long *ver) {
  rec[0] = 0; snprintf(kinds, kinds_cap, "[]"); *ver = 0;
  if (!who) return true;
  const char *wv[1] = {who};
  PGresult *r = PQexecParams(db, "SELECT coalesce((SELECT body::text FROM verse.characters WHERE who=$1),''),"
    "'[' || coalesce((SELECT string_agg(id::text, ',' ORDER BY id) FROM (SELECT id FROM verse.items WHERE owner=$1 ORDER BY id LIMIT 33) i),'') || ']',"
    "'[' || coalesce((SELECT string_agg(kind::text, ',' ORDER BY id) FROM (SELECT id, kind FROM verse.items WHERE owner=$1 ORDER BY id LIMIT 33) i),'') || ']',"
    "coalesce((SELECT version FROM verse.inventories WHERE who=$1),0)", 1, NULL, wv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || strlen(PQgetvalue(r, 0, 2)) >= kinds_cap) { PQclear(r); return false; }
  const char *kept = PQgetvalue(r, 0, 0), *items = PQgetvalue(r, 0, 1);
  *ver = atoll(PQgetvalue(r, 0, 3));
  coax_string made = character_op((coax_string){kept, (int64_t)strlen(kept)}, (coax_string){who, (int64_t)strlen(who)}, (coax_string){items, (int64_t)strlen(items)},
                                  *ver, 0, 0, 0, 0, 0, -1, 0, (coax_v_u8){(uint8_t *)rec, (int64_t)rec_cap - 1});
  rec[made.count > 0 ? made.count : 0] = 0;
  if (rec[0]) snprintf(kinds, kinds_cap, "%s", PQgetvalue(r, 0, 2));   /* a kind for each item in the record's bag */
  PQclear(r);
  return true;
}
/* The answer, every byte of it everglade/model/item-op.coax's (io_answer). */
static void item_answer(const request *q, PGconn *db, const char *who, int64_t status, const char *item, bool replay) {
  char rec[4097], kinds[512], out[8193];
  long long ver = 0;
  if (!item_view(db, who, rec, sizeof rec, kinds, sizeof kinds, &ver)) { api_error(q->fd, 500, "not kept"); return; }
  coax_string a = io_answer(status, item ? atoll(item) : 0, replay, ver, (coax_string){kinds, (int64_t)strlen(kinds)}, (coax_string){rec, (int64_t)strlen(rec)},
                            (coax_v_u8){(uint8_t *)out, (int64_t)sizeof out - 1});
  if (a.count <= 0) { api_error(q->fd, 500, "not kept"); return; }
  out[a.count] = 0;
  api_json(q->fd, 200, out);
}
static void room_items(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a mutation"); return; }
  if (q->body_len == 0 || q->body_len > 4096) { api_error(q->fd, 401, "unsigned"); return; }
  int signer = room_write_signed(q, "items", q->body, q->body_len);   /* this shard's write key, or the shared key (#13260) */
  if (signer < 0) return;
  if (room_write_owner(q, db, signer, "gen")) return;   /* the shard that holds the player's seat (0156) */
  char body[4097]; memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  const char *bv[1] = {body};
  /* the ask's fields (everglade.item-op.v1): {id, op, pid, gen, ver, item, to, kind, sent} and its claim on
     the character's record {quest, from, stage, coin, xp, slot, wear}; a body that isn't JSON is a 400 */
  PGresult *r = PQexecParams(db, "SELECT j->>'id', j->>'op', verse.item_who(j->>'pid'), (j->>'gen')::bigint, (j->>'ver')::bigint, (j->>'item')::bigint, "
    "CASE WHEN coalesce(j->>'to', '') = '' THEN NULL ELSE verse.item_who(j->>'to') END, (j->>'kind')::int, (j->>'sent')::bigint, "
    "coalesce((j->>'quest')::bigint, 0), coalesce((j->>'from')::bigint, 0), coalesce((j->>'stage')::bigint, 0), coalesce((j->>'coin')::bigint, 0), "
    "coalesce((j->>'xp')::bigint, 0), coalesce((j->>'slot')::bigint, -1), coalesce((j->>'wear')::bigint, 0) "
    "FROM (SELECT $1::jsonb j) s", 1, NULL, bv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-items: %s", PQerrorMessage(db));
    PQclear(r); api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "not kept"); return;
  }
  char *id = item_dup(r, 0, 0), *opw = item_dup(r, 0, 1), *who = item_dup(r, 0, 2), *gen = item_dup(r, 0, 3), *ver = item_dup(r, 0, 4);
  char *item = item_dup(r, 0, 5), *to = item_dup(r, 0, 6), *kind = item_dup(r, 0, 7), *sent = item_dup(r, 0, 8);
  int64_t c_quest = atoll(PQgetvalue(r, 0, 9)), c_from = atoll(PQgetvalue(r, 0, 10)), c_to = atoll(PQgetvalue(r, 0, 11)), c_coin = atoll(PQgetvalue(r, 0, 12));
  int64_t c_xp = atoll(PQgetvalue(r, 0, 13)), c_slot = atoll(PQgetvalue(r, 0, 14)), c_wear = atoll(PQgetvalue(r, 0, 15));
  PQclear(r); r = NULL;
  /* the claim as the ID's record keeps it: part of the payload */
  char rider[160]; snprintf(rider, sizeof rider, "%lld,%lld,%lld,%lld,%lld,%lld,%lld", (long long)c_quest, (long long)c_from, (long long)c_to,
                            (long long)c_coin, (long long)c_xp, (long long)c_slot, (long long)c_wear);
  bool claims = character_claims(c_quest, c_coin, c_xp, c_slot);
  char *owner = NULL, *escrow = NULL, *made = NULL, *result = NULL;
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  int64_t now = (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  char now_s[24]; snprintf(now_s, sizeof now_s, "%lld", (long long)now);
  coax_string id_s = {id ? id : "", id ? (int64_t)strlen(id) : 0};
  coax_string op_s = {opw ? opw : "", opw ? (int64_t)strlen(opw) : 0};
  /* "read": the character as the store has it; nothing changes and nothing is recorded */
  if (item_reads(op_s)) { item_answer(q, db, who, who ? ITEM_OK : item_status((coax_string){"held", 4}), NULL, false); goto done; }
  int64_t op = item_op(op_s);
  if (!progress_run(db, "BEGIN")) goto failed;
  /* one writer an operation ID at a time, and its record */
  const char *iv[7] = {id ? id : "", opw, who, item, to, kind, rider};
  if (!(r = item_q(db, "SELECT pg_advisory_xact_lock(7731029, hashtext($1))", 1, iv, PGRES_TUPLES_OK))) goto failed;
  PQclear(r);
  if (!(r = item_q(db, "SELECT at_ms, op = $2, who = $3, item IS NOT DISTINCT FROM $4::bigint, to_who IS NOT DISTINCT FROM $5, "
                       "kind IS NOT DISTINCT FROM $6::int AND coalesce(rider, '0,0,0,0,0,-1,0') = $7, result, made FROM verse.item_ops WHERE id = $1", 7, iv, PGRES_TUPLES_OK))) goto failed;
  bool seen = PQntuples(r) == 1;
  #define ITEM_T(c) (seen && PQgetvalue(r, 0, c)[0] == 't')
  int64_t g = item_gate(id_s, op, who != NULL, seen, seen ? atoll(PQgetvalue(r, 0, 0)) : 0, ITEM_T(1), ITEM_T(2), ITEM_T(3), ITEM_T(4), ITEM_T(5),
                        item_int(sent, 0), now);
  #undef ITEM_T
  if (seen) { result = strdup(PQgetvalue(r, 0, 6)); made = item_dup(r, 0, 7); }
  PQclear(r); r = NULL;
  if (g != ITEM_GO) {
    if (!progress_run(db, "COMMIT")) goto failed;
    if (g == ITEM_REPLAY) item_answer(q, db, who, item_status((coax_string){result, (int64_t)strlen(result)}), made, true);
    else item_answer(q, db, who, g, NULL, false);
    goto done;
  }
  free(made); made = NULL;
  /* the item, locked; then every inventory it touches, locked in one order */
  bool live = false;
  if (item_names(op) && item) {
    const char *mv[1] = {item};
    if (!(r = item_q(db, "SELECT owner, escrow_to FROM verse.items WHERE id = $1::bigint FOR UPDATE", 1, mv, PGRES_TUPLES_OK))) goto failed;
    live = PQntuples(r) == 1;
    if (live) { owner = item_dup(r, 0, 0); escrow = item_dup(r, 0, 1); }
    PQclear(r); r = NULL;
  }
  const char *wv[3] = {who, to, escrow};
  if (!(r = item_q(db, "INSERT INTO verse.inventories (who) SELECT w FROM unnest(ARRAY[$1, $2, $3]::text[]) w WHERE w IS NOT NULL ON CONFLICT DO NOTHING",
                   3, wv, PGRES_COMMAND_OK))) goto failed;
  PQclear(r);
  if (!(r = item_q(db, "SELECT who, version FROM verse.inventories WHERE who IN ($1, $2, $3) ORDER BY who FOR UPDATE", 3, wv, PGRES_TUPLES_OK))) goto failed;
  int64_t version = 0, to_version = 0, escrow_version = 0;
  for (int i = 0; i < PQntuples(r); i++) {
    const char *w = PQgetvalue(r, i, 0); int64_t v = atoll(PQgetvalue(r, i, 1));
    if (item_same(w, who)) version = v;
    if (item_same(w, to)) to_version = v;
    if (item_same(w, escrow)) escrow_version = v;
  }
  PQclear(r);
  if (!(r = item_q(db, "SELECT gen FROM verse.room_owners WHERE who = $1 FOR SHARE", 1, wv, PGRES_TUPLES_OK))) goto failed;
  int64_t newest = PQntuples(r) == 1 ? atoll(PQgetvalue(r, 0, 0)) : -1;
  PQclear(r);
  if (!(r = item_q(db, "SELECT count(*) FROM verse.items WHERE owner = $1", 1, wv, PGRES_TUPLES_OK))) goto failed;   /* under the inventory's lock */
  int64_t held = atoll(PQgetvalue(r, 0, 0));
  PQclear(r); r = NULL;
  int64_t st = item_rule(op, item_int(gen, -1), newest, item_int(ver, -1), version, live, item_same(owner, who), escrow != NULL,
                         to != NULL, item_same(to, who), to_version, escrow_version);
  st = item_room(op, st, held);   /* a full bag takes no grant */
  if (st == ITEM_OK) {
    /* the rules' effects on the rows, the outbox entry and the character's record, in this transaction;
       a claim the record refuses undoes them (the savepoint) and the answer is held */
    if (!progress_run(db, "SAVEPOINT item_fx")) goto failed;
    if (item_creates(op)) {
      const char *cv[2] = {kind, who};
      if (!(r = item_q(db, "INSERT INTO verse.items (kind, owner) VALUES (coalesce($1::int, 0), $2) RETURNING id", 2, cv, PGRES_TUPLES_OK))) goto failed;
      made = strdup(PQgetvalue(r, 0, 0));
      PQclear(r);
    } else if (item_destroys(op)) {
      const char *dv[1] = {item};
      if (!(r = item_q(db, "DELETE FROM verse.items WHERE id = $1::bigint", 1, dv, PGRES_COMMAND_OK))) goto failed;
      PQclear(r);
    } else if (item_names(op)) {
      const char *after_owner = item_moves(op) ? to : item_settles(op) ? escrow : owner;
      const char *after_escrow = item_escrows(op) ? to : item_clears(op) ? NULL : escrow;
      const char *uv[3] = {item, after_owner, after_escrow};
      if (!(r = item_q(db, "UPDATE verse.items SET owner = $2, escrow_to = $3 WHERE id = $1::bigint", 3, uv, PGRES_COMMAND_OK))) goto failed;
      PQclear(r);
    }
    const char *bump[3] = {who, item_moves(op) ? to : NULL, item_settles(op) ? escrow : NULL};
    for (int i = 0; i < 3; i++) {
      if (!bump[i]) continue;
      if (!(r = item_q(db, "UPDATE verse.inventories SET version = version + 1 WHERE who = $1", 1, &bump[i], PGRES_COMMAND_OK))) goto failed;
      PQclear(r);
    }
    const char *ov[6] = {opw, who, made ? made : item_names(op) ? item : "0", item_settles(op) ? escrow : to, kind, now_s};   /* a claim names no item: 0 */
    if (!(r = item_q(db, "INSERT INTO verse.item_outbox (op, who, item, to_who, kind, at_ms) VALUES ($1, $2, $3::bigint, $4, $5::int, $6::bigint)",
                     6, ov, PGRES_COMMAND_OK))) goto failed;
    PQclear(r);
    /* the record: the ledger's bag and version after the operation, then its claim (character-policy.coax) */
    char kept[4097], next[4097], items[1024];
    if (!(r = item_q(db, "SELECT coalesce((SELECT body::text FROM verse.characters WHERE who=$1 FOR UPDATE),''),"
                         "'[' || coalesce((SELECT string_agg(id::text, ',' ORDER BY id) FROM (SELECT id FROM verse.items WHERE owner=$1 ORDER BY id LIMIT 33) i),'') || ']',"
                         "(SELECT version FROM verse.inventories WHERE who=$1)", 1, wv, PGRES_TUPLES_OK))) goto failed;
    if (PQntuples(r) != 1 || strlen(PQgetvalue(r, 0, 0)) >= sizeof kept || strlen(PQgetvalue(r, 0, 1)) >= sizeof items) goto failed;
    snprintf(kept, sizeof kept, "%s", PQgetvalue(r, 0, 0)); snprintf(items, sizeof items, "%s", PQgetvalue(r, 0, 1));
    int64_t inv = atoll(PQgetvalue(r, 0, 2));
    PQclear(r); r = NULL;
    coax_string rec = character_op((coax_string){kept, (int64_t)strlen(kept)}, (coax_string){who, (int64_t)strlen(who)}, (coax_string){items, (int64_t)strlen(items)},
                                   inv, c_quest, c_from, c_to, c_coin, c_xp, c_slot, c_wear, (coax_v_u8){(uint8_t *)next, (int64_t)sizeof next - 1});
    if (rec.count > 0) {
      next[rec.count] = 0;
      const char *cv[2] = {who, next};
      if (!(r = item_q(db, "INSERT INTO verse.characters AS c (who, body, at_ms) VALUES ($1, $2::jsonb, verse.now_ms()) "
                           "ON CONFLICT (who) DO UPDATE SET body = EXCLUDED.body, at_ms = EXCLUDED.at_ms WHERE c.body IS DISTINCT FROM EXCLUDED.body", 2, cv, PGRES_COMMAND_OK))) goto failed;
      PQclear(r); r = NULL;
      if (!character_outbox(db, who, kept, next)) goto failed;
    } else if (claims || op == item_op((coax_string){"claim", 5})) {
      /* the record refused the claim: nothing of this operation is kept but its refusal */
      if (!progress_run(db, "ROLLBACK TO SAVEPOINT item_fx")) goto failed;
      free(made); made = NULL;
      st = item_status((coax_string){"held", 4});
    }
  }
  /* the answer is recorded, a refusal too, so a retry hears the same; a forgotten record is replaced */
  coax_string w = item_word(st); char ws[16]; snprintf(ws, sizeof ws, "%.*s", (int)w.count, w.data);
  const char *rv[10] = {id, opw, who, item, to, kind, ws, made, now_s, rider};   /* the payload as asked */
  if (!(r = item_q(db, "INSERT INTO verse.item_ops (id, op, who, item, to_who, kind, result, made, at_ms, rider) "
                       "VALUES ($1, $2, $3, $4::bigint, $5, $6::int, $7, $8::bigint, $9::bigint, $10) ON CONFLICT (id) DO UPDATE SET op = EXCLUDED.op, "
                       "who = EXCLUDED.who, item = EXCLUDED.item, to_who = EXCLUDED.to_who, kind = EXCLUDED.kind, result = EXCLUDED.result, "
                       "made = EXCLUDED.made, at_ms = EXCLUDED.at_ms, rider = EXCLUDED.rider", 10, rv, PGRES_COMMAND_OK))) goto failed;
  PQclear(r); r = NULL;
  if (!progress_run(db, "COMMIT")) goto failed;
  item_answer(q, db, who, st, st != ITEM_OK ? NULL : made ? made : item, false);
  goto done;
failed:
  if (r) PQclear(r);
  api_log("room-items: %s", PQerrorMessage(db));
  progress_run(db, "ROLLBACK");
  api_error(q->fd, 500, "not kept");
done:
  free(id); free(opw); free(who); free(gen); free(ver); free(item); free(to); free(kind); free(sent);
  free(owner); free(escrow); free(made); free(result);
}
// character-checkpoint-v1: SDK authentication, locked database facts, then the
// Coax policy's transaction effects. A receipt follows a WAL-flushed commit.
// This receiver is additive: no room producer or ticket restore uses it yet.
static void room_checkpoint(const request *q, PGconn *db, int version) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a checkpoint"); return; }
  if (q->body_len == 0 || q->body_len > 4096) { api_error(q->fd, 401, "unsigned"); return; }
  int signer = room_write_signed(q, "checkpoint", q->body, q->body_len);   /* this shard's write key, or the shared key (#13260) */
  if (signer < 0) return;
  if (room_write_owner(q, db, signer, "owner")) return;   /* the shard that holds the player's seat (0156) */
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  int64_t now = (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  if (!(version == 2 ? checkpoint2_input((coax_string){(const char *)q->body, q->body_len}, now) : room_checkpoint_input((coax_string){(const char *)q->body, q->body_len}, now))) {
    api_error(q->fd, 400, "bad checkpoint"); return;
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
      /* the character record's XP, through its write rule (#13290, character-policy.coax): in this
         transaction, so the record never holds less than an accepted capture */
      r=PQexecParams(db,"SELECT body::text FROM verse.characters WHERE who=$1 FOR UPDATE",1,NULL,wv,NULL,NULL,0);
      if (PQresultStatus(r)!=PGRES_TUPLES_OK) {free(who);goto failed;}
      if (PQntuples(r)==1) {
        char kept_rec[4097], next_rec[4097]; snprintf(kept_rec,sizeof kept_rec,"%s",PQgetvalue(r,0,0));PQclear(r);r=NULL;
        coax_string made=character_checkpoint((coax_string){kept_rec,(int64_t)strlen(kept_rec)},xp,(coax_v_u8){(uint8_t *)next_rec,(int64_t)sizeof next_rec-1});
        if (made.count>0) {
          next_rec[made.count]=0;const char *cv[2]={who,next_rec};
          r=PQexecParams(db,"UPDATE verse.characters SET body=$2::jsonb,at_ms=verse.now_ms() WHERE who=$1",2,NULL,cv,NULL,NULL,0);
          if (PQresultStatus(r)!=PGRES_COMMAND_OK) {free(who);goto failed;}
          if (!character_outbox(db,who,kept_rec,next_rec)) {free(who);goto failed;}
        }
      }
      PQclear(r);r=NULL;
    }
  }
  free(who);
  if (!progress_run(db, "COMMIT")) goto failed;
  coax_string status = room_checkpoint_status(code);
  char out[256];
  /* the receipt is room-report.coax's (Protocol E7, #13250) */
  if (room_report_checkpoint_receipt_c(version,status.data,(size_t)status.count,(long long)owner,(long long)process,(long long)rev,(long long)((code==0||code==1)?accepted:0),out,sizeof out)<0) { api_error(q->fd, 500, "not kept"); return; }
  api_json(q->fd, 200, out); return;
failed:
  if (r) PQclear(r);
  api_log("room-checkpoint: %s", PQerrorMessage(db));
  progress_run(db, "ROLLBACK");
  api_error(q->fd, 500, "not kept");
}
static void room_vitals(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST vitals"); return; }
  if (q->body_len == 0 || q->body_len > 8192) { api_error(q->fd, 401, "unsigned"); return; }
  int signer = room_write_signed(q, "vitals", q->body, q->body_len);   /* this shard's write key, or the shared key (#13260) */
  if (signer < 0) return;
  char *b = malloc(q->body_len + 1);
  if (!b) { api_error(q->fd, 503, "memory"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  long long at = 0;
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  if (sscanf(b, "t %lld", &at) != 1 || at < now - 120000 || at > now + 120000) { free(b); api_error(q->fd, 400, "stale"); return; }
  /* "o SHARD GEN BATCH" (#12793): every line has an operation ID, and the answer is a receipt */
  char *o = strchr(b, '\n');
  int oshard = 0; long long ogen = 0, obatch = 0;
  bool owned = o && sscanf(o + 1, "o %d %lld %lld", &oshard, &ogen, &obatch) == 3;
  /* a shard's own key posts its own vitals only, with their operation IDs (#13260) */
  if (signer > 0 && (!owned || oshard != signer)) { free(b); api_error(q->fd, 403, "another shard's vitals"); return; }
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
  if (!api_header(q, "X-Room-At", at, sizeof at)) { api_error(q->fd, 401, "unsigned"); return; }
  char text[48]; int tn = snprintf(text, sizeof text, "t %s", at);
  if (tn <= 0 || tn >= (int)sizeof text) { api_error(q->fd, 401, "unsigned"); return; }
  /* this shard's write key, or the shared key (#13260); the control answer is the realm's, not a shard's */
  if (room_write_signed(q, "control", text, (size_t)tn) < 0) return;
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000, when = atoll(at);
  if (when < now - 120000 || when > now + 120000) { api_error(q->fd, 401, "bad mac"); return; }
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
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("room-control: %s", PQerrorMessage(db)); PQclear(r); api_error(q->fd, 500, "no control"); return; }
  const char *t = PQgetvalue(r, 0, 0);
  /* the banned go too (#13302): the 64 newest bans' players, each a kick line while moderation-policy.coax
     says the ban holds. A room drops their sockets; the ticket refuses them from then on. */
  size_t kn = strlen(t), cap = kn + 64 * 48 + 16 * 64 + 1;   /* and the flags' lines (#13299) */
  char *out = malloc(cap);
  if (!out) { PQclear(r); api_error(q->fd, 503, "memory"); return; }
  memcpy(out, t, kn); out[kn] = 0;
  PQclear(r);
  r = PQexecParams(db,
    "SELECT p.id, s.until_ms, verse.now_ms() FROM (SELECT who, until_ms FROM verse.mod_sanctions WHERE kind = 'ban' ORDER BY at_ms DESC LIMIT 64) s "
    "JOIN LATERAL (SELECT id FROM verse.players WHERE id = s.who UNION SELECT id FROM verse.players WHERE account = s.who) p ON true LIMIT 64",
    0, NULL, NULL, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("room-control: %s", PQerrorMessage(db)); PQclear(r); free(out); api_error(q->fd, 500, "no control"); return; }
  for (int i = 0; i < PQntuples(r); i++) {
    const char *pid = PQgetvalue(r, i, 0);
    if (strlen(pid) > 40 || !mod_in_force(true, atoll(PQgetvalue(r, i, 1)), atoll(PQgetvalue(r, i, 2)))) continue;
    kn += (size_t)snprintf(out + kn, cap - kn, "kick %s\n", pid);
  }
  PQclear(r);
  /* the flags an admin set with no deploy (#13299, rooms.c flags_lines): "flag NAME VALUE", the whole
     list every time; a room reads one with rm_flag (coax/web/room/room-control.coax) */
  kn = flags_lines(db, out, cap, kn, false);
  api_respond(q->fd, 200, "text/plain", NULL, out, kn);
  free(out);
}
// Moderation (#13302; docs/protocol/moderation-v1.md, migrations/0170_moderation.sql). The rules and the
// record's codec are Coax's (moderation-policy.coax over metaverse-engine/session/moderation.coax); this reads the
// facts and writes what they answer. Every action a moderator takes is one row of verse.mod_log, which
// no one can change, written in the same statement as the ban, mute or kick it sets or lifts.
typedef struct { int verb; long long until; char outcome[12]; char pid[48]; bool logged; } mod_done;
/* One action of moderator `by` (a player id, or "site"): judged, then its log row and what it sets or
   lifts in one statement. source "room" carries the room's code and operation ID (shard, gen, seq), kept
   once: a repeat changes nothing (logged false). False on a database failure. done->outcome: ok, self or
   no_player (each logged); bad when the rules say it is no action, "" when rec is no record (nothing is written). */
static bool room_mod_apply(PGconn *db, coax_string rec, const char *by, const char *source, const char *room, int shard, long long gen, long long seq, mod_done *done) {
  int64_t f[6];
  memset(done, 0, sizeof *done);
  if (!mod_record(rec, 0, (coax_v_s64){f, 6}) || f[3] >= (int64_t)sizeof done->pid || f[5] > 120) return true;
  char reason[128];
  snprintf(done->pid, sizeof done->pid, "%.*s", (int)f[3], rec.data + f[2]);
  snprintf(reason, sizeof reason, "%.*s", (int)f[5], rec.data + f[4]);
  done->verb = (int)f[0];
  const char *pv[1] = {done->pid};
  PGresult *r = PQexecParams(db, "SELECT coalesce(p.account, p.id), EXISTS (SELECT 1 FROM analytics.team t WHERE t.kind = 'player' AND t.value = p.id), verse.now_ms() "
                                 "FROM (SELECT $1::text AS pid) x LEFT JOIN verse.players p ON p.id = x.pid", 1, NULL, pv, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("room-mod: %s", PQerrorMessage(db)); PQclear(r); return false; }
  bool known = !PQgetisnull(r, 0, 0), team = PQgetvalue(r, 0, 1)[0] == 't';
  char who[96]; snprintf(who, sizeof who, "%s", known ? PQgetvalue(r, 0, 0) : done->pid);
  long long now = atoll(PQgetvalue(r, 0, 2));
  PQclear(r);
  int64_t status = mod_judge(f[0], true, !strcmp(by, done->pid), f[1], (coax_string){reason, (int64_t)strlen(reason)});
  if (status != MOD_OK && status != MOD_SELF) { snprintf(done->outcome, sizeof done->outcome, "bad"); return true; }   /* no action: nothing to log */
  const char *outcome = status == MOD_SELF ? "self" : known ? "ok" : "no_player";
  bool apply = !strcmp(outcome, "ok");
  int64_t kind = mod_kind(f[0]);
  bool lifts = mod_lifts(f[0]);
  done->until = apply ? (long long)mod_until(f[0], f[1], now) : 0;
  coax_string vw = mod_verb_word(f[0]);
  char verb[12], at_s[24], min_s[12], until_s[24], shard_s[12], gen_s[24], seq_s[24];
  snprintf(verb, sizeof verb, "%.*s", (int)vw.count, vw.data);
  snprintf(at_s, sizeof at_s, "%lld", now); snprintf(min_s, sizeof min_s, "%lld", (long long)f[1]); snprintf(until_s, sizeof until_s, "%lld", done->until);
  snprintf(shard_s, sizeof shard_s, "%d", shard); snprintf(gen_s, sizeof gen_s, "%lld", gen); snprintf(seq_s, sizeof seq_s, "%lld", seq);
  bool op = !strcmp(source, "room");
  const char *v[18] = {at_s, by, verb, done->pid, who, min_s, until_s, reason, source, op ? room : NULL, op ? shard_s : NULL, op ? gen_s : NULL, op ? seq_s : NULL,
                       outcome, team ? "t" : "f", kind == MOD_KIND_BAN ? "ban" : kind == MOD_KIND_MUTE ? "mute" : kind == MOD_KIND_KICK ? "kick" : "",
                       apply && kind && !lifts ? "t" : "f", apply && kind && lifts ? "t" : "f"};
  r = PQexecParams(db,
    "WITH log AS (INSERT INTO verse.mod_log (at_ms, by_who, verb, whom, who, minutes, until_ms, reason, source, room, shard, gen, seq, outcome, team) "
    "  VALUES ($1::bigint, $2::text, $3::text, $4::text, $5::text, $6::int, $7::bigint, $8::text, $9::text, $10::text, $11::int, $12::bigint, $13::bigint, $14::text, $15::boolean) "
    "  ON CONFLICT ON CONSTRAINT mod_log_op DO NOTHING RETURNING n), "
    "sanction AS (INSERT INTO verse.mod_sanctions (who, kind, until_ms, reason, by_who, at_ms) "
    "  SELECT $5::text, $16::text, $7::bigint, $8::text, $2::text, $1::bigint FROM log WHERE $17::boolean AND $16::text IN ('ban', 'mute') "
    "  ON CONFLICT (who, kind) DO UPDATE SET until_ms = EXCLUDED.until_ms, reason = EXCLUDED.reason, by_who = EXCLUDED.by_who, at_ms = EXCLUDED.at_ms RETURNING 1), "
    "lifted AS (DELETE FROM verse.mod_sanctions WHERE $18::boolean AND who = $5::text AND kind = $16::text AND EXISTS (SELECT 1 FROM log) RETURNING 1), "
    "kick AS (INSERT INTO verse.room_kicks (player, by_who, until_ms, at_ms) SELECT $4::text, $2::text, $7::bigint, $1::bigint FROM log WHERE $17::boolean AND $16::text = 'kick' "
    "  ON CONFLICT (player) DO UPDATE SET by_who = EXCLUDED.by_who, until_ms = EXCLUDED.until_ms, at_ms = EXCLUDED.at_ms RETURNING 1), "
    "unkick AS (DELETE FROM verse.room_kicks WHERE $18::boolean AND $16::text = 'kick' AND player = $4::text AND EXISTS (SELECT 1 FROM log) RETURNING 1) "
    "SELECT (SELECT count(*) FROM log), (SELECT count(*) FROM sanction) + (SELECT count(*) FROM kick) + (SELECT count(*) FROM lifted) + (SELECT count(*) FROM unkick)",
    18, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("room-mod: %s", PQerrorMessage(db)); PQclear(r); return false; }
  done->logged = atoi(PQgetvalue(r, 0, 0)) > 0;
  PQclear(r);
  snprintf(done->outcome, sizeof done->outcome, "%s", outcome);
  return true;
}
/* A moderator's action over the API, the record already written by the codec (rec, n bytes; n < 0: not
   one): the caller must be one of Everglade's admins. True when done (out is filled); else answered. */
static bool room_mod_asked(const request *q, PGconn *db, const char *key, bool site, const char *rec, int64_t n, mod_done *done) {
  const char *v[2] = {key ? key : "", site ? "t" : "f"};
  PGresult *r = PQexecParams(db, "SELECT verse.edit_may('everglade', $1, $2::boolean), coalesce(verse.player_of($1), 'site')", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("room-mod: %s", PQerrorMessage(db)); PQclear(r); api_error(q->fd, 500, "not kept"); return false; }
  bool may = PQgetvalue(r, 0, 0)[0] == 't';
  char by[64]; snprintf(by, sizeof by, "%s", PQgetvalue(r, 0, 1));
  PQclear(r);
  if (!may) { api_error(q->fd, 403, "Everglade's admins only"); return false; }
  if (n < 0) { api_error(q->fd, 400, "not a record"); return false; }
  if (!room_mod_apply(db, (coax_string){rec, n}, by, "api", NULL, 0, 0, 0, done)) { api_error(q->fd, 500, "not kept"); return false; }
  if (!strcmp(done->outcome, "no_player")) { api_error(q->fd, 404, "no such player"); return false; }
  if (!strcmp(done->outcome, "self")) { api_error(q->fd, 403, "not on yourself"); return false; }
  if (strcmp(done->outcome, "ok")) { api_error(q->fd, 400, "not a record"); return false; }
  return true;
}
// An Everglade admin's kick (#12714): {"player": "p-...", "on": false to let them back}. 24 hours, every
// room. The same action as "kick PID 0 REASON" to /room-mod, logged alike.
static void room_kick(const request *q, PGconn *db, const char *key, bool site, const char *body) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a kick"); return; }
  const char *v[1] = {body};
  PGresult *r = PQexecParams(db, "SELECT coalesce(j->>'player', ''), j->'on' = 'false'::jsonb FROM (SELECT $1::jsonb AS j) s", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room-kick: %s", PQerrorMessage(db));
    PQclear(r); api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "no kick"); return;
  }
  char pid[64]; snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0));
  bool back = !PQgetisnull(r, 0, 1) && PQgetvalue(r, 0, 1)[0] == 't';
  PQclear(r);
  char rec[272];
  const char *why = back ? "-" : "an admin's kick";
  int64_t n = mod_record_write((coax_v_u8){(uint8_t *)rec, sizeof rec}, back ? MOD_UNKICK : MOD_KICK, (coax_string){pid, (int64_t)strlen(pid)}, 0, (coax_string){why, (int64_t)strlen(why)});
  mod_done done;
  if (n < 0) {   /* no player has such an id: as before, an admin is told so and anyone else that it is the admins' */
    const char *av[2] = {key ? key : "", site ? "t" : "f"};
    r = PQexecParams(db, "SELECT verse.edit_may('everglade', $1, $2::boolean)", 2, NULL, av, NULL, NULL, 0);
    bool may = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && PQgetvalue(r, 0, 0)[0] == 't';
    PQclear(r);
    api_error(q->fd, may ? 404 : 403, may ? "no such player" : "Everglade's admins only");
    return;
  }
  if (!room_mod_asked(q, db, key, site, rec, n, &done)) return;
  api_json(q->fd, 200, back ? "{\"kicked\":false}" : "{\"kicked\":true,\"hours\":24}");
}
// POST /api/playtest/room-mod: one of Everglade's admins acts on a player. The body is the record's line,
// "VERB PID MINUTES REASON" (kick, mute, ban, unkick, unmute, unban; look and move are a room's). The
// answer names what was done and when it ends: {"ok":true,"verb":V,"player":PID,"until":MS} (0: no end).
// 403 not an admin, or on yourself; 404 no such player; 400 not a record.
// GET /api/playtest/room-mod[?player=PID]: the log for one of Everglade's admins, the newest 200 actions
// (about that player), oldest first: {"log":[{n, at_ms, by_who, verb, whom, minutes, until_ms, reason,
// source, room, shard, gen, seq, outcome, team}]}. 403 for anyone else.
static void room_mod(const request *q, PGconn *db, const char *key, bool site, const char *body) {
  if (!strcmp(q->method, "GET")) {
    char whom[64] = "";
    if (!api_query(q, "player", whom, sizeof whom)) whom[0] = 0;
    const char *v[3] = {key ? key : "", site ? "t" : "f", whom};
    PGresult *r = PQexecParams(db,
      "SELECT verse.edit_may('everglade', $1, $2::boolean), '{\"log\":' || coalesce((SELECT json_agg(row_to_json(x) ORDER BY x.n) FROM "
      "(SELECT n, at_ms, by_who, verb, whom, minutes, until_ms, reason, source, room, shard, gen, seq, outcome, team FROM verse.mod_log "
      " WHERE ($3::text = '' OR whom = $3::text) ORDER BY n DESC LIMIT 200) x), '[]')::text || '}'", 3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("room-mod: %s", PQerrorMessage(db)); PQclear(r); api_error(q->fd, 500, "not read"); return; }
    if (PQgetvalue(r, 0, 0)[0] != 't') { PQclear(r); api_error(q->fd, 403, "Everglade's admins only"); return; }
    api_json(q->fd, 200, PQgetvalue(r, 0, 1));
    PQclear(r);
    return;
  }
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a record"); return; }
  int64_t n = (int64_t)strlen(body);
  while (n > 0 && (body[n - 1] == '\n' || body[n - 1] == '\r')) n--;
  int64_t f[6];
  if (!mod_record((coax_string){body, n}, 0, (coax_v_s64){f, 6}) || !mod_kind(f[0])) n = -1;
  mod_done done;
  if (!room_mod_asked(q, db, key, site, body, n, &done)) return;
  coax_string vw = mod_verb_word(done.verb);
  char out[192];
  snprintf(out, sizeof out, "{\"ok\":true,\"verb\":\"%.*s\",\"player\":\"%s\",\"until\":%lld}", (int)vw.count, vw.data, done.pid, done.until);
  api_json(q->fd, 200, out);
}
// POST /api/playtest/room-mod-log: a room's report of what its moderators did (room-mod.coax; a seat
// whose signed ticket has the moderator bit), signed with the shard's write key for the purpose
// "moderation" (or the shared key), within two minutes of now:
//   t MS\no SHARD GEN BATCH\nSEQ BY ROOM RECORD\n...
// Each line is logged once by (SHARD, GEN, SEQ) and applied with its log row. The answer, text/plain:
// "receipt GEN BATCH". A database failure is a 500 with no receipt: the room sends the post again.
static void room_mod_log(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a report"); return; }
  if (q->body_len == 0 || q->body_len > 8192) { api_error(q->fd, 401, "unsigned"); return; }
  int signer = room_write_signed(q, "moderation", q->body, q->body_len);
  if (signer < 0) return;
  int64_t h[5];
  coax_string body = {q->body, (int64_t)q->body_len};
  if (!mod_head(body, (coax_v_s64){h, 5})) { api_error(q->fd, 400, "not a report"); return; }
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  long long now = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  if (h[0] < now - 120000 || h[0] > now + 120000) { api_error(q->fd, 400, "stale"); return; }
  if (signer > 0 && h[1] != signer) { api_error(q->fd, 403, "another shard's report"); return; }
  for (int64_t at = h[4]; at < body.count; ) {
    int64_t e = at;
    while (e < body.count && body.data[e] != '\n') e++;
    coax_string line = {body.data + at, e - at};
    int64_t f[6];
    at = e + 1;
    if (!mod_entry(line, (coax_v_s64){f, 6}) || f[2] > 40 || f[4] > 24) { api_log("room-mod-log: a line that is no action"); continue; }
    char by[48], room[32];
    snprintf(by, sizeof by, "%.*s", (int)f[2], line.data + f[1]);
    snprintf(room, sizeof room, "%.*s", (int)f[4], line.data + f[3]);
    mod_done done;
    if (!room_mod_apply(db, (coax_string){line.data + f[5], line.count - f[5]}, by, "room", room, (int)h[1], (long long)h[2], (long long)f[0], &done)) {
      api_error(q->fd, 500, "not kept"); return;
    }
  }
  char out[64];
  int64_t n = mod_receipt((coax_v_u8){(uint8_t *)out, sizeof out}, h[2], h[3]);
  if (n < 0) { api_error(q->fd, 500, "not kept"); return; }
  api_respond(q->fd, 200, "text/plain", NULL, out, (size_t)n);
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
    PQclear(r); api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "no event"); return;
  }
  if (PQntuples(r) != 1) { PQclear(r); api_error(q->fd, 404, "no event"); return; }
  bool may = PQgetvalue(r, 0, 0)[0] == 't';
  if (post && !may) { PQclear(r); api_error(q->fd, 403, "Everglade's admins only"); return; }
  api_json(q->fd, 200, PQgetvalue(r, 0, 1));
  PQclear(r);
}

bool player_route(const request *q) {
  const char *rest = legacy(q->path);
  if (!rest && (strncmp(q->path, PREFIX, strlen(PREFIX)) || !strcmp(q->path, "/api/playtest/health"))) return false;
  if (!rest) rest = q->path + strlen(PREFIX);
  if (strcmp(q->method, "GET") && strcmp(q->method, "POST")) { api_error(q->fd, 405, "GET or POST"); return true; }
  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "no database"); return true; }

  // The path after the prefix, as a Postgres text[] literal: segments are
  // [A-Za-z0-9_-] only (anything else is not a route).
  char parts[600]; size_t pn = 0; int segs = 0;
  parts[pn++] = '{';
  for (const char *p = rest; *p; p++) {
    char ch = *p;
    if (ch == '/') { if (p[1] == 0) break; if (pn + 2 >= sizeof parts) break; parts[pn++] = ','; continue; }
    bool ok = (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '-';
    if (!ok || pn + 2 >= sizeof parts) { api_error(q->fd, 404, "unknown"); return true; }
    if (p == rest || p[-1] == '/') segs++;
    parts[pn++] = ch;
  }
  parts[pn++] = '}'; parts[pn] = 0;
  if (!segs || segs > 4) { api_error(q->fd, 404, "unknown"); return true; }
  if (!strncmp(rest, "survey", 6) && (rest[6] == 0 || rest[6] == '/')) { feedback(q, db, parts); return true; }
  // oa-room's HP report (#12715): signed with the seat-ticket key, not a player's
  if (!strcmp(rest, "room-vitals")) { room_vitals(q, db); return true; }
  if (!strcmp(rest, "room-items")) { room_items(q, db); return true; }   // the item ledger (#12849)
  if (!strcmp(rest, "room-checkpoint-v2")) { room_checkpoint(q,db,2); return true; }
  if (!strcmp(rest, "room-checkpoint")) { room_checkpoint(q, db, 1); return true; } // character-checkpoint-v1 (#12838)
  if (!strcmp(rest, "room-control")) { room_control(q, db); return true; }
  if (!strcmp(rest, "room-mod-log")) { room_mod_log(q, db); return true; }   // a room's moderator actions (#13302)
  // the Everglade instance directory (rooms.c, #12717): a shard's report, and the instances' list
  api_route_at("rooms_route");
  if (rooms_route(q, db, rest)) return true;
  api_route_at("player_route");

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
      char out[256]; snprintf(out, sizeof out, "nip98: %s", err);
      api_error(q->fd, 401, out); return true;
    }
    if (!first_use(db, &ev)) { api_error(q->fd, 401, "nip98: that signature was used already"); return true; }
    memcpy(npub, ev.pubkey, sizeof npub);
    site = admin_is(npub);
  } else if (site_session(q, db)) site = true;
  // A hand-built realm's owner or one of its admins (verse.realm_rights, docs/metaverse/realm-edit.md)
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
  if (!body) { api_error(q->fd, 503, "memory"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; }
  else strcpy(body, "{}");
  if (!strncmp(rest, "room-ticket/", 12) && segs == 2) { room_ticket(q, db, key, site, rest + 12, body); free(body); return true; }
  if (!strncmp(rest, "room-invite/", 12) && segs == 2) { room_invite(q, db, key, rest + 12); free(body); return true; }
  if (!strcmp(rest, "room-state")) { room_state(q, db, key, body); free(body); return true; }
  if (!strcmp(rest, "party") || !strncmp(rest, "party/", 6)) { party(q, db, key, rest[5] == '/' ? rest + 6 : "", body); free(body); return true; }
  if (!strcmp(rest, "player/name")) {   // a guest's own name, through the same filter (migrations/0116)
    if (strcmp(q->method, "POST")) { free(body); api_error(q->fd, 405, "POST a name"); return true; }
    const char *nv[2] = {key, body};
    PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.player_set_name($1, $2::jsonb) x", 2, NULL, nv, NULL, NULL, 0);
    free(body);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
      const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      bool theirs = state && !strncmp(state, "22", 2);
      if (!theirs) api_log("player/name: %s", PQerrorMessage(db));
      PQclear(r); api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "no name"); return true;
    }
    api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
    PQclear(r);
    return true;
  }
  if (!strcmp(rest, "room-kick")) { room_kick(q, db, key, site, body); free(body); return true; }
  if (!strcmp(rest, "room-mod")) { room_mod(q, db, key, site, body); free(body); return true; }
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
      api_error(q->fd, 400, "bytes must be base64 and at a time");
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
    api_error(q->fd, theirs ? 400 : 500, theirs ? "not json" : "not kept");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  api_json(q->fd, status, PQgetvalue(r, 0, 1));
  PQclear(r);
  return true;
}
