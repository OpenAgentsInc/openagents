// The openagents.com API: a native C service on Cloud Run behind the
// openagents.com load balancer (path rule /api/*), with Postgres on the
// Cloud SQL instance openagents-production-pg behind it. It grew out of
// coax/web/serve.c: one process, one thread, a poll loop; each request is
// read whole, answered and closed.
//
//   api [--listen HOST:PORT] [--migrations DIR]
//   OA_API_DATABASE   a libpq connection string (Secret Manager on Cloud Run)
//   OA_TEAM           our team (team.conf as kind:value,...): its visits are testing, not usage (analytics.c)
//   PORT              0.0.0.0:$PORT when set (Cloud Run)
//
// On start it applies the migrations it was shipped with (DIR/*.sql, in name
// order, each once, in a transaction, under an advisory lock so two
// instances never race) and records them in schema_migrations; then it
// writes the team into analytics.team and marks its sessions (analytics.c).
//
// Routes:
//   GET  /api/playtest/health      a database round trip
//   POST /api/analytics/events     a page's analytics batch (analytics.ingest in SQL)
//   the admin routes in admin.c    NIP-98 signed, for the pubkeys in OA_ADMIN_PUBKEYS
//   the playtest's player routes   players, guests, saves and published worlds (player.c)
//   POST /api/playtest/npc/ask     character chat, Gemini first (npc.c)
//   presence and chat in a room    /api/playtest/presence/<room>, /api/playtest/chat/<room> (room.c)
//   private projects               /projects/<slug> and /api/private/<slug>/*, members only (private.c)
//   drops for a new project        /api/drops/*, private to who dropped them (drops.c)
//   /chat, admins only             /api/chat/*, and the runner's /api/chat/runner/* (chat.c)
#include "api.h"

#include <arpa/inet.h>
#include <dirent.h>
#include <errno.h>
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
#include <time.h>
#include <unistd.h>

#define MAX_CLIENTS 64
#define REQ_MAX (512 * 1024)

typedef struct { int fd; size_t n; double since; char *buf; } client;

static client clients[MAX_CLIENTS];
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
    case 400: return "Bad Request"; case 401: return "Unauthorized"; case 403: return "Forbidden";
    case 404: return "Not Found"; case 405: return "Method Not Allowed"; case 409: return "Conflict"; case 413: return "Payload Too Large"; case 429: return "Too Many Requests";
    case 500: return "Internal Server Error";
    case 503: return "Service Unavailable"; default: return "Error";
  }
}

void api_respond(int fd, int status, const char *type, const char *extra_headers, const void *body, size_t n) {
  char head[2048];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nCache-Control: no-store, private\r\n"
                   "X-Content-Type-Options: nosniff\r\n%sConnection: close\r\n\r\n",
                   status, reason(status), type, n, extra_headers ? extra_headers : "");
  send_all(fd, head, (size_t)h);
  if (n) send_all(fd, body, n);
}

void api_json(int fd, int status, const char *json) {
  api_respond(fd, status, "application/json", NULL, json, strlen(json));
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
      ok = sql && len && exec_ok(c, "BEGIN") && exec_ok(c, sql);
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
  if (ok) analytics_team_init(c);   // under the same lock: two instances never backfill at once
  exec_ok(c, "SELECT pg_advisory_unlock(7731001)");
  return ok;
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

// The old playtest's shared links (pt-cutover-plan, docs/verse/playtest-cutover.md):
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
  snprintf(loc, sizeof loc, "https://openagents.com/realms/haunted-house?from=playtest%s%s", ok ? "#" : "", ok ? id : "");
  snprintf(head, sizeof head, "Location: %s\r\n", loc);
  api_respond(q->fd, 301, "text/plain", head, "", 0);
}

static void handle(client *cl, char *head_end, size_t body_len) {
  request q = {0};
  q.fd = cl->fd;
  q.head = cl->buf;
  q.body = head_end + 4;
  q.body_len = body_len;
  *head_end = 0;   // the head is a string now; headers are read before the body
  if (sscanf(cl->buf, "%7s %2047s", q.method, q.target) != 2) { api_json(cl->fd, 400, "{\"error\":\"bad request\"}"); return; }
  *head_end = '\r';
  snprintf(q.path, sizeof q.path, "%s", q.target);
  char *mark = strchr(q.path, '?');
  if (mark) { *mark = 0; q.query = q.target + (mark - q.path) + 1; }
  else q.query = "";
  if (!strcmp(q.path, "/api/playtest/health")) { health(cl->fd); return; }
  if (!strcmp(q.path, "/api/playtest/old-link") && !strcmp(q.method, "GET")) { old_link(&q); return; }
  if (analytics_route(&q)) return;
  if (npc_route(&q)) return;
  if (piece_route(&q)) return;
  if (voice_edit_route(&q)) return;
  if (chat_route(&q)) return;   // before the rooms: /api/chat/state etc. are /chat's, any other /api/chat/<room> a room's
  if (room_route(&q)) return;
  if (realms_route(&q)) return;   // before the players': /api/playtest/realms...
  if (player_route(&q)) return;
  if (private_route(&q)) return;
  if (drops_route(&q)) return;
  if (villa_route(&q)) return;
  if (admin_route(&q)) return;
  api_json(cl->fd, 404, "{\"error\":\"not found\"}");
}

static void drop(client *c) { close(c->fd); c->fd = -1; c->n = 0; }

// Reads until the head and its Content-Length of body are in, then answers.
static void on_readable(client *c) {
  ssize_t k = recv(c->fd, c->buf + c->n, REQ_MAX - 1 - c->n, 0);
  if (k <= 0) { drop(c); return; }
  c->n += (size_t)k; c->buf[c->n] = 0;
  char *end = strstr(c->buf, "\r\n\r\n");
  if (!end) { if (c->n >= REQ_MAX - 1) { api_json(c->fd, 413, "{\"error\":\"too large\"}"); drop(c); } return; }
  size_t want = 0;
  for (char *line = strstr(c->buf, "\r\n"); line && line < end; line = strstr(line + 2, "\r\n"))
    if (!strncasecmp(line + 2, "Content-Length:", 15)) want = strtoul(line + 17, NULL, 10);
  size_t have = c->n - (size_t)(end + 4 - c->buf);
  if (want > REQ_MAX - 8192) { api_json(c->fd, 413, "{\"error\":\"too large\"}"); drop(c); return; }
  if (have < want) return;
  handle(c, end, want);
  drop(c);
}

int main(int argc, char **argv) {
  const char *listen_at = NULL;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) listen_at = argv[++i];
    else if (!strcmp(argv[i], "--migrations") && i + 1 < argc) snprintf(migrations_dir, sizeof migrations_dir, "%s", argv[++i]);
  }
  const char *dsn = getenv("OA_API_DATABASE");
  if (!dsn || !*dsn) { fprintf(stderr, "api: OA_API_DATABASE is not set\n"); return 2; }
  snprintf(conninfo, sizeof conninfo, "%s", dsn);
  signal(SIGPIPE, SIG_IGN);
  admin_init();
  if (!migrate()) api_log("migrations did not all apply; serving anyway (health shows the last applied)");

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
  api_log("listening on %s:%d", host, port);
  for (int i = 0; i < MAX_CLIENTS; i++) { clients[i].fd = -1; clients[i].buf = malloc(REQ_MAX); }

  for (;;) {
    struct pollfd fds[MAX_CLIENTS + 1];
    int map[MAX_CLIENTS + 1], n = 0;
    fds[n].fd = lfd; fds[n].events = POLLIN; map[n++] = -1;
    for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd >= 0) { fds[n].fd = clients[i].fd; fds[n].events = POLLIN; map[n++] = i; }
    if (poll(fds, (nfds_t)n, 5000) < 0) { if (errno == EINTR) continue; perror("api: poll"); return 1; }
    double now = api_now_ms();
    for (int k = 0; k < n; k++) {
      if (!(fds[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (map[k] < 0) {
        int fd = accept(lfd, NULL, NULL);
        if (fd < 0) continue;
        int slot = -1;
        for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd < 0) { slot = i; break; }
        if (slot < 0) { close(fd); continue; }
        clients[slot].fd = fd; clients[slot].n = 0; clients[slot].since = now;
      } else on_readable(&clients[map[k]]);
    }
    // A client that sends nothing for 15 s is let go.
    for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd >= 0 && now - clients[i].since > 15000) drop(&clients[i]);
  }
}
