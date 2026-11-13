// Character chat for the Metaverse playtest (pt-chat-endpoint): the demo's
// POST /api/npc/ask (aethyr-cpp tools/npc_chat.py) as
//
//   POST /api/playtest/npc/ask  {npc: ID, world?: WORLD, realm?: SLUG, visit?, scene?, history: [{from, text}], question}
//                               X-Verse-Player: the browser's player key, required (#13261)
//                               -> 200 {text, model}; 400 no question, a persona or controls from the page;
//                               401 no player key; 404 no such character; 429 {error, limit, max, per, until_ms,
//                               retry_s} and Retry-After; 503 {error} when there is no answer (the page then
//                               says its own lines, as the demo's does)
// Who the character is comes from us, never the page: a built-in world's people are compiled in
// (npc-personas.coax, from their pages' own constants), a made realm's are its document's (SQL).
// A character remembers its player: what they said to it on earlier visits goes into its prompt
// (migrations/0031_npc_memory.sql verse.npc_ask; quoted since 0157).
//
// The prompt, the memory, the limits and the day's cap are SQL's (migrations/0005_verse_npc.sql
// verse.npc_request, verse.npc_answer, which also keeps every question and answer;
// 0157_npc_ask_keyed.sql verse.npc_ask, the limits counted in Postgres so every instance shares
// them). The numbers are NPC_LIMITS below, which /docs/limits is made from. The model is Google's
// first: Gemini on Vertex AI (OA_NPC_MODEL, default gemini-4-hydrogen on the global endpoint), with
// the service's own token from the metadata server. A model call takes seconds and this service
// answers one request at a time, so a child process makes it and answers, with its own database
// connection, while the parent goes on; how many run at once is one of the limits.
//   OA_NPC_URL    the generateContent URL (tests point it at a fake)
//   OA_NPC_TOKEN  a token to send instead of the metadata server's (tests)
//   OA_NPC_DAILY_CAP  answers a day across the server (2000)
//   OA_NPC_KEY_10M, OA_NPC_KEY_DAY, OA_NPC_ADDR_10M, OA_NPC_ADDR_DAY, OA_NPC_RUNNING, OA_NPC_RUNNING_KEY,
//   OA_NPC_RUNNING_ADDR  the limits, in place of NPC_LIMITS' numbers (tests)
#include "api.h"
#include <curl/curl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "npc-personas.generated.h"

#define PROJECT "openagentsgemini"

typedef struct { char *p; size_t n, cap; } buf;
static size_t take(char *data, size_t size, size_t n, void *user) {
  buf *b = user; size_t k = size * n;
  if (b->n + k + 1 > b->cap) {
    size_t cap = (b->n + k + 1) * 2;
    if (cap > (1u << 22)) return 0;
    char *p = realloc(b->p, cap);
    if (!p) return 0;
    b->p = p; b->cap = cap;
  }
  memcpy(b->p + b->n, data, k); b->n += k; b->p[b->n] = 0;
  return k;
}

// The service account's token, from the metadata server (or OA_NPC_TOKEN).
static bool token(char *out, size_t cap) {
  const char *given = getenv("OA_NPC_TOKEN");
  if (given && *given) { snprintf(out, cap, "%s", given); return true; }
  CURL *h = curl_easy_init();
  if (!h) return false;
  buf b = {0};
  struct curl_slist *hd = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(h, CURLOPT_URL, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token");
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 5L);
  bool ok = curl_easy_perform(h) == CURLE_OK && b.p;
  out[0] = 0;
  const char *t = ok ? strstr(b.p, "\"access_token\":\"") : NULL;
  if (t) {
    t += strlen("\"access_token\":\"");
    size_t n = 0;
    while (t[n] && t[n] != '"' && n + 1 < cap) { out[n] = t[n]; n++; }
    out[n] = 0;
  }
  curl_slist_free_all(hd); curl_easy_cleanup(h); free(b.p);
  return out[0] != 0;
}

// Asks the model (its generateContent URL from url_env, else Vertex's for
// model): the HTTP status (0 unreached) and the body into b.
static long ask_model_in(const char *url_env, const char *model, const char *request, buf *b, long timeout_ms);
static long ask_model(const char *url_env, const char *model, const char *request, buf *b) {
  return ask_model_in(url_env, model, request, b, 25000);
}
static long ask_model_in(const char *url_env, const char *model, const char *request, buf *b, long timeout_ms) {
  char url[512];
  const char *given = getenv(url_env);
  if (given && *given) snprintf(url, sizeof url, "%s", given);
  else snprintf(url, sizeof url, "https://aiplatform.us.rep.googleapis.com/v1/projects/%s/locations/us/publishers/google/models/%s:generateContent",
                PROJECT, model);
  char tok[4096], auth[4200];
  long status = 0;
  if (token(tok, sizeof tok)) {
    CURL *h = curl_easy_init();
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, request);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, b);
    curl_easy_setopt(h, CURLOPT_TIMEOUT_MS, timeout_ms);
    if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
  } else api_log("npc: no token");
  return status;
}

long npc_ask_model(const char *url_env, const char *model, const char *request, char **body) {
  buf b = {0};
  long status = ask_model(url_env, model, request, &b);
  *body = b.p;
  return status;
}

// As npc_ask_model, each try cut off at timeout_ms; an unanswered or failed try (the slow tail) is tried
// once more, as voice edits are.
long npc_ask_model_twice(const char *url_env, const char *model, const char *request, long timeout_ms, char **body) {
  size_t n = 0;
  return npc_ask_model_twice_n(url_env, model, request, timeout_ms, body, &n);
}

// Length-preserving transport for Coax byte readers: malformed JSON with an
// embedded NUL must not become a valid shorter prefix through strlen().
long npc_ask_model_twice_n(const char *url_env, const char *model, const char *request, long timeout_ms, char **body, size_t *body_n) {
  buf b = {0};
  long status = ask_model_in(url_env, model, request, &b, timeout_ms);
  if (status == 0 || status == 429 || status >= 500) {
    api_log("npc: %s answered %ld, trying again", model, status);
    free(b.p); b = (buf){0};
    status = ask_model_in(url_env, model, request, &b, timeout_ms);
  }
  *body = b.p;
  *body_n = b.n;
  return status;
}

// In the child: asks the model, keeps the answer, answers the page.
static void answer(int fd, const char *asked, const char *request) {
  const char *model = getenv("OA_NPC_MODEL"); if (!model || !*model) model = "gemini-4-hydrogen";
  buf b = {0};
  double t0 = api_now_ms();
  long status = ask_model("OA_NPC_URL", model, request, &b);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, b.p ? b.p : "", model, taken};
  PGresult *r = db ? PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.npc_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
                                  5, NULL, v, NULL, NULL, 0) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  else { api_log("npc: %s", db ? PQerrorMessage(db) : "no database"); api_json(fd, 503, "{\"error\":\"not kept\"}"); }
  if (status != 200) api_log("npc: model answered %ld", status);
  if (r) PQclear(r);
  if (db) PQfinish(db);
  free(b.p);
}

// The character chat's limits (#13261): what each is, as people read it, and its number. A real
// player asks a question every several seconds at most, while reading the answers between; these
// are well above that. /docs/limits is made from this table (api --limits).
typedef struct { const char *key, *what, *per, *env; int max; } npc_limit;
static const npc_limit NPC_LIMITS[] = {
  {"key_10m", "Questions to characters from one player", "in 10 minutes", "OA_NPC_KEY_10M", 60},
  {"key_day", "Questions to characters from one player", "a day", "OA_NPC_KEY_DAY", 300},
  {"addr_10m", "Questions to characters from one network address", "in 10 minutes", "OA_NPC_ADDR_10M", 120},
  {"addr_day", "Questions to characters from one network address", "a day", "OA_NPC_ADDR_DAY", 600},
  {"running_key", "Questions being answered at once for one player", "at once", "OA_NPC_RUNNING_KEY", 2},
  {"running_addr", "Questions being answered at once from one network address", "at once", "OA_NPC_RUNNING_ADDR", 8},
  {"running", "Questions being answered at once, all players", "at once", "OA_NPC_RUNNING", 24},
};
#define NPC_LIMIT_N (sizeof NPC_LIMITS / sizeof NPC_LIMITS[0])
#define NPC_DAILY_CAP 2000
#define NPC_BODY_MAX 24576       // a page's question, its scene and its last lines fit in a quarter of this
static int npc_limit_of(const npc_limit *l) {
  const char *v = getenv(l->env);
  return v && atoi(v) > 0 ? atoi(v) : l->max;
}
void npc_print_limits(FILE *out) {   // api --limits: the character chat's section of /docs/limits
  fprintf(out, "\n## Talking to characters\n\nEach question needs your player key. When you reach a limit, we say which and when it lifts.\n\n"
               "| What | Limit |\n| --- | --- |\n");
  for (size_t i = 0; i < NPC_LIMIT_N; i++) fprintf(out, "| %s | %d %s |\n", NPC_LIMITS[i].what, NPC_LIMITS[i].max, NPC_LIMITS[i].per);
  fprintf(out, "| Answers from characters, all players | %d a day |\n", NPC_DAILY_CAP);
  fprintf(out, "\nThis section is made from the server's own numbers (`coax/web/api/npc.c`).\n");
}

// A refusal from SQL: its fields, and for a limit how long until it lifts (Retry-After).
static void npc_refuse(int fd, int status, const char *json, const char *retry_s) {
  char hd[64] = "";
  if (status == 429 && retry_s && *retry_s) snprintf(hd, sizeof hd, "Retry-After: %d\r\n", atoi(retry_s));
  api_respond(fd, status, "application/json", hd[0] ? hd : NULL, json, strlen(json));
}

bool npc_route(const request *q) {
  if (strcmp(q->path, "/api/playtest/npc/ask")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a question\"}"); return true; }
  if (q->body_len > NPC_BODY_MAX) { api_json(q->fd, 413, "{\"error\":\"too large\"}"); return true; }
  char key[160] = "";
  api_header(q, "X-Verse-Player", key, sizeof key);
  if (!key[0]) {   // before any database work: no key, no question (SQL checks it is one we gave out)
    api_json(q->fd, 401, "{\"error\":\"Talking to characters needs a player key (X-Verse-Player). Reload the page to get one.\"}");
    return true;
  }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  // The character it names: a built-in world's from npc-personas.coax (a made realm's is SQL's).
  const char *bv[1] = {body};
  PGresult *w = PQexecParams(db, "SELECT CASE WHEN jsonb_typeof(b->'world') = 'string' THEN b->>'world' END, "
                                 "CASE WHEN jsonb_typeof(b->'npc') = 'string' THEN b->>'npc' END FROM (SELECT $1::jsonb AS b) x",
                             1, NULL, bv, NULL, NULL, 0);
  if (PQresultStatus(w) != PGRES_TUPLES_OK || PQntuples(w) != 1) {
    const char *state = PQresultErrorField(w, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("npc: %s", PQerrorMessage(db));
    PQclear(w); free(body);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not asked\"}");
    return true;
  }
  static uint8_t persona[16384];
  int64_t pn = -1;
  if (!PQgetisnull(w, 0, 0) && !PQgetisnull(w, 0, 1)) {
    const char *world = PQgetvalue(w, 0, 0), *id = PQgetvalue(w, 0, 1);
    pn = npc_persona((coax_string){world, (int64_t)strlen(world)}, (coax_string){id, (int64_t)strlen(id)},
                     (coax_v_u8){persona, (int64_t)sizeof persona - 1});
  }
  PQclear(w);
  if (pn > 0) persona[pn] = 0;
  char ip[128], cap[16], lim[512];
  auth_visitor(q, ip);
  const char *c = getenv("OA_NPC_DAILY_CAP");
  snprintf(cap, sizeof cap, "%d", c && atoi(c) > 0 ? atoi(c) : NPC_DAILY_CAP);
  size_t ln = (size_t)snprintf(lim, sizeof lim, "{");
  for (size_t i = 0; i < NPC_LIMIT_N; i++) ln += (size_t)snprintf(lim + ln, sizeof lim - ln, "%s\"%s\":%d", i ? "," : "", NPC_LIMITS[i].key, npc_limit_of(&NPC_LIMITS[i]));
  snprintf(lim + ln, sizeof lim - ln, "}");
  const char *v[6] = {body, pn > 0 ? (const char *)persona : NULL, cap, key, ip, lim};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status' - 'request')::text, (x->'request')::text, x->>'retry_s' "
                                 "FROM verse.npc_ask($1::jsonb, $2::jsonb, $3::int, $4, $5, $6::jsonb) x", 6, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("npc: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not asked\"}");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) {
    npc_refuse(q->fd, status, PQgetvalue(r, 0, 1), PQgetvalue(r, 0, 3));
    PQclear(r);
    return true;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 1));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);   // children are never waited for
  pid_t pid = fork();
  if (pid == 0) {             // the child answers; the parent's database connection is never touched here
    answer(q->fd, asked, request);
    _exit(0);
  }
  if (pid < 0) {              // no child: its running row goes now (verse.npc_answer would let it go)
    const char *fv[1] = {asked};
    PQclear(PQexecParams(db, "DELETE FROM verse.npc_running WHERE id = ($1::jsonb->>'run')::bigint", 1, NULL, fv, NULL, NULL, 0));
    api_json(q->fd, 503, "{\"error\":\"busy\"}");
  }
  free(request); free(asked);
  return true;   // the parent closes its copy of the connection
}

// ---- the Sandbox's set pieces (rl-sandbox-pieces): POST /api/playtest/sandbox/piece
//   {sentence}  (X-Verse-Player optional)  -> 200 {piece, count?, kind?, colour?, size?, model}
//   400 no sentence, 422 nothing fits, 429 one at a time, 502 not a piece, 503 no answer
// Gemini picks one piece from the catalogue (migrations/0036_sandbox_pieces.sql
// verse.piece_catalogue, a response schema of its words) and SQL holds the
// answer to it; the page lays it out (metaverse/setpiece.coax). As a question
// is, the model call is made by a child.
//   OA_PIECE_URL    the generateContent URL (tests point it at a fake)
//   OA_PIECE_MODEL  the model (gemini-4-hydrogen)
//   OA_PIECE_DAILY_CAP  set pieces a day across the server (2000)
static void piece_answer(int fd, const char *asked, const char *request) {
  const char *model = getenv("OA_PIECE_MODEL"); if (!model || !*model) model = "gemini-4-hydrogen";
  buf b = {0};
  double t0 = api_now_ms();
  long status = ask_model("OA_PIECE_URL", model, request, &b);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, b.p ? b.p : "", model, taken};
  PGresult *r = db ? PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.piece_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
                                  5, NULL, v, NULL, NULL, 0) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  else { api_log("piece: %s", db ? PQerrorMessage(db) : "no database"); api_json(fd, 503, "{\"error\":\"not kept\"}"); }
  if (status != 200) api_log("piece: model answered %ld", status);
  if (r) PQclear(r);
  if (db) PQfinish(db);
  free(b.p);
}

bool piece_route(const request *q) {
  if (strcmp(q->path, "/api/playtest/sandbox/piece")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a sentence\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *cap = getenv("OA_PIECE_DAILY_CAP");
  char key[160] = "";
  api_header(q, "X-Verse-Player", key, sizeof key);
  const char *v[3] = {body, cap && *cap ? cap : "2000", key};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, jsonb_build_object('id', x->'id')::text "
                                 "FROM verse.piece_ask($1::jsonb, $2::int, $3) x", 3, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("piece: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not asked\"}");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) {
    char out[160]; snprintf(out, sizeof out, "{\"error\":\"%s\"}", PQgetvalue(r, 0, 1));
    PQclear(r);
    api_json(q->fd, status, out);
    return true;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { piece_answer(q->fd, asked, request); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
  return true;
}

// ---- an admin's spoken edit to a realm (eg-voice-edit): POST /api/playtest/realm/<realm>/edit
//   {sentence}  a site admin (admin_who: admins.conf, by site login, the admin cookie or NIP-98), or the
//               realm's owner or one of its admin accounts (X-Verse-Player; verse.edit_may)
//   -> 200 {edit, item?, count?, model}; 400 no sentence or realm, 401/403, 422 nothing fits, 429, 502, 503
// Gemini picks one edit from the realm's catalogue (migrations/0045_realm_voice_edit.sql), or for any
// other realm with rows, hand-built or made, one of the shared verbs (switch, show, look, ask: 0106),
// and SQL holds the answer to it; the page does the edit and publishes the realm's world (an admin's write, checked there).
//   OA_EDIT_URL    the generateContent URL (tests point it at a fake)
//   OA_EDIT_MODEL  the model (gemini-2.5-flash, its thinking off: migrations/0054_voice_edit_fast.sql)
//   OA_EDIT_TIMEOUT_MS  how long one try may take (4000); an unanswered or failed try is tried once more
//   OA_EDIT_DAILY_CAP  voice edits a day across the server (2000)
// POST /api/playtest/realm/<realm>/look (migrations/0105_voice_look.sql): {question, image} from the
// same people, the image the frame on their screen (the host's flag 32), answered from the picture:
//   -> 200 {edit: "chat", reply, model}; 400, 401/403, 413 too large, 429, 503
//   OA_LOOK_TIMEOUT_MS  how long one try may take (12000)
static void voice_edit_answer(int fd, const char *asked, const char *request, bool look) {
  const char *model = getenv("OA_EDIT_MODEL"); if (!model || !*model) model = "gemini-2.5-flash";
  const char *limit = getenv(look ? "OA_LOOK_TIMEOUT_MS" : "OA_EDIT_TIMEOUT_MS");
  long timeout_ms = limit && atol(limit) > 0 ? atol(limit) : look ? 12000 : 4000;
  buf b = {0};
  double t0 = api_now_ms();
  long status = ask_model_in("OA_EDIT_URL", model, request, &b, timeout_ms);
  if (status == 0 || status == 429 || status >= 500) {   // once more, as fast: a stalled call is the slow tail
    api_log("voice edit: model answered %ld, trying again", status);
    free(b.p); b = (buf){0};
    status = ask_model_in("OA_EDIT_URL", model, request, &b, timeout_ms);
  }
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, b.p ? b.p : "", model, taken};
  PGresult *r = db ? PQexecParams(db, look ? "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.voice_look_answer($1::jsonb, $2::int, $3, $4, $5::int) x"
                                         : "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.voice_edit_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
                                  5, NULL, v, NULL, NULL, 0) : NULL;
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  else { api_log("voice edit: %s", db ? PQerrorMessage(db) : "no database"); api_json(fd, 503, "{\"error\":\"not kept\"}"); }
  if (status != 200) api_log("voice edit: model answered %ld", status);
  if (r) PQclear(r);
  if (db) PQfinish(db);
  free(b.p);
}

bool voice_edit_route(const request *q) {
  static const char prefix[] = "/api/playtest/realm/";
  if (strncmp(q->path, prefix, sizeof prefix - 1)) return false;
  const char *realm = q->path + sizeof prefix - 1, *slash = strchr(realm, '/');
  if (!slash || (strcmp(slash, "/edit") && strcmp(slash, "/look"))) return false;
  bool look = !strcmp(slash, "/look");
  char name[49]; size_t n = (size_t)(slash - realm);   // a hand-built realm's slug, or a fork's (digits too)
  if (!n || n >= sizeof name) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  for (size_t i = 0; i < n; i++) if (!((realm[i] >= 'a' && realm[i] <= 'z') || (realm[i] >= '0' && realm[i] <= '9') || realm[i] == '-')) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  memcpy(name, realm, n); name[n] = 0;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a sentence\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char from[256], who[65];
  const char *o = getenv("OA_API_ORIGIN");
  if (api_header(q, "Origin", from, sizeof from) && strcmp(from, o && *o ? o : "https://openagents.com")) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  int denied = admin_who(q, db, who);
  // Else the realm's owner or one of its admins (verse.realm_rights, docs/metaverse/realm-edit.md), by
  // the X-Verse-Player key; their edits are paced and counted under the key's hash, never the key.
  char key[160];
  if (denied && api_header(q, "X-Verse-Player", key, sizeof key) && key[0]) {
    const char *ev[2] = {name, key};
    PGresult *m = PQexecParams(db, "SELECT encode(sha256(convert_to($2, 'UTF8')), 'hex') FROM (SELECT 1) x "
                                   "WHERE verse.voice_realm($1) AND verse.edit_may($1, $2, false)", 2, NULL, ev, NULL, NULL, 0);
    if (PQresultStatus(m) == PGRES_TUPLES_OK && PQntuples(m) == 1) { snprintf(who, 65, "%s", PQgetvalue(m, 0, 0)); denied = 0; }
    else if (denied == 401) denied = 403;   // a player who may not edit this realm
    PQclear(m);
  }
  if (denied) { api_json(q->fd, denied, denied == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return true; }
  // a fork speaks with its base's catalogue and switches its own rows (migrations/0106_voice_every_realm.sql)
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *cap = getenv("OA_EDIT_DAILY_CAP");
  const char *v[4] = {name, body, who, cap && *cap ? cap : "2000"};
  PGresult *r = PQexecParams(db, look ? "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, jsonb_build_object('id', x->'id')::text "
                                        "FROM verse.voice_look_ask($1, $2::jsonb, $3, $4::int) x"
                                      : "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, jsonb_build_object('id', x->'id')::text "
                                        "FROM verse.voice_edit_ask($1, $2::jsonb, $3, $4::int) x", 4, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    if (!theirs) api_log("voice edit: %s", PQerrorMessage(db));
    PQclear(r);
    api_json(q->fd, theirs ? 400 : 500, theirs ? "{\"error\":\"not json\"}" : "{\"error\":\"not asked\"}");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) {
    char out[160]; snprintf(out, sizeof out, "{\"error\":\"%s\"}", PQgetvalue(r, 0, 1));
    PQclear(r);
    api_json(q->fd, status, out);
    return true;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { voice_edit_answer(q->fd, asked, request, look); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
  return true;
}
