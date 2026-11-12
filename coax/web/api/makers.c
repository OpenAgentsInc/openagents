// A realm's maker (ev-claim-realm, docs/verse/makers.md): who made a realm, and the Lightning
// address its tips go to. Every rule is SQL's (migrations/0072_realm_makers.sql); this file reads
// who asks.
//
//   GET  /api/playtest/makers/<realm>          -> 200 {realm, state: claimed, name, pubkey} (the maker
//                                                 also gets lightning and mine), or 200 {realm, state:
//                                                 unclaimed} (0084); 404 a bad slug
//   POST /api/playtest/makers/<realm>/claim {code, name, lightning}
//                                              -> 200 {realm, name, lightning}; 400, 401 not logged in,
//                                                 404 not this realm's code, 409 used already
//   POST /api/playtest/makers/<realm> {name?, lightning?}   the maker changes them; 403 anyone else
//   POST /api/admin/makers/<realm>/code        an admin (cookie or NIP-98): a new one-time code
//                                              -> 200 {realm, code, link}; the code is never logged or kept
//
// Tips (docs/verse/makers.md; owner decision D6, #13210): Bitcoin over Lightning, from the player's
// wallet straight to the maker's own Lightning Address. Nothing of the player's enters our wallet.
//   POST /api/playtest/makers/<realm>/tip {sats}        -> 200 {invoice, payment_hash, sats, expires, confirm, to}: the
//                                                         maker's own invoice, asked by our server (the player's
//                                                         network address never reaches the maker's service) and
//                                                         checked: the exact amount, the description hash, the expiry.
//                                                         404 unclaimed, 503 the maker changed address (ask again), 429, 502
//   POST /api/playtest/makers/<realm>/tip/<hash> [{preimage}]
//                                                      -> 200 {state: open|paid|expired, sats, confirm, to, bonus}:
//                                                         paid only when confirmed, by the maker's service (LUD-21)
//                                                         or by the preimage the player's wallet reports; counted
//                                                         once. 402 a preimage that isn't this invoice's
//   POST /api/admin/makers/<realm>/gift {sats}          an admin's gift from our wallet to the maker
//   GET  /api/admin/makers/payouts, POST .../payouts/run   the payouts as lines; run the due ones now
// Every rule is makerpay-policy.coax's (mp_*, in lnaddr-policy.generated.h); the LNURL fetch and its
// checks are lnaddr.c's and coax/lib/lnaddr/lnurl.coax's. Our wallet sends only our own money: a
// maker's first confirmed tip carries OA_MAKER_FIRST_TIP_BONUS_SATS (1,000; 0 ends the launch
// bonus) as a payment of its own, and an admin's gift. A child process sends due payouts (quote,
// keep the maker's invoice, send that invoice once, ask our wallet what became of it) and retries a
// failed one later, backing off. A tip paid into our wallet before D6 is settled and paid out the
// old way; none is made after.
//   OA_TIP_MIN_SATS, OA_TIP_MAX_SATS   a tip's bounds (10, 100,000)
//   OA_MAKER_DAY_PAYOUT_SATS           what payouts may send in a Chicago day (50,000)
//
// Who: a logged-in session (auth.c, from /login) or a NIP-98 signature that binds the body and
// was never seen before. A POST from another site's page (Origin) is refused. The claim code
// travels in the link's fragment and the request body, never a path or query, so it never
// reaches a load balancer's logs.
#include "api.h"
#include "lnaddr-policy.generated.h"
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PREFIX "/api/playtest/makers/"
#define ADMIN_PREFIX "/api/admin/makers/"

// makerpay-policy.coax's numbers this file names; makers_init checks them against the Coax.
enum { MP_TIP = 0, MP_UPGRADE = 1, MP_NO_PROOF = 0, MP_BY_VERIFY = 1, MP_BY_WALLET = 2, MP_OK = 0, MP_MOVED = 2, MP_BAD = 4,
       MP_FAKE = 10, MP_NO_TIP = 11, MP_S_PAID = 3, MP_S_FAKE = 5, MP_SRC_TIP = 0, MP_SRC_UPGRADE = 1, MP_SRC_BONUS = 2, MP_SRC_GIFT = 3 };
static coax_string cs(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }

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
    snprintf(out, sizeof out, "{\"realm\":\"%s\",\"code\":\"%s\",\"link\":\"%s/worlds/%s/claim#code=%s\"}", realm, code, origin(), realm, code);
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
    PGresult *r = PQexecParams(db, "SELECT x->>'tip', x->>'lightning', x->>'sats', x->>'invoice', x->>'expired', x->>'realm', x->>'source', x->>'direct' "
                                   "FROM makers.payout_next(150, $1::bigint) x WHERE x IS NOT NULL", 1, NULL, cap, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("makers: payouts: %s", PQerrorMessage(db)); PQclear(r); return; }
    char tip[80], ln[160], sats[24], inv[2100], realm[80]; bool expired = !strcmp(PQgetvalue(r, 0, 4), "true");
    snprintf(tip, sizeof tip, "%s", PQgetvalue(r, 0, 0)); snprintf(ln, sizeof ln, "%s", PQgetvalue(r, 0, 1));
    snprintf(sats, sizeof sats, "%s", PQgetvalue(r, 0, 2)); snprintf(inv, sizeof inv, "%s", PQgetvalue(r, 0, 3));
    snprintf(realm, sizeof realm, "%s", PQgetvalue(r, 0, 5));
    const char *sv = PQgetvalue(r, 0, 6);
    int source = !strcmp(sv, "bonus") ? MP_SRC_BONUS : !strcmp(sv, "gift") ? MP_SRC_GIFT : !strcmp(sv, "upgrade") ? MP_SRC_UPGRADE : !strcmp(sv, "tip") ? MP_SRC_TIP : -1;
    bool direct = !strcmp(PQgetvalue(r, 0, 7), "true");
    PQclear(r);
    char body[2400], state[32], fee[24], err[200], *w = NULL;
    if (!mp_payout(source, direct)) { outcome(db, tip, "failed", NULL, "not ours to send: the player paid the maker", false); continue; }
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
      snprintf(body, sizeof body, source == MP_SRC_BONUS ? "{\"address\":\"%s\",\"sats\":%s,\"comment\":\"Your first tip on %s: a bonus from OpenAgents\"}"
                                : source == MP_SRC_GIFT ? "{\"address\":\"%s\",\"sats\":%s,\"comment\":\"A gift from OpenAgents for %s\"}"
                                : "{\"address\":\"%s\",\"sats\":%s,\"comment\":\"A tip from a player of %s on OpenAgents\"}", ln, sats, realm);
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
// What the player's wallet reported with this request, for the child that answers it: the preimage
// ("" for none), and whether the wallet reports one at all.
static char asked_pre[100];
static bool asked_reports;
static void read_wallet(const request *q, PGconn *db) {
  asked_pre[0] = 0; asked_reports = false;
  if (!q->body_len || q->body_len > 512) return;
  char body[520], reports[8];
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  jget(db, body, "preimage", asked_pre, sizeof asked_pre);
  jget(db, body, "reports", reports, sizeof reports);
  asked_reports = !strcmp(reports, "true");
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
// ---- a payment straight to the maker (makerpay-policy.coax decides; lnaddr.c asks the address)
static void mp_refuse(int fd, int64_t code) {
  coax_string w = mp_text(code);
  char json[400];
  snprintf(json, sizeof json, "{\"error\":\"%.*s\"}", (int)w.count, w.data);
  api_json(fd, (int)mp_status(code), json);
}
// The maker's own invoice for sats, from their address addr, checked and kept: a tip for realm slug
// (listing NULL) or player key's purchase of listing. reports: the player's wallet reports the
// preimage. Answers fd.
static void direct_new(int fd, PGconn *db, const char *slug, const char *listing, const char *key, const char *addr, const char *sats,
                       const char *team, bool reports, long recent) {
  int kind = listing ? MP_UPGRADE : MP_TIP;
  int64_t code = mp_limit(recent);
  if (code != MP_OK) { mp_refuse(fd, code); return; }
  static lnaddr_quote q;
  int64_t lu = lnaddr_invoice_of(db, addr, atoll(sats) * 1000, &q);
  code = mp_quote(lu);
  if (code != MP_OK) { api_log("makers: %s's address refused %lld", slug, (long long)lu); mp_refuse(fd, code); return; }
  // the maker's address now: still the one the invoice came from?
  const char *fv[2] = {slug, listing};
  PGresult *f = PQexecParams(db, listing ? "SELECT lightning FROM makers.listings WHERE id = $2::bigint AND state = 'approved' AND $1::text IS NOT NULL"
                                         : "SELECT lightning FROM makers.realms WHERE realm = $1 AND $2::text IS NULL", 2, NULL, fv, NULL, NULL, 0);
  bool claimed = PQresultStatus(f) == PGRES_TUPLES_OK && PQntuples(f) == 1;
  bool same = claimed && !strcmp(PQgetvalue(f, 0, 0), addr);
  PQclear(f);
  code = mp_issue(kind, claimed, same, q.verify[0] != 0, reports);
  if (code != MP_OK) { mp_refuse(fd, code); return; }
  char exp[24];
  snprintf(exp, sizeof exp, "%lld", q.expires);
  const char *kv[10] = {slug, listing, key, addr, sats, q.pr, q.payment_hash, q.verify, exp, team};
  PGresult *k = PQexecParams(db, "SELECT (x->>'status')::int FROM makers.direct_keep($1, $2::bigint, makers.player_of($3), $4, $5::bigint, $6, $7, $8, $9::bigint, $10::boolean) x",
                             10, NULL, kv, NULL, NULL, 0);
  int kept = PQresultStatus(k) == PGRES_TUPLES_OK && PQntuples(k) == 1 ? atoi(PQgetvalue(k, 0, 0)) : 500;
  if (kept == 500) api_log("makers: keep: %s", PQerrorMessage(db));
  PQclear(k);
  if (kept == 500) { api_json(fd, 500, "{\"error\":\"not kept\"}"); return; }
  if (kept != 200) { mp_refuse(fd, kept == 502 ? MP_BAD : mp_issue(kind, kept != 404, false, true, true)); return; }
  const char *av[3] = {q.payment_hash, q.pr, exp};
  sql_answer(fd, db, "SELECT 200, (jsonb_build_object('invoice', $2::text, 'payment_hash', $1::text, 'expires', $3::bigint) || makers.direct_shown($1))::text", 3, av);
  api_log("makers: %s's own invoice shown", slug);
}
// Where a direct payment stands, confirmed here when it can be: by the preimage the player's wallet
// reported (pre, "" for none), else by the maker's service (LUD-21). facts is makers.direct_facts'
// answer. Counted once; a first tip's bonus queued as our own payment. Answers fd.
static void direct_check(int fd, PGconn *db, const char *slug, const char *hash, const char *pre, const char *facts) {
  char found[8], direct[8], paid[8], expired[8], verify[1100], purchase[8], had[8], got[65] = "";
  jget(db, facts, "found", found, sizeof found); jget(db, facts, "direct", direct, sizeof direct); jget(db, facts, "paid", paid, sizeof paid);
  jget(db, facts, "expired", expired, sizeof expired); jget(db, facts, "verify", verify, sizeof verify);
  jget(db, facts, "purchase", purchase, sizeof purchase); jget(db, facts, "had_bonus", had, sizeof had);
  bool is_paid = !strcmp(paid, "true");
  int proof = MP_NO_PROOF; bool ok = false;
  if (!is_paid && pre[0]) { proof = MP_BY_WALLET; ok = mp_proof(cs(pre), cs(hash)); if (ok) snprintf(got, sizeof got, "%s", pre); }
  else if (!is_paid && verify[0]) { proof = MP_BY_VERIFY; ok = lnaddr_settled(verify, hash, got); }
  int64_t state = mp_check(!strcmp(found, "true"), !strcmp(direct, "true"), is_paid, proof, ok, !strcmp(expired, "true"));
  if (mp_counts(state)) {
    char bonus[24];
    snprintf(bonus, sizeof bonus, "%lld", (long long)mp_bonus(!strcmp(purchase, "true") ? MP_UPGRADE : MP_TIP, state, !strcmp(had, "true"),
                                                                atoll(env_or("OA_MAKER_FIRST_TIP_BONUS_SATS", "1000"))));
    const char *cv[4] = {hash, proof == MP_BY_VERIFY ? "verify" : "wallet", got, bonus};
    PGresult *k = PQexecParams(db, "SELECT (x->>'status')::int FROM makers.direct_confirm($1, $2, $3, $4::bigint) x", 4, NULL, cv, NULL, NULL, 0);
    int st = PQresultStatus(k) == PGRES_TUPLES_OK && PQntuples(k) == 1 ? atoi(PQgetvalue(k, 0, 0)) : 500;
    if (st == 500) api_log("makers: confirm: %s", PQerrorMessage(db));
    PQclear(k);
    if (st != 200) { api_json(fd, st == 500 ? 500 : 409, "{\"error\":\"not counted; ask again\"}"); return; }
    api_log("makers: a payment to %s's maker confirmed by %s", slug, cv[1]);
  }
  int status = (int)mp_state_status(state);
  if (status != 200) { mp_refuse(fd, state == MP_S_FAKE ? MP_FAKE : MP_NO_TIP); return; }
  coax_string word = mp_state_text(state);
  char st[16];
  snprintf(st, sizeof st, "%.*s", (int)word.count, word.data);
  const char *av[2] = {hash, st};
  sql_answer(fd, db, "SELECT 200, (jsonb_build_object('state', $2::text) || makers.direct_shown($1))::text", 2, av);
}
// A tip: the terms, then the maker's own invoice. a: the body; b: "true" when our team asks.
static void tip_new(int fd, PGconn *db, const char *slug, const char *body, const char *team) {
  const char *v[4] = {slug, body, env_or("OA_TIP_MIN_SATS", "10"), env_or("OA_TIP_MAX_SATS", "100000")};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status')::text, x->>'sats', x->>'lightning', "
                                 "(SELECT count(*) FROM makers.tips WHERE created > now() - interval '1 minute') "
                                 "FROM makers.tip_terms($1, $2, $3::bigint, $4::bigint) x", 4, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("makers: tip: %s", PQerrorMessage(db)); api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(r); return; }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) { api_json(fd, status, PQgetvalue(r, 0, 1)); PQclear(r); return; }
  char sats[24], addr[200];
  snprintf(sats, sizeof sats, "%s", PQgetvalue(r, 0, 2)); snprintf(addr, sizeof addr, "%s", PQgetvalue(r, 0, 3));
  long recent = atol(PQgetvalue(r, 0, 4));
  PQclear(r);
  direct_new(fd, db, slug, NULL, NULL, addr, sats, team, false, recent);
}
// Is a tip paid? b: the preimage the player's wallet reported, or "". A tip whose invoice was our
// wallet's (before D6) is settled from our wallet's record, once, as it always was.
static void tip_check(int fd, PGconn *db, const char *slug, const char *hash, const char *pre) {
  const char *dv[2] = {hash, slug};
  PGresult *d = PQexecParams(db, "SELECT x::text, (x->>'found')::boolean AND NOT (x->>'direct')::boolean FROM makers.direct_facts($1, $2, NULL, NULL) x", 2, NULL, dv, NULL, NULL, 0);
  if (PQresultStatus(d) != PGRES_TUPLES_OK || PQntuples(d) != 1) { api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(d); return; }
  if (strcmp(PQgetvalue(d, 0, 1), "t")) { direct_check(fd, db, slug, hash, pre, PQgetvalue(d, 0, 0)); PQclear(d); return; }
  PQclear(d);
  const char *v[2] = {slug, hash};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, x->>'state', x->>'invoice', (x - 'status' - 'invoice')::text FROM makers.tip_state($1, $2) x", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(r); return; }
  if (atoi(PQgetvalue(r, 0, 0)) != 200 || strcmp(PQgetvalue(r, 0, 1), "open")) { api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 3)); PQclear(r); return; }
  char body[2400], *w = NULL;
  snprintf(body, sizeof body, "{\"invoice\":\"%s\"}", PQgetvalue(r, 0, 2));
  PQclear(r);
  long st = api_spark("/paid", body, 30, &w);
  if (st == 200 && w) {
    char state[32], amount[24], opre[80];
    jget(db, w, "state", state, sizeof state); jget(db, w, "amount_msat", amount, sizeof amount); jget(db, w, "preimage", opre, sizeof opre);
    const char *sv[5] = {hash, state, *amount ? amount : NULL, opre, env_or("OA_MAKER_FIRST_TIP_BONUS_SATS", "1000")};
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
  if (!slug_ok(realm, n)) { api_json(q->fd, 404, "{\"error\":\"no such world\"}"); return true; }
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
    if (q->body_len > 512) { api_json(q->fd, 413, "{\"error\":\"too big\"}"); return true; }
    read_wallet(q, db);
    in_child(q->fd, slug, what + 4, asked_pre, tip_check);
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


// ---- listed upgrades (ev-asset-upgrades, migrations/0077_fairy_upgrades.sql, docs/verse/makers.md)
//   GET  /api/playtest/upgrades/fairy             -> 200 {top: the three approved fairies, wearing, pending (an admin's), admin}
//   POST /api/playtest/upgrades/fairy {name, by, look, sats, lightning}   a logged-in maker lists one; it waits for an admin
//   POST /api/playtest/upgrades/<id>/buy {pay: "sats", reports?}   -> 200 {invoice, payment_hash, sats, expires, confirm}: the
//                                                            maker's own invoice (as a tip's). reports: the player's
//                                                            wallet reports the preimage. 422 when neither that wallet
//                                                            nor the maker's service could confirm the payment.
//                                                            {pay: "credit"} -> 400: service credit never buys an upgrade
//   POST /api/playtest/upgrades/<id>/buy/<hash> [{preimage}]   -> 200 {state: open|paid|expired, look, confirm}: yours
//                                                            once confirmed, counted once; 402 a preimage not this invoice's
//   POST /api/playtest/upgrades/<id>/wear          wear what you bought (id 0: the fairy you made)
//   GET  /api/admin/upgrades, POST /api/admin/upgrades/<id>/approve|reject   an admin's
// The buyer is the playtest player (X-Verse-Player): what they wear goes out with the room's presence
// (verse.room_seen), so everyone in the forest draws it. The price goes from the buyer's wallet
// straight to the listing's Lightning Address (owner decision D6, #13210); it never enters ours.
#define UP_PREFIX "/api/playtest/upgrades/"
#define UP_ADMIN "/api/admin/upgrades"

// A new purchase: the terms, then the maker's own invoice. id: the listing; key: the player key;
// flags: "t" or "f" for our team, then for a wallet that reports the preimage.
static void buy_new(int fd, PGconn *db, const char *id, const char *key, const char *flags) {
  const char *v[2] = {id, key};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x - 'status' - 'lightning')::text, x->>'sats', x->>'lightning', "
                                 "(SELECT count(*) FROM makers.tips WHERE created > now() - interval '1 minute') "
                                 "FROM makers.buy_terms($1::bigint, makers.player_of($2)) x", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("upgrades: buy: %s", PQerrorMessage(db)); api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(r); return; }
  int status = atoi(PQgetvalue(r, 0, 0));
  if (status != 200) { api_json(fd, status, PQgetvalue(r, 0, 1)); PQclear(r); return; }
  char sats[24], addr[200];
  snprintf(sats, sizeof sats, "%s", PQgetvalue(r, 0, 2)); snprintf(addr, sizeof addr, "%s", PQgetvalue(r, 0, 3));
  long recent = atol(PQgetvalue(r, 0, 4));
  PQclear(r);
  direct_new(fd, db, id, id, key, addr, sats, flags[0] == 't' ? "true" : "false", flags[1] == 't', recent);
}
// Is a purchase paid? The preimage the player's wallet reported is asked_pre. A purchase whose
// invoice was our wallet's (before D6) is settled from our wallet's record, once, as it always was.
static void buy_check(int fd, PGconn *db, const char *id, const char *hash, const char *key) {
  const char *dv[3] = {hash, id, key};
  PGresult *d = PQexecParams(db, "SELECT x::text, (x->>'found')::boolean AND NOT (x->>'direct')::boolean FROM makers.direct_facts($1, NULL, $2::bigint, makers.player_of($3)) x", 3, NULL, dv, NULL, NULL, 0);
  if (PQresultStatus(d) != PGRES_TUPLES_OK || PQntuples(d) != 1) { api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(d); return; }
  if (strcmp(PQgetvalue(d, 0, 1), "t")) { direct_check(fd, db, id, hash, asked_pre, PQgetvalue(d, 0, 0)); PQclear(d); return; }
  PQclear(d);
  const char *v[3] = {id, hash, key};
  const char *q = "SELECT (x->>'status')::int, x->>'state', x->>'invoice', (x - 'status' - 'invoice')::text FROM makers.buy_state($1::bigint, $2, makers.player_of($3)) x";
  PGresult *r = PQexecParams(db, q, 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_json(fd, 500, "{\"error\":\"not kept\"}"); PQclear(r); return; }
  if (atoi(PQgetvalue(r, 0, 0)) != 200 || strcmp(PQgetvalue(r, 0, 1), "open")) { api_json(fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 3)); PQclear(r); return; }
  char body[2400], *w = NULL;
  snprintf(body, sizeof body, "{\"invoice\":\"%s\"}", PQgetvalue(r, 0, 2));
  PQclear(r);
  long st = api_spark("/paid", body, 30, &w);
  if (st == 200 && w) {
    char state[32], amount[24], pre[80];
    jget(db, w, "state", state, sizeof state); jget(db, w, "amount_msat", amount, sizeof amount); jget(db, w, "preimage", pre, sizeof pre);
    const char *sv[4] = {hash, state, *amount ? amount : NULL, pre};
    PGresult *k = PQexecParams(db, "SELECT x->>'status' FROM makers.buy_settle($1, $2, $3::bigint, $4) x", 4, NULL, sv, NULL, NULL, 0);
    if (PQresultStatus(k) != PGRES_TUPLES_OK) api_log("upgrades: settle: %s", PQerrorMessage(db));
    else if (!strcmp(PQgetvalue(k, 0, 0), "200")) api_log("upgrades: listing %s bought", id);
    PQclear(k);
  }
  free(w);
  sql_answer(fd, db, "SELECT (x->>'status')::int, (x - 'status' - 'invoice')::text FROM makers.buy_state($1::bigint, $2, makers.player_of($3)) x", 3, v);
}
static bool digits_ok(const char *s, size_t n) {
  if (!n || n > 12) return false;
  for (size_t i = 0; i < n; i++) if (s[i] < '0' || s[i] > '9') return false;
  return true;
}

bool upgrades_route(const request *q) {
  bool admin = !strncmp(q->path, UP_ADMIN, sizeof UP_ADMIN - 1) && (q->path[sizeof UP_ADMIN - 1] == 0 || q->path[sizeof UP_ADMIN - 1] == '/');
  if (!admin && strncmp(q->path, UP_PREFIX, sizeof UP_PREFIX - 1)) return false;
  bool post = !strcmp(q->method, "POST");
  char from[256];
  if (post && api_header(q, "Origin", from, sizeof from) && strcmp(from, origin())) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char who[65] = "";
  if (admin) {
    int st = admin_who(q, db, who);
    if (st) { api_json(q->fd, st, st == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}"); return true; }
    const char *rest = q->path + sizeof UP_ADMIN - 1;
    if (!*rest || !strcmp(rest, "/")) {
      if (post) { api_json(q->fd, 405, "{\"error\":\"GET\"}"); return true; }
      PGresult *r = PQexec(db, "SELECT makers.listing_lines()");
      const char *t = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? PQgetvalue(r, 0, 0) : "";
      api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, t, strlen(t));
      PQclear(r);
      return true;
    }
    const char *id = rest + 1, *slash = strchr(id, '/');
    if (!post || !slash || !digits_ok(id, (size_t)(slash - id)) || (strcmp(slash, "/approve") && strcmp(slash, "/reject"))) {
      api_json(q->fd, post ? 404 : 405, post ? "{\"error\":\"no such route\"}" : "{\"error\":\"POST\"}"); return true;
    }
    char ids[16]; snprintf(ids, sizeof ids, "%.*s", (int)(slash - id), id);
    const char *v[3] = {ids, who, !strcmp(slash, "/approve") ? "approved" : "rejected"};
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.listing_decide($1::bigint, $2, $3) x", 3, v);
    api_log("upgrades: listing %s %s", ids, v[2]);
    return true;
  }
  const char *rest = q->path + sizeof UP_PREFIX - 1;
  char key[160] = "";
  api_header(q, "X-Verse-Player", key, sizeof key);
  if (!strcmp(rest, "fairy")) {
    if (!post) {
      bool is_admin = admin_who(q, db, who) == 0;
      const char *v[2] = {key, is_admin ? "true" : "false"};
      answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.upgrades(makers.player_of($1), $2::boolean) x", 2, v);
      return true;
    }
    if (q->body_len > 1024) { api_json(q->fd, 413, "{\"error\":\"too big\"}"); return true; }
    if (!who_asks(q, db, who, true)) return true;
    char *body = body_of(q);
    if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    const char *v[4] = {who, body, env_or("OA_UPGRADE_MIN_SATS", "10"), env_or("OA_UPGRADE_MAX_SATS", "100000")};
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.listing_new($1, $2, $3::bigint, $4::bigint) x", 4, v);
    free(body);
    api_log("upgrades: a fairy listed");
    return true;
  }
  const char *slash = strchr(rest, '/');
  if (!slash || !digits_ok(rest, (size_t)(slash - rest))) { api_json(q->fd, 404, "{\"error\":\"no such listing\"}"); return true; }
  if (!post) { api_json(q->fd, 405, "{\"error\":\"POST\"}"); return true; }
  char id[16]; snprintf(id, sizeof id, "%.*s", (int)(slash - rest), rest);
  const char *what = slash + 1;
  if (!strcmp(what, "wear")) {
    const char *v[2] = {key, id};
    answer(q, db, "SELECT (x->>'status')::int, (x - 'status')::text FROM makers.wear(makers.player_of($1), $2::bigint) x", 2, v);
    return true;
  }
  if (!strncmp(what, "buy/", 4)) {
    if (!hex64(what + 4)) { api_json(q->fd, 404, "{\"error\":\"no such purchase\"}"); return true; }
    if (q->body_len > 512) { api_json(q->fd, 413, "{\"error\":\"too big\"}"); return true; }
    read_wallet(q, db);
    char hash[65]; snprintf(hash, sizeof hash, "%s", what + 4);
    char keyc[160]; snprintf(keyc, sizeof keyc, "%s", key);
    // in_child hands answer_with (fd, db, slug, a, b): the listing, the hash, the player key
    in_child(q->fd, id, hash, keyc, buy_check);
    return true;
  }
  if (strcmp(what, "buy")) { api_json(q->fd, 404, "{\"error\":\"no such route\"}"); return true; }
  if (q->body_len > 512) { api_json(q->fd, 413, "{\"error\":\"too big\"}"); return true; }
  char *body = body_of(q);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  char pay[16];
  jget(db, body, "pay", pay, sizeof pay);
  free(body);
  // who asks, once (a NIP-98 signature is spent when it is read): the team's cookie or an admin is the team
  bool signed_req = false, payload = false, unseen = false;
  bool in = site_identity(q, db, who, &signed_req, &payload, &unseen) && (!signed_req || (payload && unseen));
  char cookies[4096];
  bool team = in && admin_is(who);
  if (!team && api_header(q, "Cookie", cookies, sizeof cookies))
    for (const char *c = strstr(cookies, "oa_team=1"); c && !team; c = strstr(c + 1, "oa_team=1")) team = c == cookies || c[-1] == ' ' || c[-1] == ';';
  int64_t code = mp_pay(cs(pay));   // sats only: service credit never buys an upgrade
  if (code != MP_OK) { mp_refuse(q->fd, code); return true; }
  read_wallet(q, db);
  char flags[3] = {team ? 't' : 'f', asked_reports ? 't' : 'f', 0};
  char keyc[160]; snprintf(keyc, sizeof keyc, "%s", key);
  in_child(q->fd, id, keyc, flags, buy_new);
  return true;
}

// makers.c's numbers against makerpay-policy.coax, at start.
void makers_init(void) {
  bool ok = mp_issue(MP_TIP, true, true, false, false) == MP_OK && mp_issue(MP_UPGRADE, true, true, false, false) != MP_OK
         && mp_issue(MP_TIP, true, false, true, true) == MP_MOVED && mp_quote(23) == MP_BAD && mp_status(MP_FAKE) == 402 && mp_status(MP_NO_TIP) == 404
         && mp_check(true, true, false, MP_BY_VERIFY, true, false) == MP_S_PAID && mp_check(true, true, false, MP_BY_WALLET, false, false) == MP_S_FAKE
         && mp_check(true, true, false, MP_NO_PROOF, true, false) != MP_S_PAID && mp_counts(MP_S_PAID) && !mp_counts(MP_S_FAKE)
         && mp_bonus(MP_TIP, MP_S_PAID, false, 7) == 7 && mp_bonus(MP_UPGRADE, MP_S_PAID, false, 7) == 0
         && mp_payout(MP_SRC_BONUS, true) && mp_payout(MP_SRC_GIFT, true) && !mp_payout(MP_SRC_TIP, true) && !mp_payout(MP_SRC_UPGRADE, true)
         && mp_payout(MP_SRC_TIP, false) && mp_payout(MP_SRC_UPGRADE, false) && mp_pay(cs("credit")) != MP_OK && mp_pay(cs("sats")) == MP_OK;
  if (!ok) { fprintf(stderr, "api: makers.c and makerpay-policy.coax disagree on their numbers\n"); exit(2); }
}

// /docs/limits (api --limits), after the Lightning Address section: what a tip or a purchase is held to.
void makers_print_limits(FILE *out) {
  fprintf(out, "\n## Tips and upgrades\n\nPaid from your wallet straight to the maker's own Lightning Address.\n\n"
               "| What | Limit |\n| --- | --- |\n");
  fprintf(out, "| A tip | %s to %s sats, within what the maker's wallet takes |\n", env_or("OA_TIP_MIN_SATS", "10"), env_or("OA_TIP_MAX_SATS", "100000"));
  fprintf(out, "| An upgrade's price | %s to %s sats |\n", env_or("OA_UPGRADE_MIN_SATS", "10"), env_or("OA_UPGRADE_MAX_SATS", "100000"));
  fprintf(out, "| Invoices asked of makers' wallets, all players | %lld a minute |\n", (long long)mp_per_min());
  fprintf(out, "| The maker's wallet's time to answer | 5 seconds |\n");
  fprintf(out, "\nThis section is made from the server's own numbers (`coax/web/api/makers.c`, `makerpay-policy.coax`).\n");
}
