// A realm's maker (ev-claim-realm, docs/verse/makers.md): who made a realm, and the Lightning
// address its tips go to. Every rule is SQL's (migrations/0072_realm_makers.sql); this file reads
// who asks.
//
//   GET  /api/playtest/makers/<realm>          -> 200 {realm, name, pubkey} (the maker also gets
//                                                 lightning and mine), 404 unclaimed
//   POST /api/playtest/makers/<realm>/claim {code, name, lightning}
//                                              -> 200 {realm, name, lightning}; 400, 401 not logged in,
//                                                 404 not this realm's code, 409 used already
//   POST /api/playtest/makers/<realm> {name?, lightning?}   the maker changes them; 403 anyone else
//   POST /api/admin/makers/<realm>/code        an admin (cookie or NIP-98): a new one-time code
//                                              -> 200 {realm, code, link}; the code is never logged or kept
//
// Tips (ev-maker-tips, migrations/0073_maker_tips.sql), Bitcoin over Lightning through our Spark wallet:
//   POST /api/playtest/makers/<realm>/tip {sats}        -> 200 {invoice, payment_hash, sats, expires}: our
//                                                         wallet's invoice; 404 unclaimed, 429, 502 no wallet
//   POST /api/playtest/makers/<realm>/tip/<hash>        -> 200 {state: open|paid|expired, sats, payout, to}:
//                                                         asks our wallet whether it is paid and settles it once
//   POST /api/admin/makers/<realm>/gift {sats}          an admin's gift from our wallet to the maker
//   GET  /api/admin/makers/payouts, POST .../payouts/run   the payouts as lines; run the due ones now
// A settled tip queues its payout in the same transaction; a child process sends due payouts from
// our wallet to the maker's Lightning address (quote, keep the maker's invoice, send that invoice
// once, ask our wallet what became of it) and retries a failed one later, backing off. The maker
// gets the tip less the routing fee; the first player tip to each maker also carries
// OA_MAKER_FIRST_TIP_BONUS_SATS (1,000; 0 ends the launch bonus).
//   OA_TIP_MIN_SATS, OA_TIP_MAX_SATS   a tip's bounds (10, 100,000)
//   OA_MAKER_DAY_PAYOUT_SATS           what payouts may send in a Chicago day (50,000)
//
// Who: the site session (oa_session, from /login) or a NIP-98 signature that binds the body and
// was never seen before. A POST from another site's page (Origin) is refused. The claim code
// travels in the link's fragment and the request body, never a path or query, so it never
// reaches a load balancer's logs.
#include "api.h"
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PREFIX "/api/playtest/makers/"
#define ADMIN_PREFIX "/api/admin/makers/"

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }

static bool slug_ok(const char *s, size_t n) {
  if (!n || n > 64) return false;
  for (size_t i = 0; i < n; i++) if (!((s[i] >= 'a' && s[i] <= 'z') || (s[i] >= '0' && s[i] <= '9') || s[i] == '-')) return false;
  return true;
}

static char *body_of(const request *q) {
  char *b = malloc(q->body_len + 3);
  if (!b) return NULL;
  if (q->body_len) { memcpy(b, q->body, q->body_len); b[q->body_len] = 0; } else strcpy(b, "{}");
  return b;
}

// One SQL call answering {"status", ...}: sent as its status and the rest.
static void answer(const request *q, PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("makers: %s", PQerrorMessage(db));
    api_json(q->fd, 500, "{\"error\":\"not kept\"}");
  } else api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
}

// The logged-in key, or false (and the answer sent).
static bool who_asks(const request *q, PGconn *db, char who[65], bool post) {
  bool signed_req = false, payload = false, unseen = false;
  if (!site_identity(q, db, who, &signed_req, &payload, &unseen)) {
    api_json(q->fd, 401, "{\"error\":\"log in first\"}");
    return false;
  }
  if (signed_req && post && (!payload || !unseen)) {   // a signed write binds its body, once
    api_json(q->fd, 401, "{\"error\":\"log in first\"}");
    return false;
  }
  return true;
}

static void make_code(const request *q, PGconn *db, const char *realm) {
  char who[65];
  int s = admin_who(q, db, who);
  if (s) { api_json(q->fd, s, s == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return; }
  unsigned char raw[16]; char code[33];
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, raw, sizeof raw) != (ssize_t)sizeof raw) { if (fd >= 0) close(fd); api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return; }
  close(fd);
  for (int i = 0; i < 16; i++) snprintf(code + 2 * i, 3, "%02x", raw[i]);
  const char *v[3] = {realm, code, who};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x::text FROM makers.make_code($1, $2, $3) x", 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("makers: code: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not kept\"}"); }
  else if (atoi(PQgetvalue(r, 0, 0)) != 200) api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  else {
    char out[512];
    snprintf(out, sizeof out, "{\"realm\":\"%s\",\"code\":\"%s\",\"link\":\"%s/realms/%s/claim#code=%s\"}", realm, code, origin(), realm, code);
    api_json(q->fd, 200, out);
    api_log("makers: a claim code for %s", realm);
  }
  PQclear(r);
  memset(code, 0, sizeof code);
}


// ---- tips and payouts

// A value of a JSON text, by SQL (""): the wallet's answers are read the way every answer here is.
static void jget(PGconn *db, const char *json, const char *key, char *out, size_t cap) {
  const char *v[2] = {json ? json : "{}", key};
  out[0] = 0;
  PGresult *r = PQexecParams(db, "SELECT coalesce($1::jsonb->>$2, '')", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
}
static void outcome(PGconn *db, const char *tip, const char *what, const char *fee, const char *why, bool requote) {
  const char *v[5] = {tip, what, fee && *fee ? fee : NULL, why, requote ? "true" : "false"};
  PGresult *r = PQexecParams(db, "SELECT makers.payout_result($1, $2, $3::bigint, $4, $5::boolean)", 5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("makers: payout result: %s", PQerrorMessage(db));
  PQclear(r);
  api_log("makers: payout %s", what);
}
// Sends what is due, a few at a time. Each payout pays one maker invoice, kept before it is sent;
// a payout whose invoice may be in flight is asked about (/sent) before anything is sent again.
static void run_payouts(PGconn *db) {
  const char *cap[1] = {env_or("OA_MAKER_DAY_PAYOUT_SATS", "50000")};
  for (int k = 0; k < 8; k++) {
    PGresult *r = PQexecParams(db, "SELECT x->>'tip', x->>'lightning', x->>'sats', x->>'invoice', x->>'expired', x->>'realm' "
                                   "FROM makers.payout_next(150, $1::bigint) x WHERE x IS NOT NULL", 1, NULL, cap, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("makers: payouts: %s", PQerrorMessage(db)); PQclear(r); return; }
    char tip[80], ln[160], sats[24], inv[2100], realm[80]; bool expired = !strcmp(PQgetvalue(r, 0, 4), "true");
    snprintf(tip, sizeof tip, "%s", PQgetvalue(r, 0, 0)); snprintf(ln, sizeof ln, "%s", PQgetvalue(r, 0, 1));
    snprintf(sats, sizeof sats, "%s", PQgetvalue(r, 0, 2)); snprintf(inv, sizeof inv, "%s", PQgetvalue(r, 0, 3));
    snprintf(realm, sizeof realm, "%s", PQgetvalue(r, 0, 5));
    PQclear(r);
    char body[2400], state[32], fee[24], err[200], *w = NULL;
    if (inv[0]) {   // an invoice kept before: what became of it?
      snprintf(body, sizeof body, "{\"invoice\":\"%s\"}", inv);
      long st = api_spark("/sent", body, 40, &w);
      if (st != 200) { free(w); outcome(db, tip, "failed", NULL, "our wallet did not answer", false); continue; }
      jget(db, w, "state", state, sizeof state); jget(db, w, "fee_sats", fee, sizeof fee); free(w); w = NULL;
      if (!strcmp(state, "completed")) { outcome(db, tip, "sent", fee, NULL, false); continue; }
      if (!strcmp(state, "pending")) { outcome(db, tip, "pending", NULL, NULL, false); continue; }
      if (!strcmp(state, "failed") || expired) inv[0] = 0;   // a new invoice: this one can no longer be paid
    }
    if (!inv[0]) {
      // what our wallet holds: short of this payout, a first tip's bonus waits for funding, else all of it waits
      long st0 = api_spark("/health", NULL, 20, &w);
      char bal[24] = "";
      if (st0 == 200) jget(db, w, "balance_sat", bal, sizeof bal);
      free(w); w = NULL;
      long need = atol(sats) + atol(sats) / 50 + 10;
      if (st0 == 200 && *bal && atol(bal) < need) {
        const char *dv[1] = {tip};
        PGresult *dr = PQexecParams(db, "SELECT makers.defer_bonus($1)", 1, NULL, dv, NULL, NULL, 0);
        bool deferred = PQresultStatus(dr) == PGRES_TUPLES_OK && PQntuples(dr) == 1 && !PQgetisnull(dr, 0, 0);
        if (deferred) snprintf(sats, sizeof sats, "%s", PQgetvalue(dr, 0, 0));
        PQclear(dr);
        need = atol(sats) + atol(sats) / 50 + 10;
        if (atol(bal) < need) {
          snprintf(err, sizeof err, "our wallet holds %s sats; this payout needs %ld", bal, need);
          outcome(db, tip, "failed", NULL, err, false);
          continue;
        }
        api_log("makers: a first tip's bonus waits for funding");
      }
      snprintf(body, sizeof body, "{\"address\":\"%s\",\"sats\":%s,\"comment\":\"A tip from a player of %s on OpenAgents\"}", ln, sats, realm);
      long st = api_spark("/quote", body, 40, &w);
      if (st != 200) { jget(db, w, "error", err, sizeof err); free(w); outcome(db, tip, "failed", NULL, *err ? err : "no invoice from the address", true); continue; }
      char hash[80], amount[24], expiry[24];
      jget(db, w, "invoice", inv, sizeof inv); jget(db, w, "payment_hash", hash, sizeof hash); jget(db, w, "amount_sats", amount, sizeof amount);
      jget(db, w, "fee_sats", fee, sizeof fee); jget(db, w, "expiry", expiry, sizeof expiry); free(w); w = NULL;
      const char *v[6] = {tip, inv, hash, amount, fee, expiry};
      PGresult *k2 = PQexecParams(db, "SELECT makers.payout_quoted($1, $2, $3, $4::bigint, $5::bigint, $6::bigint)", 6, NULL, v, NULL, NULL, 0);
      bool kept = PQresultStatus(k2) == PGRES_TUPLES_OK && PQntuples(k2) == 1 && !strcmp(PQgetvalue(k2, 0, 0), "t");
      if (PQresultStatus(k2) != PGRES_TUPLES_OK) api_log("makers: quoted: %s", PQerrorMessage(db));
      PQclear(k2);
      if (!kept) { outcome(db, tip, "failed", NULL, "the address's invoice did not fit", true); continue; }
    }
    // the fee may differ a little from the quote's; at most this many sats of our own on top
    snprintf(body, sizeof body, "{\"invoice\":\"%s\",\"max_fee_sats\":%ld}", inv, atol(sats) / 50 + 10 > 1000 ? 1000L : atol(sats) / 50 + 10);
    long st = api_spark("/send", body, 60, &w);
    jget(db, w, "state", state, sizeof state); jget(db, w, "fee_sats", fee, sizeof fee); jget(db, w, "error", err, sizeof err); free(w); w = NULL;
    if (st == 200 && !strcmp(state, "completed")) outcome(db, tip, "sent", fee, NULL, false);
    else if (st == 200 && !strcmp(state, "pending")) outcome(db, tip, "pending", NULL, NULL, false);
    else if (st == 200) outcome(db, tip, "failed", NULL, "the payment failed", true);
    else outcome(db, tip, "failed", NULL, st ? (*err ? err : "our wallet refused it") : "our wallet did not answer", false);   // unknown: ask /sent next time
  }
}
// In a child of its own (the wallet takes seconds): answer fd with tip_answer when it's set, then
// send what is due.
typedef void (*answer_fn)(int fd, PGconn *db, const char *slug, const char *a, const char *b);
static void in_child(int fd, const char *slug, const char *a, const char *b, answer_fn answer_with) {
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid != 0) { if (pid < 0 && fd >= 0) api_json(fd, 503, "{\"error\":\"busy\"}"); return; }
  PGconn *db = api_db_new();
  if (!db) { if (fd >= 0) api_json(fd, 503, "{\"error\":\"no database\"}"); _exit(0); }
  if (answer_with) answer_with(fd, db, slug, a, b);
  if (fd >= 0) close(fd);
  run_payouts(db);
  PQfinish(db);
  _exit(0);
}
static void sql_answer(int fd, PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("makers: %s", PQerrorMessage(db)); api_json(fd, 500, "{\"error\":\"not kept\"}"); }
  else api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  PQclear(r);
}
// A new tip: its terms, our wallet's invoice, kept. a: the body; b: "1" when our team asks.
static void tip_new(int fd, PGconn *db, const char *slug, const char *body, const char *team) {
  const char *v[4] = {slug, body, env_or("OA_TIP_MIN_SATS", "10"), env_or("OA_TIP_MAX_SATS", "100000")};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text, x->>'sats', x->>'memo', "
                                 "(SELECT count(*) FROM makers.tips WHERE created > now() - interval '1 minute') "
                                 "FROM makers.tip_terms($1, $2, $3::bigint, $4::bigint) x", 4, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("makers: tip: %s", PQerrorMessage(db)); api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(r); return; }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) { api_json(fd, status, PQgetvalue(r, 0, 1)); PQclear(r); return; }
  if (atoi(PQgetvalue(r, 0, 4)) >= 60) { api_json(fd, 429, "{\"error\":\"too many tips at once; try again in a minute\"}"); PQclear(r); return; }
  char sats[24], ask[400], *w = NULL;
  snprintf(sats, sizeof sats, "%s", PQgetvalue(r, 0, 2));
  const char *mv[1] = {PQgetvalue(r, 0, 3)};
  PGresult *j = PQexecParams(db, "SELECT json_build_object('sats', $2::bigint, 'memo', $1::text, 'expiry', 900)::text", 2, NULL, (const char *[]){mv[0], sats}, NULL, NULL, 0);
  snprintf(ask, sizeof ask, "%s", PQresultStatus(j) == PGRES_TUPLES_OK ? PQgetvalue(j, 0, 0) : "{}");
  PQclear(j); PQclear(r);
  long st = api_spark("/invoice", ask, 30, &w);
  if (st != 200 || !w) { free(w); api_json(fd, 502, "{\"error\":\"our wallet did not make an invoice; try again soon\"}"); return; }
  char inv[2100], hash[80], msat[24], exp[24];
  jget(db, w, "invoice", inv, sizeof inv); jget(db, w, "payment_hash", hash, sizeof hash); jget(db, w, "amount_msat", msat, sizeof msat); jget(db, w, "expiry", exp, sizeof exp);
  free(w);
  const char *iv[7] = {slug, sats, inv, hash, msat, exp, team};
  sql_answer(fd, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.tip_issue($1, $2::bigint, $3, $4, $5::bigint, $6::bigint, $7::boolean) x", 7, iv);
}
// Is it paid? Our wallet's record of the invoice, settled once.
static void tip_check(int fd, PGconn *db, const char *slug, const char *hash, const char *unused) {
  (void)unused;
  const char *v[2] = {slug, hash};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'state', x->>'invoice', (x - 'status' - 'invoice')::text FROM makers.tip_state($1, $2) x", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(r); return; }
  if (atoi(PQgetvalue(r, 0, 0)) != 200 || strcmp(PQgetvalue(r, 0, 1), "open")) { api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 3)); PQclear(r); return; }
  char body[2400], *w = NULL;
  snprintf(body, sizeof body, "{\"invoice\":\"%s\"}", PQgetvalue(r, 0, 2));
  PQclear(r);
  long st = api_spark("/paid", body, 30, &w);
  if (st == 200 && w) {
    char state[32], amount[24], pre[80];
    jget(db, w, "state", state, sizeof state); jget(db, w, "amount_msat", amount, sizeof amount); jget(db, w, "preimage", pre, sizeof pre);
    const char *sv[5] = {hash, state, *amount ? amount : NULL, pre, env_or("OA_MAKER_FIRST_TIP_BONUS_SATS", "1000")};
    PGresult *k = PQexecParams(db, "SELECT x->>'status' FROM makers.tip_settle($1, $2, $3::bigint, $4, $5::bigint) x", 5, NULL, sv, NULL, NULL, 0);
    if (PQresultStatus(k) != PGRES_TUPLES_OK) api_log("makers: settle: %s", PQerrorMessage(db));
    else if (!strcmp(PQgetvalue(k, 0, 0), "200")) api_log("makers: a tip for %s paid", slug);
    PQclear(k);
  }
  free(w);
  sql_answer(fd, db, "SELECT (x->>'status')::int, (x - 'status' - 'invoice')::text FROM makers.tip_state($1, $2) x", 2, v);
}
static bool hex64(const char *s) {
  if (strlen(s) != 64) return false;
  for (int i = 0; i < 64; i++) if (!((s[i] >= '0' && s[i] <= '9') || (s[i] >= 'a' && s[i] <= 'f'))) return false;
  return true;
}
static bool team_asks(const request *q, PGconn *db) {
  char cookies[4096], who[65];
  if (api_header(q, "Cookie", cookies, sizeof cookies)) {
    for (const char *c = strstr(cookies, "oa_team=1"); c; c = strstr(c + 1, "oa_team=1"))
      if (c == cookies || c[-1] == ' ' || c[-1] == ';') return true;
  }
  bool a, b, c;
  return site_identity(q, db, who, &a, &b, &c) && admin_is(who);
}

bool makers_route(const request *q) {
  bool admin = !strncmp(q->path, ADMIN_PREFIX, sizeof ADMIN_PREFIX - 1);
  if (!admin && strncmp(q->path, PREFIX, sizeof PREFIX - 1)) return false;
  const char *realm = q->path + (admin ? sizeof ADMIN_PREFIX : sizeof PREFIX) - 1;
  const char *slash = strchr(realm, '/');
  size_t n = slash ? (size_t)(slash - realm) : strlen(realm);
  const char *what = slash ? slash + 1 : "";
  if (!slug_ok(realm, n)) { api_json(q->fd, 404, "{\"error\":\"no such realm\"}"); return true; }
  char slug[65]; memcpy(slug, realm, n); slug[n] = 0;
  bool post = !strcmp(q->method, "POST");
  char from[256];
  if (post && api_header(q, "Origin", from, sizeof from) && strcmp(from, origin())) {
    api_json(q->fd, 403, "{\"error\":\"cross-site\"}");
    return true;
  }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  if (admin) {
    if (!strcmp(slug, "payouts")) {   // the payouts as lines, and run the due ones
      char who[65];
      int st = admin_who(q, db, who);
      if (st) { api_json(q->fd, st, st == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return true; }
      if (post && !strcmp(what, "run")) {   // every payout not sent is due now
        PGresult *u = PQexec(db, "UPDATE makers.payouts SET next_try = now() WHERE state <> 'sent' AND (lease IS NULL OR lease < now())");
        PQclear(u);
        in_child(-1, slug, NULL, NULL, NULL); api_json(q->fd, 200, "{\"running\":true}"); return true;
      }
      PGresult *r = PQexec(db, "SELECT makers.payout_lines()");
      const char *t = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? PQgetvalue(r, 0, 0) : "";
      api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, t, strlen(t));
      PQclear(r);
      return true;
    }
    if (!post) { api_json(q->fd, 405, "{\"error\":\"POST\"}"); return true; }
    if (!strcmp(what, "code")) { make_code(q, db, slug); return true; }
    if (!strcmp(what, "gift")) {
      char who[65];
      int st = admin_who(q, db, who);
      if (st) { api_json(q->fd, st, st == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return true; }
      char *body = body_of(q);
      const char *v[3] = {slug, body ? body : "{}", env_or("OA_MAKER_GIFT_MAX_SATS", "20000")};
      PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.gift($1, ($2::jsonb->>'sats')::bigint, $3::bigint) x", 3, NULL, v, NULL, NULL, 0);
      free(body);
      if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("makers: gift: %s", PQerrorMessage(db)); api_json(q->fd, 400, "{\"error\":\"sats must be a whole number\"}"); }
      else {
        int status = atoi(PQgetvalue(r, 0, 0));
        api_json(q->fd, status, PQgetvalue(r, 0, 1));
        if (status == 200) { api_log("makers: a gift for %s", slug); in_child(-1, slug, NULL, NULL, NULL); }
      }
      PQclear(r);
      return true;
    }
    api_json(q->fd, 404, "{\"error\":\"no such route\"}");
    return true;
  }
  if (post && !strcmp(what, "tip")) {
    if (q->body_len > 512) { api_json(q->fd, 413, "{\"error\":\"too big\"}"); return true; }
    char body[520]; memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
    in_child(q->fd, slug, q->body_len ? body : "{}", team_asks(q, db) ? "true" : "false", tip_new);
    return true;
  }
  if (post && !strncmp(what, "tip/", 4)) {
    if (!hex64(what + 4)) { api_json(q->fd, 404, "{\"error\":\"no such tip\"}"); return true; }
    in_child(q->fd, slug, what + 4, NULL, tip_check);
    return true;
  }
  if (!*what && !post) {
    char who[65] = ""; bool a, b, c;
    if (!site_identity(q, db, who, &a, &b, &c)) who[0] = 0;
    const char *v[2] = {slug, who};
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.of($1, $2) x", 2, v);
    return true;
  }
  if (!post || (*what && strcmp(what, "claim"))) { api_json(q->fd, post ? 404 : 405, post ? "{\"error\":\"no such route\"}" : "{\"error\":\"POST\"}"); return true; }
  char who[65];
  if (!who_asks(q, db, who, true)) return true;
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  const char *v[3] = {slug, who, body};
  if (*what) {
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.claim($1, $2, $3) x", 3, v);
    api_log("makers: a claim for %s", slug);
  } else answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.edit($1, $2, $3) x", 3, v);
  memset(body, 0, q->body_len);
  free(body);
  return true;
}
