// Build your own realm (realm-builder; docs/verse/realm-builder.md): the board from a prompt, the
// realm made from what was checked, its page written into the site's bucket (no deploy), the
// owner's settings and the cards for what others propose. Every rule is SQL's
// (migrations/0048_realm_builder.sql); this file reads who asks and does the model's and the
// bucket's part.
//
//   POST /api/playtest/realms/plan {prompt, setup?, plan?, on?}   -> 200 {plan, board, model}; 400, 429, 502, 503
//   POST /api/playtest/realms/pick {field, words}      -> 200 {field, value}: SAY IT for one setup question
//        (migrations/0049_realm_builder_setup.sql); 400, 429, 503
//   POST /api/playtest/realms {plan?, board}           -> 200 {slug, path, name, page}; 400, 401, 429
//   GET  /api/playtest/realms/<slug>                   -> text/plain, the spec a line each; 404
//   POST /api/playtest/realms/<slug>/settings {tools}  -> 200 {tools}; 403 not an owner or admin
//   POST /api/playtest/realms/<slug>/switch {row, on}  -> 200 {row, on}; 403, 404 (one row of the board)
//   POST /api/playtest/realms/<slug>/revise {prompt}   -> 200 {plan, changed}; 400, 403, 429, 502, 503
// Any realm: a made one or a hand-built one from realms.json (migrations/0050_realm_edit.sql decides
// who is its owner, an admin or a visitor; a site admin, admin_who, is an admin of every realm).
//   POST /api/playtest/realms/<slug>/place {thing, x, z, yaw}   -> 200 {placed} | {proposed}; 403
//   GET  /api/playtest/realms/<slug>/proposals         -> text/plain, the owner's cards ("" for anyone else)
//   POST /api/playtest/realms/<slug>/proposals/<id>/accept|decline   -> 200 {state}; 403, 404, 409
// The panes edit mode brings up (migrations/0055_realm_panes.sql): a character's lore and a quest's form.
//   GET  /api/playtest/realms/<slug>/panes             -> text/plain: "lore WHO|TEXT", "quest GIVER|ASK|S|S|S|REWARD"
//   POST /api/playtest/realms/<slug>/lore {who, text}  -> 200 {who, saved}; 400, 403, 409
//   POST /api/playtest/realms/<slug>/quest {giver, ask, steps, reward}   -> 200 {giver, saved}; 400, 403, 409
//
// Who asks: the X-Verse-Player key (a logged-in player's key carries its account). A write from
// another site's page (Origin) is refused. As a question is, the model call and the page's writes
// are made by a child with its own database connection.
//   OA_BUILDER_URL        the generateContent URL (tests point it at a fake)
//   OA_BUILDER_MODEL      the model (gemini-3.8-flash)
//   OA_BUILDER_DAILY_CAP  plans a day across the server (3000)
//   OA_BUILDER_PER_DAY    realms a player makes a day (12)
#include "api.h"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PREFIX "/api/playtest/realms"

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

static char *body_of(const request *q) {
  char *b = malloc(q->body_len + 3);
  if (!b) return NULL;
  if (q->body_len) { memcpy(b, q->body, q->body_len); b[q->body_len] = 0; } else strcpy(b, "{}");
  return b;
}

// One SQL call that answers {"status", ...}: the status and the rest as JSON; false on an error.
static bool sql_json(const request *q, PGconn *db, const char *sql, int n, const char *const *v, const char *what) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("realms: %s: %s", what, PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    return false;
  }
  api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
  return true;
}

static void text_answer(int fd, PGresult *r) {
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetisnull(r, 0, 0)) { api_json(fd, 404, "{\"error\":\"no such realm\"}"); return; }
  const char *t = PQgetvalue(r, 0, 0);
  api_respond(fd, 200, "text/plain; charset=utf-8", NULL, t, strlen(t));
}

// ---- the plan
// ?as=lines (a page that reads lines, builder.coax): the board's lines as text/plain, or the error alone.
static bool as_lines(const request *q) { char v[16]; return api_query(q, "as", v, sizeof v) && !strcmp(v, "lines"); }
static void answer_lines(int fd, int status, const char *json_text, const char *lines) {
  if (status == 200 && lines) { api_respond(fd, 200, "text/plain; charset=utf-8", NULL, lines, strlen(lines)); return; }
  const char *e = strstr(json_text, "\"error\": \"");
  char out[200] = "error";
  if (e) { e += 10; size_t n = 0; while (e[n] && e[n] != '"' && n + 1 < sizeof out) { out[n] = e[n]; n++; } out[n] = 0; }
  api_respond(fd, status, "text/plain; charset=utf-8", NULL, out, strlen(out));
}

// The model's answer, held by SQL: builder_answer for a plan, builder_pick_answer for a pick.
static void plan_answer(int fd, const char *asked, const char *request, bool lines, bool pick) {
  const char *model = env_or("OA_BUILDER_MODEL", "gemini-3.8-flash");
  char *got = NULL;
  double t0 = api_now_ms();
  long status = npc_ask_model("OA_BUILDER_URL", model, request, &got);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, got ? got : "", model, taken};
  PGresult *r = db ? PQexecParams(db, pick ? "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines' FROM verse.builder_pick_answer($1::jsonb, $2::int, $3, $4, $5::int) x"
                                         : "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines' FROM verse.builder_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
                                  5, NULL, v, NULL, NULL, 0) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    if (lines) answer_lines(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1), PQgetisnull(r, 0, 2) ? NULL : PQgetvalue(r, 0, 2));
    else api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  }
  else { api_log("realms: plan: %s", db ? PQerrorMessage(db) : "no database"); api_json(fd, 503, "{\"error\":\"not kept\"}"); }
  if (status != 200) api_log("realms: model answered %ld", status);
  if (r) PQclear(r);
  if (db) PQfinish(db);
  free(got);
}

static void plan(const request *q, PGconn *db, const char *key, bool pick) {
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *v[3] = {body, env_or("OA_BUILDER_DAILY_CAP", "3000"), key};
  PGresult *r = PQexecParams(db, pick ? "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, (x - 'request' - 'status')::text "
                                        "FROM verse.builder_pick_ask($1::jsonb, $2::int, $3) x"
                                      : "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, (x - 'request' - 'status')::text "
                                        "FROM verse.builder_ask($1::jsonb, $2::int, $3) x", 3, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("realms: ask: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not asked\"}");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  bool lines = as_lines(q);
  if (status != 200) {
    char out[200]; snprintf(out, sizeof out, "{\"error\": \"%s\"}", PQgetvalue(r, 0, 1));
    PQclear(r);
    if (lines) answer_lines(q->fd, status, out, NULL); else api_json(q->fd, status, out);
    return;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { plan_answer(q->fd, asked, request, lines, pick); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
}

// ---- making it: the row in the parent, the page in a child, then the answer
static char *sql_one(PGconn *c, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(c, sql, n, NULL, v, NULL, NULL, 0);
  char *out = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0) ? strdup(PQgetvalue(r, 0, 0)) : NULL;
  PQclear(r);
  return out;
}

// The page at /realms/<slug> (an object of that name, as upload.py writes a site's pages), at
// .../index.html, its index.md, and the list of made realms. Without a bucket (a dev server) it is
// skipped and the page says so: /realms/built#<slug> opens it there.
static bool write_page(PGconn *c, const char *slug) {
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) return false;
  const char *origin = env_or("OA_API_ORIGIN", "https://openagents.com");
  const char *v[2] = {slug, origin};
  char *page = sql_one(c, "SELECT verse.builder_page($1, $2)", 2, v), *md = sql_one(c, "SELECT verse.builder_md($1)", 1, v);
  char *list = sql_one(c, "SELECT verse.builder_slugs()", 0, NULL);
  char name[200];
  bool ok = page && md && list;
  snprintf(name, sizeof name, "realms/%s", slug);
  ok = ok && drops_site_write(name, "text/html; charset=utf-8", page);
  snprintf(name, sizeof name, "realms/%s/index.html", slug);
  ok = ok && drops_site_write(name, "text/html; charset=utf-8", page);
  snprintf(name, sizeof name, "realms/%s/index.md", slug);
  ok = ok && drops_site_write(name, "text/markdown; charset=utf-8", md);
  ok = ok && drops_site_write("realms/made-slugs.txt", "text/plain; charset=utf-8", list);
  free(page); free(md); free(list);
  return ok;
}

static void make_answer(int fd, const char *answer, const char *slug, bool lines) {
  PGconn *db = api_db_new();
  bool paged = db && write_page(db, slug);
  if (db) {
    const char *v[2] = {slug, paged ? NULL : "no page written"};
    PGresult *r = PQexecParams(db, "SELECT verse.builder_paged($1, $2)", 2, NULL, v, NULL, NULL, 0);
    PQclear(r);
    PQfinish(db);
  }
  if (lines) {
    char out[200];
    snprintf(out, sizeof out, "slug %s\npath /realms/%s\npage %d\n", slug, slug, paged ? 1 : 0);
    api_respond(fd, 200, "text/plain; charset=utf-8", NULL, out, strlen(out));
    return;
  }
  size_t n = strlen(answer);
  char *out = malloc(n + 32);
  if (!out) { api_json(fd, 503, "{\"error\":\"memory\"}"); return; }
  memcpy(out, answer, n - 1);   // the answer is an object: its closing brace last
  snprintf(out + n - 1, 32, ",\"page\":%s}", paged ? "true" : "false");
  api_json(fd, 200, out);
  free(out);
}

static void make(const request *q, PGconn *db, const char *key) {
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *v[3] = {body, key, env_or("OA_BUILDER_PER_DAY", "12")};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text, x->>'slug' FROM verse.builder_make($1::jsonb, $2, $3::int) x",
                             3, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("realms: make: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not made\"}");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  bool lines = as_lines(q);
  if (status != 200 || PQgetisnull(r, 0, 2)) {
    if (lines) answer_lines(q->fd, status, PQgetvalue(r, 0, 1), NULL); else api_json(q->fd, status, PQgetvalue(r, 0, 1));
    PQclear(r);
    return;
  }
  char *answer = strdup(PQgetvalue(r, 0, 1)), *slug = strdup(PQgetvalue(r, 0, 2));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { make_answer(q->fd, answer, slug, lines); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(answer); free(slug);
}

// ---- talking revises a realm's board (migrations/0050_realm_edit.sql): asked here, the model's
// answer kept by a child, as a plan is. ?as=lines answers the board's rows as lines.
static void revise_answer(int fd, const char *asked, const char *request, bool lines) {
  const char *model = env_or("OA_BUILDER_MODEL", "gemini-3.8-flash");
  char *got = NULL;
  double t0 = api_now_ms();
  long status = npc_ask_model("OA_BUILDER_URL", model, request, &got);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, got ? got : "", model, taken};
  PGresult *r = db ? PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines' FROM verse.edit_revise_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
                                  5, NULL, v, NULL, NULL, 0) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    if (lines) answer_lines(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1), PQgetisnull(r, 0, 2) ? NULL : PQgetvalue(r, 0, 2));
    else api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  }
  else { api_log("realms: revise: %s", db ? PQerrorMessage(db) : "no database"); api_json(fd, 503, "{\"error\":\"not kept\"}"); }
  if (status != 200) api_log("realms: model answered %ld", status);
  if (r) PQclear(r);
  if (db) PQfinish(db);
  free(got);
}

static void revise(const request *q, PGconn *db, const char *slug, const char *key, const char *admin) {
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *v[5] = {slug, body, env_or("OA_BUILDER_DAILY_CAP", "3000"), key, admin};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, jsonb_build_object('id', x->'id')::text "
                                 "FROM verse.edit_revise_ask($1, $2::jsonb, $3::int, $4, $5::boolean) x", 5, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("realms: revise ask: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not asked\"}");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  bool lines = as_lines(q);
  if (status != 200) {
    char out[200]; snprintf(out, sizeof out, "{\"error\": \"%s\"}", PQgetvalue(r, 0, 1));
    PQclear(r);
    if (lines) answer_lines(q->fd, status, out, NULL); else api_json(q->fd, status, out);
    return;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { revise_answer(q->fd, asked, request, lines); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
}

static bool slug_ok(const char *s, size_t n) {
  if (!n || n > 48) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9') || s[i] == '-')) return false;
  return true;
}

bool realms_route(const request *q) {
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
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST a board\"}"); return true; }
    make(q, db, key);
    return true;
  }
  if (!strcmp(rest, "plan") || !strcmp(rest, "pick")) {
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST a prompt\"}"); return true; }
    plan(q, db, key, !strcmp(rest, "pick"));
    return true;
  }
  const char *slash = strchr(rest, '/');
  size_t n = slash ? (size_t)(slash - rest) : strlen(rest);
  char slug[49];
  if (!slug_ok(rest, n)) { api_json(q->fd, 404, "{\"error\":\"no such realm\"}"); return true; }
  memcpy(slug, rest, n); slug[n] = 0;
  const char *what = slash ? slash + 1 : "";
  // A site admin (admins.conf, by site login or NIP-98) is an admin of every realm; SQL decides the rest.
  char who[65];
  const char *admin = admin_who(q, db, who) == 0 ? "true" : "false";
  if (!*what) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the realm\"}"); return true; }
    const char *v[3] = {slug, key, admin};
    PGresult *r = PQexecParams(db, "SELECT verse.edit_spec($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    text_answer(q->fd, r);
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "panes")) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the panes\"}"); return true; }
    const char *v[3] = {slug, key, admin};
    PGresult *r = PQexecParams(db, "SELECT verse.edit_panes($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else if (PQresultStatus(r) == PGRES_TUPLES_OK) api_json(q->fd, 404, "{\"error\":\"no such realm\"}");
    else { api_log("realms: panes: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "proposals")) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the proposals\"}"); return true; }
    const char *v[3] = {slug, key, admin};
    PGresult *r = PQexecParams(db, "SELECT verse.edit_proposals($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else { api_log("realms: proposals: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return true;
  }
  if (!post) { api_json(q->fd, 405, "{\"error\":\"POST it\"}"); return true; }
  if (!strcmp(what, "revise")) { revise(q, db, slug, key, admin); return true; }
  if (!strcmp(what, "lore") || !strcmp(what, "quest")) {
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[4] = {slug, body, key, admin};
    sql_json(q, db, !strcmp(what, "lore") ? "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.edit_lore($1, $2::jsonb, $3, $4::boolean) x"
                                          : "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.edit_quest($1, $2::jsonb, $3, $4::boolean) x",
             4, v, what);
    free(body);
    return true;
  }
  if (!strcmp(what, "settings") || !strcmp(what, "place") || !strcmp(what, "switch")) {
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[4] = {slug, body, key, admin};
    sql_json(q, db, !strcmp(what, "settings") ? "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.edit_settings($1, $2::jsonb, $3, $4::boolean) x"
                    : !strcmp(what, "place")  ? "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.edit_place($1, $2::jsonb, $3, $4::boolean) x"
                                              : "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.edit_switch($1, $2::jsonb, $3, $4::boolean) x",
             4, v, what);
    free(body);
    return true;
  }
  static const char card[] = "proposals/";
  if (!strncmp(what, card, sizeof card - 1)) {
    const char *id = what + sizeof card - 1, *end = strchr(id, '/');
    char num[24];
    size_t k = end ? (size_t)(end - id) : 0;
    bool ok = end && k > 0 && k < 19 && (!strcmp(end, "/accept") || !strcmp(end, "/decline"));
    for (size_t i = 0; ok && i < k; i++) ok = id[i] >= '0' && id[i] <= '9';
    if (!ok) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
    memcpy(num, id, k); num[k] = 0;
    const char *v[5] = {slug, num, !strcmp(end, "/accept") ? "true" : "false", key, admin};
    sql_json(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.edit_decide($1, $2::bigint, $3::boolean, $4, $5::boolean) x", 5, v, "decide");
    return true;
  }
  api_json(q->fd, 404, "{\"error\":\"unknown\"}");
  return true;
}
