// The Everglade instance directory (#12717; spec sections 3.4 and 9): which oa-room shard holds each
// instance, who sits in it, and where a new one goes. The state and every decision are Postgres's
// (migrations/0115_room_directory.sql: verse.room_pick, verse.room_report, verse.room_list); this file
// moves the facts in and the answers out.
//   POST /api/playtest/room-directory     a shard's report (coax/web/room/room-directory.coax), signed
//                                         with the shard's write key or the seat-ticket key (room_write_signed, #13260)
//   POST /api/playtest/room-crash         a shard's report of its last crash (coax/web/room/room-crash.coax,
//                                         #13298), signed as its `crash` write; kept by bucket (service.c crash_keep)
//   GET  /api/playtest/crashes            the crash buckets heard in 30 days, each with its newest report
//   POST /api/playtest/room-stats         a shard's vitals every 10 s (coax/web/room/room-stats.h), signed
//                                         alike, for the admins' room view (GET /api/admin/rooms, admin.c)
//   GET  /api/playtest/room-shards        the shards: live, draining or down, each one's endpoint and
//                                         health (#13179, migrations/0140_room_shard_endpoints.sql)
//   GET  /api/playtest/rooms/REALM        its public instances and which to join; ?with=PLAYER_ID also
//                                         names the instance that player sits in ("with_full" when it
//                                         is full and no invite of theirs holds a seat, #12795)
// player.c room_ticket asks rooms_pick for the instance and shard when the page names its realm.
#include "api.h"
#include "progress-policy.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/hmac.h>
#include <openssl/evp.h>


static bool code_ok(const char *s, size_t most) {
  size_t n = strlen(s);
  if (!n || n > most) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9') || s[i] == '-')) return false;
  return true;
}

// The instance for player pid in realm (asked: the instance the page names, or the realm itself to be
// given one): its code and shard, and whether the asked one was full. via: the player whose invite
// brought them ("" none), whose held seat they take (migrations/0125_room_invites.sql, #12795). build:
// the page's build "ENGINE.RULES.CONTENT" ("" unknown), placed on a shard that runs it
// (migrations/0128_room_builds.sql, #12839). False when there is no seat.
static bool build_ok(const char *s) {
  int dots = 0; size_t n = strlen(s);
  if (!n || n > 32) return false;
  for (size_t i = 0; i < n; i++) {
    if (s[i] == '.') { if (!i || s[i - 1] == '.') return false; dots++; }
    else if (s[i] < '0' || s[i] > '9') return false;
  }
  return dots == 2 && s[n - 1] != '.';
}
// A completed database-request observation. Failure to persist this metric is
// logged separately and cannot change the placement result already obtained.
static void reservation_observed(PGconn *db,int64_t event) {
  coax_string label=rsv_observe_name(event);char name[64];
  if(label.count<=0||label.count>=(int64_t)sizeof name)return;
  memcpy(name,label.data,(size_t)label.count);name[label.count]=0;
  if(PQtransactionStatus(db)!=PQTRANS_IDLE){api_log("room reservation observation lost: caller transaction active");return;}
  char begin[96];snprintf(begin,sizeof begin,"BEGIN; SET LOCAL statement_timeout='%lldms'",(long long)rsv_observe_store_ms());
  PGresult *r=PQexec(db,begin);
  bool started=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);
  if(!started){
    api_log("room reservation observation lost: %s",PQerrorMessage(db));
    if(PQtransactionStatus(db)==PQTRANS_INTRANS||PQtransactionStatus(db)==PQTRANS_INERROR){r=PQexec(db,"ROLLBACK");PQclear(r);}
    return;
  }
  const char *values[1]={name};
  r=PQexecParams(db,"SELECT verse.room_admit_count($1,1)",1,NULL,values,NULL,NULL,0);
  bool kept=PQresultStatus(r)==PGRES_TUPLES_OK;
  if(!kept)api_log("room reservation observation lost: %s",PQerrorMessage(db));
  PQclear(r);
  r=PQexec(db,kept?"COMMIT":"ROLLBACK");
  if(PQresultStatus(r)!=PGRES_COMMAND_OK)api_log("room reservation observation lost: commit failed");
  PQclear(r);
}
bool rooms_pick(PGconn *db, const char *realm, const char *asked, const char *pid, const char *via, const char *build, char *code, size_t cap, int *shard, bool *full) {
  if (!code_ok(realm, 20) || !code_ok(asked, 24)) return false;
  if (via && via[0] && !code_ok(via, 40)) via = "";
  if (!build || !build_ok(build)) build = "";
  char seats[8]; snprintf(seats, sizeof seats, "%d", ROOM_SEATS);
  const char *v[6] = {realm, asked, pid, seats, via ? via : "", build};
  PGresult *r = PQexecParams(db, "SELECT code, shard, was_full FROM verse.room_pick($1, $2, $3, $4::int, $5, $6)", 6, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) && strlen(PQgetvalue(r, 0, 0)) < cap;
  bool shaped=PQresultStatus(r)==PGRES_TUPLES_OK&&PQntuples(r)==1;
  bool named=shaped&&!PQgetisnull(r,0,0);
  int64_t event=rsv_observe_pick(PQresultStatus(r)==PGRES_TUPLES_OK,PQntuples(r),named,named?(int64_t)strlen(PQgetvalue(r,0,0)):0,(int64_t)cap,shaped&&PQgetvalue(r,0,2)[0]=='t');
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("rooms: %s", PQerrorMessage(db));
  if (ok) {
    snprintf(code, cap, "%s", PQgetvalue(r, 0, 0));
    *shard = atoi(PQgetvalue(r, 0, 1));
    *full = PQgetvalue(r, 0, 2)[0] == 't';
  }
  PQclear(r);
  reservation_observed(db,event);
  return ok;
}

// An INVITE's held seat in room for its friends (#12795), by player pid, who sits in it: 0 and its
// end in until (wall-clock ms), 1 when pid isn't in the room, 2 when it is full, -1 on an error.
int rooms_invite(PGconn *db, const char *room, const char *pid, long long *until) {
  if (!code_ok(room, 24) || !code_ok(pid, 40)) return 1;
  char seats[8]; snprintf(seats, sizeof seats, "%d", ROOM_SEATS);
  const char *v[3] = {room, pid, seats};
  PGresult *r = PQexecParams(db, "SELECT until_ms, why FROM verse.room_invite($1, $2, $3::int)", 3, NULL, v, NULL, NULL, 0);
  int out = -1;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    if (!PQgetisnull(r, 0, 0)) { *until = atoll(PQgetvalue(r, 0, 0)); out = 0; }
    else out = !strcmp(PQgetvalue(r, 0, 1), "full") ? 2 : 1;
  } else api_log("room-invite: %s", PQerrorMessage(db));
  PQclear(r);
  reservation_observed(db,rsv_observe_invite(out));
  return out;
}

int stk_room_write_c(const char *masters, const char *signer, const char *purpose, const char *body, size_t n, const char *mac);
/* A room shard's signed write (#13260; coax/web/room/room-write.coax, docs/deployment/room-keys.md),
   for player.c's routes too. With X-Room-Signer ("SHARD KID") the MAC must be that shard's own key's,
   made from OA_ROOM_WRITE_MASTER (one master, or two while one rotates), over the purpose, the shard and
   body: the answer is the shard (1 to 99), and the route then refuses a body that names another shard.
   Without it the MAC is the shared OA_ROOM_TICKET_KEY's over body, as before #13260, while that key is
   set: the answer is 0 (no shard bound). Refused, it is answered and the answer is -1. */
int room_write_signed(const request *q, const char *purpose, const char *body, size_t n) {
  char given[80], signer[24];
  if (!api_header(q, "X-Room-Mac", given, sizeof given) || strlen(given) != 64) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return -1; }
  const char *masters = getenv("OA_ROOM_WRITE_MASTER");
  if (api_header(q, "X-Room-Signer", signer, sizeof signer)) {
    if (!masters || !*masters) { api_json(q->fd, 503, "{\"error\":\"no write key\"}"); return -1; }
    int shard = stk_room_write_c(masters, signer, purpose, body, n, given);
    if (shard < 1) { api_json(q->fd, 401, "{\"error\":\"bad mac\"}"); return -1; }
    return shard;
  }
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  if (!secret || strlen(secret) < 32) {
    if (masters && *masters) api_json(q->fd, 401, "{\"error\":\"a shard's signature\"}");
    else api_json(q->fd, 503, "{\"error\":\"no ticket key\"}");
    return -1;
  }
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)body, n, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no mac\"}"); return -1; }
  char want[65]; for (unsigned i = 0; i < 32; i++) snprintf(want + 2 * i, 3, "%02x", mac[i]);
  unsigned diff = 0; for (int i = 0; i < 64; i++) diff |= (unsigned)(want[i] ^ given[i]);
  if (diff) { api_json(q->fd, 401, "{\"error\":\"bad mac\"}"); return -1; }
  return 0;
}
/* Whether every line of b that starts with key names shard (key's number right after it), and one does. */
static bool lines_name_shard(const char *b, const char *key, int shard) {
  size_t kn = strlen(key); int seen = 0;
  for (const char *line = b; line && *line; ) {
    if (!strncmp(line, key, kn)) { if (atoi(line + kn) != shard) return false; seen++; }
    line = strchr(line, '\n'); if (line) line++;
  }
  return seen > 0;
}
// A shard's signed POST of purpose: the signer's shard (room_write_signed), or -1 answered.
static int signed_post(const request *q, const char *purpose) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a report\"}"); return -1; }
  if (q->body_len == 0 || q->body_len > 32768) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return -1; }
  return room_write_signed(q, purpose, q->body, q->body_len);
}
// The endpoint and health lines a VM shard adds (#13179, migrations/0140_room_shard_endpoints.sql),
// taken out of body (in place) so room_report reads the report it always has:
//   e wss://HOST/api/room/sK/      h PLAYERS TICK_P95_US
// ep gets the endpoint ("" none, or one that isn't wss:// on an openagents.com name), *players and
// *tick_us the health (-1 none). Returns the shard the s line names, 0 when there is none.
static int shard_lines(char *body, char *ep, size_t cap, int *players, int *tick_us) {
  ep[0] = 0; *players = -1; *tick_us = -1;
  int shard = 0; char *w = body;
  for (char *line = body; *line;) {
    char *nl = strchr(line, '\n'); size_t n = nl ? (size_t)(nl - line) + 1 : strlen(line);
    if (line[0] == 'e' && line[1] == ' ') {
      size_t m = n - 2 - (nl ? 1 : 0);
      const char *host = line + 2 + 6;
      bool ok = m < cap && m > 6 && !strncmp(line + 2, "wss://", 6);
      if (ok) {   /* wss://NAME.openagents.com[:PORT]/api/room/sK/ and nothing else */
        const char *slash = memchr(host, '/', m - 6);
        size_t hn = slash ? (size_t)(slash - host) : 0;
        const char *colon = hn ? memchr(host, ':', hn) : NULL; if (colon) hn = (size_t)(colon - host);
        ok = slash && hn > 15 && !memcmp(host + hn - 15, ".openagents.com", 15) && !strncmp(slash, "/api/room/s", 11) && line[2 + m - 1] == '/';
        for (size_t i = 0; ok && i < m; i++) { char c = line[2 + i]; ok = c > ' ' && c < 127 && c != '"' && c != '\\'; }
      }
      if (ok) { memcpy(ep, line + 2, m); ep[m] = 0; }
    } else if (line[0] == 'h' && line[1] == ' ') {
      if (sscanf(line + 2, "%d %d", players, tick_us) != 2) { *players = -1; *tick_us = -1; }
    } else {
      if (line[0] == 's' && line[1] == ' ') shard = atoi(line + 2);
      memmove(w, line, n); w += n;
    }
    line += n;
  }
  *w = 0;
  return shard;
}

static void report(const request *q, PGconn *db) {
  int signer = signed_post(q, "directory");
  if (signer < 0) return;
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  /* a shard's own key reports for that shard only (#13260) */
  if (signer > 0 && (strlen(b) != q->body_len || !lines_name_shard(b, "s ", signer))) { free(b); api_json(q->fd, 403, "{\"error\":\"another shard's report\"}"); return; }
  char ep[200]; int players, tick_us;
  int shard = shard_lines(b, ep, sizeof ep, &players, &tick_us);
  const char *v[1] = {b};
  PGresult *r = PQexecParams(db, "SELECT verse.room_report($1)", 1, NULL, v, NULL, NULL, 0);
  free(b);
  int named = -1;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) named = atoi(PQgetvalue(r, 0, 0));
  else api_log("room-directory: %s", PQerrorMessage(db));
  PQclear(r);
  if (named == -2) { api_json(q->fd, 409, "{\"error\":\"a newer process has this shard\"}"); return; }   /* fenced (#12797) */
  if (named < 0) { api_json(q->fd, 400, "{\"error\":\"not a report\"}"); return; }
  if (shard >= 1 && shard <= 99) {   /* its endpoint and health, or none (#13179) */
    char ks[8], ps[16], ts[16]; snprintf(ks, sizeof ks, "%d", shard); snprintf(ps, sizeof ps, "%d", players); snprintf(ts, sizeof ts, "%d", tick_us);
    const char *sv[4] = {ks, ep[0] ? ep : NULL, players >= 0 ? ps : NULL, tick_us >= 0 ? ts : NULL};
    r = PQexecParams(db, "SELECT verse.room_shard_seen($1::int, $2, $3::int, $4::int)", 4, NULL, sv, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("room-directory: %s", PQerrorMessage(db));
    PQclear(r);
  }
  char out[48]; snprintf(out, sizeof out, "{\"rooms\":%d}", named);
  api_json(q->fd, 200, out);
}

// The endpoint a page opens for shard k (#13179): a live VM shard's wss:// URL, into out; false when
// the shard names none (the page opens the load balancer's /api/room/sK/).
bool rooms_endpoint(PGconn *db, int shard, char *out, size_t cap) {
  out[0] = 0;
  char ks[8]; snprintf(ks, sizeof ks, "%d", shard);
  const char *v[1] = {ks};
  PGresult *r = PQexecParams(db, "SELECT verse.room_endpoint($1::int)", 1, NULL, v, NULL, NULL, 0);
  bool got = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) && strlen(PQgetvalue(r, 0, 0)) < cap;
  if (got) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  return got;
}

// GET /api/playtest/room-shards: the directory's shards, live, draining or down, with their endpoints
// and health (#13179).
static void shard_list(const request *q, PGconn *db) {
  if (strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET the shards\"}"); return; }
  PGresult *r = PQexec(db, "SELECT verse.room_shard_list()");
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("room-shards: %s", PQerrorMessage(db)); PQclear(r);
    api_json(q->fd, 500, "{\"error\":\"no shards\"}"); return;
  }
  api_json(q->fd, 200, PQgetvalue(r, 0, 0));
  PQclear(r);
}

static void list(const request *q, PGconn *db, const char *realm) {
  if (strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET the rooms\"}"); return; }
  char lower[32]; size_t n = strlen(realm);
  if (n == 0 || n >= sizeof lower) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  for (size_t i = 0; i <= n; i++) lower[i] = (char)(realm[i] >= 'A' && realm[i] <= 'Z' ? realm[i] + 32 : realm[i]);
  if (!code_ok(lower, 20)) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  char with[48] = "";
  bool asked = api_query(q, "with", with, sizeof with) && with[0];
  if (asked && !code_ok(with, 40)) { api_json(q->fd, 400, "{\"error\":\"with: a player id\"}"); return; }
  char seats[8]; snprintf(seats, sizeof seats, "%d", ROOM_SEATS);
  const char *v[3] = {lower, seats, asked ? with : NULL};
  PGresult *r = PQexecParams(db, "SELECT verse.room_list($1, $2::int, $3)", 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("rooms: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, 500, "{\"error\":\"no rooms\"}");
    return;
  }
  api_json(q->fd, 200, PQgetvalue(r, 0, 0));
  PQclear(r);
}

// A shard's vitals for the admins' room view (#12791, migrations/0123_room_stats.sql), kept a day.
static void stats(const request *q, PGconn *db) {
  int signer = signed_post(q, "stats");
  if (signer < 0) return;
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  if (signer > 0 && (strlen(b) != q->body_len || !lines_name_shard(b, "shard=", signer))) { free(b); api_json(q->fd, 403, "{\"error\":\"another shard's report\"}"); return; }
  const char *v[1] = {b};
  PGresult *r = PQexecParams(db, "SELECT verse.room_stats_add($1)", 1, NULL, v, NULL, NULL, 0);
  free(b);
  bool kept = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("room-stats: %s", PQerrorMessage(db));
  PQclear(r);
  if (!kept) { api_json(q->fd, 400, "{\"error\":\"not a report\"}"); return; }
  api_json(q->fd, 200, "{\"kept\":true}");
}

// A shard's crash report (#13298, docs/deployment/crash-reports.md): crash report v1, checked by its
// codec and kept by bucket; the minute's service watch raises its alert.
int crash_report_check_c(const char *body, size_t n);
static void crash(const request *q, PGconn *db) {
  int signer = signed_post(q, "crash");
  if (signer < 0) return;
  int shard = crash_report_check_c(q->body, q->body_len);
  if (shard < 1) { api_json(q->fd, 400, "{\"error\":\"not a room's crash report\"}"); return; }
  if (signer > 0 && shard != signer) { api_json(q->fd, 403, "{\"error\":\"another shard's report\"}"); return; }
  char bucket[33], out[96];
  if (!crash_keep(db, q->body, q->body_len, false, bucket)) { api_json(q->fd, 500, "{\"error\":\"not kept\"}"); return; }
  snprintf(out, sizeof out, "{\"kept\":true,\"bucket\":\"%s\"}", bucket);
  api_json(q->fd, 200, out);
}

// GET /api/playtest/crashes: the crash buckets heard in 30 days and each one's newest report, for anyone
// (verse.crash_list builds the list; the reports are the codec's text as kept).
static void crashes(const request *q, PGconn *db) {
  if (strcmp(q->method, "GET") && strcmp(q->method, "HEAD")) { api_json(q->fd, 405, "{\"error\":\"GET the crashes\"}"); return; }
  char ns[24]; snprintf(ns, sizeof ns, "%lld", (long long)api_now_ms());
  const char *v[1] = {ns};
  PGresult *r = PQexecParams(db, "SELECT verse.crash_list($1::bigint)", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("crashes: %s", PQerrorMessage(db)); PQclear(r); api_json(q->fd, 500, "{\"error\":\"no crashes\"}"); return; }
  const char *o = getenv("OA_API_ORIGIN");
  size_t n = strlen(PQgetvalue(r, 0, 0)) + 96;
  char *out = malloc(n);
  if (out) {
    snprintf(out, n, "{\"env\":\"%s\",\"now_ms\":%s,\"buckets\":%s}", o && strstr(o, "staging") ? "staging" : "production", ns, PQgetvalue(r, 0, 0));
    api_json(q->fd, 200, out); free(out);
  } else api_json(q->fd, 503, "{\"error\":\"memory\"}");
  PQclear(r);
}

// rest: the path after /api/playtest/. True when it was one of these routes (answered).
// api --limits: the rooms' section of /docs/limits, after the character chat's (#13263).
void rooms_print_limits(FILE *out) {
  fprintf(out, "\n## Playing Everglade\n\nWhen a room is full, we place you in another room and say which.\n\n"
               "| What | Limit |\n| --- | --- |\n");
  fprintf(out, "| Players in one room | %d |\n", ROOM_SEATS);
  fprintf(out, "\nThis section is made from the server's own number (`ROOM_SEATS`, `coax/web/api/api.h`).\n");
}

bool rooms_route(const request *q, PGconn *db, const char *rest) {
  if (!strcmp(rest, "room-directory")) { report(q, db); return true; }
  if (!strcmp(rest, "room-stats")) { stats(q, db); return true; }
  if (!strcmp(rest, "room-crash")) { crash(q, db); return true; }
  if (!strcmp(rest, "crashes")) { crashes(q, db); return true; }
  if (!strcmp(rest, "room-shards")) { shard_list(q, db); return true; }
  if (!strncmp(rest, "rooms/", 6) && rest[6] && !strchr(rest + 6, '/')) { list(q, db, rest + 6); return true; }
  return false;
}
