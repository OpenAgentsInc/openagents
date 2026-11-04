/* The Orbit League room server's host (ol-room): TCP, the WebSocket
   handshake and frames (verse/voice-host/ws.c), and the 60 Hz beat. Every
   room decision is room.coax's: the host hands it each new socket's request
   (rm_join), each text frame (rm_heard) and each close (rm_left), calls
   rm_tick 60 times a second and sends each player what rm_state writes.
     room [--listen 127.0.0.1] [--port 8830] [--dev DIR]     PORT in the environment wins (Cloud Run: 0.0.0.0)
   --dev DIR is local development (coax/web/room/local.sh): the room is also the API's stand-in. Its
   world is DIR/world-everglade-5e.json, and it answers GET /api/playtest/world/everglade-5e with
   that same document, so a page served with `serve.sh --api` reads the world the room plays.
   Without --dev, the world comes from the API (OA_ROOM_API, default production's), as the page's does.
   OA_ROOM_SHARD names which shard of the everglade instance directory this is (room-directory.coax).
   GET /health answers 200 "ok" with the number of players; GET /api/playtest/rooms/SLUG
   answers rm_rooms' JSON (an everglade realm's instances and which to join).
   Every socket has its own output buffer: a frame for many goes onto each, sent as far as the
   socket takes it, the rest when poll says it can take more. No send waits on one slow socket;
   one whose waiting bytes pass OUT_MAX is closed. */
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
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define CONNS 512
#define IN_CAP 16384
#define OUT_MAX (256 * 1024)   // a connection whose unsent bytes pass this is closed (one slow socket never waits on the others)
typedef struct {
  int fd; bool ws; size_t n; uint8_t in[IN_CAP]; int64_t heard;
  uint8_t *out; size_t on, ocap;   // bytes waiting to go out (sent as the socket takes them)
  bool doomed;                     // to close once the frame being sent is out to everyone
} conn;
static conn conns[CONNS];
static coax_t_RmServer *server;
static int64_t slow_closed, out_peak;
static coax_v_t_RmServer view(void) { return (coax_v_t_RmServer){server, 1}; }
/* --dev DIR: the local stand-in for the API (local.sh). The world document is read once from DIR and
   is the one both the room and the page use. */
static const char *dev_dir;
static char *dev_world; static size_t dev_world_n;
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
/* Where the everglade tick's time goes (#12717): microseconds summed over the report's minute for the
   movement steps, the hotbar abilities, the town (watch and folk), the spells' pieces, the people for the rules and the pages,
   rm_eg_end (the rules' step and every seat's D) and the frames sent (relay_flush, pump_all), printed with the tick line. */
enum { PART_MOVE, PART_ABILITIES, PART_TOWN, PART_SPELLS, PART_PEOPLE, PART_END, PART_SEND, PARTS };
static int64_t part_us[PARTS];
static const char *part_name[PARTS] = {"move", "abilities", "town", "spells", "people", "end", "send"};
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
  memcpy(c->out + c->on, p, n); c->on += n;
  if ((int64_t)c->on > out_peak) out_peak = (int64_t)c->on;
  if (!pump_held && !pump(i)) c->doomed = true;
}
/* Every connection's held output sent: the everglade tick holds its frames (pump_held) and sends each
   socket's once at its end, one send a socket a tick and not one a frame (a send under Cloud Run's
   gVisor costs tens of us; #12717). */
static void pump_all(void) {
  pump_held = false;
  for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && !conns[j].doomed && conns[j].on && !pump(j)) conns[j].doomed = true;
}
// The relay's frames (room-relay.coax), each onto the output of the connections it names.
static void relay_flush(void) {
  int64_t len;
  static uint8_t out[4300];
  while ((len = rm_relay_next(view())) > 0) {
    size_t k = ws_build(out, sizeof out, WS_TEXT, server->rout.e, (size_t)len, false);
    if (!k) continue;
    for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && conns[j].ws && rm_relay_to(view(), j)) put_out(j, out, k);
  }
}
// The everglade rooms' tick (room-everglade.coax, 30 Hz: every other host tick): each seat at the
// gate when it sits, then its queued inputs through the movement step in its room's world
// (room-world.c), then the rolls and every seat's state, out at once.
// Room k's town for this tick (room-world.c world_town, GitHub #12713): the rules' Hit Points and the
// crimes since into it, a step against every seat, then the guards' blows, WANTED and where everyone is
// back to the room (the rules', and what the pages hear).
static void everglade_town(int k) {
  float hp[3], cr[3], seats[WORLD_SEATS * WORLD_SEAT_TOWN], took[2 * WORLD_SEATS], people[WORLD_FOLK * WORLD_PEOPLE], fv[WORLD_VIEW];
  coax_v_f32 hpv = {hp, 3}, crv = {cr, 3}, fvv = {fv, WORLD_VIEW};
  if (rm_eg_fresh(view(), k)) world_fresh(k);
  for (int j = 0; j < WORLD_FOLK; j++)
    if (rm_eg_folk_hp(view(), k, j, hpv)) world_folk_hp(k, j, (int)hp[0], (int)hp[1], (int)hp[2]);
  int64_t cn = rm_eg_crimes(view(), k);
  for (int64_t q = 0; q < cn; q++) { rm_eg_crime(view(), k, q, crv); world_crime(k, (int)cr[0], cr[1], cr[2]); }
  rm_eg_crimes_heard(view(), k);
  for (int s = 0; s < WORLD_SEATS; s++) rm_eg_seat_town(view(), k, s, (coax_v_f32){seats + s * WORLD_SEAT_TOWN, WORLD_SEAT_TOWN});
  int64_t t0 = now_us();
  world_town(k, 1.0f / WORLD_HZ, (double)wall_ms() / 1000.0, seats, took);
  int64_t t1 = now_us(); part_us[PART_TOWN] += t1 - t0;
  for (int s = 0; s < WORLD_SEATS; s++) {
    if (seats[s * WORLD_SEAT_TOWN + 3] < 0.5f) continue;
    rm_eg_hurt(view(), k, s, (int64_t)took[s], (int64_t)took[WORLD_SEATS + s]);
    rm_eg_wanted(view(), k, s, world_wanted(k, s));
    rm_eg_cond(view(), k, s, world_cond(k, s));
  }
  /* the town's pieces: the rebuild, the strikes cast since, then what broke and caught for every page */
  if (rm_eg_rebuild(view(), k)) world_rebuild(k);
  float sk[9], ev[WORLD_EVENT]; coax_v_f32 skv = {sk, 9};
  int64_t sn = rm_eg_strikes(view(), k);
  for (int64_t q = 0; q < sn; q++) { rm_eg_strike(view(), k, q, skv); world_strike(k, (int)sk[0], (int)sk[1], sk + 2, sk + 5, (int)sk[8]); }
  rm_eg_strikes_heard(view(), k);
  int64_t t2 = now_us();
  int en = world_spells(k, 1.0f / WORLD_HZ);
  for (int q = 0; q < en; q++)
    if (world_event(k, q, ev)) rm_eg_piece(view(), k, (int64_t)ev[0], (int64_t)ev[1], ev[2], ev[3], ev[4], ev[5], ev[6]);
  int64_t t3 = now_us(); part_us[PART_SPELLS] += t3 - t2;
  int pn = world_people(k, people);
  rm_eg_people(view(), k, (coax_v_f32){people, WORLD_FOLK * WORLD_PEOPLE}, pn);
  for (int i = 0; i < WORLD_FOLK; i++) if (world_view(k, i, fv)) rm_eg_folk(view(), k, i, fvv);
  part_us[PART_PEOPLE] += now_us() - t3;
}
// Room k's hotbar abilities the world runs (room-abilities.coax, everglade-room-abilities.coax; #12718):
// the casts and ends since into it, its step, then what it holds up and the falls' damage back.
static void everglade_abilities(int k) {
  float c[WORLD_CAST], l[WORLD_LIFE] = {0}, h[3]; coax_v_f32 cv = {c, WORLD_CAST};
  int64_t n = rm_ra_casts(view(), k);
  for (int64_t q = 0; q < n; q++) { rm_ra_cast(view(), k, q, cv); world_ability(k, c); }
  rm_ra_heard(view(), k);
  world_abilities(k, 1.0f / WORLD_HZ);
  /* every seat's abilities up, each its own entity */
  for (int s = 0; s < WORLD_SEATS; s++)
    for (int ab = 0; ab < WORLD_ABS; ab++) {
      bool on = world_life(k, s, ab, l);
      rm_ra_live(view(), k, s, ab, on, l[1], l[2], l[3], l[4], (int64_t)l[5]);
    }
  for (int i = 0; i < WORLD_FOLK; i++) rm_ra_air(view(), k, i, world_air(k, i));
  int hn = world_hurts(k);
  for (int q = 0; q < hn; q++) if (world_hurt(k, q, h)) rm_ra_hurt(view(), k, (int64_t)h[0], (int64_t)h[1], (int64_t)h[2]);
}
static void everglade_tick(void) {
  int64_t in[WORLD_IN]; float pose[WORLD_POSE];
  coax_v_s64 inv = {in, WORLD_IN};
  pump_held = true;
  for (int k = 0; k < WORLD_ROOMS; k++) {
    if (!rm_eg_begin(view(), k)) continue;
    int64_t tm = now_us();
    for (int s = 0; s < WORLD_SEATS; s++) {
      if (rm_eg_unplaced(view(), k, s) &&
          (rm_eg_back(view(), k, s, 0) > 0 ? world_place_at(k, s, rm_eg_back(view(), k, s, 1), rm_eg_back(view(), k, s, 2), rm_eg_back(view(), k, s, 3), rm_eg_back(view(), k, s, 4), pose)
                                            : world_place(k, s, pose)))
        rm_eg_moved(view(), k, s, 0, pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
      while (rm_eg_input(view(), k, s, inv)) {
        if (!world_step(k, s, in, pose)) break;
        rm_eg_moved(view(), k, s, in[0], pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
      }
    }
    /* the summoned creatures stand on the ground (a new one is heard once it does) */
    float xz[2]; coax_v_f32 xzv = {xz, 2};
    for (int q = 0; q < rm_eg_beasts(); q++)
      if (rm_eg_beast_at(view(), k, q, xzv)) rm_eg_beast_ground(view(), k, q, world_ground(k, xz[0], xz[1]));
    part_us[PART_MOVE] += now_us() - tm;
    int64_t ta = now_us();
    everglade_abilities(k);
    part_us[PART_ABILITIES] += now_us() - ta;
    everglade_town(k);
    int64_t te = now_us();
    rm_eg_end(view(), k);
    part_us[PART_END] += now_us() - te;
    int64_t tf = now_us();
    relay_flush();
    part_us[PART_SEND] += now_us() - tf;
  }
  int64_t tp = now_us();
  pump_all();
  part_us[PART_SEND] += now_us() - tp;
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
  /* a shard's own health through the load balancer: GET /api/room/sN/health (room-directory.coax) */
  bool shard_health = false;
  if (!strncmp((char *)c->in, "GET /api/room/s", 15)) {
    const char *p = (const char *)c->in + 15; int digits = 0;
    while (*p >= '0' && *p <= '9' && digits < 3) { p++; digits++; }
    shard_health = digits > 0 && !strncmp(p, "/health ", 8);
  }
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
  /* --dev: the API's stand-in (local.sh): the world document the room laid, byte for byte */
  if (dev_dir && !strncmp((char *)c->in, "GET /api/playtest/world/everglade-5e", 36) && (c->in[36] == ' ' || c->in[36] == '?')) {
    http(i, "200 OK", "application/json", dev_world ? dev_world : "{}"); return;
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
static int poster_fd = -1, answer_fd = -1;
static void poster_run(int in, int back) {
  static char cfg[16384], got[16384];
  FILE *f = fdopen(in, "r");
  for (;;) {
    long n; int kind = f ? fgetc(f) : EOF;
    if (kind == EOF || fscanf(f, "%ld", &n) != 1 || fgetc(f) != '\n' || n <= 0 || n >= (long)sizeof cfg) _exit(0);
    if (fread(cfg, 1, (size_t)n, f) != (size_t)n) _exit(0);
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
  int p[2], b[2];
  if (pipe(p)) { perror("room: poster pipe"); return; }
  if (pipe(b)) { perror("room: poster pipe"); close(p[0]); close(p[1]); return; }
  pid_t pid = fork();
  if (pid == 0) { close(p[1]); close(b[0]); poster_run(p[0], b[1]); _exit(0); }
  close(p[0]); close(b[1]);
  if (pid < 0) { perror("room: poster"); close(p[1]); close(b[0]); return; }
  poster_fd = p[1]; answer_fd = b[0];
  fcntl(answer_fd, F_SETFL, O_NONBLOCK);
}
static void poster_send(char kind, const char *cfg, size_t k) {
  static char msg[16384 + 32];
  if (poster_fd < 0 || k + 32 > sizeof msg) return;
  int hn = snprintf(msg, sizeof msg, "%c%zu\n", kind, k);
  memcpy(msg + hn, cfg, k);
  if (write(poster_fd, msg, (size_t)hn + k) != (ssize_t)((size_t)hn + k)) fprintf(stderr, "room: poster: request dropped\n");
}
/* The control ask, every two seconds while the server has a key, and its answers as they come. */
static void control_ask(void) {
  if (!rm_control_due(view())) return;
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
    free(dev_world); dev_world = dev_read("world-everglade-5e.json", &dev_world_n);
    if (!dev_world || dev_world[0] != '{') { fprintf(stderr, "room: --dev %s has no world-everglade-5e.json\n", dev_dir); exit(1); }
    world_edits(dev_world, (int64_t)dev_world_n);
    printf("room: the local world (%zu bytes) laid over the rooms\n", dev_world_n);
    return;
  }
  char ub[512];
  const char *url = api_url("OA_ROOM_WORLD_URL", "/api/playtest/world/everglade-5e", ub, sizeof ub);
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
    if (len > 0 && buf[head] == '{') { world_edits(buf + head, (int64_t)len); printf("room: the realm's world (%zu bytes) laid over the rooms\n", len); }
    else {
      rm_control(view(), (coax_string){buf + head, (int64_t)len});
      if (server->ctl.edit_known && server->ctl.edit != edit_asked) { edit_asked = server->ctl.edit; world_ask(); }
    }
    memmove(buf, buf + head + len, have - head - len); have -= head + len;
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0 && rm_relay_kicked(view(), i)) conns[i].doomed = true;
  }
  if (have == sizeof buf) have = 0;
}
/* A signed report to the API (vitals, the directory) through the poster: a curl config with the body. */
static void post_signed(const char *url, const uint8_t *mac, const uint8_t *body, int64_t n) {
  static char cfg[16384];
  if (poster_fd < 0) return;
  size_t k = (size_t)snprintf(cfg, sizeof cfg, "url = \"%s\"\nheader = \"X-Room-Mac: %.64s\"\nheader = \"Content-Type: text/plain\"\nsilent\nshow-error\nmax-time = 10\noutput = \"/dev/null\"\ndata-binary = \"",
                              url, (const char *)mac);
  for (int64_t i = 0; i < n && k + 4 < sizeof cfg; i++) {
    char ch = (char)body[i];
    if (ch == '\n') { cfg[k++] = '\\'; cfg[k++] = 'n'; } else cfg[k++] = ch;
  }
  if (k + 4 >= sizeof cfg) { fprintf(stderr, "room: a report too long to post\n"); return; }
  k += (size_t)snprintf(cfg + k, sizeof cfg - k, "\"\n");
  poster_send('P', cfg, k);
}
static const char *api_url(const char *env, const char *path, char *buf, size_t cap) {
  const char *u = getenv(env);
  if (u && *u) return u;
  const char *base = getenv("OA_ROOM_API");   /* the API's origin (default production's) */
  snprintf(buf, cap, "%s%s", base && *base ? base : "https://openagents.com", path);
  return buf;
}
/* Players' HP between visits (room-vitals.coax, #12715). */
static void vitals_post(void) {
  int64_t n = rm_vitals_due(view());
  if (n <= 0) return;
  char buf[512];
  post_signed(api_url("OA_ROOM_VITALS_URL", "/api/playtest/room-vitals", buf, sizeof buf), server->vit.mac.e, server->vit.out.e, n);
}
/* This shard's everglade rooms and who sits in them, for the API's instance directory
   (room-directory.coax, coax/web/api/rooms.c, #12717). */
static void directory_post(void) {
  int64_t n = rm_directory_due(view());
  if (n <= 0) return;
  char buf[512];
  post_signed(api_url("OA_ROOM_DIRECTORY_URL", "/api/playtest/room-directory", buf, sizeof buf), server->dir.mac.e, server->dir.out.e, n);
}

int main(int argc, char **argv) {
  const char *address = "127.0.0.1"; int port = 8830;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) address = argv[++i];
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = atoi(argv[++i]);
    else if (!strcmp(argv[i], "--dev") && i + 1 < argc) dev_dir = argv[++i];
  }
  const char *env = getenv("PORT");
  if (env && *env) { port = atoi(env); address = "0.0.0.0"; }
  signal(SIGPIPE, SIG_IGN);
  signal(SIGCHLD, SIG_IGN);   /* vitals_post's curls reap themselves */
  setvbuf(stdout, NULL, _IOLBF, 0);
  poster_start();   /* before the server's memory (vitals_post) */
  server = calloc(1, sizeof *server);
  if (!server) { fprintf(stderr, "room: out of memory\n"); return 1; }
  rm_init(view());
  world_ask();   /* the realm's published world, for every room's plan (room-world.c world_edits) */
  /* the seat-ticket key (Secret Manager oa-room-ticket-key): set, an everglade join needs a ticket (room-ticket.coax) */
  const char *tkey = getenv("OA_ROOM_TICKET_KEY");
  const char *must = getenv("OA_ROOM_TICKET_REQUIRED");   /* "0": a join without a ticket sits with no id (while pages roll out) */
  bool required = !(must && !strcmp(must, "0"));
  /* which shard of the instance directory this is (oa-room 1, oa-room-K K: room-directory.coax) */
  const char *shard = getenv("OA_ROOM_SHARD");
  if (shard && *shard) rm_shard(view(), atoi(shard));
  printf("room: shard %lld\n", (long long)server->shard);
  if (tkey && strlen(tkey) >= 32) { rm_ticket_key(view(), (coax_string){tkey, (int64_t)strlen(tkey)}, required); printf("room: seat tickets checked%s\n", required ? ", required" : ""); }
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
  int64_t ticks = 0, last_report = now_ms(), last_vitals = now_ms();
  for (;;) {
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
          if (slot < 0) { close(fd); continue; }
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
      if (ticks % 2 == 0) { everglade_tick(); tick_add(now_us() - t_tick); }
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
      if ((double)now - next > 250.0) {   // a stall: start the beat again, and owe every seat the everglade ticks it skipped
        rm_eg_owe(view(), (int64_t)(((double)now - next) / tick / 2.0));
        next = (double)now + tick;
      }
      if (now - last_vitals >= 1000) { last_vitals = now; vitals_post(); directory_post(); }
      control_ask(); control_hear();
      if (now - last_report >= 60000) { printf("room: %lld ticks in %.1f s, %lld players, %lld frames over the rate limits, %lld queue full, %lld slow sockets closed, %lld bytes the most waiting\n", (long long)ticks, (now - last_report) / 1000.0, (long long)rm_players(view()), (long long)rm_relay_dropped(view()), (long long)rm_relay_queue_full(view()), (long long)slow_closed, (long long)out_peak);
        printf("room: everglade tick parts over the minute (ms):"); for (int q = 0; q < PARTS; q++) { printf(" %s %lld", part_name[q], (long long)(part_us[q] / 1000)); part_us[q] = 0; }
        printf(", %lld events with no room in their tick\n", (long long)rm_eg_dropped(view()));
        ticks = 0; last_report = now; memcpy(fan_last, fan_now, sizeof fan_now); memset(fan_now, 0, sizeof fan_now); memcpy(tick_last, tick_now, sizeof tick_now); memset(tick_now, 0, sizeof tick_now); }
    }
  }
}
