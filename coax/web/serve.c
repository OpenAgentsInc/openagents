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
//   an edit saved to a world    "world" with its ID: the tabs showing it ask for its new events
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
// A world's routes (/api/worlds/..., the browser editor's E4, #13413) are answered here on the dev
// server for Splice Bots, so a page, a second tab, the CLI and `coax drive` share one world before anything is
// deployed (any other world, a realm's, goes to the API with --api, #13496): the request goes to the world's own tool (metaverse/splice-bots/tools/world-local.coax: the
// API's rules and Splice's own check, with files under APP_DIR/build/.worlds for rows), and a saved
// write is told to every tab. --static serves none of it.
//
// A world's imported assets (/api/worlds/ID/assets..., the editor's E11, #13420) are answered here
// too: a dropped file is packed by the world's asset tool (metaverse/splice-bots/tools/asset-local.coax,
// the editor's importer under the realm's limits), the pack and the file are kept by content hash
// under APP_DIR/build/.worlds, and every tab is told the pack's new hash ("asset"), so each fetches
// it again and swaps it in. --static serves none of it (assets_local has the rules).
//
// A realm's code from the browser (the editor's E12, #13421) is answered here and nowhere else:
// POST /coaxial/code (realm.code.v1, metaverse-engine/editor/code-wire.coax) lists, reads and
// writes one of the page's own realm sources, and a save answers with the compiler's check of it.
// --static and the realm origin have no such route (code_local has the rules).
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
#include <netinet/tcp.h>
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
#include "http-gzip.h"
#include "sha256-kernel.h"
#include "site-routes.h"
#include "world-alias.h"
#include "code-wire.h"
#include "realm-origin.h"

#define MAX_CLIENTS 128
#define KEEP_ALIVE_IDLE_MS 15000.0
#define GZ_CACHE_SLOTS 64
#define GZ_MIN_BYTES 256
#define GZ_MAX_BYTES (4 * 1024 * 1024)
#define REQ_MAX 16384
#define HTTP_REQUEST_MAX 262144 // a saved source file (code_local: 192 KB and its first line); live/build messages keep REQ_MAX

typedef struct { int fd; bool ws; size_t n; double last_ms; char buf[HTTP_REQUEST_MAX]; } client;

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
static int room_port;  // --room: /api/room/ (the room's socket) goes to the local room on 127.0.0.1:room_port (serve.sh, #13589)
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
static void code_local(client *c);
static bool header_is(const char *request, const char *name, const char *want);

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
  if (ends_with(path, ".js") || ends_with(path, ".mjs")) return "text/javascript; charset=utf-8";
  if (ends_with(path, ".css")) return "text/css; charset=utf-8";
  if (ends_with(path, ".wasm")) return "application/wasm";
  if (ends_with(path, ".wgsl")) return "text/plain; charset=utf-8";
  if (ends_with(path, ".md")) return "text/markdown; charset=utf-8";
  if (ends_with(path, ".svg")) return "image/svg+xml";
  if (ends_with(path, ".json")) return "application/json";
  if (ends_with(path, ".png")) return "image/png";
  if (ends_with(path, ".jpg")) return "image/jpeg";
  if (ends_with(path, ".ttf")) return "font/ttf";
  if (ends_with(path, ".bin") || ends_with(path, ".frames") || ends_with(path, ".vpk3") || ends_with(path, ".pack")) return "application/octet-stream";
  if (ends_with(path, ".ogg")) return "audio/ogg";
  return "text/plain; charset=utf-8";
}

static bool compressible_path(const char *path) {
  return ends_with(path, ".js") || ends_with(path, ".mjs") || ends_with(path, ".wasm") || ends_with(path, ".wgsl") ||
         ends_with(path, ".html") || ends_with(path, ".css") || ends_with(path, ".svg") || ends_with(path, ".json") ||
         ends_with(path, ".md") || ends_with(path, ".txt") || ends_with(path, ".frames") || ends_with(path, ".ttf") ||
         ends_with(path, ".syms") || ends_with(path, ".xml");
}

static bool ci_contains_token(const char *start, const char *end, const char *tok) {
  size_t tl = strlen(tok);
  for (const char *p = start; p + tl <= end; p++) {
    if (!strncasecmp(p, tok, tl)) {
      bool left_ok = (p == start) || (p[-1] == ' ' || p[-1] == '\t' || p[-1] == ',' || p[-1] == ':');
      bool right_ok = (p + tl == end) || (p[tl] == ' ' || p[tl] == '\t' || p[tl] == ',' || p[tl] == ';' || p[tl] == '\r');
      if (left_ok && right_ok) {
        if (p + tl < end && p[tl] == ';' && !strncasecmp(p + tl, ";q=0", 4) &&
            (p + tl + 4 == end || p[tl + 4] == ',' || p[tl + 4] == ' ' || p[tl + 4] == '\r')) return false;
        return true;
      }
    }
  }
  return false;
}

static bool wants_keep_alive(const char *request) {
  const char *line_end = strstr(request, "\r\n");
  if (!line_end) return false;
  bool http11 = (line_end - request >= 8) && !memcmp(line_end - 8, "HTTP/1.1", 8);
  bool saw_close = false, saw_ka = false;
  const char *p = line_end;
  while (p && p[2] && p[2] != '\r') {
    p += 2; const char *end = strstr(p, "\r\n"); if (!end) break;
    if (!strncasecmp(p, "Connection:", 11)) {
      if (ci_contains_token(p + 11, end, "close")) saw_close = true;
      if (ci_contains_token(p + 11, end, "keep-alive")) saw_ka = true;
    }
    p = end;
  }
  return http11 ? !saw_close : saw_ka;
}

static bool accepts_gzip(const char *request) {
  const char *p = strstr(request, "\r\n");
  while (p && p[2] && p[2] != '\r') {
    p += 2; const char *end = strstr(p, "\r\n"); if (!end) return false;
    if (!strncasecmp(p, "Accept-Encoding:", 16) && ci_contains_token(p + 16, end, "gzip")) return true;
    p = end;
  }
  return false;
}

typedef struct {
  char path[PATH_MAX];
  off_t size;
  struct timespec mtime;
  uint8_t *gz;
  size_t gz_len;
  uint64_t used;
} gz_entry;

static gz_entry gz_cache[GZ_CACHE_SLOTS];
static uint64_t gz_clock;
static coax_t_DeflateWork gz_work;

static const gz_entry *gz_get(const char *path, const struct stat *st, FILE *f) {
  struct timespec mt = MTIME(*st);
  for (int i = 0; i < GZ_CACHE_SLOTS; i++) {
    if (gz_cache[i].gz && gz_cache[i].size == st->st_size &&
        gz_cache[i].mtime.tv_sec == mt.tv_sec && gz_cache[i].mtime.tv_nsec == mt.tv_nsec &&
        !strcmp(gz_cache[i].path, path)) {
      gz_cache[i].used = ++gz_clock;
      return &gz_cache[i];
    }
  }
  size_t raw_len = (size_t)st->st_size;
  uint8_t *raw = malloc(raw_len);
  size_t out_cap = raw_len + 64 + raw_len / 512;
  uint8_t *out = raw ? malloc(out_cap) : NULL;
  if (!raw || !out) { free(raw); free(out); return NULL; }
  if (fseeko(f, 0, SEEK_SET) || fread(raw, 1, raw_len, f) != raw_len) { free(raw); free(out); return NULL; }
  int64_t gz_len = http_gzip((coax_string){(const char *)raw, (int64_t)raw_len},
                             (coax_v_u8){out, (int64_t)out_cap},
                             (coax_v_t_DeflateWork){&gz_work, 1});
  free(raw);
  if (gz_len <= 0 || (size_t)gz_len >= raw_len) { free(out); return NULL; }
  uint8_t *shrunk = realloc(out, (size_t)gz_len);
  if (shrunk) out = shrunk;
  int slot = 0;
  for (int i = 0; i < GZ_CACHE_SLOTS; i++) {
    if (!gz_cache[i].gz) { slot = i; break; }
    if (gz_cache[i].used < gz_cache[slot].used) slot = i;
  }
  free(gz_cache[slot].gz);
  snprintf(gz_cache[slot].path, sizeof gz_cache[slot].path, "%s", path);
  gz_cache[slot].size = st->st_size;
  gz_cache[slot].mtime = mt;
  gz_cache[slot].gz = out;
  gz_cache[slot].gz_len = (size_t)gz_len;
  gz_cache[slot].used = ++gz_clock;
  return &gz_cache[slot];
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

static bool respond_conn(int fd, int status, const char *reason, const char *type, const void *body, size_t n, bool keep_alive) {
  char head[1024];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nCache-Control: %s\r\n"
                   "Cross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n%s"
                   "X-Content-Type-Options: nosniff\r\nConnection: %s\r\n\r\n",
                   status, reason, type, n, static_mode && status == 200 ? cache_control : "no-store", realm_headers(),
                   keep_alive ? "keep-alive" : "close");
  if (!send_all(fd, head, (size_t)h)) return false;
  if (n && !send_all(fd, body, n)) return false;
  return keep_alive;
}
static void respond(int fd, int status, const char *reason, const char *type, const void *body, size_t n) {
  respond_conn(fd, status, reason, type, body, n, false);
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
static bool not_found(int fd, bool keep_alive) {
  char path[PATH_MAX + 1024];
  static char page[1 << 16];
  snprintf(path, sizeof path, "%s/404.html", app_dir);
  FILE *f = fopen(path, "rb");
  size_t n = f ? fread(page, 1, sizeof page, f) : 0;
  if (f) fclose(f);
  if (n > 0 && n < sizeof page) return respond_conn(fd, 404, "Not Found", "text/html; charset=utf-8", page, n, keep_alive);
  return respond_conn(fd, 404, "Not Found", "text/plain", "not found\n", 10, keep_alive);
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
static bool moved(int fd, const char *url, bool keep_alive, bool *sent_ok) {
  size_t n = strcspn(url, "?#");
  coax_t_SiteRoute route = site_route((coax_string){url, (int64_t)n});
  if (route.status != 301) return false;
  const char *query = route.query && url[n] == '?' ? url + n : "";
  char head[2600];
  int h = snprintf(head, sizeof head,
                   "HTTP/1.1 301 Moved Permanently\r\nLocation: %.*s%.*s%.*s\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: %s\r\n\r\n",
                   (int)route.to.count, route.to.data, (int)(n - (size_t)route.skip), url + route.skip, (int)strcspn(query, "#"), query,
                   keep_alive ? "keep-alive" : "close");
  *sent_ok = (h > 0 && h < (int)sizeof head) && send_all(fd, head, (size_t)h);
  return true;
}

static bool serve_file(int fd, const char *url, const char *request, bool keep_alive) {
  char path[PATH_MAX + 1024];
  bool sent_ok = false;
  if (moved(fd, url, keep_alive, &sent_ok)) return keep_alive && sent_ok;
  if (!resolve(url, path, sizeof path)) return not_found(fd, keep_alive);
  FILE *f = fopen(path, "rb");
  struct stat st;
  if (!f || fstat(fileno(f), &st) || !S_ISREG(st.st_mode)) {
    if (f) fclose(f);
    return not_found(fd, keep_alive);
  }
  char value[HTTP_REQUEST_MAX]; range_header(request, value, sizeof value);
  coax_t_HttpRange r = http_range((coax_string){value, (int64_t)strlen(value)}, st.st_size);
  if (r.status == 200 && !value[0] && st.st_size >= GZ_MIN_BYTES && st.st_size <= GZ_MAX_BYTES &&
      compressible_path(path) && accepts_gzip(request)) {
    const gz_entry *ge = gz_get(path, &st, f);
    if (ge) {
      fclose(f);
      char head[1536];
      int h = snprintf(head, sizeof head,
        "HTTP/1.1 200 OK\r\nContent-Type: %s\r\nContent-Encoding: gzip\r\nVary: Accept-Encoding\r\n"
        "Content-Length: %zu\r\nx-goog-meta-length: %lld\r\nAccept-Ranges: bytes\r\n"
        "Cache-Control: %s\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n%s"
        "X-Content-Type-Options: nosniff\r\nConnection: %s\r\n\r\n",
        content_type(path), ge->gz_len, (long long)st.st_size,
        static_mode ? cache_control : "no-store", realm_headers(), keep_alive ? "keep-alive" : "close");
      return send_all(fd, head, (size_t)h) && send_all(fd, ge->gz, ge->gz_len) && keep_alive;
    }
  }
  char extra[160] = "";
  if (r.status == 206) snprintf(extra, sizeof extra, "Content-Range: bytes %lld-%lld/%lld\r\n", (long long)r.first, (long long)(r.first + r.length - 1), (long long)st.st_size);
  if (r.status == 416) snprintf(extra, sizeof extra, "Content-Range: bytes */%lld\r\n", (long long)st.st_size);
  // A large body (a realm's pack, 90 MB) goes out from a child of its own. Sent here, it held every
  // other request (the page's shaders, fonts and sounds) until the browser had read all of it (#13141).
  bool will_fork = !shaping_send && r.length > BIG_BODY;
  bool ka_out = keep_alive && !will_fork;
  char head[1536];
  int h = snprintf(head, sizeof head,
    "HTTP/1.1 %lld %s\r\nContent-Type: %s\r\nContent-Length: %lld\r\nAccept-Ranges: bytes\r\n%s"
    "Cache-Control: %s\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n%s"
    "X-Content-Type-Options: nosniff\r\nConnection: %s\r\n\r\n",
    (long long)r.status, r.status == 206 ? "Partial Content" : r.status == 416 ? "Range Not Satisfiable" : "OK",
    content_type(path), (long long)r.length, extra, static_mode && r.status != 416 ? cache_control : "no-store", realm_headers(),
    ka_out ? "keep-alive" : "close");
  if (fseeko(f, (off_t)r.first, SEEK_SET)) { fclose(f); respond(fd,500,"Read Failed","text/plain","",0); return false; }
  bool child = false;
  if (will_fork) {
    pid_t pid = fork();
    if (pid > 0) { fclose(f); return false; }   // the caller drops this process's copy of the socket
    if (pid == 0) {
      child = true; close(listen_fd);
      if (build_fd >= 0) close(build_fd);
      for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd >= 0 && clients[i].fd != fd) close(clients[i].fd);
    }
  }
  bool ok = send_all(fd, head, (size_t)h);
  char body[65536]; int64_t left = r.length;
  while (ok && left > 0) {
    size_t want = left < (int64_t)sizeof body ? (size_t)left : sizeof body;
    size_t n = fread(body, 1, want, f); if (!n) break;
    if (!send_all(fd, body, n)) { ok = false; break; }
    left -= (int64_t)n;
  }
  fclose(f);
  if (child) _exit(0);
  return ka_out && ok && left == 0;
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
  int port = api_port;
  // The room's socket and its seat tickets: the local room is the API's stand-in for both, as local.coax runs it
  // (room-dev.coax), so the ticket names the room this server has and is signed with the key it checks (#13589).
  { char m[8], u[1024]; if (room_port && sscanf(c->buf, "%7s %1023s", m, u) == 2 && (!strncmp(u, "/api/room/", 10) || !strncmp(u, "/api/playtest/room-ticket/", 26))) port = room_port; }
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  inet_pton(AF_INET, "127.0.0.1", &a.sin_addr);
  if (fd < 0 || connect(fd, (struct sockaddr *)&a, sizeof a)) {
    if (fd >= 0) close(fd);
    static const char down[] = "{\"error\":\"The local API isn't running. Stop this dev server and start it again with coax/web/serve.sh apps/openagents-web, which starts the API.\"}";
    respond(c->fd, 502, "Bad Gateway", "application/json", down, sizeof down - 1); drop(c); return;
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
// /api/worlds/OLD... is /api/worlds/NEW...: a renamed world's old ID names it (site_world,
// world-alias-host.coax, #13486), as the API reads it (coax/web/api/worlds.c).
#include "serve-routes.h"
static size_t request_length(const client *c) {
  const char *end = strstr(c->buf, "\r\n\r\n");
  if (!end) return 0;
  size_t head = (size_t)(end - c->buf) + 4, want = 0;
  for (const char *p = c->buf; p < end; p++)
    if ((p == c->buf || p[-1] == '\n') && !strncasecmp(p, "Content-Length:", 15)) want = (size_t)strtoul(p + 15, NULL, 10);
  return head + want;
}
static bool body_in(const client *c) {
  size_t need = request_length(c);
  return need > 0 && (c->n >= need || need >= sizeof c->buf - 1);
}
static void consume_request(client *c) {
  size_t need = request_length(c);
  if (need > 0 && need < c->n) {
    memmove(c->buf, c->buf + need, c->n - need);
    c->n -= need;
    c->buf[c->n] = 0;
  } else {
    c->n = 0;
    c->buf[0] = 0;
  }
  c->last_ms = now_ms();
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
      shaping_send = true; shaping_fd = c->fd;
      for (;;) {
        sleep_until(monotonic_ms() + latency_ms);
        next_send_ms = monotonic_ms();
        handle_request(c);
        if (c->fd < 0 || c->ws) break;
        while (c->fd >= 0 && (!strstr(c->buf, "\r\n\r\n") || !body_in(c))) {
          if (c->n >= sizeof c->buf - 1) { drop(c); break; }
          struct pollfd pfd = {.fd = c->fd, .events = POLLIN};
          int pr = poll(&pfd, 1, 5000);
          if (pr <= 0) { drop(c); break; }
          ssize_t k = recv(c->fd, c->buf + c->n, sizeof c->buf - 1 - c->n, 0);
          if (k <= 0) { drop(c); break; }
          c->n += (size_t)k; c->buf[c->n] = 0;
        }
        if (c->fd < 0) break;
      }
      _exit(0);
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
  if (!static_mode && !realm_request && sscanf(c->buf, "%7s %1023s", method, url) == 2 && !strncmp(url, "/api/worlds/", 12)) {
    world_url(url, sizeof url);   // a renamed world's old ID is its new one (world-alias-host.coax, #13486)
    // The local tools keep Splice Bots and the worlds they hold files for; any other world (a realm's, #13496)
    // is the API's, so its owner, its rules and its scripts are the API's own on this server too.
    if (api_port && !world_held_here(url)) { proxy_api(c); return; }
    if (assets_route(url)) { assets_local(c, method, url); return; }
    if (files_route(url)) { files_local(c, method, url); return; }
    if (scripts_route(url)) { scripts_local(c, method, url); return; }
    worlds_local(c, method, url); return;
  }
  if (!static_mode && !realm_request && !strcmp(method, "POST") && !strcmp(url, "/coaxial/code")) { code_local(c); return; }
  if (api_port && sscanf(c->buf, "%7s %1023s", method, url) == 2 && !strncmp(url, "/api/", 5)) { proxy_api(c); return; }
  // No API behind this server (--api not given, or --static): the chat and the editor's agent say what to run (#13463).
  if (!api_port && sscanf(c->buf, "%7s %1023s", method, url) == 2 && !strncmp(url, "/api/chat/", 10)) {
    static const char none[] = "{\"error\":\"This dev server has no API behind it, so we can't reach a model. Start it with coax/web/serve.sh apps/openagents-web, which starts the API.\"}";
    respond(c->fd, 503, "Service Unavailable", "application/json", none, sizeof none - 1); drop(c); return;
  }
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
  bool ka = wants_keep_alive(c->buf);
  if (serve_file(c->fd, url, c->buf, ka)) consume_request(c);
  else drop(c);
}

static void on_readable(client *c) {
  c->last_ms = now_ms();
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
  while (c->fd >= 0 && !c->ws && strstr(c->buf, "\r\n\r\n") && body_in(c)) handle_request(c);
  if (c->fd >= 0 && !c->ws && c->n >= sizeof c->buf - 1) drop(c);
}

#include "serve-watch.h"
#include "serve-code.h"
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
// One pass over the watched files asks each for its time. With a whole site's build watched that
// is thousands of files, and a pass run in one go held every request behind it for most of a
// second. So a pass is taken a few milliseconds at a time, between requests, and acts on what
// changed when it has seen every file (#13413: an edit must reach a tab inside a second).
#define SCAN_SLICE_MS 6.0
static void check_changes(bool all) {
  static bool whole, page, some;
  static int scan_at;
  static double last_new;
  if (scan_at == 0) {
    whole = page = some = false;
    check_assets();
    if (now_ms() - last_new >= 1000) { last_new = now_ms(); scan_new(); }
  }
  double began = now_ms();
  int i = scan_at;
  for (; i < file_count; i++) {
    if (!all && i > scan_at && now_ms() - began >= SCAN_SLICE_MS) { scan_at = i; return; }   // asked each file: one answer can take milliseconds on a busy disk
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
  scan_at = 0;
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
    else if (!strcmp(argv[i], "--room") && i + 1 < argc) room_port = atoi(argv[++i]);
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
        int nodelay = 1; setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &nodelay, sizeof nodelay);
        int slot = -1;
        for (int i = 0; i < MAX_CLIENTS; i++) if (clients[i].fd < 0) { slot = i; break; }
        if (slot < 0) { close(fd); continue; }
        clients[slot].fd = fd; clients[slot].ws = false; clients[slot].n = 0; clients[slot].last_ms = now_ms();
      } else {
        on_readable(&clients[map[k]]);
      }
    }
    double tnow = now_ms();
    for (int i = 0; i < MAX_CLIENTS; i++)
      if (clients[i].fd >= 0 && !clients[i].ws && tnow - clients[i].last_ms >= KEEP_ALIVE_IDLE_MS) drop(&clients[i]);
    reap_children();
    if (!static_mode && now_ms() - last_scan >= 50) {
      last_scan = now_ms();
      if (!build_pid) check_changes(false);
      else if (build_targeted) { poll_outputs(); if (units_all_told()) release_build(); }
      for (int i = 0; i < tail_count; i++) {   // a tail's report is not read; its pipe never fills
        char scratch[4096];
        while (tails[i].fd >= 0 && read(tails[i].fd, scratch, sizeof scratch) > 0) {}
      }
    }
  }
  return 0;
}
