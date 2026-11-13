// Our inference route (gw-ours, docs/coder/microcoder.md): the Mac app's Lab
// asks models through us, so the app ships with no provider key.
//
//   POST /api/v1/inference   a Chat Completions request (model, messages, stream, response_format ...),
//                        signed NIP-98 ("Authorization: Nostr <base64>") by the user's Nostr key
//                        -> Vertex AI's answer as it comes: its status, then its JSON or its
//                           server-sent events; 400 a bad request or a model we don't serve,
//                           401 no valid signature, 403 a key that may not use the route,
//                           429 the account's day used up, 502 Vertex not reached
//
// Who: a key in OA_INFERENCE_PUBKEYS (comma separated) or an admin's
// (OA_ADMIN_PUBKEYS), free; or any signed key with credit above zero, which
// pays: each call is debited in msat at the kept BTC/USD rate, and with no
// credit the answer is 402 pointing at the top-up route below. A key a person's
// account holds is also held to that account's daily limit (wallet.c, #13330):
// past it, the same step-up answer as a wallet payment (401, step_up L2-fresh)
// until a login of the account proves L2. The account
// is the signing key: the request, its caps and its metering are SQL's
// (migrations/0059_inference.sql, 0066_inference_credit.sql
// inference.request, inference.finish), at the card's rates. The daily caps
// hold for every key.
//
//   POST /api/v1/inference/credit  {"pubkey": hex, "msat": "N"}, no signature: buys credit
//                        with x402 v2 over Lightning (NIP-X402, receiver profile
//                        openagents:spark-receiver:1). Without PAYMENT-SIGNATURE: 402 and
//                        PAYMENT-REQUIRED (base64 JSON) with an invoice from our Spark wallet,
//                        bound to the body in inference.topups. With it: our wallet's record of
//                        that invoice paid, a preimage that hashes to its payment hash, then the
//                        payment kept once and the credit added in one transaction: 200 and
//                        PAYMENT-RESPONSE; 402 payment_pending until then; 409
//                        duplicate_settlement after. Bitcoin only, mainnet only.
//   GET  /api/v1/inference/credit  signed NIP-98: the signer's credit, {"credit_msat": N}
//   OA_SPARK_URL              the wallet (coax/web/api/spark/wallet.cjs), asked with an identity token
//   OA_SPARK_TOKEN            a token to send instead of the metadata server's (tests)
//   OA_CREDIT_MIN_MSAT, OA_CREDIT_MAX_MSAT   a top-up's bounds (10 sats, 100,000 sats)
//
// The call to Vertex takes seconds, and this service answers one request at
// a time, so a child process makes it: it relays Vertex's status and bytes to
// the caller as they arrive, then keeps the call, with the usage Vertex
// reported, on a database connection of its own.
//   OA_INFERENCE_URL          the Chat Completions URL (tests point it at a fake)
//   OA_INFERENCE_TOKEN        a token to send instead of the metadata server's (tests)
//   OA_INFERENCE_PUBKEYS      the keys that may use the route, besides the admins'
//   OA_INFERENCE_DAILY_REQUESTS  requests a day per account (500)
//   OA_INFERENCE_DAILY_USD_MICROS  spend a day per account, in millionths of a dollar (2000000)
#include "api.h"
#include "nostr.h"
#include <curl/curl.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

// The us multi-region, not global: global was many times slower on Gemini 4 Carbon (2026-11-11).
#define VERTEX_CHAT "https://aiplatform.us.rep.googleapis.com/v1/projects/openagentsgemini/locations/us/endpoints/openapi/chat/completions"

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

// A key in the comma-separated list OA_INFERENCE_PUBKEYS.
static bool listed(const char *pubkey) {
  const char *l = getenv("OA_INFERENCE_PUBKEYS");
  size_t n = strlen(pubkey);
  while (l && *l) {
    const char *end = strchr(l, ',');
    size_t k = end ? (size_t)(end - l) : strlen(l);
    while (k && l[0] == ' ') { l++; k--; }
    while (k && l[k - 1] == ' ') k--;
    if (k == n && !strncmp(l, pubkey, n)) return true;
    if (!end) break;
    l = end + 1;
  }
  return false;
}

// The service account's token, from the metadata server (or OA_INFERENCE_TOKEN).
typedef struct { char *p; size_t n, cap; } buf;
static size_t keep(buf *b, const char *data, size_t k) {
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
static size_t take(char *data, size_t size, size_t n, void *user) { return keep(user, data, size * n); }
static bool token(char *out, size_t cap) {
#ifdef OA_LOCAL_SERVE
  // The local serve mode's token (#13463): test.sh --serve keeps a fresh one in this file (0600),
  // from the service account's gcloud config or the developer's own gcloud login; empty when neither.
  const char *file = getenv("OA_INFERENCE_TOKEN_FILE");
  if (file && *file) {
    FILE *f = fopen(file, "r");
    out[0] = 0;
    if (f) { if (!fgets(out, (int)cap, f)) out[0] = 0; fclose(f); }
    out[strcspn(out, "\r\n")] = 0;
    return out[0] != 0;
  }
#endif
  const char *given = getenv("OA_INFERENCE_TOKEN");
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
bool inference_google_token(char *out, size_t cap) { return token(out, cap); }

static bool send_all(int fd, const char *p, size_t n);

// An identity token for a sibling Cloud Run service (its URL is the audience), from the metadata server,
// or the token in the environment variable `given` (tests). Our Spark wallet and the compile service
// (scripts.c) are asked with one.
bool api_id_token(const char *aud, const char *given_env, char *out, size_t cap) {
  const char *given = given_env ? getenv(given_env) : NULL;
  if (given && *given) { snprintf(out, cap, "%s", given); return true; }
  CURL *h = curl_easy_init();
  if (!h) return false;
  char url[1024]; buf b = {0};
  char *a = curl_easy_escape(h, aud, 0);
  snprintf(url, sizeof url, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity?audience=%s", a ? a : "");
  curl_free(a);
  struct curl_slist *hd = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 5L);
  curl_easy_setopt(h, CURLOPT_PROXY, "");
  long status = 0;
  bool ok = curl_easy_perform(h) == CURLE_OK && (curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status), status == 200) && b.p && b.n < cap;
  if (ok) snprintf(out, cap, "%s", b.p);
  curl_slist_free_all(hd); curl_easy_cleanup(h); free(b.p);
  return ok;
}
// The same token with url's origin (scheme and host, no path) as the audience: the runner (chat.c,
// private.c, seats.c) and the mesh fleet (mesh.c) are called this way.
bool api_id_token_for_url(const char *url, const char *given_env, char *out, size_t cap) {
  char origin[1025];
  const char *host = strstr(url, "://");
  size_t k = host ? (size_t)(host + 3 - url) + strcspn(host + 3, "/") : strlen(url);
  if (k >= sizeof origin) return false;
  memcpy(origin, url, k); origin[k] = 0;
  return api_id_token(origin, given_env, out, cap);
}

// ---- our Spark wallet (OA_SPARK_URL): POST path with json, or GET with none.
// Its status (0 unreached), and its body into *out (malloc'd, or NULL).
static long spark_call(const char *path, const char *json, long timeout_s, char **out) {
  *out = NULL;
  const char *base = getenv("OA_SPARK_URL");
  if (!base || !*base) { api_log("inference: no OA_SPARK_URL"); return 0; }
  char tok[4096], auth[4200], url[1024];
  if (!api_id_token(base, "OA_SPARK_TOKEN", tok, sizeof tok)) { api_log("inference: no token for the wallet"); return 0; }
  snprintf(url, sizeof url, "%s%s", base, path);
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  CURL *h = curl_easy_init();
  if (!h) return 0;
  buf b = {0};
  struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  if (json) curl_easy_setopt(h, CURLOPT_POSTFIELDS, json);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, timeout_s);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h);
  memset(tok, 0, sizeof tok); memset(auth, 0, sizeof auth);
  if (status != 200) { api_log("inference: the wallet answered %ld on %s", status, path); free(b.p); return status; }
  *out = b.p;
  return status;
}

// The same wallet for makers.c's tips and payouts.
long api_spark(const char *path, const char *json, long timeout_s, char **out) { return spark_call(path, json, timeout_s, out); }

// The BTC/USD rate, asked of the wallet when the kept one is ten minutes old
// (or there is none). A failed ask keeps the old rate; inference.request
// refuses paid use once it is an hour old.
static void rate_refresh(PGconn *db) {
  PGresult *r = PQexec(db, "SELECT coalesce(inference.rate_age(), 1e9) > 600");
  bool stale = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  if (!stale) return;
  char *w = NULL;
  if (spark_call("/rate", NULL, 8, &w) != 200 || !w) { free(w); return; }
  const char *v[1] = {w};
  r = PQexecParams(db, "SELECT inference.rate_set((w->>'usd_per_btc')::bigint) FROM (SELECT $1::jsonb w) s "
                       "WHERE w->>'usd_per_btc' ~ '^[1-9][0-9]{3,8}$'", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("inference: rate not kept: %s", PQerrorMessage(db));
  PQclear(r); free(w);
}

// An answer with one x402 header (name: value), or none.
static void x402_reply(int fd, int status, const char *name, const char *value, const char *json) {
  char *head = NULL;
  if (name && value && *value) {
    size_t n = strlen(name) + strlen(value) + 8;
    head = malloc(n);
    if (head) snprintf(head, n, "%s: %s\r\n", name, value);
  }
  char *extra = NULL;
  size_t hn = head ? strlen(head) : 0;
  // api_respond's head is bounded; a challenge's base64 can be larger, so write the head here.
  char start[512];
  int k = snprintf(start, sizeof start, "HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nCache-Control: no-store, private\r\n"
                   "X-Content-Type-Options: nosniff\r\nAccess-Control-Expose-Headers: PAYMENT-REQUIRED, PAYMENT-RESPONSE\r\n",
                   status, status == 200 ? "OK" : status == 402 ? "Payment Required" : status == 409 ? "Conflict" : status == 400 ? "Bad Request" : "Error", strlen(json));
  extra = malloc((size_t)k + hn + 32);
  if (!extra) { free(head); api_error(fd, 503, "memory"); return; }
  memcpy(extra, start, (size_t)k);
  if (head) memcpy(extra + k, head, hn);
  memcpy(extra + k + hn, "Connection: close\r\n\r\n", 22);
  send_all(fd, extra, (size_t)k + hn + 21);
  send_all(fd, json, strlen(json));
  free(extra); free(head);
}

static long long env_ll(const char *name, long long fallback) { const char *v = getenv(name); return v && *v ? atoll(v) : fallback; }

// In the child: a top-up, issued or settled, on a connection of its own.
static void credit_answer(int fd, const char *body, const char *sig) {
  PGconn *db = api_db_new();
  if (!db) { api_error(fd, 503, "no database"); return; }
  char url[600], mn[24], mx[24];
  snprintf(url, sizeof url, "%s/api/v1/inference/credit", env_or("OA_API_ORIGIN", "https://openagents.com"));
  snprintf(mn, sizeof mn, "%lld", env_ll("OA_CREDIT_MIN_MSAT", 10000));
  snprintf(mx, sizeof mx, "%lld", env_ll("OA_CREDIT_MAX_MSAT", 100000000));
  char *w = NULL;
  PGresult *r = NULL;
  if (!sig) {
    const char *v[4] = {body, url, mn, mx};
    r = PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->>'error', ''), x->>'pubkey', x->>'msat', x->>'digest', "
                         "jsonb_build_object('sats', x->'sats', 'memo', x->'memo', 'expiry', 600)::text "
                         "FROM (SELECT inference.topup_terms($1, $2, $3::bigint, $4::bigint) x) s", 4, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("inference: credit: %s", PQerrorMessage(db)); api_error(fd, 500, "not asked"); goto done; }
    if (atoi(PQgetvalue(r, 0, 0)) != 200) {
      api_error(fd, 400, PQgetvalue(r, 0, 1)); goto done;
    }
    if (spark_call("/invoice", PQgetvalue(r, 0, 5), 30, &w) != 200 || !w) { api_error(fd, 502, "our wallet did not make an invoice; try again soon"); goto done; }
    const char *v2[5] = {PQgetvalue(r, 0, 2), PQgetvalue(r, 0, 3), PQgetvalue(r, 0, 4), url, w};
    PGresult *i = PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->>'header', ''), coalesce(x->'body', jsonb_build_object('error', x->'error'))::text "
                                   "FROM (SELECT inference.topup_issue($1, $2::bigint, $3, $4, w->>'invoice', w->>'payment_hash', w->>'pay_to', (w->>'expiry')::bigint) x "
                                   "FROM (SELECT $5::jsonb w) s) t", 5, NULL, v2, NULL, NULL, 0);
    if (PQresultStatus(i) != PGRES_TUPLES_OK || PQntuples(i) != 1) { api_log("inference: credit issue: %s", PQerrorMessage(db)); api_error(fd, 502, "our wallet's invoice did not fit"); }
    else x402_reply(fd, atoi(PQgetvalue(i, 0, 0)), "PAYMENT-REQUIRED", PQgetvalue(i, 0, 1), PQgetvalue(i, 0, 2));
    PQclear(i);
    goto done;
  }
  const char *v[5] = {body, sig, url, mn, mx};
  r = PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->>'error', ''), x->>'payment_hash', coalesce(x->>'preimage', ''), coalesce(x->>'header', ''), "
                       "jsonb_build_object('invoice', x->'invoice')::text "
                       "FROM (SELECT inference.topup_proof($1, $2, $3, $4::bigint, $5::bigint) x) s", 5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("inference: credit proof: %s", PQerrorMessage(db)); api_error(fd, 500, "not checked"); goto done; }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) {
    char out[300]; api_error_text(out, sizeof out, PQgetvalue(r, 0, 1));
    x402_reply(fd, status, "PAYMENT-REQUIRED", PQgetvalue(r, 0, 4), out); goto done;
  }
  if (spark_call("/paid", PQgetvalue(r, 0, 5), 30, &w) != 200 || !w) {
    char out[96]; api_error_text(out, sizeof out, "payment_pending: our wallet did not answer; try again");
    x402_reply(fd, 402, "PAYMENT-REQUIRED", PQgetvalue(r, 0, 4), out); goto done;
  }
  const char *v3[3] = {PQgetvalue(r, 0, 2), w, PQgetvalue(r, 0, 3)};
  PGresult *t = PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->>'header', ''), "
                                 "CASE WHEN x ? 'credit_msat' THEN jsonb_build_object('credit_msat', x->'credit_msat', 'msat', x->'msat', 'pubkey', x->'pubkey') "
                                 "ELSE jsonb_build_object('error', x->'error') END::text "
                                 "FROM (SELECT inference.topup_settle($1, w->>'state', CASE WHEN w->>'amount_msat' ~ '^[0-9]{1,18}$' THEN (w->>'amount_msat')::bigint END, "
                                 "w->>'preimage', $3) x FROM (SELECT $2::jsonb w) s) q", 3, NULL, v3, NULL, NULL, 0);
  if (PQresultStatus(t) != PGRES_TUPLES_OK || PQntuples(t) != 1) { api_log("inference: credit settle: %s", PQerrorMessage(db)); api_error(fd, 500, "not settled"); }
  else {
    int st = atoi(PQgetvalue(t, 0, 0));
    if (st == 200) x402_reply(fd, 200, "PAYMENT-RESPONSE", PQgetvalue(t, 0, 1), PQgetvalue(t, 0, 2));
    else x402_reply(fd, st, st == 402 ? "PAYMENT-REQUIRED" : NULL, PQgetvalue(r, 0, 4), PQgetvalue(t, 0, 2));
  }
  PQclear(t);
done:
  if (r) PQclear(r);
  free(w);
  PQfinish(db);
}

// GET, signed NIP-98: the signer's credit, {"credit_msat": N}.
static void credit_balance(const request *q) {
  char auth[4096];
  if (!api_header(q, "Authorization", auth, sizeof auth) || strncmp(auth, "Nostr ", 6)) { api_error(q->fd, 401, "sign the request (NIP-98)"); return; }
  char url[4200]; snprintf(url, sizeof url, "%s%s", env_or("OA_API_ORIGIN", "https://openagents.com"), q->target);
  nostr_event ev; char err[160];
  if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) {
    api_error(q->fd, 401, err); return;
  }
  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "no database"); return; }
  const char *v[1] = {ev.pubkey};
  PGresult *r = PQexecParams(db, "SELECT coalesce((SELECT msat FROM inference.credit WHERE pubkey = $1), 0)", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); api_error(q->fd, 500, "not read"); return; }
  char out[80]; api_num_text(out, sizeof out, "credit_msat", atoll(PQgetvalue(r, 0, 0)));
  PQclear(r);
  api_json(q->fd, 200, out);
}

static bool credit_route(const request *q) {
  if (!strcmp(q->method, "GET")) { credit_balance(q); return true; }
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST {pubkey, msat}"); return true; }
  if (q->body_len > 2048) { api_error(q->fd, 413, "too big"); return true; }
  char body[2049]; memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  char sig[8192];
  bool proof = api_header(q, "PAYMENT-SIGNATURE", sig, sizeof sig) != NULL;
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { credit_answer(q->fd, body, proof ? sig : NULL); _exit(0); }
  if (pid < 0) api_error(q->fd, 503, "busy");
  return true;
}

// The relay: Vertex's status line once its first bytes come, then its bytes
// as they come; everything also kept (to 4 MB) for the usage at the end.
// hold (the local serve mode): an answer that isn't a 200 is kept, not relayed (held), and answered in words.
typedef struct { int fd; CURL *h; bool stream, started, gone, hold, held; buf all; } relay_t;
static bool send_all(int fd, const char *p, size_t n) {
  while (n) { ssize_t k = write(fd, p, n); if (k <= 0) return false; p += k; n -= (size_t)k; }
  return true;
}
static void start(relay_t *r, long status) {
  char head[256];
  const char *type = r->stream && status == 200 ? "text/event-stream" : "application/json";
  int n = snprintf(head, sizeof head, "HTTP/1.1 %ld %s\r\nContent-Type: %s\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                   status, status == 200 ? "OK" : "Upstream", type);
  r->started = true;
  if (!send_all(r->fd, head, (size_t)n)) r->gone = true;
}
static size_t relay(char *data, size_t size, size_t n, void *user) {
  relay_t *r = user; size_t k = size * n;
  if (!r->started && !r->held) {
    long status = 0; curl_easy_getinfo(r->h, CURLINFO_RESPONSE_CODE, &status);
    if (r->hold && status != 200) r->held = true; else start(r, status ? status : 502);
  }
  if (r->held) { if (r->all.n < (1u << 16)) keep(&r->all, data, k); return k; }
  if (!r->gone && !send_all(r->fd, data, k)) r->gone = true;   // the caller left: stop asking Vertex
  if (r->all.n < (4u << 20)) keep(&r->all, data, k);
  return r->gone ? 0 : k;
}

// The last "name": N in text (a stream reports its usage in its last chunk), or 0.
static int last_int(const char *text, const char *name) {
  int v = 0; size_t nl = strlen(name);
  for (const char *p = text ? strstr(text, name) : NULL; p; p = strstr(p + nl, name)) {
    const char *q = p + nl;
    while (*q == ' ' || *q == ':') q++;
    v = atoi(q);
  }
  return v;
}

#ifdef OA_LOCAL_SERVE
// ---- the local serve mode's model (#13463), never in the image: Vertex on the token file's token, else
// our deployed API's /api/v1/inference signed NIP-98 by the developer's `openagents` login (the CLI signs:
// OA_INFERENCE_SIGNER is its binary, OA_INFERENCE_SIGNER_PROFILE its profile), so no key is pasted
// anywhere. A failure is answered in words that say why and what to run, not as the model's own body.
#include <spawn.h>
#include <sys/wait.h>
extern char **environ;
// "Nostr <base64>" for POST url with body, from the CLI, into out; false when it can't sign.
static bool local_sign(const char *url, const char *body, char *out, size_t cap) {
  const char *cli = getenv("OA_INFERENCE_SIGNER"), *profile = env_or("OA_INFERENCE_SIGNER_PROFILE", "default");
  if (!cli || !*cli) return false;
  char path[] = "/tmp/oa-local-sign-XXXXXX";
  int bf = mkstemp(path);
  if (bf < 0) return false;
  bool wrote = send_all(bf, body, strlen(body));
  close(bf);
  int p[2];
  if (!wrote || pipe(p)) { unlink(path); return false; }
  posix_spawn_file_actions_t fa;
  posix_spawn_file_actions_init(&fa);
  posix_spawn_file_actions_adddup2(&fa, p[1], 1);
  posix_spawn_file_actions_addclose(&fa, p[0]);
  posix_spawn_file_actions_addopen(&fa, 2, "/dev/null", O_WRONLY, 0);
  char *argv[] = {(char *)cli, "--profile", (char *)profile, "nostr", "auth", (char *)url, "--method", "POST", "--body", path, NULL};
  pid_t pid;
  signal(SIGCHLD, SIG_DFL);   // this child waits for its own
  int spawned = posix_spawn(&pid, cli, &fa, NULL, argv, environ);
  posix_spawn_file_actions_destroy(&fa);
  close(p[1]);
  size_t n = 0; ssize_t k;
  while (spawned == 0 && n + 1 < cap && (k = read(p[0], out + n, cap - 1 - n)) > 0) n += (size_t)k;
  close(p[0]);
  out[n] = 0;
  int st = 0;
  bool ok = spawned == 0 && waitpid(pid, &st, 0) == pid && WIFEXITED(st) && WEXITSTATUS(st) == 0;
  unlink(path);
  out[strcspn(out, "\r\n")] = 0;
  return ok && !strncmp(out, "Nostr ", 6);
}
// The words for a failed call: via is 'g' (Vertex), 'o' (our deployed API) or 0 (no model at all).
static const char *local_words(char via, long status) {
  if (via == 'g') {
    if (status == 401 || status == 403) return "Google turned down this computer's model token. Run gcloud auth login, then send it again.";
    if (status == 429) return "Google is limiting the model just now. Wait a minute, then send it again.";
    if (status == 0) return "We couldn't reach Google for the model. Check this computer's connection, then send it again.";
    return "The model didn't answer. Send it again; the local API's log (api.log, named when it started) says why.";
  }
  if (via == 'o') {
    if (status == -1) return "We couldn't sign the request with your openagents login. Run openagents login, then send it again.";
    if (status == 401) return "openagents.com didn't accept this computer's openagents login. Run openagents login, then send it again.";
    if (status == 402 || status == 403) return "openagents.com doesn't let this account use the model from a dev server. Run gcloud auth login to use your own Google login instead.";
    if (status == 429) return "openagents.com is limiting this account's model use just now. Wait a minute, then send it again.";
    if (status == 0) return "We couldn't reach openagents.com for the model. Check this computer's connection, then send it again.";
    return "openagents.com's model didn't answer. Send it again.";
  }
  return "We can't reach a model from this dev server: this computer has no gcloud login and no openagents login. "
         "Run gcloud auth login (or openagents login), then send it again.";
}
static void local_refuse(int fd, int status, const char *words) {
  api_error(fd, status, words);
}
#endif

// In the child: asks Vertex, relays its answer, keeps the call. original: the asker's own body (the local
// serve mode sends it on to our deployed API, which reads the model's name, not Vertex's).
static void answer(int fd, const char *who, const char *model, const char *request, const char *original, bool stream, bool free_key) {
  relay_t r = {.fd = fd, .stream = stream};
  char tok[12000], auth[12100];
  long status = 0;
  double t0 = api_now_ms();
  const char *url = env_or("OA_INFERENCE_URL", VERTEX_CHAT), *send = request;
  bool have = token(tok, sizeof tok);
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  (void)original;
#ifdef OA_LOCAL_SERVE
  char via = have ? 'g' : 0;
  const char *remote = getenv("OA_INFERENCE_REMOTE_URL");
  if (!have && remote && *remote && getenv("OA_INFERENCE_SIGNER")) {
    via = 'o'; url = remote; send = original;
    have = local_sign(remote, original, tok, sizeof tok);
    snprintf(auth, sizeof auth, "Authorization: %s", tok);
    if (!have) { local_refuse(fd, 502, local_words('o', -1)); r.started = true; }
  }
  if (!have && !via) { local_refuse(fd, 503, local_words(0, 0)); r.started = true; }
  r.hold = true;
#endif
  if (have) {
    r.h = curl_easy_init();
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    curl_easy_setopt(r.h, CURLOPT_URL, url);
    curl_easy_setopt(r.h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(r.h, CURLOPT_POSTFIELDS, send);
    curl_easy_setopt(r.h, CURLOPT_WRITEFUNCTION, relay);
    curl_easy_setopt(r.h, CURLOPT_WRITEDATA, &r);
    curl_easy_setopt(r.h, CURLOPT_TIMEOUT, 300L);
    if (curl_easy_perform(r.h) == CURLE_OK || r.started || r.held) curl_easy_getinfo(r.h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(r.h);
  } else api_log("inference: no token");
  memset(tok, 0, sizeof tok); memset(auth, 0, sizeof auth);
#ifdef OA_LOCAL_SERVE
  if (!r.started && via) local_refuse(fd, status == 401 || status == 403 || status == 0 ? 502 : (int)status, local_words(via, status));
  r.started = true;
#endif
  if (!r.started) api_error(fd, 502, "the model was not reached");
  char http[16], in[16], out[16], taken[16];
  snprintf(http, sizeof http, "%ld", status);
  snprintf(in, sizeof in, "%d", last_int(r.all.p, "\"prompt_tokens\""));
  snprintf(out, sizeof out, "%d", last_int(r.all.p, "\"completion_tokens\""));
  snprintf(taken, sizeof taken, "%d", (int)(api_now_ms() - t0));
  PGconn *db = api_db_new();
  const char *v[7] = {who, model, http, in, out, taken, free_key ? "true" : "false"};
  PGresult *q = db ? PQexecParams(db, "SELECT inference.finish($1, $2, $3::int, $4::int, $5::int, $6::int, $7::bool)", 7, NULL, v, NULL, NULL, 0) : NULL;
  if (!q || PQresultStatus(q) != PGRES_TUPLES_OK) api_log("inference: not kept: %s", db ? PQerrorMessage(db) : "no database");
  if (status != 200) api_log("inference: the model answered %ld", status);
  if (q) PQclear(q);
  if (db) PQfinish(db);
  free(r.all.p);
}

bool inference_route(const request *q) {
  if (!strcmp(q->path, "/api/v1/inference/credit")) return credit_route(q);
  if (strcmp(q->path, "/api/v1/inference")) return false;
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a Chat Completions request"); return true; }
  char auth[4096];
  if (!api_header(q, "Authorization", auth, sizeof auth) || strncmp(auth, "Nostr ", 6)) {
    api_error(q->fd, 401, "sign the request (NIP-98)"); return true;
  }
  char url[4200]; snprintf(url, sizeof url, "%s%s", env_or("OA_API_ORIGIN", "https://openagents.com"), q->target);
  nostr_event ev; char err[160];
  if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) {
    api_error(q->fd, 401, err); return true;
  }
  return inference_ask(q, ev.pubkey, admin_is(ev.pubkey) || listed(ev.pubkey));
}

// The request q's Chat Completions body asked of Vertex for this key, answered as it comes, under the
// same caps and metering as the signed route. /chat's door (chat.c, POST /api/chat/complete) calls
// this for an admin its cookie named, so the page asks without a Nostr signature.
bool inference_ask(const request *q, const char *pubkey, bool free_key) {
  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "no database"); return true; }
  if (!free_key) {
    // The wallet's daily limit holds for credit too (wallet.c, pay/wallet-policy.coax's wp_credit; #13330).
    if (wallet_credit_refused(q, db, pubkey)) return true;
    rate_refresh(db);
  }
  char *body = malloc(q->body_len + 1);
  if (!body) { api_error(q->fd, 503, "memory"); return true; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  char origin[600]; snprintf(origin, sizeof origin, "%s/api/v1/inference/credit", env_or("OA_API_ORIGIN", "https://openagents.com"));
  const char *v[6] = {pubkey, body, env_or("OA_INFERENCE_DAILY_REQUESTS", "500"), env_or("OA_INFERENCE_DAILY_USD_MICROS", "2000000"),
                      free_key ? "true" : "false", origin};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'error', (x->'request')::text, x->>'model', x->>'stream', "
                                 "(x - 'status' || CASE WHEN x ? 'credit' THEN jsonb_build_object('credit', $6::text) ELSE '{}' END)::text "
                                 "FROM inference.request($1, $2, $3::int, $4::bigint, $5::bool) x", 6, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("inference: %s", PQerrorMessage(db));
    PQclear(r); free(body);
    api_error(q->fd, 500, "not asked");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status == 402) {   // no credit: where to buy it
    char out[600]; snprintf(out, sizeof out, "%s", PQgetvalue(r, 0, 5));
    PQclear(r); free(body);
    api_json(q->fd, 402, out);
    return true;
  }
  if (status != 200) {
    char out[200]; api_error_text(out, sizeof out, PQgetvalue(r, 0, 1));
    PQclear(r); free(body);
    api_json(q->fd, status, out);
    return true;
  }
  char *request = strdup(PQgetvalue(r, 0, 2)), *model = strdup(PQgetvalue(r, 0, 3));
  bool stream = !strcmp(PQgetvalue(r, 0, 4), "true");
  PQclear(r);
  signal(SIGCHLD, SIG_IGN);   // children are never waited for
  pid_t pid = fork();
  if (pid == 0) {             // the child answers; the parent's database connection is never touched here
    answer(q->fd, pubkey, model, request, body, stream, free_key);
    _exit(0);
  }
  if (pid < 0) api_error(q->fd, 503, "busy");
  free(request); free(model); free(body);
  return true;   // the parent closes its copy of the connection
}
