// Our own authentication (docs/auth/, #13200 to #13205): who is asking, whether this route lets
// them, and logging in with an email code or a Nostr key. Accounts, authenticators, sessions, the
// audit log and the rate limits' counts live in our Postgres (migrations 0142 to 0146); no identity
// provider holds a copy. The decisions are auth-policy.coax (#authority); sealing, lookups and
// the token codec are Coax too (auth-seal.coax, coax/lib/crypto/token.coax), compiled to C.
//
// Every request first passes auth_gate: the route's rule from RULES (public, guest, account,
// human, L2, L2-fresh, scope, admin), and a route without a rule is 404. `api --rules` prints the
// table; test.sh checks every route against it.
//
// Who (auth_principal): one credential per request, never two:
//   Authorization: Nostr <NIP-98>     the signer, at L2 (an agent's key is its agent account)
//   Authorization: Bearer oa_at_...   an app's access token (1 hour)
//   Cookie __Host-oa_session=oas_...  a web session (30 days idle, 180 absolute, new token daily)
//   X-Verse-Player                    a guest's player key: never an account
//
// Routes (JSON; ?as=lines answers the same as "name value" lines for the Coax pages):
//   GET  /api/auth/account                who is logged in: account, kind, level, emails (masked), keys
//   GET  /api/auth/limits                 every limit (the table /docs/limits is made from)
//   POST /api/auth/email/start {email, client?, link?}   one message with a code and a link; sets
//                                         __Host-oa_pending (an app gets {pending} instead)
//   POST /api/auth/email/verify {code, pending?}         the code, in the browser that asked -> logged in
//   POST /api/auth/email/link {t, pending?}              the link's token: logged in in the browser that
//                                         asked; elsewhere 409 {code} to type where you asked
//   POST /api/auth/nostr/challenge {client?}             a challenge to sign over (5 minutes)
//   POST /api/auth/nostr {challenge}      Authorization: Nostr over this route and body -> logged in
//   POST /api/auth/phone/start            the phone QR's challenge and the page's poll token
//   POST /api/auth/phone/approve {challenge}   the app, NIP-98 signed by its account's key
//   POST /api/auth/phone/poll {poll}      waiting | expired | logged in
//   POST /api/auth/refresh {refresh_token}     an app's next pair; a reused one ends the family
//   POST /api/auth/logout                 this session (with no session of ours, the old route's: private.c)
//   POST /api/auth/logout-everywhere      every session of the account (L2)
//   GET  /api/auth/sessions               the devices list; POST /api/auth/sessions/<id>/remove
//   POST /api/auth/freeze {t?}            "This wasn't me": a notice's link token, or the session's own account
//
//   OA_AUTH_SEAL_KEY        "1:<64 hex>[,2:<64 hex>...]": the newest seals, every one opens (Secret Manager oa-auth-seal-key)
//   OA_AUTH_LOOKUP_PEPPER   "1:<64 hex>[,...]": the email lookup's HMAC key (oa-auth-lookup-pepper)
//   OA_API_ORIGIN           this site, for NIP-98 URLs and the email link
//   OA_EMAIL_ALLOW          (email.c) staging's allowlist of addresses mail may go to
//
// Nothing secret is ever logged: no token, code, cookie, email or key material. Refusals log the
// route, the rule and the principal's kind.
#include "api.h"
#include "nostr.h"
#include "auth-policy.generated.h"
#include "auth-seal.generated.h"

#include <ctype.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <sys/random.h>
#endif

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static const char *origin(void) { return env_or("OA_API_ORIGIN", "https://openagents.com"); }

// ---------------------------------------------------------------- the rule table
typedef struct { const char *method, *path; bool prefix; int rule; const char *note; } auth_rule;
#define R(m, p, pre, rule, note) {m, p, pre, rule, note}
// First match wins: specific paths before their families. "*" is any method.
static const auth_rule RULES[] = {
  R("GET",  "/api/playtest/health", false, AP_PUBLIC, "a database round trip"),
  R("GET",  "/api/playtest/old-link", false, AP_PUBLIC, "the old playtest's links move"),
  // ---- our own authentication (this file)
  R("GET",  "/api/auth/account", false, AP_PUBLIC, "who is logged in, or nobody"),
  R("GET",  "/api/auth/limits", false, AP_PUBLIC, "every limit"),
  R("POST", "/api/auth/email/start", false, AP_PUBLIC, "log in (or link an email when logged in)"),
  R("POST", "/api/auth/email/verify", false, AP_PUBLIC, "the code, in the browser that asked"),
  R("POST", "/api/auth/email/link", false, AP_PUBLIC, "the link, in the browser that asked"),
  R("POST", "/api/auth/nostr/challenge", false, AP_PUBLIC, "a challenge to sign"),
  R("POST", "/api/auth/nostr", false, AP_PUBLIC, "a signed challenge logs in"),
  R("POST", "/api/auth/phone/start", false, AP_PUBLIC, "the phone QR"),
  R("POST", "/api/auth/phone/approve", false, AP_HUMAN, "the app approves with its account's key"),
  R("POST", "/api/auth/phone/poll", false, AP_PUBLIC, "the page that showed the QR"),
  R("POST", "/api/auth/refresh", false, AP_PUBLIC, "an app's refresh token"),
  R("POST", "/api/auth/logout", false, AP_PUBLIC, "ends this session, if any"),
  R("POST", "/api/auth/logout-everywhere", false, AP_L2, "ends every session of the account"),
  R("GET",  "/api/auth/sessions", false, AP_ACCOUNT, "the devices list"),
  R("POST", "/api/auth/sessions/", true, AP_ACCOUNT, "remove one device's session"),
  R("POST", "/api/auth/freeze", false, AP_PUBLIC, "a notice's link token, or the session's account"),
  // ---- the old groundwork, until #13207 deletes it (each checks its own caller)
  R("*",    "/api/auth/v2/", true, AP_PUBLIC, "device login and links (access.c): NIP-98 or session, checked there"),
  R("*",    "/api/auth/", true, AP_PUBLIC, "the phone sign-in and words account (private.c), until deleted"),
  // ---- admins (admin.c, email.c, makers.c' and upgrades' admin pages)
  R("GET",  "/api/admin/login", false, AP_PUBLIC, "an admin's NIP-98 signature logs in"),
  R("*",    "/api/admin/logout", false, AP_PUBLIC, "ends the session"),
  R("*",    "/api/admin/", true, AP_ADMIN, "site admins (admins.conf and database admins)"),
  // ---- /chat for admins, its runner, and the rooms' chat under the same root (chat.c, room.c)
  R("*",    "/api/chat/runner/", true, AP_PUBLIC, "the chat runner's key, checked there"),
  R("*",    "/api/chat/state", false, AP_ADMIN, "/chat"), R("*", "/api/chat/new", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/send", false, AP_ADMIN, "/chat"), R("*", "/api/chat/model", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/stop", false, AP_ADMIN, "/chat"), R("*", "/api/chat/tab", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/record", false, AP_ADMIN, "/chat"), R("*", "/api/chat/key", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/key_remove", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/", true, AP_GUEST, "a room's chat (room.c)"),
  R("*",    "/api/presence/", true, AP_GUEST, "a room's presence (room.c)"),
  // ---- scheduled jobs: Cloud Scheduler's OIDC token or an admin, checked there (jobs.c)
  R("*",    "/api/jobs/", true, AP_PUBLIC, "the scheduler (OIDC) or an admin, checked there"),
  // ---- analytics (analytics.c)
  R("*",    "/api/analytics/events", false, AP_GUEST, "a page's batch"),
  R("*",    "/api/analytics/team", false, AP_PUBLIC, "a team link's token, checked there"),
  R("*",    "/api/analytics/realms", false, AP_PUBLIC, "public realm counts"),
  // ---- the playtest and the Verse: player keys, accounts and signers, each checked in its route
  R("*",    "/api/playtest/", true, AP_GUEST, "players, realms, makers, seats, rooms, labs (player.c, realms.c, ...)"),
  R("*",    "/api/change/", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/log/", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/award", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/planets", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/verse/packs/", true, AP_GUEST, "private human packs, a key a session (verse_packs.c)"),
  R("*",    "/api/villa/", true, AP_GUEST, "the villa (villa.c)"),
  R("*",    "/memorial", true, AP_PUBLIC, "a memorial's page (villa.c)"),
  // ---- our models and the mesh: NIP-98 or a session, metered (inference.c, mesh.c)
  R("*",    "/api/v1/", true, AP_GUEST, "NIP-98 signed or logged in, checked there"),
  // ---- private projects and drops: members only, the same 404 for everyone else (private.c, drops.c)
  R("*",    "/api/private/", true, AP_PUBLIC, "members' tools, NIP-98 signed, checked there"),
  R("*",    "/projects/", true, AP_PUBLIC, "a member's page, else the locked one"),
  R("*",    "/api/drops", true, AP_PUBLIC, "who dropped them, checked there"),
  R("*",    "/api/worldbuilder/", true, AP_PUBLIC, "/create-world's chat: anyone, capped per address there (worldbuilder.c)"),
};
#undef R

static const char *rule_name(int rule) {
  static const char *names[] = {"public", "guest", "account", "human", "L2", "L2-fresh", "scope", "admin"};
  return rule >= 0 && rule <= 7 ? names[rule] : "?";
}

static const auth_rule *rule_for(const char *method, const char *path) {
  for (size_t i = 0; i < sizeof RULES / sizeof RULES[0]; i++) {
    const auth_rule *r = &RULES[i];
    if (strcmp(r->method, "*") && strcmp(r->method, method) && !(!strcmp(r->method, "GET") && !strcmp(method, "HEAD"))) continue;
    size_t n = strlen(r->path);
    if (r->prefix ? !strncmp(path, r->path, n) : !strcmp(path, r->path)) return r;
  }
  return NULL;
}

void auth_print_rules(FILE *out) {
  for (size_t i = 0; i < sizeof RULES / sizeof RULES[0]; i++)
    fprintf(out, "%s\t%s%s\t%s\t%s\n", RULES[i].method, RULES[i].path, RULES[i].prefix ? "*" : "", rule_name(RULES[i].rule), RULES[i].note);
}
// The rule a path would get (api --rule METHOD PATH), for the route check.
const char *auth_rule_of(const char *method, const char *path) {
  const auth_rule *r = rule_for(method, path);
  return r ? rule_name(r->rule) : NULL;
}

// ---------------------------------------------------------------- limits (#13203)
// Every limit we enforce, as people read it. /docs/limits and GET /api/auth/limits are made from
// this table (api --limits), so the page and the server can't drift. The words name the limit;
// the answer adds when it lifts.
typedef struct { const char *name, *what, *per, *words; int max; long long window_ms; } auth_limit;
enum { L_EMAIL_HOUR, L_EMAIL_DAY, L_EMAIL_IP, L_CODE_TRIES, L_KEY_LOGIN, L_REFRESH, L_NEW_ACCOUNT, L_COUNT };
static const auth_limit LIMITS[L_COUNT] = {
  [L_EMAIL_HOUR]  = {"email_hour", "Email codes sent to one address", "an hour", "Too many codes for this address.", 5, 3600000LL},
  [L_EMAIL_DAY]   = {"email_day", "Email codes sent to one address", "a day", "Too many codes for this address today.", 20, 86400000LL},
  [L_EMAIL_IP]    = {"email_ip", "Email codes sent from one network address", "an hour", "Too many codes from your network.", 30, 3600000LL},
  [L_CODE_TRIES]  = {"code_tries", "Tries at one email code", "per code", "Too many tries. Ask for a new code.", 5, 600000LL},
  [L_KEY_LOGIN]   = {"key_login", "Logins with a key from one network address", "a minute", "Too many logins from your network.", 30, 60000LL},
  [L_REFRESH]     = {"refresh", "App token refreshes for one login", "an hour", "Too many refreshes.", 60, 3600000LL},
  [L_NEW_ACCOUNT] = {"new_account", "New accounts from one network address", "an hour", "Too many new accounts from your network.", 10, 3600000LL},
};

void auth_print_limits(FILE *out) {   // api --limits: the Markdown /docs/limits is
  fprintf(out, "# Limits\n\nEvery limit we enforce on logging in. When you reach one, we say which and when it lifts.\n\n"
               "| What | Limit |\n| --- | --- |\n");
  for (int i = 0; i < L_COUNT; i++) fprintf(out, "| %s | %d %s |\n", LIMITS[i].what, LIMITS[i].max, LIMITS[i].per);
  fprintf(out, "\nThis page is made from the table the API enforces (`coax/web/api/auth.c`).\n");
}

// ---------------------------------------------------------------- small pieces
static void hex_of(const unsigned char *b, size_t n, char *out) {
  static const char d[] = "0123456789abcdef";
  for (size_t i = 0; i < n; i++) { out[2 * i] = d[b[i] >> 4]; out[2 * i + 1] = d[b[i] & 15]; }
  out[2 * n] = 0;
}
static bool unhex(const char *s, unsigned char *out, size_t n) {
  if (strlen(s) < 2 * n) return false;
  for (size_t i = 0; i < n; i++) {
    int v = 0;
    for (int k = 0; k < 2; k++) {
      char c = s[2 * i + k]; v <<= 4;
      if (c >= '0' && c <= '9') v |= c - '0'; else if (c >= 'a' && c <= 'f') v |= c - 'a' + 10; else return false;
    }
    out[i] = (unsigned char)v;
  }
  return true;
}
static bool random_bytes(unsigned char *b, size_t n) {
  while (n) { size_t k = n > 256 ? 256 : n; if (getentropy(b, k)) return false; b += k; n -= k; }
  return true;
}
static bool random_hex(char *out, size_t bytes) {
  unsigned char b[64];
  if (bytes > sizeof b || !random_bytes(b, bytes)) return false;
  hex_of(b, bytes, out);
  return true;
}
// SHA-256 of a string, hex.
static void sha_hex(const char *s, char out[65]) {
  unsigned char d[32];
  as_sha256((coax_v_u8){(uint8_t *)s, (int64_t)strlen(s)}, (coax_v_u8){d, 32});
  hex_of(d, 32, out);
}
// A new token of one of the four kinds (oas_, oa_at_, oa_rt_): the string and its digest.
static bool new_token(const char *prefix, char *out, size_t cap, char digest[65]) {
  unsigned char r[32];
  if (!random_bytes(r, 32)) return false;
  int64_t n = token_make((coax_string){prefix, (int64_t)strlen(prefix)}, (coax_rv_u8){r, 32}, (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  memset(r, 0, sizeof r);
  if (n <= 0) return false;
  out[n] = 0;
  unsigned char d[32];
  if (!token_digest((coax_string){out, n}, (coax_v_u8){d, 32})) return false;
  hex_of(d, 32, digest);
  return true;
}
// A token as sent: its digest when it is well formed and of this kind, else false. Checked before
// any database lookup.
static bool token_ok(const char *text, int64_t kind, char digest[65]) {
  coax_string s = {text, (int64_t)strlen(text)};
  coax_t_TkCheck c = token_check(s);
  if (c.error || c.kind != kind) return false;
  unsigned char d[32];
  if (!token_digest(s, (coax_v_u8){d, 32})) return false;
  hex_of(d, 32, digest);
  return true;
}
enum { TOKEN_SESSION = 1, TOKEN_ACCESS = 2, TOKEN_REFRESH = 3 };

// A cookie's value (letters, digits, _ and -), or "".
static void cookie(const request *q, const char *name, char *out, size_t cap) {
  char cookies[4096], want[64];
  out[0] = 0;
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return;
  int w = snprintf(want, sizeof want, "%s=", name);
  for (const char *c = strstr(cookies, want); c; c = strstr(c + 1, want)) {
    if (c != cookies && c[-1] != ' ' && c[-1] != ';') continue;
    c += w;
    size_t n = 0;
    while (n + 1 < cap && (isalnum((unsigned char)c[n]) || c[n] == '_' || c[n] == '-')) { out[n] = c[n]; n++; }
    out[n] = 0;
    return;
  }
}

// The visitor's address: the load balancer's next to last (as access.c), and its digest for keys.
static void visitor(const request *q, char ip[128]) {
  char xff[512];
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  snprintf(ip, 128, "%s", np ? parts[np >= 2 ? np - 2 : 0] : "local");
}
static void visitor_key(const request *q, char out[65]) { char ip[128]; visitor(q, ip); char s[160]; snprintf(s, sizeof s, "ip:%s", ip); sha_hex(s, out); }

// A JSON string's value for a top-level key (flat bodies only), unescaped for \" and \\, or NULL.
static const char *json_str(const char *body, size_t n, const char *key, char *out, size_t cap) {
  char want[80]; snprintf(want, sizeof want, "\"%s\"", key);
  const char *end = body + n;
  for (const char *p = body; p && p < end; p++) {
    p = strstr(p, want);
    if (!p || p >= end) break;
    const char *v = p + strlen(want);
    while (v < end && (*v == ' ' || *v == '\t' || *v == '\n' || *v == '\r')) v++;
    if (v >= end || *v != ':') continue;
    v++;
    while (v < end && (*v == ' ' || *v == '\t' || *v == '\n' || *v == '\r')) v++;
    if (v >= end || *v != '"') return NULL;
    v++;
    size_t k = 0;
    while (v < end && *v != '"' && k + 1 < cap) {
      if (*v == '\\' && v + 1 < end) { v++; if (*v != '"' && *v != '\\' && *v != '/') return NULL; }
      if ((unsigned char)*v < 0x20) return NULL;
      out[k++] = *v++;
    }
    if (v >= end || *v != '"') return NULL;
    out[k] = 0;
    return out;
  }
  if (cap) out[0] = 0;
  return NULL;
}
static bool json_true(const char *body, size_t n, const char *key) {
  char want[80]; snprintf(want, sizeof want, "\"%s\"", key);
  char *b = strndup(body, n); if (!b) return false;
  const char *p = strstr(b, want);
  bool yes = false;
  if (p) { p += strlen(want); while (*p == ' ' || *p == ':') p++; yes = !strncmp(p, "true", 4); }
  free(b);
  return yes;
}
// A string as a JSON string's inside (quotes and controls escaped), bounded.
static void json_esc(const char *s, char *out, size_t cap) {
  size_t k = 0;
  for (; *s && k + 7 < cap; s++) {
    unsigned char c = (unsigned char)*s;
    if (c == '"' || c == '\\') { out[k++] = '\\'; out[k++] = (char)c; }
    else if (c < 0x20) k += (size_t)snprintf(out + k, cap - k, "\\u%04x", c);
    else out[k++] = (char)c;
  }
  out[k] = 0;
}

static bool as_lines(const request *q) { char v[16]; return api_query(q, "as", v, sizeof v) && !strcmp(v, "lines"); }
// An answer: JSON, or (for ?as=lines) the lines. headers: extra response headers, or NULL.
static void answer(const request *q, int status, const char *headers, const char *json, const char *lines) {
  if (as_lines(q) && lines) api_respond(q->fd, status, "text/plain; charset=utf-8", headers, lines, strlen(lines));
  else api_respond(q->fd, status, "application/json", headers, json, strlen(json));
}
static void refuse(const request *q, int status, const char *words) {
  char e[400], json[600], lines[500];
  json_esc(words, e, sizeof e);
  snprintf(json, sizeof json, "{\"error\":\"%s\"}", e);
  snprintf(lines, sizeof lines, "error %s\n", words);
  answer(q, status, NULL, json, lines);
}

// One audit row (auth.audit). detail: a JSON object of fixed keys, never secrets or PII.
static void audit(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, const char *detail) {
  const char *v[5] = {acct && *acct ? acct : NULL, actor && *actor ? actor : NULL, action, result, detail ? detail : "{}"};
  PGresult *r = PQexecParams(db, "SELECT auth.log($1, $2, $3, $4, $5::jsonb)", 5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth: audit %s: %s", action, PQerrorMessage(db));
  PQclear(r);
}

// One limit's hit, counted. True when allowed; else until_ms is when the window lifts.
static bool limit_hit(PGconn *db, int which, const char *key, long long *until_ms) {
  const auth_limit *l = &LIMITS[which];
  char win[32]; snprintf(win, sizeof win, "%lld", l->window_ms);
  const char *v[3] = {l->name, key, win};
  PGresult *r = PQexecParams(db, "SELECT started_ms, n FROM auth.limit_hit($1, $2, $3::bigint, true)", 3, NULL, v, NULL, NULL, 0);
  bool ok = true;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    long long started = atoll(PQgetvalue(r, 0, 0));
    int n = atoi(PQgetvalue(r, 0, 1));
    ok = ap_limit_ok(n, l->max);
    if (until_ms) *until_ms = started + l->window_ms;
  } else api_log("auth: limit %s: %s", l->name, PQerrorMessage(db));   // a counting failure doesn't lock people out
  PQclear(r);
  return ok;
}
// The refusal for a limit: its words and when it lifts, both shown.
static void refuse_limit(const request *q, int which, long long until_ms) {
  long long left = until_ms - (long long)api_now_ms();
  if (left < 1000) left = 1000;
  char when[64];
  if (left < 90000) snprintf(when, sizeof when, "in %lld seconds", (left + 999) / 1000);
  else if (left < 5400000) snprintf(when, sizeof when, "in %lld minutes", (left + 59999) / 60000);
  else snprintf(when, sizeof when, "in %lld hours", (left + 3599999) / 3600000);
  char words[300], e[400], json[700], lines[500];
  snprintf(words, sizeof words, "%s Try again %s.", LIMITS[which].words, when);
  json_esc(words, e, sizeof e);
  snprintf(json, sizeof json, "{\"error\":\"%s\",\"limit\":\"%s\",\"max\":%d,\"per\":\"%s\",\"until_ms\":%lld}",
           e, LIMITS[which].name, LIMITS[which].max, LIMITS[which].per, until_ms);
  snprintf(lines, sizeof lines, "error %s\nlimit %s\nuntil_ms %lld\n", words, LIMITS[which].name, until_ms);
  char retry[64]; snprintf(retry, sizeof retry, "Retry-After: %lld\r\n", (left + 999) / 1000);
  answer(q, 429, retry, json, lines);
}

// ---------------------------------------------------------------- keys for sealing and lookup
typedef struct { int version; unsigned char key[32]; } versioned_key;
// "1:<hex>,2:<hex>" into keys (newest last); how many.
static int keys_from(const char *env, versioned_key *keys, int cap) {
  const char *s = getenv(env);
  int n = 0;
  while (s && *s && n < cap) {
    int v = atoi(s);
    const char *colon = strchr(s, ':');
    if (v <= 0 || v > 255 || !colon || !unhex(colon + 1, keys[n].key, 32)) break;
    keys[n].version = v; n++;
    s = strchr(colon, ',');
    if (s) s++;
  }
  return n;
}
static bool sealing_ready(void) {
  versioned_key k[4];
  return keys_from("OA_AUTH_SEAL_KEY", k, 4) > 0 && keys_from("OA_AUTH_LOOKUP_PEPPER", k, 4) > 0;
}
// The lookup of a normalized address under every pepper: "{1:hex,2:hex}" (a Postgres array) and
// the newest one alone.
static bool lookups(const char *email, char *all, size_t cap, char newest[80]) {
  versioned_key k[4];
  int n = keys_from("OA_AUTH_LOOKUP_PEPPER", k, 4);
  if (!n) return false;
  size_t at = (size_t)snprintf(all, cap, "{");
  for (int i = 0; i < n; i++) {
    unsigned char d[32]; char h[65];
    if (!as_lookup((coax_v_u8){k[i].key, 32}, (coax_v_u8){(uint8_t *)email, (int64_t)strlen(email)}, (coax_v_u8){d, 32})) return false;
    hex_of(d, 32, h);
    at += (size_t)snprintf(all + at, cap - at, "%s%d:%s", i ? "," : "", k[i].version, h);
    if (i == n - 1) snprintf(newest, 80, "%d:%s", k[i].version, h);
  }
  snprintf(all + at, cap - at, "}");
  return true;
}
// Seals text with associated data ad under the newest key: hex of version, nonce, box.
static bool seal(const char *ad, const char *text, char *out, size_t cap) {
  versioned_key k[4];
  int n = keys_from("OA_AUTH_SEAL_KEY", k, 4);
  size_t len = strlen(text);
  if (!n || len > 512 || cap < 2 * (1 + 24 + len + 16) + 1) return false;
  unsigned char buf[1 + 24 + 512 + 16];
  buf[0] = (unsigned char)k[n - 1].version;
  if (!random_bytes(buf + 1, 24)) return false;
  if (!as_seal((coax_v_u8){k[n - 1].key, 32}, (coax_v_u8){buf + 1, 24}, (coax_v_u8){(uint8_t *)ad, (int64_t)strlen(ad)},
               (coax_v_u8){(uint8_t *)text, (int64_t)len}, (coax_v_u8){buf + 25, (int64_t)len + 16})) return false;
  hex_of(buf, 1 + 24 + len + 16, out);
  return true;
}
// Opens a sealed value with associated data ad; false when no key opens it.
static bool unseal(const char *ad, const char *sealed, char *out, size_t cap) {
  size_t h = strlen(sealed);
  if (h % 2 || h < 2 * (1 + 24 + 16) || h > 2 * (1 + 24 + 512 + 16)) return false;
  unsigned char buf[1 + 24 + 512 + 16];
  if (!unhex(sealed, buf, h / 2)) return false;
  size_t len = h / 2 - 1 - 24 - 16;
  if (cap < len + 1) return false;
  versioned_key k[4];
  int n = keys_from("OA_AUTH_SEAL_KEY", k, 4);
  for (int i = 0; i < n; i++) {
    if (k[i].version != buf[0]) continue;
    if (as_open((coax_v_u8){k[i].key, 32}, (coax_v_u8){buf + 1, 24}, (coax_v_u8){(uint8_t *)ad, (int64_t)strlen(ad)},
                (coax_v_u8){buf + 25, (int64_t)len + 16}, (coax_v_u8){(uint8_t *)out, (int64_t)len})) { out[len] = 0; return true; }
  }
  return false;
}
// a•••@example.com
static void mask_email(const char *email, char *out, size_t cap) {
  const char *at = strchr(email, '@');
  if (!at || at == email) { snprintf(out, cap, "•••"); return; }
  snprintf(out, cap, "%c•••%s", email[0], at);
}

// ---------------------------------------------------------------- the principal
static auth_principal current;
static bool resolved;
static const request *resolved_for;
static char set_cookie[700];   // a rotated or cleared session cookie, sent with this request's answer

#define SESSION_COOKIE "__Host-oa_session"
#define PENDING_COOKIE "__Host-oa_pending"
#define SESSION_MAX_AGE 2592000   // 30 days: the idle limit; every rotation sends it again

static void cookie_header(char *out, size_t cap, const char *name, const char *value, int max_age) {
  snprintf(out, cap, "Set-Cookie: %s=%s; Path=/; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n", name, value, max_age);
}

// Looks up a session by its token's digest and fills p. false when there is none or it has
// ended (an ended web session's row is deleted, and its cookie cleared).
static bool from_session(PGconn *db, const char *sha, bool web, const char *token_for_rotation, auth_principal *p) {
  const char *v[1] = {sha};
  PGresult *r = PQexecParams(db, "SELECT id, account, kind, verse_account, level, via_kind, client, created_ms, last_seen_ms, rotated_ms, "
                                 "coalesce(l2_ms, 0), now_ms, current, good_epoch, frozen, team, coalesce(pubkey, '') FROM auth.session_look($1)",
                             1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); return false; }
  long long now = atoll(PQgetvalue(r, 0, 11));
  bool app = !strcmp(PQgetvalue(r, 0, 6), "app");
  int state = (int)ap_session(app, now, atoll(PQgetvalue(r, 0, 7)), atoll(PQgetvalue(r, 0, 8)), atoll(PQgetvalue(r, 0, 9)));
  bool good_epoch = !strcmp(PQgetvalue(r, 0, 13), "t"), frozen = !strcmp(PQgetvalue(r, 0, 14), "t");
  char sid[32]; snprintf(sid, sizeof sid, "%s", PQgetvalue(r, 0, 0));
  if (state == AP_SESSION_ENDED || !good_epoch || frozen) {
    PQclear(r);
    const char *e[1] = {sid};
    PQclear(PQexecParams(db, "SELECT auth.session_end($1)", 1, NULL, e, NULL, NULL, 0));
    if (web) cookie_header(set_cookie, sizeof set_cookie, SESSION_COOKIE, "", 0);
    return false;
  }
  memset(p, 0, sizeof *p);
  const char *kind = PQgetvalue(r, 0, 2);
  p->kind = !strcmp(kind, "agent") ? AP_AGENT : !strcmp(kind, "service") ? AP_SERVICE : AP_PERSON;
  snprintf(p->account, sizeof p->account, "%s", PQgetvalue(r, 0, 1));
  snprintf(p->verse_account, sizeof p->verse_account, "%s", PQgetvalue(r, 0, 3));
  snprintf(p->session, sizeof p->session, "%s", sid);
  p->level = !strcmp(PQgetvalue(r, 0, 4), "L2") ? 2 : 1;
  long long l2 = atoll(PQgetvalue(r, 0, 10));
  p->l2_age_ms = l2 > 0 ? now - l2 : -1;
  p->team = !strcmp(PQgetvalue(r, 0, 15), "t");
  snprintf(p->pubkey, sizeof p->pubkey, "%s", PQgetvalue(r, 0, 16));
  snprintf(p->via, sizeof p->via, "%s", PQgetvalue(r, 0, 5));
  p->admin = p->pubkey[0] && admin_is(p->pubkey);
  p->credential = web ? AUTH_COOKIE : AUTH_BEARER;
  bool is_current = !strcmp(PQgetvalue(r, 0, 12), "t");
  PQclear(r);
  const char *s[1] = {sid};
  PQclear(PQexecParams(db, "SELECT auth.session_seen($1)", 1, NULL, s, NULL, NULL, 0));
  // A new web token every 24 hours of use; the old one works 60 seconds more (parallel tabs).
  if (web && is_current && state == AP_SESSION_ROTATE && token_for_rotation) {
    char tok[160], dig[65];
    if (new_token("oas_", tok, sizeof tok, dig)) {
      const char *w[3] = {sid, sha, dig};
      PGresult *x = PQexecParams(db, "SELECT auth.session_rotate($1, $2, $3)", 3, NULL, w, NULL, NULL, 0);
      if (PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1 && !strcmp(PQgetvalue(x, 0, 0), "t"))
        cookie_header(set_cookie, sizeof set_cookie, SESSION_COOKIE, tok, SESSION_MAX_AGE);
      PQclear(x);
    }
  }
  return true;
}

// Resolves the request's one credential. false with *status (400 two credentials, 401 a bad
// credential) when it answered nothing yet but must be refused.
static bool resolve(const request *q, PGconn *db, auth_principal *p, int *status) {
  memset(p, 0, sizeof *p);
  *status = 0;
  char auth[4096], tok[256];
  bool has_auth = api_header(q, "Authorization", auth, sizeof auth) != NULL;
  bool nostr = has_auth && !strncmp(auth, "Nostr ", 6);
  bool bearer = has_auth && (!strncmp(auth, "Bearer oa_at_", 13) || !strncmp(auth, "Bearer oa_key_", 14));
  cookie(q, SESSION_COOKIE, tok, sizeof tok);
  bool has_cookie = tok[0] != 0;
  int verdict = (int)ap_credentials(nostr, bearer, has_cookie);
  if (verdict != AP_OK) { *status = (int)ap_status(verdict); return false; }
  api_header(q, "X-Verse-Player", p->player, sizeof p->player);
  if (nostr) {
    char url[4300], err[160];
    snprintf(url, sizeof url, "%s%s", origin(), q->target);
    nostr_event ev;
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) { *status = 401; return false; }
    memcpy(p->pubkey, ev.pubkey, 65);
    memcpy(p->event_id, ev.id, 65);
    p->payload = ev.payload[0] != 0;
    p->credential = AUTH_NOSTR;
    p->level = 2; p->l2_age_ms = 0;
    p->kind = AP_PERSON;   // a key no account holds signs as itself; an agent's key is its agent
    const char *v[1] = {ev.pubkey};
    PGresult *r = PQexecParams(db, "SELECT a.id, a.kind, a.verse_account, a.team, a.frozen_at IS NOT NULL FROM auth.nostr_keys k "
                                   "JOIN auth.accounts a ON a.id = k.account WHERE k.pubkey = $1 AND a.deleted_at IS NULL",
                               1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      if (!strcmp(PQgetvalue(r, 0, 4), "t")) { PQclear(r); *status = 401; return false; }   // frozen
      snprintf(p->account, sizeof p->account, "%s", PQgetvalue(r, 0, 0));
      const char *k = PQgetvalue(r, 0, 1);
      p->kind = !strcmp(k, "agent") ? AP_AGENT : !strcmp(k, "service") ? AP_SERVICE : AP_PERSON;
      snprintf(p->verse_account, sizeof p->verse_account, "%s", PQgetvalue(r, 0, 2));
      p->team = !strcmp(PQgetvalue(r, 0, 3), "t");
    } else snprintf(p->verse_account, sizeof p->verse_account, "n-%s", ev.pubkey);
    PQclear(r);
    p->admin = admin_is(p->pubkey);
    snprintf(p->via, sizeof p->via, "nostr");
    return true;
  }
  if (bearer) {
    char t[256], dig[65];
    snprintf(t, sizeof t, "%s", auth + 7);
    if (!token_ok(t, TOKEN_ACCESS, dig)) { *status = 401; return false; }
    const char *v[1] = {dig};
    PGresult *r = PQexecParams(db, "SELECT auth.access_session($1)", 1, NULL, v, NULL, NULL, 0);
    char ssha[65] = "";
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) snprintf(ssha, sizeof ssha, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    if (!ssha[0] || !from_session(db, ssha, false, NULL, p)) { *status = 401; return false; }
    api_header(q, "X-Verse-Player", p->player, sizeof p->player);
    return true;
  }
  if (has_cookie) {
    char dig[65];
    if (token_ok(tok, TOKEN_SESSION, dig) && from_session(db, dig, true, tok, p)) {
      api_header(q, "X-Verse-Player", p->player, sizeof p->player);
      return true;
    }
    // A stale or unknown cookie is nobody, and is cleared.
    memset(p, 0, sizeof *p);
    api_header(q, "X-Verse-Player", p->player, sizeof p->player);
    if (!set_cookie[0]) cookie_header(set_cookie, sizeof set_cookie, SESSION_COOKIE, "", 0);
  }
  // The old key-made sessions, until #13207 deletes them: the admin page's oa_admin and the phone
  // sign-in's oa_session. Both were made by a key's signature, so both are L2; neither is an account.
  char legacy[65] = "", old[80];
  if (!admin_cookie_who(q, db, legacy)) {
    legacy[0] = 0;
    cookie(q, "oa_session", old, sizeof old);
    if (strlen(old) == 64) {
      const char *v[1] = {old};
      PGresult *r = PQexecParams(db, "SELECT signin.session($1)", 1, NULL, v, NULL, NULL, 0);
      if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) snprintf(legacy, sizeof legacy, "%s", PQgetvalue(r, 0, 0));
      PQclear(r);
    }
  }
  if (strlen(legacy) == 64) {
    p->kind = AP_PERSON; p->credential = AUTH_COOKIE; p->level = 2; p->l2_age_ms = -1;
    memcpy(p->pubkey, legacy, 65);
    p->admin = admin_is(legacy);
    snprintf(p->verse_account, sizeof p->verse_account, "n-%s", legacy);
    snprintf(p->via, sizeof p->via, "old-session");
    return true;
  }
  p->kind = p->player[0] ? AP_KIND_GUEST : AP_NOBODY;
  return true;
}

// The principal for this request (resolved once, on first ask). NULL when its credential was
// refused (two credentials, a bad signature or token).
const auth_principal *auth_principal_of(const request *q) {
  if (resolved && resolved_for == q) return current.refused ? NULL : &current;
  PGconn *db = api_db();
  resolved = true; resolved_for = q;
  int status = 0;
  if (!db || !resolve(q, db, &current, &status)) { current.refused = status ? status : 503; return NULL; }
  return &current;
}
// The numbers api.h shares with auth-policy.coax: a mismatch stops the service at start.
void auth_init(void) {
  bool ok = ap_admit(AP_ADMIN, AP_PERSON, 2, 0, true, false) == AP_OK && ap_admit(AP_ADMIN, AP_PERSON, 2, 0, false, false) == AP_NOT_ADMIN
         && ap_admit(AP_ACCOUNT, AP_NOBODY, 0, -1, false, false) == AP_LOG_IN && ap_admit(AP_HUMAN, AP_AGENT, 2, 0, false, false) == AP_HUMAN_ONLY
         && ap_admit(AP_L2, AP_PERSON, 1, -1, false, false) == AP_STEP_UP && ap_admit(AP_GUEST, AP_KIND_GUEST, 0, -1, false, false) == AP_OK
         && ap_credentials(true, false, true) == AP_TWO_CREDENTIALS && ap_status(AP_LOG_IN) == 401
         && ap_session(false, 200LL * 86400000LL, 0, 0, 0) == AP_SESSION_ENDED;
  if (!ok) { fprintf(stderr, "api: auth.c and auth-policy.coax disagree on their numbers\n"); exit(2); }
}

// Start of each request (api.c): nothing carries over from the last.
void auth_reset(void) { resolved = false; resolved_for = NULL; set_cookie[0] = 0; memset(&current, 0, sizeof current); }
const char *auth_extra_headers(void) { return set_cookie; }

// The gate: true when it answered (a refusal), false to let the route run.
bool auth_gate(const request *q) {
  const auth_rule *rule = rule_for(q->method, q->path);
  if (!rule) {
    api_log("auth: no rule for %s %s", q->method, q->path);
    api_json(q->fd, 404, "{\"error\":\"not found\"}");
    return true;
  }
  if (rule->rule == AP_PUBLIC || rule->rule == AP_GUEST) return false;
  const auth_principal *p = auth_principal_of(q);
  int verdict;
  if (!p) verdict = current.refused == 400 ? AP_TWO_CREDENTIALS : AP_LOG_IN;
  else verdict = (int)ap_admit(rule->rule, p->kind, p->level, p->l2_age_ms, p->admin, false);
  if (verdict == AP_OK) return false;
  api_log("auth: refused %s %s: rule %s, %s", q->method, q->path, rule_name(rule->rule),
          !p ? "a refused credential" : p->kind == AP_PERSON ? "person" : p->kind == AP_AGENT ? "agent" : p->kind == AP_SERVICE ? "service" : p->kind == AP_KIND_GUEST ? "guest" : "nobody");
  coax_string w = ap_words(verdict);
  char words[200]; snprintf(words, sizeof words, "%.*s", (int)w.count, w.data);
  refuse(q, (int)ap_status(verdict), words);
  return true;
}

// ---------------------------------------------------------------- logging in
// The browser's name for the devices list, from its User-Agent: "Chrome on Mac" and the like.
static void device_name(const request *q, char *out, size_t cap) {
  char ua[512]; api_header(q, "User-Agent", ua, sizeof ua);
  const char *b = strstr(ua, "Edg/") ? "Edge" : strstr(ua, "Firefox/") ? "Firefox" : strstr(ua, "Chrome/") ? "Chrome"
                : strstr(ua, "Safari/") ? "Safari" : strstr(ua, "OpenAgents") ? "OpenAgents app" : "Browser";
  const char *o = strstr(ua, "iPhone") || strstr(ua, "iPad") ? "iPhone" : strstr(ua, "Android") ? "Android" : strstr(ua, "Mac OS") ? "Mac"
                : strstr(ua, "Windows") ? "Windows" : strstr(ua, "Linux") ? "Linux" : "";
  if (*o) snprintf(out, cap, "%s on %s", b, o); else snprintf(out, cap, "%s", b);
}

// A logged-in answer: a session (cookie for the web, a token pair for an app), the guest's player
// attached, the audit row. made: the account is new. Answers the request.
static void logged_in(const request *q, PGconn *db, const char *acct, const char *via_kind, const char *via_id, const char *level,
                      bool app, bool made, const char *action) {
  char tok[160], dig[65], name[96], country[8] = "";
  device_name(q, name, sizeof name);
  api_header(q, "X-Client-Region", country, sizeof country);
  if (app) { if (!random_hex(tok, 32)) { refuse(q, 503, "No randomness here. Try again."); return; } sha_hex(tok, dig); }
  else if (!new_token("oas_", tok, sizeof tok, dig)) { refuse(q, 503, "No randomness here. Try again."); return; }
  const char *v[8] = {dig, acct, via_kind, via_id, level, app ? "app" : "web", name, country};
  PGresult *r = PQexecParams(db, "SELECT auth.session_new($1, $2, $3, $4::bigint, $5, $6, $7, $8), a.team, a.verse_account "
                                 "FROM auth.accounts a WHERE a.id = $2", 8, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetisnull(r, 0, 0)) {
    api_log("auth: session for %s: %s", action, PQerrorMessage(db));
    PQclear(r);
    audit(db, acct, NULL, action, "refused", "{\"why\":\"frozen or gone\"}");
    refuse(q, 403, "This account is frozen. Log in with your key to unfreeze it.");
    return;
  }
  char sid[32], vacct[160]; bool team = !strcmp(PQgetvalue(r, 0, 1), "t");
  snprintf(sid, sizeof sid, "%s", PQgetvalue(r, 0, 0)); snprintf(vacct, sizeof vacct, "%s", PQgetvalue(r, 0, 2));
  PQclear(r);
  // The guest's player joins the account once.
  char player[160], pjson[600] = "null";
  api_header(q, "X-Verse-Player", player, sizeof player);
  if (player[0]) {
    const char *g[2] = {vacct, player};
    PGresult *x = PQexecParams(db, "SELECT auth.attach_guest($1, $2)::text", 2, NULL, g, NULL, NULL, 0);
    if (PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1 && !PQgetisnull(x, 0, 0)) snprintf(pjson, sizeof pjson, "%s", PQgetvalue(x, 0, 0));
    PQclear(x);
  }
  char detail[200]; snprintf(detail, sizeof detail, "{\"via\":\"%s\",\"client\":\"%s\",\"new\":%s}", via_kind, app ? "app" : "web", made ? "true" : "false");
  audit(db, acct, sid, action, "ok", detail);
  char json[1600], lines[600], headers[900];
  if (app) {
    char rt[160], rtd[65], at[160], atd[65], fam[33];
    if (!new_token("oa_rt_", rt, sizeof rt, rtd) || !new_token("oa_at_", at, sizeof at, atd) || !random_hex(fam, 16)) { refuse(q, 503, "No randomness here. Try again."); return; }
    const char *w[4] = {sid, fam, rtd, atd};
    PQclear(PQexecParams(db, "SELECT auth.app_pair($1, $2, $3, $4)", 4, NULL, w, NULL, NULL, 0));
    snprintf(json, sizeof json, "{\"state\":\"logged-in\",\"account\":\"%s\",\"new\":%s,\"access_token\":\"%s\",\"refresh_token\":\"%s\",\"expires_in\":3600,\"player\":%s}",
             acct, made ? "true" : "false", at, rt, pjson);
    snprintf(lines, sizeof lines, "state logged-in\naccount %s\nnew %d\naccess_token %s\nrefresh_token %s\nexpires_in 3600\n", acct, made, at, rt);
    answer(q, 200, NULL, json, lines);
    return;
  }
  set_cookie[0] = 0;   // this answer's own cookie replaces a cleared or rotated one
  snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n"
           "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n%s",
           SESSION_COOKIE, tok, SESSION_MAX_AGE, PENDING_COOKIE, team ? TEAM_COOKIE : "");
  snprintf(json, sizeof json, "{\"state\":\"logged-in\",\"account\":\"%s\",\"new\":%s,\"player\":%s}", acct, made ? "true" : "false", pjson);
  snprintf(lines, sizeof lines, "state logged-in\naccount %s\nnew %d\n", acct, made);
  answer(q, 200, headers, json, lines);
}

// Our Origin on a cookie write (as every route here); apps and tools send no Origin.
static bool same_site(const request *q) {
  char from[256];
  if (!api_header(q, "Origin", from, sizeof from)) return true;
  return !strcmp(from, origin());
}

static char *body_text(const request *q) {
  char *b = malloc(q->body_len + 1);
  if (!b) return NULL;
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  return b;
}

// ---- email
// The 6-digit code a link token stands for: the same message carries both, so either one shown is
// no more than the email already said.
static void code_of(const char *link_token, char code[7]) {
  char s[200], h[65]; snprintf(s, sizeof s, "code:%s", link_token); sha_hex(s, h);
  unsigned long v = strtoul((char[9]){h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 0}, NULL, 16);
  snprintf(code, 7, "%06lu", v % 1000000UL);
}

static void email_start(const request *q, PGconn *db, const char *body) {
  char raw[400], email[300], client[16] = "web";
  json_str(body, strlen(body), "client", client, sizeof client);
  bool app = !strcmp(client, "app");
  bool link = json_true(body, strlen(body), "link");
  if (!json_str(body, strlen(body), "email", raw, sizeof raw)) { refuse(q, 400, "Type your email address."); return; }
  int64_t n = as_email_normalize((coax_v_u8){(uint8_t *)raw, (int64_t)strlen(raw)}, (coax_v_u8){(uint8_t *)email, (int64_t)sizeof email - 1});
  if (n < 0) { refuse(q, 400, "That isn't an email address."); return; }
  email[n] = 0;
  if (!sealing_ready()) { refuse(q, 503, "Email login isn't set up here yet."); return; }
  char all[400], lookup[80];
  if (!lookups(email, all, sizeof all, lookup)) { refuse(q, 503, "Email login isn't set up here yet."); return; }
  // Linking needs a logged-in account; logging in needs none.
  const auth_principal *p = auth_principal_of(q);
  if (link && (!p || !p->account[0])) { refuse(q, 401, "Log in first."); return; }
  // Limits: per address (hour and day) and per network address; a new address is a new account.
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (!limit_hit(db, L_EMAIL_IP, ipk, &until)) { refuse_limit(q, L_EMAIL_IP, until); return; }
  if (!limit_hit(db, L_EMAIL_HOUR, lookup, &until)) { refuse_limit(q, L_EMAIL_HOUR, until); return; }
  if (!limit_hit(db, L_EMAIL_DAY, lookup, &until)) { refuse_limit(q, L_EMAIL_DAY, until); return; }
  const char *lv[1] = {all};
  PGresult *r = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM auth.emails WHERE lookup = ANY($1::text[]))", 1, NULL, lv, NULL, NULL, 0);
  bool known = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  if (!known && !link && !limit_hit(db, L_NEW_ACCOUNT, ipk, &until)) { refuse_limit(q, L_NEW_ACCOUNT, until); return; }
  // The pending login: the code and link only as digests; the address sealed to this row.
  char id[33], link_token[65], code[7], pending[65], code_sha[65], link_sha[65], pend_sha[65], sealed[1200], ad[96], tmp[120];
  if (!random_hex(id, 16) || !random_hex(link_token, 32) || !random_hex(pending, 32)) { refuse(q, 503, "No randomness here. Try again."); return; }
  code_of(link_token, code);
  snprintf(tmp, sizeof tmp, "%s%s", code, id); sha_hex(tmp, code_sha);
  sha_hex(link_token, link_sha); sha_hex(pending, pend_sha);
  snprintf(ad, sizeof ad, "auth.pending|target_sealed|%s", id);
  if (!seal(ad, email, sealed, sizeof sealed)) { refuse(q, 503, "Email login isn't set up here yet."); return; }
  const char *v[9] = {id, lookup, sealed, code_sha, link_sha, pend_sha, app ? "app" : "web", link ? "link" : "login", link ? p->account : NULL};
  r = PQexecParams(db, "SELECT auth.email_start($1, $2, $3, $4, $5, $6, $7, $8, $9)", 9, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("auth: email start: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) { refuse(q, 500, "That didn't work. Try again."); return; }
  // One message: the code, and the link for the same browser. Short, plain.
  char message[2400], to[600], url[400];
  json_esc(email, to, sizeof to);
  snprintf(url, sizeof url, "%s/login/email#t=%s", origin(), link_token);
  snprintf(message, sizeof message,
           "{\"to\":\"%s\",\"subject\":\"Your OpenAgents code: %.3s %.3s\",\"idempotency_key\":\"login-%s\","
           "\"text\":\"Your code is %.3s %.3s.\\n\\nOr open this link in the same browser:\\n%s\\n\\nIt works for 10 minutes. If you didn't ask for it, ignore this email.\\n\"}",
           to, code, code + 3, id, code, code + 3, url);
  char sent_id[128], err[300];
  int s = email_send(db, message, sent_id, err, sizeof err);
  memset(message, 0, sizeof message); memset(code, 0, sizeof code); memset(link_token, 0, sizeof link_token);
  if (s > 1) {
    audit(db, link ? p->account : NULL, NULL, "email.code", "failed", "{\"why\":\"not sent\"}");
    refuse(q, s == 400 ? 400 : 502, s == 400 ? "We can't send email to that address." : "The email didn't send. Try again.");
    return;
  }
  audit(db, link ? p->account : NULL, link ? p->session : NULL, "email.code", "ok", link ? "{\"purpose\":\"link\"}" : "{\"purpose\":\"login\"}");
  char headers[300] = "", json[400], lines[300];
  if (app) {
    snprintf(json, sizeof json, "{\"state\":\"sent\",\"expires_in\":600,\"tries\":5,\"pending\":\"%s\"}", pending);
    snprintf(lines, sizeof lines, "state sent\nexpires_in 600\ntries 5\npending %s\n", pending);
  } else {
    snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n", PENDING_COOKIE, pending);
    snprintf(json, sizeof json, "{\"state\":\"sent\",\"expires_in\":600,\"tries\":5}");
    snprintf(lines, sizeof lines, "state sent\nexpires_in 600\ntries 5\n");
  }
  answer(q, 200, headers, json, lines);
}

// The pending value: the cookie in a browser, the body's "pending" from an app.
static bool pending_sha(const request *q, const char *body, char out[65]) {
  char v[80];
  cookie(q, PENDING_COOKIE, v, sizeof v);
  if (!v[0]) json_str(body, strlen(body), "pending", v, sizeof v);
  if (strlen(v) != 64) return false;
  sha_hex(v, out);
  return true;
}

// Finishes a proved email (code or link): the account it logs in to (a new one for a new address)
// or the account it links to. Answers.
static void email_finish(const request *q, PGconn *db, PGresult *r) {
  // columns: state, id, sealed, lookup, purpose, account, client
  char id[40], sealed_p[1200], lookup[80], purpose[8], link_acct[40], client[8], email[300], ad[96];
  snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 1)); snprintf(sealed_p, sizeof sealed_p, "%s", PQgetvalue(r, 0, 2));
  snprintf(lookup, sizeof lookup, "%s", PQgetvalue(r, 0, 3)); snprintf(purpose, sizeof purpose, "%s", PQgetvalue(r, 0, 4));
  snprintf(link_acct, sizeof link_acct, "%s", PQgetvalue(r, 0, 5)); snprintf(client, sizeof client, "%s", PQgetvalue(r, 0, 6));
  snprintf(ad, sizeof ad, "auth.pending|target_sealed|%s", id);
  if (!unseal(ad, sealed_p, email, sizeof email)) { refuse(q, 500, "That didn't work. Ask for a new code."); return; }
  char lower[300], team_sha[65];   // team.conf names team emails by the SHA-256 of the lower-cased address
  size_t k = 0; for (; email[k] && k + 1 < sizeof lower; k++) lower[k] = (char)tolower((unsigned char)email[k]);
  lower[k] = 0;
  sha_hex(lower, team_sha); memset(lower, 0, sizeof lower);
  const char *tv[1] = {team_sha};
  PGresult *t = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM analytics.team WHERE kind = 'email_sha256' AND value = $1)", 1, NULL, tv, NULL, NULL, 0);
  bool team = PQresultStatus(t) == PGRES_TUPLES_OK && PQntuples(t) == 1 && !strcmp(PQgetvalue(t, 0, 0), "t");
  PQclear(t);
  PQclear(PQexec(db, "BEGIN"));
  char acct[40], email_id[24]; bool made = false;
  if (!strcmp(purpose, "link")) {
    const char *v[2] = {link_acct, lookup};
    PGresult *x = PQexecParams(db, "SELECT state, email_id FROM auth.email_attach($1, $2)", 2, NULL, v, NULL, NULL, 0);
    bool ok = PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1;
    char state[16] = ""; if (ok) { snprintf(state, sizeof state, "%s", PQgetvalue(x, 0, 0)); snprintf(email_id, sizeof email_id, "%s", PQgetvalue(x, 0, 1)); }
    PQclear(x);
    if (!ok || strcmp(state, "ok")) {
      PQclear(PQexec(db, "ROLLBACK"));
      audit(db, link_acct, NULL, "email.link", "refused", "{\"why\":\"another account's\"}");
      if (!strcmp(state, "already")) { answer(q, 200, NULL, "{\"state\":\"linked\"}", "state linked\n"); return; }
      refuse(q, 409, "This email already has an account. Log in to it to merge.");
      return;
    }
    snprintf(acct, sizeof acct, "%s", link_acct);
  } else {
    const char *v[2] = {lookup, team ? "t" : "f"};
    PGresult *x = PQexecParams(db, "SELECT acct, email_id, made FROM auth.email_account($1, $2::boolean)", 2, NULL, v, NULL, NULL, 0);
    bool ok = PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1;
    if (ok) { snprintf(acct, sizeof acct, "%s", PQgetvalue(x, 0, 0)); snprintf(email_id, sizeof email_id, "%s", PQgetvalue(x, 0, 1)); made = !strcmp(PQgetvalue(x, 0, 2), "t"); }
    else api_log("auth: email account: %s", PQerrorMessage(db));
    PQclear(x);
    if (!ok) { PQclear(PQexec(db, "ROLLBACK")); refuse(q, 500, "That didn't work. Try again."); return; }
  }
  // Sealed to its account (auth.emails|sealed|<account>), in the same transaction as the row.
  if (made || !strcmp(purpose, "link")) {
    char sealed[1200], ad2[96];
    snprintf(ad2, sizeof ad2, "auth.emails|sealed|%s", acct);
    bool ok = seal(ad2, email, sealed, sizeof sealed);
    if (ok) { const char *v[2] = {email_id, sealed}; PGresult *x = PQexecParams(db, "SELECT auth.email_seal($1::bigint, $2)", 2, NULL, v, NULL, NULL, 0); ok = PQresultStatus(x) == PGRES_TUPLES_OK; PQclear(x); }
    if (!ok) { PQclear(PQexec(db, "ROLLBACK")); refuse(q, 500, "That didn't work. Try again."); return; }
  }
  PQclear(PQexec(db, "COMMIT"));
  memset(email, 0, sizeof email);
  if (!strcmp(purpose, "link")) {
    audit(db, acct, NULL, "email.link", "ok", "{}");
    answer(q, 200, "Set-Cookie: " PENDING_COOKIE "=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n", "{\"state\":\"linked\"}", "state linked\n");
    return;
  }
  logged_in(q, db, acct, "email", email_id, "L1", !strcmp(client, "app"), made, "login.email");
}

static void email_verify(const request *q, PGconn *db, const char *body) {
  char code[16], psha[65];
  json_str(body, strlen(body), "code", code, sizeof code);
  // "482 913" as typed: the space goes.
  char c[8]; size_t k = 0;
  for (const char *s = code; *s && k < 7; s++) if (*s != ' ') c[k++] = *s;
  c[k] = 0;
  if (!ap_code_ok((coax_string){c, (int64_t)k})) { refuse(q, 400, "Type the 6 digits from the email."); return; }
  if (!pending_sha(q, body, psha)) { refuse(q, 409, "Ask for a code in this browser first."); return; }
  const char *v[2] = {psha, c};
  PGresult *r = PQexecParams(db, "SELECT state, id, sealed, lookup, purpose, account, client, tries_left FROM auth.email_code($1, $2)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("auth: email code: %s", PQerrorMessage(db)); PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  const char *state = PQgetvalue(r, 0, 0);
  if (!strcmp(state, "ok")) { email_finish(q, db, r); PQclear(r); return; }
  if (!strcmp(state, "wrong")) {
    int left = atoi(PQgetvalue(r, 0, 7)); PQclear(r);
    audit(db, NULL, NULL, "login.email", "refused", "{\"why\":\"wrong code\"}");
    char words[120]; snprintf(words, sizeof words, "That code isn't right. %d %s left.", left, left == 1 ? "try" : "tries");
    refuse(q, 401, words);
    return;
  }
  bool dead = !strcmp(state, "dead");
  PQclear(r);
  audit(db, NULL, NULL, "login.email", "refused", dead ? "{\"why\":\"code spent\"}" : "{\"why\":\"no code\"}");
  refuse(q, dead ? 410 : 409, dead ? "That code has ended: 5 tries or 10 minutes. Ask for a new one." : "Ask for a code in this browser first.");
}

static void email_link(const request *q, PGconn *db, const char *body) {
  char t[80], tsha[65], psha[65] = "";
  if (!json_str(body, strlen(body), "t", t, sizeof t) || strlen(t) != 64) { refuse(q, 400, "That link isn't whole. Type the code instead."); return; }
  sha_hex(t, tsha);
  pending_sha(q, body, psha);
  const char *v[2] = {tsha, psha[0] ? psha : NULL};
  PGresult *r = PQexecParams(db, "SELECT state, id, sealed, lookup, purpose, account, client FROM auth.email_link($1, $2)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  const char *state = PQgetvalue(r, 0, 0);
  if (!strcmp(state, "ok")) { email_finish(q, db, r); PQclear(r); return; }
  if (!strcmp(state, "elsewhere")) {
    PQclear(r);
    char code[7], json[200], lines[200];
    code_of(t, code);
    snprintf(json, sizeof json, "{\"state\":\"elsewhere\",\"code\":\"%.3s %.3s\",\"error\":\"Type this code in the window where you asked: %.3s %.3s\"}", code, code + 3, code, code + 3);
    snprintf(lines, sizeof lines, "state elsewhere\ncode %.3s %.3s\n", code, code + 3);
    answer(q, 409, NULL, json, lines);
    return;
  }
  bool dead = !strcmp(state, "dead");
  PQclear(r);
  refuse(q, 410, dead ? "That link has ended. Ask for a new code." : "That link was used or has ended. Ask for a new code.");
}

// ---- Nostr
static void nostr_challenge(const request *q, PGconn *db, const char *body) {
  char client[16] = "web", id[33], ch[65], chsha[65];
  json_str(body, strlen(body), "client", client, sizeof client);
  if (!random_hex(id, 16) || !random_hex(ch, 32)) { refuse(q, 503, "No randomness here. Try again."); return; }
  sha_hex(ch, chsha);
  const char *v[3] = {id, chsha, !strcmp(client, "app") ? "app" : "web"};
  PGresult *r = PQexecParams(db, "SELECT auth.nostr_challenge($1, $2, $3)", 3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK; PQclear(r);
  if (!ok) { refuse(q, 500, "That didn't work. Try again."); return; }
  char json[400], lines[300];
  snprintf(json, sizeof json, "{\"challenge\":\"%s\",\"url\":\"%s/api/auth/nostr\",\"method\":\"POST\",\"body\":\"{\\\"challenge\\\":\\\"%s\\\"}\",\"expires_in\":300}", ch, origin(), ch);
  snprintf(lines, sizeof lines, "challenge %s\nurl %s/api/auth/nostr\nexpires_in 300\n", ch, origin());
  answer(q, 200, NULL, json, lines);
}

// The account a key logs in to (made on first login, D8) and its key row's id. A second statement
// for the id: the function's insert isn't visible to the statement that called it.
static bool key_account(PGconn *db, const char *pk, const char *label, char acct[40], char kid[24], bool *made) {
  const char *v[2] = {pk, label};
  PGresult *r = PQexecParams(db, "SELECT acct, made FROM auth.account_for_key($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) { snprintf(acct, 40, "%s", PQgetvalue(r, 0, 0)); *made = !strcmp(PQgetvalue(r, 0, 1), "t"); }
  else api_log("auth: key account: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) return false;
  r = PQexecParams(db, "SELECT id FROM auth.nostr_keys WHERE pubkey = $1", 1, NULL, v, NULL, NULL, 0);
  ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) snprintf(kid, 24, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  return ok;
}

// The account a key logs in to (made on first login, D8), then logged in at L2.
static void key_login(const request *q, PGconn *db, const char *pk, const char *label, bool app, const char *action) {
  const char *kv[1] = {pk};
  PGresult *r = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM auth.nostr_keys WHERE pubkey = $1)", 1, NULL, kv, NULL, NULL, 0);
  bool known = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (!known && !limit_hit(db, L_NEW_ACCOUNT, ipk, &until)) { refuse_limit(q, L_NEW_ACCOUNT, until); return; }
  char acct[40], kid[24]; bool made = false;
  if (!key_account(db, pk, label, acct, kid, &made)) { refuse(q, 500, "That didn't work. Try again."); return; }
  logged_in(q, db, acct, "nostr", kid, "L2", app, made, action);
}

static void nostr_login(const request *q, PGconn *db, const char *body) {
  char ch[80], chsha[65];
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (!limit_hit(db, L_KEY_LOGIN, ipk, &until)) { refuse_limit(q, L_KEY_LOGIN, until); return; }
  const auth_principal *p = auth_principal_of(q);
  if (!p) { refuse(q, current.refused == 400 ? 400 : 401, current.refused == 400 ? "Log out first, or send only the signature." : "That signature isn't good for this request."); return; }
  if (p->credential != AUTH_NOSTR) { refuse(q, 401, "Sign the challenge with your key (NIP-98)."); return; }
  if (!p->payload) { refuse(q, 401, "Sign the body too (a payload tag)."); return; }
  if (!json_str(body, strlen(body), "challenge", ch, sizeof ch) || strlen(ch) != 64) { refuse(q, 400, "Ask for a challenge first."); return; }
  sha_hex(ch, chsha);
  const char *v[2] = {chsha, p->event_id};
  PGresult *r = PQexecParams(db, "SELECT state, client FROM auth.nostr_spend($1, $2)", 2, NULL, v, NULL, NULL, 0);
  char state[16] = "", client[8] = "web";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(client, sizeof client, "%s", PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (strcmp(state, "ok")) {
    audit(db, p->account, NULL, "login.nostr", "refused", !strcmp(state, "replay") ? "{\"why\":\"replay\"}" : "{\"why\":\"no challenge\"}");
    refuse(q, 401, !strcmp(state, "replay") ? "That signature was used already." : "That challenge has ended. Ask for a new one.");
    return;
  }
  char label[32] = "";
  json_str(body, strlen(body), "label", label, sizeof label);
  if (strcmp(label, "extension") && strcmp(label, "bunker") && strcmp(label, "phone") && strcmp(label, "app")) label[0] = 0;
  char pk[65]; memcpy(pk, p->pubkey, 65);
  key_login(q, db, pk, label, !strcmp(client, "app"), "login.nostr");
}

// ---- the phone approves the web
static void phone_start(const request *q, PGconn *db) {
  char id[33], ch[65], poll[65], pend[65], chsha[65], pollsha[65], pendsha[65];
  if (!random_hex(id, 16) || !random_hex(ch, 32) || !random_hex(poll, 32) || !random_hex(pend, 32)) { refuse(q, 503, "No randomness here. Try again."); return; }
  sha_hex(ch, chsha); sha_hex(poll, pollsha); sha_hex(pend, pendsha);
  const char *v[4] = {id, chsha, pollsha, pendsha};
  PGresult *r = PQexecParams(db, "SELECT auth.phone_start($1, $2, $3, $4)", 4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK; PQclear(r);
  if (!ok) { refuse(q, 500, "That didn't work. Try again."); return; }
  char headers[300], json[600], lines[500];
  snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=300; HttpOnly; Secure; SameSite=Lax\r\n", PENDING_COOKIE, pend);
  snprintf(json, sizeof json, "{\"challenge\":\"%s\",\"poll\":\"%s\",\"url\":\"%s/login#c=%s\",\"expires_in\":300}", ch, poll, origin(), ch);
  snprintf(lines, sizeof lines, "challenge %s\npoll %s\nurl %s/login#c=%s\nexpires_in 300\n", ch, poll, origin(), ch);
  answer(q, 200, headers, json, lines);
}
static void phone_approve(const request *q, PGconn *db, const char *body) {
  const auth_principal *p = auth_principal_of(q);
  if (!p || p->credential != AUTH_NOSTR || !p->payload) { refuse(q, 401, "Sign the approval with the app's key (NIP-98, with the body)."); return; }
  const char *e[1] = {p->event_id};
  PGresult *r = PQexecParams(db, "SELECT auth.nip98_once($1)", 1, NULL, e, NULL, NULL, 0);
  bool once = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  if (!once) { refuse(q, 401, "That signature was used already."); return; }
  char ch[80], chsha[65];
  if (!json_str(body, strlen(body), "challenge", ch, sizeof ch) || strlen(ch) != 64) { refuse(q, 400, "Scan the code on the page again."); return; }
  sha_hex(ch, chsha);
  const char *v[2] = {chsha, p->pubkey};
  r = PQexecParams(db, "SELECT auth.phone_approve($1, $2)", 2, NULL, v, NULL, NULL, 0);
  char state[16] = ""; if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  if (!strcmp(state, "ok")) { audit(db, p->account, NULL, "login.phone_approve", "ok", "{}"); answer(q, 200, NULL, "{\"state\":\"approved\"}", "state approved\n"); return; }
  refuse(q, !strcmp(state, "already") ? 409 : 410, !strcmp(state, "already") ? "That code was approved already." : "That code has ended. Show a new one on the page.");
}
static void phone_poll(const request *q, PGconn *db, const char *body) {
  char poll[80], pollsha[65], psha[65];
  if (!json_str(body, strlen(body), "poll", poll, sizeof poll) || strlen(poll) != 64) { refuse(q, 400, "Show a code first."); return; }
  if (!pending_sha(q, body, psha)) { refuse(q, 409, "Show a code in this browser first."); return; }
  sha_hex(poll, pollsha);
  const char *v[2] = {pollsha, psha};
  PGresult *r = PQexecParams(db, "SELECT state, coalesce(pubkey, '') FROM auth.phone_poll($1, $2)", 2, NULL, v, NULL, NULL, 0);
  char state[16] = "", pk[65] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(pk, sizeof pk, "%s", PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (!strcmp(state, "waiting")) { answer(q, 200, NULL, "{\"state\":\"waiting\"}", "state waiting\n"); return; }
  if (!strcmp(state, "ok")) { key_login(q, db, pk, "phone", false, "login.phone"); return; }
  refuse(q, 410, "That code has ended. Show a new one.");
}

// ---- app tokens
static void refresh(const request *q, PGconn *db, const char *body) {
  char rt[200], dig[65];
  if (!json_str(body, strlen(body), "refresh_token", rt, sizeof rt) || !token_ok(rt, TOKEN_REFRESH, dig)) { refuse(q, 401, "Log in again."); return; }
  long long until = 0;
  // Limit per family: the family is found from the token; a digest stands in until then.
  char nrt[160], nrtd[65], nat[160], natd[65];
  if (!new_token("oa_rt_", nrt, sizeof nrt, nrtd) || !new_token("oa_at_", nat, sizeof nat, natd)) { refuse(q, 503, "No randomness here. Try again."); return; }
  const char *fv[1] = {dig};
  PGresult *f = PQexecParams(db, "SELECT family FROM auth.refresh WHERE token_sha256 = $1", 1, NULL, fv, NULL, NULL, 0);
  char fam[40] = ""; if (PQresultStatus(f) == PGRES_TUPLES_OK && PQntuples(f) == 1) snprintf(fam, sizeof fam, "%s", PQgetvalue(f, 0, 0));
  PQclear(f);
  if (fam[0] && !limit_hit(db, L_REFRESH, fam, &until)) { refuse_limit(q, L_REFRESH, until); return; }
  const char *v[3] = {dig, nrtd, natd};
  PGresult *r = PQexecParams(db, "SELECT state, coalesce(account, '') FROM auth.app_refresh($1, $2, $3)", 3, NULL, v, NULL, NULL, 0);
  char state[16] = "", acct[40] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(acct, sizeof acct, "%s", PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (!strcmp(state, "ok")) {
    char json[600], lines[500];
    snprintf(json, sizeof json, "{\"access_token\":\"%s\",\"refresh_token\":\"%s\",\"expires_in\":3600}", nat, nrt);
    snprintf(lines, sizeof lines, "access_token %s\nrefresh_token %s\nexpires_in 3600\n", nat, nrt);
    answer(q, 200, NULL, json, lines);
    return;
  }
  if (!strcmp(state, "reused")) {
    // Someone copied a refresh token: the whole login ended. The notice to the account's other
    // ways in comes with linking notices (#13213); the audit row is here now.
    audit(db, acct, NULL, "refresh.reused", "refused", "{\"ended\":\"family\"}");
    refuse(q, 401, "This login was ended because its token was used twice. Log in again.");
    return;
  }
  refuse(q, 401, "Log in again.");
}

// ---- sessions
static void logout(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  char headers[200]; cookie_header(headers, sizeof headers, SESSION_COOKIE, "", 0);
  if (p && p->session[0]) {
    const char *v[1] = {p->session};
    PQclear(PQexecParams(db, "SELECT auth.session_end($1)", 1, NULL, v, NULL, NULL, 0));
    audit(db, p->account, p->session, "logout", "ok", "{}");
  }
  set_cookie[0] = 0;
  answer(q, 200, headers, "{\"state\":\"logged-out\"}", "state logged-out\n");
}
static void logout_everywhere(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT auth.sessions_end_all($1)", 1, NULL, v, NULL, NULL, 0);
  int n = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? atoi(PQgetvalue(r, 0, 0)) : 0;
  PQclear(r);
  char d[64]; snprintf(d, sizeof d, "{\"ended\":%d}", n);
  audit(db, p->account, p->session, "logout.everywhere", "ok", d);
  char headers[200], json[80], lines[80]; cookie_header(headers, sizeof headers, SESSION_COOKIE, "", 0);
  set_cookie[0] = 0;
  snprintf(json, sizeof json, "{\"state\":\"logged-out\",\"ended\":%d}", n);
  snprintf(lines, sizeof lines, "state logged-out\nended %d\n", n);
  answer(q, 200, headers, json, lines);
}
static void sessions(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  const char *v[2] = {p->account, p->session};
  PGresult *r = PQexecParams(db, "SELECT auth.sessions_of($1, $2)::text", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  size_t n = PQgetlength(r, 0, 0) + 32;
  char *json = malloc(n);
  if (json) { snprintf(json, n, "{\"sessions\":%s}", PQgetvalue(r, 0, 0)); answer(q, 200, NULL, json, NULL); free(json); }
  PQclear(r);
}
static void session_remove(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  char sid[40];
  if (sscanf(q->path, "/api/auth/sessions/%39[a-z0-9_]/remove", sid) != 1) { refuse(q, 404, "Not found."); return; }
  const char *v[2] = {sid, p->account};
  PGresult *r = PQexecParams(db, "DELETE FROM auth.sessions WHERE id = $1 AND account = $2", 2, NULL, v, NULL, NULL, 0);
  bool gone = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  if (!gone) { refuse(q, 404, "No such session."); return; }
  audit(db, p->account, p->session, "session.remove", "ok", "{}");
  answer(q, 200, NULL, "{\"state\":\"removed\"}", "state removed\n");
}
static void freeze(const request *q, PGconn *db, const char *body) {
  char t[80], tsha[65], acct[40] = "";
  if (json_str(body, strlen(body), "t", t, sizeof t) && strlen(t) == 64) {
    sha_hex(t, tsha);
    const char *v[1] = {tsha};
    PGresult *r = PQexecParams(db, "SELECT auth.freeze_by_link($1)", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) snprintf(acct, sizeof acct, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    if (!acct[0]) { refuse(q, 410, "That link was used or has ended. Log in to freeze your account."); return; }
  } else {
    const auth_principal *p = auth_principal_of(q);
    if (!p || !p->account[0]) { refuse(q, 401, "Log in first, or use the link in the email."); return; }
    snprintf(acct, sizeof acct, "%s", p->account);
    const char *v[1] = {acct};
    PQclear(PQexecParams(db, "SELECT auth.freeze($1)", 1, NULL, v, NULL, NULL, 0));
  }
  audit(db, acct, NULL, "freeze", "ok", "{}");
  char headers[200]; cookie_header(headers, sizeof headers, SESSION_COOKIE, "", 0);
  set_cookie[0] = 0;
  answer(q, 200, headers, "{\"state\":\"frozen\"}", "state frozen\n");
}
// "This wasn't me": a new link for acct (for notices). The token goes into the notice only.
bool auth_freeze_link(PGconn *db, const char *acct, char token[65]) {
  char sha[65];
  if (!random_hex(token, 32)) return false;
  sha_hex(token, sha);
  const char *v[2] = {acct, sha};
  PGresult *r = PQexecParams(db, "SELECT auth.freeze_link($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  PQclear(r);
  return ok;
}

static void me(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  if (!p || !p->account[0]) {
    // A key that signs without an account, or nobody: not logged in (a 200, so pages ask without a console error).
    answer(q, 200, NULL, "{\"logged_in\":false}", "logged_in 0\n");
    return;
  }
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT x->>'kind', x->'emails', x->'keys', (x->>'team')::boolean FROM auth.me_of($1) x", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  char *json = malloc(16384), *lines = malloc(8192);
  if (!json || !lines) { free(json); free(lines); PQclear(r); refuse(q, 500, "Out of memory."); return; }
  size_t j = (size_t)snprintf(json, 16384, "{\"logged_in\":true,\"account\":\"%s\",\"kind\":\"%s\",\"level\":\"L%d\",\"via\":\"%s\",\"session\":\"%s\",\"admin\":%s,\"emails\":[",
                              p->account, PQgetvalue(r, 0, 0), p->level, p->via, p->session, p->admin ? "true" : "false");
  size_t l = (size_t)snprintf(lines, 8192, "logged_in 1\naccount %s\nkind %s\nlevel L%d\nvia %s\nadmin %d\n", p->account, PQgetvalue(r, 0, 0), p->level, p->via, p->admin);
  // Emails, opened and masked: a•••@example.com. The address itself never leaves.
  const char *emails = PQgetvalue(r, 0, 1);
  int count = 0;
  for (const char *s = strstr(emails, "\"sealed\": \""); s; s = strstr(s + 1, "\"sealed\": \"")) {
    s += 11;
    const char *e = strchr(s, '"'); if (!e) break;
    char sealed[1200], email[300], masked[320], ad[96];
    snprintf(sealed, sizeof sealed, "%.*s", (int)(e - s), s);
    snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
    if (!unseal(ad, sealed, email, sizeof email)) continue;
    mask_email(email, masked, sizeof masked);
    memset(email, 0, sizeof email);
    j += (size_t)snprintf(json + j, 16384 - j, "%s\"%s\"", count++ ? "," : "", masked);
    l += (size_t)snprintf(lines + l, 8192 - l, "email %s\n", masked);
  }
  j += (size_t)snprintf(json + j, 16384 - j, "],\"keys\":%s}", PQgetvalue(r, 0, 2));
  const char *keys = PQgetvalue(r, 0, 2);
  for (const char *s = strstr(keys, "\"pubkey\": \""); s && l < 8000; s = strstr(s + 1, "\"pubkey\": \"")) {
    s += 11;
    l += (size_t)snprintf(lines + l, 8192 - l, "key %.64s\n", s);
  }
  PQclear(r);
  answer(q, 200, NULL, json, lines);
  free(json); free(lines);
}

static void limits(const request *q) {
  char json[4096], lines[3000]; size_t j = 0, l = 0;
  j += (size_t)snprintf(json + j, sizeof json - j, "{\"limits\":[");
  for (int i = 0; i < L_COUNT; i++) {
    j += (size_t)snprintf(json + j, sizeof json - j, "%s{\"name\":\"%s\",\"what\":\"%s\",\"max\":%d,\"per\":\"%s\",\"window_ms\":%lld}",
                          i ? "," : "", LIMITS[i].name, LIMITS[i].what, LIMITS[i].max, LIMITS[i].per, LIMITS[i].window_ms);
    l += (size_t)snprintf(lines + l, sizeof lines - l, "limit %s|%d|%s|%s\n", LIMITS[i].name, LIMITS[i].max, LIMITS[i].per, LIMITS[i].what);
  }
  snprintf(json + j, sizeof json - j, "]}");
  answer(q, 200, "Cache-Control: public, max-age=300\r\n", json, lines);
}

// A key's login for another file's route (the admin page's signed GET, admin.c): a web session
// at L2, answered with its cookie and Location. false when it answered a refusal itself.
bool auth_login_key_redirect(const request *q, PGconn *db, const char *pk, const char *location) {
  char acct[40], kid[24]; bool made = false;
  if (!key_account(db, pk, "admin", acct, kid, &made)) { api_json(q->fd, 500, "{\"error\":\"no session\"}"); return false; }
  PGresult *r;
  char tok[160], dig[65], name[96];
  device_name(q, name, sizeof name);
  if (!new_token("oas_", tok, sizeof tok, dig)) { api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return false; }
  const char *s[8] = {dig, acct, "nostr", kid, "L2", "web", name, NULL};
  r = PQexecParams(db, "SELECT auth.session_new($1, $2, $3, $4::bigint, $5, $6, $7, $8)", 8, NULL, s, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  char sid[32] = ""; if (ok) snprintf(sid, sizeof sid, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  if (!ok) { api_json(q->fd, 403, "{\"error\":\"this account is frozen\"}"); return false; }
  audit(db, acct, sid, "admin.login", "ok", "{}");
  set_cookie[0] = 0;
  char headers[900];
  snprintf(headers, sizeof headers, "Location: %s\r\nSet-Cookie: %s=%s; Path=/; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n" TEAM_COOKIE,
           location, SESSION_COOKIE, tok, SESSION_MAX_AGE);
  api_respond(q->fd, 303, "text/plain", headers, "", 0);
  return true;
}

// ---------------------------------------------------------------- the routes
bool auth_api_route(const request *q) {
  if (strncmp(q->path, "/api/auth/", 10)) return false;
  const char *op = q->path + 10;
  bool get = !strcmp(q->method, "GET");
  static const char *const posts[] = {"email/start", "email/verify", "email/link", "nostr/challenge", "nostr", "phone/start", "phone/approve",
                                      "phone/poll", "refresh", "logout", "logout-everywhere", "freeze", NULL};
  bool known_post = false;
  for (const char *const *o = posts; *o; o++) if (!strcmp(op, *o)) known_post = true;
  bool remove = !strncmp(op, "sessions/", 9) && strstr(op, "/remove");
  if (!known_post && !remove && strcmp(op, "account") && strcmp(op, "limits") && strcmp(op, "sessions")) return false;   // the old routes, until #13207
  // POST /api/auth/logout is the phone sign-in's too (private.c, until #13207): ours when a session of ours asks.
  if (!strcmp(op, "logout")) { const auth_principal *p = auth_principal_of(q); if (!p || !p->session[0]) return false; }
  if (!strcmp(op, "limits")) { if (!get) { refuse(q, 405, "GET this."); return true; } limits(q); return true; }
  PGconn *db = api_db();
  if (!db) { refuse(q, 503, "The database isn't answering. Try again."); return true; }
  if (!strcmp(op, "account")) { if (!get) { refuse(q, 405, "GET this."); return true; } me(q, db); return true; }
  if (!strcmp(op, "sessions")) { if (!get) { refuse(q, 405, "GET this."); return true; } sessions(q, db); return true; }
  if (get || strcmp(q->method, "POST")) { refuse(q, 405, "POST this."); return true; }
  if (!same_site(q)) { refuse(q, 403, "Not from this site."); return true; }
  if (remove) { session_remove(q, db); return true; }
  char *body = body_text(q);
  if (!body) { refuse(q, 500, "Out of memory."); return true; }
  if (!strcmp(op, "email/start")) email_start(q, db, body);
  else if (!strcmp(op, "email/verify")) email_verify(q, db, body);
  else if (!strcmp(op, "email/link")) email_link(q, db, body);
  else if (!strcmp(op, "nostr/challenge")) nostr_challenge(q, db, body);
  else if (!strcmp(op, "nostr")) nostr_login(q, db, body);
  else if (!strcmp(op, "phone/start")) phone_start(q, db);
  else if (!strcmp(op, "phone/approve")) phone_approve(q, db, body);
  else if (!strcmp(op, "phone/poll")) phone_poll(q, db, body);
  else if (!strcmp(op, "refresh")) refresh(q, db, body);
  else if (!strcmp(op, "logout")) logout(q, db);
  else if (!strcmp(op, "logout-everywhere")) logout_everywhere(q, db);
  else if (!strcmp(op, "freeze")) freeze(q, db, body);
  free(body);
  return true;
}
