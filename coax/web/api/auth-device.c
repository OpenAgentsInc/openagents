// Device keys and device-bound refresh (docs/auth/sessions-and-tokens.md "Native: refresh tokens
// bound to a device key", #13216). An app makes a P-256 key its hardware won't export (Secure
// Enclave, Android Keystore) and sends the public half with its login; that login's refresh
// tokens then move only with the key's signature. Every decision is auth-device-policy.coax
// (#authority); the proof's bytes are auth-device-crypto.coax; the rows are migration 0153.
//
// A login from an app may carry (any login route; auth.c's auth_logged_in asks here):
//   OA-Device-Key: <130 hex>        the public key, 04 || x || y
//   OA-Device-Platform: ios | mac | android
//   OA-Device-Store: enclave | strongbox | tee | software     where the app says the key lives
//   OA-Device-Name: <up to 80 printable ASCII characters>     what the devices list calls it
// With a key the session is bound to it for good, and the answer says "device_bound":true.
//
// POST /api/auth/refresh of a bound session must carry
//   OA-Device-Proof: v1 <ms> <signature hex>
// the key's ES256 signature over
//   "OA-Device-Proof-v1\nPOST\n<origin>/api/auth/refresh\n<ms>\n<SHA-256 of the refresh token, hex>"
// within 60 seconds of our clock, each proof once. A refusal changes nothing (the token stays good
// for the real device), is in the audit log as refresh.proof, and says why in "device_proof":
// none, malformed, clock (with our "now_ms" to retry on), signature, replay, removed.
//
//   GET  /api/auth/devices               the devices list: name, platform, store, last used, logins
//   POST /api/auth/devices/<id>/remove   the key is revoked and every login bound to it ends
//
// Security notices as push (docs/auth/identity-and-methods.md "Notify", #13233; migration 0177):
//   POST /api/auth/devices/push          {service: apns | apns_sandbox | fcm, token}: the device this
//                                        login is bound to gets the account's security notices
//   POST /api/auth/devices/push/off      it gets none
// Only an app login bound to a live device key keeps a target (dk_push_set). auth_device_push
// sends a notice to each of the account's devices through push.c and counts them.
//
// Nothing here logs a key, a proof or a token.
#include "api.h"
#include "auth-internal.h"
#include "auth-device-policy.generated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// auth-device-policy.coax's verdicts (DK_*), checked against it at start.
enum { DK_OK = 0, DK_UNBOUND = 1, DK_NEEDED = 2, DK_MALFORMED = 3, DK_CLOCK = 4, DK_BAD = 5, DK_REPLAY = 6,
       DK_REVOKED = 7, DK_BAD_KEY = 8, DK_BAD_PLATFORM = 9, DK_NOT_FOUND = 10, DK_NO_DEVICE = 11, DK_BAD_PUSH = 12 };
enum { DK_IOS = 1, DK_ANDROID = 3, DK_APNS = 1, DK_FCM = 3 };

// auth-device-crypto.coax's two exports (auth-device-crypto-unit.c), declared here as the
// generated header declares them: that header also names the libraries' types, which other
// generated headers in this file's reach define too.
typedef struct { int64_t ts_ms; bool well_formed; bool verified; } adc_proof;
bool adc_key_c(const char *text, size_t n, unsigned char out[65]);
adc_proof adc_check_c(const char *header, const char *method, const char *url, const char *binding, const unsigned char key[65], unsigned char digest[32]);

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static void copy_words(coax_string s, char *out, size_t cap) {
  size_t n = (size_t)s.count < cap - 1 ? (size_t)s.count : cap - 1;
  memcpy(out, s.data, n); out[n] = 0;
}

bool auth_device_init(void) {
  int64_t now = 1800000000000LL;
  return dk_refresh(true, false, true, true, now, now, true, false) == DK_OK && dk_refresh(false, false, false, false, 0, now, false, false) == DK_UNBOUND
      && dk_refresh(true, false, false, false, 0, now, false, false) == DK_NEEDED && dk_refresh(true, false, true, false, 0, now, false, false) == DK_MALFORMED
      && dk_refresh(true, false, true, true, now - 60001, now, true, false) == DK_CLOCK && dk_refresh(true, false, true, true, now, now, false, false) == DK_BAD
      && dk_refresh(true, false, true, true, now, now, true, true) == DK_REPLAY && dk_refresh(true, true, true, true, now, now, true, false) == DK_REVOKED
      && dk_register(true, true, false, 1, 1) == DK_BAD_KEY && dk_register(true, true, true, 0, 1) == DK_BAD_PLATFORM
      && dk_remove(false, false, false) == DK_NOT_FOUND && dk_status(DK_CLOCK) == 400 && dk_status(DK_NEEDED) == 401
      && dk_platform(str("ios")) == DK_IOS && dk_platform(str("android")) == DK_ANDROID && dk_push_service(str("apns")) == DK_APNS && dk_push_service(str("fcm")) == DK_FCM
      && dk_push_set(true, DK_IOS, DK_APNS, true) == DK_OK && dk_push_set(false, DK_IOS, DK_APNS, true) == DK_NO_DEVICE && dk_push_set(true, DK_IOS, DK_FCM, true) == DK_BAD_PUSH
      && dk_status(DK_NO_DEVICE) == 403 && dk_status(DK_BAD_PUSH) == 400 && dk_push_gone(DK_APNS, 410) && !dk_push_gone(DK_APNS, 200) && dk_push_sent(200);
}

// A refusal with its reason: {"error", "device_proof"} and, for a clock that is off, our time.
static void refuse_proof(const request *q, int verdict, long long now_ms) {
  char words[200], why[24], e[300], json[500], lines[400];
  copy_words(dk_words(verdict), words, sizeof words);
  copy_words(dk_why(verdict), why, sizeof why);
  auth_json_esc(words, e, sizeof e);
  if (verdict == DK_CLOCK) {
    snprintf(json, sizeof json, "{\"error\":\"%s\",\"device_proof\":\"%s\",\"now_ms\":%lld}", e, why, now_ms);
    snprintf(lines, sizeof lines, "error %s\ndevice_proof %s\nnow_ms %lld\n", words, why, now_ms);
  } else {
    snprintf(json, sizeof json, "{\"error\":\"%s\",\"device_proof\":\"%s\"}", e, why);
    snprintf(lines, sizeof lines, "error %s\ndevice_proof %s\n", words, why);
  }
  auth_answer(q, (int)dk_status(verdict), NULL, json, lines);
}

// ---------------------------------------------------------------- a login's key
// Printable ASCII only, at most 80: what the devices list shows.
static void clean_name(const char *raw, char *out, size_t cap) {
  size_t n = 0;
  for (const char *c = raw; *c && n + 1 < cap && n < 80; c++) if (*c >= 0x20 && *c < 0x7f) out[n++] = *c;
  while (n && out[n - 1] == ' ') n--;
  out[n] = 0;
}

bool auth_device_login(const request *q, PGconn *db, const char *acct, bool app, auth_device_ask *ask) {
  memset(ask, 0, sizeof *ask);
  char key[200], platform[32], store[32], name[400];
  bool has_key = api_header(q, "OA-Device-Key", key, sizeof key) != NULL && key[0];
  api_header(q, "OA-Device-Platform", platform, sizeof platform);
  api_header(q, "OA-Device-Store", store, sizeof store);
  api_header(q, "OA-Device-Name", name, sizeof name);
  bool key_ok = has_key && adc_key_c(key, strlen(key), ask->key);
  int verdict = (int)dk_register(app, has_key, key_ok, dk_platform(str(platform)), dk_store(str(store)));
  if (verdict == DK_UNBOUND) return true;
  if (verdict != DK_OK) {
    char why[24], d[80]; copy_words(dk_why(verdict), why, sizeof why);
    snprintf(d, sizeof d, "{\"why\":\"%s\"}", why);
    auth_audit(db, acct, NULL, "device.bind", "refused", d);
    api_log("auth-device: a login's device key refused: %s", why);
    refuse_proof(q, verdict, 0);
    return false;
  }
  ask->bind = true;
  snprintf(ask->platform, sizeof ask->platform, "%s", platform);
  snprintf(ask->store, sizeof ask->store, "%s", store);
  clean_name(name, ask->name, sizeof ask->name);
  return true;
}

bool auth_device_bind(PGconn *db, const char *acct, const char *sid, const auth_device_ask *ask) {
  char hex[131]; auth_hex(ask->key, 65, hex);
  const char *v[6] = {sid, acct, hex, ask->platform, ask->store, ask->name};
  PGresult *r = PQexecParams(db, "SELECT auth.session_bind($1, $2, decode($3, 'hex'), $4, $5, $6)", 6, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  char d[120];
  if (ok) snprintf(d, sizeof d, "{\"device\":%s,\"platform\":\"%s\",\"store\":\"%s\"}", PQgetvalue(r, 0, 0), ask->platform, ask->store);
  else { api_log("auth-device: bind: %s", PQerrorMessage(db)); snprintf(d, sizeof d, "{\"why\":\"bind\"}"); }
  PQclear(r);
  auth_audit(db, acct, sid, "device.bind", ok ? "ok" : "failed", d);
  return ok;
}

// ---------------------------------------------------------------- a refresh's proof
bool auth_device_look(PGconn *db, const char *rt_sha, auth_refresh_facts *f) {
  memset(f, 0, sizeof *f);
  const char *v[1] = {rt_sha};
  PGresult *r = PQexecParams(db, "SELECT family, session, account, coalesce(device::text, ''), public_key, coalesce(revoked, false), now_ms "
                                 "FROM auth.refresh_look($1)", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    f->found = true;
    snprintf(f->family, sizeof f->family, "%s", PQgetvalue(r, 0, 0));
    snprintf(f->session, sizeof f->session, "%s", PQgetvalue(r, 0, 1));
    snprintf(f->account, sizeof f->account, "%s", PQgetvalue(r, 0, 2));
    snprintf(f->device, sizeof f->device, "%s", PQgetvalue(r, 0, 3));
    snprintf(f->key_hex, sizeof f->key_hex, "%s", PQgetvalue(r, 0, 4));
    f->bound = f->device[0] != 0;
    f->revoked = !strcmp(PQgetvalue(r, 0, 5), "t");
    f->now_ms = atoll(PQgetvalue(r, 0, 6));
  } else if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-device: refresh_look: %s", PQerrorMessage(db));
  PQclear(r);
  return f->found;
}

bool auth_device_refresh(const request *q, PGconn *db, const char *rt_sha, const auth_refresh_facts *f, char proved[24]) {
  proved[0] = 0;
  char header[400], url[600];
  bool has_proof = api_header(q, "OA-Device-Proof", header, sizeof header) != NULL && header[0];
  adc_proof got = {0};
  bool seen = false;
  unsigned char key[65], digest[32];
  if (f->bound && has_proof && auth_unhex(f->key_hex, key, 65) && strlen(f->key_hex) == 130) {
    snprintf(url, sizeof url, "%s%s", auth_origin(), q->path);
    got = adc_check_c(header, q->method, url, rt_sha, key, digest);
    // The replay book is asked only about a proof that is this key's and in its window, so
    // nobody without the key can fill it; what that means is still the policy's to say.
    if (got.well_formed && got.verified && !f->revoked && dk_fresh(f->now_ms, got.ts_ms)) {
      char dhex[65]; auth_hex(digest, 32, dhex);
      const char *v[1] = {dhex};
      PGresult *r = PQexecParams(db, "SELECT auth.device_proof_once($1)", 1, NULL, v, NULL, NULL, 0);
      // A book we can't read is a proof we can't accept.
      seen = !(PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t"));
      if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-device: device_proof_once: %s", PQerrorMessage(db));
      PQclear(r);
    }
  }
  int verdict = (int)dk_refresh(f->bound, f->revoked, has_proof, got.well_formed, got.ts_ms, f->now_ms, got.verified, seen);
  if (dk_proved(verdict)) snprintf(proved, 24, "%s", f->device);
  if (dk_allows(verdict)) return true;
  char why[24], d[120]; copy_words(dk_why(verdict), why, sizeof why);
  snprintf(d, sizeof d, "{\"why\":\"%s\",\"device\":%s}", why, f->device[0] ? f->device : "null");
  auth_audit(db, f->account, f->session, "refresh.proof", "refused", d);
  api_log("auth-device: refresh refused: %s", why);
  refuse_proof(q, verdict, f->now_ms);
  return false;
}

void auth_device_refuse_unproved(const request *q) { refuse_proof(q, DK_NEEDED, 0); }

// ---------------------------------------------------------------- the devices list
static void devices(const request *q, PGconn *db, const auth_principal *p) {
  const char *v[2] = {p->account, p->session};
  PGresult *r = PQexecParams(db, "SELECT id, name, platform, store, created_ms, last_seen_ms, sessions, this, "
                                 "(extract(epoch FROM now()) * 1000)::bigint FROM auth.devices_of($1, $2)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("auth-device: devices_of: %s", PQerrorMessage(db)); PQclear(r); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  int n = PQntuples(r);
  size_t cap = 400 + (size_t)n * 500;
  char *json = malloc(cap), *lines = malloc(cap);
  if (!json || !lines) { free(json); free(lines); PQclear(r); auth_refuse(q, 500, "Out of memory."); return; }
  long long now_ms = n ? atoll(PQgetvalue(r, 0, 8)) : (long long)api_now_ms();
  size_t j = (size_t)snprintf(json, cap, "{\"now_ms\":%lld,\"devices\":[", now_ms);
  size_t l = (size_t)snprintf(lines, cap, "now_ms %lld\ncount %d\n", now_ms, n);
  for (int i = 0; i < n; i++) {
    char name[200]; auth_json_esc(PQgetvalue(r, i, 1), name, sizeof name);
    bool this = !strcmp(PQgetvalue(r, i, 7), "t");
    j += (size_t)snprintf(json + j, cap - j, "%s{\"id\":%s,\"name\":\"%s\",\"platform\":\"%s\",\"store\":\"%s\",\"created_ms\":%s,\"last_seen_ms\":%s,\"sessions\":%s,\"this\":%s}",
                          i ? "," : "", PQgetvalue(r, i, 0), name, PQgetvalue(r, i, 2), PQgetvalue(r, i, 3), PQgetvalue(r, i, 4), PQgetvalue(r, i, 5),
                          PQgetvalue(r, i, 6), this ? "true" : "false");
    // device <id> <platform> <store> <last_seen_ms> <logins> <this> <name to the line's end>
    l += (size_t)snprintf(lines + l, cap - l, "device %s %s %s %s %s %d %s\n", PQgetvalue(r, i, 0), PQgetvalue(r, i, 2), PQgetvalue(r, i, 3),
                          PQgetvalue(r, i, 5), PQgetvalue(r, i, 6), this, PQgetvalue(r, i, 1));
  }
  snprintf(json + j, cap - j, "]}");
  PQclear(r);
  auth_answer(q, 200, NULL, json, lines);
  free(json); free(lines);
}

static void device_remove(const request *q, PGconn *db, const auth_principal *p) {
  char id[20]; int end = 0;
  if (sscanf(q->path, "/api/auth/devices/%18[0-9]/remove%n", id, &end) != 1 || !end || q->path[end]) { auth_refuse(q, 404, "Not found."); return; }
  const char *v[2] = {p->account, id};
  PGresult *r = PQexecParams(db, "SELECT found, mine, removed FROM auth.device_look($1, $2::bigint)", 2, NULL, v, NULL, NULL, 0);
  bool found = false, mine = false, removed = false;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    found = !strcmp(PQgetvalue(r, 0, 0), "t"); mine = !strcmp(PQgetvalue(r, 0, 1), "t"); removed = !strcmp(PQgetvalue(r, 0, 2), "t");
  }
  PQclear(r);
  int verdict = (int)dk_remove(found, mine, removed);
  if (verdict != DK_OK) { refuse_proof(q, verdict, 0); return; }
  r = PQexecParams(db, "SELECT auth.device_remove($1, $2::bigint)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetisnull(r, 0, 0)) { PQclear(r); refuse_proof(q, DK_NOT_FOUND, 0); return; }
  int ended = atoi(PQgetvalue(r, 0, 0));
  PQclear(r);
  char d[80]; snprintf(d, sizeof d, "{\"device\":%s,\"ended\":%d}", id, ended);
  auth_audit(db, p->account, p->session, "device.remove", "ok", d);
  char json[80], lines[80];
  snprintf(json, sizeof json, "{\"state\":\"removed\",\"ended\":%d}", ended);
  snprintf(lines, sizeof lines, "state removed\nended %d\n", ended);
  auth_answer(q, 200, NULL, json, lines);
}

// ---------------------------------------------------------------- push targets
static void refuse_push(const request *q, PGconn *db, const auth_principal *p, int verdict) {
  char words[200], why[24], d[80];
  copy_words(dk_words(verdict), words, sizeof words);
  copy_words(dk_why(verdict), why, sizeof why);
  snprintf(d, sizeof d, "{\"why\":\"%s\"}", why);
  auth_audit(db, p->account, p->session, "device.push", "refused", d);
  auth_refuse(q, (int)dk_status(verdict), words);
}
static void push_set(const request *q, PGconn *db, const auth_principal *p) {
  char service[32] = "", token[600] = "", platform[12] = "";
  char *body = malloc(q->body_len + 1);
  if (!body) { auth_refuse(q, 500, "Out of memory."); return; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  auth_json_str(body, q->body_len, "service", service, sizeof service);
  auth_json_str(body, q->body_len, "token", token, sizeof token);
  free(body);
  const char *v[2] = {p->account, p->session};
  PGresult *r = PQexecParams(db, "SELECT app, platform FROM auth.session_device($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool app = false;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { app = !strcmp(PQgetvalue(r, 0, 0), "t"); snprintf(platform, sizeof platform, "%s", PQgetvalue(r, 0, 1)); }
  else api_log("auth-device: session_device: %s", PQerrorMessage(db));
  PQclear(r);
  int svc = (int)dk_push_service(str(service));
  int verdict = (int)dk_push_set(app, (int)dk_platform(str(platform)), svc, dk_push_token_ok(svc, str(token)));
  if (verdict != DK_OK) { refuse_push(q, db, p, verdict); return; }
  const char *w[4] = {p->account, p->session, service, token};
  r = PQexecParams(db, "SELECT auth.device_push_set($1, $2, $3, $4)", 4, NULL, w, NULL, NULL, 0);
  // The database's own refusal (the session's device went while we looked): never a quiet success.
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetisnull(r, 0, 0)) {
    if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-device: device_push_set: %s", PQerrorMessage(db));
    PQclear(r); refuse_push(q, db, p, DK_NO_DEVICE); return;
  }
  char id[24]; snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  char d[120], json[120], lines[120];
  snprintf(d, sizeof d, "{\"device\":%s,\"service\":\"%s\"}", id, service);
  auth_audit(db, p->account, p->session, "device.push", "ok", d);
  snprintf(json, sizeof json, "{\"state\":\"on\",\"device\":%s}", id);
  snprintf(lines, sizeof lines, "state on\ndevice %s\n", id);
  auth_answer(q, 200, NULL, json, lines);
}
static void push_off(const request *q, PGconn *db, const auth_principal *p) {
  const char *v[2] = {p->account, p->session};
  PGresult *r = PQexecParams(db, "SELECT auth.device_push_off($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool was = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-device: device_push_off: %s", PQerrorMessage(db));
  PQclear(r);
  if (was) auth_audit(db, p->account, p->session, "device.push", "ok", "{\"off\":true}");
  auth_answer(q, 200, NULL, "{\"state\":\"off\"}", "state off\n");
}

int auth_device_push(PGconn *db, const char *acct, long long notice, const char *what, const char *title, const char *words, int *failed) {
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT device, platform, service, token FROM auth.push_targets($1)", 1, NULL, v, NULL, NULL, 0);
  int n = PQresultStatus(r) == PGRES_TUPLES_OK ? PQntuples(r) : 0, sent = 0;
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-device: push_targets: %s", PQerrorMessage(db));
  for (int i = 0; i < n; i++) {
    const char *service = PQgetvalue(r, i, 2), *token = PQgetvalue(r, i, 3);
    int svc = (int)dk_push_service(str(service));
    int status = push_send(service, PQgetvalue(r, i, 1), token, title, words, what, notice);
    if (dk_push_sent(status)) { sent++; continue; }
    (*failed)++;
    if (status < 0) api_log("auth-device: notice %lld: %s push isn't set up here", notice, service);
    if (dk_push_gone(svc, status)) {
      const char *g[2] = {PQgetvalue(r, i, 0), token};
      PQclear(PQexecParams(db, "SELECT auth.device_push_gone($1::bigint, $2)", 2, NULL, g, NULL, NULL, 0));
      char d[80]; snprintf(d, sizeof d, "{\"device\":%s,\"gone\":true}", g[0]);
      auth_audit(db, acct, NULL, "device.push", "ok", d);
    }
  }
  PQclear(r);
  return sent;
}

bool auth_device_route(const request *q, PGconn *db, const char *op) {
  bool list = !strcmp(op, "devices"), remove = !strncmp(op, "devices/", 8);
  if (!list && !remove) return false;
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return true; }
  if (list) {
    if (strcmp(q->method, "GET")) { auth_refuse(q, 405, "GET this."); return true; }
    devices(q, db, p);
    return true;
  }
  if (strcmp(q->method, "POST")) { auth_refuse(q, 405, "POST this."); return true; }
  if (!auth_same_site(q)) { auth_refuse(q, 403, "Not from this site."); return true; }
  if (!strcmp(op, "devices/push")) { push_set(q, db, p); return true; }
  if (!strcmp(op, "devices/push/off")) { push_off(q, db, p); return true; }
  device_remove(q, db, p);
  return true;
}
