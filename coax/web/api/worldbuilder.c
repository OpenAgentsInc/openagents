// WorldBuilder (openagents.com/create-world, apps/openagents-web/worldbuilder.coax): someone types
// what they want in their world and the model answers typed edits the page carries out.
//
//   POST /api/worldbuilder/act   the page's JSON (what was typed, the tapped spot, the camera, what is
//                                in the world): never read here, handed to the model as the user's turn
//                                -> 200 lines (worldbuilder.lines in migrations/0141_worldbuilder.sql):
//                                   "S say" then "A op|asset|count|..." an edit a line (a ground edit's
//                                   asset is its layer, the answer's `layer`: worldbuilder-land.coax,
//                                   migrations/0159_worldbuilder_layer.sql)
//                                   429 "flood WAIT_MS": this address sent too many at once
//                                   429 "resting": the day's $50 is spent, until midnight Chicago time
//                                   400 nothing typed or too long; 502 the model didn't answer
//   GET  /api/worldbuilder/status  "ok SPENT_MICROS CAP_MICROS" or "resting SPENT_MICROS CAP_MICROS"
//
// No sign-in and no per-person limit (owner, 2026-11-11): one cap for everyone, $50 a day of model
// spend, and anti-flood by address (X-Forwarded-For's first, as analytics.c reads it), both decided
// by worldbuilder-policy.coax (#authority) and both shown on the page. A call reserves the most it
// could cost before it starts, in the same transaction that reads the day's total, so calls running
// side by side never go past the cap together; it settles at what Vertex reports it used.
//
// The model is Google's: Gemini 4 Hydrogen on Vertex (OA_WORLDBUILDER_MODEL), structured output under
// a schema, its thinking low. worldbuilder-wire.coax writes that request, the schema in it, and the
// status and flood lines; this file builds no body of its own. The catalogue it chooses from
// (worldbuilder-assets.h) is written with the page's asset pack by
// metaverse/tools/assets/worldbuilder_pack.coax. A call takes
// seconds and this service answers one request at a time, so a child process makes it, settles the
// spend on its own database connection and answers the page.
//   OA_WORLDBUILDER_URL    the generateContent URL (tests point it at a fake)
//   OA_WORLDBUILDER_MODEL  the model (gemini-4-hydrogen)
#include "api.h"
#include "worldbuilder-assets.h"
#include "worldbuilder-policy.generated.h"
#include "worldbuilder-wire.generated.h"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define WB_BODY_MAX 16384   // the page's request, at most

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

static const char WB_SYSTEM[] =
  "You turn what someone types into edits of a 3D world they build on an empty grid. Units are metres; "
  "x runs to the right and z toward the viewer at the start; y is up. Answer only with the JSON the schema "
  "allows: `say`, one short friendly line (at most 60 characters), and `actions`, the edits in order.\n"
  "Edits (op):\n"
  "- place: put `count` (default 1) of `asset`, an exact name from the catalogue (never a ground layer). Where: `anchor`, `side` and "
  "`distance`. Several (count > 1) spread within `spread` metres of that spot (0: automatic). `size` scales "
  "them (1 = as made). For natural things mix variants: several place edits with different names (birch-1, "
  "birch-3, ...). Up to 200 things per edit.\n"
  "- move: move `targets` to `anchor`/`side`/`distance`; anchor \"self\" moves each from where it is.\n"
  "- rotate: turn `targets` by `degrees` (positive turns left, seen from above).\n"
  "- scale: multiply the size of `targets` by `factor` (bigger 1.5, much bigger 2.5, smaller 0.66).\n"
  "- delete: remove `targets`. duplicate: copy `targets` beside them (or at anchor/side when given).\n"
  "- undo: take back the last edit (`count` edits). redo: put back an undone edit (`count` edits). clear: remove everything (only when they clearly ask to start over).\n"
  "The ground is a 256 m square round the grid's middle. It starts as bare, level grid and can be painted and shaped:\n"
  "- paint: cover ground with a layer. `layer` is the layer: grass, dirt, sand, stone, snow, water, or grid (bare "
  "again). `factor` 0.1 to 1 is how strongly (default 1). Every edit has a `layer`; only paint reads it (give grid "
  "on the others).\n"
  "- raise / lower: a hill or a hollow, a round patch where `anchor`/`side` say (never `targets` \"all\"). `size` is "
  "its height or depth in metres (default 4 and 3; at most 40).\n"
  "- smooth: soften the ground's shape. flatten: level it to the height at the spot, or to `size` metres when given.\n"
  "Where a ground edit acts: `targets` \"all\" = the whole ground (cover everything in grass); `anchor` \"edge\" with "
  "`side` = a strip along that edge of the world as they see it, `spread` metres wide (a beach along the left); "
  "otherwise a round patch, `spread` metres in radius (default 8 to 10), at `anchor`/`side`/`distance` as for place "
  "(a hill here; flatten around the house: anchor \"named\" with the house in `anchor_names`, where `spread` is the "
  "margin round it). A pond or lake is a lower and then a paint of water on the same spot, the paint a little "
  "smaller. A path or road is a few paint edits of dirt in a row. Things already placed stay on the ground as it "
  "changes. To start the ground over: flatten with targets \"all\" and size 0, then paint grid with targets \"all\".\n"
  "Targets: \"last\" = what the last edit placed or changed (it, that, them, those); \"names\" = every object "
  "whose asset is in `names` (exact names from the world's present list); \"nearest\" = the one object nearest "
  "the anchor spot (among `names` when given); \"all\" = everything.\n"
  "Anchors: \"here\" = the spot they tapped (the request's here; when it is null use \"view\"); \"view\" = the "
  "ground in the middle of their view; \"last\" = the middle of what the last edit touched; \"named\" = the "
  "newest object whose asset is in `anchor_names`; \"origin\" = the middle of the grid; \"self\" = each target's "
  "own place.\n"
  "Always set `anchor` on place, move and duplicate. Something already in the world (it, the tree, them) is "
  "\"named\" with its asset in `anchor_names`, or \"last\" when it is what the last edit touched. Within one "
  "answer, \"last\" is what the edit just before placed or changed.\n"
  "Sides are as seen from their camera: left, right, front (toward them), behind (away from them: further "
  "back), around (a ring round the anchor), at (on the spot). `distance` is metres beyond the anchor's edge "
  "(0: just clear of it).\n"
  "When `walking` is true they are walking their character on the ground: `here` is where it stands, the camera is "
  "just behind it and `looks` is the way they face. Then \"me\", \"I\" and \"you\" are that character: anchor "
  "\"here\" with side front (ahead of them, in front of me), left, right or behind (behind me), or at (just ahead, "
  "where they can see it). \"Ahead\" or \"in front of me\" is side front.\n"
  "The user's message is JSON: text (what they typed), walking, here ([x, z] or null), cam (x, z and the way it looks), "
  "present (asset names in the world and how many), recent (the newest objects: id, asset, x, z, size) and "
  "last (the asset names the last edit touched).\n"
  "Prefer assets whose name and tags fit the words. If nothing fits, place the closest thing and say so. "
  "If the text isn't about the world, make no edits and answer in `say`.\n"
  "For a house, home or building use a whole building (house/...); the village/ pieces are parts of buildings.\n"
  "Catalogue (name|width,height,depth in metres|tags):\n";
// The whole buildings: the village kit's pieces put together by the page
// (apps/openagents-web/worldbuilder/worldbuilder-plan.coax WB_PREFABS, the same names and sizes).
static const char WB_BUILDINGS[] =
  "house/cottage|5.51,7.25,5.56|house home cottage building plaster\n"
  "house/timber-house|5.53,7.09,5.56|house home building timber wood\n"
  "house/long-house|5.51,7.23,7.57|house home building long hall brick\n";

static coax_string cs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }

// ---- anti-flood: a bucket per address (worldbuilder-policy.coax wb_take decides), kept as a row in
// Postgres (auth.limit_buckets, migration 0173) so it is one bucket for every worker and instance
// (#13300). The row is locked while the policy takes from it. The key kept is a digest.
// 0 to go ahead, else the milliseconds to wait. A bucket that can't be read refuses nobody.
static long wb_flood(const char *ip) {
  PGconn *db = api_db();
  if (!db) return 0;
  char full[24]; snprintf(full, sizeof full, "%lld", (long long)wb_bucket_new(0).fill_ms);
  const char *v[3] = {"worldbuilder", ip, full};
  PGresult *r = PQexec(db, "BEGIN"); PQclear(r);
  r = PQexecParams(db, "SELECT fill_ms, at_ms, now_ms, left(encode(sha256(convert_to($2, 'UTF8')), 'hex'), 40) "
                       "FROM auth.bucket_lock($1, left(encode(sha256(convert_to($2, 'UTF8')), 'hex'), 40), $3::bigint)", 3, NULL, v, NULL, NULL, 0);
  long wait = 0;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    coax_t_WbBucket b = {0};
    b.fill_ms = atoll(PQgetvalue(r, 0, 0)); b.at_ms = atoll(PQgetvalue(r, 0, 1));
    b = wb_take(b, atoll(PQgetvalue(r, 0, 2)));
    wait = b.ok ? 0 : (long)b.wait_ms;
    char f[24], a[24], key[48];
    snprintf(f, sizeof f, "%lld", (long long)b.fill_ms); snprintf(a, sizeof a, "%lld", (long long)b.at_ms);
    snprintf(key, sizeof key, "%s", PQgetvalue(r, 0, 3));
    const char *u[4] = {"worldbuilder", key, f, a};
    PGresult *w = PQexecParams(db, "SELECT auth.bucket_put($1, $2, $3::bigint, $4::bigint)", 4, NULL, u, NULL, NULL, 0);
    PQclear(w);
  } else api_log("worldbuilder: bucket: %s", PQerrorMessage(db));
  PQclear(r);
  r = PQexec(db, "COMMIT"); PQclear(r);
  return wait;
}

static void wb_text(int fd, int status, const char *t) { api_respond(fd, status, "text/plain; charset=utf-8", NULL, t, strlen(t)); }

// The day's total spent so far (no row: nothing yet), or -1 on a database error.
static long long wb_spent(PGconn *db) {
  PGresult *r = PQexec(db, "SELECT coalesce((SELECT spent_micros FROM worldbuilder.days WHERE day = worldbuilder.today()), 0)");
  long long v = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? atoll(PQgetvalue(r, 0, 0)) : -1;
  PQclear(r);
  return v;
}
static void wb_status(const request *q) {
  PGconn *db = api_db();
  long long spent = db ? wb_spent(db) : -1;
  if (spent < 0) { wb_text(q->fd, 503, "unknown"); return; }
  char out[96];
  int64_t n = wbw_status_write(wb_resting(spent), spent, wb_cap_micros(), (coax_v_u8){(uint8_t *)out, sizeof out - 1});
  if (n < 0) { wb_text(q->fd, 503, "unknown"); return; }
  out[n] = 0;
  wb_text(q->fd, 200, out);
}

// Reserves `reserve` of today's spend if the cap allows: 1 reserved (day into day), 0 resting, -1 an error.
static int wb_reserve(PGconn *db, long long reserve, char day[16]) {
  PGresult *r = PQexec(db, "BEGIN");
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK; PQclear(r);
  if (!ok) return -1;
  r = PQexec(db, "INSERT INTO worldbuilder.days (day) VALUES (worldbuilder.today()) ON CONFLICT DO NOTHING");
  ok = PQresultStatus(r) == PGRES_COMMAND_OK; PQclear(r);
  long long spent = -1;
  if (ok) {
    r = PQexec(db, "SELECT spent_micros, day::text FROM worldbuilder.days WHERE day = worldbuilder.today() FOR UPDATE");
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { spent = atoll(PQgetvalue(r, 0, 0)); snprintf(day, 16, "%s", PQgetvalue(r, 0, 1)); }
    PQclear(r);
  }
  int out = -1;
  if (spent >= 0) {
    bool admit = wb_admit(spent, reserve);
    char rs[32]; snprintf(rs, sizeof rs, "%lld", reserve);
    const char *v[2] = {day, rs};
    r = admit ? PQexecParams(db, "UPDATE worldbuilder.days SET spent_micros = spent_micros + $2::bigint, calls = calls + 1 WHERE day = $1::date", 2, NULL, v, NULL, NULL, 0)
              : PQexecParams(db, "UPDATE worldbuilder.days SET refused = refused + 1 WHERE day = $1::date", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_COMMAND_OK) out = admit ? 1 : 0;
    PQclear(r);
  }
  r = PQexec(db, out >= 0 ? "COMMIT" : "ROLLBACK");
  if (PQresultStatus(r) != PGRES_COMMAND_OK) out = -1;
  PQclear(r);
  return out;
}

// In the child: asks the model, settles the spend, answers the page.
static void wb_answer(int fd, const char *request_json, long long reserve, const char *day) {
  const char *model = env_or("OA_WORLDBUILDER_MODEL", "gemini-4-hydrogen");
  double t0 = api_now_ms();
  char *got = NULL; size_t got_n = 0;
  long status = npc_ask_model_twice_n("OA_WORLDBUILDER_URL", model, request_json, 20000, &got, &got_n);
  int ms = (int)(api_now_ms() - t0);
  PGconn *db = api_db_new();
  long long input = 0, cached = 0, output = 0;
  char *lines = NULL;
  if (db && status == 200 && got) {
    const char *v[1] = {got};
    PGresult *r = PQexecParams(db,
      "SELECT coalesce((b #>> '{usageMetadata,promptTokenCount}')::bigint, -1), coalesce((b #>> '{usageMetadata,cachedContentTokenCount}')::bigint, 0), "
      "coalesce((b #>> '{usageMetadata,candidatesTokenCount}')::bigint, 0) + coalesce((b #>> '{usageMetadata,thoughtsTokenCount}')::bigint, 0), "
      "worldbuilder.lines(worldbuilder.answer_text(b)) FROM (SELECT $1::jsonb b) x", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      input = atoll(PQgetvalue(r, 0, 0)); cached = atoll(PQgetvalue(r, 0, 1)); output = atoll(PQgetvalue(r, 0, 2));
      if (!PQgetisnull(r, 0, 3)) lines = strdup(PQgetvalue(r, 0, 3));
    } else input = -1;
    PQclear(r);
  }
  // What it cost: Vertex's usage at its rates; usage that can't be keeps the reservation; no answer costs nothing.
  long long cost = status == 200 ? wb_cost_micros(input, cached, output) : 0;
  if (cost < 0) cost = reserve;
  long long total = -1;
  if (db) {
    char rs[32], cs[32], st[16], mss[16], in[32], ca[32], ou[32];
    snprintf(rs, sizeof rs, "%lld", reserve); snprintf(cs, sizeof cs, "%lld", cost); snprintf(st, sizeof st, "%ld", status);
    snprintf(mss, sizeof mss, "%d", ms); snprintf(in, sizeof in, "%lld", input < 0 ? 0 : input); snprintf(ca, sizeof ca, "%lld", cached); snprintf(ou, sizeof ou, "%lld", output);
    const char *v[3] = {day, rs, cs};
    PGresult *r = PQexecParams(db, "UPDATE worldbuilder.days SET spent_micros = greatest(0, spent_micros - $2::bigint + $3::bigint) WHERE day = $1::date RETURNING spent_micros",
                               3, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) total = atoll(PQgetvalue(r, 0, 0));
    PQclear(r);
    const char *w[8] = {day, st, mss, in, ca, ou, rs, cs};
    r = PQexecParams(db, "INSERT INTO worldbuilder.calls (day, status, ms, input, cached, output, reserved, micros) VALUES ($1::date, $2::int, $3::int, $4::int, $5::int, $6::int, $7::bigint, $8::bigint)",
                     8, NULL, w, NULL, NULL, 0);
    PQclear(r);
  }
  api_log("worldbuilder: %s answered %ld in %d ms; %lld in (%lld cached), %lld out; %lld micros, the day's total %lld micros (%s)",
          model, status, ms, input, cached, output, cost, total, day);
  if (lines) wb_text(fd, 200, lines);
  else wb_text(fd, 502, "no answer");
  free(lines); free(got);
  if (db) PQfinish(db);
}

static void wb_act(const request *q) {
  if (q->body_len == 0 || q->body_len > WB_BODY_MAX) { wb_text(q->fd, 400, "too long"); return; }
  char ip[128];
  api_header(q, "X-Forwarded-For", ip, sizeof ip);
  char *comma = strchr(ip, ','); if (comma) *comma = 0;
  long wait = wb_flood(ip[0] ? ip : "unknown");
  if (wait > 0) {
    char out[48];
    int64_t n = wbw_flood_write(wait, (coax_v_u8){(uint8_t *)out, sizeof out - 1});
    out[n < 0 ? 0 : n] = 0;
    wb_text(q->fd, 429, out);
    return;
  }
  // the request: the system turn with the catalogue, the page's JSON as the user's turn, the schema
  // (worldbuilder-wire.coax writes it)
  int64_t room = wbw_request_room((int64_t)(strlen(WB_SYSTEM) + strlen(WB_BUILDINGS) + strlen(WB_ASSETS) + q->body_len));
  struct { char *p; size_t n; } s = {room < 0 ? NULL : malloc((size_t)room + 1), 0};
  if (!s.p) { wb_text(q->fd, 503, "busy"); return; }
  int64_t wrote = wbw_request_write(cs(WB_SYSTEM), cs(WB_BUILDINGS), cs(WB_ASSETS), (coax_string){q->body, (int64_t)q->body_len},
                                    wb_max_output(), (coax_v_u8){(uint8_t *)s.p, room});
  if (wrote < 0) { free(s.p); wb_text(q->fd, 503, "busy"); return; }
  s.n = (size_t)wrote; s.p[s.n] = 0;
  PGconn *db = api_db();
  if (!db) { free(s.p); wb_text(q->fd, 503, "busy"); return; }
  // about three bytes a token for the prompt: the reservation's prompt part, before Vertex counts it
  long long reserve = wb_reserve_micros((int64_t)(s.n / 3));
  char day[16] = "";
  int got = wb_reserve(db, reserve, day);
  if (got < 0) { api_log("worldbuilder: reserve: %s", PQerrorMessage(db)); free(s.p); wb_text(q->fd, 503, "busy"); return; }
  if (got == 0) { free(s.p); wb_text(q->fd, 429, "resting"); return; }
  signal(SIGCHLD, SIG_IGN);   // children are never waited for
  pid_t pid = fork();
  if (pid == 0) { wb_answer(q->fd, s.p, reserve, day); _exit(0); }
  if (pid < 0) {
    const char *v[2] = {day, ""}; char rs[32]; snprintf(rs, sizeof rs, "%lld", reserve); v[1] = rs;
    PGresult *r = PQexecParams(db, "UPDATE worldbuilder.days SET spent_micros = greatest(0, spent_micros - $2::bigint) WHERE day = $1::date", 2, NULL, v, NULL, NULL, 0);
    PQclear(r);
    wb_text(q->fd, 503, "busy");
  }
  free(s.p);
}

bool worldbuilder_route(const request *q) {
  if (!strcmp(q->path, "/api/worldbuilder/act") && !strcmp(q->method, "POST")) { wb_act(q); return true; }
  if (!strcmp(q->path, "/api/worldbuilder/status") && !strcmp(q->method, "GET")) { wb_status(q); return true; }
  return false;
}
