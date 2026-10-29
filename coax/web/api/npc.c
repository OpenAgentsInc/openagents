// Character chat for the Verse playtest (pt-chat-endpoint): the demo's
// POST /api/npc/ask (aethyr-cpp tools/npc_chat.py) as
//
//   POST /api/playtest/npc/ask  {npc: {name, role, knowledge, greet}, world, history: [{from, text}], question,
//                                controls?, visit?}  (X-Verse-Player: the browser's player key, optional)
//                               -> 200 {text, model}; 400 without a question; 503 {error} when there is
//                               no answer (the page then says its own lines, as the demo's does)
// With a player key a character remembers that player: what they said to it
// on earlier visits goes into its prompt (migrations/0031_npc_memory.sql
// verse.npc_ask).
//
// The prompt, the memory and the day's cap are SQL's (migrations/0005_verse_npc.sql
// verse.npc_request, verse.npc_answer, which also keeps every question and
// answer). The model is Google's first: Gemini on Vertex AI (OA_NPC_MODEL,
// default gemini-3.8-flash on the global endpoint), with the service's own
// token from the metadata server. A model call takes seconds and this
// service answers one request at a time, so a child process makes it and
// answers, with its own database connection, while the parent goes on.
//   OA_NPC_URL    the generateContent URL (tests point it at a fake)
//   OA_NPC_TOKEN  a token to send instead of the metadata server's (tests)
//   OA_NPC_DAILY_CAP  answers a day across the server (2000)
#include "api.h"
#include <curl/curl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

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
static long ask_model(const char *url_env, const char *model, const char *request, buf *b) {
  char url[512];
  const char *given = getenv(url_env);
  if (given && *given) snprintf(url, sizeof url, "%s", given);
  else snprintf(url, sizeof url, "https://aiplatform.googleapis.com/v1/projects/%s/locations/global/publishers/google/models/%s:generateContent",
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
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 25L);
    if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
  } else api_log("npc: no token");
  return status;
}

// In the child: asks the model, keeps the answer, answers the page.
static void answer(int fd, const char *asked, const char *request) {
  const char *model = getenv("OA_NPC_MODEL"); if (!model || !*model) model = "gemini-3.8-flash";
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

bool npc_route(const request *q) {
  if (strcmp(q->path, "/api/playtest/npc/ask")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a question\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *cap = getenv("OA_NPC_DAILY_CAP");
  char key[160] = "";
  api_header(q, "X-Verse-Player", key, sizeof key);
  const char *v[3] = {body, cap && *cap ? cap : "2000", key};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, (x - 'request' - 'status')::text "
                                 "FROM verse.npc_ask($1::jsonb, $2::int, $3) x", 3, NULL, v, NULL, NULL, 0);
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
    char out[160]; snprintf(out, sizeof out, "{\"error\":\"%s\"}", PQgetvalue(r, 0, 1));
    PQclear(r);
    api_json(q->fd, status, out);
    return true;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *asked = strdup(PQgetvalue(r, 0, 3));
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);   // children are never waited for
  pid_t pid = fork();
  if (pid == 0) {             // the child answers; the parent's database connection is never touched here
    answer(q->fd, asked, request);
    _exit(0);
  }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
  return true;   // the parent closes its copy of the connection
}

// ---- the Sandbox's set pieces (rl-sandbox-pieces): POST /api/playtest/sandbox/piece
//   {sentence}  (X-Verse-Player optional)  -> 200 {piece, count?, kind?, colour?, size?, model}
//   400 no sentence, 422 nothing fits, 429 one at a time, 502 not a piece, 503 no answer
// Gemini picks one piece from the catalogue (migrations/0036_sandbox_pieces.sql
// verse.piece_catalogue, a response schema of its words) and SQL holds the
// answer to it; the page lays it out (verse/setpiece.coax). As a question
// is, the model call is made by a child.
//   OA_PIECE_URL    the generateContent URL (tests point it at a fake)
//   OA_PIECE_MODEL  the model (gemini-3.8-flash)
//   OA_PIECE_DAILY_CAP  set pieces a day across the server (2000)
static void piece_answer(int fd, const char *asked, const char *request) {
  const char *model = getenv("OA_PIECE_MODEL"); if (!model || !*model) model = "gemini-3.8-flash";
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
//   {sentence}  an admin only (admin_who: admins.conf, by site login or NIP-98)
//   -> 200 {edit, item?, count?, model}; 400 no sentence or realm, 401/403, 422 nothing fits, 429, 502, 503
// Gemini picks one edit from the realm's catalogue (migrations/0045_realm_voice_edit.sql) and SQL holds
// the answer to it; the page does the edit and publishes the realm's world (an admin's write, checked there).
//   OA_EDIT_URL    the generateContent URL (tests point it at a fake)
//   OA_EDIT_MODEL  the model (gemini-3.8-flash)
//   OA_EDIT_DAILY_CAP  voice edits a day across the server (2000)
static void voice_edit_answer(int fd, const char *asked, const char *request) {
  const char *model = getenv("OA_EDIT_MODEL"); if (!model || !*model) model = "gemini-3.8-flash";
  buf b = {0};
  double t0 = api_now_ms();
  long status = ask_model("OA_EDIT_URL", model, request, &b);
  char http[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[5] = {asked, http, b.p ? b.p : "", model, taken};
  PGresult *r = db ? PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM verse.voice_edit_answer($1::jsonb, $2::int, $3, $4, $5::int) x",
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
  if (!slash || strcmp(slash, "/edit")) return false;
  char name[33]; size_t n = (size_t)(slash - realm);
  if (!n || n >= sizeof name) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  for (size_t i = 0; i < n; i++) if (!((realm[i] >= 'a' && realm[i] <= 'z') || realm[i] == '-')) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  memcpy(name, realm, n); name[n] = 0;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a sentence\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char from[256], who[65];
  const char *o = getenv("OA_API_ORIGIN");
  if (api_header(q, "Origin", from, sizeof from) && strcmp(from, o && *o ? o : "https://openagents.com")) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  int denied = admin_who(q, db, who);
  if (denied) { api_json(q->fd, denied, denied == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return true; }
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *cap = getenv("OA_EDIT_DAILY_CAP");
  const char *v[4] = {name, body, who, cap && *cap ? cap : "2000"};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, jsonb_build_object('id', x->'id')::text "
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
  if (pid == 0) { voice_edit_answer(q->fd, asked, request); _exit(0); }
  if (pid < 0) api_json(q->fd, 503, "{\"error\":\"busy\"}");
  free(request); free(asked);
  return true;
}
