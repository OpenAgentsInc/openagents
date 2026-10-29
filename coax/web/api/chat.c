// openagents.com/chat (docs/chat.md): an admin's direct chat with a Claude
// agent for general questions. The rules live in SQL (migrations/0021_chat.sql,
// chat.api); this file reads who is asking, checks where a write comes from,
// limits how fast, wakes the runner and hands a credential to it to check and
// seal. The page is a Coax page in the site's bucket (/chat); everything it
// shows comes from here.
//
//   GET  /api/chat/state?c=ID      the page's lines (chat.lines): conversations, the one shown, its turns
//   POST /api/chat/new {model}     a conversation
//   POST /api/chat/send {c, text, model}   a message: a turn, and the runner is woken (c absent: a new conversation)
//   POST /api/chat/model {c, model}        the model the next turn uses
//   POST /api/chat/stop {c}        STOP
//   POST /api/chat/key {credential}        an Anthropic API key or a Claude subscription token: the runner
//                                  checks it (a models list, or a one-line Haiku turn) and seals it; only
//                                  the sealed bytes, the kind and the last four characters are kept
//   POST /api/chat/key_remove      deletes it
//   POST /api/chat/runner/next|event|end   the runner (NIP-98, a key in OA_AGENT_RUNNER_PUBKEYS)
//
// Who: an admin (admin.c: the oa_admin cookie, which admin sign-in sets for
// /api/chat too, or a NIP-98 signature), or the phone sign-in's site session
// (/signin, docs/phone-sign-in.md) for a key in admins.conf. Anyone else gets
// admin.c's 401 or 403 and nothing more.
//
// Environment: OA_CHAT_DAILY_USD (10) the cap on OpenAgents' key for /chat, every
// model together; OA_AGENT_WAKE_URL the runner's wake endpoint (its /credential
// is beside it), or OA_CHAT_RUNNER_URL the runner's base URL; OA_AGENT_WAKE_TOKEN a bearer instead of the metadata server's
// identity token (tests). A credential is never logged, stored in the clear,
// put in an event or sent back: the request that carries it is answered by a
// child process that forgets it.
#include "api.h"
#include "nostr.h"
#include <ctype.h>
#include <curl/curl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define CHAT_HEADERS "Referrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nVary: Cookie, Authorization\r\n"

static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }
static const char *cap_usd(void) { const char *c = getenv("OA_CHAT_DAILY_USD"); return c && *c ? c : "10"; }
static void reply(int fd, int status, const char *json) { api_respond(fd, status, "application/json", CHAT_HEADERS, json, strlen(json)); }

// ---- how fast: a fixed window a minute per key, per instance
#define LIMITS 128
static struct { char key[80]; double since; int n; } limits[LIMITS];
static bool allowed(const char *kind, const char *who, int per_minute) {
  char key[80]; snprintf(key, sizeof key, "%s:%.64s", kind, who);
  double now = api_now_ms();
  int slot = -1, oldest = 0;
  for (int i = 0; i < LIMITS; i++) {
    if (limits[i].key[0] && !strcmp(limits[i].key, key)) {
      if (now - limits[i].since > 60000) { limits[i].since = now; limits[i].n = 0; }
      return ++limits[i].n <= per_minute;
    }
    if (!limits[i].key[0] || now - limits[i].since > 60000) { if (slot < 0) slot = i; }
    else if (limits[i].since < limits[oldest].since) oldest = i;
  }
  int k = slot >= 0 ? slot : oldest;
  snprintf(limits[k].key, sizeof limits[k].key, "%s", key);
  limits[k].since = now; limits[k].n = 1;
  return true;
}

// ---- the runner: its wake and /credential, called with an identity token
typedef struct { char *p; size_t n; } wbuf;
static size_t wtake(char *data, size_t size, size_t n, void *user) {
  wbuf *b = user; size_t k = size * n;
  if (b->n + k > 65536) return 0;
  char *p = realloc(b->p, b->n + k + 1);
  if (!p) return 0;
  memcpy(p + b->n, data, k); b->p = p; b->n += k; b->p[b->n] = 0;
  return k;
}
static bool id_token(const char *url, char *out, size_t cap) {
  const char *given = getenv("OA_AGENT_WAKE_TOKEN");
  if (given && *given) { snprintf(out, cap, "%s", given); return true; }
  char audience[512], meta[700];
  const char *host = strstr(url, "://");
  size_t k = host ? (size_t)(host + 3 - url) + strcspn(host + 3, "/") : strlen(url);
  snprintf(audience, sizeof audience, "%.*s", (int)k, url);
  snprintf(meta, sizeof meta, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity?audience=%s", audience);
  CURL *h = curl_easy_init();
  if (!h) return false;
  wbuf b = {0};
  struct curl_slist *hd = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(h, CURLOPT_URL, meta);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, wtake);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 5L);
  bool ok = curl_easy_perform(h) == CURLE_OK && b.p && b.n + 1 < cap;
  if (ok) snprintf(out, cap, "%s", b.p);
  curl_slist_free_all(hd); curl_easy_cleanup(h); free(b.p);
  return ok;
}
// POSTs body to the runner's path (beside its /wake). Its status, and its answer in *out.
static long to_runner(const char *path, const char *body, long timeout, wbuf *out) {
  const char *wake = getenv("OA_AGENT_WAKE_URL"), *given = getenv("OA_CHAT_RUNNER_URL");
  char url[700], tok[4096], auth[4200];
  if (given && *given) snprintf(url, sizeof url, "%s%s", given, path);
  else {
    if (!wake || !*wake || !strstr(wake, "://")) return 0;
    const char *slash = strrchr(wake, '/');
    size_t base = slash && slash > strstr(wake, "://") + 2 ? (size_t)(slash - wake) : strlen(wake);
    snprintf(url, sizeof url, "%.*s%s", (int)base, wake, path);
  }
  if (!id_token(url, tok, sizeof tok)) { api_log("chat: no token for the runner"); return 0; }
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  CURL *h = curl_easy_init();
  if (!h) return 0;
  long status = 0;
  struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, body);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, wtake);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, out);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, timeout);
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h);
  memset(auth, 0, sizeof auth); memset(tok, 0, sizeof tok);
  return status;
}

// ---- who
// The asker: 0 an admin (pubkey in who), else 401 or 403. The runner: 0 with *runner.
static bool runner_key(const char *pubkey) {
  const char *list = getenv("OA_AGENT_RUNNER_PUBKEYS");
  return list && strlen(pubkey) == 64 && strstr(list, pubkey) != NULL;
}
static int runner_who(const request *q, PGconn *db) {
  char auth[4096];
  if (!api_header(q, "Authorization", auth, sizeof auth) || strncmp(auth, "Nostr ", 6)) return 401;
  char url[4300]; snprintf(url, sizeof url, "%s%s", origin(), q->target);
  nostr_event ev; char err[160];
  if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) return 401;
  if (!runner_key(ev.pubkey)) return 403;
  char ms[32]; snprintf(ms, sizeof ms, "%lld", (long long)api_now_ms());
  const char *v[3] = {ev.id, ev.pubkey, ms};
  PGresult *r = PQexecParams(db, "INSERT INTO analytics.admin_events (id, pubkey, ms) VALUES ($1, $2, $3::bigint) ON CONFLICT DO NOTHING",
                             3, NULL, v, NULL, NULL, 0);
  bool once = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  return once ? 0 : 401;
}
// The admin sign-in (admin.c), else the phone sign-in's site session (/signin, oa_session) for a key in admins.conf.
static int chat_who(const request *q, PGconn *db, char who[65]) {
  int s = admin_who(q, db, who);
  if (s != 401) return s;
  char cookies[4096], token[65]; size_t n = 0;
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return 401;
  for (const char *c = strstr(cookies, "oa_session="); c; c = strstr(c + 1, "oa_session=")) {
    if (c != cookies && c[-1] != ' ' && c[-1] != ';') continue;
    c += 11;
    while (n < 64 && isxdigit((unsigned char)c[n])) { token[n] = c[n]; n++; }
    break;
  }
  token[n] = 0;
  if (n != 64) return 401;
  const char *v[1] = {token};
  PGresult *r = PQexecParams(db, "SELECT signin.session($1)", 1, NULL, v, NULL, NULL, 0);
  int status = 401;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) {
    snprintf(who, 65, "%s", PQgetvalue(r, 0, 0));
    status = admin_is(who) ? 0 : 403;
  }
  PQclear(r);
  return status;
}

// Set in a KEY child for a project member, whom private.c has checked: chat.api keeps their credential too.
static bool keys_for_member;

// chat.api: its status into *status, its body (a string for state, else JSON) and whether to wake the runner.
static char *call(PGconn *db, const char *op, const char *body, const char *pubkey, bool runner, int *status, bool *wake) {
  char who[400];
  snprintf(who, sizeof who, "{\"pubkey\":\"%s\",\"may\":%s,\"runner\":%s,\"cap_usd\":\"%.16s\"}",
           pubkey, pubkey[0] && (admin_is(pubkey) || keys_for_member) ? "true" : "false", runner ? "true" : "false", cap_usd());
  const char *v[3] = {op, body, who};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, CASE WHEN jsonb_typeof(x->'body') = 'string' THEN x->>'body' ELSE (x->'body')::text END, "
                                 "coalesce(x->>'wake', 'false') = 'true' FROM chat.api($1, $2::jsonb, $3::jsonb) x", 3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    api_log("chat %s: sql %s", op, state ? state : "?");   // never the message: it can quote what was asked
    *status = state && !strncmp(state, "22", 2) ? 400 : 500;
    PQclear(r);
    return strdup(*status == 400 ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
  }
  *status = atoi(PQgetvalue(r, 0, 0));
  char *out = strdup(PQgetvalue(r, 0, 1));
  if (wake) *wake = !strcmp(PQgetvalue(r, 0, 2), "t");
  PQclear(r);
  return out;
}

// ---- a credential: its shape here, its truth at the runner
// "oauth" for sk-ant-oat…, "api" for any other sk-ant-… key, else NULL. Only [A-Za-z0-9_-], 24 to 400 long.
static const char *credential_kind(const char *c) {
  size_t n = strlen(c);
  if (n < 24 || n > 400) return NULL;
  for (size_t i = 0; i < n; i++) if (!isalnum((unsigned char)c[i]) && c[i] != '_' && c[i] != '-') return NULL;
  if (!strncmp(c, "sk-ant-oat", 10)) return "oauth";
  if (!strncmp(c, "sk-ant-", 7)) return "api";   // sk-ant-api…, and other Anthropic keys: the models list decides
  return NULL;
}
// The "credential" string of a JSON body, only when it is a plain string with no escapes.
static bool credential_of(const char *body, size_t len, char *out, size_t cap) {
  const char *k = NULL;
  for (size_t i = 0; i + 12 < len; i++) if (!strncmp(body + i, "\"credential\"", 12)) { k = body + i + 12; break; }
  if (!k) return false;
  const char *end = body + len;
  while (k < end && (*k == ' ' || *k == ':' || *k == '\t' || *k == '\n')) k++;
  if (k >= end || *k != '"') return false;
  k++;
  size_t n = 0;
  while (k < end && *k != '"' && *k != '\\' && n + 1 < cap) out[n++] = *k++;
  out[n] = 0;
  bool ok = k < end && *k == '"';
  if (!ok) memset(out, 0, cap);
  return ok;
}
// A JSON string's contents from the runner's answer (no escapes expected), or "".
static void jfield(const char *json, const char *key, char *out, size_t cap) {
  char pat[64]; snprintf(pat, sizeof pat, "\"%s\":", key);
  out[0] = 0;
  const char *p = json ? strstr(json, pat) : NULL;
  if (!p) return;
  p += strlen(pat);
  while (*p == ' ') p++;
  if (*p != '"') return;
  p++;
  size_t n = 0;
  while (*p && *p != '"' && n + 1 < cap) { if (*p == '\\' && p[1]) p++; out[n++] = *p++; }
  out[n] = 0;
}
// A JSON string, escaped, into out.
static void jquote(const char *s, char *out, size_t cap) {
  size_t n = 0;
  if (cap < 3) return;
  out[n++] = '"';
  for (; *s && n + 8 < cap; s++) {
    unsigned char c = (unsigned char)*s;
    if (c == '"' || c == '\\') { out[n++] = '\\'; out[n++] = (char)c; }
    else if (c < 0x20) n += (size_t)snprintf(out + n, cap - n, "\\u%04x", c);
    else out[n++] = (char)c;
  }
  out[n++] = '"'; out[n] = 0;
}

// Runs in a child: the runner checks and seals it; the sealed bytes are kept. Answers the page.
static void save_key(const request *q, const char *pubkey, char *cred, const char *kind) {
  char body[700];
  snprintf(body, sizeof body, "{\"credential\":\"%s\",\"owner\":\"%s\",\"kind\":\"%s\"}", cred, pubkey, kind);
  memset(cred, 0, strlen(cred));
  wbuf out = {0};
  long st = to_runner("/credential", body, 120, &out);
  memset(body, 0, sizeof body);
  char reason[400], sealed[4096], last4[8];
  bool good = st == 200 && out.p && strstr(out.p, "\"ok\":true");
  jfield(out.p, "reason", reason, sizeof reason);
  jfield(out.p, "sealed", sealed, sizeof sealed);
  jfield(out.p, "last4", last4, sizeof last4);
  free(out.p);
  if (!good) {
    char msg[600], qd[700];
    snprintf(msg, sizeof msg, "That key didn't work: %s", st == 200 && reason[0] ? reason : st ? "the checker could not be reached" : "the checker is not running");
    jquote(msg, qd, sizeof qd);
    char json[800]; snprintf(json, sizeof json, "{\"error\":%s}", qd);
    api_log("chat key: refused (%ld)", st);
    reply(q->fd, 400, json);
    return;
  }
  PGconn *db = api_db_new();
  if (!db) { reply(q->fd, 503, "{\"error\":\"no database\"}"); return; }
  char set[4400];
  snprintf(set, sizeof set, "{\"kind\":\"%s\",\"last4\":\"%s\",\"sealed\":\"%s\"}", kind, last4, sealed);
  int status = 0;
  char *answer = call(db, "key_set", set, pubkey, false, &status, NULL);
  api_log("chat key: saved %s %d", kind, status);
  reply(q->fd, status, answer);
  free(answer);
  PQfinish(db);
}

// KEY: a pasted credential, for the signed-in pubkey: its shape here, then a
// child has the runner check and seal it and keeps the sealed bytes. /chat
// and the project page (private.c, once it knows the member) both use it.
void chat_key(const request *q, const char *pubkey, bool member) {
  char cred[512];
  if (!credential_of(q->body, q->body_len, cred, sizeof cred)) { reply(q->fd, 400, "{\"error\":\"That key didn't work: paste the whole key\"}"); return; }
  const char *kind = credential_kind(cred);
  if (!kind) {
    memset(cred, 0, sizeof cred);
    reply(q->fd, 400, "{\"error\":\"That key didn't work: an Anthropic key starts sk-ant-, a Claude subscription token sk-ant-oat\"}");
    return;
  }
  // A check can take a while (a model turn): a child answers, and the service goes on.
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { keys_for_member = member; save_key(q, pubkey, cred, kind); _exit(0); }
  memset(cred, 0, sizeof cred);
  if (pid < 0) reply(q->fd, 503, "{\"error\":\"busy\"}");
}

bool chat_route(const request *q) {
  if (strncmp(q->path, "/api/chat/", 10)) return false;
  const char *op = q->path + 10;
  bool runner = !strncmp(op, "runner/", 7);
  if (runner) op += 7;
  static const char *const member_ops[] = {"state", "new", "send", "model", "stop", "key", "key_remove", NULL};
  static const char *const runner_ops[] = {"next", "event", "end", NULL};
  bool known = false;
  for (const char *const *o = runner ? runner_ops : member_ops; *o; o++) if (!strcmp(*o, op)) known = true;
  // Anything else under /api/chat/ is the Verse demo's room chat (room.c, /api/chat/<room>).
  if (!known) return false;
  bool post = !strcmp(q->method, "POST");
  if (!post && strcmp(q->method, "GET")) { reply(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  if ((!strcmp(op, "state")) == post) { reply(q->fd, 405, post ? "{\"error\":\"GET this\"}" : "{\"error\":\"POST this\"}"); return true; }
  if (q->body_len > 64 * 1024) { reply(q->fd, 413, "{\"error\":\"too large\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { reply(q->fd, 503, "{\"error\":\"no database\"}"); return true; }

  char pubkey[65] = "";
  if (runner) {
    int s = runner_who(q, db);
    if (s) { reply(q->fd, s, s == 403 ? "{\"error\":\"the runner only\"}" : "{\"error\":\"sign the request\"}"); return true; }
  } else {
    int s = chat_who(q, db, pubkey);
    if (s) { reply(q->fd, s, s == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"sign in as an admin\"}"); return true; }
    if (post) {
      // CSRF: a cookie's write comes from this site's pages (Origin); a signed one may come from anywhere.
      char auth[16], from[256];
      bool signed_req = api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6);
      bool has_origin = api_header(q, "Origin", from, sizeof from) != NULL;
      if ((has_origin && strcmp(from, origin())) || (!has_origin && !signed_req)) { reply(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
      if (!allowed(!strcmp(op, "key") ? "chat-key" : "chat", pubkey, !strcmp(op, "key") ? 6 : 60)) { api_respond(q->fd, 429, "application/json", CHAT_HEADERS "Retry-After: 60\r\n", "{\"error\":\"slow down\"}", 21); return true; }
    }
  }

  if (!strcmp(op, "key")) { chat_key(q, pubkey, false); return true; }

  char *body;
  if (post) {
    body = malloc(q->body_len + 3);
    if (!body) { reply(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
    if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  } else {
    char c[24]; api_query(q, "c", c, sizeof c);
    body = malloc(64);
    bool digits = c[0] != 0;
    for (const char *p = c; *p; p++) if (!isdigit((unsigned char)*p)) digits = false;
    snprintf(body, 64, digits ? "{\"c\":\"%s\"}" : "{}", c);
  }
  int status = 0; bool wake = false;
  char *out = call(db, op, body, pubkey, runner, &status, &wake);
  free(body);
  bool lines = !strcmp(op, "state") && status == 200;
  if (wake) {   // a child wakes the runner, then answers; the parent goes on
    signal(SIGCHLD, SIG_IGN);
    pid_t pid = fork();
    if (pid == 0) {
      wbuf w = {0};
      long ws = to_runner("/wake", "{\"chat\":true}", 20, &w);
      free(w.p);
      api_log("chat wake %ld", ws);
      reply(q->fd, status, out);
      _exit(0);
    }
    if (pid > 0) { api_log("chat %s %d", op, status); free(out); return true; }
  }
  api_respond(q->fd, status, lines ? "text/plain; charset=utf-8" : "application/json", CHAT_HEADERS, out, strlen(out));
  if (strcmp(op, "state") && strcmp(op, "event")) api_log("chat %s%s %d", runner ? "runner " : "", op, status);
  free(out);
  return true;
}
