// coax serve: Coaxial's development server. One process, one thread.
//
// It serves an app directory over HTTP, watches the app's files and the
// browser host, rebuilds what changed, and tells every open tab over a
// WebSocket (/coaxial/live) what to swap:
//   .coax / .c   rebuild the engine (the app's build script), then "wasm"
//   .wgsl        "wgsl": the tab recompiles its shaders
//   .html .js .md .svg   "page": the tab reloads
// A failed build sends "error" with the compiler's output; the tab keeps
// running the last good engine.
//
//   serve APP_DIR [--listen HOST:PORT] [--build CMD] [--host-dir DIR] [--static] [--cache SECONDS]
// Defaults: 127.0.0.1:8800, build command APP_DIR/build.sh, the host files
// next to this binary's directory (coax/web/).
//
// --static is production: no building, no watching, no live channel; files
// are served as they are, cacheable for five minutes (--cache SECONDS; 0
// means browsers check back every time). It listens on
// 0.0.0.0:$PORT when PORT is set (Cloud Run). TLS is the platform's.
#include <arpa/inet.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "../../verse/voice-host/ws.h"

#define MAX_CLIENTS 64
#define MAX_WATCH 256
#define REQ_MAX 16384

typedef struct { int fd; bool ws; size_t n; char buf[REQ_MAX]; } client;
typedef struct { char path[1024]; struct timespec mtime; } watched;

static client clients[MAX_CLIENTS];
static watched files[MAX_WATCH];
static int file_count;
static char app_dir[1024], host_dir[1024], build_cmd[2048];
static bool static_mode;
static char cache_control[64] = "public, max-age=300";

#ifdef __APPLE__
#define MTIME(st) ((st).st_mtimespec)
#else
#define MTIME(st) ((st).st_mtim)
#endif

static double now_ms(void) {
  struct timeval tv; gettimeofday(&tv, NULL);
  return tv.tv_sec * 1000.0 + tv.tv_usec / 1000.0;
}

static void logf_(const char *fmt, ...) {
  time_t t = time(NULL); char ts[16]; strftime(ts, sizeof ts, "%H:%M:%S", localtime(&t));
  va_list ap; va_start(ap, fmt);
  fprintf(stderr, "coax serve %s  ", ts); vfprintf(stderr, fmt, ap); fputc('\n', stderr);
  va_end(ap);
}

static bool ends_with(const char *s, const char *suffix) {
  size_t a = strlen(s), b = strlen(suffix);
  return a >= b && !strcmp(s + a - b, suffix);
}

static const char *content_type(const char *path) {
  if (ends_with(path, ".html")) return "text/html; charset=utf-8";
  if (ends_with(path, ".js")) return "text/javascript; charset=utf-8";
  if (ends_with(path, ".wasm")) return "application/wasm";
  if (ends_with(path, ".wgsl")) return "text/plain; charset=utf-8";
  if (ends_with(path, ".md")) return "text/markdown; charset=utf-8";
  if (ends_with(path, ".svg")) return "image/svg+xml";
  if (ends_with(path, ".json")) return "application/json";
  if (ends_with(path, ".png")) return "image/png";
  if (ends_with(path, ".bin")) return "application/octet-stream";
  return "text/plain; charset=utf-8";
}

static void send_all(int fd, const void *data, size_t n) {
  const char *p = data;
  while (n) {
    ssize_t k = send(fd, p, n, 0);
    if (k < 0) { if (errno == EINTR) continue; return; }
    p += k; n -= (size_t)k;
  }
}

static void respond(int fd, int status, const char *reason, const char *type, const void *body, size_t n) {
  char head[512];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nCache-Control: %s\r\n"
                   "Cross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n"
                   "X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
                   status, reason, type, n, static_mode && status == 200 ? cache_control : "no-store");
  send_all(fd, head, (size_t)h);
  if (n) send_all(fd, body, n);
}

// Maps a URL path to a file: /coaxial/* from the host directory, anything
// else from the app directory; a directory ("/", "/project/coax") is its
// index.html, as the bucket serves it. No "..", no dotfiles.
static bool resolve(const char *url, char *out, size_t cap) {
  char path[1024];
  size_t i = 0;
  for (; url[i] && url[i] != '?' && url[i] != '#' && i + 1 < sizeof path; i++) path[i] = url[i];
  path[i] = 0;
  if (path[0] != '/' || strstr(path, "..") || strstr(path, "/.")) return false;
  if (!strcmp(path, "/")) return snprintf(out, cap, "%s/index.html", app_dir) < (int)cap;
  if (!strncmp(path, "/coaxial/", 9)) return snprintf(out, cap, "%s/%s", host_dir, path + 9) < (int)cap;
  if (snprintf(out, cap, "%s%s", app_dir, path) >= (int)cap) return false;
  struct stat st;
  if (stat(out, &st) == 0 && S_ISDIR(st.st_mode)) {
    size_t n = strlen(out);
    return snprintf(out + n, cap - n, "%sindex.html", out[n - 1] == '/' ? "" : "/") < (int)(cap - n);
  }
  return true;
}

// An unknown path gets the app's 404.html, if it has one, with a 404 status.
static void not_found(int fd) {
  char path[2048];
  static char page[1 << 16];
  snprintf(path, sizeof path, "%s/404.html", app_dir);
  FILE *f = fopen(path, "rb");
  size_t n = f ? fread(page, 1, sizeof page, f) : 0;
  if (f) fclose(f);
  if (n > 0 && n < sizeof page) respond(fd, 404, "Not Found", "text/html; charset=utf-8", page, n);
  else respond(fd, 404, "Not Found", "text/plain", "not found\n", 10);
}

static void serve_file(int fd, const char *url) {
  char path[2048];
  if (!resolve(url, path, sizeof path)) { not_found(fd); return; }
  FILE *f = fopen(path, "rb");
  struct stat st;
  if (!f || fstat(fileno(f), &st) || !S_ISREG(st.st_mode)) {
    if (f) fclose(f);
    not_found(fd);
    return;
  }
  char *body = malloc((size_t)st.st_size + 1);
  size_t n = body ? fread(body, 1, (size_t)st.st_size, f) : 0;
  fclose(f);
  respond(fd, 200, "OK", content_type(path), body, n);
  free(body);
}

static void drop(client *c) { close(c->fd); c->fd = -1; c->ws = false; c->n = 0; }

static void broadcast(const char *json) {
  uint8_t frame[REQ_MAX + 16];
  size_t n = strlen(json);
  if (n > REQ_MAX) n = REQ_MAX;
  size_t len = ws_build(frame, sizeof frame, WS_TEXT, (const uint8_t *)json, n, false);
  int sent = 0;
  for (int i = 0; i < MAX_CLIENTS; i++)
    if (clients[i].fd >= 0 && clients[i].ws) { send_all(clients[i].fd, frame, len); sent++; }
  (void)sent;
}

// A request is complete: a WebSocket upgrade to /coaxial/live, or a file.
static void handle_request(client *c) {
  char method[8] = {0}, url[1024] = {0};
  if (sscanf(c->buf, "%7s %1023s", method, url) != 2 || strcmp(method, "GET")) {
    respond(c->fd, 405, "Method Not Allowed", "text/plain", "GET only\n", 9); drop(c); return;
  }
  if (!strcmp(url, "/coaxial/live") && !static_mode) {
    char resp[512];
    size_t n = ws_server_handshake(c->buf, c->n, resp, sizeof resp);
    if (!n) { respond(c->fd, 400, "Bad Request", "text/plain", "", 0); drop(c); return; }
    send_all(c->fd, resp, n);
    c->ws = true; c->n = 0;
    return;
  }
  serve_file(c->fd, url);
  drop(c);
}

static void on_readable(client *c) {
  if (c->ws) {
    // Tabs only listen; read and discard (a close frame or EOF drops them).
    uint8_t tmp[4096];
    ssize_t k = recv(c->fd, tmp, sizeof tmp, 0);
    if (k <= 0) { drop(c); return; }
    if (k >= 2 && (tmp[0] & 0x0f) == WS_CLOSE) drop(c);
    return;
  }
  ssize_t k = recv(c->fd, c->buf + c->n, sizeof c->buf - 1 - c->n, 0);
  if (k <= 0) { drop(c); return; }
  c->n += (size_t)k; c->buf[c->n] = 0;
  if (strstr(c->buf, "\r\n\r\n")) handle_request(c);
  else if (c->n >= sizeof c->buf - 1) drop(c);
}

// ---- watching
static bool watched_kind(const char *name) {
  return ends_with(name, ".coax") || ends_with(name, ".c") || ends_with(name, ".h") || ends_with(name, ".wgsl") ||
         ends_with(name, ".html") || ends_with(name, ".js") || ends_with(name, ".md") || ends_with(name, ".svg");
}

static void scan_dir(const char *dir, bool add) {
  DIR *d = opendir(dir);
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d))) {
    if (e->d_name[0] == '.' || !watched_kind(e->d_name)) continue;
    char path[1024];
    snprintf(path, sizeof path, "%s/%s", dir, e->d_name);
    if (add && file_count < MAX_WATCH) {
      struct stat st;
      if (stat(path, &st)) continue;
      snprintf(files[file_count].path, sizeof files[0].path, "%s", path);
      files[file_count++].mtime = MTIME(st);
    }
  }
  closedir(d);
}

static void json_escape(const char *in, char *out, size_t cap) {
  size_t o = 0;
  for (; *in && o + 7 < cap; in++) {
    unsigned char ch = (unsigned char)*in;
    if (ch == '"' || ch == '\\') { out[o++] = '\\'; out[o++] = (char)ch; }
    else if (ch == '\n') { out[o++] = '\\'; out[o++] = 'n'; }
    else if (ch < 0x20) { o += (size_t)snprintf(out + o, cap - o, "\\u%04x", ch); }
    else out[o++] = (char)ch;
  }
  out[o] = 0;
}

// Runs the build; returns true on success with its output in log.
static bool build(char *log, size_t cap, double *ms) {
  char cmd[4200];
  snprintf(cmd, sizeof cmd, "%s 2>&1", build_cmd);
  double t0 = now_ms();
  FILE *p = popen(cmd, "r");
  size_t n = 0;
  if (p) { n = fread(log, 1, cap - 1, p); }
  log[n] = 0;
  int status = p ? pclose(p) : -1;
  *ms = now_ms() - t0;
  return status == 0;
}

static void rebuild_and_tell(void) {
  static char log[REQ_MAX / 2], escaped[REQ_MAX];
  double ms;
  if (build(log, sizeof log, &ms)) {
    char msg[128];
    snprintf(msg, sizeof msg, "{\"kind\":\"wasm\",\"ms\":%.0f}", ms);
    logf_("engine rebuilt in %.0f ms", ms);
    broadcast(msg);
  } else {
    logf_("build failed in %.0f ms:\n%s", ms, log);
    json_escape(log, escaped, sizeof escaped - 32);
    char *msg = malloc(strlen(escaped) + 40);
    sprintf(msg, "{\"kind\":\"error\",\"text\":\"%s\"}", escaped);
    broadcast(msg);
    free(msg);
  }
}

static void check_changes(void) {
  bool engine = false, shaders = false, page = false;
  for (int i = 0; i < file_count; i++) {
    struct stat st;
    if (stat(files[i].path, &st)) continue;
    if (MTIME(st).tv_sec == files[i].mtime.tv_sec && MTIME(st).tv_nsec == files[i].mtime.tv_nsec) continue;
    files[i].mtime = MTIME(st);
    const char *p = files[i].path;
    if (ends_with(p, ".coax") || ends_with(p, ".c") || ends_with(p, ".h")) engine = true;
    else if (ends_with(p, ".wgsl")) shaders = true;
    else page = true;
    logf_("changed %s", strrchr(p, '/') + 1);
  }
  if (engine) rebuild_and_tell();
  if (shaders) broadcast("{\"kind\":\"wgsl\"}");
  if (page) broadcast("{\"kind\":\"page\"}");
}

int main(int argc, char **argv) {
  const char *listen_spec = "127.0.0.1:8800";
  const char *app = NULL, *cmd = NULL, *hosts = NULL;
  static char port_spec[64];
  if (getenv("PORT")) { snprintf(port_spec, sizeof port_spec, "0.0.0.0:%s", getenv("PORT")); listen_spec = port_spec; }
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) listen_spec = argv[++i];
    else if (!strcmp(argv[i], "--host-dir") && i + 1 < argc) hosts = argv[++i];
    else if (!strcmp(argv[i], "--static")) static_mode = true;
    else if (!strcmp(argv[i], "--cache") && i + 1 < argc) {
      int seconds = atoi(argv[++i]);
      if (seconds > 0) snprintf(cache_control, sizeof cache_control, "public, max-age=%d", seconds);
      else snprintf(cache_control, sizeof cache_control, "no-cache");
    }
    else if (!strcmp(argv[i], "--build") && i + 1 < argc) cmd = argv[++i];
    else if (!app) app = argv[i];
    else { fprintf(stderr, "usage: serve APP_DIR [--listen HOST:PORT] [--build CMD]\n"); return 2; }
  }
  if (!app || !realpath(app, app_dir)) { fprintf(stderr, "usage: serve APP_DIR [--listen HOST:PORT] [--build CMD]\n"); return 2; }
  if (hosts) {
    if (!realpath(hosts, host_dir)) { fprintf(stderr, "serve: no host directory %s\n", hosts); return 2; }
  } else {
    // The binary lives in coax/web/build/; the host files in coax/web/.
    char self[1024];
    if (!realpath(argv[0], self)) { fprintf(stderr, "serve: cannot find itself\n"); return 2; }
    char *slash = strrchr(self, '/'); *slash = 0;
    slash = strrchr(self, '/'); *slash = 0;
    snprintf(host_dir, sizeof host_dir, "%s", self);
  }
  if (cmd) snprintf(build_cmd, sizeof build_cmd, "%s", cmd);
  else snprintf(build_cmd, sizeof build_cmd, "'%s/build.sh'", app_dir);

  char host[64] = "127.0.0.1"; int port = 8800;
  const char *colon = strrchr(listen_spec, ':');
  if (colon) { snprintf(host, sizeof host, "%.*s", (int)(colon - listen_spec), listen_spec); port = atoi(colon + 1); }
  signal(SIGPIPE, SIG_IGN);
  int lfd = socket(AF_INET, SOCK_STREAM, 0);
  int one = 1; setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons((uint16_t)port)};
  if (inet_pton(AF_INET, host, &a.sin_addr) != 1 || bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 64)) {
    fprintf(stderr, "serve: cannot listen on %s: %s\n", listen_spec, strerror(errno));
    return 1;
  }
  for (int i = 0; i < MAX_CLIENTS; i++) clients[i].fd = -1;
  if (!static_mode) {
    scan_dir(app_dir, true);
    scan_dir(host_dir, true);
    static char log[REQ_MAX / 2];
    double ms;
    if (!build(log, sizeof log, &ms)) logf_("first build failed in %.0f ms:\n%s", ms, log);
    else logf_("first build in %.0f ms", ms);
  }
  printf("coax serve: http://%s:%d/  (app %s, %s)\n", host, port, app_dir, static_mode ? "static" : "watching");
  fflush(stdout);

  double last_scan = now_ms();
  for (;;) {
    struct pollfd p[MAX_CLIENTS + 1];
    int map[MAX_CLIENTS + 1], n = 0;
    p[n].fd = lfd; p[n].events = POLLIN; map[n++] = -1;
    for (int i = 0; i < MAX_CLIENTS; i++)
      if (clients[i].fd >= 0) { p[n].fd = clients[i].fd; p[n].events = POLLIN; map[n++] = i; }
    int r = poll(p, (nfds_t)n, 50);
    if (r < 0 && errno != EINTR) break;
    for (int k = 0; r > 0 && k < n; k++) {
      if (!(p[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (map[k] < 0) {
        int fd = accept(lfd, NULL, NULL);
        if (fd < 0) continue;
        int slot = -1;
        for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd < 0) { slot = i; break; }
        if (slot < 0) { close(fd); continue; }
        clients[slot].fd = fd; clients[slot].ws = false; clients[slot].n = 0;
      } else {
        on_readable(&clients[map[k]]);
      }
    }
    if (!static_mode && now_ms() - last_scan >= 50) { last_scan = now_ms(); check_changes(); }
  }
  return 0;
}
