// Passkeys (WebAuthn) on our own API (docs/auth/identity-and-methods.md#passkeys-webauthn, #13215).
// A passkey is an L2 way in: it logs in, steps a session up and is added like a key. The checks of
// a browser's answer are coax/lib/crypto/webauthn.coax (ES256, EdDSA, RS256; attestation "none"),
// through passkey-verify.coax; what follows is passkey-policy.coax's (#authority); whether this
// session may add a way in at all is auth-recover-policy.coax's ar_link, the rule a key follows.
//
//   POST /api/auth/passkey/login/options {client?}       a login challenge (5 minutes, once). The web's
//                                         is bound to this browser (__Host-oa_passkey); an app's to its holder
//   POST /api/auth/passkey/login {challenge, id, auth_data, client_data, signature, user_handle?}
//                                         a session at L2 (cookie, or the app's token pair)
//   POST /api/auth/passkey/register/options              a challenge to add a passkey, bound to this session
//   POST /api/auth/passkey/register {challenge, attestation, client_data, transports?, prf?, name?}
//   POST /api/auth/passkey/step-up/options               a challenge for this account's passkeys
//   POST /api/auth/passkey/step-up {challenge, id, auth_data, client_data, signature}   this session to L2
//   GET  /api/auth/passkeys               the account's passkeys (names, when added and used, synced) and the offer
//   POST /api/auth/passkeys/<id>/rename {name}
//   POST /api/auth/passkeys/offered       the one offer after an email login was shown
// Removing one is POST /api/auth/unlink {kind: passkey, id} (auth-recover.c, L2-fresh).
//
// Every binary field is base64url without padding, as WebAuthn's JSON has it. Options answer both
// our fields (challenge, rp_id, user_id, allow, exclude) and the standard `publicKey` object, so a
// browser, the iOS app and Android's Credential Manager each take what they read.
//
//   RP ID      the host of OA_API_ORIGIN (openagents.com; staging.openagents.com), or OA_PASSKEY_RP_ID
//   origins    OA_API_ORIGIN (the web, and the iOS and Mac apps, which WebAuthn gives the RP's own
//              origin), plus OA_PASSKEY_ORIGINS: Android apps' "android:apk-key-hash:..." origins
//
// User verification is required on every ceremony. Every step is in auth.audit; adding a passkey,
// and a sign count that went backwards (a copied passkey), are told to every channel of the account.
// Nothing secret is logged: no challenge, key, handle or signature.
#include "auth-internal.h"
#include "auth-recover-policy.generated.h"
#include "passkey-policy.generated.h"
#include "passkey-verify.generated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { PK_LOGIN = 1, PK_REGISTER = 2, PK_STEPUP = 3 };
enum { PK_OK = 0, PK_NO_CHALLENGE = 1, PK_EXPIRED = 2, PK_ELSEWHERE = 3, PK_REFUSED = 4, PK_UNKNOWN = 5, PK_CLONE = 6,
       PK_NOT_YOURS = 7, PK_TOO_MANY = 8, PK_NAME = 9, PK_HANDLE = 10 };
enum { WA_OK = 0, WA_ORIGIN = 4 };
#define PASSKEY_COOKIE "__Host-oa_passkey"


// The numbers this file shares with passkey-policy.coax, checked at start (auth_init).
bool passkeys_init(void) {
  return pk_uv() && pk_challenge(true, PK_LOGIN, PK_LOGIN, 10, 10, true, false, false, false) == PK_OK
      && pk_challenge(false, PK_LOGIN, PK_LOGIN, 10, 10, true, false, false, false) == PK_NO_CHALLENGE
      && pk_challenge(true, PK_LOGIN, PK_LOGIN, 400000, 10, true, false, false, false) == PK_EXPIRED
      && pk_challenge(true, PK_STEPUP, PK_STEPUP, 10, 10, false, true, false, true) == PK_ELSEWHERE
      && pk_assertion(14, 3, 3) == PK_CLONE && pk_assertion(13, 0, 1) == PK_REFUSED && pk_add(20) == PK_TOO_MANY
      && pk_status(PK_NOT_YOURS) == 403 && pk_status(PK_NAME) == 400 && pk_status(PK_HANDLE) == 401;
}

// ---------------------------------------------------------------- small pieces
static void words_of(coax_string w, char *out, size_t cap) { snprintf(out, cap, "%.*s", (int)w.count, w.data); }
static void refuse_pk(const request *q, int verdict) {
  char w[240]; words_of(pk_words(verdict), w, sizeof w);
  auth_refuse(q, (int)pk_status(verdict), w);
}
static bool one(PGconn *db, const char *sql, int n, const char *const *v, char *out, size_t cap) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) >= 1;
  if (out && cap) { out[0] = 0; if (ok && !PQgetisnull(r, 0, 0)) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0)); }
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("passkeys: %.60s: %s", sql, PQerrorMessage(db));
  PQclear(r);
  return ok;
}
// The RP ID: OA_PASSKEY_RP_ID, else the host of this site's origin.
static void rp_id(char out[256]) {
  const char *o = getenv("OA_PASSKEY_RP_ID");
  if (o && *o) { snprintf(out, 256, "%s", o); return; }
  const char *s = auth_origin(), *h = strstr(s, "://");
  h = h ? h + 3 : s;
  size_t n = strcspn(h, ":/");
  snprintf(out, 256, "%.*s", (int)n, h);
}
static size_t b64url(const unsigned char *b, size_t n, char *out, size_t cap) {
  if (cap < (4 * n + 2) / 3 + 1) { out[0] = 0; return 0; }
  int64_t k = pv_b64url((coax_v_u8){(uint8_t *)b, (int64_t)n}, (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  if (k < 0) k = 0;
  out[k] = 0;
  return (size_t)k;
}
// A base64url field of the body into out: its byte count, or -1 when missing or not base64url.
static long field(const char *body, const char *key, unsigned char *out, size_t cap) {
  size_t text_cap = cap * 4 / 3 + 8;
  char *text = malloc(text_cap);
  if (!text) return -1;
  long n = -1;
  if (auth_json_str(body, strlen(body), key, text, text_cap) && text[0])
    n = (long)pv_unb64url((coax_string){text, (int64_t)strlen(text)}, (coax_v_u8){out, (int64_t)cap});
  free(text);
  return n;
}
static int purpose_of(const char *p) { return !strcmp(p, "login") ? PK_LOGIN : !strcmp(p, "link") ? PK_REGISTER : !strcmp(p, "stepup") ? PK_STEPUP : 0; }
static const char *purpose_row(int purpose) { return purpose == PK_LOGIN ? "login" : purpose == PK_REGISTER ? "link" : "stepup"; }
static const char *purpose_action(int purpose) { return purpose == PK_LOGIN ? "login.passkey" : purpose == PK_REGISTER ? "passkey.add" : "stepup"; }

// ---------------------------------------------------------------- challenges
// A new challenge for purpose: the row (its SHA-256 only) and the answer with the options.
static void options(const request *q, PGconn *db, const auth_principal *p, int purpose, const char *body) {
  char client[16] = "web", rp[256];
  auth_json_str(body, strlen(body), "client", client, sizeof client);
  bool app = !strcmp(client, "app");
  rp_id(rp);
  unsigned char ch[32];
  char id[33], chb[64], chhex[65], chsha[65], browser[65] = "", browser_sha[65] = "", headers[300] = "";
  if (!auth_random(ch, 32) || !auth_random_hex(id, 16)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  b64url(ch, 32, chb, sizeof chb);
  auth_hex(ch, 32, chhex); auth_sha_hex(chhex, chsha);
  if (purpose == PK_LOGIN && !app) {
    // The browser that asked: only its cookie's digest is kept.
    if (!auth_random_hex(browser, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
    auth_sha_hex(browser, browser_sha);
    snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=300; HttpOnly; Secure; SameSite=Strict\r\n", PASSKEY_COOKIE, browser);
  }
  const char *acct = purpose == PK_LOGIN ? NULL : p->account, *sid = purpose == PK_LOGIN ? NULL : p->session;
  const char *v[7] = {id, chsha, purpose_row(purpose), app ? "app" : "web", acct, sid, browser_sha[0] ? browser_sha : NULL};
  if (!one(db, "SELECT auth.passkey_challenge($1, $2, $3, $4, $5, $6, $7)", 7, v, NULL, 0)) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  memset(browser, 0, sizeof browser);
  // The credentials to list: the account's (step-up: allow; adding: exclude, so a manager that has
  // one says so rather than making a second).
  char *creds = calloc(1, 24000), *lines_creds = calloc(1, 24000);
  if (!creds || !lines_creds) { free(creds); free(lines_creds); auth_refuse(q, 500, "Out of memory."); return; }
  size_t c = 0, lc = 0;
  char handle[64] = "", user_name[320] = "OpenAgents account";
  if (purpose != PK_LOGIN) {
    const char *a[1] = {p->account};
    PGresult *r = PQexecParams(db, "SELECT credential_id, coalesce(array_to_string(transports, ','), '') FROM auth.passkeys WHERE account = $1 ORDER BY id", 1, NULL, a, NULL, NULL, 0);
    for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r) && c + 1600 < 24000; i++) {
      c += (size_t)snprintf(creds + c, 24000 - c, "%s{\"type\":\"public-key\",\"id\":\"%s\"}", i ? "," : "", PQgetvalue(r, i, 0));
      lc += (size_t)snprintf(lines_creds + lc, 24000 - lc, "%s %s\n", purpose == PK_STEPUP ? "allow" : "exclude", PQgetvalue(r, i, 0));
    }
    PQclear(r);
  }
  if (purpose == PK_REGISTER) {
    // The account's user handle: its passkeys share one, 32 random bytes, never the account id.
    const char *a[1] = {p->account};
    one(db, "SELECT auth.passkey_handle($1)", 1, a, handle, sizeof handle);
    if (!handle[0]) { unsigned char h[32]; if (!auth_random(h, 32)) { free(creds); free(lines_creds); auth_refuse(q, 503, "No randomness here. Try again."); return; } b64url(h, 32, handle, sizeof handle); }
    // What the person's password manager shows: their email, when the account has one.
    char sealed[1200] = "", ad[96], email[300];
    one(db, "SELECT sealed FROM auth.emails WHERE account = $1 ORDER BY is_primary DESC, id LIMIT 1", 1, a, sealed, sizeof sealed);
    snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
    if (sealed[0] && auth_unseal(ad, sealed, email, sizeof email)) { auth_json_esc(email, user_name, sizeof user_name); memset(email, 0, sizeof email); }
  }
  // The answer: ours, and the standard options under publicKey.
  size_t cap = 60000; char *json = malloc(cap), *lines = malloc(30000);
  if (!json || !lines) { free(json); free(lines); free(creds); free(lines_creds); auth_refuse(q, 500, "Out of memory."); return; }
  size_t j = (size_t)snprintf(json, cap, "{\"challenge\":\"%s\",\"rp_id\":\"%s\",\"timeout\":300000,\"purpose\":\"%s\"", chb, rp, purpose_row(purpose));
  size_t l = (size_t)snprintf(lines, 30000, "challenge %s\nrp_id %s\ntimeout 300000\n", chb, rp);
  if (purpose == PK_REGISTER) {
    j += (size_t)snprintf(json + j, cap - j, ",\"user_id\":\"%s\",\"user_name\":\"%s\",\"publicKey\":{\"challenge\":\"%s\",\"rp\":{\"id\":\"%s\",\"name\":\"OpenAgents\"},"
                          "\"user\":{\"id\":\"%s\",\"name\":\"%s\",\"displayName\":\"%s\"},"
                          "\"pubKeyCredParams\":[{\"type\":\"public-key\",\"alg\":-7},{\"type\":\"public-key\",\"alg\":-8},{\"type\":\"public-key\",\"alg\":-257}],"
                          "\"timeout\":300000,\"attestation\":\"none\",\"excludeCredentials\":[%s],"
                          "\"authenticatorSelection\":{\"residentKey\":\"required\",\"requireResidentKey\":true,\"userVerification\":\"required\"},"
                          "\"extensions\":{\"credProps\":true,\"prf\":{}}}}",
                          handle, user_name, chb, rp, handle, user_name, user_name, creds);
    l += (size_t)snprintf(lines + l, 30000 - l, "user_id %s\nuser_name %s\n%s", handle, user_name, lines_creds);
  } else {
    j += (size_t)snprintf(json + j, cap - j, ",\"publicKey\":{\"challenge\":\"%s\",\"rpId\":\"%s\",\"timeout\":300000,\"userVerification\":\"required\",\"allowCredentials\":[%s]}}",
                          chb, rp, purpose == PK_STEPUP ? creds : "");
    l += (size_t)snprintf(lines + l, 30000 - l, "%s", lines_creds);
  }
  char d[80]; snprintf(d, sizeof d, "{\"purpose\":\"%s\",\"client\":\"%s\"}", purpose_row(purpose), app ? "app" : "web");
  auth_audit(db, acct, sid, "passkey.challenge", "ok", d);
  auth_answer(q, 200, headers[0] ? headers : NULL, json, lines);
  free(json); free(lines); free(creds); free(lines_creds);
}

// Uses the body's challenge for purpose; answers the refusal and returns false when it isn't good
// here. app: the challenge was an app's.
static bool spend(const request *q, PGconn *db, const auth_principal *p, int purpose, const char *body, unsigned char ch[32], bool *app) {
  unsigned char raw[40];
  long n = field(body, "challenge", raw, sizeof raw);
  *app = false;
  if (n != 32) { refuse_pk(q, PK_NO_CHALLENGE); return false; }
  memcpy(ch, raw, 32);
  char chhex[65], chsha[65];
  auth_hex(ch, 32, chhex); auth_sha_hex(chhex, chsha);
  const char *v[1] = {chsha};
  PGresult *r = PQexecParams(db, "SELECT purpose, client, coalesce(account, ''), coalesce(session, ''), coalesce(browser_sha256, ''), made_ms, now_ms "
                                 "FROM auth.passkey_spend($1)", 1, NULL, v, NULL, NULL, 0);
  bool found = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  int row = 0; long long made = 0, now = 0; bool browser = false, account = false, session = false;
  if (found) {
    row = purpose_of(PQgetvalue(r, 0, 0));
    *app = !strcmp(PQgetvalue(r, 0, 1), "app");
    made = atoll(PQgetvalue(r, 0, 5)); now = atoll(PQgetvalue(r, 0, 6));
    char cookie[80] = "", csha[65] = "";
    auth_cookie(q, PASSKEY_COOKIE, cookie, sizeof cookie);
    if (cookie[0]) auth_sha_hex(cookie, csha);
    const char *bs = PQgetvalue(r, 0, 4);
    browser = cookie[0] && bs[0] && !strcmp(csha, bs);
    account = p && p->account[0] && !strcmp(PQgetvalue(r, 0, 2), p->account);
    session = p && p->session[0] && !strcmp(PQgetvalue(r, 0, 3), p->session);
  }
  PQclear(r);
  int verdict = (int)pk_challenge(found, row, purpose, now, made, *app, browser, account, session);
  if (verdict != PK_OK) {
    char d[120]; snprintf(d, sizeof d, "{\"why\":\"challenge\",\"verdict\":%d}", verdict);
    auth_audit(db, p && p->account[0] ? p->account : NULL, p && p->session[0] ? p->session : NULL, purpose_action(purpose), "refused", d);
    refuse_pk(q, verdict);
    return false;
  }
  return true;
}

// webauthn.coax against every origin we take: this site's first, then Android apps'.
static int origins(char list[8][300]) {
  int n = 0;
  snprintf(list[n++], 300, "%s", auth_origin());
  const char *s = getenv("OA_PASSKEY_ORIGINS");
  while (s && *s && n < 8) {
    size_t k = strcspn(s, ",");
    if (k > 0 && k < 300) snprintf(list[n++], 300, "%.*s", (int)k, s);
    s += k; if (*s == ',') s++;
  }
  return n;
}

// ---------------------------------------------------------------- adding a passkey
static void do_register(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  if (!p->session[0]) { auth_refuse(q, 400, "Log in with a session to add a passkey."); return; }
  unsigned char ch[32]; bool app = false;
  if (!spend(q, db, p, PK_REGISTER, body, ch, &app)) return;
  unsigned char *att = malloc(8192), *cd = malloc(4200);
  if (!att || !cd) { free(att); free(cd); auth_refuse(q, 500, "Out of memory."); return; }
  long na = field(body, "attestation", att, 8192), nc = field(body, "client_data", cd, 4200);
  char rp[256], list[8][300]; rp_id(rp);
  int no = origins(list);
  coax_t_WaRegistration reg = {0};
  reg.error = 1;
  if (na > 0 && nc > 0) {
    for (int i = 0; i < no; i++) {
      reg = pv_register((coax_v_u8){att, na}, (coax_v_u8){cd, nc}, (coax_v_u8){ch, 32}, (coax_string){list[i], (int64_t)strlen(list[i])},
                        (coax_string){rp, (int64_t)strlen(rp)}, pk_uv());
      if (reg.error != WA_ORIGIN) break;
    }
  }
  if (reg.error != WA_OK) {
    char d[120]; snprintf(d, sizeof d, "{\"why\":\"webauthn\",\"code\":%lld}", (long long)reg.error);
    auth_audit(db, p->account, p->session, "passkey.add", "refused", d);
    free(att); free(cd);
    refuse_pk(q, PK_REFUSED);
    return;
  }
  char cred[1500], cose[1300], aaguid[33], salt[65], handle[64] = "";
  b64url(att + reg.id_at, (size_t)reg.id_count, cred, sizeof cred);
  if (reg.key_count * 2 + 1 > (long long)sizeof cose) { free(att); free(cd); refuse_pk(q, PK_REFUSED); return; }
  auth_hex(att + reg.key_at, (size_t)reg.key_count, cose);
  auth_hex(reg.aaguid.e, 16, aaguid);
  free(att); free(cd);
  // May this session add a way in (the rule a key follows), and does the account have room?
  const char *who[2] = {cred, p->account};
  char owner[8] = "0", have[16] = "0";
  one(db, "SELECT auth.passkey_owner($1, $2)", 2, who, owner, sizeof owner);
  int o = atoi(owner);
  int verdict = (int)ar_link(p->level, p->strong, o == 1 ? AR_OWNER_THIS : o == 2 ? AR_OWNER_OTHER : AR_OWNER_NONE);
  if (verdict != AR_OK) {
    char d[120]; snprintf(d, sizeof d, "{\"why\":\"%s\"}", verdict == AR_OTHER ? "another account's" : verdict == AR_ALREADY ? "already" : "step up");
    auth_audit(db, p->account, p->session, "passkey.add", "refused", d);
    char w[200]; words_of(ar_words(verdict), w, sizeof w);
    auth_refuse(q, (int)ar_status(verdict), w);
    return;
  }
  const char *a[1] = {p->account};
  one(db, "SELECT count(*) FROM auth.passkeys WHERE account = $1", 1, a, have, sizeof have);
  if (pk_add(atoi(have)) != PK_OK) { auth_audit(db, p->account, p->session, "passkey.add", "refused", "{\"why\":\"too many\"}"); refuse_pk(q, PK_TOO_MANY); return; }
  one(db, "SELECT auth.passkey_handle($1)", 1, a, handle, sizeof handle);
  if (!handle[0]) {
    // The first passkey: the handle the browser was given. The client data names no handle, so the
    // body says it; it is only ever this account's, and is checked at every login.
    char h[80] = "";
    auth_json_str(body, strlen(body), "user_id", h, sizeof h);
    unsigned char hb[40];
    if (pv_unb64url((coax_string){h, (int64_t)strlen(h)}, (coax_v_u8){hb, sizeof hb}) != 32) { auth_refuse(q, 400, "Say the passkey's user id."); return; }
    snprintf(handle, sizeof handle, "%s", h);
  }
  // Its name: the body's (checked), else its password manager's (from the AAGUID), else this device's.
  char name[200] = "", typed[200] = "";
  if (auth_json_str(body, strlen(body), "name", typed, sizeof typed) && typed[0]) {
    if (!pk_name_ok((coax_string){typed, (int64_t)strlen(typed)})) { refuse_pk(q, PK_NAME); return; }
    snprintf(name, sizeof name, "%s", typed);
  } else {
    words_of(pk_aaguid_name((coax_string){aaguid, 32}), name, sizeof name);
    if (!name[0]) auth_device_name(q, name, sizeof name);
  }
  unsigned char sb[32];
  if (!auth_random(sb, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  auth_hex(sb, 32, salt);
  char transports[200] = "", tarr[240], alg[16], count[24];
  auth_json_str(body, strlen(body), "transports", transports, sizeof transports);
  // {internal,hybrid}: only the names WebAuthn has
  size_t t = (size_t)snprintf(tarr, sizeof tarr, "{");
  for (char *s = strtok(transports, ","); s; s = strtok(NULL, ","))
    if (!strcmp(s, "internal") || !strcmp(s, "hybrid") || !strcmp(s, "usb") || !strcmp(s, "nfc") || !strcmp(s, "ble") || !strcmp(s, "smart-card"))
      t += (size_t)snprintf(tarr + t, sizeof tarr - t, "%s%s", t > 1 ? "," : "", s);
  snprintf(tarr + t, sizeof tarr - t, "}");
  char prfv[8] = "";
  auth_json_str(body, strlen(body), "prf", prfv, sizeof prfv);
  snprintf(alg, sizeof alg, "%lld", (long long)reg.alg);
  snprintf(count, sizeof count, "%u", reg.count);
  bool be = reg.flags & 0x08, bs = reg.flags & 0x10;
  const char *v[14] = {p->account, cred, handle, cose, alg, count, be ? "t" : "f", bs ? "t" : "f", aaguid, tarr, name, salt, !strcmp(prfv, "1") ? "t" : "f", rp};
  char pid[24] = "";
  if (!one(db, "SELECT auth.passkey_add($1, $2, $3, $4, $5::int, $6::bigint, $7::boolean, $8::boolean, $9, $10::text[], $11, $12, $13::boolean, $14)", 14, v, pid, sizeof pid) || !pid[0]) {
    auth_audit(db, p->account, p->session, "passkey.add", "refused", "{\"why\":\"not stored\"}");
    auth_refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  one(db, "SELECT auth.passkey_offered($1)", 1, a, NULL, 0);
  char d[160]; snprintf(d, sizeof d, "{\"id\":%s,\"alg\":%s,\"synced\":%s,\"client\":\"%s\"}", pid, alg, bs ? "true" : "false", app ? "app" : "web");
  auth_audit(db, p->account, p->session, "passkey.add", "ok", d);
  char words[320], esc[200];
  snprintf(words, sizeof words, "A passkey (%s) was added to your OpenAgents account. It can now log in and change the account.", name);
  auth_notice(db, p->account, "link.passkey", "a passkey was added", words);
  auth_json_esc(name, esc, sizeof esc);
  char json[400], lines[300];
  snprintf(json, sizeof json, "{\"state\":\"added\",\"id\":%s,\"name\":\"%s\",\"synced\":%s}", pid, esc, bs ? "true" : "false");
  snprintf(lines, sizeof lines, "state added\nid %s\nname %s\nsynced %d\n", pid, name, bs ? 1 : 0);
  auth_answer(q, 200, NULL, json, lines);
}

// ---------------------------------------------------------------- logging in and stepping up
static void do_assert(const request *q, PGconn *db, const auth_principal *p, int purpose, const char *body) {
  if (purpose == PK_LOGIN) {
    char ipk[65]; auth_visitor_key(q, ipk);
    if (!auth_limit_hit(q, db, AUTH_LIMIT_KEY_LOGIN, ipk)) return;
  } else if (!p || !p->session[0]) { auth_refuse(q, 400, "Step up a login session."); return; }
  unsigned char ch[32]; bool app = false;
  if (!spend(q, db, p, purpose, body, ch, &app)) return;
  const char *action = purpose_action(purpose);
  char cred[1500] = "", rp[256];
  rp_id(rp);
  auth_json_str(body, strlen(body), "id", cred, sizeof cred);
  unsigned char idb[1100];
  if (!cred[0] || pv_unb64url((coax_string){cred, (int64_t)strlen(cred)}, (coax_v_u8){idb, sizeof idb}) < 1) { refuse_pk(q, PK_UNKNOWN); return; }
  const char *v[2] = {cred, rp};
  PGresult *r = PQexecParams(db, "SELECT id, account, user_handle, cose_hex, sign_count, backup_eligible, frozen FROM auth.passkey_find($1, $2)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    auth_audit(db, p && p->account[0] ? p->account : NULL, NULL, action, "refused", "{\"why\":\"unknown passkey\"}");
    refuse_pk(q, PK_UNKNOWN);
    return;
  }
  char pid[24], acct[40], handle[64], cose_hex[1300];
  snprintf(pid, sizeof pid, "%s", PQgetvalue(r, 0, 0)); snprintf(acct, sizeof acct, "%s", PQgetvalue(r, 0, 1));
  snprintf(handle, sizeof handle, "%s", PQgetvalue(r, 0, 2)); snprintf(cose_hex, sizeof cose_hex, "%s", PQgetvalue(r, 0, 3));
  long long stored = atoll(PQgetvalue(r, 0, 4));
  bool be = !strcmp(PQgetvalue(r, 0, 5), "t");
  PQclear(r);
  // Stepping up: only this account's passkeys.
  if (purpose == PK_STEPUP && strcmp(acct, p->account)) {
    auth_audit(db, p->account, p->session, action, "refused", "{\"via\":\"passkey\",\"why\":\"not this account's passkey\"}");
    refuse_pk(q, PK_NOT_YOURS);
    return;
  }
  // The handle the authenticator returned, when it returned one, is the row's.
  char uh[80] = "";
  if (auth_json_str(body, strlen(body), "user_handle", uh, sizeof uh) && uh[0] && strcmp(uh, handle)) {
    auth_audit(db, acct, NULL, action, "refused", "{\"why\":\"user handle\"}");
    refuse_pk(q, PK_HANDLE);
    return;
  }
  unsigned char *ad = malloc(1100), *cd = malloc(4200), *sig = malloc(1100), key[650];
  if (!ad || !cd || !sig) { free(ad); free(cd); free(sig); auth_refuse(q, 500, "Out of memory."); return; }
  long nd = field(body, "auth_data", ad, 1100), nc = field(body, "client_data", cd, 4200), ns = field(body, "signature", sig, 1100);
  size_t nk = strlen(cose_hex) / 2;
  bool key_ok = nk <= sizeof key && auth_unhex(cose_hex, key, nk);
  char list[8][300]; int no = origins(list);
  coax_t_WaAssertion got = {0};
  got.error = 1;
  if (nd > 0 && nc > 0 && ns > 0 && key_ok) {
    for (int i = 0; i < no; i++) {
      got = pv_assert((coax_v_u8){ad, nd}, (coax_v_u8){cd, nc}, (coax_v_u8){sig, ns}, (coax_v_u8){key, (int64_t)nk}, (coax_v_u8){ch, 32},
                      (coax_string){list[i], (int64_t)strlen(list[i])}, (coax_string){rp, (int64_t)strlen(rp)}, pk_uv(), (uint32_t)stored, be);
      if (got.error != WA_ORIGIN) break;
    }
  }
  free(ad); free(cd); free(sig);
  int verdict = (int)pk_assertion(got.error, stored, (long long)got.count);
  if (verdict == PK_CLONE) {
    // A copied passkey: the login is refused, the row marked, and the account told.
    const char *c[1] = {pid};
    one(db, "SELECT auth.passkey_clone($1::bigint)", 1, c, NULL, 0);
    char d[120]; snprintf(d, sizeof d, "{\"id\":%s,\"stored\":%lld,\"got\":%u}", pid, stored, got.count);
    auth_audit(db, acct, NULL, "passkey.clone", "refused", d);
    auth_notice(db, acct, "passkey.clone", "a passkey may have been copied",
                        "A login with one of your passkeys showed a counter lower than its last login, which can mean the passkey was copied. "
                        "We refused that login. If you don't recognize it, remove that passkey in Security.");
    refuse_pk(q, PK_CLONE);
    return;
  }
  if (verdict != PK_OK) {
    char d[120]; snprintf(d, sizeof d, "{\"via\":\"passkey\",\"why\":\"webauthn\",\"code\":%lld}", (long long)got.error);
    auth_audit(db, acct, purpose == PK_STEPUP ? p->session : NULL, action, "refused", d);
    refuse_pk(q, PK_REFUSED);
    return;
  }
  char count[24]; snprintf(count, sizeof count, "%u", got.count);
  const char *u[3] = {pid, count, (got.flags & 0x10) ? "t" : "f"};
  one(db, "SELECT auth.passkey_used($1::bigint, $2::bigint, $3::boolean)", 3, u, NULL, 0);
  if (purpose == PK_STEPUP) {
    const char *s[3] = {p->session, "passkey", pid};
    char ok[8] = "";
    one(db, "SELECT auth.session_raise($1, $2, $3::bigint)", 3, s, ok, sizeof ok);
    if (strcmp(ok, "t")) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    auth_recover_strong_login(db, p->account, p->session);
    auth_audit(db, p->account, p->session, "stepup", "ok", "{\"via\":\"passkey\"}");
    auth_answer(q, 200, NULL, "{\"state\":\"stepped-up\",\"level\":\"L2\"}", "state stepped-up\nlevel L2\n");
    return;
  }
  auth_logged_in(q, db, acct, "passkey", pid, "L2", app, false, "login.passkey", NULL, NULL);
}

// ---------------------------------------------------------------- the list, names, the offer
// The account's passkeys appended to an answer: JSON objects (comma-separated) and lines
// "passkey ID ADDED_MS USED_MS SYNCED CLONE NAME" (the name last: it may hold spaces). Never a
// key, a handle or a salt. GET /api/auth/ways lists them too (auth-recover.c). The count.
int passkeys_rows(PGconn *db, const char *acct, char *json, size_t jcap, size_t *j, char *lines, size_t lcap, size_t *l) {
  const char *a[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT id, name, added_ms, used_ms, synced, clone, prf, transports FROM auth.passkeys_of($1)", 1, NULL, a, NULL, NULL, 0);
  int have = PQresultStatus(r) == PGRES_TUPLES_OK ? PQntuples(r) : 0;
  for (int i = 0; i < have && *j + 600 < jcap && *l + 300 < lcap; i++) {
    char esc[200]; auth_json_esc(PQgetvalue(r, i, 1), esc, sizeof esc);
    bool synced = !strcmp(PQgetvalue(r, i, 4), "t"), clone = !strcmp(PQgetvalue(r, i, 5), "t"), prf = !strcmp(PQgetvalue(r, i, 6), "t");
    *j += (size_t)snprintf(json + *j, jcap - *j, "%s{\"id\":%s,\"name\":\"%s\",\"added_ms\":%s,\"used_ms\":%s,\"synced\":%s,\"clone\":%s,\"prf\":%s,\"transports\":\"%s\"}",
                           i ? "," : "", PQgetvalue(r, i, 0), esc, PQgetvalue(r, i, 2), PQgetvalue(r, i, 3), synced ? "true" : "false",
                           clone ? "true" : "false", prf ? "true" : "false", PQgetvalue(r, i, 7));
    *l += (size_t)snprintf(lines + *l, lcap - *l, "passkey %s %s %s %d %d %s\n", PQgetvalue(r, i, 0), PQgetvalue(r, i, 2), PQgetvalue(r, i, 3), synced, clone, PQgetvalue(r, i, 1));
  }
  PQclear(r);
  return have;
}
static void list(const request *q, PGconn *db, const auth_principal *p) {
  size_t cap = 40000; char *json = malloc(cap), *lines = malloc(20000);
  if (!json || !lines) { free(json); free(lines); auth_refuse(q, 500, "Out of memory."); return; }
  size_t j = (size_t)snprintf(json, cap, "{\"passkeys\":["), l = 0;
  lines[0] = 0;
  int have = passkeys_rows(db, p->account, json, cap, &j, lines, 20000, &l);
  const char *a[1] = {p->account};
  char offered[8] = "";
  one(db, "SELECT passkey_offered_at IS NOT NULL FROM auth.accounts WHERE id = $1", 1, a, offered, sizeof offered);
  // A limited session (an account with a stronger way in) would be told to step up: no offer there.
  bool offer = pk_offer(have, !strcmp(offered, "t"), !strcmp(p->via, "email") && p->session[0] && !p->limited);
  snprintf(json + j, cap - j, "],\"offer\":%s}", offer ? "true" : "false");
  snprintf(lines + l, 20000 - l, "offer %d\n", offer);
  auth_answer(q, 200, NULL, json, lines);
  free(json); free(lines);
}
static void rename_one(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char idtxt[24] = "", name[200] = "";
  if (sscanf(q->path, "/api/auth/passkeys/%20[0-9]/rename", idtxt) != 1) { auth_refuse(q, 404, "Not found."); return; }
  if (!auth_json_str(body, strlen(body), "name", name, sizeof name) || !pk_name_ok((coax_string){name, (int64_t)strlen(name)})) { refuse_pk(q, PK_NAME); return; }
  const char *v[3] = {p->account, idtxt, name};
  char ok[8] = "";
  one(db, "SELECT auth.passkey_rename($1, $2::bigint, $3)", 3, v, ok, sizeof ok);
  if (strcmp(ok, "t")) { auth_refuse(q, 404, "That isn't on this account."); return; }
  char d[60]; snprintf(d, sizeof d, "{\"id\":%s}", idtxt);
  auth_audit(db, p->account, p->session, "passkey.rename", "ok", d);
  auth_answer(q, 200, NULL, "{\"state\":\"renamed\"}", "state renamed\n");
}

// ---------------------------------------------------------------- dispatch
bool passkeys_route(const request *q, PGconn *db, const char *op) {
  bool get_list = !strcmp(op, "passkeys");
  bool rename = !strncmp(op, "passkeys/", 9) && strstr(op, "/rename");
  bool offered = !strcmp(op, "passkeys/offered");
  static const char *const posts[] = {"passkey/login/options", "passkey/login", "passkey/register/options", "passkey/register",
                                      "passkey/step-up/options", "passkey/step-up", NULL};
  bool post = rename || offered;
  for (const char *const *o = posts; *o; o++) if (!strcmp(op, *o)) post = true;
  if (!get_list && !post) return false;
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  bool is_get = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD");
  if (get_list && !is_get) { auth_refuse(q, 405, "GET this."); return true; }
  if (post && strcmp(q->method, "POST")) { auth_refuse(q, 405, "POST this."); return true; }
  if (post && !auth_same_site(q)) { auth_refuse(q, 403, "Not from this site."); return true; }
  const auth_principal *p = auth_principal_of(q);
  bool login = !strncmp(op, "passkey/login", 13);
  if (!login && (!p || !p->account[0])) { auth_refuse(q, 401, "Log in first."); return true; }
  if (get_list) { list(q, db, p); return true; }
  char *body = malloc(q->body_len + 1);
  if (!body) { auth_refuse(q, 500, "Out of memory."); return true; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  if (!strcmp(op, "passkey/login/options")) options(q, db, p, PK_LOGIN, body);
  else if (!strcmp(op, "passkey/login")) do_assert(q, db, p, PK_LOGIN, body);
  else if (!strcmp(op, "passkey/register/options")) {
    // The same rule as adding a key, before a challenge is made: step up first when it says so.
    int verdict = (int)ar_link(p->level, p->strong, AR_OWNER_NONE);
    if (verdict != AR_OK) { char w[200]; words_of(ar_words(verdict), w, sizeof w); auth_refuse(q, (int)ar_status(verdict), w); }
    else options(q, db, p, PK_REGISTER, body);
  }
  else if (!strcmp(op, "passkey/register")) do_register(q, db, p, body);
  else if (!strcmp(op, "passkey/step-up/options")) {
    const char *a[1] = {p->account};
    char have[16] = "0";
    one(db, "SELECT count(*) FROM auth.passkeys WHERE account = $1", 1, a, have, sizeof have);
    if (atoi(have) == 0) auth_refuse(q, 404, "This account has no passkey yet. Confirm another way.");
    else options(q, db, p, PK_STEPUP, body);
  }
  else if (!strcmp(op, "passkey/step-up")) do_assert(q, db, p, PK_STEPUP, body);
  else if (offered) {
    const char *a[1] = {p->account};
    one(db, "SELECT auth.passkey_offered($1)", 1, a, NULL, 0);
    auth_audit(db, p->account, p->session, "passkey.offer", "ok", "{}");
    auth_answer(q, 200, NULL, "{\"state\":\"offered\"}", "state offered\n");
  }
  else rename_one(q, db, p, body);
  memset(body, 0, q->body_len);
  free(body);
  return true;
}
