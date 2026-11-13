// Step-up, linking and the delayed recovery (docs/auth/identity-and-methods.md, decision D10;
// #13212, #13213). What may happen is auth-policy.coax's (levels, limited sessions) and
// auth-recover-policy.coax's (linking, unlinking, the 72-hour recovery); the words people see are
// theirs too. The rows are 0148_auth_recover.sql's; the database refuses a recovery sooner than
// 72 hours and a second open one, whatever this file does.
//
//   GET  /api/auth/ways                  the account's ways in, the session's level, recovery, notices' limits
//   GET  /api/auth/notices               the security notices sent (what, when, how many delivered)
//   POST /api/auth/proof/challenge       {purpose: stepup|link|finish} -> {challenge, url}: sign url with your key
//   POST /api/auth/step-up?c=...         {proof} a NIP-98 event over the url (no payload tag), or {words} the
//                                        recovery code: this session becomes L2 (a code is spent; a new one shows once)
//   POST /api/auth/link/nostr?c=...      {proof, label}: add the key that signed (L2, or email alone on an
//                                        account with nothing stronger)
//   POST /api/auth/unlink                {kind: email|nostr|passkey, id}: L2-fresh, never the last way in;
//                                        sessions it made end; the removed way is told too
//   GET  /api/auth/recovery              the newest request: none, pending, ready, cancelled or done, with times
//   POST /api/auth/recovery/start        email alone, on an account with a stronger way: ready in 72 hours
//   POST /api/auth/recovery/cancel       {t} a notice's cancel link, or any session or signed key of the account
//   POST /api/auth/recovery/finish?c=... {proof, label}: after 72 hours, the key that signed replaces the lost ones
//   POST /api/auth/recovery-code         L2-fresh: a new recovery code, shown once (the old one stops working)
//   POST /api/auth/recovery-code/login   {email | pubkey, words, client}: L2 by the code; it is spent and a
//                                        new one shows once
//
// Every step is in auth.audit, and every change to the ways in sends a notice to each email and
// key of the account (and to the way removed): an email through email.c, and a NIP-17 message (a
// NIP-59 gift wrap, auth-recover-crypto.coax) from our notice key (OA_AUTH_NOTICE_NSEC) through
// OA_AUTH_NOTICE_RELAY (wss://relay.openagents.com). It goes to the account's apps too, as a push
// to each device that gave a target (auth-device.c's auth_device_push, push.c; #13233). Notices go from a child process, so a slow
// mail service or relay never holds up the API; OA_AUTH_NOTICE_SYNC=1 (the checks) sends them inline.
//
// A recovery code is 24 BIP-39 words, stored as scrypt(words, salt, N = 2^17, r = 8, p = 1): 128 MiB
// and most of a second, so each check runs in a child process too (the API answers one request at
// a time). OA_AUTH_SCRYPT_LOG_N (10 to 17) lowers N for the checks only; it is never set in deploys.
#include "auth-internal.h"
#include "auth-recover-policy.generated.h"
#include "auth-recover-crypto.generated.h"
#include "auth-recover-wire.generated.h"
#include "nostr.h"
#include "relay.h"

#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

// ---------------------------------------------------------------- small helpers
// Every body here is auth-recover-wire.coax's: its functions take a string as cs(s) and write into
// OUT(buffer), one byte kept for the NUL; each gives the length written, or -1 when it does not fit.
static coax_string cs(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }
#define OUT(b) ((coax_v_u8){(uint8_t *)(b), (int64_t)sizeof(b) - 1})
// An answer whose two forms the wire unit wrote: jn and ln are their lengths in json and lines.
static void answer_made(const request *q, int status, char *json, int64_t jn, char *lines, int64_t ln) {
  if (jn < 0 || ln < 0) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  json[jn] = 0; lines[ln] = 0;
  auth_answer(q, status, NULL, json, lines);
}
// An audit row whose detail the wire unit wrote: n is its length in d (NULL detail: an empty object).
static void audit_made(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, char *d, int64_t n) {
  if (n >= 0) d[n] = 0;
  auth_audit(db, acct, actor, action, result, n >= 0 ? d : NULL);
}
// One row's first column as text into out ("" when none or NULL); true when there was a row.
static bool one(PGconn *db, const char *sql, int n, const char *const *v, char *out, size_t cap) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) >= 1;
  if (out && cap) { out[0] = 0; if (ok && !PQgetisnull(r, 0, 0)) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0)); }
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth: %.60s: %s", sql, PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static int one_int(PGconn *db, const char *sql, int n, const char *const *v) {
  char b[32]; one(db, sql, n, v, b, sizeof b); return atoi(b);
}
static void words_of(coax_string w, char *out, size_t cap) { snprintf(out, cap, "%.*s", (int)w.count, w.data); }
static void refuse_ar(const request *q, int verdict) {
  char w[200]; words_of(ar_words(verdict), w, sizeof w);
  auth_refuse(q, (int)ar_status(verdict), w);
}
static void utc(long long ms, char *out, size_t cap) {
  time_t t = (time_t)(ms / 1000); struct tm tm; gmtime_r(&t, &tm);
  strftime(out, cap, "%Y-%m-%d %H:%M UTC", &tm);
}
static bool same_bytes(const unsigned char *a, const unsigned char *b, size_t n) {
  unsigned char d = 0;
  for (size_t i = 0; i < n; i++) d |= (unsigned char)(a[i] ^ b[i]);
  return d == 0;
}
static int scrypt_log_n(void) {
  int n = atoi(env_or("OA_AUTH_SCRYPT_LOG_N", "17"));
  return n >= 10 && n <= 17 ? n : 17;
}
// A child process answers the request (fork); the parent goes on at once. false in the parent.
static bool in_child(void) {
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  return pid == 0;
}
static char *body_of(const request *q) {
  char *b = malloc(q->body_len + 1);
  if (!b) return NULL;
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  return b;
}

// The AR_* numbers auth-internal.h shares with auth-recover-policy.coax.
bool auth_recover_init(void) {
  return ar_state(10, 0, 0, 0, 0) == AR_NONE && ar_state(1, 1, 1, 0, 0) == AR_PENDING && ar_state(1LL << 40, 1, 1, 0, 0) == AR_READY
      && ar_state(1, 1, 1, 1, 0) == AR_CANCELLED && ar_state(1, 1, 1, 0, 1) == AR_DONE
      && ar_link(2, 1, AR_OWNER_NONE) == AR_OK && ar_link(2, 1, AR_OWNER_THIS) == AR_ALREADY && ar_link(2, 1, AR_OWNER_OTHER) == AR_OTHER
      && ar_link(1, 1, AR_OWNER_NONE) == AR_STEP_UP && ar_unlink(0) == AR_LAST_WAY && ar_start(2, 1, AR_NONE) == AR_NOT_NEEDED
      && ar_start(1, 0, AR_NONE) == AR_NOTHING && ar_start(1, 1, AR_PENDING) == AR_ASKED && ar_finish(1, true, AR_PENDING) == AR_WAITING
      && ar_finish(1, true, AR_NONE) == AR_NO_REQUEST && ar_finish(1, false, AR_READY) == AR_NOT_EMAIL && ar_start(0, 1, AR_NONE) == AR_NOT_EMAIL;
}

// ---------------------------------------------------------------- recovery's state
typedef struct { long long id, asked, ready, cancelled, done, now; int state; char via[16]; } recovery_state;
static recovery_state recovery_of(PGconn *db, const char *acct) {
  recovery_state s; memset(&s, 0, sizeof s);
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT id, asked_ms, ready_ms, cancelled_ms, done_ms, now_ms, coalesce(cancelled_via, '') FROM auth.recovery_of($1)", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    s.id = atoll(PQgetvalue(r, 0, 0)); s.asked = atoll(PQgetvalue(r, 0, 1)); s.ready = atoll(PQgetvalue(r, 0, 2));
    s.cancelled = atoll(PQgetvalue(r, 0, 3)); s.done = atoll(PQgetvalue(r, 0, 4)); s.now = atoll(PQgetvalue(r, 0, 5));
    snprintf(s.via, sizeof s.via, "%s", PQgetvalue(r, 0, 6));
  }
  PQclear(r);
  s.state = (int)ar_state(s.now ? s.now : (long long)api_now_ms(), s.asked, s.ready, s.cancelled, s.done);
  return s;
}
static const char *state_name(int st) {
  return st == AR_PENDING ? "pending" : st == AR_READY ? "ready" : st == AR_CANCELLED ? "cancelled" : st == AR_DONE ? "done" : "none";
}
// The recovery as JSON in out (400 bytes): its length, and "" in out when it did not fit.
static int64_t recovery_json(const recovery_state *s, char out[400]) {
  int64_t n = arw_recovery_write(cs(state_name(s->state)), s->asked, s->ready, s->cancelled, s->done, s->now, cs(s->via), (coax_v_u8){(uint8_t *)out, 399});
  out[n < 0 ? 0 : n] = 0;
  return n;
}
// The account's methods as JSON in out (200 bytes); acct "": a login with no account, by its key.
static void methods_json(PGconn *db, const char *acct, char out[200]) {
  const char *v[1] = {acct};
  coax_v_u8 o = {(uint8_t *)out, 199};
  int64_t n = -1;
  if (!acct[0]) n = arw_methods_key_write(o);
  else if (!one(db, "SELECT to_json(auth.methods_of($1))::text", 1, v, out, 200) || !out[0]) n = arw_methods_none_write(o);
  if (n >= 0) out[n] = 0;
}

// ---------------------------------------------------------------- notices
// Sends one notice to every email, key and app device of the account, and to also_email /
// also_pubkey (a way just removed). Every delivery is counted on the notice row and in the audit log.
// A notice's cancel link: the line above it and where it goes (the recovery's, a limit raise's).
typedef struct { const char *lead, *url; } notice_link;
static void deliver(PGconn *db, long long notice_id, const char *acct, const char *what, const char *subject, const char *words,
                    const notice_link *cancel, const char *also_email, const char *also_pubkey) {
  char freeze_tok[65] = "", freeze_url[300] = "", cancel_url[300] = "";
  if (auth_freeze_link(db, acct, freeze_tok)) snprintf(freeze_url, sizeof freeze_url, "%s/settings/security#freeze=%s", auth_origin(), freeze_tok);
  if (cancel && cancel->url && *cancel->url) snprintf(cancel_url, sizeof cancel_url, "%s", cancel->url);
  char text[1400];
  snprintf(text, sizeof text, "%s\n\n%s%s%s%s%s%s%sThis is a security notice from OpenAgents. We send one for every change to how you log in or to your wallet's limit and backup.\n",
           words,
           cancel_url[0] ? cancel->lead : "", cancel_url[0] ? "\n" : "", cancel_url, cancel_url[0] ? "\n\n" : "",
           freeze_url[0] ? "If this wasn't you, freeze your account (every login ends):\n" : "", freeze_url, freeze_url[0] ? "\n\n" : "");
  int emailed = 0, nostr_sent = 0, failed = 0;
  // ---- email: each address, opened from its seal
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT sealed FROM auth.emails WHERE account = $1 AND sealed IS NOT NULL ORDER BY id", 1, NULL, v, NULL, NULL, 0);
  int ne = PQresultStatus(r) == PGRES_TUPLES_OK ? PQntuples(r) : 0;
  char ad[96]; snprintf(ad, sizeof ad, "auth.emails|sealed|%s", acct);
  for (int i = 0; i <= ne; i++) {
    char email[300] = "";
    if (i < ne) { if (!auth_unseal(ad, PQgetvalue(r, i, 0), email, sizeof email)) { failed++; continue; } }
    else if (also_email && *also_email) snprintf(email, sizeof email, "%s", also_email);
    if (!email[0]) continue;
    char message[3600], id[128], err[300];
    // The key names the account too: staging and production share one mail account, and their
    // notice ids both start at 1 (a reused key with other words is refused, 409).
    int64_t mn = arw_mail_write(cs(email), cs(subject), cs(acct), notice_id, i, cs(text), OUT(message));
    if (mn < 0) { failed++; memset(email, 0, sizeof email); memset(message, 0, sizeof message); continue; }
    message[mn] = 0;
    int s = email_send(db, message, id, err, sizeof err);
    if (s > 1) { failed++; api_log("auth: notice %lld email: %s", notice_id, err); } else emailed++;
    memset(email, 0, sizeof email); memset(message, 0, sizeof message);
  }
  PQclear(r);
  // ---- Nostr: a NIP-17 message to each key, from our notice key
  r = PQexecParams(db, "SELECT pubkey FROM auth.nostr_keys WHERE account = $1 ORDER BY id", 1, NULL, v, NULL, NULL, 0);
  int nk = PQresultStatus(r) == PGRES_TUPLES_OK ? PQntuples(r) : 0;
  unsigned char sk[32];
  bool keyed = auth_unhex(env_or("OA_AUTH_NOTICE_NSEC", ""), sk, 32);
  relay *rl = NULL;
  unsigned char *work = NULL; char *wrap = NULL;
  if (keyed && (nk > 0 || (also_pubkey && *also_pubkey))) {
    char err[200];
    rl = relay_open(env_or("OA_AUTH_NOTICE_RELAY", "wss://relay.openagents.com"), sk, err, sizeof err);
    if (!rl) api_log("auth: notice %lld relay: %s", notice_id, err);
    work = malloc(458752); wrap = malloc(16384);
  }
  // also_pubkey: more keys, comma apart (the ones a recovery is about to remove)
  int na = also_pubkey && *also_pubkey ? (int)(strlen(also_pubkey) + 1) / 65 : 0;
  for (int i = 0; i < nk + na; i++) {
    char pk[65];
    if (i < nk) snprintf(pk, sizeof pk, "%s", PQgetvalue(r, i, 0));
    else snprintf(pk, sizeof pk, "%.64s", also_pubkey + (i - nk) * 65);
    if (strlen(pk) != 64) continue;
    unsigned char x[32], fresh[176];
    if (!rl || !work || !wrap || !auth_unhex(pk, x, 32) || !auth_random(fresh, sizeof fresh)) { failed++; continue; }
    int64_t n = arc_wrap((coax_v_u8){sk, 32}, (coax_v_u8){x, 32}, (int64_t)time(NULL), (coax_v_u8){(uint8_t *)text, (int64_t)strlen(text)},
                         (coax_v_u8){fresh, 176}, (coax_v_u8){work, 458752}, (coax_v_u8){(uint8_t *)wrap, 16383});
    memset(fresh, 0, sizeof fresh);
    if (n <= 0) { failed++; continue; }
    wrap[n] = 0;
    char id[65] = "", err[200] = "";
    if (arw_event_id((coax_string){wrap, n}, (coax_v_u8){(uint8_t *)id, 64}) == 64 && relay_send(rl, wrap, id, err, sizeof err)) nostr_sent++;
    else { failed++; api_log("auth: notice %lld nostr: %s", notice_id, err); }
  }
  PQclear(r);
  if (rl) relay_close(rl);
  free(work); free(wrap);
  memset(sk, 0, sizeof sk);
  // ---- push: each device of the account that still holds a login (auth-device.c). The words
  // alone: the app opens its Security page, where its own login cancels or freezes, so no link.
  char title[200]; snprintf(title, sizeof title, "OpenAgents security: %s", subject);
  int pushed = auth_device_push(db, acct, notice_id, what, title, words, &failed);
  char sent[160], idb[24];
  int64_t sn = arw_sent_write(emailed, nostr_sent, pushed, failed, OUT(sent));
  sent[sn < 0 ? 0 : sn] = 0;   // four numbers: it fits
  snprintf(idb, sizeof idb, "%lld", notice_id);
  const char *w[2] = {idb, sent};
  PQclear(PQexecParams(db, "SELECT auth.notice_sent($1::bigint, $2::jsonb)", 2, NULL, w, NULL, NULL, 0));
  char detail[240];
  audit_made(db, acct, NULL, "notice", failed && !emailed && !nostr_sent && !pushed ? "failed" : "ok", detail,
             arw_notice_detail_write(cs(what), emailed, nostr_sent, pushed, failed, OUT(detail)));
}
// The notice's row now, its delivery from a child (inline under OA_AUTH_NOTICE_SYNC=1).
static void notify(PGconn *db, const char *acct, const char *what, const char *subject, const char *words,
                   const notice_link *cancel, const char *also_email, const char *also_pubkey) {
  const char *v[3] = {acct, what, words};
  char idb[24];
  if (!one(db, "SELECT auth.notice_add($1, $2, $3)", 3, v, idb, sizeof idb) || !idb[0]) return;
  long long id = atoll(idb);
  if (!strcmp(env_or("OA_AUTH_NOTICE_SYNC", "0"), "1")) { deliver(db, id, acct, what, subject, words, cancel, also_email, also_pubkey); return; }
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid != 0) return;   // the parent, or no child: the row says nothing was sent
  PGconn *c = api_db_new();
  if (c) { deliver(c, id, acct, what, subject, words, cancel, also_email, also_pubkey); PQfinish(c); }
  _exit(0);
}
static void short_key(const char *pk, char *out, size_t cap) { snprintf(out, cap, "%.8s…%.4s", pk, pk + 60); }

// ---------------------------------------------------------------- the gate's step-up answer
void auth_step_up_refusal(const request *q, PGconn *db, const auth_principal *p, int need, int status, const char *words) {
  char methods[200], rec[400], json[1400], lines[800];
  // An admin page's old key-made login has no account: its key is its one way in.
  recovery_state s = {0};
  methods_json(db, p->account, methods);
  if (p->account[0]) s = recovery_of(db, p->account);
  recovery_json(&s, rec);
  const char *lvl = need == 3 ? "L2-fresh" : "L2";
  // in the lines, methods are one line: nostr,recovery_code
  answer_made(q, status, json, arw_step_up_write(cs(words), cs(lvl), cs(methods), p->limited, p->level, cs(rec), OUT(json)),
              lines, arw_step_up_lines(cs(words), cs(lvl), cs(methods), p->limited, cs(state_name(s.state)), s.ready, OUT(lines)));
}

// ---------------------------------------------------------------- proofs made while logged in
static const char *proof_route(const char *purpose) {
  return !strcmp(purpose, "stepup") ? "/api/auth/step-up" : !strcmp(purpose, "link") ? "/api/auth/link/nostr"
       : !strcmp(purpose, "finish") ? "/api/auth/recovery/finish" : !strcmp(purpose, "merge") ? "/api/auth/merge/prove" : NULL;
}
static void proof_challenge(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char purpose[16] = "";
  auth_json_str(body, strlen(body), "purpose", purpose, sizeof purpose);
  const char *route = proof_route(purpose);
  if (!route) { auth_refuse(q, 400, "Say what the signature is for: stepup, link, finish or merge."); return; }
  if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return; }
  char id[33], ch[65], chsha[65];
  if (!auth_random_hex(id, 16) || !auth_random_hex(ch, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  auth_sha_hex(ch, chsha);
  const char *v[4] = {id, chsha, p->account, purpose};
  if (!one(db, "SELECT auth.proof_challenge($1, $2, $3, $4)", 4, v, NULL, 0)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  char json[600], lines[500];
  answer_made(q, 200, json, arw_challenge_write(cs(ch), cs(auth_origin()), cs(route), OUT(json)),
              lines, arw_challenge_lines(cs(ch), cs(auth_origin()), cs(route), OUT(lines)));
}
// The key that signed this request's proof: a NIP-98 event (kind 27235, within 60 seconds) over
// this route with ?c=<challenge>, method POST, no payload tag, sent as {"proof": base64}. The
// challenge must be this account's, for this purpose, unused. false: it answered the refusal.
static bool proof_key(const request *q, PGconn *db, const auth_principal *p, const char *purpose, const char *body, char pk[65]) {
  char c[80], proof[4000], csha[65];
  if (!api_query(q, "c", c, sizeof c) || strlen(c) != 64) { auth_refuse(q, 400, "Ask for a challenge first."); return false; }
  if (!auth_json_str(body, strlen(body), "proof", proof, sizeof proof) || !proof[0]) { auth_refuse(q, 400, "Sign the challenge with your key."); return false; }
  char authz[4100], url[400], err[160], d[64];
  snprintf(authz, sizeof authz, "Nostr %s", proof);
  snprintf(url, sizeof url, "%s%s?c=%s", auth_origin(), q->path, c);
  nostr_event ev;
  if (!nip98_check(authz, "POST", url, NULL, 0, (long long)time(NULL), &ev, err, sizeof err)) {
    audit_made(db, p->account, p->session, purpose, "refused", d, arw_why_write(cs("bad signature"), OUT(d)));
    auth_refuse(q, 401, "That signature isn't good for this challenge. Sign it again.");
    return false;
  }
  auth_sha_hex(c, csha);
  const char *v[4] = {csha, p->account, purpose, ev.id};
  char state[16];
  one(db, "SELECT auth.proof_spend($1, $2, $3, $4)", 4, v, state, sizeof state);
  if (strcmp(state, "ok")) {
    audit_made(db, p->account, p->session, purpose, "refused", d, arw_why_write(cs(!strcmp(state, "replay") ? "replay" : "no challenge"), OUT(d)));
    auth_refuse(q, 401, !strcmp(state, "replay") ? "That signature was used already." : "That challenge has ended. Ask for a new one.");
    return false;
  }
  memcpy(pk, ev.pubkey, 65);
  return true;
}
// For auth-life.c: the merge's proof by the other account's key (purpose "merge").
bool auth_proof_key(const request *q, PGconn *db, const auth_principal *p, const char *purpose, const char *body, char pk[65]) {
  return proof_key(q, db, p, purpose, body, pk);
}
// For auth-life.c: a notice of freezing, deletion or merging to every email and key of acct.
void auth_notice(PGconn *db, const char *acct, const char *what, const char *subject, const char *words) {
  notify(db, acct, what, subject, words, NULL, NULL, NULL);
}
// For wallet.c: the same notice with a link under it (lead: the line above the link).
void auth_notice_link(PGconn *db, const char *acct, const char *what, const char *subject, const char *words, const char *lead, const char *url) {
  notice_link link = {lead, url};
  notify(db, acct, what, subject, words, &link, NULL, NULL);
}
static int key_owner(PGconn *db, const char *pk, const char *acct) {
  const char *v[2] = {pk, acct};
  return one_int(db, "SELECT auth.key_owner($1, $2)", 2, v);
}

// ---------------------------------------------------------------- recovery codes
typedef struct { char words[220]; char id[24]; } new_code;
// A new code for acct (the old one is spent), its words in out. Runs scrypt: call from a child.
static bool code_make(PGconn *db, const char *acct, new_code *out) {
  unsigned char e[32], salt[16], hash[32], wk[1520];
  if (!auth_random(e, 32) || !auth_random(salt, 16)) return false;
  int64_t n = arc_words((coax_v_u8){e, 32}, (coax_v_u8){wk, sizeof wk}, (coax_v_u8){(uint8_t *)out->words, (int64_t)sizeof out->words - 1});
  memset(e, 0, sizeof e);
  if (n <= 0) return false;
  out->words[n] = 0;
  int log_n = scrypt_log_n();
  size_t mem = (size_t)1024 << log_n;
  unsigned char *m = malloc(mem);
  if (!m) return false;
  bool ok = arc_hash((coax_v_u8){(uint8_t *)out->words, n}, (coax_v_u8){salt, 16}, (coax_v_u8){m, (int64_t)mem}, log_n, (coax_v_u8){hash, 32});
  free(m);
  if (!ok) return false;
  char hh[65], sh[33], hx[80], sx[48];
  auth_hex(hash, 32, hh); auth_hex(salt, 16, sh);
  snprintf(hx, sizeof hx, "\\x%s", hh); snprintf(sx, sizeof sx, "\\x%s", sh);
  const char *v[3] = {acct, hx, sx};
  return one(db, "SELECT auth.code_new($1, $2::bytea, $3::bytea)", 3, v, out->id, sizeof out->id) && out->id[0];
}
// The unspent code these words are (its row id in id), or false. Runs scrypt: call from a child.
static bool code_matches(PGconn *db, const char *acct, const char *typed, char id[24]) {
  char words[220]; unsigned char wk[1520];
  int64_t n = arc_normal((coax_v_u8){(uint8_t *)typed, (int64_t)strlen(typed)}, (coax_v_u8){wk, sizeof wk}, (coax_v_u8){(uint8_t *)words, (int64_t)sizeof words - 1});
  if (n <= 0) return false;
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT id, encode(code_scrypt, 'hex'), encode(salt, 'hex') FROM auth.recovery WHERE account = $1 AND spent_at IS NULL ORDER BY id DESC LIMIT 2",
                             1, NULL, v, NULL, NULL, 0);
  bool found = false;
  int log_n = scrypt_log_n();
  size_t mem = (size_t)1024 << log_n;
  unsigned char *m = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) ? malloc(mem) : NULL;
  for (int i = 0; m && i < PQntuples(r) && !found; i++) {
    unsigned char want[32], salt[16], got[32];
    if (!auth_unhex(PQgetvalue(r, i, 1), want, 32) || !auth_unhex(PQgetvalue(r, i, 2), salt, 16)) continue;
    if (arc_hash((coax_v_u8){(uint8_t *)words, n}, (coax_v_u8){salt, 16}, (coax_v_u8){m, (int64_t)mem}, log_n, (coax_v_u8){got, 32}) && same_bytes(got, want, 32)) {
      snprintf(id, 24, "%s", PQgetvalue(r, i, 0));
      found = true;
    }
  }
  free(m);
  PQclear(r);
  memset(words, 0, sizeof words);
  return found;
}

// ---------------------------------------------------------------- routes
static void ways(const request *q, PGconn *db, const auth_principal *p) {
  enum { JSON_CAP = 24000, LINES_CAP = 8000 };
  const char *v[1] = {p->account};
  char *json = malloc(JSON_CAP), *lines = malloc(LINES_CAP);
  if (!json || !lines) { free(json); free(lines); auth_refuse(q, 500, "Out of memory."); return; }
  // Each piece goes on where the last ended (j in json, l in lines); -1 once one did not fit.
  coax_v_u8 J = {(uint8_t *)json, JSON_CAP - 1}, L = {(uint8_t *)lines, LINES_CAP - 1};
  int64_t j = arw_ways_head_write(cs(p->account), p->level, cs(p->via), p->l2_age_ms, p->limited, J);
  int64_t l = arw_ways_head_lines(cs(p->account), p->level, cs(p->via), p->l2_age_ms, p->limited, L);
  PGresult *r = PQexecParams(db, "SELECT id, coalesce(sealed, ''), (extract(epoch FROM created_at) * 1000)::bigint FROM auth.emails WHERE account = $1 ORDER BY id", 1, NULL, v, NULL, NULL, 0);
  char ad[96]; snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r); i++) {
    char email[300], masked[320] = "•••";
    if (auth_unseal(ad, PQgetvalue(r, i, 1), email, sizeof email)) { auth_mask_email(email, masked, sizeof masked); memset(email, 0, sizeof email); }
    j = arw_ways_email_write(i == 0, cs(PQgetvalue(r, i, 0)), cs(masked), cs(PQgetvalue(r, i, 2)), J, j);
    l = arw_ways_email_line(cs(PQgetvalue(r, i, 0)), cs(masked), L, l);
  }
  PQclear(r);
  j = arw_ways_keys_write(J, j);
  r = PQexecParams(db, "SELECT id, pubkey, coalesce(label, ''), (extract(epoch FROM created_at) * 1000)::bigint FROM auth.nostr_keys WHERE account = $1 ORDER BY id", 1, NULL, v, NULL, NULL, 0);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r) && j + 300 < JSON_CAP; i++) {
    j = arw_ways_key_write(i == 0, cs(PQgetvalue(r, i, 0)), cs(PQgetvalue(r, i, 1)), cs(PQgetvalue(r, i, 2)), cs(PQgetvalue(r, i, 3)), J, j);
    if (l + 100 < LINES_CAP) l = arw_ways_key_line(cs(PQgetvalue(r, i, 0)), cs(PQgetvalue(r, i, 1)), L, l);
  }
  PQclear(r);
  j = arw_ways_passkeys_write(J, j);
  if (p->account[0] && j >= 0 && l >= 0) {   // passkeys.c writes its rows on the end of both
    size_t pj = (size_t)j, pl = (size_t)l;
    passkeys_rows(db, p->account, json, JSON_CAP, &pj, lines, LINES_CAP, &pl);
    j = (int64_t)pj; l = (int64_t)pl;
  }
  char methods[200], rec[400], extra[200];
  // An admin page's old key-made login has no account: its key is its one way in.
  recovery_state s = {0};
  methods_json(db, p->account, methods);
  if (p->account[0]) s = recovery_of(db, p->account);
  recovery_json(&s, rec);
  one(db, "SELECT json_build_object('passkeys', (SELECT count(*) FROM auth.passkeys WHERE account = $1), 'recovery_code', "
          "EXISTS (SELECT 1 FROM auth.recovery WHERE account = $1 AND spent_at IS NULL), 'ways', auth.login_ways($1), 'strong', auth.strong_ways($1))::text",
      1, v, extra, sizeof extra);
  j = arw_ways_tail_write(cs(methods), cs(rec), cs(extra), J, j);
  char codes[8] = "0", passkeys[16] = "0";
  one(db, "SELECT CASE WHEN EXISTS (SELECT 1 FROM auth.recovery WHERE account = $1 AND spent_at IS NULL) THEN 1 ELSE 0 END", 1, v, codes, sizeof codes);
  one(db, "SELECT count(*) FROM auth.passkeys WHERE account = $1", 1, v, passkeys, sizeof passkeys);
  l = arw_ways_tail_lines(cs(methods), cs(codes), cs(passkeys), cs(state_name(s.state)), s.asked, s.ready, s.now, L, l);
  answer_made(q, 200, json, j, lines, l);
  free(json); free(lines);
}
static void notices(const request *q, PGconn *db, const auth_principal *p) {
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT auth.notices_of($1)::text", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  size_t n = (size_t)PQgetlength(r, 0, 0) + 32;
  char *json = malloc(n);
  int64_t jn = json ? arw_notices_write(cs(PQgetvalue(r, 0, 0)), (coax_v_u8){(uint8_t *)json, (int64_t)n - 1}) : -1;
  PQclear(r);
  // lines: "notice AT_MS WHAT", newest first (the page shows when and what; the words went in the notice)
  char lines[3000]; int64_t l = 0;
  r = PQexecParams(db, "SELECT (extract(epoch FROM at) * 1000)::bigint, what FROM auth.notices WHERE account = $1 ORDER BY id DESC LIMIT 30", 1, NULL, v, NULL, NULL, 0);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r) && l + 80 < (int64_t)sizeof lines; i++)
    l = arw_notice_line(cs(PQgetvalue(r, i, 0)), cs(PQgetvalue(r, i, 1)), OUT(lines), l);
  PQclear(r);
  if (json) { answer_made(q, 200, json, jn, lines, l); free(json); } else auth_refuse(q, 500, "Out of memory.");
}

// The session to L2 now (key or code), and what follows: a pending recovery ends.
static bool raise_session(PGconn *db, const auth_principal *p, const char *kind, const char *via) {
  const char *v[3] = {p->session, kind, via};
  char ok[8];
  one(db, "SELECT auth.session_raise($1, $2, $3::bigint)", 3, v, ok, sizeof ok);
  if (strcmp(ok, "t")) return false;
  auth_recover_strong_login(db, p->account, p->session);
  return true;
}

static void step_up(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  if (!p->session[0]) { auth_refuse(q, 400, "A signed request is already L2. Step up a login session."); return; }
  char typed[600] = "";
  auth_json_str(body, strlen(body), "words", typed, sizeof typed);
  if (!typed[0]) {
    char pk[65];
    if (!proof_key(q, db, p, "stepup", body, pk)) return;
    const char *v[2] = {pk, p->account};
    char kid[24], d[64], json[64], lines[64];
    one(db, "SELECT id FROM auth.nostr_keys WHERE pubkey = $1 AND account = $2", 2, v, kid, sizeof kid);
    if (!kid[0]) {
      audit_made(db, p->account, p->session, "stepup", "refused", d, arw_via_write(cs("nostr"), cs("not this account's key"), OUT(d)));
      auth_refuse(q, 403, "That key isn't on this account. Sign with a key you added, or use your recovery code.");
      return;
    }
    if (!raise_session(db, p, "nostr", kid)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    audit_made(db, p->account, p->session, "stepup", "ok", d, arw_via_write(cs("nostr"), cs(""), OUT(d)));
    answer_made(q, 200, json, arw_stepped_write(cs(""), OUT(json)), lines, arw_stepped_lines(cs(""), OUT(lines)));
    return;
  }
  // The recovery code: limits first, then scrypt in a child that answers.
  char ipk[65]; auth_visitor_key(q, ipk);
  if (!auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY_CODE_IP, ipk) || !auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY_CODE, p->account)) return;
  if (!in_child()) return;
  PGconn *c = api_db_new();
  if (!c) { auth_refuse(q, 503, "The database isn't answering. Try again."); _exit(0); }
  char id[24], d[64]; new_code fresh;
  if (!code_matches(c, p->account, typed, id)) {
    audit_made(c, p->account, p->session, "stepup", "refused", d, arw_via_write(cs("recovery_code"), cs("wrong code"), OUT(d)));
    auth_refuse(q, 401, "That recovery code isn't right. Check the 24 words and their order.");
    PQfinish(c); _exit(0);
  }
  PQclear(PQexec(c, "BEGIN"));
  const char *v[1] = {id};
  char spent[8]; one(c, "SELECT auth.code_spend($1::bigint)", 1, v, spent, sizeof spent);
  bool made = !strcmp(spent, "t") && code_make(c, p->account, &fresh);
  if (!made) { PQclear(PQexec(c, "ROLLBACK")); auth_refuse(q, 500, "That didn't work. Try again."); PQfinish(c); _exit(0); }
  PQclear(PQexec(c, "COMMIT"));
  raise_session(c, p, "recovery", id);
  audit_made(c, p->account, p->session, "stepup", "ok", d, arw_via_write(cs("recovery_code"), cs(""), OUT(d)));
  char json[600], lines[400];
  answer_made(q, 200, json, arw_stepped_write(cs(fresh.words), OUT(json)), lines, arw_stepped_lines(cs(fresh.words), OUT(lines)));
  memset(&fresh, 0, sizeof fresh); memset(json, 0, sizeof json); memset(lines, 0, sizeof lines);
  notify(c, p->account, "code.used", "your recovery code was used",
         "Your OpenAgents recovery code was used to confirm a login. It no longer works; a new code was shown once to whoever used it.", NULL, NULL, NULL);
  PQfinish(c);
  _exit(0);
}

// The email link's check (auth.c's email/start with link): the same rule as a key's.
bool auth_recover_may_link(const request *q, PGconn *db, const auth_principal *p) {
  (void)db;
  int verdict = (int)ar_link(p->level, p->strong, AR_OWNER_NONE);
  if (verdict == AR_OK) return true;
  refuse_ar(q, verdict);
  return false;
}
void auth_recover_linked_email(PGconn *db, const char *acct, const char *sid) {
  (void)sid;
  notify(db, acct, "link.email", "an email address was added",
         "An email address was added to your OpenAgents account. You can now log in with it.", NULL, NULL, NULL);
}

static void link_nostr(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char pk[65];
  if (!proof_key(q, db, p, "link", body, pk)) return;
  int owner = key_owner(db, pk, p->account);
  int verdict = (int)ar_link(p->level, p->strong, owner == 1 ? AR_OWNER_THIS : owner == 2 ? AR_OWNER_OTHER : AR_OWNER_NONE);
  if (verdict != AR_OK) {
    char d[120];
    audit_made(db, p->account, p->session, "link", "refused", d,
               arw_kind_write(cs("nostr"), cs(verdict == AR_OTHER ? "another account's" : verdict == AR_ALREADY ? "already" : "step up"), OUT(d)));
    refuse_ar(q, verdict);
    return;
  }
  char label[40] = "";
  auth_json_str(body, strlen(body), "label", label, sizeof label);
  if (strcmp(label, "extension") && strcmp(label, "bunker") && strcmp(label, "phone") && strcmp(label, "app")) label[0] = 0;
  const char *v[3] = {p->account, pk, label};
  char kid[24];
  if (!one(db, "SELECT auth.key_link($1, $2, $3)", 3, v, kid, sizeof kid) || !kid[0]) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  char d[64];
  audit_made(db, p->account, p->session, "link", "ok", d, arw_kind_write(cs("nostr"), cs(""), OUT(d)));
  char sk[40], words[300];
  short_key(pk, sk, sizeof sk);
  snprintf(words, sizeof words, "A Nostr key (%s) was added to your OpenAgents account. It can now log in and change the account.", sk);
  notify(db, p->account, "link.nostr", "a Nostr key was added", words, NULL, NULL, NULL);
  char json[200], lines[200];
  answer_made(q, 200, json, arw_linked_write(cs(kid), cs(pk), OUT(json)), lines, arw_linked_lines(cs(kid), cs(pk), OUT(lines)));
}

static void unlink_way(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char kind[16] = "", idtxt[24] = "";
  auth_json_str(body, strlen(body), "kind", kind, sizeof kind);
  // id: a number, as a JSON number or string
  idtxt[arw_unlink_id(cs(body), OUT(idtxt))] = 0;
  if ((strcmp(kind, "email") && strcmp(kind, "nostr") && strcmp(kind, "passkey")) || !idtxt[0]) { auth_refuse(q, 400, "Say which way in to remove: kind and id."); return; }
  const char *v[1] = {p->account};
  int verdict = (int)ar_unlink(one_int(db, "SELECT auth.login_ways($1)", 1, v) - 1);
  char d[120];
  if (verdict != AR_OK) { audit_made(db, p->account, p->session, "unlink", "refused", d, arw_why_write(cs("last way in"), OUT(d))); refuse_ar(q, verdict); return; }
  // Who to tell besides the account's ways that stay: the way itself.
  char also_email[300] = "", also_pk[65] = "", label[120] = "";
  const char *w[2] = {idtxt, p->account};
  if (!strcmp(kind, "email")) {
    char sealed[1200], ad[96];
    snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
    if (one(db, "SELECT sealed FROM auth.emails WHERE id = $1::bigint AND account = $2", 2, w, sealed, sizeof sealed) && auth_unseal(ad, sealed, also_email, sizeof also_email)) {
      char m[320]; auth_mask_email(also_email, m, sizeof m); snprintf(label, sizeof label, "the email address %s", m);
    } else snprintf(label, sizeof label, "an email address");
  } else if (!strcmp(kind, "nostr")) {
    one(db, "SELECT pubkey FROM auth.nostr_keys WHERE id = $1::bigint AND account = $2", 2, w, also_pk, sizeof also_pk);
    char sk[40]; short_key(also_pk[0] ? also_pk : "????????????????????????????????????????????????????????????????", sk, sizeof sk);
    snprintf(label, sizeof label, "the Nostr key %s", sk);
  } else snprintf(label, sizeof label, "a passkey");
  const char *u[3] = {p->account, kind, idtxt};
  PGresult *r = PQexecParams(db, "SELECT auth.unlink($1, $2, $3::bigint)", 3, NULL, u, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  int ended = ok ? atoi(PQgetvalue(r, 0, 0)) : 0;
  bool last = PQresultStatus(r) != PGRES_TUPLES_OK && strstr(PQerrorMessage(db), "Add another way");
  PQclear(r);
  if (!ok) {
    memset(also_email, 0, sizeof also_email);
    if (last) { refuse_ar(q, AR_LAST_WAY); return; }
    auth_refuse(q, 404, "That isn't on this account.");
    return;
  }
  audit_made(db, p->account, p->session, "unlink", "ok", d, arw_unlinked_write(cs(kind), ended, OUT(d)));
  char words[400];
  snprintf(words, sizeof words, "%c%s was removed from your OpenAgents account. It can't log in any more, and its logins ended.", label[0] == 't' ? 'T' : 'A', label + 1);
  notify(db, p->account, "unlink", "a way to log in was removed", words, NULL, also_email, also_pk[0] ? also_pk : NULL);
  memset(also_email, 0, sizeof also_email);
  char json[120], lines[80];
  answer_made(q, 200, json, arw_removed_write(ended, OUT(json)), lines, arw_removed_lines(ended, OUT(lines)));
}

static void recovery_get(const request *q, PGconn *db, const auth_principal *p) {
  recovery_state s = recovery_of(db, p->account);
  char rec[400], lines[300];
  answer_made(q, 200, rec, recovery_json(&s, rec), lines, arw_recovery_lines(cs(state_name(s.state)), s.asked, s.ready, s.now, OUT(lines)));
}

static void recovery_start(const request *q, PGconn *db, const auth_principal *p) {
  if (!p->session[0]) { auth_refuse(q, 400, "Log in with your email to ask for recovery."); return; }
  recovery_state s = recovery_of(db, p->account);
  int verdict = (int)ar_start(p->level, p->strong, s.state);
  if (verdict != AR_OK) { auth_audit(db, p->account, p->session, "recovery.start", "refused", NULL); refuse_ar(q, verdict); return; }
  if (!auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY, p->account)) return;
  char tok[65], tsha[65];
  if (!auth_random_hex(tok, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  auth_sha_hex(tok, tsha);
  const char *v[3] = {p->account, p->session, tsha};
  PGresult *r = PQexecParams(db, "SELECT id, asked_ms, ready_ms FROM auth.recovery_ask($1, $2, $3)", 3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  long long asked = ok ? atoll(PQgetvalue(r, 0, 1)) : 0, ready = ok ? atoll(PQgetvalue(r, 0, 2)) : 0;
  if (!ok) api_log("auth: recovery ask: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) { refuse_ar(q, AR_ASKED); return; }
  char d[64];
  audit_made(db, p->account, p->session, "recovery.start", "ok", d, arw_wait_write(OUT(d)));
  char when[40], words[600];
  utc(ready, when, sizeof when);
  snprintf(words, sizeof words, "Someone logged in to your OpenAgents account with its email and asked to recover it. "
           "If nothing stops it, after %s a new key they choose replaces every key and passkey on the account. "
           "Logging in with your key or passkey cancels it, and you can cancel it from this notice.", when);
  char cancel_url[300]; snprintf(cancel_url, sizeof cancel_url, "%s/settings/security#cancel=%s", auth_origin(), tok);
  notice_link cancel = {"Cancel the recovery:", cancel_url};
  notify(db, p->account, "recovery.start", "someone asked to recover your account", words, &cancel, NULL, NULL);
  memset(cancel_url, 0, sizeof cancel_url);
  memset(tok, 0, sizeof tok);
  char json[300], lines[200];
  answer_made(q, 200, json, arw_pending_write(asked, ready, OUT(json)), lines, arw_pending_lines(asked, ready, OUT(lines)));
}

static void cancelled_notice(PGconn *db, const char *acct, const char *how) {
  char words[300];
  snprintf(words, sizeof words, "The recovery of your OpenAgents account was cancelled (%s). Nothing changed on the account.", how);
  notify(db, acct, "recovery.cancel", "recovery was cancelled", words, NULL, NULL, NULL);
}
static void recovery_cancel(const request *q, PGconn *db, const char *body) {
  char t[80], acct[40] = "", via[16] = "";
  if (auth_json_str(body, strlen(body), "t", t, sizeof t) && strlen(t) == 64) {
    char tsha[65]; auth_sha_hex(t, tsha);
    const char *v[1] = {tsha};
    one(db, "SELECT auth.recovery_cancel_by_link($1)", 1, v, acct, sizeof acct);
    if (!acct[0]) { auth_refuse(q, 410, "That link was used, or the recovery already ended."); return; }
    snprintf(via, sizeof via, "link");
  } else {
    const auth_principal *p = auth_principal_of(q);
    if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in, or use the link in the notice."); return; }
    snprintf(acct, sizeof acct, "%s", p->account);
    snprintf(via, sizeof via, "%s", p->credential == AUTH_NOSTR ? "key" : "session");
    const char *v[2] = {acct, via};
    if (one_int(db, "SELECT auth.recovery_cancel($1, $2)", 2, v) != 1) { auth_refuse(q, 409, "There is no recovery waiting."); return; }
  }
  char d[60], json[64], lines[64];
  audit_made(db, acct, NULL, "recovery.cancel", "ok", d, arw_via_write(cs(via), cs(""), OUT(d)));
  cancelled_notice(db, acct, !strcmp(via, "link") ? "from the link in a notice" : !strcmp(via, "key") ? "by a signature of a key on the account" : "from a login");
  answer_made(q, 200, json, arw_cancelled_write(OUT(json)), lines, arw_cancelled_lines(OUT(lines)));
}
void auth_recover_strong_login(PGconn *db, const char *acct, const char *sid) {
  (void)sid;
  if (!ar_cancels(2)) return;
  const char *v[2] = {acct, "login"};
  if (one_int(db, "SELECT auth.recovery_cancel($1, $2)", 2, v) != 1) return;
  char d[64];
  audit_made(db, acct, sid, "recovery.cancel", "ok", d, arw_via_write(cs("login"), cs(""), OUT(d)));
  cancelled_notice(db, acct, "someone logged in with a key, passkey or recovery code");
}

static void recovery_finish(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  recovery_state s = recovery_of(db, p->account);
  int verdict = (int)ar_finish(p->level, !strcmp(p->via, "email") && p->session[0], s.state);
  if (verdict != AR_OK) { auth_audit(db, p->account, p->session, "recovery.finish", "refused", NULL); refuse_ar(q, verdict); return; }
  char pk[65], d[64];
  if (!proof_key(q, db, p, "finish", body, pk)) return;
  if (key_owner(db, pk, p->account) == 2) {
    audit_made(db, p->account, p->session, "recovery.finish", "refused", d, arw_why_write(cs("another account's key"), OUT(d)));
    refuse_ar(q, AR_OTHER); return;
  }
  // Who hears it: every key and email now, the keys that are about to go included.
  const char *v[1] = {p->account};
  PGresult *old = PQexecParams(db, "SELECT string_agg(pubkey, ',' ORDER BY id) FROM auth.nostr_keys WHERE account = $1 AND pubkey <> $2", 2, NULL, (const char *[]){p->account, pk}, NULL, NULL, 0);
  (void)v;
  char old_keys[65 * 16] = "";   // the first 16 keys that are about to go
  if (PQresultStatus(old) == PGRES_TUPLES_OK && PQntuples(old) == 1 && !PQgetisnull(old, 0, 0)) {
    snprintf(old_keys, sizeof old_keys, "%s", PQgetvalue(old, 0, 0));   // cut at 16 whole keys: 16 * 65 - 1 characters
  }
  PQclear(old);
  char label[40] = "";
  auth_json_str(body, strlen(body), "label", label, sizeof label);
  if (strcmp(label, "extension") && strcmp(label, "bunker") && strcmp(label, "phone") && strcmp(label, "app")) label[0] = 0;
  const char *f[4] = {p->account, p->session, pk, label};
  char kid[24];
  PGresult *r = PQexecParams(db, "SELECT auth.recovery_finish($1, $2, $3, $4)", 4, NULL, f, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  if (ok) snprintf(kid, sizeof kid, "%s", PQgetvalue(r, 0, 0)); else api_log("auth: recovery finish: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) { refuse_ar(q, AR_WAITING); return; }
  auth_audit(db, p->account, p->session, "recovery.finish", "ok", NULL);
  char sk[40], words[500];
  short_key(pk, sk, sizeof sk);
  snprintf(words, sizeof words, "Your OpenAgents account was recovered: a new key (%s) replaced its keys and passkeys, and every other login ended. "
           "If this wasn't you, freeze the account from this notice and write to us.", sk);
  // one notice: every email, the new key, and the keys that were removed
  notify(db, p->account, "recovery.done", "your account was recovered", words, NULL, NULL, old_keys);
  char json[200], lines[200];
  answer_made(q, 200, json, arw_recovered_write(cs(pk), OUT(json)), lines, arw_recovered_lines(cs(pk), OUT(lines)));
}

static void code_new_route(const request *q, PGconn *db, const auth_principal *p) {
  (void)db;
  if (!in_child()) return;
  PGconn *c = api_db_new();
  if (!c) { auth_refuse(q, 503, "The database isn't answering. Try again."); _exit(0); }
  new_code fresh;
  if (!code_make(c, p->account, &fresh)) { auth_refuse(q, 500, "That didn't work. Try again."); PQfinish(c); _exit(0); }
  auth_audit(c, p->account, p->session, "recovery_code.new", "ok", NULL);
  char json[600], lines[400];
  answer_made(q, 200, json, arw_code_write(cs(fresh.words), OUT(json)), lines, arw_code_lines(cs(fresh.words), OUT(lines)));
  memset(&fresh, 0, sizeof fresh); memset(json, 0, sizeof json); memset(lines, 0, sizeof lines);
  notify(c, p->account, "code.new", "a new recovery code was made",
         "A new recovery code was made for your OpenAgents account. The old one no longer works.", NULL, NULL, NULL);
  PQfinish(c);
  _exit(0);
}

static void code_login(const request *q, PGconn *db, const char *body) {
  char raw[400], email[300], pk[80] = "", typed[600] = "", client[16] = "web", acct[40] = "";
  auth_json_str(body, strlen(body), "words", typed, sizeof typed);
  auth_json_str(body, strlen(body), "pubkey", pk, sizeof pk);
  auth_json_str(body, strlen(body), "client", client, sizeof client);
  if (!typed[0]) { auth_refuse(q, 400, "Type the 24 words of your recovery code."); return; }
  char ipk[65]; auth_visitor_key(q, ipk);
  if (!auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY_CODE_IP, ipk)) return;
  if (auth_json_str(body, strlen(body), "email", raw, sizeof raw) && raw[0]) {
    char all[400], newest[80];
    if (!auth_normal_email(raw, email, sizeof email)) { auth_refuse(q, 400, "That isn't an email address."); return; }
    if (!auth_lookups(email, all, sizeof all, newest)) { auth_refuse(q, 503, "Recovery isn't set up here yet."); return; }
    const char *v[2] = {all, NULL};
    one(db, "SELECT auth.account_named($1::text[], $2)", 2, v, acct, sizeof acct);
    memset(email, 0, sizeof email);
  } else if (strlen(pk) == 64) {
    const char *v[2] = {"{}", pk};
    one(db, "SELECT auth.account_named($1::text[], $2)", 2, v, acct, sizeof acct);
  } else { auth_refuse(q, 400, "Type the email or key of the account."); return; }
  if (!acct[0]) { auth_refuse(q, 401, "That recovery code isn't right for that account."); return; }
  if (!auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY_CODE, acct)) return;
  if (!in_child()) return;
  PGconn *c = api_db_new();
  if (!c) { auth_refuse(q, 503, "The database isn't answering. Try again."); _exit(0); }
  char id[24], d[64]; new_code fresh;
  if (!code_matches(c, acct, typed, id)) {
    audit_made(c, acct, NULL, "login.recovery_code", "refused", d, arw_why_write(cs("wrong code"), OUT(d)));
    auth_refuse(q, 401, "That recovery code isn't right for that account.");
    PQfinish(c); _exit(0);
  }
  PQclear(PQexec(c, "BEGIN"));
  const char *v[1] = {id};
  char spent[8]; one(c, "SELECT auth.code_spend($1::bigint)", 1, v, spent, sizeof spent);
  if (strcmp(spent, "t") || !code_make(c, acct, &fresh)) { PQclear(PQexec(c, "ROLLBACK")); auth_refuse(q, 500, "That didn't work. Try again."); PQfinish(c); _exit(0); }
  PQclear(PQexec(c, "COMMIT"));
  char ej[300], el[300];
  int64_t en = arw_code_member_write(cs(fresh.words), OUT(ej)), ln = arw_code_lines(cs(fresh.words), OUT(el));
  ej[en < 0 ? 0 : en] = 0; el[ln < 0 ? 0 : ln] = 0;   // 24 words of at most 8 letters: both fit
  auth_logged_in(q, c, acct, "recovery", id, "L2", !strcmp(client, "app"), false, "login.recovery_code", ej, el);
  memset(&fresh, 0, sizeof fresh); memset(ej, 0, sizeof ej); memset(el, 0, sizeof el);
  notify(c, acct, "code.used", "your recovery code was used",
         "Your OpenAgents recovery code was used to log in. It no longer works; a new code was shown once to whoever used it.", NULL, NULL, NULL);
  PQfinish(c);
  _exit(0);
}

// ---------------------------------------------------------------- dispatch
bool auth_recover_route(const request *q, PGconn *db, const char *op) {
  static const char *const gets[] = {"ways", "notices", "recovery", NULL};
  static const char *const posts[] = {"proof/challenge", "step-up", "link/nostr", "unlink", "recovery/start", "recovery/cancel",
                                      "recovery/finish", "recovery-code", "recovery-code/login", NULL};
  bool get = false, post = false;
  for (const char *const *o = gets; *o; o++) if (!strcmp(op, *o)) get = true;
  for (const char *const *o = posts; *o; o++) if (!strcmp(op, *o)) post = true;
  if (!get && !post) return false;
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  bool is_get = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD");
  if (get && !is_get) { auth_refuse(q, 405, "GET this."); return true; }
  if (post && strcmp(q->method, "POST")) { auth_refuse(q, 405, "POST this."); return true; }
  if (post) {
    char from[256];
    if (api_header(q, "Origin", from, sizeof from) && strcmp(from, auth_origin())) { auth_refuse(q, 403, "Not from this site."); return true; }
  }
  const auth_principal *p = auth_principal_of(q);
  bool public_route = !strcmp(op, "recovery/cancel") || !strcmp(op, "recovery-code/login");
  if (!public_route && (!p || !p->account[0])) { auth_refuse(q, 401, "Log in first."); return true; }
  if (!strcmp(op, "ways")) { ways(q, db, p); return true; }
  if (!strcmp(op, "notices")) { notices(q, db, p); return true; }
  if (!strcmp(op, "recovery")) { recovery_get(q, db, p); return true; }
  char *body = body_of(q);
  if (!body) { auth_refuse(q, 500, "Out of memory."); return true; }
  if (!strcmp(op, "proof/challenge")) proof_challenge(q, db, p, body);
  else if (!strcmp(op, "step-up")) step_up(q, db, p, body);
  else if (!strcmp(op, "link/nostr")) link_nostr(q, db, p, body);
  else if (!strcmp(op, "unlink")) unlink_way(q, db, p, body);
  else if (!strcmp(op, "recovery/start")) recovery_start(q, db, p);
  else if (!strcmp(op, "recovery/cancel")) recovery_cancel(q, db, body);
  else if (!strcmp(op, "recovery/finish")) recovery_finish(q, db, p, body);
  else if (!strcmp(op, "recovery-code")) code_new_route(q, db, p);
  else if (!strcmp(op, "recovery-code/login")) code_login(q, db, body);
  memset(body, 0, q->body_len);
  free(body);
  return true;
}
