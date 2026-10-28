// Rooms (pt-presence-chat): who else is in a world, and its chat. The rules
// live in SQL (migrations/0013_verse_rooms.sql, verse.room); this file reads
// the path, the player key and the body and hands them over.
//
//   POST /api/playtest/presence/<room>   {x, y, z, yaw, after}: here now -> {you, others, messages
//                                        after the number `after`, last}
//   GET  /api/playtest/chat/<room>?after=N   the lines after N (the last 50) -> {messages, last}
//   POST /api/playtest/chat/<room>       {text, sid} -> {n}
//
// The demo's /api/presence/<room> and /api/chat/<room> answer the same.
#include "api.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *after_prefix(const char *path, const char *kind) {
  static const char *const roots[] = {"/api/playtest/", "/api/"};
  for (size_t i = 0; i < sizeof roots / sizeof roots[0]; i++) {
    size_t a = strlen(roots[i]), b = strlen(kind);
    if (!strncmp(path, roots[i], a) && !strncmp(path + a, kind, b) && path[a + b] == '/') return path + a + b + 1;
  }
  return NULL;
}

bool room_route(const request *q) {
  const char *kind = "presence", *room = after_prefix(q->path, kind);
  if (!room) { kind = "chat"; room = after_prefix(q->path, kind); }
  if (!room) return false;
  if (strcmp(q->method, "GET") && strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  if (strlen(room) > 64 || strchr(room, '/')) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char key[160], given[24], after[24] = "-1";
  api_header(q, "X-Verse-Player", key, sizeof key);
  if (api_query(q, "after", given, sizeof given)) {
    bool digits = given[0] != 0 && strlen(given) < 10;
    for (char *c = given; *c; c++) if (*c < '0' || *c > '9') digits = false;
    snprintf(after, sizeof after, "%s", digits ? given : "0");
  }
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *v[6] = {q->method, kind, room, body, key, after};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM verse.room($1, $2, $3, $4::jsonb, $5, $6::int) x",
                             6, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("room: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return true;
  }
  api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
  return true;
}
