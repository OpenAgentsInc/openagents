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
// OA_AUTH_NOTICE_RELAY (wss://relay.openagents.com). Notices go from a child process, so a slow
// mail service or relay never holds up the API; OA_AUTH_NOTICE_SYNC=1 (the checks) sends them inline.
//
// A recovery code is 24 BIP-39 words, stored as scrypt(words, salt, N = 2^17, r = 8, p = 1): 128 MiB
// and most of a second, so each check runs in a child process too (the API answers one request at
// a time). OA_AUTH_SCRYPT_LOG_N (10 to 17) lowers N for the checks only; it is never set in deploys.
#include "auth-internal.h"
#include "auth-recover-policy.generated.h"
#include "auth-recover-crypto.generated.h"
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
static void recovery_json(const recovery_state *s, char *out, size_t cap) {
  snprintf(out, cap, "{\"state\":\"%s\",\"asked_ms\":%lld,\"ready_ms\":%lld,\"cancelled_ms\":%lld,\"done_ms\":%lld,\"now_ms\":%lld,\"wait_hours\":72%s%s%s}",
           state_name(s->state), s->asked, s->ready, s->cancelled, s->done, s->now,
           s->via[0] ? ",\"cancelled_via\":\"" : "", s->via, s->via[0] ? "\"" : "");
}
static void methods_json(PGconn *db, const char *acct, char *out, size_t cap) {
  const char *v[1] = {acct};
  if (!one(db, "SELECT to_json(auth.methods_of($1))::text", 1, v, out, cap) || !out[0]) snprintf(out, cap, "[]");
}

// ---------------------------------------------------------------- notices
// Sends one notice to every email and key of the account, and to also_email / also_pubkey (a way
// just removed). Every delivery is counted on the notice row and in the audit log.
static void deliver(PGconn *db, long long notice_id, const char *acct, const char *what, const char *subject, const char *words,
                    const char *cancel_token, const char *also_email, const char *also_pubkey) {
  char freeze_tok[65] = "", freeze_url[300] = "", cancel_url[300] = "";
  if (auth_freeze_link(db, acct, freeze_tok)) snprintf(freeze_url, sizeof freeze_url, "%s/settings/security#freeze=%s", auth_origin(), freeze_tok);
  if (cancel_token && *cancel_token) snprintf(cancel_url, sizeof cancel_url, "%s/settings/security#cancel=%s", auth_origin(), cancel_token);
  char text[1400];
  snprintf(text, sizeof text, "%s\n\n%s%s%s%s%s%sThis is a security notice from OpenAgents. We send one for every change to how you log in.\n",
           words,
           cancel_url[0] ? "Cancel the recovery:\n" : "", cancel_url, cancel_url[0] ? "\n\n" : "",
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
    char to[600], subj[200], body[2400], message[3600], id[128], err[300];
    auth_json_esc(email, to, sizeof to); auth_json_esc(subject, subj, sizeof subj); auth_json_esc(text, body, sizeof body);
    // The key names the account too: staging and production share one mail account, and their
    // notice ids both start at 1 (a reused key with other words is refused, 409).
    snprintf(message, sizeof message, "{\"to\":\"%s\",\"subject\":\"OpenAgents security: %s\",\"idempotency_key\":\"notice-%s-%lld-%d\",\"text\":\"%s\"}",
             to, subj, acct, notice_id, i, body);
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
    char id[65] = "", err[200];
    const char *at = strstr(wrap, "\"id\":\"");
    if (at) snprintf(id, sizeof id, "%.64s", at + 6);
    if (strlen(id) == 64 && relay_send(rl, wrap, id, err, sizeof err)) nostr_sent++;
    else { failed++; api_log("auth: notice %lld nostr: %s", notice_id, err); }
  }
  PQclear(r);
  if (rl) relay_close(rl);
  free(work); free(wrap);
  memset(sk, 0, sizeof sk);
  char sent[120], idb[24];
  snprintf(sent, sizeof sent, "{\"email\":%d,\"nostr\":%d,\"failed\":%d}", emailed, nostr_sent, failed);
  snprintf(idb, sizeof idb, "%lld", notice_id);
  const char *w[2] = {idb, sent};
  PQclear(PQexecParams(db, "SELECT auth.notice_sent($1::bigint, $2::jsonb)", 2, NULL, w, NULL, NULL, 0));
  char detail[200]; snprintf(detail, sizeof detail, "{\"what\":\"%s\",\"email\":%d,\"nostr\":%d,\"failed\":%d}", what, emailed, nostr_sent, failed);
  auth_audit(db, acct, NULL, "notice", failed && !emailed && !nostr_sent ? "failed" : "ok", detail);
}
// The notice's row now, its delivery from a child (inline under OA_AUTH_NOTICE_SYNC=1).
static void notify(PGconn *db, const char *acct, const char *what, const char *subject, const char *words,
                   const char *cancel_token, const char *also_email, const char *also_pubkey) {
  const char *v[3] = {acct, what, words};
  char idb[24];
  if (!one(db, "SELECT auth.notice_add($1, $2, $3)", 3, v, idb, sizeof idb) || !idb[0]) return;
  long long id = atoll(idb);
  if (!strcmp(env_or("OA_AUTH_NOTICE_SYNC", "0"), "1")) { deliver(db, id, acct, what, subject, words, cancel_token, also_email, also_pubkey); return; }
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid != 0) return;   // the parent, or no child: the row says nothing was sent
  PGconn *c = api_db_new();
  if (c) { deliver(c, id, acct, what, subject, words, cancel_token, also_email, also_pubkey); PQfinish(c); }
  _exit(0);
}
static void short_key(const char *pk, char *out, size_t cap) { snprintf(out, cap, "%.8s…%.4s", pk, pk + 60); }

// ---------------------------------------------------------------- the gate's step-up answer
void auth_step_up_refusal(const request *q, PGconn *db, const auth_principal *p, int need, int status, const char *words) {
  char methods[200], rec[400], e[400], json[1400], lines[800];
  // An admin page's old key-made login has no account: its key is its one way in.
  recovery_state s = {0};
  if (p->account[0]) { methods_json(db, p->account, methods, sizeof methods); s = recovery_of(db, p->account); }
  else snprintf(methods, sizeof methods, "[\"nostr\"]");
  recovery_json(&s, rec, sizeof rec);
  auth_json_esc(words, e, sizeof e);
  const char *lvl = need == 3 ? "L2-fresh" : "L2";
  snprintf(json, sizeof json, "{\"error\":\"%s\",\"step_up\":\"%s\",\"methods\":%s,\"limited\":%s,\"level\":\"L%d\",\"recovery\":%s}",
           e, lvl, methods, p->limited ? "true" : "false", p->level, rec);
  // methods as one line: nostr,recovery_code
  char flat[200]; size_t k = 0;
  for (const char *c = methods; *c && k + 1 < sizeof flat; c++) if (*c != '[' && *c != ']' && *c != '"' && *c != ' ') flat[k++] = *c;
  flat[k] = 0;
  snprintf(lines, sizeof lines, "error %s\nstep_up %s\nmethods %s\nlimited %d\nrecovery %s\nready_ms %lld\n", words, lvl, flat, p->limited, state_name(s.state), s.ready);
  auth_answer(q, status, NULL, json, lines);
}

// ---------------------------------------------------------------- proofs made while logged in
static const char *proof_route(const char *purpose) {
  return !strcmp(purpose, "stepup") ? "/api/auth/step-up" : !strcmp(purpose, "link") ? "/api/auth/link/nostr"
       : !strcmp(purpose, "finish") ? "/api/auth/recovery/finish" : NULL;
}
static void proof_challenge(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char purpose[16] = "";
  auth_json_str(body, strlen(body), "purpose", purpose, sizeof purpose);
  const char *route = proof_route(purpose);
  if (!route) { auth_refuse(q, 400, "Say what the signature is for: stepup, link or finish."); return; }
  if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return; }
  char id[33], ch[65], chsha[65];
  if (!auth_random_hex(id, 16) || !auth_random_hex(ch, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  auth_sha_hex(ch, chsha);
  const char *v[4] = {id, chsha, p->account, purpose};
  if (!one(db, "SELECT auth.proof_challenge($1, $2, $3, $4)", 4, v, NULL, 0)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  char json[600], lines[500];
  snprintf(json, sizeof json, "{\"challenge\":\"%s\",\"url\":\"%s%s?c=%s\",\"method\":\"POST\",\"expires_in\":300}", ch, auth_origin(), route, ch);
  snprintf(lines, sizeof lines, "challenge %s\nurl %s%s?c=%s\nexpires_in 300\n", ch, auth_origin(), route, ch);
  auth_answer(q, 200, NULL, json, lines);
}
// The key that signed this request's proof: a NIP-98 event (kind 27235, within 60 seconds) over
// this route with ?c=<challenge>, method POST, no payload tag, sent as {"proof": base64}. The
// challenge must be this account's, for this purpose, unused. false: it answered the refusal.
static bool proof_key(const request *q, PGconn *db, const auth_principal *p, const char *purpose, const char *body, char pk[65]) {
  char c[80], proof[4000], csha[65];
  if (!api_query(q, "c", c, sizeof c) || strlen(c) != 64) { auth_refuse(q, 400, "Ask for a challenge first."); return false; }
  if (!auth_json_str(body, strlen(body), "proof", proof, sizeof proof) || !proof[0]) { auth_refuse(q, 400, "Sign the challenge with your key."); return false; }
  char authz[4100], url[400], err[160];
  snprintf(authz, sizeof authz, "Nostr %s", proof);
  snprintf(url, sizeof url, "%s%s?c=%s", auth_origin(), q->path, c);
  nostr_event ev;
  if (!nip98_check(authz, "POST", url, NULL, 0, (long long)time(NULL), &ev, err, sizeof err)) {
    auth_audit(db, p->account, p->session, purpose, "refused", "{\"why\":\"bad signature\"}");
    auth_refuse(q, 401, "That signature isn't good for this challenge. Sign it again.");
    return false;
  }
  auth_sha_hex(c, csha);
  const char *v[4] = {csha, p->account, purpose, ev.id};
  char state[16];
  one(db, "SELECT auth.proof_spend($1, $2, $3, $4)", 4, v, state, sizeof state);
  if (strcmp(state, "ok")) {
    auth_audit(db, p->account, p->session, purpose, "refused", !strcmp(state, "replay") ? "{\"why\":\"replay\"}" : "{\"why\":\"no challenge\"}");
    auth_refuse(q, 401, !strcmp(state, "replay") ? "That signature was used already." : "That challenge has ended. Ask for a new one.");
    return false;
  }
  memcpy(pk, ev.pubkey, 65);
  return true;
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
  const char *v[1] = {p->account};
  size_t cap = 24000;
  char *json = malloc(cap), *lines = malloc(8000);
  if (!json || !lines) { free(json); free(lines); auth_refuse(q, 500, "Out of memory."); return; }
  size_t j = (size_t)snprintf(json, cap, "{\"account\":\"%s\",\"level\":\"L%d\",\"via\":\"%s\",\"l2_age_ms\":%lld,\"limited\":%s,\"emails\":[",
                              p->account, p->level, p->via, p->l2_age_ms, p->limited ? "true" : "false");
  size_t l = (size_t)snprintf(lines, 8000, "account %s\nlevel L%d\nvia %s\nl2_age_ms %lld\nlimited %d\n", p->account, p->level, p->via, p->l2_age_ms, p->limited);
  PGresult *r = PQexecParams(db, "SELECT id, coalesce(sealed, ''), (extract(epoch FROM created_at) * 1000)::bigint FROM auth.emails WHERE account = $1 ORDER BY id", 1, NULL, v, NULL, NULL, 0);
  char ad[96]; snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r); i++) {
    char email[300], masked[320] = "•••";
    if (auth_unseal(ad, PQgetvalue(r, i, 1), email, sizeof email)) { auth_mask_email(email, masked, sizeof masked); memset(email, 0, sizeof email); }
    j += (size_t)snprintf(json + j, cap - j, "%s{\"id\":%s,\"email\":\"%s\",\"added_ms\":%s}", i ? "," : "", PQgetvalue(r, i, 0), masked, PQgetvalue(r, i, 2));
    l += (size_t)snprintf(lines + l, 8000 - l, "email %s %s\n", PQgetvalue(r, i, 0), masked);
  }
  PQclear(r);
  j += (size_t)snprintf(json + j, cap - j, "],\"keys\":[");
  r = PQexecParams(db, "SELECT id, pubkey, coalesce(label, ''), (extract(epoch FROM created_at) * 1000)::bigint FROM auth.nostr_keys WHERE account = $1 ORDER BY id", 1, NULL, v, NULL, NULL, 0);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r) && j + 300 < cap; i++) {
    char lab[100]; auth_json_esc(PQgetvalue(r, i, 2), lab, sizeof lab);
    j += (size_t)snprintf(json + j, cap - j, "%s{\"id\":%s,\"pubkey\":\"%s\",\"label\":\"%s\",\"added_ms\":%s}", i ? "," : "", PQgetvalue(r, i, 0), PQgetvalue(r, i, 1), lab, PQgetvalue(r, i, 3));
    if (l + 100 < 8000) l += (size_t)snprintf(lines + l, 8000 - l, "key %s %s\n", PQgetvalue(r, i, 0), PQgetvalue(r, i, 1));
  }
  PQclear(r);
  char methods[200], rec[400], extra[200];
  // An admin page's old key-made login has no account: its key is its one way in.
  recovery_state s = {0};
  if (p->account[0]) { methods_json(db, p->account, methods, sizeof methods); s = recovery_of(db, p->account); }
  else snprintf(methods, sizeof methods, "[\"nostr\"]");
  recovery_json(&s, rec, sizeof rec);
  one(db, "SELECT json_build_object('passkeys', (SELECT count(*) FROM auth.passkeys WHERE account = $1), 'recovery_code', "
          "EXISTS (SELECT 1 FROM auth.recovery WHERE account = $1 AND spent_at IS NULL), 'ways', auth.login_ways($1), 'strong', auth.strong_ways($1))::text",
      1, v, extra, sizeof extra);
  snprintf(json + j, cap - j, "],\"methods\":%s,\"recovery\":%s,\"counts\":%s}", methods, rec, extra[0] ? extra : "{}");
  char flat[200], codes[8] = "0", passkeys[16] = "0";
  size_t k = 0;
  for (const char *c = methods; *c && k + 1 < sizeof flat; c++) if (*c != '[' && *c != ']' && *c != '"' && *c != ' ') flat[k++] = *c;
  flat[k] = 0;
  one(db, "SELECT CASE WHEN EXISTS (SELECT 1 FROM auth.recovery WHERE account = $1 AND spent_at IS NULL) THEN 1 ELSE 0 END", 1, v, codes, sizeof codes);
  one(db, "SELECT count(*) FROM auth.passkeys WHERE account = $1", 1, v, passkeys, sizeof passkeys);
  snprintf(lines + l, 8000 - l, "methods %s\nrecovery_code %s\npasskeys %s\nrecovery %s\nasked_ms %lld\nready_ms %lld\nnow_ms %lld\n",
           flat, codes, passkeys, state_name(s.state), s.asked, s.ready, s.now);
  auth_answer(q, 200, NULL, json, lines);
  free(json); free(lines);
}
static void notices(const request *q, PGconn *db, const auth_principal *p) {
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT auth.notices_of($1)::text", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  size_t n = (size_t)PQgetlength(r, 0, 0) + 32;
  char *json = malloc(n);
  if (json) { snprintf(json, n, "{\"notices\":%s}", PQgetvalue(r, 0, 0)); }
  PQclear(r);
  // lines: "notice AT_MS WHAT", newest first (the page shows when and what; the words went in the notice)
  char lines[3000]; size_t l = 0; lines[0] = 0;
  r = PQexecParams(db, "SELECT (extract(epoch FROM at) * 1000)::bigint, what FROM auth.notices WHERE account = $1 ORDER BY id DESC LIMIT 30", 1, NULL, v, NULL, NULL, 0);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r) && l + 80 < sizeof lines; i++)
    l += (size_t)snprintf(lines + l, sizeof lines - l, "notice %s %s\n", PQgetvalue(r, i, 0), PQgetvalue(r, i, 1));
  PQclear(r);
  if (json) { auth_answer(q, 200, NULL, json, lines); free(json); } else auth_refuse(q, 500, "Out of memory.");
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
    char kid[24];
    one(db, "SELECT id FROM auth.nostr_keys WHERE pubkey = $1 AND account = $2", 2, v, kid, sizeof kid);
    if (!kid[0]) {
      auth_audit(db, p->account, p->session, "stepup", "refused", "{\"via\":\"nostr\",\"why\":\"not this account's key\"}");
      auth_refuse(q, 403, "That key isn't on this account. Sign with a key you added, or use your recovery code.");
      return;
    }
    if (!raise_session(db, p, "nostr", kid)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    auth_audit(db, p->account, p->session, "stepup", "ok", "{\"via\":\"nostr\"}");
    auth_answer(q, 200, NULL, "{\"state\":\"stepped-up\",\"level\":\"L2\"}", "state stepped-up\nlevel L2\n");
    return;
  }
  // The recovery code: limits first, then scrypt in a child that answers.
  char ipk[65]; auth_visitor_key(q, ipk);
  if (!auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY_CODE_IP, ipk) || !auth_limit_hit(q, db, AUTH_LIMIT_RECOVERY_CODE, p->account)) return;
  if (!in_child()) return;
  PGconn *c = api_db_new();
  if (!c) { auth_refuse(q, 503, "The database isn't answering. Try again."); _exit(0); }
  char id[24]; new_code fresh;
  if (!code_matches(c, p->account, typed, id)) {
    auth_audit(c, p->account, p->session, "stepup", "refused", "{\"via\":\"recovery_code\",\"why\":\"wrong code\"}");
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
  auth_audit(c, p->account, p->session, "stepup", "ok", "{\"via\":\"recovery_code\"}");
  char json[600], lines[400];
  snprintf(json, sizeof json, "{\"state\":\"stepped-up\",\"level\":\"L2\",\"recovery_code\":\"%s\"}", fresh.words);
  snprintf(lines, sizeof lines, "state stepped-up\nlevel L2\nrecovery_code %s\n", fresh.words);
  auth_answer(q, 200, NULL, json, lines);
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
    char d[120]; snprintf(d, sizeof d, "{\"kind\":\"nostr\",\"why\":\"%s\"}", verdict == AR_OTHER ? "another account's" : verdict == AR_ALREADY ? "already" : "step up");
    auth_audit(db, p->account, p->session, "link", "refused", d);
    refuse_ar(q, verdict);
    return;
  }
  char label[40] = "";
  auth_json_str(body, strlen(body), "label", label, sizeof label);
  if (strcmp(label, "extension") && strcmp(label, "bunker") && strcmp(label, "phone") && strcmp(label, "app")) label[0] = 0;
  const char *v[3] = {p->account, pk, label};
  char kid[24];
  if (!one(db, "SELECT auth.key_link($1, $2, $3)", 3, v, kid, sizeof kid) || !kid[0]) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  auth_audit(db, p->account, p->session, "link", "ok", "{\"kind\":\"nostr\"}");
  char sk[40], words[300];
  short_key(pk, sk, sizeof sk);
  snprintf(words, sizeof words, "A Nostr key (%s) was added to your OpenAgents account. It can now log in and change the account.", sk);
  notify(db, p->account, "link.nostr", "a Nostr key was added", words, NULL, NULL, NULL);
  char json[200], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"linked\",\"id\":%s,\"pubkey\":\"%s\"}", kid, pk);
  snprintf(lines, sizeof lines, "state linked\nid %s\npubkey %s\n", kid, pk);
  auth_answer(q, 200, NULL, json, lines);
}

static void unlink_way(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char kind[16] = "", idtxt[24] = "";
  auth_json_str(body, strlen(body), "kind", kind, sizeof kind);
  // id: a number, as a JSON number or string
  const char *at = strstr(body, "\"id\"");
  if (at) { at = strchr(at + 4, ':'); if (at) { at++; while (*at == ' ' || *at == '"') at++; size_t k = 0; while (k + 1 < sizeof idtxt && at[k] >= '0' && at[k] <= '9') { idtxt[k] = at[k]; k++; } idtxt[k] = 0; } }
  if ((strcmp(kind, "email") && strcmp(kind, "nostr") && strcmp(kind, "passkey")) || !idtxt[0]) { auth_refuse(q, 400, "Say which way in to remove: kind and id."); return; }
  const char *v[1] = {p->account};
  int verdict = (int)ar_unlink(one_int(db, "SELECT auth.login_ways($1)", 1, v) - 1);
  if (verdict != AR_OK) { auth_audit(db, p->account, p->session, "unlink", "refused", "{\"why\":\"last way in\"}"); refuse_ar(q, verdict); return; }
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
  char d[120]; snprintf(d, sizeof d, "{\"kind\":\"%s\",\"ended\":%d}", kind, ended);
  auth_audit(db, p->account, p->session, "unlink", "ok", d);
  char words[400];
  snprintf(words, sizeof words, "%c%s was removed from your OpenAgents account. It can't log in any more, and its logins ended.", label[0] == 't' ? 'T' : 'A', label + 1);
  notify(db, p->account, "unlink", "a way to log in was removed", words, NULL, also_email, also_pk[0] ? also_pk : NULL);
  memset(also_email, 0, sizeof also_email);
  char json[120], lines[80];
  snprintf(json, sizeof json, "{\"state\":\"removed\",\"ended\":%d}", ended);
  snprintf(lines, sizeof lines, "state removed\nended %d\n", ended);
  auth_answer(q, 200, NULL, json, lines);
}

static void recovery_get(const request *q, PGconn *db, const auth_principal *p) {
  recovery_state s = recovery_of(db, p->account);
  char rec[400], lines[300];
  recovery_json(&s, rec, sizeof rec);
  snprintf(lines, sizeof lines, "state %s\nasked_ms %lld\nready_ms %lld\nnow_ms %lld\n", state_name(s.state), s.asked, s.ready, s.now);
  auth_answer(q, 200, NULL, rec, lines);
}

static void recovery_start(const request *q, PGconn *db, const auth_principal *p) {
  if (!p->session[0]) { auth_refuse(q, 400, "Log in with your email to ask for recovery."); return; }
  recovery_state s = recovery_of(db, p->account);
  int verdict = (int)ar_start(p->level, p->strong, s.state);
  if (verdict != AR_OK) { auth_audit(db, p->account, p->session, "recovery.start", "refused", "{}"); refuse_ar(q, verdict); return; }
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
  auth_audit(db, p->account, p->session, "recovery.start", "ok", "{\"wait_hours\":72}");
  char when[40], words[600];
  utc(ready, when, sizeof when);
  snprintf(words, sizeof words, "Someone logged in to your OpenAgents account with its email and asked to recover it. "
           "If nothing stops it, after %s a new key they choose replaces every key and passkey on the account. "
           "Logging in with your key or passkey cancels it, and so does the link below.", when);
  notify(db, p->account, "recovery.start", "someone asked to recover your account", words, tok, NULL, NULL);
  memset(tok, 0, sizeof tok);
  char json[300], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"pending\",\"asked_ms\":%lld,\"ready_ms\":%lld,\"wait_hours\":72}", asked, ready);
  snprintf(lines, sizeof lines, "state pending\nasked_ms %lld\nready_ms %lld\n", asked, ready);
  auth_answer(q, 200, NULL, json, lines);
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
  char d[60]; snprintf(d, sizeof d, "{\"via\":\"%s\"}", via);
  auth_audit(db, acct, NULL, "recovery.cancel", "ok", d);
  cancelled_notice(db, acct, !strcmp(via, "link") ? "from the link in a notice" : !strcmp(via, "key") ? "by a signature of a key on the account" : "from a login");
  auth_answer(q, 200, NULL, "{\"state\":\"cancelled\"}", "state cancelled\n");
}
void auth_recover_strong_login(PGconn *db, const char *acct, const char *sid) {
  (void)sid;
  if (!ar_cancels(2)) return;
  const char *v[2] = {acct, "login"};
  if (one_int(db, "SELECT auth.recovery_cancel($1, $2)", 2, v) != 1) return;
  auth_audit(db, acct, sid, "recovery.cancel", "ok", "{\"via\":\"login\"}");
  cancelled_notice(db, acct, "someone logged in with a key, passkey or recovery code");
}

static void recovery_finish(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  recovery_state s = recovery_of(db, p->account);
  int verdict = (int)ar_finish(p->level, !strcmp(p->via, "email") && p->session[0], s.state);
  if (verdict != AR_OK) { auth_audit(db, p->account, p->session, "recovery.finish", "refused", "{}"); refuse_ar(q, verdict); return; }
  char pk[65];
  if (!proof_key(q, db, p, "finish", body, pk)) return;
  if (key_owner(db, pk, p->account) == 2) { auth_audit(db, p->account, p->session, "recovery.finish", "refused", "{\"why\":\"another account's key\"}"); refuse_ar(q, AR_OTHER); return; }
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
  auth_audit(db, p->account, p->session, "recovery.finish", "ok", "{}");
  char sk[40], words[500];
  short_key(pk, sk, sizeof sk);
  snprintf(words, sizeof words, "Your OpenAgents account was recovered: a new key (%s) replaced its keys and passkeys, and every other login ended. "
           "If this wasn't you, freeze the account with the link below and write to us.", sk);
  // one notice: every email, the new key, and the keys that were removed
  notify(db, p->account, "recovery.done", "your account was recovered", words, NULL, NULL, old_keys);
  char json[200], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"recovered\",\"level\":\"L2\",\"pubkey\":\"%s\"}", pk);
  snprintf(lines, sizeof lines, "state recovered\nlevel L2\npubkey %s\n", pk);
  auth_answer(q, 200, NULL, json, lines);
}

static void code_new_route(const request *q, PGconn *db, const auth_principal *p) {
  (void)db;
  if (!in_child()) return;
  PGconn *c = api_db_new();
  if (!c) { auth_refuse(q, 503, "The database isn't answering. Try again."); _exit(0); }
  new_code fresh;
  if (!code_make(c, p->account, &fresh)) { auth_refuse(q, 500, "That didn't work. Try again."); PQfinish(c); _exit(0); }
  auth_audit(c, p->account, p->session, "recovery_code.new", "ok", "{}");
  char json[600], lines[400];
  snprintf(json, sizeof json, "{\"recovery_code\":\"%s\",\"words\":24}", fresh.words);
  snprintf(lines, sizeof lines, "recovery_code %s\n", fresh.words);
  auth_answer(q, 200, NULL, json, lines);
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
  char id[24]; new_code fresh;
  if (!code_matches(c, acct, typed, id)) {
    auth_audit(c, acct, NULL, "login.recovery_code", "refused", "{\"why\":\"wrong code\"}");
    auth_refuse(q, 401, "That recovery code isn't right for that account.");
    PQfinish(c); _exit(0);
  }
  PQclear(PQexec(c, "BEGIN"));
  const char *v[1] = {id};
  char spent[8]; one(c, "SELECT auth.code_spend($1::bigint)", 1, v, spent, sizeof spent);
  if (strcmp(spent, "t") || !code_make(c, acct, &fresh)) { PQclear(PQexec(c, "ROLLBACK")); auth_refuse(q, 500, "That didn't work. Try again."); PQfinish(c); _exit(0); }
  PQclear(PQexec(c, "COMMIT"));
  char ej[300], el[300];
  snprintf(ej, sizeof ej, ",\"recovery_code\":\"%s\"", fresh.words);
  snprintf(el, sizeof el, "recovery_code %s\n", fresh.words);
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
