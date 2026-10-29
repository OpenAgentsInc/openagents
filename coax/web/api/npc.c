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

// In the child: asks the model, keeps the answer, answers the page.
static void answer(int fd, const char *asked, const char *request) {
  const char *model = getenv("OA_NPC_MODEL"); if (!model || !*model) model = "gemini-3.8-flash";
  char url[512];
  const char *given = getenv("OA_NPC_URL");
  if (given && *given) snprintf(url, sizeof url, "%s", given);
  else snprintf(url, sizeof url, "https://aiplatform.googleapis.com/v1/projects/%s/locations/global/publishers/google/models/%s:generateContent",
                PROJECT, model);
  char tok[4096], auth[4200];
  long status = 0;
  buf b = {0};
  double t0 = api_now_ms();
  if (token(tok, sizeof tok)) {
    CURL *h = curl_easy_init();
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, request);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 25L);
    if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
  } else api_log("npc: no token");
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
