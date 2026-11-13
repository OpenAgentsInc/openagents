// coax serve: Coaxial's development server. One event loop; shaped file
// connections use children so a slow download cannot stall the other clients.
//
// It serves an app directory over HTTP, watches every file the app's build
// read, rebuilds what changed, and tells every open tab over a WebSocket
// (/coaxial/live) what to swap:
//   a module's source   rebuild that module alone, then "wasm" naming it
//   a Coax shader's source   write its WGSL again, then "wgsl" naming it
//   .wgsl        "wgsl": the tab recompiles its shaders
//   .html .js .md .svg   "page": the tab reloads
//   a pack or texture a tab loaded   "asset" with its URL and SHA-256
// A failed build sends "error" with the compiler's output; the tab keeps
// running the last good engine.
//
// What the build read is its dependency lists (#13410), APP_DIR/build/.live/*.deps,
// one per module or shader, written by coax/web/wasm.sh (COAXIAL_MODE=dev) and
// coax/web/wgsl-module.coax:
//   coax-live-1
//   kind wasm|wgsl
//   out ABSOLUTE_PATH          what the unit writes
//   cmd SHELL COMMAND          builds this unit alone again
//   src ABSOLUTE_PATH          one line for each file the build read
// A change to a listed source runs only the commands of the units that read it,
// side by side. A watched .coax, .c or .h in the app's folders that no list
// names runs the whole build (--build), as does the first build. A tab names
// the packs and textures it loaded ("watch URL..."), and those files are
// watched too. The watch has no fixed size.
//
//   serve APP_DIR [--listen HOST:PORT] [--build CMD] [--host-dir DIR] [--static] [--cache SECONDS] [--api PORT]
//         [--realm-host HOST:PORT]   requests whose Host is this are the realm origin's (#13432): only a
//                                    realm's page and what it loads, every answer sandboxed with no origin
//     [--throttle-mbps N] [--latency-ms M]
// Shaping covers HTTP responses, including API forwarding and headers: N is decimal
// Mbit/s per connection (0 disables; minimum 0.001), M delays the first byte.
// Upgraded WebSockets (including hot reload) are not shaped.
// Defaults: 127.0.0.1:8800, build command APP_DIR/build.sh, the host files
// next to this binary's directory (coax/web/).
//
// --api PORT sends every /api/ request to 127.0.0.1:PORT and its answer back, as the load balancer
// sends /api/ to the API: a local room's stand-in for the API (coax/web/room/local.coax), so a page
// asks the same paths it asks in production. A WebSocket there (the room's socket) is carried both
// ways, so a page with no ?relay= still sits in the local room.
//
// --static is production: no building, no watching, no live channel; files
// are served as they are, cacheable for five minutes (--cache SECONDS; 0
// means browsers check back every time). It listens on
// 0.0.0.0:$PORT when PORT is set (Cloud Run). TLS is the platform's.
#include <arpa/inet.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <math.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "../../metaverse/voice-host/ws.h"
#include "http-range.h"
#include "sha256-kernel.h"
#include "site-routes.h"
#include "realm-origin.h"

#define MAX_CLIENTS 64
#define REQ_MAX 16384
#define HTTP_REQUEST_MAX 65536 // inline take64 query; live/build messages keep REQ_MAX

typedef struct { int fd; bool ws; size_t n; char buf[HTTP_REQUEST_MAX]; } client;

static client clients[MAX_CLIENTS];
// realpath's buffers are PATH_MAX: glibc's _FORTIFY_SOURCE (on by default under Nix) aborts a smaller one.
static char app_dir[PATH_MAX], host_dir[PATH_MAX], build_cmd[PATH_MAX + 64];
static bool static_mode;
static char cache_control[64] = "public, max-age=300";
// --realm-host HOST:PORT (#13432, coax/web/realm-origin.coax): a request whose Host is this is the
// realm origin's. It answers only a realm's page and what a page loads, and every answer carries
// the realm origin's headers: its documents are sandboxed with no origin of their own.
static const char *realm_host = "";
static bool realm_request;   // the request being answered is the realm origin's
static const char *realm_headers(void) {
  static char text[512];
  if (!realm_request) return "";
  if (!text[0]) { coax_string h = realm_origin_headers(); snprintf(text, sizeof text, "%.*s", (int)h.count, h.data); }
  return text;
}
static int api_port;   // --api: /api/ requests go to 127.0.0.1:api_port
static int local_port; // --local: /local/ requests go to `coder mcp --http` on 127.0.0.1:local_port (its local routes)
static double throttle_bytes, latency_ms, next_send_ms;
static bool shaping_send;
static int shaping_fd = -1;
static int listen_fd = -1;
static pid_t build_pid;
static int build_fd = -1;
static double build_started;
static char build_log[REQ_MAX / 2];
static size_t build_used;
static bool build_page;
static bool build_targeted;   // the running build is the changed units' own commands, not --build
// A targeted build whose modules are all written and told: wasm.sh still writes its drive
// files and dependency list, but the next edit doesn't wait for that. Reaped quietly.
typedef struct { pid_t pid; int fd; } tail;
static tail *tails;
static int tail_count, tail_cap;
static void tab_says(const uint8_t *text, size_t n);

// This server forks only on its event-loop thread. Mark every owned socket
// and pipe before the next fork, including accepted live-channel clients.
static int private_fd(int fd) {
  if (fd < 0) return -1;
  int flags = fcntl(fd, F_GETFD);
  if (flags >= 0 && fcntl(fd, F_SETFD, flags | FD_CLOEXEC) == 0) return fd;
  int error = errno; close(fd); errno = error; return -1;
}

#ifdef __APPLE__
#define MTIME(st) ((st).st_mtimespec)
#else
#define MTIME(st) ((st).st_mtim)
#endif

static double now_ms(void) {
  struct timeval tv; gettimeofday(&tv, NULL);
  return tv.tv_sec * 1000.0 + tv.tv_usec / 1000.0;
}
static double monotonic_ms(void) {
  struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec * 1000.0 + t.tv_nsec / 1000000.0;
}
static void sleep_until(double deadline) {
  for (;;) {
    double ms = deadline - monotonic_ms();
    if (ms <= 0) return;
    struct timespec t = {.tv_sec = (time_t)(ms / 1000), .tv_nsec = (long)(fmod(ms, 1000) * 1000000)};
    if (!nanosleep(&t, NULL) || errno != EINTR) return;
  }
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
  if (ends_with(path, ".jpg")) return "image/jpeg";
  if (ends_with(path, ".bin")) return "application/octet-stream";
  if (ends_with(path, ".ogg")) return "audio/ogg";
  return "text/plain; charset=utf-8";
}

static bool send_all(int fd, const void *data, size_t n) {
  const char *p = data;
  while (n) {
    bool paced = shaping_send && fd == shaping_fd && throttle_bytes > 0;
    if (paced) sleep_until(next_send_ms);
    size_t part = paced && n > 4096 ? 4096 : n;
    ssize_t k = send(fd, p, part, 0);
    if (k < 0) { if (errno == EINTR) continue; return false; }
    if (!k) return false;
    if (paced) next_send_ms = monotonic_ms() + (double)k * 1000.0 / throttle_bytes;
    p += k; n -= (size_t)k;
  }
  return true;
}

static void respond(int fd, int status, const char *reason, const char *type, const void *body, size_t n) {
  char head[1024];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nCache-Control: %s\r\n"
                   "Cross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n%s"
                   "X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
                   status, reason, type, n, static_mode && status == 200 ? cache_control : "no-store", realm_headers());
  if (!send_all(fd, head, (size_t)h)) return;
  if (n) send_all(fd, body, n);
}

// Maps a URL path to a file: /coaxial/* from the host directory, anything
// else from the app directory; a directory ("/", "/projects/coax") is its
// index.html, as the bucket serves it. No "..", no dotfiles.
static bool resolve(const char *url, char *out, size_t cap) {
  char path[PATH_MAX + 1024];
  size_t i = 0;
  for (; url[i] && url[i] != '?' && url[i] != '#' && i + 1 < sizeof path; i++) path[i] = url[i];
  path[i] = 0;
  if (path[0] != '/' || strstr(path, "..") || strstr(path, "/.")) return false;
  // An address whose files have another name (site-routes.coax: /projects/metaverse is projects/verse/).
  coax_t_SiteRoute route = site_route((coax_string){path, (int64_t)strlen(path)});
  if (route.status == 0 && route.to.count > 0) {
    char named[sizeof path];
    if (snprintf(named, sizeof named, "%.*s%s", (int)route.to.count, route.to.data, path + route.skip) >= (int)sizeof named) return false;
    memcpy(path, named, strlen(named) + 1);
  }
  if (!strcmp(path, "/")) return snprintf(out, cap, "%s/index.html", app_dir) < (int)cap;
  if (!strncmp(path, "/coaxial/", 9)) return snprintf(out, cap, "%s/%s", host_dir, path + 9) < (int)cap;
  // /private/* (watching only, never --static): private builds outside the repo, such as a MetaHuman
  // pack (docs/metaverse/metahuman-support.md), from ~/.openagents/verse/private-build/web/. A deploy never
  // has them: the host asks for one only on a local address (coax/web/host.js "private:").
  if (!strncmp(path, "/private/", 9)) {
    const char *home = getenv("HOME");
    if (static_mode || !home || !path[9]) return false;
    return snprintf(out, cap, "%s/.openagents/verse/private-build/web/%s", home, path + 9) < (int)cap;
  }
  // /worlds and /worlds/* are the app's realms/, as the load balancer serves them (coax/web/api/lb.sh)
  if (!strcmp(path, "/worlds") || !strncmp(path, "/worlds/", 8)) {
    if (snprintf(out, cap, "%s/realms%s", app_dir, path + 7) >= (int)cap) return false;
  } else if (snprintf(out, cap, "%s%s", app_dir, path) >= (int)cap) return false;
  struct stat st;
  if (stat(out, &st) == 0 && S_ISDIR(st.st_mode)) {
    size_t n = strlen(out);
    return snprintf(out + n, cap - n, "%sindex.html", out[n - 1] == '/' ? "" : "/") < (int)(cap - n);
  }
  return true;
}

// An unknown path gets the app's 404.html, if it has one, with a 404 status.
static void not_found(int fd) {
  char path[PATH_MAX + 1024];
  static char page[1 << 16];
  snprintf(path, sizeof path, "%s/404.html", app_dir);
  FILE *f = fopen(path, "rb");
  size_t n = f ? fread(page, 1, sizeof page, f) : 0;
  if (f) fclose(f);
  if (n > 0 && n < sizeof page) respond(fd, 404, "Not Found", "text/html; charset=utf-8", page, n);
  else respond(fd, 404, "Not Found", "text/plain", "not found\n", 10);
}

// Header transport only; the Coax policy selects the range. Duplicate Range
// fields and If-Range (we do not emit validators) fall back to the full body.
// The request's Host header, trimmed, into out ("" when it has none or it does not fit).
static void request_host(const char *request, char *out, size_t cap) {
  out[0] = 0;
  const char *p = strstr(request, "\r\n");
  while (p && p[2] && p[2] != '\r') {
    p += 2; const char *end = strstr(p, "\r\n"); if (!end) return;
    if (!strncasecmp(p, "Host:", 5)) {
      const char *v = p + 5; while (v < end && (*v == ' ' || *v == '\t')) v++;
      const char *e = end; while (e > v && (e[-1] == ' ' || e[-1] == '\t')) e--;
      size_t n = (size_t)(e - v); if (n >= cap) return;
      memcpy(out, v, n); out[n] = 0; return;
    }
    p = end;
  }
}
static void range_header(const char *request, char *out, size_t cap) {
  out[0] = 0; bool seen = false;
  const char *p = strstr(request, "\r\n");
  while (p && p[2] && p[2] != '\r') {
    p += 2; const char *end = strstr(p, "\r\n"); if (!end) return;
    if (!strncasecmp(p, "If-Range:", 9)) { out[0] = 0; return; }
    if (!strncasecmp(p, "Range:", 6)) {
      if (seen) { out[0] = 0; return; } seen = true;
      const char *v = p + 6; while (v < end && (*v == ' ' || *v == '\t')) v++;
      const char *e = end; while (e > v && (e[-1] == ' ' || e[-1] == '\t')) e--;
      size_t n = (size_t)(e - v); if (n >= cap) return;
      memcpy(out, v, n); out[n] = 0;
    }
    p = end;
  }
}
#define BIG_BODY (4 * 1024 * 1024)   // bytes: a body longer than this is sent by a child (serve_file)
// An address that moved for good (site-routes.coax, as the load balancer answers it: coax/web/api/lb.sh):
// 301 to the new one, with the request's query when the route keeps it. Whether it answered.
static bool moved(int fd, const char *url) {
  size_t n = strcspn(url, "?#");
  coax_t_SiteRoute route = site_route((coax_string){url, (int64_t)n});
  if (route.status != 301) return false;
  const char *query = route.query && url[n] == '?' ? url + n : "";
  char head[2600];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 301 Moved Permanently\r\nLocation: %.*s%.*s%.*s\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                   (int)route.to.count, route.to.data, (int)(n - (size_t)route.skip), url + route.skip, (int)strcspn(query, "#"), query);
  if (h > 0 && h < (int)sizeof head) send_all(fd, head, (size_t)h);
  return true;
}

static void serve_file(int fd, const char *url, const char *request) {
  char path[PATH_MAX + 1024];
  if (moved(fd, url)) return;
  if (!resolve(url, path, sizeof path)) { not_found(fd); return; }
  FILE *f = fopen(path, "rb");
  struct stat st;
  if (!f || fstat(fileno(f), &st) || !S_ISREG(st.st_mode)) {
    if (f) fclose(f);
    not_found(fd);
    return;
  }
  char value[HTTP_REQUEST_MAX]; range_header(request, value, sizeof value);
  coax_t_HttpRange r = http_range((coax_string){value, (int64_t)strlen(value)}, st.st_size);
  char extra[160] = "";
  if (r.status == 206) snprintf(extra, sizeof extra, "Content-Range: bytes %lld-%lld/%lld\r\n", (long long)r.first, (long long)(r.first + r.length - 1), (long long)st.st_size);
  if (r.status == 416) snprintf(extra, sizeof extra, "Content-Range: bytes */%lld\r\n", (long long)st.st_size);
  char head[1536];
  int h = snprintf(head, sizeof head,
    "HTTP/1.1 %lld %s\r\nContent-Type: %s\r\nContent-Length: %lld\r\nAccept-Ranges: bytes\r\n%s"
    "Cache-Control: %s\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n%s"
    "X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
    (long long)r.status, r.status == 206 ? "Partial Content" : r.status == 416 ? "Range Not Satisfiable" : "OK",
    content_type(path), (long long)r.length, extra, static_mode && r.status != 416 ? cache_control : "no-store", realm_headers());
  if (fseeko(f, (off_t)r.first, SEEK_SET)) { fclose(f); respond(fd,500,"Read Failed","text/plain","",0); return; }
  // A large body (a realm's pack, 90 MB) goes out from a child of its own. Sent here, it held every
  // other request (the page's shaders, fonts and sounds) until the browser had read all of it (#13141).
  bool child = false;
  if (!shaping_send && r.length > BIG_BODY) {
    pid_t pid = fork();
    if (pid > 0) { fclose(f); return; }   // the caller drops this process's copy of the socket
    if (pid == 0) {
      child = true; close(listen_fd);
      if (build_fd >= 0) close(build_fd);
      for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd >= 0 && clients[i].fd != fd) close(clients[i].fd);
    }
  }
  if (!send_all(fd, head, (size_t)h)) {fclose(f);if (child) _exit(0);return;}
  char body[16384]; int64_t left = r.length;
  while (left > 0) {
    size_t want = left < (int64_t)sizeof body ? (size_t)left : sizeof body;
    size_t n = fread(body, 1, want, f); if (!n) break;
    if (!send_all(fd, body, n)) break;
    left -= (int64_t)n;
  }
  fclose(f);
  if (child) _exit(0);
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

// Whether the request asks for a WebSocket (an Upgrade: websocket header).
static bool is_upgrade(const client *c) {
  const char *end = strstr(c->buf, "\r\n\r\n");
  if (!end) return false;
  for (const char *p = c->buf; p + 8 < end; p++)
    if ((p == c->buf || p[-1] == '\n') && !strncasecmp(p, "Upgrade:", 8)) {
      const char *v = p + 8;
      while (*v == ' ' || *v == '\t') v++;
      return !strncasecmp(v, "websocket", 9);
    }
  return false;
}
// A WebSocket to the --api stand-in (the room's socket, /api/room/relay/...): carried both ways until
// either side closes, as the load balancer carries it in production. A child process carries it, so
// the server goes on answering other requests meanwhile; the child holds only these two sockets.
static void tunnel(int page, int api) {
  pid_t pid = fork();
  if (pid != 0) return;   // the server (or a failed fork: the socket closes, the page asks again)
  for (int f = 3; f < 1024; f++) if (f != page && f != api) close(f);
  struct pollfd p[2] = {{.fd = page, .events = POLLIN}, {.fd = api, .events = POLLIN}};
  char tmp[16384];
  for (;;) {
    if (poll(p, 2, -1) < 0) { if (errno == EINTR) continue; break; }
    bool done = false;
    for (int i = 0; i < 2; i++) {
      if (!(p[i].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      ssize_t k = recv(p[i].fd, tmp, sizeof tmp, 0);
      if (k <= 0) { done = true; break; }
      send_all(p[1 - i].fd, tmp, (size_t)k);
    }
    if (done) break;
  }
  _exit(0);
}
// An /api/ request, its body whole, to the --api stand-in and the answer back to the page.
static void proxy_api(client *c) {
  int fd = private_fd(socket(AF_INET, SOCK_STREAM, 0));
  struct sockaddr_in a = {0};
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)api_port);
  inet_pton(AF_INET, "127.0.0.1", &a.sin_addr);
  if (fd < 0 || connect(fd, (struct sockaddr *)&a, sizeof a)) {
    if (fd >= 0) close(fd);
    respond(c->fd, 502, "Bad Gateway", "application/json", "{\"error\":\"the local API is not running\"}", 40); drop(c); return;
  }
  send_all(fd, c->buf, c->n);
  if (is_upgrade(c)) { tunnel(c->fd, fd); close(fd); drop(c); return; }
  // A child carries the answer back, so the server goes on answering meanwhile: an answer can take
  // minutes (a model's stream, a key being checked), and the API's own helpers call back through this
  // server while it waits (/chat's runner asks for its turn), which a server that waited would never hear.
  pid_t pid = fork();
  if (pid == 0) {
    char tmp[16384]; ssize_t k;
    while ((k = recv(fd, tmp, sizeof tmp, 0)) > 0) send_all(c->fd, tmp, (size_t)k);
    _exit(0);
  }
  if (pid < 0) respond(c->fd, 503, "Service Unavailable", "application/json", "{\"error\":\"the dev server is busy\"}", 35);
  close(fd);
  drop(c);
}
// A /local/ request to Coder on this machine (`coder mcp --http PORT`, coder/tui/mcp_http.coax): /local/X
// goes on as /X with the header Coder's local routes ask for. Those routes start agents with a shell, so
// only this server's own pages may use them: the browser must say the request is same-origin
// (Sec-Fetch-Site, which a page can't set), and Coder checks the Origin again.
static void proxy_local(client *c, const char *url) {
  const char *line_end = strstr(c->buf, "\r\n");
  bool same = false;
  for (const char *p = c->buf; line_end && (p = strstr(p, "\r\n")) && p[2] && p[2] != '\r'; p += 2)
    if (!strncasecmp(p + 2, "Sec-Fetch-Site:", 15)) { const char *v = p + 17; while (*v == ' ') v++; same = !strncmp(v, "same-origin\r", 12); }
  if (!line_end || !same) { respond(c->fd, 403, "Forbidden", "application/json", "{\"error\":\"only this page may use Coder\"}", 38); drop(c); return; }
  int fd = private_fd(socket(AF_INET, SOCK_STREAM, 0));
  struct sockaddr_in a = {0};
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)local_port);
  inet_pton(AF_INET, "127.0.0.1", &a.sin_addr);
  if (fd < 0 || connect(fd, (struct sockaddr *)&a, sizeof a)) {
    if (fd >= 0) close(fd);
    respond(c->fd, 502, "Bad Gateway", "application/json", "{\"error\":\"Coder is not running here\"}", 36); drop(c); return;
  }
  char method[8] = {0}, first[1200];
  sscanf(c->buf, "%7s", method);
  int n = snprintf(first, sizeof first, "%s %s HTTP/1.1\r\nX-Coder-Local: 1", method, url + 6);
  send_all(fd, first, (size_t)n);
  send_all(fd, line_end, c->n - (size_t)(line_end - c->buf));
  pid_t pid = fork();
  if (pid == 0) {
    char tmp[16384]; ssize_t k;
    while ((k = recv(fd, tmp, sizeof tmp, 0)) > 0) send_all(c->fd, tmp, (size_t)k);
    _exit(0);
  }
  if (pid < 0) respond(c->fd, 503, "Service Unavailable", "application/json", "{\"error\":\"the dev server is busy\"}", 35);
  close(fd);
  drop(c);
}
// Whether the request's body (Content-Length) is all in.
static bool body_in(const client *c) {
  const char *end = strstr(c->buf, "\r\n\r\n");
  if (!end) return false;
  size_t head = (size_t)(end - c->buf) + 4, want = 0;
  for (const char *p = c->buf; p < end; p++)
    if ((p == c->buf || p[-1] == '\n') && !strncasecmp(p, "Content-Length:", 15)) want = (size_t)strtoul(p + 15, NULL, 10);
  return c->n >= head + want || head + want >= sizeof c->buf - 1;
}

// A request is complete: a WebSocket upgrade to /coaxial/live, an /api/ request (--api), or a file.
static void handle_request(client *c) {
  if (!shaping_send && (throttle_bytes > 0 || latency_ms > 0) && !is_upgrade(c)) {
    pid_t pid = fork();
    if (pid < 0) {respond(c->fd,503,"Busy","text/plain","",0);drop(c);return;}
    if (pid == 0) {
      close(listen_fd);
      if (build_fd >= 0) close(build_fd);
      for (int i=0;i<MAX_CLIENTS;i++) if (clients[i].fd>=0 && clients[i].fd!=c->fd) close(clients[i].fd);
      sleep_until(monotonic_ms()+latency_ms);
      shaping_send = true; shaping_fd = c->fd;
      next_send_ms = monotonic_ms();
      handle_request(c);_exit(0);
    }
    drop(c);return;
  }
  char method[8] = {0}, url[1024] = {0};
  // The realm origin (--realm-host): by the request's Host. It answers a realm's page and what a page
  // loads, and nothing else: no API, no live channel, no page of the site's (realm-origin.coax).
  realm_request = false;
  if (realm_host[0]) {
    char host[256]; request_host(c->buf, host, sizeof host);
    realm_request = realm_origin_host((coax_string){host, (int64_t)strlen(host)}, (coax_string){realm_host, (int64_t)strlen(realm_host)});
    if (realm_request && (sscanf(c->buf, "%7s %1023s", method, url) != 2 || strcmp(method, "GET") || !realm_origin_serves((coax_string){url, (int64_t)strlen(url)}))) {
      respond(c->fd, 404, "Not Found", "text/plain", "not on the realm origin\n", 24); drop(c); return;
    }
  }
  if (api_port && sscanf(c->buf, "%7s %1023s", method, url) == 2 && !strncmp(url, "/api/", 5)) { proxy_api(c); return; }
  if (local_port && sscanf(c->buf, "%7s %1023s", method, url) == 2 && !strncmp(url, "/local/", 7)) { proxy_local(c, url); return; }
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
  serve_file(c->fd, url, c->buf);
  drop(c);
}

static void on_readable(client *c) {
  if (c->ws) {
    // A tab says only which files it loaded ("watch URL..."); a close frame or EOF drops it.
    ssize_t k = recv(c->fd, c->buf + c->n, sizeof c->buf - c->n, 0);
    if (k <= 0) { drop(c); return; }
    c->n += (size_t)k;
    for (;;) {
      ws_frame f;
      int r = ws_parse((uint8_t *)c->buf, c->n, sizeof c->buf - 16, &f);
      if (r < 0 || (r == 0 && c->n >= sizeof c->buf)) { drop(c); return; }
      if (r == 0) return;
      if (f.opcode == WS_CLOSE) { drop(c); return; }
      if (f.opcode == WS_TEXT && f.fin) tab_says(f.payload, f.length);
      memmove(c->buf, c->buf + f.consumed, c->n - f.consumed); c->n -= f.consumed;
    }
  }
  ssize_t k = recv(c->fd, c->buf + c->n, sizeof c->buf - 1 - c->n, 0);
  if (k <= 0) { drop(c); return; }
  c->n += (size_t)k; c->buf[c->n] = 0;
  if (strstr(c->buf, "\r\n\r\n") && ((!api_port && !local_port) || body_in(c))) handle_request(c);
  else if (c->n >= sizeof c->buf - 1) drop(c);
}

// ---- watching
// Files: one row a path, in a table that grows (no cap), found by a hash of the
// path. A row is watched for what put it there: SCAN (a page, shader or source
// file found in the app's folders) or SRC (a unit's dependency list names it).
// NEAR: a source file in a folder a unit's list reads from, that no list names (#13441): a
// new one there is seen at once, and builds again the units whose last build failed.
enum { ROLE_SCAN = 1, ROLE_SRC = 2, ROLE_NEAR = 4 };
typedef struct { char *path; struct timespec mtime; int roles; } watched;
// A unit: one module or shader the build writes, from its .live/NAME.deps.
// started: when its last build began to read its sources (.live/NAME.started; the output's time without one).
typedef struct { bool wgsl; char *out, *cmd; int *src; int nsrc, src_cap; struct timespec out_mtime, started; bool dirty, told; } unit;
// A file a tab loaded (a pack, a texture), by its URL; hash is its SHA-256 when known.
typedef struct { char *url, *path; struct timespec mtime; char hash[65]; } loaded;
static watched *files; static int file_count, file_cap;
static int *slots; static int slot_cap;   // indexes into files, -1 empty; a power of two
static unit *units; static int unit_count, unit_cap;
static loaded *assets; static int asset_count, asset_cap;
static char **notes; static int note_count, note_cap;   // .wgsl paths changed, told once each
static char **near_dirs; static int near_count, near_cap;   // folders outside the app that a unit's list reads from
static char **failed_outs; static int failed_count, failed_cap;   // the units whose last build failed, by output
static struct timespec build_wall;   // the running build's start, on the files' clock
static bool later(struct timespec a, struct timespec b) { return a.tv_sec > b.tv_sec || (a.tv_sec == b.tv_sec && a.tv_nsec > b.tv_nsec); }

static void *grow(void *p, int *cap, int need, size_t size) {
  if (need <= *cap) return p;
  int next = *cap ? *cap : 64;
  while (next < need) next *= 2;
  void *q = realloc(p, (size_t)next * size);
  if (!q) { logf_("out of memory watching files"); exit(1); }
  *cap = next; return q;
}
static char *copy(const char *s) { char *t = strdup(s); if (!t) { logf_("out of memory watching files"); exit(1); } return t; }
static bool same_time(struct timespec a, struct timespec b) { return a.tv_sec == b.tv_sec && a.tv_nsec == b.tv_nsec; }
static struct timespec mtime_of(const char *path) {
  struct stat st; struct timespec none = {0, 0};
  return stat(path, &st) == 0 ? MTIME(st) : none;
}
static uint64_t path_hash(const char *s) { uint64_t h = 1469598103934665603ull; for (; *s; s++) h = (h ^ (uint8_t)*s) * 1099511628211ull; return h; }
static int find_file(const char *path) {
  if (!slot_cap) return -1;
  for (uint64_t at = path_hash(path) & (uint64_t)(slot_cap - 1);; at = (at + 1) & (uint64_t)(slot_cap - 1)) {
    if (slots[at] < 0) return -1;
    if (!strcmp(files[slots[at]].path, path)) return slots[at];
  }
}
static void place(int i) {
  uint64_t at = path_hash(files[i].path) & (uint64_t)(slot_cap - 1);
  while (slots[at] >= 0) at = (at + 1) & (uint64_t)(slot_cap - 1);
  slots[at] = i;
}
// The row for path (added with the file's time now: a missing file is time 0, so its
// appearing is a change), with role added.
static int watch_file(const char *path, int role) {
  int i = find_file(path);
  if (i < 0) {
    if ((file_count + 1) * 2 > slot_cap) {   // keep the table at most half full
      int next = slot_cap ? slot_cap * 2 : 1024;
      while ((file_count + 1) * 2 > next) next *= 2;
      int *s = realloc(slots, (size_t)next * sizeof *s);
      if (!s) { logf_("out of memory watching files"); exit(1); }
      slots = s; slot_cap = next;
      for (int k = 0; k < slot_cap; k++) slots[k] = -1;
      for (int k = 0; k < file_count; k++) place(k);
    }
    files = grow(files, &file_cap, file_count + 1, sizeof *files);
    i = file_count++;
    files[i] = (watched){copy(path), mtime_of(path), 0};
    place(i);
  }
  files[i].roles |= role;
  return i;
}
static void watch_reset(void) {
  for (int i = 0; i < file_count; i++) free(files[i].path);
  for (int i = 0; i < unit_count; i++) { free(units[i].out); free(units[i].cmd); free(units[i].src); }
  for (int i = 0; i < near_count; i++) free(near_dirs[i]);
  file_count = unit_count = near_count = 0;
  for (int k = 0; k < slot_cap; k++) slots[k] = -1;
}

static bool engine_kind(const char *name) { return ends_with(name, ".coax") || ends_with(name, ".c") || ends_with(name, ".h"); }
static bool page_kind(const char *name) {
  return ends_with(name, ".html") || ends_with(name, ".js") || ends_with(name, ".md") || ends_with(name, ".svg");
}
// The app's folders, all the way down (nested app folders: realms/NAME/index.html), but not
// build/ (what the build writes) or a dot folder. sources: also .coax, .c and .h, which run the
// whole build when no unit's list names them. The host folder is only its top level.
static void scan_dir(const char *dir, bool deep, bool sources) {
  DIR *d = opendir(dir);
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d))) {
    if (e->d_name[0] == '.') continue;
    char path[PATH_MAX + 1024];
    if (snprintf(path, sizeof path, "%s/%s", dir, e->d_name) >= (int)sizeof path) continue;
    struct stat st;
    if (lstat(path, &st)) continue;
    if (S_ISDIR(st.st_mode)) {
      if (deep && strcmp(e->d_name, "build") && strcmp(e->d_name, "node_modules")) scan_dir(path, deep, sources);
      continue;
    }
    if (!S_ISREG(st.st_mode)) continue;
    if (page_kind(e->d_name) || ends_with(e->d_name, ".wgsl") || (sources && engine_kind(e->d_name))) watch_file(path, ROLE_SCAN);
  }
  closedir(d);
}
// A unit's output with its folder's symlinks resolved, as app_dir is (/tmp is /private/tmp
// on a Mac), so url_of finds it; as written when the folder doesn't resolve.
static char *real_out(const char *path) {
  char dir[PATH_MAX + 1024], real[PATH_MAX], joined[PATH_MAX + 1024];
  const char *slash = strrchr(path, '/');
  if (!slash || slash == path || (size_t)(slash - path) >= sizeof dir) return copy(path);
  memcpy(dir, path, (size_t)(slash - path)); dir[slash - path] = 0;
  if (!realpath(dir, real) || snprintf(joined, sizeof joined, "%s%s", real, slash) >= (int)sizeof joined) return copy(path);
  return copy(joined);
}
// The folder of a listed source outside the app (the app's own folders are scanned whole).
static void near_dir(const char *path) {
  size_t a = strlen(app_dir);
  if (!strncmp(path, app_dir, a) && path[a] == '/') return;
  const char *slash = strrchr(path, '/');
  if (!slash || slash == path) return;
  size_t n = (size_t)(slash - path);
  for (int i = 0; i < near_count; i++) if (strlen(near_dirs[i]) == n && !strncmp(near_dirs[i], path, n)) return;
  near_dirs = grow(near_dirs, &near_cap, near_count + 1, sizeof *near_dirs);
  char *d = malloc(n + 1);
  if (!d) { logf_("out of memory watching files"); exit(1); }
  memcpy(d, path, n); d[n] = 0;
  near_dirs[near_count++] = d;
}
// The source files of those folders that no row has yet, each added as NEAR.
static void scan_near(void) {
  for (int i = 0; i < near_count; i++) {
    DIR *d = opendir(near_dirs[i]);
    if (!d) continue;
    struct dirent *e;
    while ((e = readdir(d))) {
      if (e->d_name[0] == '.' || !engine_kind(e->d_name)) continue;
      char path[PATH_MAX + 1024];
      if (snprintf(path, sizeof path, "%s/%s", near_dirs[i], e->d_name) >= (int)sizeof path || find_file(path) >= 0) continue;
      struct stat st;
      if (lstat(path, &st) || !S_ISREG(st.st_mode)) continue;
      watch_file(path, ROLE_NEAR);
    }
    closedir(d);
  }
}
// Every APP_DIR/build/.live/*.deps: the units and every file each one read.
static void load_deps(void) {
  char dir[PATH_MAX + 64];
  snprintf(dir, sizeof dir, "%s/build/.live", app_dir);
  DIR *d = opendir(dir);
  if (!d) return;
  struct dirent *e;
  char *line = NULL; size_t cap = 0;
  while ((e = readdir(d))) {
    if (e->d_name[0] == '.' || !ends_with(e->d_name, ".deps")) continue;
    char path[PATH_MAX + 1024];
    if (snprintf(path, sizeof path, "%s/%s", dir, e->d_name) >= (int)sizeof path) continue;
    FILE *f = fopen(path, "r");
    if (!f) continue;
    unit u = {0};
    bool ok = getline(&line, &cap, f) > 0 && !strcmp(line, "coax-live-1\n");
    ssize_t n;
    while (ok && (n = getline(&line, &cap, f)) > 0) {
      if (line[n - 1] == '\n') line[--n] = 0;
      if (!strncmp(line, "kind ", 5)) u.wgsl = !strcmp(line + 5, "wgsl");
      else if (!strncmp(line, "out ", 4) && !u.out) u.out = real_out(line + 4);
      else if (!strncmp(line, "cmd ", 4) && !u.cmd) u.cmd = copy(line + 4);
      else if (!strncmp(line, "src /", 5)) {
        u.src = grow(u.src, &u.src_cap, u.nsrc + 1, sizeof *u.src);
        u.src[u.nsrc++] = watch_file(line + 4, ROLE_SRC);
        near_dir(line + 4);
      }
    }
    fclose(f);
    if (!ok || !u.out || !u.cmd || !u.nsrc) { free(u.out); free(u.cmd); free(u.src); continue; }
    u.out_mtime = mtime_of(u.out);
    {
      char stamp[PATH_MAX + 1024];
      int stem = (int)strlen(path) - 5;   // without ".deps"
      u.started = snprintf(stamp, sizeof stamp, "%.*s.started", stem, path) < (int)sizeof stamp ? mtime_of(stamp) : (struct timespec){0, 0};
      if (!u.started.tv_sec || later(u.started, u.out_mtime)) u.started = u.out_mtime;   // no stamp, or a build that began and did not end
    }
    units = grow(units, &unit_cap, unit_count + 1, sizeof *units);
    units[unit_count++] = u;
  }
  free(line);
  closedir(d);
}
// What is watched, read again: after every build (its lists may name new files, and new
// files may be in the folders), so each row's time is the time after that build.
static void watch_all(void) {
  watch_reset();
  scan_dir(app_dir, true, true);
  scan_dir(host_dir, false, false);
  load_deps();
  scan_near();
}
// A file that was not there at the last look (#13441): found between builds, about once a
// second, and given time 0, so check_changes takes it as a change the same tick. Before this a
// new page, shader or source waited for the next build's rescan.
static void scan_new(void) {
  int before = file_count;
  scan_dir(app_dir, true, true);
  scan_dir(host_dir, false, false);
  scan_near();
  for (int i = before; i < file_count; i++) files[i].mtime = (struct timespec){0, 0};
}
static bool unit_failed(const unit *u) {
  for (int i = 0; i < failed_count; i++) if (!strcmp(failed_outs[i], u->out)) return true;
  return false;
}
// After a build of `dirty` units (every unit for a whole build): which of them failed last.
static void note_failed(bool success, bool whole) {
  for (int k = 0; k < unit_count; k++) {
    if (!whole && !units[k].dirty) continue;
    int at = -1;
    for (int i = 0; i < failed_count; i++) if (!strcmp(failed_outs[i], units[k].out)) at = i;
    if (success && at >= 0) { free(failed_outs[at]); failed_outs[at] = failed_outs[--failed_count]; }
    if (!success && at < 0) { failed_outs = grow(failed_outs, &failed_cap, failed_count + 1, sizeof *failed_outs); failed_outs[failed_count++] = copy(units[k].out); }
  }
}

// A watched path as the URL a tab asks for (/build/x.wasm, /coaxial/host.js): false when none.
static bool url_of(const char *path, char *out, size_t cap) {
  size_t a = strlen(app_dir), h = strlen(host_dir);
  if (!strncmp(path, app_dir, a) && path[a] == '/') return snprintf(out, cap, "%s", path + a) < (int)cap;
  if (!strncmp(path, host_dir, h) && path[h] == '/') return snprintf(out, cap, "/coaxial%s", path + h) < (int)cap;
  return false;
}
static void note_wgsl(const char *path) {
  for (int i = 0; i < note_count; i++) if (!strcmp(notes[i], path)) return;
  notes = grow(notes, &note_cap, note_count + 1, sizeof *notes);
  notes[note_count++] = copy(path);
}
// "wgsl" for each changed shader, naming it (a tab whose shaders are another file ignores it).
static void tell_wgsl(void) {
  for (int i = 0; i < note_count; i++) {
    char url[PATH_MAX + 1024], msg[PATH_MAX + 1100];
    if (url_of(notes[i], url, sizeof url)) snprintf(msg, sizeof msg, "{\"kind\":\"wgsl\",\"path\":\"%s\"}", url);
    else snprintf(msg, sizeof msg, "{\"kind\":\"wgsl\"}");
    logf_("shader changed: %s", strrchr(notes[i], '/') + 1);
    broadcast(msg);
    free(notes[i]);
  }
  note_count = 0;
}
// "wasm" naming one unit's module, once, when it has been written.
static void tell_wasm(unit *u) {
  char url[PATH_MAX + 1024], msg[PATH_MAX + 1100];
  u->told = true;
  if (!url_of(u->out, url, sizeof url)) return;   // nothing a tab loads
  double ms = monotonic_ms() - build_started;
  logf_("%s rebuilt in %.0f ms", strrchr(u->out, '/') + 1, ms);
  snprintf(msg, sizeof msg, "{\"kind\":\"wasm\",\"ms\":%.0f,\"path\":\"%s\"}", ms, url);
  broadcast(msg);
}
// While the changed units build: a module written is told at once, before its builder
// exits (wasm.sh writes the module, then its drive files).
static void poll_outputs(void) {
  for (int i = 0; i < unit_count; i++) {
    unit *u = &units[i];
    if (!u->dirty || u->told || u->wgsl) continue;
    if (!same_time(mtime_of(u->out), u->out_mtime)) tell_wasm(u);
  }
}

// ---- the files tabs loaded (packs, textures): "watch URL..." from a tab
// The file's SHA-256 as 64 hex digits into hex; false when it can't be read.
static bool file_sha256(const char *path, char *hex) {
  int fd = open(path, O_RDONLY | O_CLOEXEC);
  if (fd < 0) return false;
  struct stat st;
  if (fstat(fd, &st) || !S_ISREG(st.st_mode)) { close(fd); return false; }
  void *p = st.st_size ? mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0) : NULL;
  close(fd);
  if (st.st_size && p == MAP_FAILED) return false;
  coax_t_S_Digest digest = S_hash((coax_string){st.st_size ? p : "", (int64_t)st.st_size});
  char out[64];
  bool ok = !digest.error && S_hex(digest, (coax_v_u8){(uint8_t *)out, 64});
  if (st.st_size) munmap(p, (size_t)st.st_size);
  if (ok) { memcpy(hex, out, 64); hex[64] = 0; }
  return ok;
}
static void watch_url(const char *url) {
  for (int i = 0; i < asset_count; i++) if (!strcmp(assets[i].url, url)) return;
  char path[PATH_MAX + 1024];
  if (url[0] != '/' || strstr(url, "/coaxial/live") || !resolve(url, path, sizeof path)) return;
  assets = grow(assets, &asset_cap, asset_count + 1, sizeof *assets);
  assets[asset_count++] = (loaded){copy(url), copy(path), mtime_of(path), ""};
}
// "watch URL URL...": what the tab loaded, as a text frame.
static void tab_says(const uint8_t *text, size_t n) {
  if (n < 6 || memcmp(text, "watch ", 6)) return;
  char url[1024];
  for (size_t i = 6; i < n;) {
    while (i < n && text[i] == ' ') i++;
    size_t j = i;
    while (j < n && text[j] != ' ') j++;
    if (j > i && j - i < sizeof url) { memcpy(url, text + i, j - i); url[j - i] = 0; watch_url(url); }
    i = j;
  }
}
// A loaded file whose bytes changed: "asset" with its URL and new SHA-256 (a touch that
// leaves the bytes as they were tells nothing).
static void check_assets(void) {
  for (int i = 0; i < asset_count; i++) {
    loaded *a = &assets[i];
    struct timespec t = mtime_of(a->path);
    if (same_time(t, a->mtime)) continue;
    a->mtime = t;
    char hex[65];
    if (!file_sha256(a->path, hex) || !strcmp(hex, a->hash)) continue;
    memcpy(a->hash, hex, sizeof hex);
    char msg[1200];
    snprintf(msg, sizeof msg, "{\"kind\":\"asset\",\"url\":\"%s\",\"hash\":\"%s\"}", a->url, a->hash);
    logf_("asset changed: %s", a->url);
    broadcast(msg);
  }
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

static void build_finished(bool success) {
  static char escaped[REQ_MAX];
  double ms = monotonic_ms() - build_started;
  build_pid = 0;
  if (build_fd >= 0) { close(build_fd); build_fd = -1; }
  build_log[build_used] = 0;
  // A .wgsl the build wrote (WGSL generated from Coax) is a changed shader. Before #13410 the
  // times below were reset without telling the tabs.
  for (int i = 0; i < file_count; i++)
    if ((files[i].roles & ROLE_SCAN) && ends_with(files[i].path, ".wgsl") && !same_time(mtime_of(files[i].path), files[i].mtime)) note_wgsl(files[i].path);
  for (int i = 0; i < unit_count; i++) {
    unit *u = &units[i];
    if (!u->dirty || u->told || same_time(mtime_of(u->out), u->out_mtime)) continue;
    if (u->wgsl) note_wgsl(u->out);
    else tell_wasm(u);
  }
  note_failed(success, !build_targeted);
  // A page file saved while a whole build ran is shown: the tabs load the page again (#13441).
  for (int i = 0; !build_targeted && i < file_count; i++)
    if ((files[i].roles & ROLE_SCAN) && page_kind(files[i].path) && !same_time(mtime_of(files[i].path), files[i].mtime)) build_page = true;
  // An edit saved while the changed units built, or just after a tab was told, is the next
  // build's: a targeted build keeps every source's time from before the rescan, so only its
  // module outputs count as its own. Only a whole build writes sources (the realms' tables).
  typedef struct { char *path; struct timespec mtime; } seen;
  seen *kept = NULL; int kept_count = 0, kept_cap = 0;
  for (int i = 0; build_targeted && i < file_count; i++) {
    if (ends_with(files[i].path, ".wgsl")) continue;
    bool out = false;
    for (int k = 0; k < unit_count && !out; k++) out = !strcmp(units[k].out, files[i].path);
    if (out) continue;
    kept = grow(kept, &kept_cap, kept_count + 1, sizeof *kept);
    kept[kept_count++] = (seen){copy(files[i].path), files[i].mtime};
  }
  if (success && !build_targeted) {
    char msg[128];
    snprintf(msg, sizeof msg, "{\"kind\":\"wasm\",\"ms\":%.0f}", ms);
    logf_("engine rebuilt in %.0f ms", ms);
    broadcast(msg);
  } else if (!success) {
    logf_("build failed in %.0f ms:\n%s", ms, build_log);
    json_escape(build_log, escaped, sizeof escaped - 32);
    char msg[REQ_MAX + 64];
    snprintf(msg, sizeof msg, "{\"kind\":\"error\",\"text\":\"%s\"}", escaped);
    broadcast(msg);
  }
  tell_wgsl();
  if (build_page) broadcast("{\"kind\":\"page\"}");
  build_page = false;
  // What is watched again, at its times now: generated files the build wrote don't start
  // another build. An edit made in response to the completion message belongs to the next one.
  bool whole = !build_targeted;
  watch_all();
  for (int i = 0; i < kept_count; i++) {
    int at = find_file(kept[i].path);
    if (at >= 0) files[at].mtime = kept[i].mtime;
    free(kept[i].path);
  }
  free(kept);
  // A whole build wrote sources of its own (the realms' tables) before its modules began, so
  // their times are the build's. A listed source saved after the build began and after a
  // module that reads it began is an edit that module did not read (#13441): it gets time
  // 0, and the next look builds the modules that read it. Before this the edit was counted
  // as the build's own and lost until the next save.
  for (int k = 0; whole && k < unit_count; k++)
    for (int j = 0; j < units[k].nsrc; j++) {
      watched *f = &files[units[k].src[j]];
      if (later(f->mtime, build_wall) && later(f->mtime, units[k].started)) { logf_("saved during the build: %s", strrchr(f->path, '/') + 1); f->mtime = (struct timespec){0, 0}; }
    }
}

static void build_start(const char *cmd, bool targeted) {
  int p[2];
  build_used = 0; build_started = monotonic_ms(); build_targeted = targeted;
  clock_gettime(CLOCK_REALTIME, &build_wall);
  if (pipe(p)) goto failed;
  p[0] = private_fd(p[0]); p[1] = private_fd(p[1]);
  if (p[0] < 0 || p[1] < 0) goto close_failed;
  int flags = fcntl(p[0], F_GETFL);
  if (flags < 0 || fcntl(p[0], F_SETFL, flags | O_NONBLOCK)) goto close_failed;
  pid_t pid = fork();
  if (pid < 0) goto close_failed;
  if (pid == 0) {
    close(p[0]);
    if (dup2(p[1], STDOUT_FILENO) < 0 || dup2(p[1], STDERR_FILENO) < 0) _exit(127);
    if (p[1] > STDERR_FILENO) close(p[1]);
    signal(SIGPIPE, SIG_DFL);
    execl("/bin/sh", "sh", "-c", cmd, (char *)NULL);
    _exit(127);
  }
  close(p[1]); build_fd = p[0]; build_pid = pid;
  return;
close_failed: {
    int error = errno;
    if (p[0] >= 0) close(p[0]);
    if (p[1] >= 0) close(p[1]);
    errno = error;
  }
failed:
  build_used = (size_t)snprintf(build_log, sizeof build_log, "cannot start build: %s", strerror(errno));
  build_finished(false);
}

static void build_drain(void) {
  // Keep draining after the retained log fills, without letting a noisy
  // builder monopolise the HTTP loop. No fread/pclose waits on the child.
  char bytes[4096];
  for (int i = 0; build_fd >= 0 && i < 16; i++) {
    ssize_t n = read(build_fd, bytes, sizeof bytes);
    if (n > 0) {
      size_t keep = (size_t)n;
      if (keep > sizeof build_log - 1 - build_used) keep = sizeof build_log - 1 - build_used;
      memcpy(build_log + build_used, bytes, keep); build_used += keep;
    } else if (n < 0 && errno == EINTR) { i--; }
    else if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
    else { close(build_fd); build_fd = -1; }
  }
}

// The running targeted build has written and told every module it builds (no shader).
static bool units_all_told(void) {
  bool any = false;
  for (int i = 0; i < unit_count; i++) {
    if (!units[i].dirty) continue;
    if (units[i].wgsl || !units[i].told) return false;
    any = true;
  }
  return any;
}
// The build is done as far as the tabs and the watcher go; its tail runs on (see tail).
static void release_build(void) {
  build_drain();
  tails = grow(tails, &tail_cap, tail_count + 1, sizeof *tails);
  tails[tail_count++] = (tail){build_pid, build_fd};
  build_fd = -1;
  build_finished(true);
}

static void reap_children(void) {
  int status; pid_t pid;
  while ((pid = waitpid(-1, &status, WNOHANG)) > 0) {
    for (int i = 0; i < tail_count; i++) {
      if (tails[i].pid != pid) continue;
      if (tails[i].fd >= 0) close(tails[i].fd);
      tails[i] = tails[--tail_count];
      break;
    }
    if (pid == build_pid) {
      build_drain();
      build_finished(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    // Shaped responses and WebSocket tunnels have no completion message.
  }
}

// The changed units' own commands, side by side, as one shell script; its status is 1 when
// any failed.
static char *units_script(void) {
  size_t n = 64;
  for (int i = 0; i < unit_count; i++) if (units[i].dirty) n += strlen(units[i].cmd) + 64;
  char *s = malloc(n);
  if (!s) { logf_("out of memory starting a build"); exit(1); }
  size_t at = (size_t)snprintf(s, n, "s=0\n");
  for (int i = 0; i < unit_count; i++)
    if (units[i].dirty) at += (size_t)snprintf(s + at, n - at, "(%s) & p%d=$!\n", units[i].cmd, i);
  for (int i = 0; i < unit_count; i++)
    if (units[i].dirty) at += (size_t)snprintf(s + at, n - at, "wait $p%d || s=1\n", i);
  snprintf(s + at, n - at, "exit $s\n");
  return s;
}
static void check_changes(void) {
  bool whole = false, page = false, some = false;
  static double last_new;
  check_assets();
  if (now_ms() - last_new >= 1000) { last_new = now_ms(); scan_new(); }
  for (int i = 0; i < file_count; i++) {
    struct timespec t = mtime_of(files[i].path);
    if (same_time(t, files[i].mtime)) continue;
    files[i].mtime = t;
    const char *p = files[i].path;
    if (files[i].roles == ROLE_NEAR && !failed_count) continue;   // a source nothing reads yet
    logf_("changed %s", strrchr(p, '/') + 1);
    bool read = false;   // a unit's list names it
    if (files[i].roles & ROLE_SRC)
      for (int k = 0; k < unit_count; k++)
        for (int j = 0; j < units[k].nsrc; j++)
          if (units[k].src[j] == i) { units[k].dirty = true; read = some = true; break; }
    if (read) continue;
    if (engine_kind(p) && failed_count) {
      // a source no list names, with a build that failed: what it was missing may be this
      // file (a new one a changed module loads), so those units build again, not everything
      for (int k = 0; k < unit_count; k++) if (unit_failed(&units[k])) { units[k].dirty = true; some = true; }
      if (some) continue;
    }
    if (!(files[i].roles & ROLE_SCAN)) continue;   // NEAR alone: a source nothing reads yet
    if (engine_kind(p)) whole = true;
    else if (ends_with(p, ".wgsl")) note_wgsl(p);
    else page = true;
  }
  if (whole || some) {
    // A watched source no list names: the whole build, as before lists (a unit it builds is
    // told in the whole build's one "wasm").
    build_page = page;
    for (int k = 0; whole && k < unit_count; k++) units[k].dirty = false;
    for (int k = 0; k < unit_count; k++) units[k].told = false;
    if (whole) { build_start(build_cmd, false); return; }
    char *script = units_script(), names[1024] = "";
    for (int k = 0; k < unit_count; k++)
      if (units[k].dirty && strlen(names) + strlen(units[k].out) + 3 < sizeof names) { if (names[0]) strcat(names, ", "); strcat(names, strrchr(units[k].out, '/') + 1); }
    logf_("building %s", names);
    build_start(script, true);
    free(script);
    return;
  }
  tell_wgsl();
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
    else if (!strcmp(argv[i], "--realm-host") && i + 1 < argc) realm_host = argv[++i];
    else if ((!strcmp(argv[i], "--throttle-mbps") || !strcmp(argv[i], "--latency-ms")) && i+1<argc) {
      bool rate = !strcmp(argv[i], "--throttle-mbps"); char *end;
      errno = 0;
      double v = strtod(argv[++i], &end);
      if (!argv[i][0] || *end || errno == ERANGE || !isfinite(v) || v<0 || v>1000000 || (rate && v>0 && v<0.001)) {fprintf(stderr,"serve: invalid network shaping value\n");return 2;}
      if (rate) throttle_bytes = v * 1000000.0 / 8.0; else latency_ms = v;
    }
    else if (!strcmp(argv[i], "--cache") && i + 1 < argc) {
      int seconds = atoi(argv[++i]);
      if (seconds > 0) snprintf(cache_control, sizeof cache_control, "public, max-age=%d", seconds);
      else snprintf(cache_control, sizeof cache_control, "no-cache");
    }
    else if (!strcmp(argv[i], "--build") && i + 1 < argc) cmd = argv[++i];
    else if (!strcmp(argv[i], "--api") && i + 1 < argc) api_port = atoi(argv[++i]);
    else if (!strcmp(argv[i], "--local") && i + 1 < argc) local_port = atoi(argv[++i]);
    else if (!app && argv[i][0]!='-') app = argv[i];
    else { fprintf(stderr, "usage: serve APP_DIR [--listen HOST:PORT] [--build CMD]\n"); return 2; }
  }
  if (!app || !realpath(app, app_dir)) { fprintf(stderr, "usage: serve APP_DIR [--listen HOST:PORT] [--build CMD]\n"); return 2; }
  if (hosts) {
    if (!realpath(hosts, host_dir)) { fprintf(stderr, "serve: no host directory %s\n", hosts); return 2; }
  } else {
    // The binary lives in coax/web/build/; the host files in coax/web/.
    char self[PATH_MAX];
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
  signal(SIGCHLD, SIG_DFL);   // reap without blocking, including build exit status
  int lfd = private_fd(socket(AF_INET, SOCK_STREAM, 0));
  if (lfd < 0) { perror("serve: socket"); return 1; }
  listen_fd = lfd;
  int one = 1; setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons((uint16_t)port)};
  if (inet_pton(AF_INET, host, &a.sin_addr) != 1 || bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 64)) {
    fprintf(stderr, "serve: cannot listen on %s: %s\n", listen_spec, strerror(errno));
    return 1;
  }
  for (int i = 0; i < MAX_CLIENTS; i++) clients[i].fd = -1;
  if (!static_mode) {
    watch_all();
    build_start(build_cmd, false);
  }
  printf("coax serve: http://%s:%d/  (app %s, %s)\n", host, port, app_dir, static_mode ? "static" : "watching");
  fflush(stdout);

  double last_scan = now_ms();
  for (;;) {
    struct pollfd p[MAX_CLIENTS + 2];
    int map[MAX_CLIENTS + 2], n = 0;
    p[n].fd = lfd; p[n].events = POLLIN; map[n++] = -1;
    if (build_fd >= 0) { p[n].fd = build_fd; p[n].events = POLLIN; map[n++] = -2; }
    for (int i = 0; i < MAX_CLIENTS; i++)
      if (clients[i].fd >= 0) { p[n].fd = clients[i].fd; p[n].events = POLLIN; map[n++] = i; }
    int r = poll(p, (nfds_t)n, 50);
    if (r < 0 && errno != EINTR) break;
    for (int k = 0; r > 0 && k < n; k++) {
      if (!(p[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (map[k] == -2) { build_drain(); }
      else if (map[k] == -1) {
        int fd = private_fd(accept(lfd, NULL, NULL));
        if (fd < 0) continue;
        int slot = -1;
        for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd < 0) { slot = i; break; }
        if (slot < 0) { close(fd); continue; }
        clients[slot].fd = fd; clients[slot].ws = false; clients[slot].n = 0;
      } else {
        on_readable(&clients[map[k]]);
      }
    }
    reap_children();
    if (!static_mode && now_ms() - last_scan >= 50) {
      last_scan = now_ms();
      if (!build_pid) check_changes();
      else if (build_targeted) { poll_outputs(); if (units_all_told()) release_build(); }
      for (int i = 0; i < tail_count; i++) {   // a tail's report is not read; its pipe never fills
        char scratch[4096];
        while (tails[i].fd >= 0 && read(tails[i].fd, scratch, sizeof scratch) > 0) {}
      }
    }
  }
  return 0;
}
