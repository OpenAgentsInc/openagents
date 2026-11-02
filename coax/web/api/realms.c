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
// Its link preview (migrations/0071_made_realm_share.sql, docs/web/og-tags.md): its line and its own picture.
//   POST /api/playtest/realms/<slug>/describe {description}   -> 200 {description}; 400, 403, 404
//   GET  /api/playtest/realms/<slug>/card              -> text/plain "want" (take its picture) or "no"
//   POST /api/playtest/realms/<slug>/card  (image/jpeg) -> 200 {image}; 400 not a 1200x630 JPEG, 413, 403, 404
//   GET  /api/playtest/realms/made                     -> text/plain, the newest made realms as /og's rows
//   GET  /api/playtest/realms/listed                   -> text/plain, "total N" then the newest public made realms as
//        /realms' rows SLUG|TITLE|LINE|YYYY-MM-DDTHH:MM|STYLE (migrations/0101_realm_list.sql), at most 60
//   GET  /api/playtest/realms/mine                     -> the caller's made realms: {realms: [{slug, name, rev, published,
//        listed, visibility}]}, or ?as=lines "realm SLUG|REV|PUBLISHED|LISTED|NAME|VISIBILITY" (published 0: never); none is {realms: []} or ""
//   POST /api/playtest/realms/<slug>/visibility {visibility}   -> 200 {slug, visibility, was, page}; 400, 403, 404 (0099:
//        private|unlisted|public; its owner when access.gate admits them, or a site admin. A private realm is 404 on every
//        route under its slug to all but its owner and site admins, and has no page in the site bucket.)
// FORK (migrations/0075_realm_forks.sql, docs/verse/realm-fork.md): any built-in realm with rule sets, or a fork.
//   GET  /api/playtest/realms/<slug>/fork              -> text/plain: base, title, fork, from, maker, name, owner, published, rule ID 0|1 LABEL
//   POST /api/playtest/realms/<slug>/fork {rules, name?}   -> 200 {slug, path, name, base, from, rules, page}; 400, 401, 404, 429
//   POST /api/playtest/realms/<fork>/publish {published?, description?}   -> 200 {published, description}; 403, 404
// A made realm is a document with revisions (migrations/0093_realm_documents.sql, Aethyr book chapters 51-55, 82):
//   GET  /api/playtest/realms/<slug>/doc               -> 200 {doc, sha256}; 404 (a visitor's: the revision published)
//   POST /api/playtest/realms/<slug>/save {rev, moved, shaped?} -> 200 {rev, sha256}; 400, 403, 404, 409 {error, rev} (changed since;
//        shaped: turns, sizes and removals, 0109; absent keeps them)
//   POST /api/playtest/realms/<slug>/publish {rev, listed?}   -> 200 {published, rev, sha256, listed}; 400, 403, 404, 409
//   POST /api/playtest/realms/<slug>/undo {rev}        -> 200 {rev, undo}; 400, 403, 404, 409 {error, rev} (0094: every edit is a step)
//   GET  /api/playtest/realms/<slug>/published         -> 200 {slug, n, rev, sha256, engine, engine_sha256, why, ms}; 404 not pinned
//   (0107: a publish pins the engine, /realms/built's app script, with the revision; 503 when the bucket
//   has no engine page named by content, and nothing is published. The page plays its pin.)
//   (a visitor's spec, document and fork are the revision published; its owner's are the realm now)
//   POST /api/playtest/realms/<slug>/fork {name?}      -> 200 {slug, path, name, from, rev}: its document copied, parent and revision kept
// A realm made or forked by a request with an automation mark (X-Verse-Automation, or the cookie
// verse-automation) is unlisted: off /og's rows and noindex.
// Any realm: a made one or a hand-built one from realms.json (migrations/0050_realm_edit.sql decides
// who is its owner, an admin or a visitor; a site admin, admin_who, is an admin of every realm).
//   POST /api/playtest/realms/<slug>/place {thing, x, z, yaw}   -> 200 {placed} | {proposed}; 403
//   GET  /api/playtest/realms/<slug>/proposals         -> text/plain, the owner's cards ("" for anyone else)
//   POST /api/playtest/realms/<slug>/proposals/<id>/accept|decline   -> 200 {state}; 403, 404, 409
// The panes edit mode brings up (migrations/0055_realm_panes.sql): a character's lore and a quest's form.
//   GET  /api/playtest/realms/<slug>/panes             -> text/plain: "lore WHO|TEXT", "quest GIVER|ASK|S|S|S|REWARD"
//   POST /api/playtest/realms/<slug>/lore {who, text}  -> 200 {who, saved}; 400, 403, 409
//   POST /api/playtest/realms/<slug>/quest {giver, ask, steps, reward}   -> 200 {giver, saved}; 400, 403, 409
// What its owner asked for (migrations/0078_realm_asks.sql): the feature list, ADD and DELETE.
//   GET  /api/playtest/realms/<slug>/asks              -> text/plain: "ask ID applied|waiting|removing ask|remove WORDS" ("" for a visitor)
//   POST /api/playtest/realms/<slug>/asks {words}      -> text/plain: "ask ID applied|waiting", the board's rows; 400, 403, 404, 429
//   POST /api/playtest/realms/<slug>/asks/<id>/delete  -> 200 {deleted, reverted} | {ask, state: removing}; 403, 404
//   POST /api/playtest/realms/<slug>/rows/<key>/delete -> 200 {deleted} | {ask, state: waiting}; 403, 404
//   POST /api/playtest/realms/<slug>/asks/<id>/done {commit}   a site admin: built in code, or its code out; 400, 403, 404, 409
// A design document dropped on the list or the builder (migrations/0080_realm_docs.sql): {name, data}, the
// file's bytes in base64 (.md, .txt, .pdf or .docx, at most what a request holds). Its text is taken out
// here (a .docx's word/document.xml, inflated) or, for a PDF, read by the model itself; every feature
// or change it asks for becomes a line on the list (or the board), applied where it can be drawn.
//   POST /api/playtest/realms/<slug>/asks/doc {name, data}   -> text/plain: "doc N applied M waiting", the rows; 400, 403, 413, 415
//   POST /api/playtest/realms/plan/doc {name, data, setup}   -> the builder's board, as /plan answers it
//
// Realm invites (access.c, migrations/0090_realm_invites.sql): with the flag realm_invites on, plan,
// pick, plan/doc, MAKE IT and a FORK's POST answer 403 "invite only" to anyone not an approved
// creator or a site admin. Off (the default), they are as before.
// Who asks: the X-Verse-Player key (a logged-in player's key carries its account), or a NIP-98
// signature, which is its key's account (auth.account_of) through the player key SQL derives for that
// account (auth.caller_key, migrations/0098_cli_callers.sql); a signed write binds its body and every
// signature is good once (caller-policy.coax). A write from another site's page (Origin) is refused.
// As a question is, the model call and the page's writes are made by a child with its own database
// connection.
//   OA_BUILDER_URL        the generateContent URL (tests point it at a fake)
//   OA_BUILDER_MODEL      the model (gemini-2.5-flash, its thinking off: migrations/0060_builder_fast.sql)
//   OA_BUILDER_TIMEOUT_MS how long one try may take (3500); an unanswered or failed try is tried once more
//   OA_BUILDER_DAILY_CAP  plans a day across the server (3000)
//   OA_BUILDER_PER_DAY    realms a player makes a day (12)
#include "api.h"
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <zlib.h>

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
static long ask_builder(const char *model, const char *request, char **got) {
  long limit = atol(env_or("OA_BUILDER_TIMEOUT_MS", "3500"));
  return npc_ask_model_twice("OA_BUILDER_URL", model, request, limit > 0 ? limit : 3500, got);
}
// ?as=lines (a page that reads lines, builder.coax): the board's lines as text/plain, or the error alone.
static bool as_lines(const request *q) { char v[16]; return api_query(q, "as", v, sizeof v) && !strcmp(v, "lines"); }
static void answer_lines(int fd, int status, const char *json_text, const char *lines) {
  if (status == 200 && lines) { api_respond(fd, 200, "text/plain; charset=utf-8", NULL, lines, strlen(lines)); return; }
  // A fenced refusal retains the current revision even for callers that request lines.
  if (status == 409 && strstr(json_text, "\"rev\"")) { api_json(fd, status, json_text); return; }
  const char *e = strstr(json_text, "\"error\": \"");
  char out[200] = "error";
  if (e) { e += 10; size_t n = 0; while (e[n] && e[n] != '"' && n + 1 < sizeof out) { out[n] = e[n]; n++; } out[n] = 0; }
  api_respond(fd, status, "text/plain; charset=utf-8", NULL, out, strlen(out));
}

// The model's answer, held by SQL: builder_answer for a plan, builder_pick_answer for a pick. A
// board that leaves out something asked for (migrations/0065_builder_subject.sql) comes back as
// {retry: REQUEST}: the model is asked that once more and its answer held as the second try.
static PGresult *answer_of(PGconn *db, bool pick, const char *asked, long status, const char *got, const char *model, double t0, const char *request) {
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  if (pick) {
    const char *v[5] = {asked, http, got ? got : "", model, taken};
    return PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines', NULL::text FROM verse.builder_pick_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
                        5, NULL, v, NULL, NULL, 0);
  }
  const char *v[6] = {asked, http, got ? got : "", model, taken, request};
  return PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status' - 'lines' - 'retry')::text, x->>'lines', (x->'retry')::text "
                          "FROM verse.builder_answer($1::jsonb || CASE WHEN $6::jsonb IS NULL THEN '{\"retry\": true}' ELSE '{}' END::jsonb, $2::int, $3, $4, $5::int, $6::jsonb) x", 6, NULL, v, NULL, NULL, 0);
}
static void plan_answer(int fd, const char *asked, const char *request, bool lines, bool pick) {
  const char *model = env_or("OA_BUILDER_MODEL", "gemini-2.5-flash");
  char *got = NULL;
  double t0 = api_now_ms();
  long status = ask_builder(model, request, &got);
  PGconn *db = api_db_new();
  PGresult *r = db ? answer_of(db, pick, asked, status, got, model, t0, request) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 3)) {
    char *again = strdup(PQgetvalue(r, 0, 3));
    PQclear(r); r = NULL;
    if (again) {
      free(got); got = NULL;
      long second = ask_builder(model, again, &got);
      if (second != 200) api_log("realms: model answered %ld on the second try", second);
      r = answer_of(db, false, asked, second, got, model, t0, NULL);   // no request: the second try
      free(again);
    }
  }
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
// A private realm (0099) has no page there: write_page writes nothing for it, and making a realm
// private (POST .../visibility) removes its objects and its picture and takes it off the list. It
// opens at /realms/built#<slug> for its owner and site admins only.
static bool remove_page(PGconn *c, const char *slug) {
  static const char *const parts[4] = {"realms/%s", "realms/%s/index.html", "realms/%s/index.md", "realms/%s/card.jpg"};
  char name[200];
  bool ok = true;
  for (int i = 0; i < 4; i++) { snprintf(name, sizeof name, parts[i], slug); ok = drops_site_delete(name) && ok; }
  char *list = sql_one(c, "SELECT verse.builder_slugs()", 0, NULL);
  ok = list && drops_site_write("realms/made-slugs.txt", "text/plain; charset=utf-8", list) && ok;
  free(list);
  const char *v[1] = {slug};
  PGresult *r = PQexecParams(c, "SELECT verse.realm_card_gone($1)", 1, NULL, v, NULL, NULL, 0); PQclear(r);
  if (ok) { r = PQexecParams(c, "SELECT verse.builder_page_written($1)", 1, NULL, v, NULL, NULL, 0); PQclear(r); }
  return false;   // no page written
}

// The made-realm engine's page as the site serves it now (/realms/built: its app script names the
// engine's files by their content), which a publish pins (migrations/0107_realm_engine_pins.sql).
// OA_ENGINE_TEMPLATE names a file to read instead (checks). NULL: this server has no site bucket,
// so nothing is pinned; "" (malloc'd): it has one and the page couldn't be read, which refuses.
static char *engine_template(void) {
  const char *file = getenv("OA_ENGINE_TEMPLATE");
  if (file && *file) {
    FILE *f = fopen(file, "rb");
    char *t = calloc(65537, 1);
    size_t n = f && t ? fread(t, 1, 65536, f) : 0;
    if (f) fclose(f);
    if (t && n == 65536) t[0] = 0;   // too big to be the page: refused
    return t ? t : strdup("");
  }
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) return NULL;
  char *t = drops_site_read("realms/built/index.html");
  return t ? t : strdup("");
}

// unhide: the realm was just made private, so a page it had is removed.
static bool write_page_or_remove(PGconn *c, const char *slug, bool unhide) {
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) return false;
  const char *origin = env_or("OA_API_ORIGIN", "https://openagents.com");
  const char *v[2] = {slug, origin};
  char *action = sql_one(c, "SELECT verse.realm_page_action($1)", 1, v);
  bool remove = !action || strcmp(action, "write");
  free(action);
  if (remove && unhide) return remove_page(c, slug);
  if (remove) {   // private: nothing to write, and nothing stale (realms_refresh_pages moves on)
    PGresult *r = PQexecParams(c, "SELECT verse.builder_page_written($1)", 1, NULL, v, NULL, NULL, 0); PQclear(r);
    return false;
  }
  // A published revision not yet pinned (a new realm's first, or one published before 0107) is
  // pinned to the engine served now before its page names it.
  char *needs = sql_one(c, "SELECT verse.realm_needs_pin($1)::text", 1, v);
  if (needs && !strcmp(needs, "true")) {
    char *t = engine_template();
    if (t) {
      const char *w[2] = {slug, t};
      PGresult *r = PQexecParams(c, "SELECT verse.realm_pin_first($1, $2)", 2, NULL, w, NULL, NULL, 0);
      if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("realms: pin %s: %s", slug, PQerrorMessage(c));
      PQclear(r);
      free(t);
    }
  }
  free(needs);
  char *page =sql_one(c, "SELECT verse.builder_page($1, $2)", 2, v), *md = sql_one(c, "SELECT verse.builder_md($1)", 1, v);
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
  if (ok) { PGresult *r = PQexecParams(c, "SELECT verse.builder_page_written($1)", 1, NULL, v, NULL, NULL, 0); PQclear(r); }
  return ok;
}
static bool write_page(PGconn *c, const char *slug) { return write_page_or_remove(c, slug, false); }

// On start: every made realm's page written with an older template is written again, in a child
// (any instance may; each page is claimed by one, verse.builder_page_stale).
void realms_refresh_pages(void) {
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) return;
  signal(SIGCHLD, SIG_IGN);
  if (fork() != 0) return;
  PGconn *db = api_db_new();
  int n = 0, failed = 0;
  for (char *slug; db && n < 5000 && (slug = sql_one(db, "SELECT verse.builder_page_stale()", 0, NULL)); n++) {
    if (!write_page(db, slug)) failed++;
    free(slug);
  }
  if (n) api_log("realms: %d made pages written again, %d failed", n, failed);
  if (db) PQfinish(db);
  _exit(0);
}

// ---- its picture: a 1200x630 baseline or progressive JPEG under 300 KB, as the page's own frame
// (coax/web/card.js) or anything else its owner sends. Only the frame header is read.
#define CARD_MAX (300 * 1024)
static bool jpeg_1200x630(const unsigned char *b, size_t n) {
  if (n < 4 || b[0] != 0xFF || b[1] != 0xD8) return false;
  size_t i = 2;
  while (i + 4 <= n) {
    if (b[i] != 0xFF) return false;
    unsigned char m = b[i + 1];
    if (m == 0xFF) { i++; continue; }
    if (m == 0xD9 || m == 0xDA) return false;          // the image or its end before a frame header
    size_t len = ((size_t)b[i + 2] << 8) | b[i + 3];
    if (len < 2 || i + 2 + len > n) return false;
    if ((m >= 0xC0 && m <= 0xC3) || (m >= 0xC5 && m <= 0xC7) || (m >= 0xC9 && m <= 0xCB) || (m >= 0xCD && m <= 0xCF)) {
      if (len < 7) return false;
      unsigned h = ((unsigned)b[i + 5] << 8) | b[i + 6], w = ((unsigned)b[i + 7] << 8) | b[i + 8];
      return w == 1200 && h == 630;
    }
    i += 2 + len;
  }
  return false;
}
bool realms_jpeg_card(const unsigned char *b, size_t n) { return jpeg_1200x630(b, n); }   // replies.c's card too
static void card(const request *q, PGconn *db, const char *slug, const char *key, const char *admin, bool post) {
  const char *v[3] = {slug, key, admin};
  if (!post) {
    char *want = sql_one(db, "SELECT verse.builder_card_want($1, $2, $3::boolean)", 3, v);
    if (!want) { api_json(q->fd, 404, "{\"error\":\"no such realm\"}"); return; }
    api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, want, strlen(want));
    free(want);
    return;
  }
  char *may = sql_one(db, "SELECT verse.builder_card_may($1, $2, $3::boolean)", 3, v);
  int status = may ? atoi(may) : 500;
  free(may);
  if (status == 404) { api_json(q->fd, 404, "{\"error\":\"no such realm\"}"); return; }
  if (status != 200) { api_json(q->fd, status == 403 ? 403 : 500, status == 403 ? "{\"error\":\"only its owner and admins\"}" : "{\"error\":\"not kept\"}"); return; }
  if (q->body_len > CARD_MAX) { api_json(q->fd, 413, "{\"error\":\"a card is under 300 KB\"}"); return; }
  if (!jpeg_1200x630((const unsigned char *)q->body, q->body_len)) { api_json(q->fd, 400, "{\"error\":\"a 1200x630 JPEG\"}"); return; }
  if (!getenv("OA_SITE_BUCKET") || !*getenv("OA_SITE_BUCKET")) { api_json(q->fd, 503, "{\"error\":\"no bucket here\"}"); return; }
  // the bucket's part in a child, as making one is
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid < 0) { api_json(q->fd, 503, "{\"error\":\"busy\"}"); return; }
  if (pid > 0) return;
  char name[120], kb[16], out[200];
  snprintf(name, sizeof name, "realms/%s/card.jpg", slug);
  snprintf(kb, sizeof kb, "%zu", (q->body_len + 1023) / 1024);
  PGconn *c = api_db_new();
  if (!c || !drops_site_write_bytes(name, "image/jpeg", (const unsigned char *)q->body, q->body_len)) { api_json(q->fd, 502, "{\"error\":\"not kept\"}"); _exit(0); }
  const char *w[2] = {slug, kb};
  char *at = sql_one(c, "SELECT verse.builder_card_kept($1, $2::int)", 2, w);
  bool paged = at && write_page(c, slug);
  snprintf(out, sizeof out, "{\"image\":\"/realms/%s/card.jpg?v=%s\",\"page\":%s}", slug, at ? at : "0", paged ? "true" : "false");
  api_json(q->fd, 200, out);
  _exit(0);
}

// The page again after its line changed, in a child.
static void rewrite_page(const char *slug) {
  signal(SIGCHLD, SIG_IGN);
  if (fork() != 0) return;
  PGconn *c = api_db_new();
  if (c) write_page(c, slug);
  _exit(0);
}


// A request a tool or test made (the mark analytics.c reads): its realms stay unlisted.
static bool automated(const request *q) {
  char v[64], cookies[2048];
  if (api_header(q, "X-Verse-Automation", v, sizeof v) && v[0]) return true;
  return api_header(q, "Cookie", cookies, sizeof cookies) && strstr(cookies, "verse-automation=") != NULL;
}
static void unlist_if_automated(const request *q, PGconn *db, const char *slug) {
  if (!automated(q)) return;
  const char *v[1] = {slug};
  PGresult *r = PQexecParams(db, "UPDATE verse.made_realms SET unlisted = true WHERE slug = $1", 1, NULL, v, NULL, NULL, 0);
  PQclear(r);
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
  // its own line, when MAKE IT sends one ({description}); else the line is written from what is in it
  char *again = body_of(q);
  if (again) {
    const char *d[2] = {slug, again};
    r = PQexecParams(db, "UPDATE verse.made_realms SET description = nullif(verse.builder_one_line($2::jsonb->'description', 200), '') "
                         "WHERE slug = $1 AND jsonb_typeof($2::jsonb->'description') = 'string'", 2, NULL, d, NULL, NULL, 0);
    PQclear(r);
    free(again);
  }
  unlist_if_automated(q, db, slug);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { make_answer(q->fd, answer, slug, lines); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(answer); free(slug);
}

// ---- FORK (migrations/0075_realm_forks.sql, docs/verse/realm-fork.md): GET what the pane shows;
// POST makes a made realm whose base is this one's, with the rules chosen, and writes its page as
// MAKE IT does (?as=lines answers "slug S", "path P", "page 0|1").
static void fork_realm(const request *q, PGconn *db, const char *slug, const char *key, const char *admin, bool post) {
  if (!post) {
    const char *v[3] = {slug, key, admin};
    PGresult *r = PQexecParams(db, "SELECT verse.fork_info($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else if (PQresultStatus(r) == PGRES_TUPLES_OK) api_json(q->fd, 404, "{\"error\":\"this realm can't be forked\"}");
    else { api_log("realms: fork info: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return;
  }
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *v[4] = {slug, body, key, env_or("OA_BUILDER_PER_DAY", "12")};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text, x->>'slug' FROM verse.fork_make($1, $2::jsonb, $3, $4::int) x",
                             4, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("realms: fork: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not forked\"}");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  bool lines = as_lines(q);
  if (status != 200 || PQgetisnull(r, 0, 2)) {
    if (lines) answer_lines(q->fd, status, PQgetvalue(r, 0, 1), NULL); else api_json(q->fd, status, PQgetvalue(r, 0, 1));
    PQclear(r);
    return;
  }
  char *answer = strdup(PQgetvalue(r, 0, 1)), *made = strdup(PQgetvalue(r, 0, 2));
  PQclear(r);
  unlist_if_automated(q, db, made);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { make_answer(q->fd, answer, made, lines); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(answer); free(made);
}

// ---- talking revises a realm's board (migrations/0050_realm_edit.sql): asked here, the model's
// answer kept by a child, as a plan is. ?as=lines answers the board's rows as lines.
static void revise_answer(int fd, const char *asked, const char *request, bool lines, bool ask) {
  const char *model = env_or("OA_BUILDER_MODEL", "gemini-2.5-flash");
  char *got = NULL;
  double t0 = api_now_ms();
  long status = ask_builder(model, request, &got);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[6] = {asked, http, got ? got : "", model, taken, ask ? "ask" : "revise"};
  PGresult *r = db ? PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines' FROM verse.realm_edit_finish($1::jsonb, $2::int, $3, $4, $5::int, $6) x",
                                  6, NULL, v, NULL, NULL, 0) : NULL;
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

// ask: an ADD from the feature list (migrations/0078_realm_asks.sql): the same revision, kept as an
// ask with what it changed; it always answers lines.
static void revise(const request *q, PGconn *db, const char *slug, const char *key, const char *admin, bool ask) {
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *v[6] = {slug, body, env_or("OA_BUILDER_DAILY_CAP", "3000"), key, admin, ask ? "ask" : "revise"};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'request' - 'status' - 'lines')::text, (x->'request')::text, "
                                "(x - 'request' - 'status' - 'error' - 'lines')::text, x->>'lines' "
                                "FROM verse.realm_edit_start($1, $2::jsonb, $3::int, $4, $5::boolean, $6) x", 6, NULL, v, NULL, NULL, 0);
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
  bool lines = ask || as_lines(q);
  if (status == 200 && PQgetisnull(r, 0, 2)) {   // kept, but the model can't be asked today: it waits
    const char *t = PQgetisnull(r, 0, 4) ? "" : PQgetvalue(r, 0, 4);
    api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, t, strlen(t));
    PQclear(r);
    return;
  }
  if (status != 200) {
    if (lines) answer_lines(q->fd, status, PQgetvalue(r, 0, 1), NULL); else api_json(q->fd, status, PQgetvalue(r, 0, 1));
    PQclear(r);
    return;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { revise_answer(q->fd, asked, request, lines, ask); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
}

// ---- a design document: its text
static uint32_t le16(const unsigned char *p) { return (uint32_t)p[0] | (uint32_t)p[1] << 8; }
static uint32_t le32(const unsigned char *p) { return le16(p) | le16(p + 2) << 16; }

// A .docx's word/document.xml, inflated: malloc'd, NUL-ended, or NULL.
static char *docx_xml(const unsigned char *z, size_t n, size_t *out_n) {
  if (n < 22) return NULL;
  size_t e = n - 22;
  while (e > 0 && le32(z + e) != 0x06054b50) { if (n - e > 22 + 65535) return NULL; e--; }
  if (le32(z + e) != 0x06054b50) return NULL;
  size_t entries = le16(z + e + 10), at = le32(z + e + 16);
  static const char want[] = "word/document.xml";
  for (size_t i = 0; i < entries && at + 46 <= n; i++) {
    if (le32(z + at) != 0x02014b50) return NULL;
    uint32_t method = le16(z + at + 10), packed = le32(z + at + 20), size = le32(z + at + 24);
    uint32_t nl = le16(z + at + 28), xl = le16(z + at + 30), cl = le16(z + at + 32), local = le32(z + at + 42);
    if (at + 46 + nl > n) return NULL;
    if (nl == sizeof want - 1 && !memcmp(z + at + 46, want, nl)) {
      if (local + 30 > n || le32(z + local) != 0x04034b50 || size > 8u << 20) return NULL;
      size_t data = local + 30 + le16(z + local + 26) + le16(z + local + 28);
      if (data > n || packed > n - data) return NULL;
      char *xml = malloc((size_t)size + 1);
      if (!xml) return NULL;
      if (method == 0 && packed == size) memcpy(xml, z + data, size);
      else if (method == 8) {
        z_stream st = {0};
        if (inflateInit2(&st, -15) != Z_OK) { free(xml); return NULL; }
        st.next_in = (Bytef *)(z + data); st.avail_in = packed;
        st.next_out = (Bytef *)xml; st.avail_out = size;
        int rc = inflate(&st, Z_FINISH);
        size_t got = size - st.avail_out;
        inflateEnd(&st);
        if (rc != Z_STREAM_END || got != size) { free(xml); return NULL; }
      }
      else { free(xml); return NULL; }
      xml[size] = 0; *out_n = size;
      return xml;
    }
    at += 46 + nl + xl + cl;
  }
  return NULL;
}
// Its words: a paragraph a line, a tab or break a space or a line; tags out, the five entities read.
static size_t docx_text(const char *x, size_t n, char *out, size_t cap) {
  size_t m = 0;
  for (size_t i = 0; i < n && m + 1 < cap; ) {
    if (x[i] == '<') {
      const char *end = memchr(x + i, '>', n - i);
      if (!end) break;
      size_t len = (size_t)(end - (x + i)) + 1;
      if ((len >= 6 && !strncmp(x + i, "</w:p>", 6)) || (len >= 7 && !strncmp(x + i, "<w:br", 5))) { if (m && out[m - 1] != '\n') out[m++] = '\n'; }
      else if (len >= 7 && !strncmp(x + i, "<w:tab", 6)) out[m++] = ' ';
      i += len;
      continue;
    }
    if (x[i] == '&') {
      static const char *names[5] = {"&amp;", "&lt;", "&gt;", "&quot;", "&apos;"};
      static const char chars[5] = {'&', '<', '>', '"', '\''};
      int hit = -1;
      for (int k = 0; k < 5; k++) if (!strncmp(x + i, names[k], strlen(names[k]))) hit = k;
      if (hit >= 0) { out[m++] = chars[hit]; i += strlen(names[hit]); continue; }
    }
    out[m++] = x[i++];
  }
  out[m] = 0;
  return m;
}

// {name, data}: the document as the SQL asks it, {name, text} or {name, pdf: base64}, malloc'd; or
// NULL with *status and *why set.
#define DOC_TEXT (60 * 1024)
static char *doc_read(PGconn *db, const char *body, int *status, const char **why) {
  const char *v[1] = {body};
  PGresult *r = PQexecParams(db, "SELECT coalesce(x->>'name', ''), decode(coalesce(x->>'data', ''), 'base64'), coalesce(x->>'data', ''), "
                                 "coalesce(x->'setup', 'null')::text FROM (SELECT $1::jsonb x) y", 1, NULL, v, NULL, NULL, 1);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); *status = 400; *why = "that isn't a file"; return NULL; }
  char name[200];
  int nn = PQgetlength(r, 0, 0);
  snprintf(name, sizeof name, "%.*s", nn < 199 ? nn : 199, PQgetvalue(r, 0, 0));
  const unsigned char *bytes = (const unsigned char *)PQgetvalue(r, 0, 1);
  size_t n = (size_t)PQgetlength(r, 0, 1);
  for (char *c = name; *c; c++) if (*c >= 'A' && *c <= 'Z') *c = (char)(*c - 'A' + 'a');   // its kind by its ending, any case
  size_t ln = strlen(name);
  bool md = (ln > 3 && !strcmp(name + ln - 3, ".md")) || (ln > 9 && !strcmp(name + ln - 9, ".markdown")) || (ln > 4 && !strcmp(name + ln - 4, ".txt"));
  bool docx = ln > 5 && !strcmp(name + ln - 5, ".docx"), pdf = ln > 4 && !strcmp(name + ln - 4, ".pdf");
  if (!md && !docx && !pdf) { PQclear(r); *status = 415; *why = "drop a .md, .txt, .pdf or .docx"; return NULL; }
  if (!n) { PQclear(r); *status = 400; *why = "that file is empty"; return NULL; }
  char *text = NULL;
  size_t tn = 0;
  if (md) {
    text = malloc(DOC_TEXT + 1);
    if (text) { tn = n < DOC_TEXT ? n : DOC_TEXT; while (tn < n && tn > 0 && (bytes[tn] & 0xC0) == 0x80) tn--; memcpy(text, bytes, tn); text[tn] = 0; for (size_t i = 0; i < tn; i++) if (!text[i]) text[i] = ' '; }
  } else if (docx) {
    size_t xn = 0;
    char *xml = docx_xml(bytes, n, &xn);
    if (!xml) { PQclear(r); *status = 400; *why = "that .docx couldn't be opened"; return NULL; }
    text = malloc(DOC_TEXT + 1);
    if (text) tn = docx_text(xml, xn, text, DOC_TEXT + 1);
    free(xml);
  }
  // the SQL's {name, text|pdf, setup}: a text's bad UTF-8 is its error (22), said as not text
  const char *w[6] = {PQgetvalue(r, 0, 0), text ? text : "", PQgetvalue(r, 0, 2), PQgetvalue(r, 0, 3), pdf ? "true" : "false", body};
  PGresult *j = PQexecParams(db, "SELECT jsonb_strip_nulls(jsonb_build_object('name', $1::text, 'text', CASE WHEN NOT $5::boolean THEN $2::text END, "
                                 "'pdf', CASE WHEN $5::boolean THEN $3::text END, 'setup', $4::jsonb)) "
                                 "|| CASE WHEN $6::jsonb ? 'rev' THEN jsonb_build_object('rev',$6::jsonb->'rev') ELSE '{}'::jsonb END", 6, NULL, w, NULL, NULL, 0);
  free(text);
  PQclear(r);
  if (PQresultStatus(j) != PGRES_TUPLES_OK || PQntuples(j) != 1) { PQclear(j); *status = 400; *why = "that file isn't text we can read"; return NULL; }
  char *out = strdup(PQgetvalue(j, 0, 0));
  PQclear(j);
  (void)tn;
  if (!out) { *status = 503; *why = "memory"; }
  return out;
}

// The model reads the document: a longer try than a sentence's.
static void doc_answer(int fd, const char *asked, const char *request, bool made_board) {
  const char *model = env_or("OA_BUILDER_MODEL", "gemini-2.5-flash");
  char *got = NULL;
  double t0 = api_now_ms();
  long limit = atol(env_or("OA_BUILDER_DOC_TIMEOUT_MS", "30000"));
  long status = npc_ask_model_twice("OA_BUILDER_URL", model, request, limit > 0 ? limit : 30000, &got);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, got ? got : "", model, taken};
  PGresult *r = db ? PQexecParams(db, made_board ? "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines' FROM verse.builder_answer($1::jsonb, $2::int, $3, $4, $5::int, NULL) x"
                                                 : "SELECT (x->>'status')::int, (x - 'status' - 'lines')::text, x->>'lines' FROM verse.realm_edit_finish($1::jsonb, $2::int, $3, $4, $5::int, 'ask-doc') x",
                                  5, NULL, v, NULL, NULL, 0) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) answer_lines(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1), PQgetisnull(r, 0, 2) ? NULL : PQgetvalue(r, 0, 2));
  else { api_log("realms: doc: %s", db ? PQerrorMessage(db) : "no database"); api_json(fd, 503, "{\"error\":\"not kept\"}"); }
  if (status != 200) api_log("realms: model answered %ld", status);
  if (r) PQclear(r);
  if (db) PQfinish(db);
  free(got);
}

// slug NULL: the builder's plan from the document; else an edit-mode list's.
static void doc(const request *q, PGconn *db, const char *slug, const char *key, const char *admin) {
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  int st = 0; const char *why = "";
  char *d = doc_read(db, body, &st, &why);
  free(body);
  if (!d) { char out[160]; snprintf(out, sizeof out, "{\"error\": \"%s\"}", why); answer_lines(q->fd, st, out, NULL); return; }
  const char *v[5] = {slug ? slug : "", d, env_or("OA_BUILDER_DAILY_CAP", "3000"), key, admin};
  PGresult *r = PQexecParams(db, slug ? "SELECT (x->>'status')::int, (x - 'request' - 'status' - 'lines')::text, (x->'request')::text, (x - 'request' - 'status' - 'error' - 'lines')::text FROM verse.realm_edit_start($1, $2::jsonb, $3::int, $4, $5::boolean, 'ask-doc') x"
                                      : "SELECT (x->>'status')::int, jsonb_build_object('error',x->'error')::text, (x->'request')::text, jsonb_build_object('id', x->'id', 'setup', x->'setup')::text FROM verse.builder_doc_ask($2::jsonb, $3::int, $4) x WHERE $1::text IS NOT NULL AND $5::boolean IS NOT NULL",
                             5, NULL, v, NULL, NULL, 0);
  free(d);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("realms: doc ask: %s", PQerrorMessage(db));
    PQclear(r);
    answer_lines(q->fd, 500, "{\"error\":\"not asked\"}", NULL);
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200 || PQgetisnull(r, 0, 2)) {
    answer_lines(q->fd, status == 200 ? 503 : status, PQgetvalue(r, 0, 1), NULL);
    PQclear(r);
    return;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { doc_answer(q->fd, asked, request, !slug); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
}

static bool slug_ok(const char *s, size_t n) {
  if (!n || n > 48) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9') || s[i] == '-')) return false;
  return true;
}

// After a visibility change: its page written (unlisted, public) or removed (private) by a child,
// which then answers {..., page: true|false}, so the answer comes when the bucket says so.
// A visibility change's answer, after its page is written (unlisted, public) or removed (private).
static void answer_paged(int fd, const char *slug, const char *json) {
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid < 0) { api_json(fd, 503, "{\"error\":\"busy\"}"); return; }
  if (pid > 0) return;
  PGconn *c = api_db_new();
  bool paged = c && write_page_or_remove(c, slug, true);
  if (c) PQfinish(c);
  size_t n = strlen(json);
  char *out = malloc(n + 32);
  if (!out) { api_json(fd, 503, "{\"error\":\"memory\"}"); _exit(0); }
  memcpy(out, json, n - 1);   // an object: its closing brace last
  snprintf(out + n - 1, 32, ", \"page\": %s}", paged ? "true" : "false");
  api_json(fd, 200, out);
  free(out);
  _exit(0);
}

// A private realm, to anyone but its owner and site admins, is not there (verse.realm_hidden,
// 0099): true when it answered 404 (or 500 when it couldn't ask).
static bool realm_hidden(const request *q, PGconn *db, const char *slug, const char *key, const char *admin) {
  const char *v[3] = {slug, key, admin};
  PGresult *r = PQexecParams(db, "SELECT verse.realm_hidden($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  bool hidden = ok && !strcmp(PQgetvalue(r, 0, 0), "t");
  if (!ok) api_log("realms: hidden: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) { api_json(q->fd, 500, "{\"error\":\"not read\"}"); return true; }
  if (hidden) api_json(q->fd, 404, "{\"error\":\"no such realm\"}");
  return hidden;
}

// Route metadata and bytes only; SQL locks, compares the optional revision and performs the edit.
static void realm_edit_call(const request *q, PGconn *db, const char *slug, const char *key,
                            const char *admin, const char *op, const char *id) {
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *v[6] = {slug, body, key, admin, op, id};
  sql_json(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_edit_call($1, $2::jsonb, $3, $4::boolean, $5, $6) x",
           6, v, op);
  free(body);
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
  // A NIP-98 signature (a CLI or an agent) is its key's account: its player key comes from SQL
  // (auth.caller_key), so every rule below is the one the page's key meets. Forged, unbound or
  // replayed: 401, never the player key instead (caller-policy.coax).
  char signer[65] = "";
  int sig = access_signed(q, db, signer);
  if (sig < 0) return true;
  if (sig > 0 && !access_caller_key(db, signer, key)) { api_json(q->fd, 500, "{\"error\":\"not read\"}"); return true; }
  const char *by = sig > 0 ? signer : NULL;
  if (!*rest) {
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST a board\"}"); return true; }
    if (access_refused(q, db, key, by)) return true;   // realm_invites on: approved creators only
    make(q, db, key);
    return true;
  }
  if (!strcmp(rest, "plan/doc")) {
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST a document\"}"); return true; }
    if (access_refused(q, db, key, by)) return true;
    doc(q, db, NULL, key, "false");
    return true;
  }
  if (!strcmp(rest, "plan") || !strcmp(rest, "pick")) {
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST a prompt\"}"); return true; }
    if (access_refused(q, db, key, by)) return true;
    plan(q, db, key, !strcmp(rest, "pick"));
    return true;
  }
  if (!strcmp(rest, "mine")) {   // the caller's realms (0098): "realm SLUG|REV|PUBLISHED|LISTED|NAME" lines, or JSON
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET your realms\"}"); return true; }
    const char *v[1] = {key};
    PGresult *r = PQexecParams(db, "SELECT x->>'lines', (x->'body')::text FROM verse.realms_mine($1) x", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("realms: mine: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    else if (as_lines(q)) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else api_json(q->fd, 200, PQgetvalue(r, 0, 1));
    PQclear(r);
    return true;
  }
  if (!strcmp(rest, "made")) {   // /og's rows ("made" is a reserved address: no realm has it)
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the rows\"}"); return true; }
    char *rows = sql_one(db, "SELECT verse.builder_og_rows(60)", 0, NULL);
    if (rows) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, rows, strlen(rows));
    else api_json(q->fd, 500, "{\"error\":\"not read\"}");
    free(rows);
    return true;
  }
  if (!strcmp(rest, "listed")) {   // /realms' rows of public made realms (0101; "listed" is reserved too)
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the rows\"}"); return true; }
    char *rows = sql_one(db, "SELECT verse.realm_list_rows(60)", 0, NULL);
    if (rows) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, rows, strlen(rows));
    else api_json(q->fd, 500, "{\"error\":\"not read\"}");
    free(rows);
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
  const char *admin = (by ? admin_is(by) : admin_who(q, db, who) == 0) ? "true" : "false";
  if (realm_hidden(q, db, slug, key, admin)) return true;   // private (0099): 404 to all but its owner and site admins
  bool edit_v1 = false;
  // A distinct family cannot silently downgrade when a POST reaches an older instance during
  // rollout: that instance answers unknown-route 404. The old paths retain optional fences.
  if (!strncmp(what, "edit-v1/", 8)) {
    edit_v1 = true;
    what += 8;
    bool known = !strcmp(what, "switch") || !strcmp(what, "settings") || !strcmp(what, "place")
              || !strcmp(what, "lore") || !strcmp(what, "quest") || !strcmp(what, "visibility") || !strcmp(what, "publish")
              || !strcmp(what, "revise") || !strcmp(what, "asks") || !strncmp(what, "asks/", 5)
              || !strncmp(what, "rows/", 5) || !strncmp(what, "proposals/", 10);
    if (!known) { api_json(q->fd, 404, "{\"error\":\"unknown edit\"}"); return true; }
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST an edit\"}"); return true; }
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[1] = {body};
    PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_edit_required($1::jsonb) x", 1, NULL, v, NULL, NULL, 0);
    free(body);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
      PQclear(r); api_json(q->fd, 400, "{\"error\":\"not json\"}"); return true;
    }
    int status = atoi(PQgetvalue(r, 0, 0));
    if (status != 200) api_json(q->fd, status, PQgetvalue(r, 0, 1));
    PQclear(r);
    if (status != 200) return true;
  }
  if (!strcmp(what, "visibility")) {   // who sees it (0099): its owner when an approved creator, or a site admin
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST {visibility}\"}"); return true; }
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[4] = {slug, body, key, admin};
    PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_edit_call($1, $2::jsonb, $3, $4::boolean, 'visibility', '') x", 4, NULL, v, NULL, NULL, 0);
    free(body);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      int status = atoi(PQgetvalue(r, 0, 0));
      if (status == 200) answer_paged(q->fd, slug, PQgetvalue(r, 0, 1));   // written for unlisted and public, removed for private
      else api_json(q->fd, status, PQgetvalue(r, 0, 1));
    } else {
      const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      bool theirs = state && !strncmp(state, "22", 2);
      if (!theirs) api_log("realms: visibility: %s", PQerrorMessage(db));
      api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    }
    PQclear(r);
    return true;
  }
  if (!*what) {
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the realm\"}"); return true; }
    const char *v[3] = {slug, key, admin};
    PGresult *r = PQexecParams(db, "SELECT verse.realm_edit_spec($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    text_answer(q->fd, r);
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "doc")) {   // the document now and its hash (0093)
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the document\"}"); return true; }
    const char *v[3] = {slug, key, admin};   // the owner's now, anyone else's the revision published (0094)
    sql_json(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_doc_of($1, $2, $3::boolean) x", 3, v, "doc");
    return true;
  }
  if (!strcmp(what, "published")) {   // the publish record: revision, document hash, engine and its hash (0107)
    if (post) { api_json(q->fd, 405, "{\"error\":\"GET the record\"}"); return true; }
    const char *v[1] = {slug};
    sql_json(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_published_of($1) x", 1, v, "published");
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
  if (!strcmp(what, "asks") && !post) {   // what its owner and admins asked for (0078): "" for anyone else
    const char *v[3] = {slug, key, admin};
    PGresult *r = PQexecParams(db, "SELECT verse.ask_list($1, $2, $3::boolean)", 3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), strlen(PQgetvalue(r, 0, 0)));
    else { api_log("realms: asks: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not read\"}"); }
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "card")) { card(q, db, slug, key, admin, post); return true; }
  if (!strcmp(what, "fork")) {   // a fork is a new realm: behind the same gate (its GET is not)
    if (post && access_refused(q, db, key, by)) return true;
    fork_realm(q, db, slug, key, admin, post);
    return true;
  }
  if (!post) { api_json(q->fd, 405, "{\"error\":\"POST it\"}"); return true; }
  if (!strcmp(what, "describe")) {
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[4] = {slug, body, key, admin};
    PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.builder_describe($1, $2::jsonb, $3, $4::boolean) x", 4, NULL, v, NULL, NULL, 0);
    free(body);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      int status = atoi(PQgetvalue(r, 0, 0));
      if (status == 200) rewrite_page(slug);   // the page says its new line at once
      api_json(q->fd, status, PQgetvalue(r, 0, 1));
    }
    else {
      const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      bool theirs = state && !strncmp(state, "22", 2);
      if (!theirs) api_log("realms: describe: %s", PQerrorMessage(db));
      api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    }
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "publish")) {   // a fork listed, its page written again (0075); a made realm's engine pinned (0107)
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    char *template = engine_template();
    const char *v[6] = {slug, body, key, admin, edit_v1 ? "true" : "false", template};
    PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_publish_pinned($1, $2::jsonb, $3, $4::boolean, $5::boolean, $6) x",
                               6, NULL, v, NULL, NULL, 0);
    free(body); free(template);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      int status = atoi(PQgetvalue(r, 0, 0));
      if (status == 200) rewrite_page(slug);
      api_json(q->fd, status, PQgetvalue(r, 0, 1));
    }
    else {
      const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      bool theirs = state && !strncmp(state, "22", 2);
      if (!theirs) api_log("realms: publish: %s", PQerrorMessage(db));
      api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    }
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "save") || !strcmp(what, "undo")) {   // only from the revision there now (0093, 0094)
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[4] = {slug, body, key, admin};
    PGresult *r = PQexecParams(db, !strcmp(what, "save") ? "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_save($1, $2::jsonb, $3, $4::boolean) x"
                                                         : "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.realm_undo($1, $2::jsonb, $3, $4::boolean) x", 4, NULL, v, NULL, NULL, 0);
    free(body);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
    }
    else {
      const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      bool theirs = state && !strncmp(state, "22", 2);
      if (!theirs) api_log("realms: %s: %s", what, PQerrorMessage(db));
      api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
    }
    PQclear(r);
    return true;
  }
  if (!strcmp(what, "revise")) { revise(q, db, slug, key, admin, false); return true; }
  if (!strcmp(what, "asks")) { revise(q, db, slug, key, admin, true); return true; }
  if (!strcmp(what, "asks/doc")) { doc(q, db, slug, key, admin); return true; }
  if (!strcmp(what, "lore") || !strcmp(what, "quest")) {
    realm_edit_call(q, db, slug, key, admin, what, "");
    return true;
  }
  if (!strcmp(what, "settings") || !strcmp(what, "place") || !strcmp(what, "switch")) {
    realm_edit_call(q, db, slug, key, admin, what, "");
    return true;
  }
  // DELETE an ask, an agent's done for one, or DELETE a board's row (0078)
  static const char asks[] = "asks/", rows[] = "rows/";
  if (!strncmp(what, asks, sizeof asks - 1) || !strncmp(what, rows, sizeof rows - 1)) {
    bool row = what[0] == 'r';
    const char *id = what + (row ? sizeof rows - 1 : sizeof asks - 1), *end = strchr(id, '/');
    char name[64];
    size_t k = end ? (size_t)(end - id) : 0;
    bool done = end && !row && !strcmp(end, "/done");
    bool ok = end && k > 0 && k < (row ? sizeof name : 19) && (done || !strcmp(end, "/delete"));
    for (size_t i = 0; ok && i < k; i++) ok = row ? ((id[i] >= 'a' && id[i] <= 'z') || (id[i] >= '0' && id[i] <= '9') || id[i] == '-' || id[i] == ':') : (id[i] >= '0' && id[i] <= '9');
    if (!ok) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
    memcpy(name, id, k); name[k] = 0;
    if (done) {
      realm_edit_call(q, db, slug, key, admin, "ask-done", name);
      return true;
    }
    realm_edit_call(q, db, slug, key, admin, row ? "row-delete" : "ask-delete", name);
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
    realm_edit_call(q, db, slug, key, admin, !strcmp(end, "/accept") ? "accept" : "decline", num);
    return true;
  }
  api_json(q->fd, 404, "{\"error\":\"unknown\"}");
  return true;
}
