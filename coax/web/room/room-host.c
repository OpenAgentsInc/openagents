/* The Orbit League room server's host (ol-room): TCP, the WebSocket
   handshake and frames (verse/voice-host/ws.c), and the 60 Hz beat. Every
   room decision is room.coax's: the host hands it each new socket's request
   (rm_join), each text frame (rm_heard) and each close (rm_left), calls
   rm_tick 60 times a second and sends each player what rm_state writes.
     room [--listen 127.0.0.1] [--port 8830] [--dev DIR [--no-rooms]]     PORT in the environment wins (Cloud Run: 0.0.0.0)
   --dev DIR is local development (coax/web/room/local.coax): the room is also the API's stand-in. Its
   world is DIR/world-everglade.json, and it answers GET /api/playtest/world/everglade with
   that same document, so a page served with `serve.sh --api` reads the world the room plays.
   The local player is the realm's admin (GET /api/auth/viewer says A|1; OA_ROOM_DEV_ADMIN=0 a
   visitor), so EDIT and Aethyr Forge save: POST /api/playtest/world/everglade writes that document
   (the next version; a stale base 409) and .../reset puts the designed city back, and the room lays
   it and tells every page (e edit) as production's API and control do (dev_publish).
   --no-rooms keeps the stand-in's API (players, tickets, the world) and answers the rooms list and every
   join 503, as when the rooms can't be reached: an offline page's API (the drift check).
   Without --dev, the world comes from the API (OA_ROOM_API, default production's), as the page's does.
   OA_ROOM_SHARD names which shard of the everglade instance directory this is (room-directory.coax).
   GET /health answers 200 "ok" with the number of players; GET /api/playtest/rooms/SLUG
   answers rm_rooms' JSON (an everglade realm's instances and which to join).
   Every socket has its own output buffer: a frame for many goes onto each, sent as far as the
   socket takes it, the rest when poll says it can take more. No send waits on one slow socket;
   one whose waiting bytes pass OUT_MAX, or whose oldest waiting byte is OUT_AGE_MS old, is closed.
   Each everglade tick the room hears how much each socket still has waiting (rm_conn_waiting), and
   a socket behind gets no new rows that tick: a row is superseded by the next, so it waits for the
   page to catch up instead of queueing state already out of date (#12783). */
#define _POSIX_C_SOURCE 200809L
#include "room.h"
#include "room-world.h"
#include "ws.h"
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>

#include <string.h>
#include <strings.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define CONNS 512
#define IN_CAP 16384
#define OUT_MAX (256 * 1024)   // a connection whose unsent bytes pass this is closed (one slow socket never waits on the others)
#define OUT_AGE_MS 3000        // nor one whose output hasn't emptied for this long
typedef struct {
  int fd; bool ws; size_t n; uint8_t in[IN_CAP]; int64_t heard;
  uint8_t *out; size_t on, ocap;   // bytes waiting to go out (sent as the socket takes them)
  int64_t since;                   // when out last went from empty to waiting (ms), 0 empty
  bool doomed;                     // to close once the frame being sent is out to everyone
} conn;
static conn conns[CONNS];
static coax_t_RmServer *server;
static int64_t slow_closed, out_peak;
static coax_v_t_RmServer view(void) { return (coax_v_t_RmServer){server, 1}; }
#include "room-stats.h"   /* the vitals for the admins' room view (#12791) */
/* --dev DIR: the local stand-in for the API (local.coax). The world document is read once from DIR and
   is the one both the room and the page use. */
static const char *dev_dir;
static bool no_rooms;   /* --no-rooms (with --dev): the rooms list and joins answer 503, the rooms down */
static void world_ask(void);
static char *dev_world; static size_t dev_world_n;
static long long dev_stamp;   /* --dev: the wall ms of the last local publish (its "edit" stamp), 0 none */
static char *dev_read(const char *name, size_t *n) {
  char path[1024]; snprintf(path, sizeof path, "%s/%s", dev_dir, name);
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  static char tmp[1 << 16];
  size_t k = fread(tmp, 1, sizeof tmp - 1, f); fclose(f);
  char *out = malloc(k + 1);
  if (!out) return NULL;
  memcpy(out, tmp, k); out[k] = 0; *n = k;
  return out;
}

/* --dev: a top-level JSON field's value in text (after "key": and spaces), NULL none. Depth-aware, so a
   field of that name inside the doc isn't taken. */
static const char *dev_field(const char *t, size_t n, const char *key) {
  size_t kn = strlen(key); int depth = 0; bool str = false;
  for (size_t i = 0; i < n; i++) {
    char c = t[i];
    if (str) { if (c == '\\') i++; else if (c == '"') str = false; continue; }
    if (c == '{' || c == '[') { depth++; continue; }
    if (c == '}' || c == ']') { depth--; continue; }
    if (c != '"') continue;
    if (depth == 1 && i + kn + 1 < n && !strncmp(t + i + 1, key, kn) && t[i + kn + 1] == '"') {
      size_t j = i + kn + 2;
      while (j < n && t[j] == ' ') j++;
      if (j < n && t[j] == ':') { j++; while (j < n && t[j] == ' ') j++; return t + j; }
    }
    str = true;
  }
  return NULL;
}
/* The end of the JSON value at v (one past it): an object or array matched, a string, or a bare word. */
static const char *dev_value_end(const char *v, const char *end) {
  if (v >= end) return v;
  if (*v == '{' || *v == '[') {
    int depth = 0; bool str = false;
    for (const char *p = v; p < end; p++) {
      if (str) { if (*p == '\\') p++; else if (*p == '"') str = false; continue; }
      if (*p == '"') str = true;
      else if (*p == '{' || *p == '[') depth++;
      else if ((*p == '}' || *p == ']') && --depth == 0) return p + 1;
    }
    return v;
  }
  const char *p = v;
  while (p < end && *p != ',' && *p != '}' && *p != ' ' && *p != '\n') p++;
  return p;
}
static long long dev_version(void) {
  const char *v = dev_world ? dev_field(dev_world, dev_world_n, "version") : NULL;
  return v ? atoll(v) : 0;
}
static void world_ask(void);
/* --dev: the control's answer is DIR/room-control.txt, with the last local publish's stamp after it, so
   a publish is said to every page in every room as production's is ("e edit", room-control.coax). */
static void dev_control(void) {
  static int64_t edit_seen = -1;
  size_t n = 0; char *t = dev_read("room-control.txt", &n);
  if (!t) { fprintf(stderr, "room: --dev %s has no room-control.txt\n", dev_dir); exit(1); }
  char *all = malloc(n + 40);
  if (!all) { free(t); return; }
  memcpy(all, t, n); free(t);
  if (dev_stamp > 0) n += (size_t)snprintf(all + n, 40, "\nedit %lld\n", dev_stamp);
  rm_control(view(), (coax_string){all, (int64_t)n});
  free(all);
  if (server->ctl.edit_known && server->ctl.edit != edit_seen) { if (edit_seen >= 0) world_ask(); edit_seen = server->ctl.edit; }
}
/* --dev: POST /api/playtest/world/everglade ({doc, summary}, an optional base: the version it was made
   from, 409 when another publish came first) and .../reset, as the API's (coax/web/api/player.c): the
   world document DIR/world-everglade.json written whole (a link there is replaced by the file, so the
   repo's dev/ copy is never written), the next version, and every room and page told. 0: not one. */
static int dev_publish(const char *req, size_t n, char *out, size_t on) {
  bool reset = !strncmp(req, "POST /api/playtest/world/everglade/reset", 40) && (req[40] == ' ' || req[40] == '?');
  bool publish = !strncmp(req, "POST /api/playtest/world/everglade", 34) && (req[34] == ' ' || req[34] == '?');
  if (!reset && !publish) return 0;
  const char *body = req + n;
  for (size_t k = 3; k < n; k++) if (!memcmp(req + k - 3, "\r\n\r\n", 4)) { body = req + k + 1; break; }
  size_t bn = (size_t)(req + n - body);
  long long was = dev_version(), next = reset ? 0 : was + 1;
  char *doc = NULL; size_t dn = 0;
  const char *dv = NULL, *de = NULL;
  if (publish) {
    const char *base = dev_field(body, bn, "base");
    if (base && atoll(base) != was) { snprintf(out, on, "{\"error\":\"stale\",\"version\":%lld}", was); return 409; }
    dv = dev_field(body, bn, "doc");
    de = dv ? dev_value_end(dv, body + bn) : NULL;
    if (!dv || *dv != '{' || de <= dv) { snprintf(out, on, "{\"error\":\"a doc\"}"); return 400; }
  }
  struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  doc = malloc((de ? (size_t)(de - dv) : 4) + 160);
  if (!doc) return 500;
  dn = (size_t)sprintf(doc, "{\"doc\": ");
  if (dv) { memcpy(doc + dn, dv, (size_t)(de - dv)); dn += (size_t)(de - dv); } else { memcpy(doc + dn, "null", 4); dn += 4; }
  dn += (size_t)sprintf(doc + dn, ", \"version\": %lld, \"publishedAt\": %lld.%03ld, \"publishedBy\": \"local\"}\n", next, (long long)ts.tv_sec, ts.tv_nsec / 1000000);
  char path[1024], tmp[1100];
  snprintf(path, sizeof path, "%s/world-everglade.json", dev_dir); snprintf(tmp, sizeof tmp, "%s.new", path);
  FILE *f = fopen(tmp, "wb");
  bool ok = f && fwrite(doc, 1, dn, f) == dn;
  if (f) ok = fclose(f) == 0 && ok;
  free(doc);
  if (!ok || rename(tmp, path) != 0) { unlink(tmp); snprintf(out, on, "{\"error\":\"not written\"}"); return 500; }
  dev_stamp = (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
  dev_control();   /* the room lays it (world_ask) and every page hears e edit */
  if (reset) snprintf(out, on, "{\"reset\":true,\"was\":%lld}", was); else snprintf(out, on, "{\"version\":%lld}", next);
  printf("room: --dev %s: version %lld\n", reset ? "the designed city back" : "a publish", next);
  return 200;
}

// How long the server takes over one socket's bytes (its frames read and every frame they make put
// onto every recipient's output): 0.1 ms buckets, this window's and the last full window's.
#define FAN_BUCKETS 1000
static int64_t fan_now[FAN_BUCKETS], fan_last[FAN_BUCKETS];
static int64_t now_us(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}
static void fan_add(int64_t us) { int64_t b = us / 100; fan_now[b < 0 ? 0 : b >= FAN_BUCKETS ? FAN_BUCKETS - 1 : b]++; }
// The q-th thousandth of h in microseconds (-1: nothing counted), and the count.
static int64_t fan_at(const int64_t *h, int q, int64_t *count) {
  int64_t n = 0; for (int i = 0; i < FAN_BUCKETS; i++) n += h[i];
  *count = n;
  if (!n) return -1;
  int64_t want = (n * q + 999) / 1000, k = 0;
  for (int i = 0; i < FAN_BUCKETS; i++) { k += h[i]; if (k >= want) return (int64_t)(i + 1) * 100; }
  return FAN_BUCKETS * 100;
}
// The everglade tick's time (every room's world stepped, its frames out): 0.1 ms buckets, as above.
static int64_t tick_now[FAN_BUCKETS], tick_last[FAN_BUCKETS];
static void tick_add(int64_t us) { int64_t b = us / 100; tick_now[b < 0 ? 0 : b >= FAN_BUCKETS ? FAN_BUCKETS - 1 : b]++; }
static int64_t now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static int64_t wall_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_REALTIME, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static void clock_in(void) { rm_clock(view(), now_ms(), wall_ms()); }
// As much of the waiting bytes as the socket takes now; false when the socket failed.
static bool pump_held;
static bool pump(int i) {
  conn *c = &conns[i];
  size_t done = 0;
  while (done < c->on) {
    ssize_t w = send(c->fd, c->out + done, c->on - done, MSG_NOSIGNAL);
    if (w < 0 && errno == EINTR) continue;
    if (w < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
    if (w <= 0) return false;
    done += (size_t)w;
  }
  if (done) { memmove(c->out, c->out + done, c->on - done); c->on -= done; }
  if (!c->on) { c->since = 0; if (st_c[i].burst == 2) st_drained(i, now_us()); }
  return true;
}
// n bytes onto i's output, sent as far as the socket takes them now. Past OUT_MAX, or on a failed
// socket, i is doomed (closed by reap, after the frame being sent reaches everyone else).
static void put_out(int i, const void *p, size_t n) {
  conn *c = &conns[i];
  if (c->fd < 0 || c->doomed) return;
  if (c->on + n > OUT_MAX) { c->doomed = true; slow_closed++; return; }
  if (c->on + n > c->ocap) {
    size_t cap = c->ocap ? c->ocap : 8192;
    while (cap < c->on + n) cap *= 2;
    uint8_t *grown = realloc(c->out, cap);
    if (!grown) { c->doomed = true; return; }
    c->out = grown; c->ocap = cap;
  }
  if (!c->on) c->since = now_ms();
  st_put(i, n);
  memcpy(c->out + c->on, p, n); c->on += n;
  if ((int64_t)c->on > out_peak) out_peak = (int64_t)c->on;
  if (!pump_held && !pump(i)) c->doomed = true;
}
/* Every connection's held output sent: the everglade tick holds its frames (pump_held) and sends each
   socket's once at its end, one send a socket a tick and not one a frame (a send under Cloud Run's
   gVisor costs tens of us; #12717). */
static void pump_all(void) {
  pump_held = false;
  int64_t now = now_ms();
  for (int j = 0; j < CONNS; j++) {
    if (conns[j].fd < 0 || conns[j].doomed || !conns[j].on) continue;
    if (!pump(j)) { conns[j].doomed = true; continue; }
    if (conns[j].on && now - conns[j].since >= OUT_AGE_MS) { conns[j].doomed = true; slow_closed++; }
  }
}
// The relay's frames (room-relay.coax), each onto the output of the connections it names.
static void relay_flush(void) {
  int64_t len;
  static uint8_t out[4300];
  while ((len = rm_relay_next(view())) > 0) {
    size_t k = ws_build(out, sizeof out, WS_TEXT, server->rout.e, (size_t)len, false);
    if (!k) continue;
    int to = 0, last = -1;
    for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && conns[j].ws && rm_relay_to(view(), j)) { put_out(j, out, k); to++; last = j; }
    /* counted by type (room-stats.h); a D frame (one seat's) with no more parts ends a joiner's first snapshot */
    int64_t t0 = now_us();
    int f = st_frame_out(server->rout.e, (size_t)len, to, k);
    if (f >= 0 && !(f & 2) && to == 1 && st_c[last].burst == 1) { st_c[last].burst = 2; if (!conns[last].on) st_drained(last, now_us()); }
    st_cost_us += now_us() - t0;
  }
}
#include "room-tick.c"   /* the tick both hosts run (room-worker.c too) */
static void everglade_tick(void) {
  int64_t was[PARTS]; memcpy(was, part_us, sizeof was);
  pump_held = true;
  for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && conns[j].ws) rm_conn_waiting(view(), j, (int64_t)conns[j].on);
  for (int k = 0; k < WORLD_ROOMS; k++) {
    if (!everglade_room_tick(k)) continue;
    int64_t tf = now_us();
    relay_flush();
    part_us[PART_SEND] += now_us() - tf;
    /* each seat's input that this tick's D acknowledges: heard to sent, plus its socket's round trip */
    int64_t tk = now_us(), ms = now_ms();
    for (int s = 0; s < WORLD_SEATS; s++) {
      int64_t at = rm_st_ack(view(), k, s);
      int c = at >= 0 ? (int)server->eg.e[k].seat.e[s].conn : -1;
      if (c >= 0 && c < CONNS) st_ack_add(c, ms - at + (st_c[c].rtt_ms > 0 ? st_c[c].rtt_ms : 0));
    }
    st_cost_us += now_us() - tk;
  }
  int64_t tp = now_us();
  pump_all();
  part_us[PART_SEND] += now_us() - tp;
  /* the tick's workloads, and every socket's queue now (room-stats.h) */
  int64_t ts = now_us(), d[PARTS];
  for (int q = 0; q < PARTS; q++) d[q] = part_us[q] - was[q];
  st_hist_add(&st_work[STW_PLAYERS], d[PART_MOVE]);
  st_hist_add(&st_work[STW_RAIDERS], d[PART_RULES]);
  st_hist_add(&st_work[STW_EFFECTS], d[PART_ABILITIES] + d[PART_SPELLS]);
  st_hist_add(&st_work[STW_FOLK], d[PART_TOWN] + d[PART_PEOPLE]);
  st_hist_add(&st_work[STW_WRITE], d[PART_WRITE]);
  st_hist_add(&st_work[STW_SEND], d[PART_SEND]);
  int64_t ms = now_ms();
  for (int j = 0; j < CONNS; j++) {
    if (conns[j].fd < 0 || !conns[j].on) continue;
    if ((int64_t)conns[j].on > st_c[j].q_bytes) st_c[j].q_bytes = (int64_t)conns[j].on;
    if (conns[j].since && ms - conns[j].since > st_c[j].q_ms) st_c[j].q_ms = ms - conns[j].since;
  }
  st_cost_us += now_us() - ts;
}
static void reap(void);
static void shut(int i) {
  if (conns[i].fd < 0) return;
  bool was = conns[i].ws;
  if (was) rm_left(view(), i);
  close(conns[i].fd);
  free(conns[i].out);
  conns[i] = (conn){.fd = -1};
  if (was) relay_flush();
}
// Closes every doomed connection (each close may queue an "l SEAT" that dooms another).
static void reap(void) {
  for (bool again = true; again;) {
    again = false;
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0 && conns[i].doomed) { shut(i); again = true; }
  }
}
// A short answer on a fresh socket (its send buffer is empty), then the close.
static void http(int i, const char *status, const char *type, const char *body) {
  static char out[1 << 17];
  int n = snprintf(out, sizeof out, "HTTP/1.1 %s\r\nContent-Type: %s\r\nCache-Control: no-store\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s", status, type, strlen(body), body);
  if (n > 0 && (size_t)n < sizeof out) { ssize_t w = send(conns[i].fd, out, (size_t)n, MSG_NOSIGNAL); (void)w; }
  shut(i);
}
// The request is in: a room's WebSocket, the health check, the rooms list, or nothing.
static void opening(int i) {
  conn *c = &conns[i];
  uint8_t *end = NULL;
  for (size_t k = 3; k < c->n; k++) if (!memcmp(c->in + k - 3, "\r\n\r\n", 4)) { end = c->in + k + 1; break; }
  if (!end) { if (c->n >= IN_CAP) http(i, "431 Request Header Fields Too Large", "text/plain", "too big"); return; }
  size_t len = (size_t)(end - c->in);
  coax_string req = {(const char *)c->in, (int64_t)len};
  /* --dev: the API's stand-in answers the page's /api/playtest/ requests (room-dev.coax), the body whole */
  if (dev_dir && (!strncmp((char *)c->in, "POST /api/", 10) || !strncmp((char *)c->in, "GET /api/playtest/", 18) || !strncmp((char *)c->in, "GET /api/auth/viewer", 20))) {
    size_t want = 0;
    for (uint8_t *p = c->in; p + 15 < end; p++)
      if ((p == c->in || p[-1] == '\n') && !strncasecmp((char *)p, "Content-Length:", 15)) want = (size_t)strtoul((char *)p + 15, NULL, 10);
    if (len + want > c->n && len + want < IN_CAP) return;   /* the rest of the body is still coming */
    coax_string whole = {(const char *)c->in, (int64_t)(len + want <= c->n ? len + want : c->n)};
    /* Coax proposes the floor probes and consumes the actual world's answers. */
    int64_t probes = rm_forge_probe(view(), whole);
    if (probes > 0) { float xy[2]; int k = (int)rm_forge_probe_at(view(), 0, (coax_v_f32){xy, 2}); world_forge_prepare(k); }
    for (int64_t p = 0; p < probes; p++) {
      float xy[2]; int k = (int)rm_forge_probe_at(view(), p, (coax_v_f32){xy, 2});
      rm_forge_probe_open(view(), p, world_forge_open(k, xy[0], xy[1]));
    }
    int fs = (int)rm_forge_api(view(), whole);
    if (fs > 0) {
      if (fs == 200 && server->forge.pending) {
        int64_t nn = rm_forge_save(view());
        char path[1024], tmp[1100];
        snprintf(path, sizeof path, "%s/forge-npcs.json", dev_dir); snprintf(tmp, sizeof tmp, "%s.new", path);
        FILE *f = nn > 0 ? fopen(tmp, "wb") : NULL;
        bool saved = f && fwrite(server->forge.out.e, 1, (size_t)nn, f) == (size_t)nn;
        if (f) { saved = fflush(f) == 0 && saved; saved = fsync(fileno(f)) == 0 && saved; saved = fclose(f) == 0 && saved; }
        if (saved) saved = rename(tmp, path) == 0;
        if (!saved) { unlink(tmp); rm_forge_abort(view()); http(i, "500 Internal Server Error", "application/json", "{\"ok\":false,\"error\":\"NPC document was not saved.\"}"); return; }
        rm_forge_commit(view());
      }
      static char body[32769]; size_t nn = (size_t)server->forge.n;
      if (nn >= sizeof body) { http(i, "500 Internal Server Error", "application/json", "{\"ok\":false}"); return; }
      memcpy(body, server->forge.out.e, nn); body[nn] = 0;
      http(i, fs == 200 ? "200 OK" : fs == 409 ? "409 Conflict" : fs == 403 ? "403 Forbidden" : "400 Bad Request", "application/json", body); return;
    }
    int64_t an = rm_dev_api(view(), whole);
    if (an < 0) {
      static char pub[256];
      int st = dev_publish((const char *)c->in, len + want <= c->n ? len + want : c->n, pub, sizeof pub);
      if (st > 0) {
        http(i, st == 200 ? "200 OK" : st == 409 ? "409 Conflict" : st == 400 ? "400 Bad Request" : "500 Internal Server Error", "application/json", pub); return;
      }
    }
    if (an > 0) {
      char status[8] = {0}; int64_t nl = 0;
      while (nl < an && nl < 7 && server->dev.out.e[nl] != '\n') { status[nl] = (char)server->dev.out.e[nl]; nl++; }
      static char body[1100]; int64_t bn = an - nl - 1; if (bn < 0) bn = 0; if (bn > (int64_t)sizeof body - 1) bn = sizeof body - 1;
      memcpy(body, server->dev.out.e + nl + 1, (size_t)bn); body[bn] = 0;
      const char *line = !strcmp(status, "200") ? "200 OK" : !strcmp(status, "401") ? "401 Unauthorized" : !strcmp(status, "403") ? "403 Forbidden" : !strcmp(status, "404") ? "404 Not Found" : "500 Internal Server Error";
      http(i, line, "application/json", body); return;
    }
  }
  /* a shard's own health through the load balancer: GET /api/room/sN/health (room-directory.coax) */
  bool shard_health = false;
  if (!strncmp((char *)c->in, "GET /api/room/s", 15)) {
    const char *p = (const char *)c->in + 15; int digits = 0;
    while (*p >= '0' && *p <= '9' && digits < 3) { p++; digits++; }
    shard_health = digits > 0 && !strncmp(p, "/health ", 8);
  }
  /* the last window's vitals (room-stats.h): GET /api/room/vitals, or a shard's through the load balancer */
  bool shard_vitals = false;
  if (!strncmp((char *)c->in, "GET /api/room/s", 15)) {
    const char *p = (const char *)c->in + 15; int digits = 0;
    while (*p >= '0' && *p <= '9' && digits < 3) { p++; digits++; }
    shard_vitals = digits > 0 && !strncmp(p, "/vitals ", 8);
  }
  if (shard_vitals || !strncmp((char *)c->in, "GET /api/room/vitals ", 21)) { http(i, "200 OK", "text/plain", st_last[0] ? st_last : "no window yet\n"); return; }
  if (shard_health || !strncmp((char *)c->in, "GET /health ", 12) || !strncmp((char *)c->in, "GET /api/room/health ", 21)) {
    // the fan-out time: this window's, else the last full minute's
    int64_t n = 0, p95 = fan_at(fan_now, 950, &n), p50, m;
    const int64_t *h = fan_now;
    if (n < 100) { int64_t last_n; int64_t last = fan_at(fan_last, 950, &last_n); if (last_n > n) { h = fan_last; p95 = last; n = last_n; } }
    p50 = fan_at(h, 500, &m);
    int64_t tn = 0, tp95 = fan_at(tick_now, 950, &tn), tm;
    const int64_t *th = tick_now;
    if (tn < 100) { int64_t ln; int64_t lp = fan_at(tick_last, 950, &ln); if (ln > tn) { tp95 = lp; tn = ln; th = tick_last; } }
    int64_t tp50 = fan_at(th, 500, &tm);
    char body[320]; snprintf(body, sizeof body, "ok %lld players, fanout p50 %lld us p95 %lld us over %lld reads, %lld rate-limited, %lld slow closed, shard %lld, everglade tick p50 %lld us at the 95th %lld us over %lld ticks\n",
                             (long long)rm_players(view()), (long long)p50, (long long)p95, (long long)n, (long long)rm_relay_dropped(view()), (long long)slow_closed,
                             (long long)server->shard, (long long)tp50, (long long)tp95, (long long)tn);
    http(i, "200 OK", "text/plain", body); return;
  }
  /* --dev: the API's stand-in (local.coax): the world document the room laid, byte for byte */
  if (dev_dir && !strncmp((char *)c->in, "GET /api/playtest/world/everglade", 33) && (c->in[33] == ' ' || c->in[33] == '?')) {
    http(i, "200 OK", "application/json", dev_world ? dev_world : "{}"); return;
  }
  /* --dev --no-rooms: the API's stand-in with the rooms down (an offline page's API, the drift check) */
  if (no_rooms && (!strncmp((char *)c->in, "GET /api/playtest/rooms/", 24) || !strncmp((char *)c->in, "GET /api/room/", 14))) {
    http(i, "503 Service Unavailable", "text/plain", "The rooms can't be reached (--no-rooms).\n"); return;
  }
  int64_t listed = rm_rooms(view(), req);
  if (listed >= 0) {
    char body[8200]; size_t n = (size_t)listed < sizeof body - 1 ? (size_t)listed : sizeof body - 1;
    memcpy(body, server->jout.e, n); body[n] = 0;
    http(i, "200 OK", "application/json", body); return;
  }
  char out[512];
  size_t hn = ws_server_handshake((char *)c->in, len, out, sizeof out);
  if (!hn) { http(i, "400 Bad Request", "text/plain", "Rooms: open a WebSocket to /api/room/orbit-league/NAME, or to /api/room/relay/GAME/CODE.\n"); return; }
  int64_t seat = rm_join(view(), i, req);
  if (seat < 0) { http(i, "404 Not Found", "text/plain", "No such room, or it is full.\n"); return; }
  c->ws = true;
  st_joined(i, now_us());
  put_out(i, out, hn);
  memmove(c->in, end, c->n - len); c->n -= len;
  relay_flush();
}
static void frames(int i) {
  conn *c = &conns[i];
  while (c->fd >= 0 && !c->doomed && c->n) {
    ws_frame f;
    int r = ws_parse(c->in, c->n, 4096, &f);
    if (r == 0) return;
    if (r < 0) { c->doomed = true; return; }
    if (f.opcode == WS_TEXT) {
      st_frame_in(f.payload, f.length);
      rm_heard(view(), i, (coax_string){(const char *)f.payload, (int64_t)f.length});
      relay_flush();
      if (rm_relay_kicked(view(), i)) { c->doomed = true; return; }
    } else if (f.opcode == WS_PING) {
      uint8_t out[160]; size_t k = ws_build(out, sizeof out, WS_PONG, f.payload, f.length, false);
      if (!k) { c->doomed = true; return; }
      put_out(i, out, k);
    } else if (f.opcode == WS_CLOSE) {
      uint8_t out[8]; size_t k = ws_build(out, sizeof out, WS_CLOSE, (const uint8_t *)"\x03\xe8", 2, false);
      if (k) put_out(i, out, k);
      c->doomed = true; return;
    }
    memmove(c->in, c->in + f.consumed, c->n - f.consumed); c->n -= f.consumed;
  }
}

/* Players' HP between visits (room-vitals.coax, #12715): what rm_vitals_due wrote goes to the API by
   curl, its config on stdin (so the MAC is in no argument list). The curls are started by a small
   poster process forked before the server's memory is taken (a fork of the full server can fail on
   Cloud Run): the server writes each config to it down a pipe, "LEN\n" then the bytes, and never waits. */
/* The poster also asks the API's word on the rooms (room-control.coax, #12714): a "G" request's
   answer comes back down a second pipe, "LEN\n" then the bytes, which the server reads without waiting. */
/* A "V" request is a vitals post (room-vitals.coax, #12793): "BATCH\n" then the curl config. The poster
   hands it to a child of its own and goes on; the child waits for curl's answer and writes one message
   down a third pipe, "BATCH STATUS LEN\n" then the body, in a single write of at most RECEIPT_MSG bytes
   (under PIPE_BUF, so messages never interleave). STATUS is the HTTP status, 0 when curl got none. */
#define RECEIPT_MSG 500
static int poster_fd = -1, answer_fd = -1, receipt_fd = -1;
static void poster_vitals(const char *req, size_t n, int out) {
  const char *nl = memchr(req, '\n', n);
  if (!nl) return;
  long long batch = strtoll(req, NULL, 10);
  const char *cfg = nl + 1; size_t cn = n - (size_t)(nl + 1 - req);
  pid_t pid = fork();
  if (pid != 0) { if (pid < 0) fprintf(stderr, "room: poster: fork failed: %s\n", strerror(errno)); return; }
  signal(SIGCHLD, SIG_DFL);   /* this child waits for its curl */
  int p[2], o[2];
  char msg[RECEIPT_MSG], body[RECEIPT_MSG];
  size_t k = 0; int status = 0;
  if (!pipe(p) && !pipe(o)) {
    pid_t c = fork();
    if (c == 0) {
      dup2(p[0], 0); dup2(o[1], 1);
      for (int fd = 3; fd < 1024; fd++) close(fd);
      execlp("curl", "curl", "-K", "-", (char *)NULL);
      _exit(127);
    }
    close(p[0]); close(o[1]);
    if (c > 0) { ssize_t w = write(p[1], cfg, cn); (void)w; }
    close(p[1]);
    static char got[16384]; size_t gn = 0; ssize_t r;
    while (gn < sizeof got - 1 && (r = read(o[0], got + gn, sizeof got - 1 - gn)) > 0) gn += (size_t)r;
    close(o[0]);
    if (c > 0) waitpid(c, NULL, 0);
    got[gn] = 0;
    /* curl's write-out puts "\n#HTTP CODE" after the body */
    char *mark = NULL;
    for (char *q = strstr(got, "\n#HTTP "); q; q = strstr(q + 1, "\n#HTTP ")) mark = q;
    if (mark) { status = atoi(mark + 7); k = (size_t)(mark - got); }
    if (k > sizeof body - 1) k = sizeof body - 1;
    memcpy(body, got, k);
  }
  int hn = snprintf(msg, sizeof msg, "%lld %d %zu\n", batch, status, k);
  if (hn > 0 && (size_t)hn + k <= sizeof msg) { memcpy(msg + hn, body, k); ssize_t w = write(out, msg, (size_t)hn + k); (void)w; }
  _exit(0);
}
static void poster_run(int in, int back, int receipts) {
  static char cfg[16384], got[16384];
  FILE *f = fdopen(in, "r");
  for (;;) {
    long n; int kind = f ? fgetc(f) : EOF;
    if (kind == EOF || fscanf(f, "%ld", &n) != 1 || fgetc(f) != '\n' || n <= 0 || n >= (long)sizeof cfg) _exit(0);
    if (fread(cfg, 1, (size_t)n, f) != (size_t)n) _exit(0);
    if (kind == 'V') { poster_vitals(cfg, (size_t)n, receipts); continue; }
    int p[2], o[2] = {-1, -1};
    if (pipe(p)) continue;
    if (kind == 'G' && pipe(o)) { close(p[0]); close(p[1]); continue; }
    pid_t pid = fork();
    if (pid == 0) {
      dup2(p[0], 0);
      if (o[1] >= 0) dup2(o[1], 1);
      for (int fd = 3; fd < 1024; fd++) close(fd);
      execlp("curl", "curl", "-K", "-", (char *)NULL);
      _exit(127);
    }
    close(p[0]);
    if (o[1] >= 0) close(o[1]);
    if (pid > 0) { ssize_t w = write(p[1], cfg, (size_t)n); (void)w; }
    else fprintf(stderr, "room: poster: fork failed: %s\n", strerror(errno));
    close(p[1]);
    if (o[0] >= 0) {
      size_t k = 0; ssize_t r;
      while (k < sizeof got && (r = read(o[0], got + k, sizeof got - k)) > 0) k += (size_t)r;
      close(o[0]);
      if (k > 0 && k < sizeof got) {
        char head[24]; int hn = snprintf(head, sizeof head, "%zu\n", k);
        if (write(back, head, (size_t)hn) != hn || write(back, got, k) != (ssize_t)k) _exit(0);
      }
    }
  }
}
static void poster_start(void) {
  int p[2], b[2], v[2];
  if (pipe(p)) { perror("room: poster pipe"); return; }
  if (pipe(b)) { perror("room: poster pipe"); close(p[0]); close(p[1]); return; }
  if (pipe(v)) { perror("room: poster pipe"); close(p[0]); close(p[1]); close(b[0]); close(b[1]); return; }
  pid_t pid = fork();
  if (pid == 0) { close(p[1]); close(b[0]); close(v[0]); poster_run(p[0], b[1], v[1]); _exit(0); }
  close(p[0]); close(b[1]); close(v[1]);
  if (pid < 0) { perror("room: poster"); close(p[1]); close(b[0]); close(v[0]); return; }
  poster_fd = p[1]; answer_fd = b[0]; receipt_fd = v[0];
  fcntl(answer_fd, F_SETFL, O_NONBLOCK); fcntl(receipt_fd, F_SETFL, O_NONBLOCK);
}
static bool poster_send(char kind, const char *cfg, size_t k) {
  static char msg[16384 + 32];
  if (poster_fd < 0 || k + 32 > sizeof msg) return false;
  int hn = snprintf(msg, sizeof msg, "%c%zu\n", kind, k);
  memcpy(msg + hn, cfg, k);
  if (write(poster_fd, msg, (size_t)hn + k) != (ssize_t)((size_t)hn + k)) { fprintf(stderr, "room: poster: request dropped\n"); return false; }
  return true;
}
/* The control ask, every two seconds while the server has a key, and its answers as they come. */
static void control_ask(void) {
  if (!rm_control_due(view())) return;
  if (dev_dir) {   /* --dev: DIR/room-control.txt is the API's answer, read as often as the API is asked */
    dev_control();
    return;
  }
  const char *url = getenv("OA_ROOM_CONTROL_URL");
  if (!url || !*url) url = "https://openagents.com/api/playtest/room-control";
  char cfg[512];
  int k = snprintf(cfg, sizeof cfg, "url = \"%s\"\nheader = \"X-Room-Mac: %.64s\"\nheader = \"X-Room-At: %.*s\"\nsilent\nshow-error\nfail\nmax-time = 5\n",
                   url, (const char *)server->ctl.mac.e, (int)(server->ctl.ask_n - 2), (const char *)server->ctl.ask.e + 2);
  if (k > 0 && k < (int)sizeof cfg) poster_send('G', cfg, (size_t)k);
}
static const char *api_url(const char *env, const char *path, char *buf, size_t cap);
/* The realm's published world, asked for at the start and again each time the control says it
   changed (its edit stamp): the answer is laid over every room (room-world.c world_edits, #12731).
   It is the API's, the same URL the page asks (OA_ROOM_API's origin); under --dev, DIR's document. */
static void world_ask(void) {
  if (dev_dir) {
    free(dev_world); dev_world = dev_read("world-everglade.json", &dev_world_n);
    if (!dev_world || dev_world[0] != '{') { fprintf(stderr, "room: --dev %s has no world-everglade.json\n", dev_dir); exit(1); }
    world_edits(dev_world, (int64_t)dev_world_n); rm_eg_placed_reset(view());
    printf("room: the local world (%zu bytes) laid over the rooms\n", dev_world_n);
    return;
  }
  char ub[512];
  const char *url = api_url("OA_ROOM_WORLD_URL", "/api/playtest/world/everglade", ub, sizeof ub);
  char cfg[512];
  int k = snprintf(cfg, sizeof cfg, "url = \"%s\"\nsilent\nshow-error\nfail\nmax-time = 10\n", url);
  if (k > 0 && k < (int)sizeof cfg) poster_send('G', cfg, (size_t)k);
}
static void control_hear(void) {
  static int64_t edit_asked = -1;
  static char buf[32768]; static size_t have;
  if (answer_fd < 0) return;
  ssize_t r;
  while (have < sizeof buf && (r = read(answer_fd, buf + have, sizeof buf - have)) > 0) have += (size_t)r;
  for (;;) {
    char *nl = memchr(buf, '\n', have);
    if (!nl) break;
    size_t len = (size_t)strtoul(buf, NULL, 10), head = (size_t)(nl - buf) + 1;
    if (have < head + len) break;
    /* the world's answer is JSON ({"doc": ...}); the control's is lines */
    if (len > 0 && buf[head] == '{') { world_edits(buf + head, (int64_t)len); rm_eg_placed_reset(view()); printf("room: the realm's world (%zu bytes) laid over the rooms\n", len); }
    else {
      rm_control(view(), (coax_string){buf + head, (int64_t)len});
      if (server->ctl.edit_known && server->ctl.edit != edit_asked) { edit_asked = server->ctl.edit; world_ask(); }
    }
    memmove(buf, buf + head + len, have - head - len); have -= head + len;
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0 && rm_relay_kicked(view(), i)) conns[i].doomed = true;
  }
  if (have == sizeof buf) have = 0;
}
/* A signed report to the API (the directory, the stats) through the poster: a curl config with the
   body. A vitals post (batch >= 0) goes as a "V" request, its answer back as a receipt (receipt_hear). */
static bool post_report(const char *url, const uint8_t *mac, const uint8_t *body, int64_t n, int64_t batch) {
  static char cfg[16384];
  if (poster_fd < 0) return false;
  size_t k = 0;
  if (batch >= 0) k = (size_t)snprintf(cfg, sizeof cfg, "%lld\n", (long long)batch);
  k += (size_t)snprintf(cfg + k, sizeof cfg - k, "url = \"%s\"\nheader = \"X-Room-Mac: %.64s\"\nheader = \"Content-Type: text/plain\"\nsilent\nshow-error\n%s",
                        url, (const char *)mac,
                        batch >= 0 ? "max-time = 5\nwrite-out = \"\\n#HTTP %{http_code}\"\ndata-binary = \"" : "max-time = 10\noutput = \"/dev/null\"\ndata-binary = \"");
  for (int64_t i = 0; i < n && k + 4 < sizeof cfg; i++) {
    char ch = (char)body[i];
    if (ch == '\n') { cfg[k++] = '\\'; cfg[k++] = 'n'; } else cfg[k++] = ch;
  }
  if (k + 4 >= sizeof cfg) { fprintf(stderr, "room: a report too long to post\n"); return false; }
  k += (size_t)snprintf(cfg + k, sizeof cfg - k, "\"\n");
  return poster_send(batch >= 0 ? 'V' : 'P', cfg, k);
}
static void post_signed(const char *url, const uint8_t *mac, const uint8_t *body, int64_t n) { (void)post_report(url, mac, body, n, -1); }
static const char *api_url(const char *env, const char *path, char *buf, size_t cap) {
  const char *u = getenv(env);
  if (u && *u) return u;
  const char *base = getenv("OA_ROOM_API");   /* the API's origin (default production's) */
  snprintf(buf, cap, "%s%s", base && *base ? base : "https://openagents.com", path);
  return buf;
}
/* Players' HP, XP and WANTED between visits (room-vitals.coax, #12715, #12793, #12794): the outbox's
   next post, and its receipt when it comes. A request the poster's pipe dropped failed at once.
   --dev posts only when OA_ROOM_VITALS_URL names an API (the vitals check, coax/web/api/
   tests/room_checks.coax); otherwise the stand-in keeps the room's own notes (room-dev.coax). */
static void vitals_post(void) {
  int64_t n = rm_vitals_due(view());
  if (n <= 0) return;
  const char *dev_url = getenv("OA_ROOM_VITALS_URL");
  if (dev_dir && !(dev_url && *dev_url)) { rm_vitals_local(view()); return; }
  char buf[512];
  int64_t batch = server->vit.batch;
  if (!post_report(api_url("OA_ROOM_VITALS_URL", "/api/playtest/room-vitals", buf, sizeof buf), server->vit.mac.e, server->vit.out.e, n, batch))
    rm_vitals_answer(view(), batch, 0, (coax_string){"", 0});
}
static void receipt_hear(void) {
  static char buf[8192]; static size_t have;
  if (receipt_fd < 0) return;
  ssize_t r;
  while (have < sizeof buf && (r = read(receipt_fd, buf + have, sizeof buf - have)) > 0) have += (size_t)r;
  for (;;) {
    char *nl = memchr(buf, '\n', have);
    if (!nl) break;
    long long batch = 0; int status = 0; size_t len = 0;
    if (sscanf(buf, "%lld %d %zu", &batch, &status, &len) != 3) { have = 0; break; }
    size_t head = (size_t)(nl - buf) + 1;
    if (have < head + len) break;
    if (status < 200 || status >= 300) fprintf(stderr, "room: vitals: post %lld answered %d\n", batch, status);
    rm_vitals_answer(view(), batch, status, (coax_string){buf + head, (int64_t)len});
    memmove(buf, buf + head + len, have - head - len); have -= head + len;
  }
  if (have == sizeof buf) have = 0;
}
/* SIGTERM (Cloud Run gives 10 s before SIGKILL): every seat's HP, XP and WANTED noted and the outbox
   flushed, then the process exits once every note has a receipt or DRAIN_MS has passed (#12793).
   The rest of a drain (no new seats, the directory told, the pages told) is #12797's. */
#define DRAIN_MS 8000
static volatile sig_atomic_t term_asked;
static void on_term(int sig) { (void)sig; term_asked = 1; }
/* This shard's everglade rooms and who sits in them, for the API's instance directory
   (room-directory.coax, coax/web/api/rooms.c, #12717). */
static void directory_post(void) {
  int64_t n = rm_directory_due(view());
  if (n <= 0 || dev_dir) return;   /* --dev: the room's own list answers /api/playtest/rooms/ */
  char buf[512];
  post_signed(api_url("OA_ROOM_DIRECTORY_URL", "/api/playtest/room-directory", buf, sizeof buf), server->dir.mac.e, server->dir.out.e, n);
}

/* Once a second, off the tick: each seat's relevant entities, and its socket's round trip (room-stats.h). */
static void stats_sample(void) {
  int64_t t0 = now_us(), ks[2];
  for (int i = 0; i < CONNS; i++) {
    if (conns[i].fd < 0 || !conns[i].ws || !rm_st_conn(view(), i, (coax_v_s64){ks, 2})) continue;
    st_c[i].relevant = (int)rm_st_relevant(view(), ks[0], ks[1]);
    st_c[i].rtt_ms = st_rtt(conns[i].fd);
    int64_t w[4];
    if (rm_st_wire(view(), ks[0], ks[1], (coax_v_s64){w, 4})) {
      st_c[i].aged += w[0] >= st_c[i].aged_was ? w[0] - st_c[i].aged_was : w[0];   /* a new seat counts from 0 */
      st_c[i].aged_was = w[0];
      if (w[1] > st_c[i].wait_most) st_c[i].wait_most = w[1];
      if (w[2] > st_c[i].left_most) st_c[i].left_most = w[2];
    }
  }
  st_cost_us += now_us() - t0;
}
/* Each window: its report kept for GET /api/room/vitals, a line in the log, and the signed post the
   admins' room view reads (POST /api/playtest/room-stats, coax/web/api/rooms.c; never under --dev). */
static void stats_window(double secs) {
  static char body[ST_BODY_CAP];
  st_buf o = {body, 0, sizeof body};
  body[0] = 0;
  st_line(&o, "t %lld", (long long)wall_ms());
  size_t head = o.n;
  double cost = st_tick_us > 0 ? 100.0 * (double)st_cost_us / (double)st_tick_us : 0;
  int64_t p95 = st_hist_at(&st_work[STW_ALL], 950), ack95 = st_ack_q(NULL, st_ack_all, st_ack_all_n, 950);
  st_report(&o, view(), rm_players(view()), server->shard, secs, true);
  memcpy(st_last, body + head, o.n - head + 1);
  if (rm_players(view()) > 0)
    printf("room: vitals: tick p95 %.2f ms, input to ack p95 %lld ms, measuring %.2f%% of the tick\n", p95 / 1000.0, (long long)ack95, cost);
  if (dev_dir || poster_fd < 0) return;
  static uint8_t mac[65];
  if (!rm_st_sign(view(), (coax_string){body, (int64_t)o.n}, (coax_v_u8){mac, 64})) return;
  char buf[512];
  post_signed(api_url("OA_ROOM_STATS_URL", "/api/playtest/room-stats", buf, sizeof buf), mac, (const uint8_t *)body, (int64_t)o.n);
}

/* The server's memory with every field at its declared default (coax_default_t_RmServer), then rm_init
   (room.coax): never the zeros calloc gives, which would read the level cap as 0 (#12774). */
static coax_t_RmServer *server_new(void) {
  coax_t_RmServer *s = malloc(sizeof *s);
  if (!s) return NULL;
  coax_default_t_RmServer(s);
  rm_init((coax_v_t_RmServer){s, 1});
  return s;
}
/* --start-check (build.sh): the started server holds every declared default (rm_start_check), and is
   the same bytes however its memory was filled before. */
static int start_check(void) {
  coax_t_RmServer *a = server_new();
  coax_t_RmServer *b = malloc(sizeof *b);
  if (!a || !b) { fprintf(stderr, "room: out of memory\n"); return 1; }
  memset(b, 0xa5, sizeof *b);
  coax_default_t_RmServer(b);
  rm_init((coax_v_t_RmServer){b, 1});
  int64_t bad = rm_start_check((coax_v_t_RmServer){a, 1});
  bool same = !memcmp(a, b, sizeof *a);
  printf("room: start check: %lld fields off their defaults, %s\n", (long long)bad, same ? "the same from any memory" : "differs with the memory it started in");
  return bad == 0 && same ? 0 : 1;
}

int main(int argc, char **argv) {
  const char *address = "127.0.0.1"; int port = 8830; bool native_local = false;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) address = argv[++i];
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = atoi(argv[++i]);
    else if (!strcmp(argv[i], "--dev") && i + 1 < argc) dev_dir = argv[++i];
    else if (!strcmp(argv[i], "--no-rooms")) no_rooms = true;
    else if (!strcmp(argv[i], "--native-local")) native_local = true;
    else if (!strcmp(argv[i], "--start-check")) return start_check();
  }
  const char *env = getenv("PORT");
  if (env && *env) { port = atoi(env); address = "0.0.0.0"; }
  signal(SIGPIPE, SIG_IGN);
  signal(SIGCHLD, SIG_IGN);   /* vitals_post's curls reap themselves */
  setvbuf(stdout, NULL, _IOLBF, 0);
  poster_start();   /* before the server's memory (vitals_post) */
  server = server_new();
  if (!server) { fprintf(stderr, "room: out of memory\n"); return 1; }
  rm_vitals_gen(view(), wall_ms());   /* this process's generation, in every vitals operation ID (#12793) */
  signal(SIGTERM, on_term);
  if (dev_dir) { rm_dev_start(view()); const char *da = getenv("OA_ROOM_DEV_ADMIN"); if (da && !strcmp(da, "0")) rm_dev_admin(view(), false); printf("room: --dev %s: the API's stand-in (seat tickets checked and required)\n", dev_dir); }
  if (native_local && !rm_native_local(view(), (coax_string){address,(int64_t)strlen(address)})) {
    fprintf(stderr,"room: --native-local requires --dev and --listen 127.0.0.1\n");return 1;
  }
  world_ask();   /* the realm's published world, for every room's plan (room-world.c world_edits) */
  if (dev_dir) {
    size_t nn = 0; char *doc = dev_read("forge-npcs.json", &nn);
    if (doc && !rm_forge_load(view(), (coax_string){doc, (int64_t)nn})) { fprintf(stderr, "room: invalid saved forge-npcs.json; retained for recovery\n"); free(doc); return 1; }
    free(doc);
  }
  /* the seat-ticket key (Secret Manager oa-room-ticket-key): set, an everglade join needs a ticket (room-ticket.coax) */
  const char *tkey = getenv("OA_ROOM_TICKET_KEY");
  const char *must = getenv("OA_ROOM_TICKET_REQUIRED");   /* "0": a join without a ticket sits with no id (while pages roll out) */
  bool required = !(must && !strcmp(must, "0"));
  /* which shard of the instance directory this is (oa-room 1, oa-room-K K: room-directory.coax) */
  const char *shard = getenv("OA_ROOM_SHARD");
  if (shard && *shard) rm_shard(view(), atoi(shard));
  printf("room: shard %lld\n", (long long)server->shard);
  if (dev_dir) {}
  else if (tkey && strlen(tkey) >= 32) { rm_ticket_key(view(), (coax_string){tkey, (int64_t)strlen(tkey)}, required); printf("room: seat tickets checked%s\n", required ? ", required" : ""); }
  else printf("room: no OA_ROOM_TICKET_KEY, seat tickets not checked\n");
  int lfd = socket(AF_INET, SOCK_STREAM, 0);
  int one = 1; setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0};
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  inet_pton(AF_INET, address, &a.sin_addr);
  if (bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 512)) { perror("room: listen"); return 1; }
  fcntl(lfd, F_SETFL, O_NONBLOCK); fcntl(lfd, F_SETFD, FD_CLOEXEC);
  for (int i = 0; i < CONNS; i++) conns[i].fd = -1;
  printf("room: listening on %s:%d\n", address, port);
  const double tick = 1000.0 / 60.0;
  double next = (double)now_ms();
  int64_t ticks = 0, last_report = now_ms(), last_vitals = now_ms(), last_stats = now_ms(), drain_at = 0;
  for (;;) {
    if (term_asked && !drain_at) { drain_at = now_ms(); rm_vitals_flush(view()); last_vitals = 0; printf("room: SIGTERM: saving %lld players' vitals before exit\n", (long long)rm_vitals_pending(view())); }
    if (drain_at) {
      int64_t left = rm_vitals_pending(view());
      if ((left == 0 && !server->vit.fly) || now_ms() - drain_at > DRAIN_MS) {
        printf("room: exit after %lld ms, %lld players' vitals with no receipt\n", (long long)(now_ms() - drain_at), (long long)left);
        return 0;
      }
    }
    struct pollfd fds[CONNS + 1]; int map[CONNS + 1], n = 0;
    fds[n] = (struct pollfd){lfd, POLLIN, 0}; map[n++] = -1;
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0) { fds[n] = (struct pollfd){conns[i].fd, (short)(POLLIN | (conns[i].on ? POLLOUT : 0)), 0}; map[n++] = i; }
    int wait = (int)(next - (double)now_ms());
    if (poll(fds, (nfds_t)n, wait > 0 ? wait : 0) < 0 && errno != EINTR) { perror("room: poll"); return 1; }
    int64_t now = now_ms();
    clock_in();
    for (int k = 0; k < n; k++) {
      if (map[k] >= 0 && (fds[k].revents & POLLOUT) && conns[map[k]].fd >= 0 && !pump(map[k])) conns[map[k]].doomed = true;
      if (!(fds[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (map[k] < 0) {
        for (;;) {
          int fd = accept(lfd, NULL, NULL);
          if (fd < 0) break;
          int slot = -1;
          for (int i = 0; i < CONNS; i++) if (conns[i].fd < 0) { slot = i; break; }
          if (slot < 0) { close(fd); rm_st_refuse_more(view(), 0); continue; }
          fcntl(fd, F_SETFL, O_NONBLOCK); fcntl(fd, F_SETFD, FD_CLOEXEC);   /* never a vitals curl's */
          setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
          conns[slot] = (conn){.fd = fd, .heard = now};
        }
        continue;
      }
      int i = map[k]; conn *c = &conns[i];
      int64_t t_in = now_us();
      if (c->fd < 0 || c->doomed) continue;
      ssize_t r = recv(c->fd, c->in + c->n, IN_CAP - c->n, 0);
      if (r <= 0) { if (r < 0 && (errno == EAGAIN || errno == EINTR)) continue; c->doomed = true; continue; }
      c->n += (size_t)r; c->heard = now;
      if (!c->ws) opening(i);
      if (c->fd >= 0 && c->ws) frames(i);
      if (c->fd >= 0 && c->n >= IN_CAP) c->doomed = true;
      if (r > 0 && conns[i].ws) fan_add(now_us() - t_in);
    }
    /* silent 20 s, or kicked: a player's silent socket their new one replaced (rl_replace_stale) says
       nothing more, so no frame of its own will doom it; nor may it wait on the API's next control reply */
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0 && (now - conns[i].heard > 20000 || (conns[i].ws && rm_relay_kicked(view(), i)))) conns[i].doomed = true;
    reap();
    if ((double)now >= next) {
      clock_in();
      int64_t t_tick = now_us();
      rm_tick(view());
      if (ticks % 2 == 0) { everglade_tick(); int64_t dt = now_us() - t_tick; tick_add(dt); st_hist_add(&st_work[STW_ALL], dt); st_ticks++; st_tick_us += dt; }
      for (int i = 0; i < CONNS; i++) {
        if (conns[i].fd < 0 || !conns[i].ws) continue;
        int64_t len = rm_state(view(), i);
        if (len <= 0) continue;
        uint8_t out[2100]; size_t k = ws_build(out, sizeof out, WS_TEXT, server->out.e, (size_t)len, false);
        if (!k) conns[i].doomed = true; else put_out(i, out, k);
      }
      reap();
      ticks++;
      next += tick;
      if ((double)now - next > 250.0) {   // a stall: start the beat again (each seat earns its steps by the clock: rm_eg_begin)
        printf("room: a stall of %.0f ms, the beat started again\n", (double)now - next);
        next = (double)now + tick;
      }
      if (now - last_vitals >= 1000) { last_vitals = now; vitals_post(); directory_post(); stats_sample(); }
      if (now - last_stats >= ST_WINDOW_MS) { stats_window((now - last_stats) / 1000.0); last_stats = now; }
      control_ask(); control_hear(); receipt_hear();
      if (now - last_report >= 60000) { printf("room: %lld ticks in %.1f s, %lld players, %lld frames over the rate limits, %lld queue full, %lld slow sockets closed, %lld bytes the most waiting\n", (long long)ticks, (now - last_report) / 1000.0, (long long)rm_players(view()), (long long)rm_relay_dropped(view()), (long long)rm_relay_queue_full(view()), (long long)slow_closed, (long long)out_peak);
        printf("room: everglade tick parts over the minute (ms):"); for (int q = 0; q < PARTS; q++) { printf(" %s %lld", part_name[q], (long long)(part_us[q] / 1000)); part_us[q] = 0; }
        printf(", %lld events with no room in their tick\n", (long long)rm_eg_dropped(view()));
        { coax_v_t_RmServer w = view();
          printf("room: vitals posts %lld (%lld retried, %lld failed), lines applied %lld, already applied %lld, stale %lld, refused %lld, late answers %lld, given up %lld, evicted unsaved %lld, %lld players unsaved now\n",
                 (long long)rm_vitals_stat(w, 0), (long long)rm_vitals_stat(w, 1), (long long)rm_vitals_stat(w, 2), (long long)rm_vitals_stat(w, 3), (long long)rm_vitals_stat(w, 4),
                 (long long)rm_vitals_stat(w, 5), (long long)rm_vitals_stat(w, 6), (long long)rm_vitals_stat(w, 7), (long long)rm_vitals_stat(w, 8), (long long)rm_vitals_stat(w, 9), (long long)rm_vitals_stat(w, 10)); }
        ticks = 0; last_report = now; memcpy(fan_last, fan_now, sizeof fan_now); memset(fan_now, 0, sizeof fan_now); memcpy(tick_last, tick_now, sizeof tick_now); memset(tick_now, 0, sizeof tick_now); }
    }
  }
}
