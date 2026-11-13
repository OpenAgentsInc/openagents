// Private projects (docs/private-projects.md): a project page only its
// members and the agents they register can read. The rules live in SQL
// (migrations/0007_private_projects.sql, projects.api); this file reads who
// is asking (a NIP-98 signature, the project's own cookie a login link made,
// or a logged-in session's key: auth.c),
// checks where a write comes from, limits how fast, and writes the answer.
//
//   GET  /projects/<slug>                 the page: a member's shell (200), else the locked one (404)
//   GET  /projects/<slug>/api/state       the state as lines, for private.coax (cookie)
//   POST /projects/<slug>/api/message     {text}: a message to the agents (cookie)
//   POST /projects/<slug>/api/redeem      {token}: a sign-in link's token -> the session cookie
//   POST /projects/<slug>/api/logout      ends the session
//   /api/private/<slug>/<op>              the same for agents and members' tools, NIP-98 signed:
//        GET state (JSON), GET inbox, POST push {base, doc}, note {kind, text, by},
//        status {id, status, note, by}, message {text, reply_to, by}, invite {pubkey}, agent {pubkey, name, role}
//   The project's agent (docs/project-agent.md; migrations/0013_project_agent.sql), on both paths:
//        GET talk (lines on the page, else JSON); POST ask {text}, stop {turn}, undo {turn}, agree {goal} (members);
//        POST next, event, end (the runner's key). Only projects in OA_AGENT_PROJECTS have them; elsewhere the same 404.
//        A new turn wakes the runner (OA_AGENT_WAKE_URL) before the answer goes back.
//
// Anyone who may not read a project, signed in or not, and any slug that
// doesn't exist get the same answer: 404, the same body, after the same
// queries. Nothing private is ever logged: no tokens, cookies or contents.
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

#define COOKIE "oa_project"
#define SESSION_DAYS 30
// Every private answer: never kept by a cache, never framed, no referrer.
#define PRIVATE_HEADERS "Referrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nVary: Cookie, Authorization\r\n"

static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }

// ---- how fast: a fixed window a minute per key (an address or a pubkey), counted in Postgres
// (api_limit) so the limit is one for every worker and instance (#13300)
static bool allowed(const char *kind, const char *who, int per_minute) {
  char name[48]; snprintf(name, sizeof name, "%s-%s", "private", kind);
  return api_limit(api_db(), name, who, per_minute, NULL);
}

// A cookie's token (64 hex), or "".
static void cookie_named(const request *q, const char *name, char out[65]) {
  char cookies[4096], want[32];
  out[0] = 0;
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return;
  int w = snprintf(want, sizeof want, "%s=", name);
  for (const char *c = strstr(cookies, want); c; c = strstr(c + 1, want)) {
    if (c != cookies && c[-1] != ' ' && c[-1] != ';') continue;
    c += w;
    size_t n = 0;
    while (n < 64 && isxdigit((unsigned char)c[n])) { out[n] = c[n]; n++; }
    out[n] = 0;
    if (n != 64) out[0] = 0;
    return;
  }
}
static void cookie_token(const request *q, char out[65]) { cookie_named(q, COOKIE, out); }

static bool first_use(PGconn *db, const nostr_event *ev) {
  const char *v[1] = {ev->id};
  PGresult *r = PQexecParams(db, "INSERT INTO projects.seen (id, ms) VALUES ($1, projects.now_ms()) ON CONFLICT DO NOTHING",
                             1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  return ok;
}

// Existing site identity for other authenticated same-origin product routes.
// Verification/cookie/database effects only: the caller's Coax policy decides
// whether these observations admit the operation. An explicit bad signature
// never falls back to a cookie. Signed writes must additionally bind their body.
bool site_identity(const request *q, PGconn *db, char who[65], bool *signed_req, bool *payload, bool *unseen) {
  char auth[4096]; who[0] = 0; *payload = false; *unseen = false;
  const char *header = api_header(q, "Authorization", auth, sizeof auth);
  *signed_req = header != NULL;
  if (header) {
    if (strncmp(auth, "Nostr ", 6)) return false;
    char url[4300]; snprintf(url, sizeof url, "%s%s", origin(), q->target);
    nostr_event ev; char err[160];
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) return false;
    *payload = ev.payload[0] != 0;
    if (*payload) *unseen = first_use(db, &ev);
    memcpy(who, ev.pubkey, 65);
    return true;
  }
  // A session of our own (auth.c, __Host-oa_session or an app's token): its account's Nostr key.
  // An account with no key yet is not a signer here: these routes name people by key.
  (void)db;
  const auth_principal *p = auth_principal_of(q);
  if (!p || !p->session[0] || !p->pubkey[0]) return false;
  memcpy(who, p->pubkey, 65);
  return true;
}

// {"error": "not found"}, as Postgres writes projects.no(): every refusal is the same bytes (error-wire.coax).
static void no(const request *q) { char b[40]; size_t n = api_error_spaced_text(b, sizeof b, "not found"); api_respond(q->fd, 404, "application/json", PRIVATE_HEADERS, b, n); }

// The page. Both shells name nothing: the slug is already in the address.
// The engine's files are named by their content in the site's bucket, so the
// shell takes them from the generic page there (/projects/private/), whose
// script it copies with this page's data-page (0 a member, 1 locked) and data-text (the slug).
// A member's page also lists the project's attachments at its right (project-files-web.coax, through
// dom-host.js, which reads GET /api/projects/SLUG/files; GitHub #13468).
static void shell(const request *q, const char *slug, bool member) {
  char files[512] = "";
  if (member)
    snprintf(files, sizeof files,
      "<aside id=\"files\" aria-label=\"Attachments\"></aside>\n"
      "<script type=\"module\" src=\"/coaxial/dom-host.js?app=project-files-web&amp;root=files&amp;path=/projects/%s\"></script>\n", slug);
  char html[4608];
  int n = snprintf(html, sizeof html,
    "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n"
    "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n"
    "<title>%s · OpenAgents</title>\n<meta name=\"robots\" content=\"noindex\">\n<meta name=\"referrer\" content=\"no-referrer\">\n"
    "<meta name=\"theme-color\" content=\"#000000\">\n<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\n"
    "<script>if (\"gpu\" in navigator) document.documentElement.classList.add(\"gpu\")</script>\n"
    "<style>\nhtml,body{margin:0;height:100%%;background:#000;color:#ededed}\n"
    "body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,\"Segoe UI\",Roboto,\"Helvetica Neue\",Arial,sans-serif;font-size:14px;line-height:1.5}\n"
    "canvas{position:fixed;inset:0;width:100%%;height:100%%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}\n"
    "canvas[data-cursor=pointer]{cursor:pointer}\nmain{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}\n"
    "html:not(.gpu) body{overflow:auto}\na{color:#ff8c1f}\n"
    ".gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%%);white-space:nowrap;padding:0}\n"
    "#files{position:fixed;top:0;right:0;bottom:0;z-index:2;width:min(420px,100%%);overflow-y:auto}#files:empty{display:none}\n</style>\n</head>\n<body>\n"
    "<canvas id=\"coaxial\" aria-hidden=\"true\"></canvas>\n<main>\n<h1>%s</h1>\n<p><a href=\"/projects\">Projects</a></p>\n</main>\n%s"
    "<script type=\"module\">\n"
    "const t = await (await fetch(\"/projects/private/\", { cache: \"no-store\" })).text();\n"
    "const from = new DOMParser().parseFromString(t, \"text/html\").querySelector(\"script[data-app]\");\n"
    "const s = document.createElement(\"script\");\n"
    "for (const a of from.attributes) s.setAttribute(a.name, a.value);\n"
    "s.dataset.page = \"%d\"; s.dataset.text = \"%s\";\n"
    "document.body.append(s);\n</script>\n</body>\n</html>\n",
    member ? "Project" : "Private project", member ? "Project" : "Private project — ask a member for a login link", files,
    member ? 0 : 1, slug);
  api_respond(q->fd, member ? 200 : 404, "text/html; charset=utf-8", PRIVATE_HEADERS, html, (size_t)n);
}

// ---- the project's agent: which projects have one, its daily cap, and waking the runner
// OA_AGENT_PROJECTS  approved slugs, comma separated (server config, never in this repository)
// OA_AGENT_DAILY_USD the cap per project per Chicago day (10)
// OA_AGENT_WAKE_URL  the runner's wake endpoint; OA_AGENT_WAKE_TOKEN a bearer to send instead of
//                    the metadata server's identity token for it (tests)
static bool agent_on(const char *slug) {
  const char *list = getenv("OA_AGENT_PROJECTS");
  size_t n = strlen(slug);
  for (const char *p = list; p && *p; ) {
    while (*p == ',' || *p == ' ') p++;
    size_t k = strcspn(p, ", ");
    if (k == n && n && !strncmp(p, slug, n)) return true;
    p += k;
  }
  return false;
}
// The project agent's runner keys (OA_AGENT_RUNNER_PUBKEYS, from runners.conf): a turn writes
// events every few seconds, so their writes are limited at 600 a minute, not 30.
static bool runner_key(const char *pubkey) {
  const char *list = getenv("OA_AGENT_RUNNER_PUBKEYS");
  return list && strlen(pubkey) == 64 && strstr(list, pubkey) != NULL;
}
static const char *agent_cap(void) { const char *c = getenv("OA_AGENT_DAILY_USD"); return c && *c ? c : "10"; }

typedef struct { char *p; size_t n; } wbuf;
static size_t wtake(char *data, size_t size, size_t n, void *user) {
  wbuf *b = user; size_t k = size * n;
  if (b->n + k > 8192) return 0;
  char *p = realloc(b->p, b->n + k + 1);
  if (!p) return 0;
  memcpy(p + b->n, data, k); b->p = p; b->n += k; b->p[b->n] = 0;
  return k;
}
// Tells the runner a project has a turn waiting. It answers at once and
// works on its own; whatever it says, the turn stays queued until it takes it.
static long agent_wake(const char *slug) {
  const char *url = getenv("OA_AGENT_WAKE_URL");
  if (!url || !*url) return 0;
  char tok[4096], auth[4200], body[128];
  if (!api_id_token_for_url(url, "OA_AGENT_WAKE_TOKEN", tok, sizeof tok)) { api_log("agent wake: no token"); return 0; }
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  api_word_text(body, sizeof body, "slug", slug);
  CURL *h = curl_easy_init();
  if (!h) return 0;
  wbuf b = {0};
  long status = 0;
  struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, body);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, wtake);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 20L);
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h); free(b.p);
  api_log("agent wake %ld", status);
  return status;
}

// The visitor's address: the load balancer appends it and then its own, so
// it's the next to last; anything before it the visitor could have written.
static void visitor(const request *q, char ip[128]) {
  char xff[512];
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  ip[0] = 0;
  if (np) snprintf(ip, 128, "%s", parts[np >= 2 ? np - 2 : 0]);
}

bool private_route(const request *q) {
  const char *p = q->path, *rest;
  bool page_api = false;
  if (!strncmp(p, "/api/private/", 13)) rest = p + 13;
  else if (!strncmp(p, "/projects/", 10)) { rest = p + 10; page_api = true; }
  else return false;
  // <slug>[/api]/<op>, or the page itself
  char slug[72] = "", op[24] = "";
  size_t n = 0;
  while (rest[n] && rest[n] != '/' && n < sizeof slug - 1) { slug[n] = rest[n]; n++; }
  slug[n] = 0;
  const char *after = rest + n;
  bool page = false;
  if (page_api) {
    if (!*after || !strcmp(after, "/")) page = true;
    else if (!strncmp(after, "/api/", 5)) snprintf(op, sizeof op, "%s", after + 5);
    else { no(q); return true; }
  } else if (*after == '/') snprintf(op, sizeof op, "%s", after + 1);
  for (const char *c = slug; *c; c++) if (!islower((unsigned char)*c) && !isdigit((unsigned char)*c) && *c != '-') { no(q); return true; }
  if (!slug[0] || (!page && !op[0])) { no(q); return true; }

  bool post = !strcmp(q->method, "POST");
  if (!post && strcmp(q->method, "GET")) { no(q); return true; }
  char ip[128];
  visitor(q, ip);
  char auth[4096], from[256], token[65];
  bool signed_req = api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6);
  if (post) {
    // CSRF: a write comes from this site's pages (Origin), or is signed.
    bool has_origin = api_header(q, "Origin", from, sizeof from) != NULL;
    if ((has_origin && strcmp(from, origin())) || (!has_origin && !signed_req)) {
      api_error_with(q->fd, 403, PRIVATE_HEADERS, "cross-site"); return true;
    }
    if (!allowed("ip", ip, 60) || ((!strcmp(op, "redeem") || !strcmp(op, "claim")) && !allowed("redeem", ip, 10))) {
      api_error_with(q->fd, 429, PRIVATE_HEADERS "Retry-After: 60\r\n", "slow down"); return true;
    }
  }
  if (q->body_len > 256 * 1024) { api_error_with(q->fd, 413, PRIVATE_HEADERS, "too large"); return true; }
  PGconn *db = api_db();
  if (!db) { api_error_with(q->fd, 503, PRIVATE_HEADERS, "no database"); return true; }

  // Who: a signature, checked once, else the cookie (pages only).
  char pubkey[65] = "";
  token[0] = 0;
  if (signed_req) {
    char url[4300]; snprintf(url, sizeof url, "%s%s", origin(), q->target);
    nostr_event ev; char err[160];
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err) || !first_use(db, &ev)) {
      no(q); return true;
    }
    if (!strcmp(op, "claim") && !ev.payload[0]) { no(q); return true; }
    memcpy(pubkey, ev.pubkey, sizeof pubkey);
  } else if (page_api) cookie_token(q, token);
  // A page's request from a logged-in session (auth.c): its account's key, which projects.api maps
  // to the member it is (projects.member_of), as a signature's key is.
  char site[65] = "";
  if (!signed_req && page_api) {
    const auth_principal *sp = auth_principal_of(q);
    if (sp && sp->session[0] && sp->pubkey[0]) snprintf(site, sizeof site, "%s", sp->pubkey);
  }
  if (post && pubkey[0] && !allowed("key", pubkey, runner_key(pubkey) ? 600 : 30)) {
    api_error_with(q->fd, 429, PRIVATE_HEADERS "Retry-After: 60\r\n", "slow down"); return true;
  }

  char who[800];
  snprintf(who, sizeof who, "{\"pubkey\":\"%s\",\"session\":\"%s\",\"site\":\"%s\",\"origin\":\"%.200s\",\"admin\":%s,\"format\":\"%s\",\"agent_on\":%s,\"cap_usd\":\"%.16s\"}",
           pubkey, token, site, origin(), pubkey[0] && admin_is(pubkey) ? "true" : "false", page_api ? "lines" : "json",
           agent_on(slug) ? "true" : "false", agent_cap());
  char *body = malloc(q->body_len + 3);
  if (!body) { api_error_with(q->fd, 503, PRIVATE_HEADERS, "memory"); return true; }
  if (post && q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *what = page ? "shell" : op;
  // GETs read; everything else is a POST.
  bool reads = !strcmp(what, "shell") || !strcmp(what, "state") || !strcmp(what, "inbox") || !strcmp(what, "talk");
  if (reads == post) { free(body); no(q); return true; }
  // KEY (docs/chat.md): the member's own Anthropic key or Claude token, checked and sealed as /chat's are.
  if (!strcmp(what, "key")) {
    free(body);
    if (!allowed("keyset", ip, 6)) { api_error_with(q->fd, 429, PRIVATE_HEADERS "Retry-After: 60\r\n", "slow down"); return true; }
    const char *kv[4] = {"key_owner", slug, "{}", who};
    PGresult *kr = PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->'body'->>'pubkey', '') FROM projects.api($1, $2, $3::jsonb, $4::jsonb) x", 4, NULL, kv, NULL, NULL, 0);
    char owner[65] = "";
    if (PQresultStatus(kr) == PGRES_TUPLES_OK && PQntuples(kr) == 1 && atoi(PQgetvalue(kr, 0, 0)) == 200) snprintf(owner, sizeof owner, "%s", PQgetvalue(kr, 0, 1));
    PQclear(kr);
    if (strlen(owner) != 64) { no(q); return true; }
    api_log("private key by %.8s", owner);
    chat_key(q, owner, true);
    return true;
  }
  const char *v[4] = {what, slug, body, who};
  // The agent's document tools (migration 0018) build a new version, then push through projects.api.
  bool tool = !strcmp(what, "patch") || !strcmp(what, "milestone_edit") || !strcmp(what, "goal_propose");
  PGresult *r = PQexecParams(db, tool
      ? "SELECT (x->>'status')::int, x->>'body', '', false FROM projects.agent_edit($1, $2, $3::jsonb, $4::jsonb) x"
      : "SELECT (x->>'status')::int, x->>'body', coalesce(x->>'cookie', ''), coalesce(x->>'wake', 'false') = 'true' FROM projects.api($1, $2, $3::jsonb, $4::jsonb) x",
                             4, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    bool theirs = state && !strncmp(state, "22", 2);
    api_log("private %s: sql %s", what, state ? state : "?");   // never the message: it can quote the document
    PQclear(r);
    if (theirs) api_error_with(q->fd, 400, PRIVATE_HEADERS, "not json");
    else api_error_with(q->fd, 500, PRIVATE_HEADERS, "not kept");
    return true;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  const char *out = PQgetvalue(r, 0, 1), *made = PQgetvalue(r, 0, 2);
  if (page) { PQclear(r); shell(q, slug, status == 200); return true; }
  char headers[900];
  if (made[0] && status == 200)   // a sign-in: the session's token goes only into the cookie
    snprintf(headers, sizeof headers, PRIVATE_HEADERS "Set-Cookie: " COOKIE "=%s; Path=/projects/%s; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n",
             made, slug, SESSION_DAYS * 86400);
  else if (!strcmp(what, "logout") && status == 200)
    snprintf(headers, sizeof headers, PRIVATE_HEADERS "Set-Cookie: " COOKIE "=; Path=/projects/%s; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n", slug);
  else snprintf(headers, sizeof headers, PRIVATE_HEADERS);
  bool lines = page_api && (!strcmp(what, "state") || !strcmp(what, "talk")) && status == 200;
  bool wake = status == 200 && !strcmp(PQgetvalue(r, 0, 3), "t");
  if (wake) {   // a child wakes the runner, then answers; the parent goes on
    signal(SIGCHLD, SIG_IGN);
    pid_t pid = fork();
    if (pid == 0) {
      agent_wake(slug);
      api_respond(q->fd, status, lines ? "text/plain; charset=utf-8" : "application/json", headers, out, strlen(out));
      _exit(0);
    }
    if (pid > 0) { api_log("private %s %d%s%.8s", what, status, pubkey[0] ? " by " : "", pubkey); PQclear(r); return true; }
  }
  api_respond(q->fd, status, lines ? "text/plain; charset=utf-8" : "application/json", headers, out, strlen(out));
  if (status != 404) api_log("private %s %d%s%.8s", what, status, pubkey[0] ? " by " : "", pubkey);
  PQclear(r);
  return true;
}
