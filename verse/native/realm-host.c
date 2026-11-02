// A realm natively (realm-host.h).
#define _GNU_SOURCE
#include "realm-host.h"
#include "face.h"
#include <fcntl.h>
#include <stddef.h>
#include <malloc.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

extern char **environ;
static int golden_mode;

// The face's offsets are 32 bits. Built without PIE, its static data is
// below 2 GB; malloc stays on the brk heap (no mmap), which follows it; and
// the arena is mapped there too. rh_ptr refuses anything else.
void *rh_ptr(uint32_t offset) { return (void *)(uintptr_t)offset; }
uint32_t rh_off(const void *p) {
  uintptr_t a = (uintptr_t)p;
  if (a >> 32) { fprintf(stderr, "realm-host: %p is above 4 GB, where the face cannot point\n", p); exit(1); }
  return (uint32_t)a;
}

static uint32_t align16(uint32_t n) { return (n + 15u) & ~15u; }

void rh_open(RealmHost *h, int golden) {
  memset(h, 0, sizeof *h);
  golden_mode = golden;
  mallopt(M_MMAP_MAX, 0);
  h->floats = (int)coaxial_vertex_floats();
  h->max_vertices = (int)coaxial_max_vertices();
  h->uniform_floats = (int)coaxial_uniform_floats();
  uint32_t state = align16(coaxial_state_size() > 1024 ? coaxial_state_size() : 1024);
  h->arena_size = (size_t)state + (size_t)h->max_vertices * h->floats * 4 + (size_t)h->uniform_floats * 4 + 64;
  h->arena = mmap(NULL, h->arena_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT, -1, 0);
  if (h->arena == MAP_FAILED) { perror("realm-host: arena"); exit(1); }
  h->state = rh_off(h->arena);
  h->vertices = h->state + state;
  h->uniforms = h->vertices + (uint32_t)(h->max_vertices * h->floats * 4);
  coaxial_state_init(h->state);   // its struct defaults, as host.js has them applied
  if (golden) coaxial_golden(1);
}

static uint32_t le32(const uint8_t *p) { return p[0] | p[1] << 8 | p[2] << 16 | (uint32_t)p[3] << 24; }

int rh_pack(RealmHost *h, const char *path) {
  if (!strcmp(path, "-")) return 0;   // a realm with no pack (its page names none)
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "realm-host: cannot read %s\n", path); return 1; }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  h->pack = malloc(n > 0 ? (size_t)n : 1);
  if (!h->pack || fread(h->pack, 1, (size_t)n, f) != (size_t)n) { fclose(f); fprintf(stderr, "realm-host: cannot read %s\n", path); return 1; }
  fclose(f);
  h->pack_size = (size_t)n;
  // The header and table checked against the length, as host.js unpack does.
  const uint8_t *b = h->pack;
  if (h->pack_size < 32 || memcmp(b, "VPK1", 4) || le32(b + 8) != h->pack_size) { fprintf(stderr, "realm-host: %s is not a whole asset pack\n", path); return 1; }
  uint32_t count = le32(b + 4), table = le32(b + 16);
  if ((uint64_t)table + (uint64_t)count * 48 > h->pack_size) { fprintf(stderr, "realm-host: %s: its table runs past its end\n", path); return 1; }
  h->entries = calloc(count ? count : 1, sizeof *h->entries);
  for (uint32_t k = 0; k < count; k++) {
    const uint8_t *at = b + table + k * 48;
    uint32_t offset = le32(at + 4), length = le32(at + 8);
    if ((uint64_t)offset + length > h->pack_size) { fprintf(stderr, "realm-host: %s: an entry runs past its end\n", path); return 1; }
    RhEntry *e = &h->entries[k];
    e->kind = at[0];
    memcpy(e->name, at + 16, 32);
    e->bytes = b + offset;
    e->length = length;
    if (e->kind == RH_LAYER) h->layers++;
  }
  h->entry_count = (int)count;
  return 0;
}

const RhEntry *rh_entry(const RealmHost *h, int kind) {
  for (int i = 0; i < h->entry_count; i++) if (h->entries[i].kind == kind) return &h->entries[i];
  return NULL;
}

void rh_deliver(RealmHost *h, int extra, const uint32_t *extra_sizes) {
  if (!h->pack) {
    // no pack: the page's assets are the only ones, from 0 (host.js lists them so)
    for (int i = 0; i < extra; i++) coaxial_asset_ready(h->state, i, extra_sizes[i]);
    return;
  }
  char loaded[64];
  int n = snprintf(loaded, sizeof loaded, "%zu %zu", h->pack_size, h->pack_size);
  rh_send(h, RH_PACK_KIND, loaded, (size_t)n);
  const RhEntry *catalog = rh_entry(h, RH_CATALOG), *vertices = rh_entry(h, RH_VERTICES), *indices = rh_entry(h, RH_INDICES);
  uint32_t sizes[4] = {catalog ? catalog->length : 0, vertices ? vertices->length : 0, indices ? indices->length : 0, (uint32_t)h->layers};
  if (catalog) {
    uint32_t at = coaxial_asset(h->state, 0, catalog->length);
    if (at) memcpy(rh_ptr(at), catalog->bytes, catalog->length);
  }
  for (int i = 0; i < 4; i++) coaxial_asset_ready(h->state, i, sizes[i]);
  for (int i = 0; i < extra; i++) coaxial_asset_ready(h->state, 4 + i, extra_sizes[i]);
}

void rh_send(RealmHost *h, int kind, const char *text, size_t length) {
  uint32_t cap = coaxial_inbox_cap();
  if (length > cap) length = cap;
  memcpy(rh_ptr(coaxial_inbox(h->state)), text, length);
  coaxial_message(h->state, kind, (uint32_t)length);
  rh_after(h, 0);
}

uint32_t rh_frame(RealmHost *h, float w, float ht, float scale, float dt) {
  uint32_t n = coaxial_frame(h->state, h->vertices, h->uniforms, w, ht, scale, dt, 0);
  return n > (uint32_t)h->max_vertices ? (uint32_t)h->max_vertices : n;
}

static void keep(int action, const char *text, uint32_t len);
static char went[1100];
static size_t went_len;
static int went_set;
int rh_after(RealmHost *h, int say) {
  int action = coaxial_action(h->state);
  if (!action) return 0;
  uint32_t len = coaxial_outbox_len(h->state);
  const char *text = (const char *)rh_ptr(coaxial_outbox(h->state));
  if (say) printf("realm-host: action %d: %.*s\n", action, (int)(len > 200 ? 200 : len), text);
  if (action == 1 && len < sizeof went) { memcpy(went, text, len); went_len = len; went_set = 1; }
  keep(action, text, len);
  coaxial_action_done(h->state);
  return action;
}

void rh_page(RealmHost *h, int page) { coaxial_page(h->state, page); }

static int starts(const char *p, size_t n, const char *with) { size_t k = strlen(with); return n >= k && !memcmp(p, with, k); }
static int put_slug(const char *s, size_t n, char *slug, size_t cap) {
  if (!n || n >= cap) return 0;
  memcpy(slug, s, n); slug[n] = 0;
  return 1;
}
int rh_realm_of(const char *p, size_t n, char *slug, size_t cap) {
  if (starts(p, n, "https://openagents.com/")) { p += 22; n -= 22; }
  size_t end = 0;   // the path, without its query or hash
  while (end < n && p[end] != '?' && p[end] != '#') end++;
  if (end == 1 && p[0] == '/') return put_slug("the-grid", 8, slug, cap);
  if ((end == 9 || end == 10) && starts(p, end, "/new-land") && (end == 9 || p[9] == '/')) return put_slug("the-grid", 8, slug, cap);
  if ((end == 7 || end == 8) && starts(p, end, "/realms") && (end == 7 || p[7] == '/')) return put_slug("galaxy", 6, slug, cap);
  if (!starts(p, end, "/realms/")) return 0;
  size_t a = 8, b = a;
  while (b < end && p[b] != '/') {
    char c = p[b];
    if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-')) return 0;
    b++;
  }
  if (b < end && b + 1 != end) return 0;   // a page under the realm (its tip): the web's
  return put_slug(p + a, b - a, slug, cap);
}
int rh_went(char *slug, size_t cap) {
  if (!went_set) return 0;
  went_set = 0;
  return rh_realm_of(went, went_len, slug, cap);
}

void rh_close(RealmHost *h) {
  munmap(h->arena, h->arena_size);
  free(h->entries);
  free(h->pack);
}

// ---- the network and what the realm keeps (rh_net_open)
// Each request is one verse/native/net process (net.coax: the approved TLS, its one owner
// thread), so the frame never waits on it; its answer is a file, read when the process has
// ended. The slots, the same-site rule and the answers are coax/web/http-requests.js's.
enum { RH_SLOTS = 8, RH_BEACONS = 16, RH_PLAYER = 5, RH_PLACE = 11, RH_BAR = 12, RH_ANSWER = 512,
       RH_UNBOUNDED_MS = 35000 };
typedef struct { pid_t pid; int64_t deadline; char out[1100], body[1100]; } RhRequest;
static struct { int on, port, log; char helper[1024], host[256], dir[1024], name[64]; } net;
static RhRequest slots[RH_SLOTS], beacons[RH_BEACONS];
static unsigned serial;

static int64_t mono_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static char *read_all(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  size_t cap = 4096, len = 0;
  char *b = malloc(cap);
  size_t k;
  while (b && (k = fread(b + len, 1, cap - len, f)) > 0) {
    len += k;
    if (len == cap) { char *m = realloc(b, cap *= 2); if (!m) { free(b); b = NULL; } else b = m; }
  }
  fclose(f);
  if (!b) return NULL;
  *n = len;
  return b;
}
// Written whole or not at all (a new file, then renamed over the old).
static int write_all(const char *path, const char *bytes, size_t n) {
  char tmp[1200];
  snprintf(tmp, sizeof tmp, "%s.new", path);
  int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC, 0600);
  if (fd < 0) return -1;
  size_t done = 0;
  while (done < n) { ssize_t w = write(fd, bytes + done, n - done); if (w <= 0) { close(fd); unlink(tmp); return -1; } done += (size_t)w; }
  if (close(fd) || rename(tmp, path)) { unlink(tmp); return -1; }
  return 0;
}
static void kept_path(char *out, size_t cap, const char *what) {
  if (!strcmp(what, "player")) snprintf(out, cap, "%s/player", net.dir);   // the browser's verse.player: every realm's
  else snprintf(out, cap, "%s/%s.%s", net.dir, net.name, what);
}
// A path on this site: "/" first, not "//", no backslash, space, control or DEL (the browser
// would resolve those against another origin or escape them; this host refuses them).
static int same_site(const char *u, uint32_t n) {
  if (n < 1 || n > 4000 || u[0] != '/' || (n > 1 && u[1] == '/')) return 0;
  for (uint32_t i = 0; i < n; i++) if ((unsigned char)u[i] <= ' ' || u[i] == 127 || u[i] == '\\') return 0;
  return 1;
}
// The kept player's key: the "key" string of the JSON action 5 kept, or "" (no escapes in a key).
static void player_key(char *out, size_t cap) {
  out[0] = 0;
  char path[1100];
  kept_path(path, sizeof path, "player");
  size_t n;
  char *j = read_all(path, &n);
  if (!j) return;
  char *t = memmem(j, n, "\"key\"", 5);
  if (t) {
    char *e = j + n, *p = t + 5;
    while (p < e && (*p == ' ' || *p == ':')) p++;
    if (p < e && *p == '"') {
      p++;
      size_t k = 0;
      while (p < e && *p != '"' && *p != '\\' && (unsigned char)*p >= ' ' && k + 1 < cap) out[k++] = *p++;
      out[(p < e && *p == '"') ? k : 0] = 0;
    }
  }
  free(j);
}
static void keep(int action, const char *text, uint32_t len) {
  if (!net.on) return;
  char path[1100];
  // host.js keeps the player when it is JSON with a string key; the place and bar unless golden
  if (action == RH_PLAYER) {
    if (len < 2 || text[0] != '{' || !memmem(text, len, "\"key\"", 5)) return;
    kept_path(path, sizeof path, "player");
  } else if ((action == RH_PLACE || action == RH_BAR) && !golden_mode) kept_path(path, sizeof path, action == RH_PLACE ? "place" : "bar");
  else return;
  if (write_all(path, text, len)) fprintf(stderr, "realm-host: cannot keep %s\n", path);
}
static void forget(RhRequest *r) {
  if (r->out[0]) unlink(r->out);
  if (r->body[0]) unlink(r->body);
  memset(r, 0, sizeof *r);
}
static int64_t request(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags, int64_t timeout) {
  if (!net.on) return -1;
  const char *u = rh_ptr(url);
  if (!same_site(u, url_len)) return -1;
  int wanted = (flags & 4) != 0, beacon = (flags & 2) != 0;
  if (timeout >= 0 && (!wanted || beacon || timeout < 1 || timeout > 2147483647)) return -1;
  RhRequest *r = NULL;
  int id = 0;
  if (wanted) {
    for (id = 0; id < RH_SLOTS && slots[id].pid; id++) {}
    if (id == RH_SLOTS) return -1;
    r = &slots[id];
  } else {
    rh_net_poll(NULL);   // reap the beacons that ended
    for (int i = 0; i < RH_BEACONS && !r; i++) if (!beacons[i].pid) r = &beacons[i];
    if (!r) return -1;
  }
  unsigned k = serial++;
  char path[4001], port[16], fl[16], cap[16], to[24], key[256];
  memcpy(path, u, url_len);
  path[url_len] = 0;
  snprintf(r->out, sizeof r->out, "%s/.net-%d-%u.answer", net.dir, (int)getpid(), k);
  unlink(r->out);
  if (body_len) {
    snprintf(r->body, sizeof r->body, "%s/.net-%d-%u.body", net.dir, (int)getpid(), k);
    if (write_all(r->body, rh_ptr(body), body_len)) { forget(r); return -1; }
  }
  snprintf(port, sizeof port, "%d", net.port);
  snprintf(fl, sizeof fl, "%d", (int)flags);
  snprintf(cap, sizeof cap, "%u", coaxial_inbox_cap());
  snprintf(to, sizeof to, "%lld", (long long)(timeout >= 0 ? timeout : 0));
  if (flags & 8) player_key(key, sizeof key); else key[0] = 0;
  char *argv[] = {net.helper, r->out, net.host, port, fl, path, body_len ? r->body : "-", key[0] ? key : "-", cap, to, NULL};
  posix_spawn_file_actions_t fa;
  posix_spawn_file_actions_init(&fa);
  posix_spawn_file_actions_addopen(&fa, 0, "/dev/null", O_RDONLY, 0);
  posix_spawn_file_actions_addopen(&fa, 1, "/dev/null", O_WRONLY, 0);
  pid_t pid;
  int e = posix_spawn(&pid, net.helper, &fa, NULL, argv, environ);
  posix_spawn_file_actions_destroy(&fa);
  if (e) { forget(r); return -1; }
  r->pid = pid;
  r->deadline = mono_ms() + (timeout >= 0 ? timeout : RH_UNBOUNDED_MS);
  if (net.log) fprintf(stderr, "realm-host: request %d: %s %s (flags %d)\n", wanted ? id : -1, flags & 1 ? "POST" : "GET", path, (int)flags);
  return wanted ? id : 0;
}
int rh_net_open(RealmHost *h, const char *helper, const char *host, int port, const char *dir, const char *name) {
  (void)h;
  if (access(helper, X_OK)) { fprintf(stderr, "realm-host: no network helper %s\n", helper); return -1; }
  mkdir(dir, 0700);
  struct stat st;
  if (stat(dir, &st) || !S_ISDIR(st.st_mode)) { fprintf(stderr, "realm-host: no directory %s\n", dir); return -1; }
  snprintf(net.helper, sizeof net.helper, "%s", helper);
  snprintf(net.host, sizeof net.host, "%s", host);
  snprintf(net.dir, sizeof net.dir, "%s", dir);
  snprintf(net.name, sizeof net.name, "%s", name);
  net.port = port;
  net.on = 1;
  net.log = getenv("VERSE_NATIVE_NET_LOG") != NULL;   // each request and its answer's status, on stderr
  signal(SIGPIPE, SIG_IGN);
  return 0;
}
void rh_net_poll(RealmHost *h) {
  if (!net.on) return;
  for (int i = 0; i < RH_BEACONS; i++) {
    int st;
    if (beacons[i].pid && waitpid(beacons[i].pid, &st, WNOHANG) != 0) forget(&beacons[i]);
  }
  if (!h) return;
  int64_t now = mono_ms();
  for (int i = 0; i < RH_SLOTS; i++) {
    RhRequest *r = &slots[i];
    if (!r->pid) continue;
    int st = 0;
    pid_t w = waitpid(r->pid, &st, WNOHANG);
    if (w == 0 && now < r->deadline) continue;
    if (w == 0) { kill(r->pid, SIGKILL); waitpid(r->pid, &st, 0); }
    size_t n = 0;
    char *text = w > 0 && WIFEXITED(st) && WEXITSTATUS(st) == 0 ? read_all(r->out, &n) : NULL;
    // free before the answer: the realm may ask again as it reads it
    forget(r);
    if (net.log) {
      const char *nl = text && n ? memchr(text, '\n', n) : NULL;
      int status_n = text && n ? (int)(nl ? nl - text : (ptrdiff_t)n) : 1;
      fprintf(stderr, "realm-host: answer %d: %.*s, %zu bytes%s\n", i, status_n, text && n ? text : "0", n, w == 0 ? " (past its deadline)" : "");
    }
    if (text && n) rh_send(h, RH_ANSWER + i, text, n);
    else rh_send(h, RH_ANSWER + i, "0\n", 2);
    free(text);
  }
}
static int32_t cancel(int32_t id) {
  if (id < 0 || id >= RH_SLOTS || !slots[id].pid) return 0;
  kill(slots[id].pid, SIGKILL);
  waitpid(slots[id].pid, NULL, 0);
  forget(&slots[id]);
  return 1;
}
void rh_restore(RealmHost *h) {
  if (!net.on || golden_mode) return;
  static const int kinds[2] = {RH_PLACE, RH_BAR};
  static const char *whats[2] = {"place", "bar"};
  for (int i = 0; i < 2; i++) {
    char path[1100];
    kept_path(path, sizeof path, whats[i]);
    size_t n;
    char *t = read_all(path, &n);
    if (t && n) rh_send(h, kinds[i], t, n);
    free(t);
  }
}
void rh_net_close(void) {
  for (int i = 0; i < RH_SLOTS; i++) if (slots[i].pid) cancel(i);
  for (int i = 0; i < RH_BEACONS; i++) if (beacons[i].pid) { waitpid(beacons[i].pid, NULL, 0); forget(&beacons[i]); }
}

// The host's imports (coax/web/host.js imports.env), natively. Requests go out once
// rh_net_open has been called; before it they are refused, as a page offline has them.
static int64_t clock_ms(void) {
  const char *fake = getenv("VERSE_NATIVE_CLOCK_MS");
  if (fake) return atoll(fake);
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
int64_t guard_now_ms(void) { return clock_ms(); }
// the seconds clock (env.coaxial_now_seconds), on the same clock
int64_t coaxial_now_seconds(void) { return clock_ms() / 1000; }
// byte storage (env.coaxial_storage_*: the browser's IndexedDB): none natively, refused as host.js
// refuses it where the browser has none (UNAVAILABLE), so the app keeps going without it
int32_t coaxial_storage_put(int32_t a, int32_t b, uint32_t c, int32_t d, int32_t e, uint32_t f, int32_t g, uint32_t h, int32_t i) {
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)f; (void)g; (void)h; (void)i; return 2;
}
int32_t coaxial_storage_get(int32_t a, int32_t b, uint32_t c, int32_t d, int32_t e, uint32_t f, int32_t g) {
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)f; (void)g; return 2;
}
// coaxial_http_bounded and coaxial_http_cancel, as the face names them
int32_t guard_http(uint32_t a, uint32_t b, uint32_t c, uint32_t d, uint32_t e, int64_t f) {
  return (int32_t)request(a, b, c, d, (int32_t)e, f < 0 ? 0 : f);
}
int32_t guard_cancel(int32_t id) { return cancel(id); }
int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags) {
  return (int32_t)request(url, url_len, body, body_len, flags, -1);
}
// WebSockets (host.js coaxial_socket_*): refused natively, as host.js refuses a URL it cannot
// open, so a realm that plays with others (Hold the Stair's co-op) plays alone.
int32_t coaxial_socket_open(uint32_t url, uint32_t len) { (void)url; (void)len; return -1; }
void coaxial_socket_send(int32_t id, uint32_t ptr, uint32_t len) { (void)id; (void)ptr; (void)len; }
void coaxial_socket_close(int32_t id) { (void)id; }
// The exports host.js calls only when a realm's face has them (x.coaxial_draws?.(...) and
// the like): a face without them gets these, which say it has none.
#define NONE __attribute__((weak))
NONE void coaxial_state_init(uint32_t s) { (void)s; }
// a realm that never asks the host for anything (waterlab)
NONE int32_t coaxial_action(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_outbox(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_outbox_len(uint32_t s) { (void)s; return 0; }
NONE void coaxial_action_done(uint32_t s) { (void)s; }
NONE uint32_t coaxial_asset(uint32_t s, int32_t i, uint32_t b) { (void)s; (void)i; (void)b; return 0; }
NONE uint32_t coaxial_asset_capacity(uint32_t s, int32_t i) { (void)s; (void)i; return 0; }
NONE void coaxial_asset_ready(uint32_t s, int32_t i, uint32_t b) { (void)s; (void)i; (void)b; }
NONE void coaxial_hash(uint32_t s, uint32_t n) { (void)s; (void)n; }
NONE uint32_t coaxial_draws(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_draw_capacity(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_draw_count(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_water_draws(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_grass_instances(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_surface_vertices(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_caster_vertices(uint32_t s) { (void)s; return 0; }   // a traced scene's casters (the fixture's)
NONE uint32_t coaxial_caster_instances(uint32_t s) { (void)s; return 0; }
NONE void coaxial_wheel(uint32_t s, float dy) { (void)s; (void)dy; }   // a realm that does not zoom (Orbit League)
NONE void coaxial_page(uint32_t s, int32_t p) { (void)s; (void)p; }   // an app of one page
NONE void coaxial_golden(uint32_t on) { (void)on; }   // an app with no golden mode (the homepage)
NONE uint32_t coaxial_stores(void) { return 0; }
NONE uint32_t coaxial_store_cap(int32_t k) { (void)k; return 0; }
NONE uint32_t coaxial_store(uint32_t s, int32_t k) { (void)s; (void)k; return 0; }
NONE uint32_t coaxial_store_bytes(uint32_t s, int32_t k) { (void)s; (void)k; return 0; }
// an app without store versions has its stores copied every frame, as host.js copies them (its
// version undefined): a number that is new at every call
NONE uint32_t coaxial_store_version(uint32_t s, int32_t k) { static uint32_t calls; (void)s; (void)k; return ++calls; }
NONE uint32_t coaxial_particle_cap(void) { return 0; }
NONE uint32_t coaxial_particles(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_particle_bytes(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_particle_instances(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_media(uint32_t s) { (void)s; return 0; }
NONE uint32_t coaxial_media_len(uint32_t s) { (void)s; return 0; }
// The phone's thumbs (verse/grid.coax) draw through these; a desktop has none.
void host_line(int64_t h, double ax, double ay, double bx, double by, double la, double lb, double w) {
  (void)h; (void)ax; (void)ay; (void)bx; (void)by; (void)la; (void)lb; (void)w;
}
void host_dot(int64_t h, double x, double y, double size, double light, double soft, double crisp) {
  (void)h; (void)x; (void)y; (void)size; (void)light; (void)soft; (void)crisp;
}
void host_slot(int64_t h, double x, double y, double w, double ht, double press, double flash, double selected, double icon) {
  (void)h; (void)x; (void)y; (void)w; (void)ht; (void)press; (void)flash; (void)selected; (void)icon;
}
