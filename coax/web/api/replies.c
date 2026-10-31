// Reply with a realm (ev-reply-realm, docs/verse/replies.md): a line or a post's link a player put
// on a house in Everglade and smashed with a meteor shower, kept so its link replays it. Every rule
// is SQL's (migrations/0076_realm_replies.sql); this file reads who asks and does the bucket's part.
// Only what the player typed or pasted is kept; a link is text, never fetched.
//
//   POST /api/playtest/replies {realm, text | link, seed}   -> 200 {id, path, page}; 400, 401, 429
//   GET  /api/playtest/replies/<id>                         -> text/plain, what the realm replays (a line each); 404
//   GET  /api/playtest/replies/<id>/card                    -> text/plain "want" (its maker, no card yet) or "no"
//   POST /api/playtest/replies/<id>/card  (image/jpeg)      -> 200 {image, page}: its maker; 400 not a 1200x630 JPEG, 413, 403, 404
//
// Its page, /realms/everglade/reply/<id> (and .../index.html, .../index.md), and its card,
// .../card.jpg, are written into the site's bucket (OA_SITE_BUCKET) by a child with its own
// connection; a release keeps them (coax/web/upload.py).
//   OA_REPLIES_PER_DAY   replies a player makes a day (40)
#include "api.h"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PREFIX "/api/playtest/replies"
#define CARD_MAX (300 * 1024)

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

static char *sql_one(PGconn *c, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(c, sql, n, NULL, v, NULL, NULL, 0);
  char *out = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) ? strdup(PQgetvalue(r, 0, 0)) : NULL;
  PQclear(r);
  return out;
}

static bool id_ok(const char *s, size_t n) {
  if (n != 12) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9'))) return false;
  return true;
}

// Its page, its bare path and its text, into the site's bucket. False without a bucket (a dev server).
static bool write_page(PGconn *c, const char *id) {
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) return false;
  const char *v[2] = {id, env_or("OA_API_ORIGIN", "https://openagents.com")};
  char *page = sql_one(c, "SELECT verse.reply_page($1, $2)", 2, v), *md = sql_one(c, "SELECT verse.reply_md($1)", 1, v);
  char name[120];
  bool ok = page && md;
  snprintf(name, sizeof name, "realms/everglade/reply/%s", id);
  ok = ok && drops_site_write(name, "text/html; charset=utf-8", page);
  snprintf(name, sizeof name, "realms/everglade/reply/%s/index.html", id);
  ok = ok && drops_site_write(name, "text/html; charset=utf-8", page);
  snprintf(name, sizeof name, "realms/everglade/reply/%s/index.md", id);
  ok = ok && drops_site_write(name, "text/markdown; charset=utf-8", md);
  free(page); free(md);
  return ok;
}

static void make(const request *q, PGconn *db, const char *key) {
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  if (q->body_len > 4096) { free(body); api_json(q->fd, 413, "{\"error\":\"a reply is a line or a link\"}"); return; }
  char per[16];
  snprintf(per, sizeof per, "%d", atoi(env_or("OA_REPLIES_PER_DAY", "40")));
  const char *v[3] = {body, key, per};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text, x->>'id' FROM verse.reply_make($1::jsonb, $2, $3::int) x", 3, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("replies: make: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) { api_json(q->fd, status, PQgetvalue(r, 0, 1)); PQclear(r); return; }
  char id[16];
  snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 2));
  PQclear(r);
  // the page in a child, then the answer, as a made realm's
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid < 0) { api_json(q->fd, 503, "{\"error\":\"busy\"}"); return; }
  if (pid > 0) return;
  PGconn *c = api_db_new();
  bool paged = c && write_page(c, id);
  char out[200];
  snprintf(out, sizeof out, "{\"id\":\"%s\",\"path\":\"/realms/everglade/reply/%s\",\"page\":%s}", id, id, paged ? "true" : "false");
  api_json(q->fd, 200, out);
  if (c) PQfinish(c);
  _exit(0);
}

static void card(const request *q, PGconn *db, const char *id, const char *key, bool post) {
  const char *v[2] = {id, key};
  if (!post) {
    char *want = sql_one(db, "SELECT verse.reply_card_want($1, $2)", 2, v);
    if (!want) { api_json(q->fd, 404, "{\"error\":\"no such reply\"}"); return; }
    api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, want, strlen(want));
    free(want);
    return;
  }
  char *may = sql_one(db, "SELECT verse.reply_card_may($1, $2)", 2, v);
  int status = may ? atoi(may) : 500;
  free(may);
  if (status == 404) { api_json(q->fd, 404, "{\"error\":\"no such reply\"}"); return; }
  if (status != 200) { api_json(q->fd, status == 403 ? 403 : 500, status == 403 ? "{\"error\":\"only its maker\"}" : "{\"error\":\"not kept\"}"); return; }
  if (q->body_len > CARD_MAX) { api_json(q->fd, 413, "{\"error\":\"a card is under 300 KB\"}"); return; }
  if (!realms_jpeg_card((const unsigned char *)q->body, q->body_len)) { api_json(q->fd, 400, "{\"error\":\"a 1200x630 JPEG\"}"); return; }
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) { api_json(q->fd, 503, "{\"error\":\"no bucket here\"}"); return; }
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid < 0) { api_json(q->fd, 503, "{\"error\":\"busy\"}"); return; }
  if (pid > 0) return;
  char name[120], kb[16], out[200];
  snprintf(name, sizeof name, "realms/everglade/reply/%s/card.jpg", id);
  snprintf(kb, sizeof kb, "%zu", (q->body_len + 1023) / 1024);
  PGconn *c = api_db_new();
  if (!c || !drops_site_write_bytes(name, "image/jpeg", (const unsigned char *)q->body, q->body_len)) { api_json(q->fd, 502, "{\"error\":\"not kept\"}"); _exit(0); }
  const char *w[2] = {id, kb};
  char *at = sql_one(c, "SELECT verse.reply_card_kept($1, $2::int)", 2, w);
  bool paged = at && write_page(c, id);
  snprintf(out, sizeof out, "{\"image\":\"/realms/everglade/reply/%s/card.jpg?v=%s\",\"page\":%s}", id, at ? at : "0", paged ? "true" : "false");
  api_json(q->fd, 200, out);
  free(at);
  PQfinish(c);
  _exit(0);
}

bool replies_route(const request *q) {
  if (strncmp(q->path, PREFIX, sizeof PREFIX - 1) || (q->path[sizeof PREFIX - 1] && q->path[sizeof PREFIX - 1] != '/')) return false;
  const char *rest = q->path + sizeof PREFIX - 1;
  if (*rest == '/') rest++;
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char key[160] = "", from[256];
  api_header(q, "X-Verse-Player", key, sizeof key);
  bool post = !strcmp(q->method, "POST");
  if (post && api_header(q, "Origin", from, sizeof from) && strcmp(from, env_or("OA_API_ORIGIN", "https://openagents.com"))) {
    api_json(q->fd, 403, "{\"error\":\"cross-site\"}");
    return true;
  }
  if (!*rest) {
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST a reply\"}"); return true; }
    make(q, db, key);
    return true;
  }
  const char *slash = strchr(rest, '/');
  size_t n = slash ? (size_t)(slash - rest) : strlen(rest);
  if (!id_ok(rest, n)) { api_json(q->fd, 404, "{\"error\":\"no such reply\"}"); return true; }
  char id[16];
  memcpy(id, rest, n); id[n] = 0;
  const char *what = slash ? slash + 1 : "";
  if (!strcmp(what, "card")) { card(q, db, id, key, post); return true; }
  if (*what || post) { api_json(q->fd, 404, "{\"error\":\"no such reply\"}"); return true; }
  const char *v[1] = {id};
  char *lines = sql_one(db, "SELECT verse.reply_lines($1)", 1, v);
  if (!lines) { api_json(q->fd, 404, "{\"error\":\"no such reply\"}"); return true; }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, lines, strlen(lines));
  free(lines);
  return true;
}
