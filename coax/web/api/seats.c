// Seats in running realms (docs/verse/realm-seat.md, #12655): an agent's place in a made realm,
// opened by its account. seat-policy.coax (#authority) decides who may open one and how long it
// lives; migrations/0110_realm_seats.sql keeps them; this file moves the facts in and the answers
// out, and for a hosted seat talks to the runner that holds its page.
//
// Every route takes the account by a NIP-98 signature, or from this site's own page by its logged-in
// session (the YOUR AGENT pane, verse/edit/agent.coax; a POST must carry this site's Origin). Never
// by the player key.
//   POST /api/playtest/seats              {slug, hosted?}  signed (NIP-98) -> {id, slug, hosted, player: {id, key}, until}
//                                         the seat's own player key: its page plays with it as the account
//   GET  /api/playtest/seats              signed -> {seats: [{id, slug, hosted, opened_ms, kept_ms}]}
//   POST /api/playtest/seats/<id>/keep    signed: an action happened (the idle clock starts again)
//   POST /api/playtest/seats/<id>/close   signed: closed, its key dead
//   POST /api/playtest/seats/<id>/act     signed, a hosted seat: one action line (realm_seat.coax) -> its observation
//
// A hosted seat's page runs on the project agents' runner (coax/web/agent/runner.c): OA_AGENT_WAKE_URL
// is its wake endpoint, and /seat/open, /seat/act and /seat/close sit beside it, reached with its
// identity token (OA_AGENT_WAKE_TOKEN in tests). The seat's key goes to the runner and to no one else.
#include "api.h"
#include "seat-policy.generated.h"
#include <curl/curl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *body_text(const request *q) {
  char *b = malloc(q->body_len + 3);
  if (!b) return NULL;
  if (q->body_len) { memcpy(b, q->body, q->body_len); b[q->body_len] = 0; } else strcpy(b, "{}");
  return b;
}
static const char *site_origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }

// One value from SQL, or NULL (freed by the caller).
static char *one(PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  char *out = NULL;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) out = strdup(PQgetvalue(r, 0, 0));
  else if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("seats: %s", PQerrorMessage(db));
  PQclear(r);
  return out;
}
static bool run(PGconn *db, const char *sql) {
  PGresult *r = PQexec(db, sql);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK || PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("seats: %s", PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static void refuse(const request *q, int64_t code) {
  coax_string why = sp_refusal_text(code);
  char out[200];
  snprintf(out, sizeof out, "{\"error\":\"%.*s\"}", (int)why.count, why.data);
  api_json(q->fd, (int)sp_status(code), out);
}

// Closes every open seat seat-policy.coax says is no longer live (sp_live): called before a seat
// opens and on every made realm's presence post (room.c), so an idle seat's key dies within a
// second while anyone is in a made realm.
void seats_sweep(PGconn *db) {
  PGresult *r = PQexec(db, "SELECT id, kept_ms, now_ms FROM verse.seats_kept()");
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { PQclear(r); return; }   // before 0110: no seats
  for (int i = 0; i < PQntuples(r); i++) {
    long long kept = atoll(PQgetvalue(r, i, 1)), now = atoll(PQgetvalue(r, i, 2));
    if (sp_live(now, kept, false)) continue;
    const char *v[1] = {PQgetvalue(r, i, 0)};
    PGresult *x = PQexecParams(db, "SELECT verse.seat_idle($1::bigint)", 1, NULL, v, NULL, NULL, 0);
    PQclear(x);
  }
  PQclear(r);
}

// ---- the runner, for hosted seats
typedef struct { char *p; size_t n; } sbuf;
static size_t stake(char *data, size_t size, size_t n, void *user) {
  sbuf *b = user; size_t k = size * n;
  if (b->n + k > 262144) return 0;
  char *p = realloc(b->p, b->n + k + 1);
  if (!p) return 0;
  memcpy(p + b->n, data, k); b->p = p; b->n += k; b->p[b->n] = 0;
  return k;
}
// The runner's base (the wake URL without its /wake), or false when there is none.
static bool runner_base(char *out, size_t cap) {
  const char *wake = getenv("OA_AGENT_WAKE_URL");
  if (!wake || !*wake) return false;
  size_t n = strlen(wake);
  if (n >= 5 && !strcmp(wake + n - 5, "/wake")) n -= 5;
  if (n + 1 > cap) return false;
  memcpy(out, wake, n); out[n] = 0;
  return true;
}
static bool runner_token(const char *base, char *out, size_t cap) {
  const char *given = getenv("OA_AGENT_WAKE_TOKEN");
  if (given && *given) { snprintf(out, cap, "%s", given); return true; }
  char meta[800];
  snprintf(meta, sizeof meta, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity?audience=%s", base);
  CURL *h = curl_easy_init();
  if (!h) return false;
  sbuf b = {0};
  struct curl_slist *hd = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(h, CURLOPT_URL, meta);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, stake);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 5L);
  bool ok = curl_easy_perform(h) == CURLE_OK && b.p && b.n + 1 < cap;
  if (ok) snprintf(out, cap, "%s", b.p);
  curl_slist_free_all(hd); curl_easy_cleanup(h); free(b.p);
  return ok;
}
// POST base+path with a JSON body: the status (0 unreached); the answer in *out (freed by the caller).
static long runner_post(const char *path, const char *json, long timeout_s, char **out) {
  *out = NULL;
  char base[600], url[800], tok[4096], auth[4200];
  if (!runner_base(base, sizeof base) || !runner_token(base, tok, sizeof tok)) return 0;
  snprintf(url, sizeof url, "%s%s", base, path);
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  CURL *h = curl_easy_init();
  if (!h) return 0;
  sbuf b = {0};
  long status = 0;
  struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, json);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, stake);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, timeout_s);
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h);
  *out = b.p;
  return status;
}

// ---- the routes
static void open_seat(const request *q, PGconn *db, const char *signer, const char *acct, const char *key) {
  char *body = body_text(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  const char *bv[1] = {body};
  char *slug = one(db, "SELECT CASE WHEN jsonb_typeof(b->'slug') = 'string' AND b->>'slug' ~ '^[a-z0-9-]{1,48}$' THEN b->>'slug' END "
                       "FROM (SELECT $1::jsonb b) x", 1, bv);
  char *hosted_s = one(db, "SELECT coalesce((SELECT b->'hosted' = 'true'::jsonb FROM (SELECT $1::jsonb b) x), false)::text", 1, bv);
  free(body);
  bool hosted = hosted_s && !strcmp(hosted_s, "true");
  free(hosted_s);
  if (!slug) { api_json(q->fd, 400, "{\"error\":\"slug: a made realm's address\"}"); return; }
  bool admin = admin_is(signer);
  char base[600];
  bool runner = runner_base(base, sizeof base);
  if (!run(db, "BEGIN") || !run(db, "SELECT pg_advisory_xact_lock(7731027)")) { free(slug); run(db, "ROLLBACK"); api_json(q->fd, 500, "{\"error\":\"not opened\"}"); return; }
  seats_sweep(db);
  const char *fv[4] = {acct, key, slug, admin ? "true" : "false"};
  char *facts = one(db, "SELECT (f->>'others') || ' ' || (f->>'seen') FROM verse.seat_facts($1, $2, $3, $4::boolean) f", 4, fv);
  long long others = 0; bool seen = false;
  if (facts) { char word[8] = ""; sscanf(facts, "%lld %7s", &others, word); seen = !strcmp(word, "true"); free(facts); }
  int64_t code = sp_open(true, seen, others, hosted, runner);
  if (code != 0) { run(db, "ROLLBACK"); free(slug); refuse(q, code); return; }   // not SP_OPEN
  char until[32]; snprintf(until, sizeof until, "%lld", (long long)sp_until((int64_t)api_now_ms()));
  const char *ov[6] = {acct, signer, slug, admin ? "true" : "false", hosted ? "true" : "false", until};
  char *opened = one(db, "SELECT verse.seat_open($1, $2, $3, $4::boolean, $5::boolean, $6::bigint)::text", 6, ov);
  free(slug);
  if (!opened || !run(db, "COMMIT")) { free(opened); run(db, "ROLLBACK"); api_json(q->fd, 500, "{\"error\":\"not opened\"}"); return; }
  if (hosted) {
    // the runner starts the seat's Chrome with its key; the seat closes again when it can't
    size_t n = strlen(opened) + 600;
    char *msg = malloc(n), *answer = NULL;
    if (!msg) { free(opened); api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
    snprintf(msg, n, "{\"seat\":%s,\"origin\":\"%s\"}", opened, site_origin());
    long status = runner_post("/seat/open", msg, 120L, &answer);
    free(msg);
    if (status != 200) {
      const char *iv[2] = {"", acct};
      char *id = one(db, "SELECT ($1::jsonb->>'id')", 1, (const char *const[]){opened});
      iv[0] = id ? id : "0";
      char *closed = one(db, "SELECT verse.seat_close($1::bigint, $2)::text", 2, iv);
      free(closed); free(id); free(answer); free(opened);
      api_log("seats: the runner didn't open a hosted seat (%ld)", status);
      api_json(q->fd, 502, "{\"error\":\"the runner didn't open the seat: try again\"}");
      return;
    }
    free(answer);
    // a hosted seat's key stays with the runner: the caller gets the seat without it
    char *shown = one(db, "SELECT ($1::jsonb #- '{player,key}')::text", 1, (const char *const[]){opened});
    api_json(q->fd, 200, shown ? shown : "{}");
    free(shown); free(opened);
    return;
  }
  api_json(q->fd, 200, opened);
  free(opened);
}

static void seat_op(const request *q, PGconn *db, const char *acct, const char *id, const char *op) {
  const char *v[2] = {id, acct};
  if (!strcmp(op, "keep") || !strcmp(op, "close")) {
    bool closing = !strcmp(op, "close");
    char *hosted = one(db, "SELECT hosted::text FROM verse.seat_mine($1::bigint, $2) WHERE id IS NOT NULL", 2, v);
    char *status = one(db, closing ? "SELECT verse.seat_close($1::bigint, $2)::text" : "SELECT verse.seat_keep($1::bigint, $2)::text", 2, v);
    int s = status ? atoi(status) : 500;
    free(status);
    if (closing && s == 200 && hosted && !strcmp(hosted, "true")) {
      char msg[64], *answer = NULL; snprintf(msg, sizeof msg, "{\"id\":%s}", id);
      long st = runner_post("/seat/close", msg, 30L, &answer);
      free(answer);
      if (st != 200) api_log("seats: the runner didn't close seat %s (%ld)", id, st);
    }
    free(hosted);
    api_json(q->fd, s, s == 200 ? (closing ? "{\"closed\":true}" : "{\"kept\":true}") : s == 404 ? "{\"error\":\"no such open seat\"}" : "{\"error\":\"not done\"}");
    return;
  }
  if (!strcmp(op, "act")) {
    char *hosted = one(db, "SELECT hosted::text FROM verse.seat_mine($1::bigint, $2) WHERE id IS NOT NULL", 2, v);
    bool is_hosted = hosted && !strcmp(hosted, "true");
    free(hosted);
    if (!is_hosted) { api_json(q->fd, 404, "{\"error\":\"no such open hosted seat\"}"); return; }
    char *body = body_text(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
    const char *bv[2] = {body, id};
    // the action as one JSON object, its id the seat's
    char *msg = one(db, "SELECT jsonb_build_object('id', $2::bigint, 'action', $1::jsonb)::text WHERE jsonb_typeof($1::jsonb) = 'object' AND length($1::text) <= 8192", 2, bv);
    free(body);
    if (!msg) { api_json(q->fd, 400, "{\"error\":\"an action is one JSON object\"}"); return; }
    char *answer = NULL;
    long st = runner_post("/seat/act", msg, 150L, &answer);
    free(msg);
    if (st == 200 && answer) {
      char *kept = one(db, "SELECT verse.seat_keep($1::bigint, $2)::text", 2, v);
      free(kept);
      api_json(q->fd, 200, answer);
    } else api_json(q->fd, st == 404 ? 404 : 502, st == 404 ? "{\"error\":\"the seat is gone from the runner\"}" : "{\"error\":\"the runner didn't answer\"}");
    free(answer);
    return;
  }
  api_json(q->fd, 404, "{\"error\":\"not found\"}");
}

bool seats_route(const request *q) {
  static const char root[] = "/api/playtest/seats";
  size_t rl = sizeof root - 1;
  if (strncmp(q->path, root, rl) || (q->path[rl] && q->path[rl] != '/')) return false;
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  bool post = !strcmp(q->method, "POST");
  if (!post && strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  char from[256];
  if (post && api_header(q, "Origin", from, sizeof from) && strcmp(from, site_origin())) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  // a seat is an account's: a signed caller (caller-policy.coax), or, unsigned, this site's own page
  // logged in (its oa_session; a POST only with this site's Origin); never the player key
  char signer[65] = "", key[160] = "";
  int sig = access_signed(q, db, signer);
  if (sig < 0) return true;
  if (sig == 0) {
    bool signed_req = false, payload = false, unseen = false;
    bool here = !post || (api_header(q, "Origin", from, sizeof from) && !strcmp(from, site_origin()));
    if (!here || !site_identity(q, db, signer, &signed_req, &payload, &unseen) || signed_req) signer[0] = 0;
    if (!signer[0]) { refuse(q, sp_open(false, false, 0, false, false)); return true; }
  }
  const char *pv[1] = {signer};
  char *acct = one(db, "SELECT auth.account_of($1)", 1, pv);
  if (!acct || !access_caller_key(db, signer, key)) { free(acct); api_json(q->fd, 500, "{\"error\":\"not read\"}"); return true; }
  const char *rest = q->path + rl;
  if (!*rest) {
    if (post) open_seat(q, db, signer, acct, key);
    else {
      const char *v[1] = {acct};
      char *mine = one(db, "SELECT jsonb_build_object('seats', verse.seats_mine($1))::text", 1, v);
      api_json(q->fd, mine ? 200 : 500, mine ? mine : "{\"error\":\"not read\"}");
      free(mine);
    }
    free(acct);
    return true;
  }
  // /<id>/<op>
  char id[24] = "", op[16] = "";
  if (!post || sscanf(rest, "/%20[0-9]/%15[a-z]", id, op) != 2) { free(acct); api_json(q->fd, 404, "{\"error\":\"not found\"}"); return true; }
  seat_op(q, db, acct, id, op);
  free(acct);
  return true;
}
