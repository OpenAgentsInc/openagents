// name@openagents.com Lightning Addresses (docs/auth/lightning-addresses.md, #13208): the LNURL-pay
// server and the routes that manage an address. The money never lands with us. An address either
// forwards to the person's own Lightning Address elsewhere (forward mode: we fetch its payRequest
// and its invoice from our server, so the payer's address never reaches it, and pass them on
// checked) or makes invoices for the person's own Spark wallet (spark mode, through the Breez
// LNURL backend at OA_LNADDR_SPARK_URL; while that isn't set, a spark address answers "This
// address isn't receiving yet"). Every decision is Coax: coax/lib/lnaddr/lnurl.coax (the LUDs,
// invoices, upstream answers, limits) and lnaddr-policy.coax (who may claim, change, cancel,
// turn off), linked as lnaddr-unit.c. This file gathers the facts and keeps the results.
//
// Public, for wallets (no credential; errors are LUD-06's {"status":"ERROR","reason"}):
//   GET /.well-known/lnurlp/<name[+tag]>      LUD-16 lookup
//   GET /lnurlp/<name[+tag]>                  the same, as a plain LNURL-pay URL
//   GET /lnurlp/<name[+tag]>/invoice?amount=<msat>[&comment=][&payerdata=][&nostr=]   the callback
//   GET /lnurlp/verify/<payment hash>         LUD-21
// The holder's (auth.c's rules: an account to read, a person to act, L2-fresh to re-point):
//   GET  /api/lightning/address               the card: the address, where it goes, the limits,
//                                             a waiting change and the notices
//   POST /api/lightning/address?name=N[&forward_to=A][&spark_pubkey=K&sig=S&nonce=X&expires=T]
//                                             claim N; with a destination it receives at once
//   POST /api/lightning/address/off | /on     at once; the name stays yours
//   POST /api/lightning/address/change?forward_to=A | ?spark_pubkey=K&sig=S&nonce=X&expires=T
//                                             waits 24 hours (72 for email-only accounts)
//   POST /api/lightning/address/cancel        cancels the waiting change
//
//   OA_API_ORIGIN          https://openagents.com or https://staging.openagents.com: the domain
//   OA_LNADDR_NETWORK      mainnet (default), testnet, signet or regtest: the invoices' network
//   OA_LNADDR_SPARK_URL    the Breez LNURL backend for spark mode, when it is deployed
//   OA_LNADDR_ZAP_PUBKEY   our zap receipt key (hex), advertised in spark mode only
#include "api.h"
#include "auth-policy.generated.h"
#include "lnaddr-policy.generated.h"

#include <arpa/inet.h>
#include <curl/curl.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <sys/random.h>
#endif

// lnurl.coax's refusal numbers this file names (LU_*); lnaddr_init checks them against the Coax.
enum { LU_OK = 0, LU_NO_ADDRESS = 1, LU_BAD_NAME = 2, LU_NOT_RECEIVING = 3, LU_TOO_SMALL = 8, LU_TOO_BIG = 9,
       LU_AMOUNT = 7, LU_NOTE_LONG = 12, LU_PAYERDATA = 14, LU_ZAP = 15, LU_LOOKUP_LIMIT = 17, LU_NAME_LIMIT = 18,
       LU_IP_LIMIT = 19, LU_UPSTREAM = 20, LU_UPSTREAM_BAD = 21, LU_UPSTREAM_REFUSED = 22, LU_INVOICE = 23, LU_NOT_FOUND = 24 };
enum { LA_OK = 0, LA_NAME = 3, LA_TARGET = 11, LA_TARGET_DOWN = 12, LA_NO_ADDRESS = 9 };
enum { LA_ACTIVE = 0, LA_OFF = 1, LA_SUSPENDED = 2, LA_RETIRED = 3, LA_NONE = 0, LA_SPARK = 1, LA_FORWARD = 2 };
#define LU_MIN_MSAT 1000LL
#define LU_COMMENT_MAX 280
#define UPSTREAM_MAX 65536
#define UPSTREAM_TIMEOUT_MS 5000L
#define CACHE_MS 60000.0

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static const char *domain(void) {
  const char *o = env_or("OA_API_ORIGIN", "https://openagents.com");
  return !strncmp(o, "https://", 8) ? o + 8 : !strncmp(o, "http://", 7) ? o + 7 : o;   // http only in test.sh
}
static int network(void) {
  const char *n = env_or("OA_LNADDR_NETWORK", "mainnet");
  return !strcmp(n, "testnet") ? 2 : !strcmp(n, "signet") ? 3 : !strcmp(n, "regtest") ? 4 : 1;
}
static bool spark_backend(void) { return getenv("OA_LNADDR_SPARK_URL") && *getenv("OA_LNADDR_SPARK_URL"); }
static coax_string cs(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }
static coax_v_u8 cv(void *p, size_t n) { return (coax_v_u8){(uint8_t *)p, (int64_t)n}; }
static void hex_of(const unsigned char *b, size_t n, char *out) {
  static const char d[] = "0123456789abcdef";
  for (size_t i = 0; i < n; i++) { out[2 * i] = d[b[i] >> 4]; out[2 * i + 1] = d[b[i] & 15]; }
  out[2 * n] = 0;
}
static bool unhex(const char *s, unsigned char *out, size_t n) {
  if (strlen(s) != 2 * n) return false;
  for (size_t i = 0; i < n; i++) {
    int v = 0;
    for (int k = 0; k < 2; k++) {
      char c = s[2 * i + k];
      int d = c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : -1;
      if (d < 0) return false;
      v = v * 16 + d;
    }
    out[i] = (unsigned char)v;
  }
  return true;
}

// ---------------------------------------------------------------- answers
#define NO_STORE "Cache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\n"
static void reply(const request *q, int status, const char *json) {
  api_respond(q->fd, status, "application/json", NO_STORE, json, strlen(json));
}
static void reply_n(const request *q, int status, const uint8_t *json, int64_t n) {
  if (n < 0) { reply(q, 500, "{\"status\":\"ERROR\",\"reason\":\"That didn't work. Try again.\"}"); return; }
  api_respond(q->fd, status, "application/json", NO_STORE, json, (size_t)n);
}
// LUD-06's error with lnurl.coax's words; limit is the msat or characters the words name.
static void lnurl_error(const request *q, int status, int64_t code, int64_t limit) {
  uint8_t out[512];
  int64_t n = lu_error_json(code, limit, cv(out, sizeof out));
  reply_n(q, status, out, n);
}
// The management routes' refusal, lnaddr-policy.coax's words: {"error": "..."}.
static void la_refuse(const request *q, int status, int64_t code) {
  coax_string w = la_text(code);
  char json[400];
  snprintf(json, sizeof json, "{\"error\":\"%.*s\",\"code\":%lld}", (int)w.count, w.data, (long long)code);
  reply(q, status, json);
}
static void say(const request *q, int status, const char *words) {
  char json[400];
  snprintf(json, sizeof json, "{\"error\":\"%s\"}", words);
  reply(q, status, json);
}

// A query parameter decoded exactly: its length; -1 absent, -2 given twice, -3 bad (a % that
// isn't two hex digits, a NUL, or longer than cap - 1). '+' is a space, as forms send it.
static int qparam(const request *q, const char *name, char *out, size_t cap) {
  size_t n = strlen(name);
  int found = -1;
  for (const char *p = q->query; p && *p; ) {
    const char *amp = strchr(p, '&');
    size_t len = amp ? (size_t)(amp - p) : strlen(p);
    if (len >= n && !strncmp(p, name, n) && (len == n || p[n] == '=')) {
      if (found != -1) return -2;
      size_t o = 0;
      for (size_t i = n + 1; i < len; i++) {
        char c = p[i];
        if (c == '%') {
          if (i + 2 >= len) return -3;
          char hx[3] = {p[i + 1], p[i + 2], 0};
          char *end; long v = strtol(hx, &end, 16);
          if (end != hx + 2 || v == 0) return -3;
          c = (char)v; i += 2;
        } else if (c == '+') c = ' ';
        if (o + 1 >= cap) return -3;
        out[o++] = c;
      }
      out[o] = 0;
      found = (int)o;
    }
    p = amp ? amp + 1 : NULL;
  }
  if (found == -1 && cap) out[0] = 0;
  return found;
}

// ---------------------------------------------------------------- the network address, keyed
// The visitor's address: the load balancer's next to last entry of X-Forwarded-For (as auth.c).
// It is used only as HMAC-SHA256 under pay.secrets' key (la_ip_key), never stored or sent on.
static bool ip_key(const request *q, PGconn *db, char out[65]) {
  static unsigned char key[32]; static bool have;
  if (!have) {
    unsigned char fresh[32]; char hex[65];
    if (getentropy(fresh, sizeof fresh) != 0) return false;
    hex_of(fresh, 32, hex);
    const char *v[1] = {hex};
    PGresult *r = PQexecParams(db, "INSERT INTO pay.secrets (name, value) VALUES ('ip_key', $1) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING value",
                               1, NULL, v, NULL, NULL, 0);
    have = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && unhex(PQgetvalue(r, 0, 0), key, 32);
    PQclear(r);
    if (!have) return false;
  }
  char xff[512], ip[128] = "local";
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  if (np) snprintf(ip, sizeof ip, "%s", parts[np >= 2 ? np - 2 : 0]);
  unsigned char h[32];
  if (!la_ip_key(cv(key, 32), cs(ip), cv(h, 32))) return false;
  hex_of(h, 32, out);
  return true;
}
// One hit counted in auth.limit_counts (the shared rate-limit table) for a minute's window; the
// count including this hit, or -1 when counting failed (which never locks a payer out).
static long long hits(PGconn *db, const char *name, const char *key) {
  const char *v[2] = {name, key};
  PGresult *r = PQexecParams(db, "SELECT n FROM auth.limit_hit($1, $2, 60000, true)", 2, NULL, v, NULL, NULL, 0);
  long long n = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? atoll(PQgetvalue(r, 0, 0)) : -1;
  if (n < 0) api_log("lnaddr: limit %s: %s", name, PQerrorMessage(db));
  PQclear(r);
  return n;
}

// ---------------------------------------------------------------- fetching the forward target
// Only https, only public addresses (lu_ip_public at connect time, so a name that resolves to a
// private address is refused too), no redirects, no proxy, our own User-Agent and nothing of the
// payer's: no address, no headers, no cookies. 5 seconds, 64 KB.
typedef struct { char *p; size_t n; bool over; } fetched;
static size_t take(char *data, size_t size, size_t n, void *user) {
  fetched *b = user; size_t k = size * n;
  if (b->n + k > UPSTREAM_MAX) { b->over = true; return 0; }
  char *p = realloc(b->p, b->n + k + 1);
  if (!p) return 0;
  b->p = p; memcpy(b->p + b->n, data, k); b->n += k; b->p[b->n] = 0;
  return k;
}
static curl_socket_t open_public(void *refused, curlsocktype purpose, struct curl_sockaddr *a) {
  (void)purpose;
  bool ok = false;
  if (a->family == AF_INET) ok = lu_ip_public(4, cv(&((struct sockaddr_in *)&a->addr)->sin_addr, 4));
  else if (a->family == AF_INET6) ok = lu_ip_public(6, cv(&((struct sockaddr_in6 *)&a->addr)->sin6_addr, 16));
  if (!ok) { *(bool *)refused = true; return CURL_SOCKET_BAD; }
  return socket(a->family, a->socktype, a->protocol);
}
// GET url: the HTTP status (0 unreached or refused), the body into *out (malloc'd) and its length.
static long fetch(const char *url, char **out, size_t *len) {
  *out = NULL; *len = 0;
  CURL *h = curl_easy_init();
  if (!h) return 0;
  fetched b = {0}; bool refused = false;
  struct curl_slist *hd = curl_slist_append(NULL, "Accept: application/json");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_PROTOCOLS_STR, "https");
  curl_easy_setopt(h, CURLOPT_REDIR_PROTOCOLS_STR, "https");
  curl_easy_setopt(h, CURLOPT_FOLLOWLOCATION, 0L);
  curl_easy_setopt(h, CURLOPT_NOPROXY, "*");
  curl_easy_setopt(h, CURLOPT_USERAGENT, "OpenAgents-LNURL/1");
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_TIMEOUT_MS, UPSTREAM_TIMEOUT_MS);
  curl_easy_setopt(h, CURLOPT_CONNECTTIMEOUT_MS, UPSTREAM_TIMEOUT_MS);
  curl_easy_setopt(h, CURLOPT_MAXFILESIZE, (long)UPSTREAM_MAX);
  curl_easy_setopt(h, CURLOPT_OPENSOCKETFUNCTION, open_public);
  curl_easy_setopt(h, CURLOPT_OPENSOCKETDATA, &refused);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK && !b.over && !refused) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h);
  if (status == 0) { free(b.p); return 0; }
  *out = b.p ? b.p : calloc(1, 1); *len = b.n;
  return status;
}

// The target's payRequest, kept a minute per target so a payer's lookup and callback cost one
// fetch. Returns lnurl.coax's code; *text is the answer (owned by the cache) and u its reading.
typedef struct { char target[200]; char *text; double at; } cached;
static cached cache[32];
static int64_t upstream_pay(const char *forward_to, const char **text, coax_t_LuUpstream *u) {
  *text = NULL;
  double now = api_now_ms();
  unsigned h = 2166136261u;
  for (const char *c = forward_to; *c; c++) h = (h ^ (unsigned char)*c) * 16777619u;
  cached *slot = &cache[h % 32];
  if (!slot->text || strcmp(slot->target, forward_to) || now - slot->at > CACHE_MS) {
    uint8_t url[400];
    int64_t n = lu_target(cs(forward_to), cs(domain()), cv(url, sizeof url - 1));
    if (n < 0) return LU_UPSTREAM_BAD;
    url[n] = 0;
    char *body; size_t len;
    long status = fetch((const char *)url, &body, &len);
    if (status == 0) return LU_UPSTREAM;
    if (status != 200 && status != 400 && status != 404) { free(body); return LU_UPSTREAM_BAD; }
    free(slot->text);
    slot->text = body; slot->at = now;
    snprintf(slot->target, sizeof slot->target, "%s", forward_to);
  }
  *text = slot->text;
  return lu_upstream_pay((coax_string){slot->text, (int64_t)strlen(slot->text)}, (coax_v_t_LuUpstream){u, 1});
}

// ---------------------------------------------------------------- the address rows
typedef struct {
  bool found;
  char name[40], account[40], mode[12], state[12], forward_to[200], spark_pubkey[80];
  long long revision, max_msat;
  bool comments_on;
} address;
static int mode_of(const address *a) { return !strcmp(a->mode, "forward") ? LA_FORWARD : !strcmp(a->mode, "spark") ? LA_SPARK : LA_NONE; }
static int state_of(const address *a) {
  return !strcmp(a->state, "active") ? LA_ACTIVE : !strcmp(a->state, "off") ? LA_OFF : !strcmp(a->state, "suspended") ? LA_SUSPENDED : LA_RETIRED;
}
static void audit(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, const char *detail) {
  const char *v[5] = {acct && *acct ? acct : NULL, actor && *actor ? actor : NULL, action, result, detail ? detail : "{}"};
  PGresult *r = PQexecParams(db, "SELECT auth.log($1, $2, $3, $4, $5::jsonb)", 5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("lnaddr: audit %s: %s", action, PQerrorMessage(db));
  PQclear(r);
}
// A notice: kept on the address's card, and emailed to every verified address of the account.
static void notice(PGconn *db, const char *acct, const char *name, const char *kind, const char *detail, const char *text) {
  const char *v[4] = {acct, name, kind, detail ? detail : "{}"};
  PGresult *r = PQexecParams(db, "INSERT INTO pay.notices (account, name, kind, detail) VALUES ($1, $2, $3, $4::jsonb) RETURNING id", 4, NULL, v, NULL, NULL, 0);
  char id[24] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 0));
  else api_log("lnaddr: notice %s: %s", kind, PQerrorMessage(db));
  PQclear(r);
  if (!id[0] || !text) return;
  char subject[120], key[40];
  snprintf(subject, sizeof subject, "Your Lightning Address %s@%s", name, domain());
  snprintf(key, sizeof key, "lnaddr-%s", id);
  if (auth_notice(db, acct, key, subject, text) > 0) {
    const char *w[1] = {id};
    PQclear(PQexecParams(db, "UPDATE pay.notices SET emailed_at = now() WHERE id = $1::bigint", 1, NULL, w, NULL, NULL, 0));
  }
}
// Applies a waiting change whose time has come (lnaddr-policy.coax la_apply decides).
static void apply_due(PGconn *db, const char *name) {
  const char *v[1] = {name};
  PGresult *r = PQexecParams(db, "SELECT c.id, extract(epoch FROM c.ready_at)::bigint, extract(epoch FROM now())::bigint, c.account, c.new_mode, c.new_target, "
                                 "a.frozen_at IS NOT NULL OR a.deleted_at IS NOT NULL FROM pay.address_changes c JOIN auth.accounts a ON a.id = c.account "
                                 "WHERE c.name = $1 AND c.cancelled_at IS NULL AND c.applied_at IS NULL", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); return; }
  bool frozen = !strcmp(PQgetvalue(r, 0, 6), "t");   // a frozen account's change waits until it is unfrozen
  bool due = la_apply(atoll(PQgetvalue(r, 0, 2)), atoll(PQgetvalue(r, 0, 1)), frozen, false);
  char id[24], acct[40], mode[12], target[200];
  snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 0)); snprintf(acct, sizeof acct, "%s", PQgetvalue(r, 0, 3));
  snprintf(mode, sizeof mode, "%s", PQgetvalue(r, 0, 4)); snprintf(target, sizeof target, "%s", PQgetvalue(r, 0, 5));
  PQclear(r);
  if (!due) return;
  const char *w[2] = {id, name};
  r = PQexecParams(db,
    "WITH c AS (UPDATE pay.address_changes SET applied_at = now() WHERE id = $1::bigint AND cancelled_at IS NULL AND applied_at IS NULL RETURNING new_mode, new_target) "
    "UPDATE pay.addresses a SET mode = c.new_mode, forward_to = CASE WHEN c.new_mode = 'forward' THEN c.new_target END, "
    "spark_pubkey = CASE WHEN c.new_mode = 'spark' THEN c.new_target END, revision = a.revision + 1, updated_at = now() "
    "FROM c WHERE a.name = $2", 2, NULL, w, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  if (!ok) return;
  char detail[300]; snprintf(detail, sizeof detail, "{\"name\":\"%s\",\"mode\":\"%s\"}", name, mode);
  audit(db, acct, NULL, "lnaddr.change.applied", "ok", detail);
  char text[400];
  snprintf(text, sizeof text, "Payments to %s@%s now go to the new destination you asked for a day ago (%s).", name, domain(), target);
  notice(db, acct, name, "change.applied", detail, text);
}
static bool load(PGconn *db, const char *sql, const char *key, address *a) {
  memset(a, 0, sizeof *a);
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, sql, 1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  if (ok && PQntuples(r) == 1) {
    a->found = true;
    snprintf(a->name, sizeof a->name, "%s", PQgetvalue(r, 0, 0)); snprintf(a->account, sizeof a->account, "%s", PQgetvalue(r, 0, 1));
    snprintf(a->mode, sizeof a->mode, "%s", PQgetvalue(r, 0, 2)); snprintf(a->state, sizeof a->state, "%s", PQgetvalue(r, 0, 3));
    snprintf(a->forward_to, sizeof a->forward_to, "%s", PQgetvalue(r, 0, 4)); snprintf(a->spark_pubkey, sizeof a->spark_pubkey, "%s", PQgetvalue(r, 0, 5));
    a->revision = atoll(PQgetvalue(r, 0, 6)); a->max_msat = atoll(PQgetvalue(r, 0, 7)); a->comments_on = !strcmp(PQgetvalue(r, 0, 8), "t");
  }
  PQclear(r);
  return ok;
}
#define ADDRESS_COLUMNS "SELECT name, account, mode, state, coalesce(forward_to, ''), coalesce(spark_pubkey, ''), revision, max_msat, comments_on FROM pay.addresses "
static bool by_name(PGconn *db, const char *name, address *a) {
  apply_due(db, name);
  return load(db, ADDRESS_COLUMNS "WHERE name = $1", name, a);
}
static bool by_account(PGconn *db, const char *acct, address *a) {
  if (!load(db, ADDRESS_COLUMNS "WHERE account = $1", acct, a)) return false;
  if (a->found) { char name[40]; snprintf(name, sizeof name, "%s", a->name); apply_due(db, name); return load(db, ADDRESS_COLUMNS "WHERE account = $1", acct, a); }
  return true;
}

// ---------------------------------------------------------------- the LNURL routes
// The path's name[+tag], read by the name rules (lnaddr_lookup).
static bool read_name(const char *input, coax_t_LnaddrResult *r, char name[40], char tag[80]) {
  if (strlen(input) > 100) return false;
  *r = lnaddr_lookup(cs(input));
  if (r->reason != 0) return false;
  snprintf(name, 40, "%.*s", (int)r->name_len, (const char *)r->name.e);
  snprintf(tag, 80, "%.*s", r->has_tag ? (int)r->tag_len : 0, (const char *)r->tag.e);
  return true;
}
static int64_t issue_code(const address *a) { return a->found ? la_issue(state_of(a), mode_of(a), spark_backend()) : LU_NO_ADDRESS; }

static void lookup(const request *q, PGconn *db, const char *input) {
  char key[65];
  long long n = -1;
  if (ip_key(q, db, key)) n = hits(db, "lnurl_lookup_ip", key);
  if (n >= 0 && lu_limit_lookup(n) != LU_OK) { lnurl_error(q, 429, LU_LOOKUP_LIMIT, 0); return; }
  coax_t_LnaddrResult r; char name[40], tag[80];
  if (!read_name(input, &r, name, tag)) { lnurl_error(q, 404, LU_BAD_NAME, 0); return; }
  address a;
  if (!by_name(db, name, &a)) { say(q, 503, "busy"); return; }
  int64_t code = issue_code(&a);
  if (code != LU_OK) { lnurl_error(q, code == LU_NO_ADDRESS ? 404 : 200, code, 0); return; }
  static uint8_t out[UPSTREAM_MAX + 2048];
  if (mode_of(&a) == LA_FORWARD) {
    const char *text; coax_t_LuUpstream u;
    code = upstream_pay(a.forward_to, &text, &u);
    if (code != LU_OK) { lnurl_error(q, 502, code, 0); return; }
    int64_t m = lu_forward_pay_json((coax_string){text, (int64_t)strlen(text)}, u, cs(domain()), cs(name), a.max_msat, a.comments_on, cv(out, sizeof out));
    if (m < 0) { lnurl_error(q, 502, LU_UPSTREAM_BAD, 0); return; }
    reply_n(q, 200, out, m);
    return;
  }
  int64_t m = lu_pay_json(cs(domain()), cs(name), cs(tag), a.max_msat, a.comments_on, cs(env_or("OA_LNADDR_ZAP_PUBKEY", "")), cv(out, sizeof out));
  reply_n(q, 200, out, m);
}

static void callback(const request *q, PGconn *db, const char *input) {
  coax_t_LnaddrResult r; char name[40], tag[80], key[65];
  if (!read_name(input, &r, name, tag)) { lnurl_error(q, 404, LU_BAD_NAME, 0); return; }
  long long per_name = hits(db, "lnurl_invoice_name", name), per_ip = ip_key(q, db, key) ? hits(db, "lnurl_invoice_ip", key) : -1;
  int64_t lim = lu_limit_invoice(per_name < 0 ? 0 : per_name, per_ip < 0 ? 0 : per_ip);
  if (lim != LU_OK) { lnurl_error(q, 429, lim, 0); return; }
  static char amount[40], comment[1300], payerdata[600], zap[8300];
  int na = qparam(q, "amount", amount, sizeof amount);
  int64_t msat = na > 0 ? lu_amount(cs(amount)) : -1;
  if (msat < 0) { lnurl_error(q, 400, LU_AMOUNT, 0); return; }
  int nc = qparam(q, "comment", comment, sizeof comment), np = qparam(q, "payerdata", payerdata, sizeof payerdata);
  int nz = qparam(q, "nostr", zap, sizeof zap);
  if (nc < -1) { lnurl_error(q, 400, LU_NOTE_LONG, LU_COMMENT_MAX); return; }
  if (np < -1) { lnurl_error(q, 400, LU_PAYERDATA, 0); return; }
  if (nz < -1) { lnurl_error(q, 400, LU_ZAP, 0); return; }
  address a;
  if (!by_name(db, name, &a)) { say(q, 503, "busy"); return; }
  int64_t code = issue_code(&a);
  if (code != LU_OK) { lnurl_error(q, code == LU_NO_ADDRESS ? 404 : 200, code, 0); return; }
  if (mode_of(&a) != LA_FORWARD) { lnurl_error(q, 200, LU_NOT_RECEIVING, 0); return; }   // spark: its backend, when deployed

  // ---- forward mode: the target's own payRequest and invoice, checked
  const char *text; coax_t_LuUpstream u;
  code = upstream_pay(a.forward_to, &text, &u);
  if (code != LU_OK) { lnurl_error(q, 502, code, 0); return; }
  int64_t min = u.min < LU_MIN_MSAT ? LU_MIN_MSAT : u.min, max = u.max < a.max_msat ? u.max : a.max_msat;
  code = lu_check_amount(msat, min, max, false);
  if (code != LU_OK) { lnurl_error(q, 200, code, code == LU_TOO_SMALL ? min : max); return; }
  int64_t allowed = !a.comments_on ? 0 : u.comments < LU_COMMENT_MAX ? u.comments : LU_COMMENT_MAX;
  code = lu_check_note(cs(comment), allowed);
  if (code != LU_OK) { lnurl_error(q, 200, code, allowed); return; }
  if (payerdata[0]) { lnurl_error(q, 200, LU_PAYERDATA, 0); return; }   // we never pass a payer's details on
  if (zap[0] && !u.nostr) { lnurl_error(q, 200, LU_ZAP, 0); return; }
  code = lu_check_zap(cs(zap), msat);
  if (code != LU_OK) { lnurl_error(q, 200, code, 0); return; }
  static uint8_t meta[8200];
  int64_t mn = lu_upstream_metadata_decoded((coax_string){text, (int64_t)strlen(text)}, u, cv(meta, sizeof meta));
  uint8_t hash[32];
  if (mn < 0 || !lu_description_hash((coax_string){(const char *)meta, mn}, cs(""), cs(zap), cv(hash, 32))) { lnurl_error(q, 502, LU_UPSTREAM_BAD, 0); return; }
  char up_callback[1100];
  snprintf(up_callback, sizeof up_callback, "%.*s", (int)u.nc, (const char *)u.callback.e);
  static uint8_t url[1100 + 3 * 8300 + 3 * 1300 + 64];
  int64_t un = lu_upstream_callback(cs(up_callback), msat, cs(comment), cs(zap), cv(url, sizeof url - 1));
  if (un < 0) { lnurl_error(q, 502, LU_UPSTREAM_BAD, 0); return; }
  url[un] = 0;
  char *body; size_t len;
  long status = fetch((const char *)url, &body, &len);
  if (status == 0) { lnurl_error(q, 502, LU_UPSTREAM, 0); return; }
  static coax_t_LuForwarded v;
  code = lu_forward_invoice((coax_string){body, (int64_t)len}, network(), msat, cv(hash, 32), (int64_t)time(NULL), (coax_v_t_LuForwarded){&v, 1});
  free(body);
  if (code != LU_OK) { api_log("lnaddr: %s's forward target answered %ld, refused %lld", name, status, (long long)code); lnurl_error(q, 502, code, 0); return; }
  char ph[65], pr[2100], verify[1100], rev[24], amt[24];
  hex_of(v.ph.e, 32, ph);
  snprintf(pr, sizeof pr, "%.*s", (int)v.npr, (const char *)v.pr.e);
  snprintf(verify, sizeof verify, "%.*s", (int)v.nv, (const char *)v.verify.e);
  snprintf(rev, sizeof rev, "%lld", a.revision); snprintf(amt, sizeof amt, "%lld", (long long)msat);
  // The invoice row and the name's hold, together: the first invoice holds the name for good.
  const char *w[9] = {ph, name, rev, amt, pr, verify[0] ? verify : NULL, zap[0] ? zap : NULL, comment[0] ? comment : NULL, a.account};
  PGresult *res = PQexecParams(db,
    "WITH i AS (INSERT INTO pay.invoices (payment_hash, name, revision, mode, amount_msat, pr, verify_url, zap_request, comment) "
    "VALUES ($1, $2, $3::bigint, 'forward', $4::bigint, $5, $6, $7, $8) ON CONFLICT (payment_hash) DO NOTHING RETURNING 1) "
    "INSERT INTO pay.name_holds (name, skeleton, account, reason) SELECT $2, a.skeleton, $9, 'paid' FROM pay.addresses a "
    "WHERE a.name = $2 ON CONFLICT (name) DO NOTHING", 9, NULL, w, NULL, NULL, 0);
  bool ok = PQresultStatus(res) == PGRES_COMMAND_OK;
  if (!ok) api_log("lnaddr: invoice row for %s: %s", name, PQerrorMessage(db));
  PQclear(res);
  if (!ok) { say(q, 503, "busy"); return; }
  static uint8_t out[4096];
  int64_t m = lu_callback_json(cs(domain()), cs(name), cs(pr), cs(ph), cv(out, sizeof out));
  reply_n(q, 200, out, m);
}

static void verify(const request *q, PGconn *db, const char *hash) {
  char key[65];
  long long n = ip_key(q, db, key) ? hits(db, "lnurl_lookup_ip", key) : -1;
  if (n >= 0 && lu_limit_lookup(n) != LU_OK) { lnurl_error(q, 429, LU_LOOKUP_LIMIT, 0); return; }
  unsigned char b[32];
  if (!unhex(hash, b, 32)) { lnurl_error(q, 404, LU_NOT_FOUND, 0); return; }
  const char *v[1] = {hash};
  PGresult *r = PQexecParams(db, "SELECT pr, coalesce(verify_url, ''), coalesce(preimage, ''), settled_at IS NOT NULL FROM pay.invoices WHERE payment_hash = $1",
                             1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { PQclear(r); say(q, 503, "busy"); return; }
  if (PQntuples(r) != 1) { PQclear(r); lnurl_error(q, 404, LU_NOT_FOUND, 0); return; }
  char pr[2100], url[1100], pre[80];
  snprintf(pr, sizeof pr, "%s", PQgetvalue(r, 0, 0)); snprintf(url, sizeof url, "%s", PQgetvalue(r, 0, 1));
  snprintf(pre, sizeof pre, "%s", PQgetvalue(r, 0, 2));
  bool settled = !strcmp(PQgetvalue(r, 0, 3), "t");
  PQclear(r);
  if (!settled && url[0]) {
    char *body; size_t len;
    long status = fetch(url, &body, &len);
    if (status) {
      coax_t_LuVerify got;
      if (lu_verify_read((coax_string){body, (int64_t)len}, cs(hash), (coax_v_t_LuVerify){&got, 1}) == LU_OK && got.settled) {
        snprintf(pre, sizeof pre, "%.*s", (int)got.np, (const char *)got.preimage.e);
        const char *w[2] = {hash, pre};
        PGresult *u = PQexecParams(db, "UPDATE pay.invoices SET settled_at = now(), preimage = $2 WHERE payment_hash = $1 AND settled_at IS NULL", 2, NULL, w, NULL, NULL, 0);
        PQclear(u);
        settled = true;
      }
      free(body);
    }
  }
  uint8_t out[2400];
  int64_t m = lu_verify_json(settled, cs(settled ? pre : ""), cs(pr), cv(out, sizeof out));
  reply_n(q, 200, out, m);
}

// ---------------------------------------------------------------- the holder's routes
typedef struct { bool signed_in, agent, fresh, email_only; const auth_principal *p; } who;
static who caller(const request *q, PGconn *db) {
  who w = {0};
  w.p = auth_principal_of(q);
  if (!w.p || !w.p->account[0]) return w;
  w.signed_in = true;
  w.agent = w.p->kind == AP_AGENT;
  w.fresh = ap_admit(AP_L2_FRESH, w.p->kind, w.p->level, w.p->l2_age_ms, false, false) == AP_OK;
  const char *v[1] = {w.p->account};
  PGresult *r = PQexecParams(db, "SELECT NOT EXISTS (SELECT 1 FROM auth.nostr_keys WHERE account = $1) AND NOT EXISTS (SELECT 1 FROM auth.passkeys WHERE account = $1)",
                             1, NULL, v, NULL, NULL, 0);
  w.email_only = PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  return w;
}
static void card(const request *q, PGconn *db) {
  who w = caller(q, db);
  if (!w.signed_in) { la_refuse(q, 401, 1); return; }
  address a;
  if (!by_account(db, w.p->account, &a)) { say(q, 503, "busy"); return; }
  char json[4000];
  int n = snprintf(json, sizeof json, "{\"limits\":{\"min_msat\":%lld,\"max_msat\":%lld,\"comment_chars\":%lld,\"invoice_expiry_s\":%lld,"
                   "\"lookups_per_min_per_network\":%lld,\"invoices_per_min_per_name\":%lld,\"invoices_per_min_per_network\":%lld,\"change_wait_hours\":%lld}",
                   (long long)lu_limit(0), (long long)lu_limit(1), (long long)lu_limit(2), (long long)lu_limit(3), (long long)lu_limit(4),
                   (long long)lu_limit(5), (long long)lu_limit(6), (long long)(la_change(true, false, true, true, false, false, w.email_only, 0).ready_at / 3600));
  if (!a.found) { snprintf(json + n, sizeof json - (size_t)n, ",\"address\":null}"); reply(q, 200, json); return; }
  n += snprintf(json + n, sizeof json - (size_t)n, ",\"address\":\"%s@%s\",\"name\":\"%s\",\"mode\":\"%s\",\"state\":\"%s\",\"forward_to\":\"%s\",\"spark_pubkey\":\"%s\","
                "\"revision\":%lld,\"max_msat\":%lld,\"comments_on\":%s",
                a.name, domain(), a.name, a.mode, a.state, a.forward_to, a.spark_pubkey, a.revision, a.max_msat, a.comments_on ? "true" : "false");
  const char *v[1] = {a.name};
  PGresult *r = PQexecParams(db, "SELECT new_mode, new_target, extract(epoch FROM ready_at)::bigint FROM pay.address_changes "
                                 "WHERE name = $1 AND cancelled_at IS NULL AND applied_at IS NULL", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1)
    n += snprintf(json + n, sizeof json - (size_t)n, ",\"waiting\":{\"mode\":\"%s\",\"target\":\"%s\",\"ready_at\":%s}", PQgetvalue(r, 0, 0), PQgetvalue(r, 0, 1), PQgetvalue(r, 0, 2));
  PQclear(r);
  const char *u[1] = {w.p->account};
  r = PQexecParams(db, "SELECT coalesce(jsonb_agg(jsonb_build_object('kind', kind, 'at', extract(epoch FROM created_at)::bigint, 'detail', detail) ORDER BY id DESC), '[]')::text "
                       "FROM (SELECT * FROM pay.notices WHERE account = $1 ORDER BY id DESC LIMIT 10) x", 1, NULL, u, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) n += snprintf(json + n, sizeof json - (size_t)n, ",\"notices\":%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  snprintf(json + n, sizeof json - (size_t)n, "}");
  reply(q, 200, json);
}
// A forward target: its normal form ("bob@getalby.com", lowercase) and whether it answers with a
// payRequest we can pass on. LA_OK, LA_TARGET or LA_TARGET_DOWN.
static int64_t check_target(const char *raw, char out[200]) {
  uint8_t url[400];
  if (strlen(raw) >= 160 || lu_target(cs(raw), cs(domain()), cv(url, sizeof url)) < 0) return LA_TARGET;
  size_t i = 0;
  for (; raw[i] && i + 1 < 200; i++) out[i] = (char)(raw[i] >= 'A' && raw[i] <= 'Z' ? raw[i] + 32 : raw[i]);
  out[i] = 0;
  const char *text; coax_t_LuUpstream u;
  return upstream_pay(out, &text, &u) == LU_OK ? LA_OK : LA_TARGET_DOWN;
}
// A Spark wallet's proof (spark_pubkey, sig, nonce, expires) for operation on name at revision.
static bool spark_proof(const request *q, const char *acct, const char *name, const char *op, long long revision, char pubkey[80]) {
  char sig[200], nonce[80], expires[24];
  if (qparam(q, "spark_pubkey", pubkey, 80) != 66 || qparam(q, "sig", sig, sizeof sig) != 128
      || qparam(q, "nonce", nonce, sizeof nonce) < 16 || qparam(q, "expires", expires, sizeof expires) < 1) return false;
  unsigned char pk[33], s[64];
  if (!unhex(pubkey, pk, 33) || !unhex(sig, s, 64)) return false;
  uint8_t msg[600];
  int64_t m = la_proof_message(cs(acct), cs(name), cs(domain()), cs(op), revision, cs(nonce), atoll(expires), cv(msg, sizeof msg));
  return m > 0 && la_proof_ok((coax_string){(const char *)msg, m}, cv(pk, 33), cv(s, 64), atoll(expires), (int64_t)time(NULL));
}
static void claim(const request *q, PGconn *db) {
  who w = caller(q, db);
  char raw[80], target[200] = "", pubkey[80] = "";
  int nn = qparam(q, "name", raw, sizeof raw);
  coax_t_LnaddrResult r = lnaddr_claim(cs(nn > 0 ? raw : ""));
  char name[40] = "", skel[40] = "";
  bool name_ok = nn > 0 && r.reason == 0;
  if (name_ok) { snprintf(name, sizeof name, "%.*s", (int)r.name_len, (const char *)r.name.e); snprintf(skel, sizeof skel, "%.*s", (int)r.skeleton_len, (const char *)r.skeleton.e); }
  address mine; bool has_one = false, taken = false;
  if (w.signed_in) {
    if (!by_account(db, w.p->account, &mine)) { say(q, 503, "busy"); return; }
    has_one = mine.found;
    const char *v[3] = {name, skel, w.p->account};
    PGresult *t = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM pay.addresses WHERE (name = $1 OR skeleton = $2) AND account <> $3) "
                                   "OR EXISTS (SELECT 1 FROM pay.name_holds WHERE (name = $1 OR skeleton = $2) AND account <> $3)", 3, NULL, v, NULL, NULL, 0);
    taken = PQresultStatus(t) != PGRES_TUPLES_OK || PQntuples(t) != 1 || strcmp(PQgetvalue(t, 0, 0), "f");
    PQclear(t);
  }
  int64_t code = la_claim(w.signed_in, w.agent, name_ok, has_one, taken);
  if (code != LA_OK) {
    if (w.signed_in) audit(db, w.p->account, w.p->session, "lnaddr.claim", "refused", NULL);
    la_refuse(q, code == 1 ? 401 : code == 2 ? 403 : code == LA_NAME ? 400 : 409, code);
    return;
  }
  // The first destination: a forward target that answers, a wallet that signed, or none yet.
  char raw_target[200];
  const char *mode = "none";
  if (qparam(q, "forward_to", raw_target, sizeof raw_target) > 0) {
    code = check_target(raw_target, target);
    if (code != LA_OK) { la_refuse(q, 400, code); return; }
    mode = "forward";
  } else if (qparam(q, "spark_pubkey", pubkey, sizeof pubkey) >= 0) {
    if (!spark_proof(q, w.p->account, name, "claim", 1, pubkey)) { la_refuse(q, 400, 7); return; }
    mode = "spark";
  }
  const char *v[6] = {name, skel, w.p->account, mode, !strcmp(mode, "forward") ? target : NULL, !strcmp(mode, "spark") ? pubkey : NULL};
  PGresult *t = PQexecParams(db, "INSERT INTO pay.addresses (name, skeleton, account, mode, forward_to, spark_pubkey) VALUES ($1, $2, $3, $4, $5, $6)",
                             6, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(t) == PGRES_COMMAND_OK;
  PQclear(t);
  if (!ok) { la_refuse(q, 409, 5); return; }   // a race for the same name or skeleton: the unique index or a hold
  char detail[200]; snprintf(detail, sizeof detail, "{\"name\":\"%s\",\"mode\":\"%s\"}", name, mode);
  audit(db, w.p->account, w.p->session, "lnaddr.claim", "ok", detail);
  notice(db, w.p->account, name, "claimed", detail, NULL);
  card(q, db);
}
static void toggle(const request *q, PGconn *db, bool on) {
  who w = caller(q, db);
  address a;
  if (w.signed_in && !by_account(db, w.p->account, &a)) { say(q, 503, "busy"); return; }
  int64_t code = la_toggle(w.signed_in, w.agent, w.signed_in && a.found ? state_of(&a) : -1);
  if (code != LA_OK) { la_refuse(q, code == 1 ? 401 : code == 2 ? 403 : 409, code); return; }
  const char *v[2] = {a.name, on ? "active" : "off"};
  PGresult *r = PQexecParams(db, "UPDATE pay.addresses SET state = $2, updated_at = now() WHERE name = $1 AND state IN ('active', 'off')", 2, NULL, v, NULL, NULL, 0);
  PQclear(r);
  char detail[100]; snprintf(detail, sizeof detail, "{\"name\":\"%s\"}", a.name);
  audit(db, w.p->account, w.p->session, on ? "lnaddr.on" : "lnaddr.off", "ok", detail);
  notice(db, w.p->account, a.name, on ? "on" : "off", detail, NULL);
  card(q, db);
}
static void change(const request *q, PGconn *db) {
  who w = caller(q, db);
  address a;
  if (w.signed_in && (!by_account(db, w.p->account, &a) || !a.found)) { la_refuse(q, 404, LA_NO_ADDRESS); return; }
  char raw[200], target[200] = "", pubkey[80] = "";
  const char *mode = NULL;
  bool proof_ok = false;
  if (w.signed_in && qparam(q, "forward_to", raw, sizeof raw) > 0) { mode = "forward"; proof_ok = check_target(raw, target) == LA_OK; }
  else if (w.signed_in) { mode = "spark"; proof_ok = spark_proof(q, w.p->account, a.name, "change", a.revision + 1, pubkey); snprintf(target, sizeof target, "%s", pubkey); }
  bool pending = false;
  if (w.signed_in) {
    const char *v[1] = {a.name};
    PGresult *r = PQexecParams(db, "SELECT 1 FROM pay.address_changes WHERE name = $1 AND cancelled_at IS NULL AND applied_at IS NULL", 1, NULL, v, NULL, NULL, 0);
    pending = PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) > 0;
    PQclear(r);
  }
  bool same = w.signed_in && mode && !strcmp(mode, a.mode) && !strcmp(target, !strcmp(mode, "forward") ? a.forward_to : a.spark_pubkey);
  coax_t_LaChange c = la_change(w.signed_in, w.agent, w.fresh, proof_ok, pending, same, w.email_only, (int64_t)time(NULL));
  if (c.code != LA_OK) {
    if (w.signed_in) audit(db, w.p->account, w.p->session, "lnaddr.change", "refused", NULL);
    la_refuse(q, c.code == 1 ? 401 : c.code == 2 ? 403 : c.code == 6 ? 401 : 409, c.code);
    return;
  }
  char ready[24]; snprintf(ready, sizeof ready, "%lld", (long long)c.ready_at);
  const char *v[5] = {a.name, w.p->account, mode, target, ready};
  PGresult *r = PQexecParams(db, "INSERT INTO pay.address_changes (name, account, new_mode, new_target, ready_at) VALUES ($1, $2, $3, $4, to_timestamp($5::bigint))",
                             5, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  if (!ok) { la_refuse(q, 409, 8); return; }
  char detail[300]; snprintf(detail, sizeof detail, "{\"name\":\"%s\",\"mode\":\"%s\",\"ready_at\":%s}", a.name, mode, ready);
  audit(db, w.p->account, w.p->session, "lnaddr.change.requested", "ok", detail);
  char text[600];
  snprintf(text, sizeof text, "Someone logged in to your account asked to send payments to %s@%s somewhere new: %s.\n\n"
           "Nothing changes for %d hours. If you asked for it, there is nothing to do.",
           a.name, domain(), target, (int)((c.ready_at - (int64_t)time(NULL)) / 3600));
  notice(db, w.p->account, a.name, "change.requested", detail, text);
  card(q, db);
}
static void cancel(const request *q, PGconn *db) {
  who w = caller(q, db);
  address a; bool waiting = false;
  if (w.signed_in && by_account(db, w.p->account, &a) && a.found) {
    const char *v[1] = {a.name};
    PGresult *r = PQexecParams(db, "SELECT 1 FROM pay.address_changes WHERE name = $1 AND cancelled_at IS NULL AND applied_at IS NULL", 1, NULL, v, NULL, NULL, 0);
    waiting = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
    PQclear(r);
  }
  int64_t code = la_cancel(w.signed_in, w.agent, !waiting, false);
  if (code != LA_OK) { la_refuse(q, code == 1 ? 401 : code == 2 ? 403 : 409, code); return; }
  const char *v[1] = {a.name};
  PGresult *r = PQexecParams(db, "UPDATE pay.address_changes SET cancelled_at = now() WHERE name = $1 AND cancelled_at IS NULL AND applied_at IS NULL", 1, NULL, v, NULL, NULL, 0);
  PQclear(r);
  char detail[100]; snprintf(detail, sizeof detail, "{\"name\":\"%s\"}", a.name);
  audit(db, w.p->account, w.p->session, "lnaddr.change.cancelled", "ok", detail);
  char text[300];
  snprintf(text, sizeof text, "The change to where payments to %s@%s go was cancelled. Nothing changed.", a.name, domain());
  notice(db, w.p->account, a.name, "change.cancelled", detail, text);
  card(q, db);
}

// ---------------------------------------------------------------- /docs/limits
// The Lightning Address limits as people read them, after auth.c's (api --limits). The numbers are
// lnurl.coax's and lnaddr-policy.coax's, asked here, so the page can't drift from the server.
void lnaddr_print_limits(FILE *out) {
  int64_t wait_s = la_change(true, false, true, true, false, false, false, 0).ready_at;
  int64_t wait_email_s = la_change(true, false, true, true, false, false, true, 0).ready_at;
  fprintf(out, "\n## Lightning Addresses\n\nOn name@openagents.com. A wallet shows the limit's words when it refuses.\n\n"
               "| What | Limit |\n| --- | --- |\n");
  fprintf(out, "| Smallest payment | %lld sat |\n", (long long)(lu_limit(0) / 1000));
  fprintf(out, "| Largest payment | %lld sats (you can set it lower) |\n", (long long)(lu_limit(1) / 1000));
  fprintf(out, "| Note with a payment | %lld characters |\n", (long long)lu_limit(2));
  fprintf(out, "| Invoice expiry | %lld minutes |\n", (long long)(lu_limit(3) / 60));
  fprintf(out, "| Lookups from one network address | %lld a minute |\n", (long long)lu_limit(4));
  fprintf(out, "| Invoice requests for one address | %lld a minute |\n", (long long)lu_limit(5));
  fprintf(out, "| Invoice requests from one network address | %lld a minute |\n", (long long)lu_limit(6));
  fprintf(out, "| Addresses | 1 per person |\n");
  fprintf(out, "| Wait before a new destination takes effect | %lld hours (%lld for an account with only email) |\n",
          (long long)(wait_s / 3600), (long long)(wait_email_s / 3600));
  fprintf(out, "\nThis section is made from the Lightning Address server's own numbers (`coax/web/api/lnaddr.c`).\n");
}

// ---------------------------------------------------------------- routing
void lnaddr_init(void) {
  uint8_t out[256];
  bool ok = lu_error_json(LU_NOT_RECEIVING, 0, cv(out, sizeof out)) > 40 && la_issue(LA_ACTIVE, LA_FORWARD, false) == LU_OK
         && la_issue(LA_OFF, LA_FORWARD, false) == 4 && lu_limit_lookup(31) == LU_LOOKUP_LIMIT && lu_limit_invoice(121, 0) == LU_NAME_LIMIT
         && lu_limit_invoice(0, 31) == LU_IP_LIMIT && lu_check_amount(1, 1000, 2000, false) == LU_TOO_SMALL && lu_check_amount(3000, 1000, 2000, false) == LU_TOO_BIG
         && lu_check_note(cs("abc"), 2) == LU_NOTE_LONG && lu_check_payerdata(cs("{\"email\":\"x\"}")) == LU_PAYERDATA && lu_check_zap(cs("{}"), 1) == LU_ZAP
         && lu_upstream_pay(cs("{\"status\":\"ERROR\"}"), (coax_v_t_LuUpstream){&(coax_t_LuUpstream){0}, 1}) == LU_UPSTREAM_REFUSED
         && lu_upstream_pay(cs("x"), (coax_v_t_LuUpstream){&(coax_t_LuUpstream){0}, 1}) == LU_UPSTREAM_BAD
         && la_claim(true, false, false, false, false) == LA_NAME && la_toggle(true, false, LA_RETIRED) == LA_NO_ADDRESS;
  if (!ok) { fprintf(stderr, "api: lnaddr.c and lnaddr-policy.coax disagree on their numbers\n"); exit(2); }
}

bool lnaddr_route(const request *q) {
  const char *p = q->path;
  bool get = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD");
  bool wk = !strncmp(p, "/.well-known/lnurlp/", 20), ln = !strncmp(p, "/lnurlp/", 8), api = !strncmp(p, "/api/lightning/address", 22);
  if (!wk && !ln && !api) return false;
  PGconn *db = api_db();
  if (!db) { say(q, 503, "busy"); return true; }
  if (wk || ln) {
    if (!get) { say(q, 405, "GET only"); return true; }
    const char *rest = p + (wk ? 20 : 8);
    char input[120];
    if (ln && !strncmp(rest, "verify/", 7)) { snprintf(input, sizeof input, "%s", rest + 7); verify(q, db, input); return true; }
    const char *slash = strchr(rest, '/');
    size_t n = slash ? (size_t)(slash - rest) : strlen(rest);
    if (n == 0 || n >= sizeof input) { lnurl_error(q, 404, LU_BAD_NAME, 0); return true; }
    memcpy(input, rest, n); input[n] = 0;
    if (!slash) { lookup(q, db, input); return true; }
    if (ln && !strcmp(slash, "/invoice")) { callback(q, db, input); return true; }
    lnurl_error(q, 404, LU_BAD_NAME, 0);
    return true;
  }
  const char *rest = p + 22;
  if (!strcmp(rest, "") && get) { card(q, db); return true; }
  if (!strcmp(q->method, "POST")) {
    if (!strcmp(rest, "")) { claim(q, db); return true; }
    if (!strcmp(rest, "/off")) { toggle(q, db, false); return true; }
    if (!strcmp(rest, "/on")) { toggle(q, db, true); return true; }
    if (!strcmp(rest, "/change")) { change(q, db); return true; }
    if (!strcmp(rest, "/cancel")) { cancel(q, db); return true; }
  }
  say(q, 404, "not found");
  return true;
}
