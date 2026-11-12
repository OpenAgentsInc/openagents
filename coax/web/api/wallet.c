// The wallet's encrypted seed backup and its daily spend limit (docs/auth/wallets-and-custody.md,
// decision D4; #13217). What may happen is wallet-policy.coax's (#authority); what a backup's
// slots and object must be is coax/lib/nostr/vault-wallet.coax's, through wallet-vault.coax (the
// same code the apps seal and open with). The rows are 0163's.
//
//   GET  /api/wallet                    the daily limit, what the last 24 hours counted, a raise that
//                                       is waiting, and the backup's public facts (no ciphertext)
//   POST /api/wallet/backup/export      the backup itself: its vault, epoch, slots and object (a fresh L2)
//   POST /api/wallet/backup {after, slots, object}   store one (a fresh L2): after is the epoch the
//                                       writer saw (0: none yet); it becomes after + 1
//   POST /api/wallet/backup/delete      delete it (a fresh L2)
//   POST /api/wallet/limit {daily_sat}  set the limit (a fresh L2): a raise counts in 24 hours
//   POST /api/wallet/limit/lower {daily_sat}   lower it: now, with no prompt
//   POST /api/wallet/limit/cancel {t}   cancel a waiting raise: a notice's link, or any login of the account
//   POST /api/wallet/spend {sats, payment_id, new_destination}   the app asks before it pays: go, or
//                                       prove it is you first (a fresh L2); a payment id counts once
//   POST /api/wallet/spend/void {payment_id}   that payment didn't go: it no longer counts
//
// slots is the base64 of every slot's text with a newline between them; object is the base64 of the
// object's bytes; numbers in a body are strings of digits.
//
// We can't open a backup. What reaches this file is the device's ciphertext and each slot's public
// fields (method, ids, label, time, and for a passkey its credential id); no PRF output, recovery
// code, key or seed is ever sent here, and nothing here could use one. We also can't stop a
// payment: the wallet's keys are on the person's device. /spend is the app asking; the app obeys.
//
// Every change to the backup or the limit is in auth.audit and told to every email and key of
// the account. Nothing of a slot or an object is logged.
#include "auth-internal.h"
#include "wallet-policy.generated.h"
#include "wallet-vault.generated.h"
#include "passkey-verify.generated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

// wallet-policy.coax's verdicts (WP_*), checked against it at start.
enum { WP_OK = 0, WP_STEP_UP = 1, WP_LIMITED = 2, WP_BAD_AMOUNT = 3, WP_REPLAY = 4, WP_BAD_LIMIT = 5, WP_SAME = 6, WP_LOWER_NOW = 7,
       WP_RAISE_LATER = 8, WP_NO_RAISE = 9, WP_CONFLICT = 10, WP_BAD_SLOTS = 11, WP_NEEDS_RECOVERY = 12, WP_NEEDS_PASSKEY = 13,
       WP_BAD_OBJECT = 14, WP_NOT_YOUR_PASSKEY = 15, WP_NO_BACKUP = 16, WP_NOT_YOURS = 17 };
enum { WV_PRF = 1, WV_RECOVERY = 2, WV_NOSTR = 3 };
#define SLOTS_MAX 24576
#define OBJECT_MAX 4096

// wallet-vault.coax's exports (wallet-vault-unit.c): wv_slots, wv_vault, wv_count, wv_object,
// wv_slot, wv_credential, wv_b64, wv_unb64. passkey-verify.coax's pv_b64url is how auth.passkeys
// keeps a credential id (base64url).
typedef coax_t_WvSlot wv_slot_t;

bool wallet_init(void) {
  return wp_default_daily() == 20000 && wp_spend(1, 0, 20000, false, -1, false, 0) == WP_OK && wp_spend(20001, 0, 20000, false, -1, false, 0) == WP_STEP_UP
      && wp_spend(1, 0, 20000, false, 0, true, 0) == WP_LIMITED && wp_spend(0, 0, 0, false, 0, false, 0) == WP_BAD_AMOUNT && wp_spend(5, 0, 9, false, 0, false, 6) == WP_REPLAY
      && wp_limit(-1, 0, 0, false) == WP_BAD_LIMIT && wp_limit(5, 5, 0, false) == WP_SAME && wp_limit(4, 5, -1, false) == WP_LOWER_NOW
      && wp_limit(6, 5, 0, false) == WP_RAISE_LATER && wp_cancel(false, true, false) == WP_NO_RAISE && wp_cancel(true, false, false) == WP_NOT_YOURS
      && wp_store(0, true, true, 1, 0, 0, false) == WP_CONFLICT && wp_store(1, true, true, 0, 0, 0, false) == WP_BAD_SLOTS
      && wp_store(5, true, true, 0, 0, 0, false) == WP_NEEDS_RECOVERY && wp_store(6, true, true, 0, 0, 0, false) == WP_NEEDS_PASSKEY
      && wp_store(0, false, true, 0, 0, 0, false) == WP_BAD_OBJECT && wp_store(0, true, false, 0, 0, 0, false) == WP_NOT_YOUR_PASSKEY
      && wp_remove(false, 0, false) == WP_NO_BACKUP && wp_read(-1, false) == WP_STEP_UP && wp_read(0, false) == WP_OK && wp_status(WP_CONFLICT) == 409 && wp_status(WP_STEP_UP) == 401;
}

// ---------------------------------------------------------------- small pieces
static void words_of(coax_string w, char *out, size_t cap) { snprintf(out, cap, "%.*s", (int)w.count, w.data); }
// A refusal in the policy's words; a step-up the page or app can act on names the level wanted.
static void refuse_wp(const request *q, PGconn *db, const auth_principal *p, int verdict) {
  char w[200]; words_of(wp_words(verdict), w, sizeof w);
  if (verdict == WP_STEP_UP || verdict == WP_LIMITED) { auth_step_up_refusal(q, db, p, 3, (int)wp_status(verdict), w); return; }
  auth_refuse(q, (int)wp_status(verdict), w);
}
static bool one(PGconn *db, const char *sql, int n, const char *const *v, char *out, size_t cap) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) >= 1;
  if (out && cap) { out[0] = 0; if (ok && !PQgetisnull(r, 0, 0)) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0)); }
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("wallet: %.60s: %s", sql, PQerrorMessage(db));
  PQclear(r);
  return ok;
}
// A body's number: a string of 1 to 16 digits (or a bare number). -1 when it isn't one.
static long long number(const char *body, const char *key) {
  char t[32] = "";
  if (!auth_json_str(body, strlen(body), key, t, sizeof t) || !t[0]) {
    char want[48]; snprintf(want, sizeof want, "\"%s\"", key);
    const char *p = strstr(body, want);
    if (!p) return -1;
    p += strlen(want);
    while (*p == ' ' || *p == ':') p++;
    size_t k = 0;
    while (*p >= '0' && *p <= '9' && k + 1 < sizeof t) t[k++] = *p++;
    t[k] = 0;
  }
  size_t n = strlen(t);
  if (n < 1 || n > 16) return -1;
  for (size_t i = 0; i < n; i++) if (t[i] < '0' || t[i] > '9') return -1;
  return atoll(t);
}
static void sats_text(long long sats, char *out, size_t cap) {   // 20,000
  char raw[32]; int n = snprintf(raw, sizeof raw, "%lld", sats); size_t k = 0;
  for (int i = 0; i < n && k + 2 < cap; i++) { out[k++] = raw[i]; if ((n - i - 1) % 3 == 0 && i != n - 1) out[k++] = ','; }
  out[k] = 0;
}
static void utc(long long ms, char *out, size_t cap) {
  time_t t = (time_t)(ms / 1000); struct tm tm; gmtime_r(&t, &tm);
  strftime(out, cap, "%Y-%m-%d %H:%M UTC", &tm);
}

// ---------------------------------------------------------------- the limit's facts
typedef struct { long long daily, pending, asked_ms, pending_ms, now_ms, in_force; bool waiting; } limit_facts;
static bool limit_of(PGconn *db, const char *acct, limit_facts *f) {
  memset(f, 0, sizeof *f);
  char def[24]; snprintf(def, sizeof def, "%lld", (long long)wp_default_daily());
  const char *v[2] = {acct, def};
  PGresult *r = PQexecParams(db, "SELECT daily_sat, pending_sat, pending_asked_ms, pending_ms, now_ms FROM auth.wallet_limit_of($1, $2::bigint)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) {
    f->daily = atoll(PQgetvalue(r, 0, 0)); f->pending = atoll(PQgetvalue(r, 0, 1)); f->asked_ms = atoll(PQgetvalue(r, 0, 2));
    f->pending_ms = atoll(PQgetvalue(r, 0, 3)); f->now_ms = atoll(PQgetvalue(r, 0, 4));
  } else api_log("wallet: limit: %s", PQerrorMessage(db));
  PQclear(r);
  f->in_force = wp_in_force(f->daily, f->pending, f->pending_ms, f->now_ms);
  f->waiting = wp_waiting(f->pending, f->pending_ms, f->now_ms);
  return ok;
}
static bool spent_of(PGconn *db, const char *acct, const char *pid, long long *spent, long long *seen) {
  const char *v[2] = {acct, pid};
  PGresult *r = PQexecParams(db, "SELECT spent, seen FROM auth.wallet_spent($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) { *spent = atoll(PQgetvalue(r, 0, 0)); *seen = atoll(PQgetvalue(r, 0, 1)); }
  PQclear(r);
  return ok;
}

// ---------------------------------------------------------------- the backup's facts
typedef struct { bool found; long long epoch, created_ms, updated_ms; char vault[65]; char *slots; size_t slots_n; unsigned char *object; size_t object_n; } backup_row;
static void backup_free(backup_row *b) { free(b->slots); free(b->object); b->slots = NULL; b->object = NULL; }
static bool backup_of(PGconn *db, const char *acct, backup_row *b) {
  memset(b, 0, sizeof *b);
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT vault, epoch, slots, object_hex, created_ms, updated_ms FROM auth.wallet_backup_of($1)", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("wallet: backup: %s", PQerrorMessage(db)); PQclear(r); return false; }
  if (PQntuples(r) == 1) {
    b->found = true;
    snprintf(b->vault, sizeof b->vault, "%s", PQgetvalue(r, 0, 0));
    b->epoch = atoll(PQgetvalue(r, 0, 1)); b->created_ms = atoll(PQgetvalue(r, 0, 4)); b->updated_ms = atoll(PQgetvalue(r, 0, 5));
    b->slots_n = (size_t)PQgetlength(r, 0, 2);
    b->slots = malloc(b->slots_n + 1);
    const char *hex = PQgetvalue(r, 0, 3);
    b->object_n = strlen(hex) / 2;
    b->object = malloc(b->object_n + 1);
    if (!b->slots || !b->object || !auth_unhex(hex, b->object, b->object_n)) { backup_free(b); PQclear(r); return false; }
    memcpy(b->slots, PQgetvalue(r, 0, 2), b->slots_n); b->slots[b->slots_n] = 0;
  }
  PQclear(r);
  return true;
}
// Whether slot i's passkey is one this account has (true for a slot that isn't a passkey's).
static bool slot_passkey(PGconn *db, const char *acct, coax_v_u8 slots, int64_t i, bool *is_passkey) {
  unsigned char cred[1024]; char text[1400];
  int64_t n = wv_credential(slots, i, (coax_v_u8){cred, sizeof cred});
  *is_passkey = n > 0;
  if (n == 0) return true;
  if (n < 0) return false;
  int64_t k = pv_b64url((coax_v_u8){cred, n}, (coax_v_u8){(uint8_t *)text, (int64_t)sizeof text - 1});
  if (k <= 0) return false;
  text[k] = 0;
  const char *v[2] = {acct, text};
  char have[8] = "0";
  one(db, "SELECT count(*) FROM auth.passkeys WHERE account = $1 AND credential_id = $2", 2, v, have, sizeof have);
  return atoi(have) == 1;
}
// The slots' public facts appended to an answer: JSON objects, and lines "slot METHOD CREATED LIVE LABEL".
// live: for a passkey, whether it is still on the account. The count of live passkey slots.
static int slots_rows(PGconn *db, const char *acct, const char *slots, size_t n, char *json, size_t jcap, size_t *j, char *lines, size_t lcap, size_t *l) {
  coax_v_u8 s = {(uint8_t *)slots, (int64_t)n};
  int live = 0;
  int64_t count = wv_count(s);
  for (int64_t i = 0; i < count && *j + 400 < jcap && *l + 200 < lcap; i++) {
    wv_slot_t one_slot = wv_slot(s, i);
    if (one_slot.from < 0) break;
    bool is_passkey = false, mine = slot_passkey(db, acct, s, i, &is_passkey);
    if (is_passkey && mine) live++;
    const char *method = one_slot.method == WV_PRF ? "passkey" : one_slot.method == WV_RECOVERY ? "recovery-code" : "nostr";
    char label[80], esc[200];
    snprintf(label, sizeof label, "%.*s", (int)(one_slot.label_to - one_slot.label_from), slots + one_slot.label_from);
    auth_json_esc(label, esc, sizeof esc);
    *j += (size_t)snprintf(json + *j, jcap - *j, "%s{\"method\":\"%s\",\"label\":\"%s\",\"created_at\":%lld,\"live\":%s}", i ? "," : "", method, esc,
                           (long long)one_slot.created_at, !is_passkey || mine ? "true" : "false");
    *l += (size_t)snprintf(lines + *l, lcap - *l, "slot %s %lld %d %s\n", method, (long long)one_slot.created_at, !is_passkey || mine, label);
  }
  return live;
}

// ---------------------------------------------------------------- GET /api/wallet
static void summary(const request *q, PGconn *db, const auth_principal *p) {
  limit_facts f; backup_row b;
  long long spent = 0, seen = 0;
  if (!limit_of(db, p->account, &f) || !spent_of(db, p->account, "", &spent, &seen) || !backup_of(db, p->account, &b)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  size_t cap = 6000;
  char *json = malloc(cap), *lines = malloc(cap);
  if (!json || !lines) { free(json); free(lines); backup_free(&b); auth_refuse(q, 500, "Out of memory."); return; }
  char raise[120] = "null";
  if (f.waiting) snprintf(raise, sizeof raise, "{\"sat\":%lld,\"asked_ms\":%lld,\"at_ms\":%lld}", f.pending, f.asked_ms, f.pending_ms);
  size_t j = (size_t)snprintf(json, cap, "{\"limit\":{\"daily_sat\":%lld,\"default_sat\":%lld,\"spent_sat\":%lld,\"window_hours\":24,\"new_destination_sat\":%lld,"
                              "\"raise\":%s,\"raise_wait_hours\":24,\"now_ms\":%lld,\"enforced_by\":\"app\"},\"level\":\"L%d\",\"l2_age_ms\":%lld,\"backup\":",
                              f.in_force, (long long)wp_default_daily(), spent, (long long)wp_new_destination(), raise, f.now_ms, p->level, p->l2_age_ms);
  size_t l = (size_t)snprintf(lines, cap, "daily_sat %lld\ndefault_sat %lld\nspent_sat %lld\nnew_destination_sat %lld\nraise_sat %lld\nraise_asked_ms %lld\nraise_at_ms %lld\nnow_ms %lld\nlevel L%d\nl2_age_ms %lld\n",
                              f.in_force, (long long)wp_default_daily(), spent, (long long)wp_new_destination(), f.waiting ? f.pending : 0,
                              f.waiting ? f.asked_ms : 0, f.waiting ? f.pending_ms : 0, f.now_ms, p->level, p->l2_age_ms);
  if (!b.found) {
    j += (size_t)snprintf(json + j, cap - j, "null}");
    l += (size_t)snprintf(lines + l, cap - l, "backup 0\n");
  } else {
    j += (size_t)snprintf(json + j, cap - j, "{\"vault\":\"%s\",\"epoch\":%lld,\"created_ms\":%lld,\"updated_ms\":%lld,\"slots\":[", b.vault, b.epoch, b.created_ms, b.updated_ms);
    l += (size_t)snprintf(lines + l, cap - l, "backup 1\nvault %s\nepoch %lld\ncreated_ms %lld\nupdated_ms %lld\n", b.vault, b.epoch, b.created_ms, b.updated_ms);
    int live = slots_rows(db, p->account, b.slots, b.slots_n, json, cap, &j, lines, cap, &l);
    bool counts = wp_counts(true, live);
    j += (size_t)snprintf(json + j, cap - j, "],\"counts\":%s}}", counts ? "true" : "false");
    l += (size_t)snprintf(lines + l, cap - l, "counts %d\n", counts);
  }
  backup_free(&b);
  auth_answer(q, 200, NULL, json, lines);
  free(json); free(lines);
}

// ---------------------------------------------------------------- POST /api/wallet/backup/export
static void backup_get(const request *q, PGconn *db, const auth_principal *p) {
  int verdict = (int)wp_read(p->l2_age_ms, p->limited);
  if (verdict != WP_OK) { auth_audit(db, p->account, p->session, "wallet.backup.read", "refused", "{}"); refuse_wp(q, db, p, verdict); return; }
  backup_row b;
  if (!backup_of(db, p->account, &b)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  if (!b.found) { auth_answer(q, 200, NULL, "{\"backup\":null}", "backup 0\n"); return; }
  size_t cap = 2 * (b.slots_n + b.object_n) + 2000;
  char *json = malloc(cap), *lines = malloc(cap), *s64 = malloc(b.slots_n * 2 + 8), *o64 = malloc(b.object_n * 2 + 8);
  if (!json || !lines || !s64 || !o64) { free(json); free(lines); free(s64); free(o64); backup_free(&b); auth_refuse(q, 500, "Out of memory."); return; }
  int64_t sn = wv_b64((coax_v_u8){(uint8_t *)b.slots, (int64_t)b.slots_n}, (coax_v_u8){(uint8_t *)s64, (int64_t)(b.slots_n * 2 + 7)});
  int64_t on = wv_b64((coax_v_u8){b.object, (int64_t)b.object_n}, (coax_v_u8){(uint8_t *)o64, (int64_t)(b.object_n * 2 + 7)});
  if (sn <= 0 || on <= 0) { free(json); free(lines); free(s64); free(o64); backup_free(&b); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  s64[sn] = 0; o64[on] = 0;
  snprintf(json, cap, "{\"backup\":{\"account\":\"%s\",\"vault\":\"%s\",\"epoch\":%lld,\"updated_ms\":%lld,\"slots\":\"%s\",\"object\":\"%s\"}}",
           p->account, b.vault, b.epoch, b.updated_ms, s64, o64);
  snprintf(lines, cap, "backup 1\naccount %s\nvault %s\nepoch %lld\nupdated_ms %lld\nslots %s\nobject %s\n", p->account, b.vault, b.epoch, b.updated_ms, s64, o64);
  auth_audit(db, p->account, p->session, "wallet.backup.read", "ok", "{}");
  auth_answer(q, 200, NULL, json, lines);
  free(json); free(lines); free(s64); free(o64);
  backup_free(&b);
}

// ---------------------------------------------------------------- POST /api/wallet/backup
static void backup_put(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  size_t bl = strlen(body);
  char *s64 = malloc(bl + 1), *o64 = malloc(bl + 1);
  unsigned char *slots = malloc(SLOTS_MAX + 1), *object = malloc(OBJECT_MAX + 1);
  if (!s64 || !o64 || !slots || !object) { free(s64); free(o64); free(slots); free(object); auth_refuse(q, 500, "Out of memory."); return; }
  int64_t sn = -1, on = -1;
  if (auth_json_str(body, bl, "slots", s64, bl + 1) && s64[0]) sn = wv_unb64((coax_v_u8){(uint8_t *)s64, (int64_t)strlen(s64)}, (coax_v_u8){slots, SLOTS_MAX});
  if (auth_json_str(body, bl, "object", o64, bl + 1) && o64[0]) on = wv_unb64((coax_v_u8){(uint8_t *)o64, (int64_t)strlen(o64)}, (coax_v_u8){object, OBJECT_MAX});
  free(s64); free(o64);
  long long after = number(body, "after");
  backup_row cur;
  if (!backup_of(db, p->account, &cur)) { free(slots); free(object); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  long long epoch = cur.found ? cur.epoch : 0;
  bool replaced_vault = false;
  // What the slots and the object are: read by the code that seals them. Nothing here opens them.
  coax_v_u8 sv = {slots, sn > 0 ? sn : 0};
  int slots_code = sn > 0 ? (int)wv_slots(sv) : 1;
  bool object_ok = slots_code == 0 && on > 0 && wv_object((coax_v_u8){object, on}, sv);
  bool mine = true;
  char vault[65] = "";
  if (slots_code == 0) {
    int64_t count = wv_count(sv);
    for (int64_t i = 0; i < count; i++) { bool is_passkey = false; if (!slot_passkey(db, p->account, sv, i, &is_passkey)) mine = false; }
    if (!wv_vault(sv, (coax_v_u8){(uint8_t *)vault, 64})) slots_code = 1;
    vault[64] = 0;
    replaced_vault = cur.found && strcmp(cur.vault, vault) != 0;
  }
  backup_free(&cur);
  int verdict = (int)wp_store(slots_code, object_ok, mine, after, epoch, p->l2_age_ms, p->limited);
  if (verdict != WP_OK) {
    char d[120]; snprintf(d, sizeof d, "{\"verdict\":%d,\"after\":%lld,\"epoch\":%lld}", verdict, after, epoch);
    auth_audit(db, p->account, p->session, "wallet.backup", "refused", d);
    free(slots); free(object);
    refuse_wp(q, db, p, verdict);
    return;
  }
  char *hex = malloc((size_t)on * 2 + 4), after_s[24], got[24] = "";
  if (!hex) { free(slots); free(object); auth_refuse(q, 500, "Out of memory."); return; }
  hex[0] = '\\'; hex[1] = 'x'; auth_hex(object, (size_t)on, hex + 2);
  slots[sn] = 0;
  snprintf(after_s, sizeof after_s, "%lld", after);
  const char *v[5] = {p->account, after_s, vault, (const char *)slots, hex};
  one(db, "SELECT auth.wallet_backup_put($1, $2::bigint, $3, $4, $5::bytea)", 5, v, got, sizeof got);
  int64_t count = wv_count(sv);
  free(hex); free(slots); free(object);
  if (!got[0]) {   // another write came between the look and the store
    auth_audit(db, p->account, p->session, "wallet.backup", "refused", "{\"why\":\"epoch\"}");
    refuse_wp(q, db, p, WP_CONFLICT);
    return;
  }
  long long now_epoch = atoll(got);
  char d[160]; snprintf(d, sizeof d, "{\"epoch\":%lld,\"slots\":%lld,\"new_vault\":%s}", now_epoch, (long long)count, replaced_vault || epoch == 0 ? "true" : "false");
  auth_audit(db, p->account, p->session, "wallet.backup", "ok", d);
  if (epoch == 0)
    auth_notice(db, p->account, "wallet.backup", "your wallet was backed up",
                "An encrypted backup of your wallet was saved to your OpenAgents account. Only your passkey or your wallet recovery code opens it. We can't.");
  else if (replaced_vault)
    auth_notice(db, p->account, "wallet.backup.replace", "your wallet backup was replaced",
                "The encrypted backup of your wallet was replaced with a new one. The old one is gone. If this wasn't you, freeze your account and move your money to a new wallet.");
  else
    auth_notice(db, p->account, "wallet.backup.change", "your wallet backup changed",
                "The ways to open your wallet's encrypted backup changed. If this wasn't you, freeze your account and move your money to a new wallet.");
  char json[200], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"stored\",\"epoch\":%lld,\"vault\":\"%s\"}", now_epoch, vault);
  snprintf(lines, sizeof lines, "state stored\nepoch %lld\nvault %s\n", now_epoch, vault);
  auth_answer(q, 200, NULL, json, lines);
}

static void backup_delete(const request *q, PGconn *db, const auth_principal *p) {
  const char *a[1] = {p->account};
  char have[8] = "0";
  one(db, "SELECT count(*) FROM auth.wallet_backups WHERE account = $1", 1, a, have, sizeof have);
  int verdict = (int)wp_remove(atoi(have) == 1, p->l2_age_ms, p->limited);
  if (verdict != WP_OK) {
    char d[60]; snprintf(d, sizeof d, "{\"verdict\":%d}", verdict);
    auth_audit(db, p->account, p->session, "wallet.backup.delete", "refused", d);
    refuse_wp(q, db, p, verdict);
    return;
  }
  one(db, "SELECT auth.wallet_backup_drop($1)", 1, a, NULL, 0);
  auth_audit(db, p->account, p->session, "wallet.backup.delete", "ok", "{}");
  auth_notice(db, p->account, "wallet.backup.delete", "your wallet backup was deleted",
              "The encrypted backup of your wallet was deleted from your OpenAgents account. If you lose your phone now, only your wallet's written words bring your money back.");
  auth_answer(q, 200, NULL, "{\"state\":\"deleted\"}", "state deleted\n");
}

// ---------------------------------------------------------------- the limit
static void limit_set(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  long long want = number(body, "daily_sat");
  limit_facts f;
  if (!limit_of(db, p->account, &f)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  int verdict = (int)wp_limit(want, f.in_force, p->l2_age_ms, p->limited);
  char was[32], to[32], sat[24], cur[24];
  sats_text(f.in_force, was, sizeof was); sats_text(want < 0 ? 0 : want, to, sizeof to);
  snprintf(sat, sizeof sat, "%lld", want); snprintf(cur, sizeof cur, "%lld", f.in_force);
  char w[200]; words_of(wp_words(verdict), w, sizeof w);
  if (verdict == WP_LOWER_NOW) {
    const char *v[2] = {p->account, sat};
    if (!one(db, "SELECT auth.wallet_limit_lower($1, $2::bigint)", 2, v, NULL, 0)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    char d[120]; snprintf(d, sizeof d, "{\"from\":%lld,\"to\":%lld,\"how\":\"lower\"}", f.in_force, want);
    auth_audit(db, p->account, p->session, "wallet.limit", "ok", d);
    char words[300];
    snprintf(words, sizeof words, "Your wallet's daily limit was lowered from %s to %s sats. It counts now.%s", was, to, f.waiting ? " The raise that was waiting was cancelled." : "");
    auth_notice(db, p->account, "wallet.limit.lower", "your wallet's daily limit was lowered", words);
    char json[300], lines[200];
    snprintf(json, sizeof json, "{\"state\":\"lowered\",\"daily_sat\":%lld,\"words\":\"%s\"}", want, w);
    snprintf(lines, sizeof lines, "state lowered\ndaily_sat %lld\nwords %s\n", want, w);
    auth_answer(q, 200, NULL, json, lines);
    return;
  }
  if (verdict == WP_RAISE_LATER) {
    char tok[65], sha[65], at[24] = "";
    if (!auth_random_hex(tok, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
    auth_sha_hex(tok, sha);
    const char *v[4] = {p->account, cur, sat, sha};
    if (!one(db, "SELECT auth.wallet_limit_raise($1, $2::bigint, $3::bigint, $4)", 4, v, at, sizeof at) || !at[0]) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    long long at_ms = atoll(at);
    char d[140]; snprintf(d, sizeof d, "{\"from\":%lld,\"to\":%lld,\"how\":\"raise\",\"wait_hours\":24}", f.in_force, want);
    auth_audit(db, p->account, p->session, "wallet.limit", "ok", d);
    char when[40], words[400], url[300];
    utc(at_ms, when, sizeof when);
    snprintf(words, sizeof words, "Someone asked to raise your wallet's daily limit from %s to %s sats. If nothing stops it, the new limit counts after %s. "
             "Any login of your account can cancel it, and so does the link below.", was, to, when);
    snprintf(url, sizeof url, "%s/settings/wallet#cancel=%s", auth_origin(), tok);
    auth_notice_link(db, p->account, "wallet.limit.raise", "someone asked to raise your wallet's daily limit", words, "Cancel the raise:", url);
    memset(tok, 0, sizeof tok); memset(url, 0, sizeof url);
    char json[400], lines[300];
    snprintf(json, sizeof json, "{\"state\":\"waiting\",\"daily_sat\":%lld,\"raise\":{\"sat\":%lld,\"at_ms\":%lld},\"wait_hours\":24,\"words\":\"%s\"}", f.in_force, want, at_ms, w);
    snprintf(lines, sizeof lines, "state waiting\ndaily_sat %lld\nraise_sat %lld\nraise_at_ms %lld\nwords %s\n", f.in_force, want, at_ms, w);
    auth_answer(q, 200, NULL, json, lines);
    return;
  }
  if (verdict == WP_SAME) {
    char json[300], lines[200];
    snprintf(json, sizeof json, "{\"state\":\"same\",\"daily_sat\":%lld,\"words\":\"%s\"}", f.in_force, w);
    snprintf(lines, sizeof lines, "state same\ndaily_sat %lld\nwords %s\n", f.in_force, w);
    auth_answer(q, 200, NULL, json, lines);
    return;
  }
  char d[120]; snprintf(d, sizeof d, "{\"verdict\":%d,\"from\":%lld}", verdict, f.in_force);
  auth_audit(db, p->account, p->session, "wallet.limit", "refused", d);
  refuse_wp(q, db, p, verdict);
}

static void limit_cancel(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char tok[80] = "", sha[65], acct[40] = "";
  auth_json_str(body, strlen(body), "t", tok, sizeof tok);
  bool by_link = false, mine = p && p->kind == AP_PERSON && p->account[0];
  if (strlen(tok) == 64) {
    auth_sha_hex(tok, sha);
    const char *v[1] = {sha};
    one(db, "SELECT auth.wallet_limit_by_link($1)", 1, v, acct, sizeof acct);
    by_link = acct[0] != 0;
  }
  if (!by_link && mine) snprintf(acct, sizeof acct, "%s", p->account);
  limit_facts f = {0};
  if (acct[0] && !limit_of(db, acct, &f)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  int verdict = (int)wp_cancel(f.waiting, mine, by_link);
  char sat[24] = "";
  if (verdict == WP_OK) {
    const char *v[1] = {acct};
    one(db, "SELECT auth.wallet_limit_cancel($1)", 1, v, sat, sizeof sat);
    if (!sat[0]) verdict = WP_NO_RAISE;
  }
  if (verdict != WP_OK) { char w[200]; words_of(wp_words(verdict), w, sizeof w); auth_refuse(q, (int)wp_status(verdict), w); return; }
  char d[100]; snprintf(d, sizeof d, "{\"how\":\"cancel\",\"via\":\"%s\",\"raise\":%s}", by_link ? "link" : "login", sat);
  auth_audit(db, acct, by_link ? NULL : p->session, "wallet.limit.cancel", "ok", d);
  char to[32], was[32], words[300];
  sats_text(atoll(sat), to, sizeof to); sats_text(f.in_force, was, sizeof was);
  snprintf(words, sizeof words, "The raise of your wallet's daily limit to %s sats was cancelled (%s). Your limit is still %s sats.", to,
           by_link ? "from the link in a notice" : "from a login", was);
  auth_notice(db, acct, "wallet.limit.cancel", "the raise of your wallet's daily limit was cancelled", words);
  char json[160], lines[120];
  snprintf(json, sizeof json, "{\"state\":\"cancelled\",\"daily_sat\":%lld}", f.in_force);
  snprintf(lines, sizeof lines, "state cancelled\ndaily_sat %lld\n", f.in_force);
  auth_answer(q, 200, NULL, json, lines);
}

// ---------------------------------------------------------------- a payment the app is about to make
static bool payment_id_ok(const char *id) {
  size_t n = strlen(id);
  if (n < 8 || n > 64) return false;
  for (size_t i = 0; i < n; i++) { char c = id[i]; if (!((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '-')) return false; }
  return true;
}
static void spend(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char pid[80] = "", nd[8] = "";
  auth_json_str(body, strlen(body), "payment_id", pid, sizeof pid);
  auth_json_str(body, strlen(body), "new_destination", nd, sizeof nd);
  long long sats = number(body, "sats");
  if (!payment_id_ok(pid)) { auth_refuse(q, 400, "Name the payment: 8 to 64 letters, digits, - or _."); return; }
  limit_facts f; long long spent = 0, seen = 0;
  if (!limit_of(db, p->account, &f) || !spent_of(db, p->account, pid, &spent, &seen)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  bool new_destination = !strcmp(nd, "1") || !strcmp(nd, "true");
  int verdict = (int)wp_spend(sats, spent, f.in_force, new_destination, p->l2_age_ms, p->limited, seen);
  if (wp_counted(verdict, seen)) {
    char s[24]; snprintf(s, sizeof s, "%lld", sats);
    const char *v[3] = {p->account, pid, s};
    char added[8] = "";
    one(db, "SELECT auth.wallet_spend_add($1, $2, $3::bigint)", 3, v, added, sizeof added);
    if (strcmp(added, "t")) { auth_refuse(q, 409, "That payment is being asked about twice. Try again."); return; }
    spent += sats;
    if (spent > f.in_force || (new_destination && sats > wp_new_destination())) {   // past the limit, with a fresh L2: on the record
      char d[140]; snprintf(d, sizeof d, "{\"sats\":%lld,\"limit\":%lld,\"spent\":%lld,\"how\":\"stepped-up\"}", sats, f.in_force, spent);
      auth_audit(db, p->account, p->session, "wallet.spend", "ok", d);
    }
  }
  if (verdict != WP_OK) {
    char d[140]; snprintf(d, sizeof d, "{\"verdict\":%d,\"sats\":%lld,\"limit\":%lld,\"spent\":%lld}", verdict, sats, f.in_force, spent);
    auth_audit(db, p->account, p->session, "wallet.spend", "refused", d);
    refuse_wp(q, db, p, verdict);
    return;
  }
  char json[240], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"ok\",\"sats\":%lld,\"daily_sat\":%lld,\"spent_sat\":%lld}", sats, f.in_force, spent);
  snprintf(lines, sizeof lines, "state ok\nsats %lld\ndaily_sat %lld\nspent_sat %lld\n", sats, f.in_force, spent);
  auth_answer(q, 200, NULL, json, lines);
}
static void spend_void(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char pid[80] = "", done[8] = "";
  auth_json_str(body, strlen(body), "payment_id", pid, sizeof pid);
  if (!payment_id_ok(pid)) { auth_refuse(q, 400, "Name the payment: 8 to 64 letters, digits, - or _."); return; }
  const char *v[2] = {p->account, pid};
  one(db, "SELECT auth.wallet_spend_void($1, $2)", 2, v, done, sizeof done);
  bool voided = !strcmp(done, "t");
  auth_answer(q, 200, NULL, voided ? "{\"state\":\"voided\"}" : "{\"state\":\"nothing\"}", voided ? "state voided\n" : "state nothing\n");
}

// ---------------------------------------------------------------- dispatch
bool wallet_route(const request *q) {
  if (strncmp(q->path, "/api/wallet", 11) || (q->path[11] && q->path[11] != '/')) return false;
  const char *op = q->path[11] ? q->path + 12 : "";
  PGconn *db = api_db();
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  const auth_principal *p = auth_principal_of(q);
  bool get = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD"), post = !strcmp(q->method, "POST");
  bool cancel = !strcmp(op, "limit/cancel");
  // The gate let only a person through, but for the cancel link, which anyone holding it may use.
  if (!cancel && (!p || p->kind != AP_PERSON || !p->account[0])) { auth_refuse(q, 401, "Log in first."); return true; }
  if (post && !auth_same_site(q)) { auth_refuse(q, 403, "Not from this site."); return true; }
  char *body = malloc(q->body_len + 1);
  if (!body) { auth_refuse(q, 500, "Out of memory."); return true; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  if (!*op && get) summary(q, db, p);
  else if (!strcmp(op, "backup/export") && post) backup_get(q, db, p);
  else if (!strcmp(op, "backup") && post) backup_put(q, db, p, body);
  else if (!strcmp(op, "backup/delete") && post) backup_delete(q, db, p);
  else if ((!strcmp(op, "limit") || !strcmp(op, "limit/lower")) && post) limit_set(q, db, p, body);
  else if (cancel && post) limit_cancel(q, db, p, body);
  else if (!strcmp(op, "spend") && post) spend(q, db, p, body);
  else if (!strcmp(op, "spend/void") && post) spend_void(q, db, p, body);
  else auth_refuse(q, 404, "Not found.");
  free(body);
  return true;
}
