// The Everglade instance directory (#12717; spec sections 3.4 and 9): which oa-room shard holds each
// instance, who sits in it, and where a new one goes. The state and every decision are Postgres's
// (migrations/0115_room_directory.sql: verse.room_pick, verse.room_report, verse.room_list); this file
// moves the facts in and the answers out.
//   POST /api/playtest/room-directory     a shard's report (coax/web/room/room-directory.coax), signed
//                                         with the seat-ticket key (X-Room-Mac, as room-vitals is)
//   GET  /api/playtest/rooms/REALM        its public instances and which to join; ?with=PLAYER_ID also
//                                         names the instance that player sits in
// player.c room_ticket asks rooms_pick for the instance and shard when the page names its realm.
#include "api.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/hmac.h>
#include <openssl/evp.h>

#define ROOM_SEATS 16   // coax/web/room/room-relay.coax RELAY_SEATS

static bool code_ok(const char *s, size_t most) {
  size_t n = strlen(s);
  if (!n || n > most) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9') || s[i] == '-')) return false;
  return true;
}

// The instance for player pid in realm (asked: the instance the page names, or the realm itself to be
// given one): its code and shard, and whether the asked one was full. False when there is no seat.
bool rooms_pick(PGconn *db, const char *realm, const char *asked, const char *pid, char *code, size_t cap, int *shard, bool *full) {
  if (!code_ok(realm, 20) || !code_ok(asked, 24)) return false;
  char seats[8]; snprintf(seats, sizeof seats, "%d", ROOM_SEATS);
  const char *v[4] = {realm, asked, pid, seats};
  PGresult *r = PQexecParams(db, "SELECT code, shard, was_full FROM verse.room_pick($1, $2, $3, $4::int)", 4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) && strlen(PQgetvalue(r, 0, 0)) < cap;
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("rooms: %s", PQerrorMessage(db));
  if (ok) {
    snprintf(code, cap, "%s", PQgetvalue(r, 0, 0));
    *shard = atoi(PQgetvalue(r, 0, 1));
    *full = PQgetvalue(r, 0, 2)[0] == 't';
  }
  PQclear(r);
  return ok;
}

static void report(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a report\"}"); return; }
  const char *secret = getenv("OA_ROOM_TICKET_KEY");
  if (!secret || strlen(secret) < 32) { api_json(q->fd, 503, "{\"error\":\"no ticket key\"}"); return; }
  char given[80];
  if (!api_header(q, "X-Room-Mac", given, sizeof given) || strlen(given) != 64 || q->body_len == 0 || q->body_len > 32768) { api_json(q->fd, 401, "{\"error\":\"unsigned\"}"); return; }
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)q->body, q->body_len, mac, &ml) || ml != 32) { api_json(q->fd, 500, "{\"error\":\"no mac\"}"); return; }
  char want[65]; for (unsigned i = 0; i < 32; i++) snprintf(want + 2 * i, 3, "%02x", mac[i]);
  unsigned diff = 0; for (int i = 0; i < 64; i++) diff |= (unsigned)(want[i] ^ given[i]);
  if (diff) { api_json(q->fd, 401, "{\"error\":\"bad mac\"}"); return; }
  char *b = malloc(q->body_len + 1);
  if (!b) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  const char *v[1] = {b};
  PGresult *r = PQexecParams(db, "SELECT verse.room_report($1)", 1, NULL, v, NULL, NULL, 0);
  free(b);
  int named = -1;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) named = atoi(PQgetvalue(r, 0, 0));
  else api_log("room-directory: %s", PQerrorMessage(db));
  PQclear(r);
  if (named < 0) { api_json(q->fd, 400, "{\"error\":\"not a report\"}"); return; }
  char out[48]; snprintf(out, sizeof out, "{\"rooms\":%d}", named);
  api_json(q->fd, 200, out);
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

// rest: the path after /api/playtest/. True when it was one of these routes (answered).
bool rooms_route(const request *q, PGconn *db, const char *rest) {
  if (!strcmp(rest, "room-directory")) { report(q, db); return true; }
  if (!strncmp(rest, "rooms/", 6) && rest[6] && !strchr(rest + 6, '/')) { list(q, db, rest + 6); return true; }
  return false;
}
