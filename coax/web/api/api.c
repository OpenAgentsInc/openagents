// The openagents.com API: a native C service on Cloud Run behind the
// openagents.com load balancer (path rule /api/*), with Postgres on the
// Cloud SQL instance openagents-production-pg behind it. It grew out of
// coax/web/serve.c. A parent process listens and keeps OA_API_WORKERS worker
// processes (4 unless set; 1: no workers, the parent serves). Each worker takes
// one connection at a time off the shared socket, reads the request whole,
// answers and closes, on a database connection of its own, so one slow query
// holds one worker and the others keep answering (#13300). Nothing a request
// decides lives in a worker's memory: rate limits are counted in Postgres
// (auth.c api_limit), so they hold across workers and instances.
//
//   api [--listen HOST:PORT] [--migrations DIR] [--migrate-only]
//   api --rules | --limits | --rule METHOD PATH   (auth.c's tables; no database)
//   OA_API_DATABASE   a libpq connection string (Secret Manager on Cloud Run)
//   OA_TEAM           our team (team.conf as kind:value,...): its visits are testing, not usage (analytics.c)
//   PORT              0.0.0.0:$PORT when set (Cloud Run)
//   OA_API_WORKERS    worker processes, 1 to 32 (4: with four instances of two services that is 32 of
//                     the database's 100 connections at most; a worker idle five minutes gives its back)
//   OA_API_FAULTS     1: a room-signed POST /api/playtest/fault/slow holds this worker's database
//                     connection for the ms it names (at most 5000), for load checks; staging and tests only
//
// With --migrations DIR it applies DIR/*.sql that schema_migrations doesn't
// list (in name order, each once, in a transaction that sets oa.allow_ddl so
// the database's DROP/TRUNCATE guard lets it through, under an advisory lock
// so two instances never race). On Cloud Run the service doesn't: it runs as
// a role with no DDL (coax/web/api/db/), and deploy.sh runs the migrations
// first as a job, `api --migrations /migrations --migrate-only`, with the
// owner role (docs/auth/data-and-backups.md). Then it writes the team into
// analytics.team and marks its sessions (analytics.c).
//
// Routes:
//   GET  /api/playtest/health      a database round trip
//   POST /api/analytics/events     a page's analytics batch (analytics.ingest in SQL)
//   GET  /api/playtest/service-health  the service levels and open alerts (service.c, #13297)
//   /api/admin/flags                   the flags an admin sets with no deploy, maintenance among them (rooms.c, #13299)
//   GET  /api/playtest/crashes         the crash buckets and each one's newest report (rooms.c, #13298)
//   the admin routes in admin.c    NIP-98 signed, for the pubkeys in OA_ADMIN_PUBKEYS
//   the playtest's player routes   players, guests, saves and published worlds (player.c)
//   POST /api/playtest/npc/ask     character chat, Gemini first (npc.c)
//   POST /api/v1/inference             our models for the Mac app, NIP-98 signed, metered (inference.c)
//   presence and chat in a room    /api/playtest/presence/<room>, /api/playtest/chat/<room> (room.c)
//   private projects               /projects/<slug> and /api/private/<slug>/*, members only (private.c)
//   a realm's maker                /api/playtest/makers/<realm>, its claim, the admin's codes (makers.c)
//   listed upgrades                /api/playtest/upgrades/..., /api/admin/upgrades (makers.c)
//   drops for a new project        /api/drops/*, private to who dropped them (drops.c)
//   scheduled jobs                POST /api/jobs/<job>, Cloud Scheduler or an admin (jobs.c)
//   email, admins only            POST /api/admin/email, through Resend (email.c)
//   /chat, admins only             /api/chat/*, and the runner's /api/chat/runner/* (chat.c)
//   realm invites                  /api/playtest/access, /api/admin/access (access.c, flag off)
//   private human packs            /api/metaverse/packs/<name>, /api/metaverse/packs/fetch/<sid>, per-session keys; /api/verse/packs/ kept (verse_packs.c)
#include "api.h"
#include "error-wire.generated.h"

#include <arpa/inet.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define REQ_MAX (512 * 1024)
#define WORKERS_MAX 32
#define DB_IDLE_MS 300000   // a worker that has answered nothing for this long gives its connection back

typedef struct { int fd; size_t n; char *buf; } client;

static PGconn *db;
static char conninfo[2048];
static char migrations_dir[1024] = "migrations";

double api_now_ms(void) {
  struct timeval tv; gettimeofday(&tv, NULL);
  return tv.tv_sec * 1000.0 + tv.tv_usec / 1000.0;
}

void api_log(const char *fmt, ...) {
  va_list ap; va_start(ap, fmt);
  fputs("api  ", stderr); vfprintf(stderr, fmt, ap); fputc('\n', stderr);
  va_end(ap);
}

static void send_all(int fd, const void *data, size_t n) {
  const char *p = data;
  while (n) {
    ssize_t k = send(fd, p, n, 0);
    if (k < 0) { if (errno == EINTR) continue; return; }
    p += k; n -= (size_t)k;
  }
}

static const char *reason(int status) {
  switch (status) {
    case 200: return "OK"; case 204: return "No Content"; case 301: return "Moved Permanently"; case 303: return "See Other";
    case 400: return "Bad Request"; case 401: return "Unauthorized"; case 402: return "Payment Required"; case 403: return "Forbidden";
    case 404: return "Not Found"; case 405: return "Method Not Allowed"; case 409: return "Conflict"; case 413: return "Payload Too Large"; case 429: return "Too Many Requests";
    case 500: return "Internal Server Error";
    case 503: return "Service Unavailable"; default: return "Error";
  }
}

// OA_API_ROUTE_HEADER=1 (the checks only; never set in production): every answer names the route
// function that wrote it in an X-OA-Route header, so the suite can hold each route file's bodies to
// that file's messages (routes-goldens.coax --read-all; Protocol, #13250).
static const char *route_at;
static int route_header = -1;
#define ROUTE(fn) do { route_at = #fn; if (fn(&q)) return; } while (0)
void api_route_at(const char *name) { route_at = name; }
void api_respond(int fd, int status, const char *type, const char *extra_headers, const void *body, size_t n) {
  char head[4096], named[96] = "";
  if (route_header < 0) { const char *v = getenv("OA_API_ROUTE_HEADER"); route_header = v && !strcmp(v, "1"); }
  if (route_header && route_at) snprintf(named, sizeof named, "X-OA-Route: %s\r\n", route_at);
  // auth.c's rotated or cleared session cookie rides on whatever this request answers.
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nCache-Control: no-store, private\r\n"
                   "X-Content-Type-Options: nosniff\r\n%s%s%sConnection: close\r\n\r\n",
                   status, reason(status), type, n, extra_headers ? extra_headers : "", auth_extra_headers(), named);
  if (h < 0 || h >= (int)sizeof head) { api_log("response head too long (%d)", h); return; }
  send_all(fd, head, (size_t)h);
  if (n) send_all(fd, body, n);
}

// A 200 whose bytes are named by their own hash, so they never change: kept by any cache for good.
// It carries no cookie: the answer is the same for everyone.
void api_respond_immutable(int fd, const char *type, const void *body, size_t n) {
  char head[400];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 200 OK\r\nContent-Type: %s\r\nContent-Length: %zu\r\nCache-Control: public, max-age=31536000, immutable\r\n"
                   "X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n", type, n);
  if (h < 0 || h >= (int)sizeof head) { api_log("response head too long (%d)", h); return; }
  send_all(fd, head, (size_t)h);
  if (n) send_all(fd, body, n);
}

void api_json(int fd, int status, const char *json) {
  api_respond(fd, status, "application/json", NULL, json, strlen(json));
}
// The refusal body, written by error-wire.coax. Words too long for out are cut to what fits.
static coax_string err_cs(const char *s, size_t n) { return (coax_string){s, (int64_t)n}; }
static size_t err_fit(char *out, size_t cap, const char *words, int spaced) {
  if (cap == 0) return 0;
  size_t wn = strlen(words);
  for (;;) {
    coax_v_u8 o = {(uint8_t *)out, (int64_t)cap - 1};
    int64_t n = spaced ? erw_error_spaced_write(err_cs(words, wn), o) : erw_error_write(err_cs(words, wn), o);
    if (n >= 0) { out[n] = 0; return (size_t)n; }
    if (wn == 0) { out[0] = 0; return 0; }
    wn = wn > 64 ? wn - 64 : 0;
  }
}
size_t api_error_text(char *out, size_t cap, const char *words) { return err_fit(out, cap, words, 0); }
size_t api_error_spaced_text(char *out, size_t cap, const char *words) { return err_fit(out, cap, words, 1); }
void api_error_with(int fd, int status, const char *extra_headers, const char *words) {
  char out[2048]; size_t n = api_error_text(out, sizeof out, words);
  api_respond(fd, status, "application/json", extra_headers, out, n);
}
void api_error(int fd, int status, const char *words) { api_error_with(fd, status, NULL, words); }
size_t api_error_num_text(char *out, size_t cap, const char *words, const char *key, long long num) {
  if (cap == 0) return 0;
  int64_t n = erw_error_num_write(err_cs(words, strlen(words)), err_cs(key, strlen(key)), (int64_t)num, (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  out[n < 0 ? 0 : n] = 0; return n < 0 ? 0 : (size_t)n;
}
void api_error_num(int fd, int status, const char *words, const char *key, long long num) {
  char out[2048]; size_t n = api_error_num_text(out, sizeof out, words, key, num);
  if (!n) { api_error(fd, status, words); return; }
  api_respond(fd, status, "application/json", NULL, out, n);
}
size_t api_num_text(char *out, size_t cap, const char *key, long long num) {
  if (cap == 0) return 0;
  int64_t n = erw_num_write(err_cs(key, strlen(key)), (int64_t)num, (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  out[n < 0 ? 0 : n] = 0; return n < 0 ? 0 : (size_t)n;
}
size_t api_word_text(char *out, size_t cap, const char *key, const char *word) {
  if (cap == 0) return 0;
  int64_t n = erw_word_write(err_cs(key, strlen(key)), err_cs(word, strlen(word)), (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  out[n < 0 ? 0 : n] = 0; return n < 0 ? 0 : (size_t)n;
}
size_t api_words2_text(char *out, size_t cap, const char *key, const char *word, const char *key2, const char *word2) {
  if (cap == 0) return 0;
  int64_t n = erw_words2_write(err_cs(key, strlen(key)), err_cs(word, strlen(word)), err_cs(key2, strlen(key2)), err_cs(word2, strlen(word2)), (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  out[n < 0 ? 0 : n] = 0; return n < 0 ? 0 : (size_t)n;
}
void api_error_word(int fd, int status, const char *words, const char *key, const char *word) {
  char out[2048];
  int64_t n = erw_error_word_write(err_cs(words, strlen(words)), err_cs(key, strlen(key)), err_cs(word, strlen(word)), (coax_v_u8){(uint8_t *)out, sizeof out - 1});
  if (n < 0) { api_error(fd, status, words); return; }
  api_respond(fd, status, "application/json", NULL, out, (size_t)n);
}

// ---- the database: one connection, made again when it drops
PGconn *api_db(void) {
  if (db && PQstatus(db) == CONNECTION_OK) return db;
  if (db) PQfinish(db);
  db = PQconnectdb(conninfo);
  if (PQstatus(db) != CONNECTION_OK) {
    api_log("database: %s", PQerrorMessage(db));
    PQfinish(db); db = NULL;
  }
  return db;
}

// A connection of its own, for a child process (npc.c), which never touches the parent's.
PGconn *api_db_new(void) {
  PGconn *c = PQconnectdb(conninfo);
  if (PQstatus(c) != CONNECTION_OK) { api_log("database: %s", PQerrorMessage(c)); PQfinish(c); return NULL; }
  return c;
}

static bool exec_ok(PGconn *c, const char *sql) {
  PGresult *r = PQexec(c, sql);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK || PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("sql: %s", PQerrorMessage(c));
  PQclear(r);
  return ok;
}

static int by_name(const void *a, const void *b) { return strcmp(*(char *const *)a, *(char *const *)b); }

// Applies DIR/*.sql that schema_migrations doesn't list, in name order.
static bool migrate(void) {
  PGconn *c = api_db();
  if (!c) return false;
  if (!exec_ok(c, "SELECT pg_advisory_lock(7731001)")) return false;
  bool ok = exec_ok(c, "CREATE TABLE IF NOT EXISTS schema_migrations (name text PRIMARY KEY, applied timestamptz NOT NULL DEFAULT now())");
  DIR *d = opendir(migrations_dir);
  char *names[256]; int count = 0;
  struct dirent *e;
  while (ok && d && (e = readdir(d)) && count < 256) {
    size_t n = strlen(e->d_name);
    if (n > 4 && !strcmp(e->d_name + n - 4, ".sql")) names[count++] = strdup(e->d_name);
  }
  if (d) closedir(d);
  qsort(names, (size_t)count, sizeof names[0], by_name);
  for (int i = 0; i < count; i++) {
    if (!ok) { free(names[i]); continue; }
    const char *p[1] = {names[i]};
    PGresult *r = PQexecParams(c, "SELECT 1 FROM schema_migrations WHERE name = $1", 1, NULL, p, NULL, NULL, 0);
    bool done = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
    PQclear(r);
    if (!done) {
      char path[2048]; snprintf(path, sizeof path, "%s/%s", migrations_dir, names[i]);
      FILE *f = fopen(path, "rb");
      char *sql = NULL; size_t len = 0;
      if (f) { fseek(f, 0, SEEK_END); len = (size_t)ftell(f); fseek(f, 0, SEEK_SET); sql = calloc(len + 1, 1); if (fread(sql, 1, len, f) != len) len = 0; fclose(f); }
      // oa.allow_ddl names the migration: the guard in db/guard.sql refuses DROP and TRUNCATE without it.
      ok = sql && len && exec_ok(c, "BEGIN");
      if (ok) {
        r = PQexecParams(c, "SELECT set_config('oa.allow_ddl', $1, true)", 1, NULL, p, NULL, NULL, 0);
        ok = PQresultStatus(r) == PGRES_TUPLES_OK;
        PQclear(r);
      }
      ok = ok && exec_ok(c, sql);
      if (ok) {
        r = PQexecParams(c, "INSERT INTO schema_migrations (name) VALUES ($1)", 1, NULL, p, NULL, NULL, 0);
        ok = PQresultStatus(r) == PGRES_COMMAND_OK;
        PQclear(r);
      }
      ok = ok && exec_ok(c, "COMMIT");
      if (!ok) exec_ok(c, "ROLLBACK");
      api_log("migration %s: %s", names[i], ok ? "applied" : "failed");
      free(sql);
    }
    free(names[i]);
  }
  exec_ok(c, "SELECT pg_advisory_unlock(7731001)");
  return ok;
}

// The team into analytics.team, under the migrations' lock: two instances never backfill at once.
static void team_init(void) {
  PGconn *c = api_db();
  if (!c || !exec_ok(c, "SELECT pg_advisory_lock(7731001)")) return;
  analytics_team_init(c);
  exec_ok(c, "SELECT pg_advisory_unlock(7731001)");
}

// ---- requests
const char *api_header(const request *q, const char *name, char *out, size_t cap) {
  size_t n = strlen(name);
  for (const char *line = strstr(q->head, "\r\n"); line && line[2] && line < q->body; line = strstr(line + 2, "\r\n")) {
    const char *h = line + 2;
    if (!strncasecmp(h, name, n) && h[n] == ':') {
      const char *v = h + n + 1;
      while (*v == ' ' || *v == '\t') v++;
      const char *end = strstr(v, "\r\n");
      size_t len = end ? (size_t)(end - v) : strlen(v);
      if (len >= cap) len = cap - 1;
      memcpy(out, v, len); out[len] = 0;
      return out;
    }
  }
  if (cap) out[0] = 0;
  return NULL;
}

// A query parameter's value, percent-decoded.
const char *api_query(const request *q, const char *name, char *out, size_t cap) {
  size_t n = strlen(name);
  for (const char *p = q->query; p && *p; ) {
    const char *amp = strchr(p, '&');
    size_t len = amp ? (size_t)(amp - p) : strlen(p);
    if (len > n && !strncmp(p, name, n) && p[n] == '=') {
      size_t o = 0;
      for (size_t i = n + 1; i < len && o + 1 < cap; i++) {
        if (p[i] == '%' && i + 2 < len) { char hx[3] = {p[i + 1], p[i + 2], 0}; out[o++] = (char)strtol(hx, NULL, 16); i += 2; }
        else out[o++] = p[i] == '+' ? ' ' : p[i];
      }
      out[o] = 0;
      return out;
    }
    p = amp ? amp + 1 : NULL;
  }
  if (cap) out[0] = 0;
  return NULL;
}

// Never reflect unsanitized environment bytes into the public JSON response.
static const char *health_identity(const char *key) {
  const char *value = getenv(key);
  if (!value || strlen(value) > 128) return "";
  for (const unsigned char *p = (const unsigned char *)value; *p; p++)
    if (!((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || *p == '-')) return "";
  return value;
}

static void health(int fd) {
  PGconn *c = api_db();
  if (!c) { api_json(fd, 503, "{\"ok\":false,\"error\":\"database unavailable\"}"); return; }
  double t0 = api_now_ms();
  PGresult *r = PQexec(c, "SELECT current_database(), (SELECT max(name) FROM schema_migrations), "
                          "to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')");
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    api_json(fd, 503, "{\"ok\":false,\"error\":\"database query failed\"}");
    return;
  }
  char json[1200];
  snprintf(json, sizeof json, "{\"ok\":true,\"database\":\"%s\",\"migration\":\"%s\",\"at\":\"%s\",\"roundTripMs\":%.1f,\"build\":\"%s\",\"release\":\"%s\",\"service\":\"%s\",\"revision\":\"%s\"}",
           PQgetvalue(r, 0, 0), PQgetvalue(r, 0, 1), PQgetvalue(r, 0, 2), api_now_ms() - t0,
           health_identity("OA_BUILD_SHA"), health_identity("OA_RELEASE_ID"), health_identity("K_SERVICE"), health_identity("K_REVISION"));
  PQclear(r);
  api_json(fd, 200, json);
}

// The old playtest's shared links (pt-cutover-plan, docs/metaverse/playtest-cutover.md):
// playtest.openagents.com/haunt.html?visit=ID reaches here rewritten to
// /api/playtest/old-link and moves for good to that world on the realm page,
// whose hash names it; a link with no world opens the Haunted House.
static void old_link(const request *q) {
  char id[80], loc[256];
  api_query(q, "visit", id, sizeof id);
  bool ok = id[0] && strlen(id) <= 64;
  for (const char *c = id; ok && *c; c++) ok = (*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-';
  if (ok && !strcmp(id, "haunted-house")) ok = false;
  char head[300];
  snprintf(loc, sizeof loc, "https://openagents.com/worlds/haunted-house?from=playtest%s%s", ok ? "#" : "", ok ? id : "");
  snprintf(head, sizeof head, "Location: %s\r\n", loc);
  api_respond(q->fd, 301, "text/plain", head, "", 0);
}

// POST /api/playtest/fault/slow, body "fault MS AT" (OA_API_FAULTS=1 only: staging and the tests),
// signed as a room's write is (X-Room-Mac, rooms.c room_write_signed): a query that takes MS ms
// (1 to 5000) on this worker's connection, as a slow statement would. AT is the sender's clock in ms,
// within two minutes of ours. The load checks send it beside their tickets to show the other
// workers keep answering (#13300).
static void fault_slow(const request *q) {
  const char *on = getenv("OA_API_FAULTS");
  if (!on || strcmp(on, "1")) { api_error(q->fd, 404, "not found"); return; }
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST a fault"); return; }
  if (q->body_len == 0 || q->body_len > 64) { api_error(q->fd, 401, "unsigned"); return; }
  char body[80]; memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  if (room_write_signed(q, "fault", body, q->body_len) < 0) return;
  long n = 0; long long at = 0;
  double now = api_now_ms();
  if (sscanf(body, "fault %ld %lld", &n, &at) != 2 || n < 1 || n > 5000 || at < now - 120000 || at > now + 120000) { api_error(q->fd, 400, "fault MS AT: ms 1 to 5000, at now"); return; }
  PGconn *c = api_db();
  if (!c) { api_error(q->fd, 503, "database unavailable"); return; }
  char sql[64], out[64]; snprintf(sql, sizeof sql, "SELECT pg_sleep(%ld / 1000.0)", n);
  PGresult *r = PQexec(c, sql); PQclear(r);
  snprintf(out, sizeof out, "{\"slept_ms\":%.0f,\"pid\":%d}", api_now_ms() - now, (int)getpid());
  api_json(q->fd, 200, out);
}

static void handle(client *cl, char *head_end, size_t body_len) {
  request q = {0};
  q.fd = cl->fd;
  q.head = cl->buf;
  q.body = head_end + 4;
  q.body_len = body_len;
  *head_end = 0;   // the head is a string now; headers are read before the body
  if (sscanf(cl->buf, "%7s %2047s", q.method, q.target) != 2) { api_error(cl->fd, 400, "bad request"); return; }
  *head_end = '\r';
  snprintf(q.path, sizeof q.path, "%s", q.target);
  char *mark = strchr(q.path, '?');
  if (mark) { *mark = 0; q.query = q.target + (mark - q.path) + 1; }
  else q.query = "";
  auth_reset();
  route_at = "auth_gate";
  if (auth_gate(&q)) return;
  route_at = "api";   // the route's rule (auth.c); a route with no rule is 404
  if (!strcmp(q.path, "/api/playtest/health")) { health(cl->fd); return; }
  if (!strcmp(q.path, "/api/playtest/fault/slow")) { fault_slow(&q); return; }
  ROUTE(auth_api_route);
  ROUTE(agents_route);   // agents, grants and `openagents login` (agents.c)
  ROUTE(lnaddr_route);
  ROUTE(wallet_route);   // the wallet's encrypted backup and daily limit (wallet.c)
  if (!strcmp(q.path, "/api/playtest/old-link") && !strcmp(q.method, "GET")) { old_link(&q); return; }
  ROUTE(analytics_route);
  ROUTE(access_route);   // before the admin routes and /api/auth (access.c)
  ROUTE(upgrades_route);   // listed upgrades (makers.c), before the admin routes too
  ROUTE(jobs_route);
  ROUTE(flags_route);   // before the admin routes: /api/admin/flags is its (rooms.c, #13299)
  ROUTE(email_route);   // before the admin routes: /api/admin/email is its
  ROUTE(makers_route);   // before the admin routes: /api/admin/makers/... is its
  ROUTE(npc_route);
  ROUTE(anime_route);
  ROUTE(worldbuilder_route);
  ROUTE(scripts_admin_route);   // before the admin routes: /api/admin/scripts/... is its (scripts.c, #13467)
  ROUTE(worlds_route);
  ROUTE(world_files_project_route);
  ROUTE(inference_route);
  ROUTE(mesh_route);
  ROUTE(piece_route);
  ROUTE(voice_edit_route);
  ROUTE(chat_history_route);
  ROUTE(chat_route);   // before the rooms: /api/chat/state etc. are /chat's, any other /api/chat/<room> a room's
  ROUTE(room_route);
  ROUTE(seats_route);
  ROUTE(metaverse_packs_route);
  ROUTE(service_route);   // the service levels and alerts (service.c, #13297)
  ROUTE(realms_route);   // before the players': /api/playtest/realms...
  ROUTE(player_route);
  ROUTE(private_route);
  ROUTE(drops_route);
  ROUTE(villa_route);
  ROUTE(admin_route);
  api_error(cl->fd, 404, "not found");
}

// One connection: read until the head and its Content-Length of body are in, answer, close. A
// client that sends nothing for 15 s is let go.
static void serve_one(client *c) {
  double since = api_now_ms();
  c->n = 0;
  for (;;) {
    struct pollfd p = {c->fd, POLLIN, 0};
    double left = 15000 - (api_now_ms() - since);
    if (left <= 0) break;
    int got = poll(&p, 1, (int)left);
    if (got < 0 && errno == EINTR) continue;
    if (got <= 0) break;
    ssize_t k = recv(c->fd, c->buf + c->n, REQ_MAX - 1 - c->n, 0);
    if (k <= 0) break;
    c->n += (size_t)k; c->buf[c->n] = 0;
    char *end = strstr(c->buf, "\r\n\r\n");
    if (!end) { if (c->n >= REQ_MAX - 1) { api_error(c->fd, 413, "too large"); break; } continue; }
    size_t want = 0;
    for (char *line = strstr(c->buf, "\r\n"); line && line < end; line = strstr(line + 2, "\r\n"))
      if (!strncasecmp(line + 2, "Content-Length:", 15)) want = strtoul(line + 17, NULL, 10);
    size_t have = c->n - (size_t)(end + 4 - c->buf);
    if (want > REQ_MAX - 8192) { api_error(c->fd, 413, "too large"); break; }
    if (have < want) continue;
    handle(c, end, want);
    break;
  }
  close(c->fd); c->fd = -1;
}

// A worker (or, with one worker, the only process): connections off the shared socket, one at a
// time. `parent` is the supervisor's pid (0: none); a worker whose parent is gone ends.
static volatile sig_atomic_t stopping;
static void on_term(int sig) { (void)sig; stopping = 1; }
static int serve(int lfd, pid_t parent) {
  client c = {-1, 0, malloc(REQ_MAX)};
  if (!c.buf) return 1;
  double last = api_now_ms();
  while (!stopping) {
    struct pollfd p = {lfd, POLLIN, 0};
    int got = poll(&p, 1, 5000);
    if (got < 0) { if (errno == EINTR) continue; perror("api: poll"); return 1; }
    if (parent && getppid() != parent) return 0;
    if (got == 0) {
      if (db && api_now_ms() - last > DB_IDLE_MS) { PQfinish(db); db = NULL; }
      continue;
    }
    int fd = accept(lfd, NULL, NULL);   // the socket doesn't block: another worker may have taken it
    if (fd < 0) continue;
    fcntl(fd, F_SETFL, fcntl(fd, F_GETFL, 0) & ~O_NONBLOCK);   // BSD hands the listener's flag down
    c.fd = fd;
    serve_one(&c);
    last = api_now_ms();
  }
  return 0;
}

// The supervisor: `count` workers, each started again a moment after it ends, until SIGTERM or
// SIGINT, which it passes on.
static int supervise(int lfd, int count) {
  pid_t kids[WORKERS_MAX] = {0}, me = getpid();
  struct sigaction sa = {0}; sa.sa_handler = on_term;
  sigaction(SIGTERM, &sa, NULL); sigaction(SIGINT, &sa, NULL);
  while (!stopping) {
    for (int i = 0; i < count && !stopping; i++) {
      if (kids[i] > 0) continue;
      pid_t pid = fork();
      if (pid == 0) _exit(serve(lfd, me));   // SIGTERM: it finishes the request it has, then ends
      if (pid < 0) { perror("api: fork"); usleep(200000); continue; }
      kids[i] = pid;
    }
    int st; pid_t gone = waitpid(-1, &st, 0);
    if (gone < 0) { if (errno == EINTR) continue; usleep(200000); continue; }
    for (int i = 0; i < count; i++) if (kids[i] == gone) {
      kids[i] = 0;
      if (!stopping) { api_log("worker %d ended (status %d); starting another", (int)gone, st); usleep(100000); }
    }
  }
  for (int i = 0; i < count; i++) if (kids[i] > 0) kill(kids[i], SIGTERM);
  for (int i = 0; i < count; i++) if (kids[i] > 0) waitpid(kids[i], NULL, 0);
  return 0;
}

int main(int argc, char **argv) {
  const char *listen_at = NULL;
  bool migrations = false, migrate_only = false;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) listen_at = argv[++i];
    else if (!strcmp(argv[i], "--migrations") && i + 1 < argc) { snprintf(migrations_dir, sizeof migrations_dir, "%s", argv[++i]); migrations = true; }
    else if (!strcmp(argv[i], "--migrate-only")) migrate_only = true;
    else if (!strcmp(argv[i], "--rules")) { auth_print_rules(stdout); return 0; }    // every route's rule
    else if (!strcmp(argv[i], "--limits")) { auth_print_limits(stdout); lnaddr_print_limits(stdout); makers_print_limits(stdout); npc_print_limits(stdout); rooms_print_limits(stdout); return 0; }  // docs/limits/index.md
    else if (!strcmp(argv[i], "--rule") && i + 2 < argc) { const char *r = auth_rule_of(argv[i + 1], argv[i + 2]); printf("%s\n", r ? r : "none"); return r ? 0 : 1; }
  }
  auth_init();
  lnaddr_init();
  makers_init();
  const char *dsn = getenv("OA_API_DATABASE");
  if (!dsn || !*dsn) { fprintf(stderr, "api: OA_API_DATABASE is not set\n"); return 2; }
  snprintf(conninfo, sizeof conninfo, "%s", dsn);
  signal(SIGPIPE, SIG_IGN);
  if (migrate_only) {   // the deploy's migration job: apply and exit, nonzero when one failed
    bool ok = migrations && migrate();
    api_log("migrations: %s", ok ? "all applied" : "FAILED");
    return ok ? 0 : 1;
  }
  admin_init();
  if (migrations && !migrate()) api_log("migrations did not all apply; serving anyway (health shows the last applied)");
  team_init();
  realms_refresh_pages();

  char host[256] = "127.0.0.1"; int port = 8810;
  const char *env_port = getenv("PORT");
  if (env_port) { snprintf(host, sizeof host, "0.0.0.0"); port = atoi(env_port); }
  if (listen_at) { const char *colon = strrchr(listen_at, ':'); if (colon) { snprintf(host, sizeof host, "%.*s", (int)(colon - listen_at), listen_at); port = atoi(colon + 1); } }
  int lfd = socket(AF_INET, SOCK_STREAM, 0);
  int one = 1; setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in addr = {0};
  addr.sin_family = AF_INET; addr.sin_port = htons((uint16_t)port);
  inet_pton(AF_INET, host, &addr.sin_addr);
  if (bind(lfd, (struct sockaddr *)&addr, sizeof addr) || listen(lfd, 128)) { perror("api: listen"); return 1; }
  int count = 4;
  const char *wk = getenv("OA_API_WORKERS");
  if (wk && *wk) count = atoi(wk);
  if (count < 1) count = 1;
  if (count > WORKERS_MAX) count = WORKERS_MAX;
  api_log("listening on %s:%d, %d worker%s", host, port, count, count == 1 ? "" : "s");
  // no worker blocks in accept on a connection another one took
  fcntl(lfd, F_SETFL, fcntl(lfd, F_GETFL, 0) | O_NONBLOCK);
  if (count == 1) return serve(lfd, 0);
  // each worker opens its own database connection: the parent's goes before they start
  if (db) { PQfinish(db); db = NULL; }
  return supervise(lfd, count);
}
