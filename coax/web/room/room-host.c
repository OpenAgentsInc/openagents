/* The room server's host: TCP, the WebSocket
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
#ifdef COAX_FAST_EDIT_ENABLED
#include "fast-edit-native.h"
#endif
#include "room-world.h"
#include "room-crash.h"   /* crash capture: the handlers, the dump, the last crash at the next start (#13298) */
#include "ws.h"
#include "wt.h"
#include <arpa/inet.h>
#include <curl/curl.h>   /* the posters' HTTPS client, one kept handle a lane (#13300) */
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
#include <sys/stat.h>
#include <sys/resource.h>   /* getrusage: the resident memory in the vitals (room-stats.h) */
#include <sys/random.h>   /* getentropy: the dice key (room-tick.c dice_key_in) */
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
  // a WebTransport session's (room-wt.c, #13176): its id + 1 (0 a WebSocket), whether it already ended,
  // its streams by channel (id + 1), the page's stream (id + 1), datagrams sent, and state dropped stale
  int wt; bool wt_gone; int64_t wt_stream[5]; int64_t wt_in, wt_dg, wt_stale;
  uint8_t wt_again[4][256]; uint16_t wt_again_n[4]; int wt_agains;   /* hastened frames to send once more as datagrams next tick (room-wt.c) */
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
/* --dev: the stand-in's checkpoint store (room-dev.coax rm_dev_store, #13057), kept in a file so a local
   room killed and started again finds its players' places: OA_ROOM_DEV_STORE, else
   ~/.openagents/room-dev/checkpoints-PORT.txt (the room's port, so local rooms keep their own). */
static char dev_store_path[1024];
static void dev_store_read(void) {
  FILE *f = dev_store_path[0] ? fopen(dev_store_path, "rb") : NULL;
  if (!f) return;
  static char text[1 << 16]; size_t k = fread(text, 1, sizeof text, f); fclose(f);
  rm_dev_restore(view(), (coax_string){text, (int64_t)k});
}
static void dev_store_save(void) {
  if (!dev_store_path[0] || !rm_dev_dirty(view())) return;
  dev_store_read();   /* what another local room wrote since, merged first */
  int64_t n = rm_dev_store(view());
  char tmp[1100]; snprintf(tmp, sizeof tmp, "%s.new", dev_store_path);
  FILE *f = fopen(tmp, "wb");
  bool ok = f && fwrite(server->dev.store.e, 1, (size_t)n, f) == (size_t)n;
  if (f) { ok = fflush(f) == 0 && ok; ok = fsync(fileno(f)) == 0 && ok; ok = fclose(f) == 0 && ok; }
  if (!ok || rename(tmp, dev_store_path) != 0) { unlink(tmp); fprintf(stderr, "room: the local checkpoint store %s was not saved\n", dev_store_path); }
}
static void dev_store_open(int port) {
  const char *p = getenv("OA_ROOM_DEV_STORE"), *home = getenv("HOME");
  if (p && *p) snprintf(dev_store_path, sizeof dev_store_path, "%s", p);
  else if (home && *home) {
    char dir[900]; snprintf(dir, sizeof dir, "%s/.openagents", home); mkdir(dir, 0700);
    snprintf(dir, sizeof dir, "%s/.openagents/room-dev", home); mkdir(dir, 0700);
    snprintf(dev_store_path, sizeof dev_store_path, "%s/checkpoints-%d.txt", dir, port);
  } else return;
  dev_store_read();
  printf("room: the local checkpoint store %s: %lld players\n", dev_store_path, (long long)server->dev.ck_n);
}
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
    if(c->ws)st_kernel_bytes+=(int64_t)w;
    done += (size_t)w;
  }
  if (done) { memmove(c->out, c->out + done, c->on - done); c->on -= done; }
  if (!c->on) { c->since = 0; if (st_c[i].burst == 2) st_drained(i, now_us()); }
  return true;
}
// n bytes onto i's output, sent as far as the socket takes them now. Past OUT_MAX, or on a failed
// socket, i is doomed (closed by reap, after the frame being sent reaches everyone else).
static bool put_out(int i, const void *p, size_t n) {
  conn *c = &conns[i];
  if (c->fd < 0 || c->doomed) return false;
  if (c->on + n > OUT_MAX) { c->doomed = true; slow_closed++; return false; }
  if (c->on + n > c->ocap) {
    size_t cap = c->ocap ? c->ocap : 8192;
    while (cap < c->on + n) cap *= 2;
    uint8_t *grown = realloc(c->out, cap);
    if (!grown) { c->doomed = true; return false; }
    c->out = grown; c->ocap = cap;
  }
  if (!c->on) c->since = now_ms();
  st_put(i, n);
  memcpy(c->out + c->on, p, n); c->on += n;
  if ((int64_t)c->on > out_peak) out_peak = (int64_t)c->on;
  if (!pump_held && !pump(i)) c->doomed = true;
  return true;
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
static void wt_out(int i, const uint8_t *text, size_t n);   /* room-wt.c */
// Optional observation bytes use the same bounded socket queue and byte
// observer. This reports encoding/queue acceptance, never delivery.
static void latency_out(int i,const uint8_t *text,int64_t n) {
  if(n<=0)return;
  if(conns[i].wt){wt_out(i,text,(size_t)n);return;}
  uint8_t out[300];size_t k=ws_build(out,sizeof out,WS_TEXT,text,(size_t)n,false);
  if(k&&put_out(i,out,k))st_frame_out(text,(size_t)n,1,k);
  else if(k)fo_refused((coax_v_t_FrameObserved){server->st.frames.e,1},(int64_t)k);
}
// The relay's frames (room-relay.coax), each onto the output of the connections it names.
static void relay_flush(void) {
  int64_t len;
  static uint8_t out[4300];
  while ((len = rm_relay_next(view())) > 0) {
    size_t k = ws_build(out, sizeof out, WS_TEXT, server->rout.e, (size_t)len, false);
    if (!k) continue;
    int to = 0, last = -1;
    for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && conns[j].ws && rm_relay_to(view(), j)) { if(conns[j].wt){wt_out(j,server->rout.e,(size_t)len);to++;last=j;continue;} if(put_out(j,out,k)){to++;last=j;}else{fo_refused((coax_v_t_FrameObserved){server->st.frames.e,1},(int64_t)k);} }
    /* counted by type (room-stats.h); a D frame (one seat's) with no more parts ends a joiner's first snapshot */
    int64_t t0 = now_us();
    int f = st_frame_out(server->rout.e, (size_t)len, to, k);
    if(f>=0&&!(f&2)&&to==1){int64_t at=rm_st_sent_ack(view(),last,(coax_string){(const char *)server->rout.e,len});if(at>=0)st_queued_ack_add(last,now_ms()-at);}
    if (f >= 0 && !(f & 2) && to == 1 && st_c[last].burst == 1) { st_c[last].burst = 2; if (!conns[last].on) st_drained(last, now_us()); }
    if(to==1){uint8_t trace[256];int64_t n=rm_lt_sent(view(),last,(coax_string){(const char *)server->rout.e,len},now_us(),(coax_v_u8){trace,256});latency_out(last,trace,n);}
    st_cost_us += now_us() - t0;
  }
}
#define ROOM_LATENCY_STEP(k,s,seq,began,ended) rm_lt_step(view(),k, (coax_t_SeatId){s},seq,began,ended)
#define ROOM_LATENCY_ACTIVE(k,s) rm_lt_active(view(),k, (coax_t_SeatId){s})
#define ROOM_LATENCY_SERIALIZED(k,simulation,began,ended) rm_lt_serialized(view(),k,simulation,began,ended)
#include "room-tick.c"   /* the tick both hosts run (room-worker.c too) */
static bool rooms_warmed;
static int64_t rooms_warm_us;
static void warm_rooms(void) {
  if(rooms_warmed)return;
  int64_t preparing=now_us();
  rooms_warmed=world_ready();
  rooms_warm_us=now_us()-preparing;
}
#include "room-wt.c"   /* WebTransport sessions as conns (#13176) */
/* Seeded faults for the soak's own check (#13296, soak.coax self-check), in a --dev room only:
   OA_ROOM_FAULT_TICK_US busies every tick that long; OA_ROOM_FAULT_LEAK_KB takes and touches that
   many KB each second and never gives them back. A room that is not --dev reads neither. */
static void fault_step(void) {
  static int ready, tick_us, leak_kb; static int64_t leaked_at;
#ifdef ROOM_SEEDED_TICK_US
  /* A seeded bad build (compile.sh OA_ROOM_SEED_TICK_US, vm-deploy.sh deploy --staging --seed-tick-us,
     #13299): every tick this much longer, so a release's canary has a bad room to stop at. Its
     release is named "-seeded", which no production shard takes. */
  { int64_t until = now_us() + ROOM_SEEDED_TICK_US; while (now_us() < until) { } }
#endif
  if (!dev_dir) return;
  if (!ready) {
    const char *t = getenv("OA_ROOM_FAULT_TICK_US"), *l = getenv("OA_ROOM_FAULT_LEAK_KB");
    tick_us = t ? atoi(t) : 0; leak_kb = l ? atoi(l) : 0; ready = 1; leaked_at = now_ms();
    if (tick_us > 0 || leak_kb > 0) printf("room: --dev seeded faults: %d us a tick, %d KB a second leaked\n", tick_us, leak_kb);
  }
  if (tick_us > 0) { int64_t until = now_us() + tick_us; while (now_us() < until) { } }
  if (leak_kb > 0 && now_ms() - leaked_at >= 1000) {
    leaked_at = now_ms();
    static char *volatile kept;   /* read by nothing, so the compiler keeps the block */
    char *p = malloc((size_t)leak_kb * 1024);
    if (p) { memset(p, 1, (size_t)leak_kb * 1024); kept = p; }
    (void)kept;
  }
}
static void everglade_tick(void) {
  int64_t was[PARTS]; memcpy(was, part_us, sizeof was);
  pump_held = true;
  for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && conns[j].ws) rm_conn_waiting(view(), j, (int64_t)conns[j].on);
  wt_tick_begin();   /* a WebTransport session's state of the last tick, still waiting, is stale */
  for (int k = 0; k < WORLD_ROOMS; k++) {
    if (!everglade_room_tick(k)) continue;
    int64_t tf = now_us();
    relay_flush();
    part_us[PART_SEND] += now_us() - tf;
    /* each seat's input that this tick's D acknowledges: heard to sent, plus its socket's round trip */
    int64_t tk = now_us(), ms = now_ms();
    for (int s = 0; s < WORLD_SEATS; s++) {
      int64_t at = rm_st_ack(view(), k, (coax_t_SeatId){ s});
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
  if(was)st_discarded_bytes+=(int64_t)conns[i].on;
  wt_forget(i);
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
/* "METHOD /api/room/sN/REST" for a shard N (1 to 3 digits); rest begins with '/' and ends with ' '. */
static bool is_shard_path(const char *in, const char *method, const char *rest) {
  size_t m = strlen(method);
  if (strncmp(in, method, m) || strncmp(in + m, "/api/room/s", 11)) return false;
  const char *p = in + m + 11; int digits = 0;
  while (*p >= '0' && *p <= '9' && digits < 3) { p++; digits++; }
  return digits > 0 && !strncmp(p, rest, strlen(rest));
}
/* A request header's value (name with its colon, any case), trimmed, into out. */
static bool header_of(const uint8_t *in, const uint8_t *end, const char *name, char *out, size_t cap) {
  size_t nn = strlen(name);
  for (const uint8_t *p = in; p + nn < end; p++) {
    if (p != in && p[-1] != '\n') continue;
    if (strncasecmp((const char *)p, name, nn)) continue;
    const uint8_t *v = p + nn; while (v < end && *v == ' ') v++;
    size_t k = 0; while (v < end && *v != '\r' && *v != '\n' && k + 1 < cap) out[k++] = (char)*v++;
    out[k] = 0;
    return k > 0;
  }
  return false;
}
/* The drain (#12797): asked by the deploy, then each loop: WAITING for the world events under way (up
   to the ask's wait), TELLING every page "restarting" (rm_drain_tell; their sockets close, their seats
   end), SAVING every seat's vitals until the API has them all (DRAIN_SAVE_MS at most), DONE. The deploy
   reads the phase in /health ("drain done") before it replaces the shard. */
enum { DRAIN_NONE, DRAIN_WAITING, DRAIN_TELLING, DRAIN_SAVING, DRAIN_DONE, DRAIN_FAILED };
#define DRAIN_SAVE_MS 20000
static int drain_phase; static int64_t drain_asked, drain_wait_ms, drain_step_at, drain_told;
static bool started_ok;
static const char *drain_word(void) {
  static const char *w[] = {"drain none\n", "drain waiting\n", "drain telling\n", "drain saving\n", "drain done\n", "drain failed\n"};
  return w[drain_phase];
}
static void drain_begin(int64_t wait_ms) {
  if (drain_phase != DRAIN_NONE) return;
  drain_phase = DRAIN_WAITING; drain_asked = now_ms(); drain_wait_ms = wait_ms;
  printf("room: drain asked: no new seats (up to %lld s)\n", (long long)(wait_ms / 1000));
}
static void drain_step(void) {
  int64_t now = now_ms();
  /* no world event to wait for: v1's raid left with v1 (#13346) */
  if (drain_phase == DRAIN_WAITING) {
    drain_told = rm_drain_tell(view());
    relay_flush();   /* the word goes out before the main loop closes the kicked sockets */
    drain_phase = DRAIN_TELLING; drain_step_at = now;
    printf("room: drain: told %lld pages restarting after %lld ms\n", (long long)drain_told, (long long)(now - drain_asked));
  } else if (drain_phase == DRAIN_TELLING && now - drain_step_at >= 500) {
    /* their sockets closed, their seats ended: every seat's vitals noted and sent */
    rm_vitals_flush(view()); rm_mod_flush(view());
    drain_phase = DRAIN_SAVING; drain_step_at = now;
  } else if (drain_phase == DRAIN_SAVING) {
    int64_t left = rm_vitals_pending(view()) + rm_mod_pending(view());   /* and moderators' actions with no receipt (#13302) */
    if ((left == 0 && !server->vit.fly && rm_checkpoint_clean(view())) || now - drain_step_at >= DRAIN_SAVE_MS) {
      drain_phase = left == 0 && !server->vit.fly && rm_checkpoint_clean(view()) ? DRAIN_DONE : DRAIN_FAILED;
      printf("room: %s after %lld ms: %lld legacy saves, %lld checkpoints with no receipt\n",drain_phase==DRAIN_DONE?"drain done":"drain failed", (long long)(now - drain_asked), (long long)left,(long long)rm_checkpoint_pending(view()));
    }
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
  /* a drain before a deploy (#12797, room-directory.coax rm_drain_ask): POST /drain or a shard's
     POST /api/room/sN/drain, signed (X-Room-At, X-Room-Wait, X-Room-Mac) */
  if (is_shard_path((char *)c->in, "POST ", "/drain ") || !strncmp((char *)c->in, "POST /drain ", 12)) {
    char at[24], wait[8], mac[72];
    bool have = header_of(c->in, end, "X-Room-At:", at, sizeof at) && header_of(c->in, end, "X-Room-Mac:", mac, sizeof mac);
    if (!header_of(c->in, end, "X-Room-Wait:", wait, sizeof wait)) snprintf(wait, sizeof wait, "0");
    if (!have || !rm_drain_ask(view(), strtoll(at, NULL, 10), strtoll(wait, NULL, 10), (coax_string){mac, (int64_t)strlen(mac)})) {
      http(i, "403 Forbidden", "text/plain", "not a drain\n"); return;
    }
    drain_begin(strtoll(wait, NULL, 10) * 1000);
    http(i, "200 OK", "text/plain", drain_word()); return;
  }
#ifdef COAX_FAST_EDIT_ENABLED
  if (dev_dir && !strncmp((char *)c->in,"GET /api/playtest/dev/fast-edit ",sizeof("GET /api/playtest/dev/fast-edit ")-1)) {
    char status[1024];coax_fast_status(status,sizeof status);http(i,"200 OK","application/json",status);return;
  }
#endif
  /* readiness (#12797): 200 once the start check held, the realm's world is laid, and a room's world
     builds (room-world.c world_ready); the deploy's startup probe, so a revision takes no traffic before */
  if (!strncmp((char *)c->in, "GET /ready ", 11) || is_shard_path((char *)c->in, "GET ", "/ready ")) {
    bool prepared=started_ok&&rooms_warmed;
    if (prepared) http(i, "200 OK", "text/plain", "ready\n");
    else http(i, "503 Service Unavailable", "text/plain", started_ok ? "no world yet\n" : "start check failed\n");
    return;
  }
  /* --dev: the API's stand-in answers the page's /api/playtest/ requests (room-dev.coax), the body whole */
  if (dev_dir && (!strncmp((char *)c->in, "POST /api/", 10) || !strncmp((char *)c->in, "GET /api/playtest/", 18) || !strncmp((char *)c->in, "GET /api/auth/viewer", 20))) {
    size_t want = 0;
    for (uint8_t *p = c->in; p + 15 < end; p++)
      if ((p == c->in || p[-1] == '\n') && !strncasecmp((char *)p, "Content-Length:", 15)) want = (size_t)strtoul((char *)p + 15, NULL, 10);
    if (len + want > c->n && len + want < IN_CAP) return;   /* the rest of the body is still coming */
    coax_string whole = {(const char *)c->in, (int64_t)(len + want <= c->n ? len + want : c->n)};
    if (!strncmp((char *)c->in, "POST /api/playtest/room-ticket/", 31)) dev_store_read();   /* another shard's captures (a drain's) */
    int64_t an = rm_dev_api(view(), whole);
    dev_store_save();   /* a v4 ticket's owner generation, before its answer goes */
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
    /* the socket with the most output waiting now, and the oldest wait (a slow link's, #12903) */
    int64_t wait_most = 0, wait_ms = 0, t_now = now_ms();
    for (int j = 0; j < CONNS; j++) {
      if (conns[j].fd < 0 || !conns[j].on) continue;
      if ((int64_t)conns[j].on > wait_most) wait_most = (int64_t)conns[j].on;
      if (t_now - conns[j].since > wait_ms) wait_ms = t_now - conns[j].since;
    }
    char body[1024]; snprintf(body, sizeof body, "ok %lld players, fanout p50 %lld us p95 %lld us over %lld reads, %lld rate-limited, %lld slow closed, shard %lld, everglade tick p50 %lld us at the 95th %lld us over %lld ticks, waiting %lld bytes most, %lld ms oldest, %s",
                             (long long)rm_players(view()), (long long)p50, (long long)p95, (long long)n, (long long)rm_relay_dropped(view()), (long long)slow_closed,
                             (long long)server->shard, (long long)tp50, (long long)tp95, (long long)tn, (long long)wait_most, (long long)wait_ms, drain_word());
    snprintf(body+strlen(body),sizeof body-strlen(body),"checkpoints pending %lld, flying %d, refused %lld, applied %lld, duplicates %lld\n",(long long)rm_checkpoint_pending(view()),server->ck.fly?1:0,(long long)server->ck.refused,(long long)server->ck.applied,(long long)server->ck.duplicates);
    int pending=0;for(int lane=0;lane<7;lane++)if(server->bg.e[0].slot.e[lane].id>0)pending++;
    snprintf(body+strlen(body),sizeof body-strlen(body),"background pending %d/7, offered %lld, refused %lld, expired %lld, late %lld, failed %lld\n",pending,(long long)server->bg.e[0].offered,(long long)server->bg.e[0].refused,(long long)server->bg.e[0].expired,(long long)server->bg.e[0].late,(long long)server->bg.e[0].failed);
    int64_t work[8];rm_work_stats(view(),(coax_v_s64){work,8});
    snprintf(body+strlen(body),sizeof body-strlen(body),"work process rooms %lld, lowest credit %lld us, largest last tick %lld us room %lld, deferred ticks %lld, refused new work %lld, retired rooms %lld\n",(long long)work[0],(long long)work[1],(long long)work[2],(long long)work[7],(long long)work[3],(long long)work[4],(long long)work[5]);
    int64_t warm=0;int warm_room=-1;for(int k=0;k<WORLD_ROOMS;k++)if(room_prepare_us[k]>warm){warm=room_prepare_us[k];warm_room=k;}
    snprintf(body+strlen(body),sizeof body-strlen(body),"world preparation max %lld us room %d, startup preparation %lld us, last publication %lld us\n",(long long)warm,warm_room,(long long)rooms_warm_us,(long long)world_publication_us);
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
  if (!hn) { http(i, "400 Bad Request", "text/plain", "Rooms: open a WebSocket to a game's room path (room-games.coax), or to /api/room/relay/GAME/CODE.\n"); return; }
  if(!rm_work_request_ready(req,rooms_warmed)){http(i,"503 Service Unavailable","text/plain","The world is getting ready.\n");return;}
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
      st_frame_in(f.payload, f.length,f.consumed);
      uint8_t trace[256];coax_string text={(const char *)f.payload,(int64_t)f.length};
      int64_t received=now_us();int64_t observed=rm_lt_request(view(),i,text,received,now_us(),(coax_v_u8){trace,256});
      if(observed>=0)latency_out(i,trace,observed);
      else{int64_t tail=rm_lt_tail(view(),i);rm_heard(view(),i,text);rm_lt_received(view(),i,tail,received);}
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

/* Players' HP between visits (room-vitals.coax, #12715): what rm_vitals_due wrote goes to the API
   over HTTPS from a small poster process a lane, forked before the server's memory is taken (a fork
   of the full server can fail on Cloud Run). Each poster keeps one libcurl handle for its life, so a
   lane's posts reuse its connection to the API: no process, DNS lookup or TLS handshake a post
   (#13300; before, each post forked and ran curl). A request is still written as curl's config lines
   (url, header, data-binary, max-time, fail), so the MAC is in no argument list and the datagram's
   shape is as it was. The main loop offers an atomic local datagram and never waits. */
/* The eight lanes mirror background-budget.coax. Each has one serial SDK
   helper, one Coax-owned request identity and an atomic, bounded nonblocking
   datagram transport. A full SDK queue cannot stop the simulation, a control
   outage cannot queue behind saves, and no response prefix is published. */
enum { POST_CONTROL, POST_WORLD, POST_VITALS, POST_CHECKPOINT, POST_DIRECTORY, POST_STATS, POST_MOD, POST_ITEMS, POST_LANES };
#define POST_CAP 16640
static int poster_fd[POST_LANES] = {-1,-1,-1,-1,-1,-1,-1,-1};
static coax_v_t_BgBudget background(void) { return (coax_v_t_BgBudget){server->bg.e,1}; }
typedef struct { char *p; size_t n, cap; bool over; } post_body;
static size_t post_take(char *data, size_t size, size_t count, void *u) {
  post_body *b = u; size_t k = size * count;
  if (b->n + k > b->cap) { b->over = true; return 0; }   /* an answer too long is no answer, as it was */
  memcpy(b->p + b->n, data, k); b->n += k;
  return k;
}
/* One value of a curl config line (after `key = `), in place: quoted, with curl's escapes. */
static char *post_value(char *v, char **next) {
  char *o = v, *start = v;
  if (*v == '"') {
    v++; start = o;
    while (*v && *v != '"') {
      if (*v == '\\' && v[1]) { v++; *o++ = *v == 'n' ? '\n' : *v == 't' ? '\t' : *v == 'r' ? '\r' : *v; v++; }
      else *o++ = *v++;
    }
    if (*v == '"') v++;
  } else { while (*v && *v != '\n') *o++ = *v++; }
  while (*v && *v != '\n') v++;
  *next = *v ? v + 1 : v;
  *o = 0;
  return start;
}
/* The request in cfg, sent on h (kept between calls: its connection is reused). The HTTP status
   (0: no answer) and the answer's body in got; a `fail` request's refusal has no body, as curl's. */
static int post_do(CURL *h, char *cfg, char *got, size_t cap, size_t *gn) {
  struct curl_slist *headers = NULL;
  char *url = NULL, *data = NULL; long most = 10; bool fail = false, has_data = false;
  for (char *line = cfg; *line; ) {
    char *eq = line; while (*eq && *eq != '\n' && *eq != ' ') eq++;
    size_t kn = (size_t)(eq - line);
    if (!strncmp(eq, " = ", 3)) {
      char *next, *v = post_value(eq + 3, &next);
      if (kn == 3 && !strncmp(line, "url", 3)) url = v;
      else if (kn == 6 && !strncmp(line, "header", 6)) headers = curl_slist_append(headers, v);
      else if (kn == 11 && !strncmp(line, "data-binary", 11)) { data = v; has_data = true; }
      else if (kn == 8 && !strncmp(line, "max-time", 8)) most = atol(v);
      line = next;
    } else {
      if (kn == 4 && !strncmp(line, "fail", 4)) fail = true;
      while (*line && *line != '\n') line++;
      if (*line) line++;
    }
  }
  post_body b = {got, 0, cap - 1, false};
  long status = 0;
  if (url) {
    curl_easy_reset(h);   /* the options go; the connection, DNS and TLS session stay */
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);
    if (has_data) { curl_easy_setopt(h, CURLOPT_POST, 1L); curl_easy_setopt(h, CURLOPT_POSTFIELDS, data); curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)strlen(data)); }
    curl_easy_setopt(h, CURLOPT_TIMEOUT, most > 0 ? most : 10L);
    curl_easy_setopt(h, CURLOPT_NOSIGNAL, 1L);
    curl_easy_setopt(h, CURLOPT_TCP_KEEPALIVE, 1L);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, post_take); curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
    if (curl_easy_perform(h) == CURLE_OK && !b.over) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    else b.n = 0;
  }
  curl_slist_free_all(headers);
  if (fail && status >= 400) b.n = 0;
  got[b.n] = 0; *gn = b.n;
  return (int)status;
}
static void poster_run(int fd, pid_t owner) {
  signal(SIGTERM,SIG_IGN); /* final saves continue during a process-group drain */
  signal(SIGCHLD,SIG_DFL);
  static char req[POST_CAP+1], got[16384], msg[POST_CAP];
  CURL *h = curl_easy_init();
  while (getppid()==owner) {
    struct pollfd ready={fd,POLLIN,0};
    if (poll(&ready,1,250)<=0) continue;
    ssize_t n=recv(fd,req,sizeof req-1,0); if(n<=0||n>=POST_CAP) continue;req[n]=0;
    char *nl=memchr(req,'\n',(size_t)n); long long id,batch,at;
    if(!nl||sscanf(req,"%lld %lld %lld",&id,&batch,&at)!=3)continue;
    size_t k=0;int status=0;
    if(h&&now_ms()-at<15000)status=post_do(h,nl+1,got,sizeof got,&k);
    int hn=snprintf(msg,sizeof msg,"%lld %lld %d %zu\n",id,batch,status,k);
    if(hn>0&&(size_t)hn+k<=sizeof msg){memcpy(msg+hn,got,k);(void)send(fd,msg,(size_t)hn+k,0);}
  }
  _exit(0);
}
static void poster_start(void) {
  pid_t owner=getpid();
  for(int lane=0;lane<POST_LANES;lane++) {
    int p[2];if(socketpair(AF_UNIX,SOCK_DGRAM,0,p)){perror("room: background socket");continue;}
    int cap=POST_CAP*2;
    setsockopt(p[0],SOL_SOCKET,SO_SNDBUF,&cap,sizeof cap);setsockopt(p[1],SOL_SOCKET,SO_SNDBUF,&cap,sizeof cap);
    fcntl(p[0],F_SETFL,O_NONBLOCK);fcntl(p[1],F_SETFL,O_NONBLOCK);
    pid_t child=fork();
    if(child==0){close(p[0]);for(int j=0;j<lane;j++)if(poster_fd[j]>=0)close(poster_fd[j]);poster_run(p[1],owner);}
    close(p[1]);
    if(child<0){perror("room: background helper");close(p[0]);continue;}
    poster_fd[lane]=p[0];
  }
}
static bool poster_send(int lane, const char *cfg, size_t k, int64_t batch) {
  static char msg[POST_CAP];
  if(lane<0||lane>=POST_LANES||poster_fd[lane]<0||k>16383)return false;
  int64_t at=now_ms(),id=bg_offer(background(),lane,at);if(id<=0)return false;
  int hn=snprintf(msg,sizeof msg,"%lld %lld %lld\n",(long long)id,(long long)batch,(long long)at);
  memcpy(msg+hn,cfg,k);
  if(send(poster_fd[lane],msg,(size_t)hn+k,0)!=(ssize_t)((size_t)hn+k)){
    (void)bg_finish(background(),lane,id,false,now_ms());return false;
  }
  return true;
}
/* Local control-file publication is an immediate platform event. In particular,
   two independent native hosts must not apply the same scripted hold up to two
   seconds apart merely because their API polling phases differ. */
static bool dev_control_changed(void) {
  static struct timespec seen = {-1, -1};
  static off_t size = -1;
  static ino_t inode;
  char path[1024];
  int n = snprintf(path, sizeof path, "%s/room-control.txt", dev_dir);
  struct stat st;
  if (n < 0 || (size_t)n >= sizeof path || stat(path, &st)) return false;
#ifdef __APPLE__
  struct timespec modified = {st.st_mtime, st.st_mtimensec};
#else
  struct timespec modified = st.st_mtim;
#endif
  bool changed = modified.tv_sec != seen.tv_sec || modified.tv_nsec != seen.tv_nsec || st.st_size != size || st.st_ino != inode;
  seen = modified; size = st.st_size; inode = st.st_ino;
  return changed;
}
/* Production asks every two seconds; --dev also observes new file publications. */
/* X-Room-Signer, "SHARD KID", when this shard signs with its own write key (#13260, room-write.coax);
   empty while it signs with the shared key (an API before #13260 ignores the header). */
static char write_signer[16], write_signer_line[48];   /* the line: 'header = "X-Room-Signer: ..."', or "" */
static void control_ask(void) {
  bool due = rm_control_due(view());
  if (dev_dir) {
    if (dev_control_changed() || due) dev_control();
    return;
  }
  if (!due) return;
  const char *url = getenv("OA_ROOM_CONTROL_URL");
  if (!url || !*url) url = "https://openagents.com/api/playtest/room-control";
  char cfg[512];
  int k = snprintf(cfg, sizeof cfg, "url = \"%s\"\nheader = \"X-Room-Mac: %.64s\"\n%sheader = \"X-Room-At: %.*s\"\nsilent\nshow-error\nfail\nmax-time = 5\n",
                   url, (const char *)server->ctl.mac.e, write_signer_line, (int)(server->ctl.ask_n - 2), (const char *)server->ctl.ask.e + 2);
  if(k>0&&k+48<(int)sizeof cfg){k+=snprintf(cfg+k,sizeof cfg-(size_t)k,"write-out = \"\\n#HTTP %%{http_code}\"\n");poster_send(POST_CONTROL,cfg,(size_t)k,0);}
}
static const char *api_url(const char *env, const char *path, char *buf, size_t cap);
/* The realm's published world, asked for at the start and again each time the control says it
   changed (its edit stamp): the answer is laid over every room (room-world.c world_edits, #12731).
   It is the API's, the same URL the page asks (OA_ROOM_API's origin); under --dev, DIR's document. */
static void world_ask(void) {
  if (dev_dir) {
    free(dev_world); dev_world = dev_read("world-everglade.json", &dev_world_n);
    if (!dev_world || dev_world[0] != '{') { fprintf(stderr, "room: --dev %s has no world-everglade.json\n", dev_dir); exit(1); }
    int64_t preparing=now_us();world_edits(dev_world, (int64_t)dev_world_n);world_publication_us=now_us()-preparing;warm_rooms();
    printf("room: the local world (%zu bytes) laid over the rooms\n", dev_world_n);
    return;
  }
  if(!server->bg.e[0].world_needed||!bg_ready(background(),POST_WORLD,now_ms()))return;
  char ub[512];
  const char *url = api_url("OA_ROOM_WORLD_URL", "/api/playtest/world/everglade", ub, sizeof ub);
  char cfg[512];
  int k=snprintf(cfg,sizeof cfg,"url = \"%s\"\nsilent\nshow-error\nfail\nmax-time = 10\nwrite-out = \"\\n#HTTP %%{http_code}\"\n",url);
  if(k>0&&k<(int)sizeof cfg)poster_send(POST_WORLD,cfg,(size_t)k,0);
}
/* A signed report's lane keeps telemetry separate from each durable outbox.
   Every complete HTTP result returns through poster_hear. */
static bool post_report(int lane, const char *url, const uint8_t *mac, const uint8_t *body, int64_t n, int64_t batch) {
  static char cfg[16384];
  size_t k=(size_t)snprintf(cfg,sizeof cfg,"url = \"%s\"\nheader = \"X-Room-Mac: %.64s\"\n%sheader = \"Content-Type: text/plain\"\nsilent\nshow-error\nmax-time = 5\nwrite-out = \"\\n#HTTP %%{http_code}\"\ndata-binary = \"",url,(const char *)mac,write_signer_line);
  if(k>=sizeof cfg)return false;
  for (int64_t i = 0; i < n && k + 4 < sizeof cfg; i++) {
    char ch = (char)body[i];
    if (ch == '\n') { cfg[k++] = '\\'; cfg[k++] = 'n'; } else { if(ch=='\"'||ch=='\\') cfg[k++]='\\'; cfg[k++]=ch; }
  }
  if (k + 4 >= sizeof cfg) { fprintf(stderr, "room: a report too long to post\n"); return false; }
  k += (size_t)snprintf(cfg + k, sizeof cfg - k, "\"\n");
  return poster_send(lane,cfg,k,batch);
}
static void post_signed(int lane,const char *url,const uint8_t *mac,const uint8_t *body,int64_t n){(void)post_report(lane,url,mac,body,n,0);}
static const char *api_url(const char *env, const char *path, char *buf, size_t cap) {
  const char *u = getenv(env);
  if (u && *u) return u;
  const char *base = getenv("OA_ROOM_API");   /* the API's origin (default production's) */
  snprintf(buf, cap, "%s%s", base && *base ? base : "https://openagents.com", path);
  return buf;
}
/* Players' HP, XP and WANTED between visits (room-vitals.coax, #12715, #12793, #12794): the outbox's
   next post, and its receipt when it comes. A refused SDK offer fails at once.
   --dev posts only when OA_ROOM_VITALS_URL names an API (the vitals check, coax/web/api/
   tests/room_checks.coax); otherwise the stand-in keeps the room's own notes (room-dev.coax). */
static void vitals_post(void) {
  int64_t n = rm_vitals_due(view());
  if (n <= 0) return;
  const char *dev_url = getenv("OA_ROOM_VITALS_URL");
  if (dev_dir && !(dev_url && *dev_url)) { rm_vitals_local(view()); return; }
  char buf[512];
  int64_t batch = server->vit.batch;
  if (!post_report(POST_VITALS,api_url("OA_ROOM_VITALS_URL", "/api/playtest/room-vitals", buf, sizeof buf), server->vit.mac.e, server->vit.out.e, n, batch))
    rm_vitals_answer(view(), batch, 0, (coax_string){"", 0});
}
/* Moderators' actions (room-mod.coax, #13302): the log's outbox goes to the API on its own lane, and
   an action leaves it only on the API's receipt. --dev posts only when OA_ROOM_MOD_URL names an API;
   otherwise the stand-in takes each post as logged. */
static void mod_post(void) {
  int64_t n = rm_mod_due(view());
  if (n <= 0) return;
  const char *dev_url = getenv("OA_ROOM_MOD_URL");
  if (dev_dir && !(dev_url && *dev_url)) { rm_mod_local(view()); return; }
  char buf[512];
  int64_t batch = server->mod.batch;
  if (!post_report(POST_MOD, api_url("OA_ROOM_MOD_URL", "/api/playtest/room-mod-log", buf, sizeof buf), server->mod.mac.e, server->mod.out.e, n, batch))
    rm_mod_answer(view(), batch, 0, (coax_string){"", 0});
}
/* The item ledger's lane (room-items.coax, #13291): loot, quest stages and rewards, and what a character
   wears, one operation a post (everglade.item-op.v1), each sent again as it was until the API answers.
   --dev posts only when OA_ROOM_ITEMS_URL names an API; otherwise the room's stand-in store answers. */
static void items_post(void) {
  int64_t n = rm_items_due(view());
  if (n <= 0) return;
  const char *dev_url = getenv("OA_ROOM_ITEMS_URL");
  if (dev_dir && !(dev_url && *dev_url)) { rm_items_local(view()); return; }
  char buf[512];
  int64_t request = server->itm.request;
  if (!post_report(POST_ITEMS, api_url("OA_ROOM_ITEMS_URL", "/api/playtest/room-items", buf, sizeof buf), server->itm.mac.e, server->itm.out.e, n, request))
    rm_items_answer(view(), request, 0, (coax_string){"", 0});
}
/* The content tables (aethyr-content-v1; room-items.coax rm_content_load, #13294), read once at start: the
   file OA_ROOM_CONTENT names, else the repository's under --dev, else the one staged beside this binary
   (stage.sh: src/everglade.content on a VM, /usr/share/everglade/ in the image). A file that is not
   Everglade's content stops the room at once; with no file the room runs with no loot or quests. */
static void content_load(void) {
  static char text[65537], beside[1100];
  const char *env = getenv("OA_ROOM_CONTENT");
  const char *paths[4] = {env && *env ? env : NULL, dev_dir ? "everglade/content/everglade.content" : NULL, NULL, "/usr/share/everglade/everglade.content"};
#ifdef __linux__
  char exe[1024]; ssize_t en = readlink("/proc/self/exe", exe, sizeof exe - 1);
  if (en > 0) { exe[en] = 0; char *slash = strrchr(exe, '/'); if (slash) { *slash = 0; snprintf(beside, sizeof beside, "%s/src/everglade.content", exe); paths[2] = beside; } }
#endif
  FILE *f = NULL; const char *path = NULL;
  for (int i = 0; i < 4 && !f; i++) if (paths[i]) { path = paths[i]; f = fopen(path, "rb"); if (!f && i == 0) { fprintf(stderr, "room: OA_ROOM_CONTENT names %s, which can't be read\n", path); exit(1); } }
  if (!f) { printf("room: no content tables: no loot or quests\n"); return; }
  size_t n = fread(text, 1, sizeof text, f);
  fclose(f);
  if (n == 0 || n > 65536 || !rm_content_load(view(), (coax_string){text, (int64_t)n})) {
    fprintf(stderr, "room: %s is not Everglade's content tables (coax/web/tool.sh everglade/model/tools/content-check.coax FILE)\n", path); exit(1);
  }
  printf("room: content tables revision %lld loaded from %s\n", (long long)rm_content_rev(view()), path);
}
static void checkpoint_post(void) {
  const char *dev_url=getenv("OA_ROOM_CHECKPOINT_URL");
  /* --dev with no API: the stand-in's store answers, as production's API does (#13057) */
  if (dev_dir && !(dev_url && *dev_url)) { if (rm_dev_checkpoint(view())) dev_store_save(); return; }
  int64_t n=rm_checkpoint_due(view()); if(n<=0)return;
  char buf[512];int64_t request=server->ck.request;
  if(!post_report(POST_CHECKPOINT,api_url("OA_ROOM_CHECKPOINT_URL","/api/playtest/room-checkpoint-v2",buf,sizeof buf),server->ck.mac.e,server->ck.out.e,n,request))
    rm_checkpoint_answer(view(),request,0,(coax_string){"",0});
}
static void poster_hear(void) {
  static int64_t edit_asked=-1;static char buf[POST_CAP+1];
  for(int lane=0;lane<POST_LANES;lane++)for(int tries=0;tries<8;tries++){
    if(poster_fd[lane]<0)break;
    ssize_t n=recv(poster_fd[lane],buf,sizeof buf-1,0);if(n<=0)break;if(n>=POST_CAP)continue;buf[n]=0;
    char *nl=memchr(buf,'\n',(size_t)n);long long id,batch;int status;size_t len;
    if(!nl||sscanf(buf,"%lld %lld %d %zu",&id,&batch,&status,&len)!=4)continue;
    size_t head=(size_t)(nl+1-buf);if(len!=(size_t)n-head)continue;
    bool ok=status>=200&&status<300;
    if(lane==POST_WORLD)ok=ok&&len>0&&buf[head]=='{';
    if(!bg_finish(background(),lane,id,ok,now_ms()))continue;
    coax_string body={buf+head,(int64_t)len};
    if(lane==POST_CHECKPOINT)rm_checkpoint_answer(view(),batch,status,body);
    else if(lane==POST_VITALS)rm_vitals_answer(view(),batch,status,body);
    else if(lane==POST_MOD)rm_mod_answer(view(),batch,status,body);
    else if(lane==POST_ITEMS)rm_items_answer(view(),batch,status,body);
    else if(lane==POST_WORLD&&ok){int64_t preparing=now_us();world_edits(body.data,body.count);world_publication_us=now_us()-preparing;warm_rooms();printf("room: the realm's world (%zu bytes) laid over the rooms\n",len);}
    else if(lane==POST_CONTROL&&ok){
      rm_control(view(),body);
      if(server->ctl.edit_known&&server->ctl.edit!=edit_asked){edit_asked=server->ctl.edit;bg_world_changed(background());}
      for(int i=0;i<CONNS;i++)if(conns[i].fd>=0&&rm_relay_kicked(view(),i))conns[i].doomed=true;
    }
  }
  /* what an answer told the pages (r 0 elsewhere to a fenced seat, #12834; g CODE to a traveller whose
     state is now saved, #12850) goes out before the main loop closes a kicked socket */
  relay_flush();
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
  const char *dev_url = getenv("OA_ROOM_DIRECTORY_URL");
  /* --dev: the room's own list answers /api/playtest/rooms/, unless OA_ROOM_DIRECTORY_URL names an API
     (the parties check, coax/web/api/tests/room_checks.coax, #12852) */
  if (n <= 0 || (dev_dir && !(dev_url && *dev_url))) return;
  char buf[512];
  post_signed(POST_DIRECTORY,api_url("OA_ROOM_DIRECTORY_URL", "/api/playtest/room-directory", buf, sizeof buf), server->dir.mac.e, server->dir.out.e, n);
}

/* Once a second, off the tick: each seat's relevant entities, and its socket's round trip (room-stats.h). */
static void stats_sample(void) {
  int64_t t0 = now_us(), ks[2];
  for (int i = 0; i < CONNS; i++) {
    if (conns[i].fd < 0 || !conns[i].ws || !rm_st_conn(view(), i, (coax_v_s64){ks, 2})) continue;
    st_c[i].relevant = (int)rm_st_relevant(view(), ks[0], (coax_t_SeatId){ ks[1]});
    st_c[i].rtt_ms = conns[i].wt ? wt_rtt(i) : st_rtt(conns[i].fd);
    int64_t w[4];
    if (rm_st_wire(view(), ks[0], (coax_t_SeatId){ ks[1]}, (coax_v_s64){w, 4})) {
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
  int64_t observation_started=now_us(),observation_path_us=st_cost_us;
  static char body[ST_BODY_CAP];
  st_buf o = {body, 0, sizeof body-512,0};
  body[0] = 0;
  st_line(&o, "t %lld", (long long)wall_ms());
  size_t head = o.n;
  st_line(&o,"process generation=%lld",(long long)server->vit.gen);
  st_line(&o,"capacity startup ready=%d",started_ok&&rooms_warmed?1:0);
  double cost = st_tick_us > 0 ? 100.0 * (double)st_cost_us / (double)st_tick_us : 0;
  int64_t p95 = st_hist_at(&st_work[STW_ALL], 950), ack95 = st_ack_q(NULL, st_ack_all, st_ack_all_n, 950);
  rm_dir_tick(view(), p95);   /* the directory's health line (#13179) */
  st_report(&o, view(), rm_players(view()), server->shard, secs, true);
  int64_t report_us=now_us()-observation_started,omitted=o.omitted;o.cap=sizeof body;
  st_line(&o,"report omitted lines=%lld",(long long)omitted);
  st_line(&o,"report generation us=%lld",(long long)report_us);
  st_line(&o,"observation measured wall share %%=%.3f",secs>0?100.0*(observation_path_us+report_us)/(secs*1000000.0):0.0);
  st_line(&o,"observation timing scope=timed observer spans plus report; excludes inline serializer input and transport counters");
  memcpy(st_last, body + head, o.n - head + 1);
  if (rm_players(view()) > 0) {
    printf("room: vitals: tick p95 %.2f ms, input to ack p95 %lld ms, measuring %.2f%% of the tick\n", p95 / 1000.0, (long long)ack95, cost);
    /* Effects only: Coax supplies each initialized world's contract and observations. Keep this
       local log outside the published signed stats body; unavailable counters remain -1. */
    for (int realm = 1; realm <= 2; realm++) {
      size_t count = (size_t)rm_net_rooms(realm);   /* Everglade's rooms, or a game's (room-games.coax) */
      for (size_t room = 0; room < count; room++) {
        int64_t ready = rm_net_observation(view(), realm, (int64_t)room, 0);
        if (ready < 0) continue;
        printf("room: net realm %d room %zu ready %lld hz %lld steps %lld tick %lld seats %lld accepted %lld refused %lld frames %lld\n",
          realm, room, (long long)ready,
          (long long)rm_net_observation(view(), realm, room, 1),
          (long long)rm_net_observation(view(), realm, room, 2),
          (long long)rm_net_observation(view(), realm, room, 7),
          (long long)rm_net_observation(view(), realm, room, 8),
          (long long)rm_net_observation(view(), realm, room, 9),
          (long long)rm_net_observation(view(), realm, room, 10),
          (long long)rm_net_observation(view(), realm, room, 11));
      }
    }
  }
  if (dev_dir) return;
  static uint8_t mac[65];
  if (!rm_st_sign(view(), (coax_string){body, (int64_t)o.n}, (coax_v_u8){mac, 64})) return;
  char buf[512];
  post_signed(POST_STATS,api_url("OA_ROOM_STATS_URL", "/api/playtest/room-stats", buf, sizeof buf), mac, (const uint8_t *)body, (int64_t)o.n);
}

/* The last process's crash (room-crash.c, #13298, docs/deployment/crash-reports.md): its dump and
   addr2line's names become the report in Coax (room-crash.coax crash_room_report), signed as this
   shard's `crash` write and posted to POST /api/playtest/room-crash on the stats lane, once. */
static char crash_dump[32768], crash_names[16384];
static size_t crash_dump_n, crash_names_n;
static void crash_post(void) {
  if (dev_dir || !crash_dump_n) return;
  static uint8_t body[8192], mac[65];
  coax_string r = crash_room_report((coax_string){crash_dump, (int64_t)crash_dump_n}, (coax_string){crash_names, (int64_t)crash_names_n}, (coax_v_u8){body, sizeof body});
  if (r.count <= 0) { fprintf(stderr, "room: the last crash left a dump that is not one: no report\n"); return; }
  printf("room: the last process crashed; its report:\n%.*s", (int)r.count, r.data);
  if (!rm_crash_sign(view(), r, (coax_v_u8){mac, 64})) { fprintf(stderr, "room: no write key: the crash report is not posted\n"); return; }
  char buf[512];
  post_signed(POST_STATS, api_url("OA_ROOM_CRASH_URL", "/api/playtest/room-crash", buf, sizeof buf), mac, body, r.count);
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
  const char *wt_cert = getenv("OA_ROOM_WT_CERT"), *wt_key = getenv("OA_ROOM_WT_KEY");
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--wt") && i + 2 < argc) { wt_cert = argv[++i]; wt_key = argv[++i]; continue; }
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) address = argv[++i];
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = atoi(argv[++i]);
    else if (!strcmp(argv[i], "--dev") && i + 1 < argc) dev_dir = argv[++i];
    else if (!strcmp(argv[i], "--no-rooms")) no_rooms = true;
    else if (!strcmp(argv[i], "--native-local")) native_local = true;
    else if (!strcmp(argv[i], "--start-check")) return start_check();
  }
  const char *env = getenv("PORT");
  if (env && *env) { port = atoi(env); address = "0.0.0.0"; }
  /* crash capture from here on, and the last process's crash read before SIGCHLD is ignored (#13298) */
  { const char *sh = getenv("OA_ROOM_SHARD"); crash_install(sh && *sh ? atoi(sh) : 0); }
  crash_dump_n = crash_pending(crash_dump, sizeof crash_dump, crash_names, sizeof crash_names, &crash_names_n);
#ifdef COAX_FAST_EDIT_ENABLED
  const char *fast_directory=getenv("COAX_FAST_EDIT");
  if(!dev_dir||strcmp(address,"127.0.0.1")||!coax_fast_start(fast_directory)) {
    fprintf(stderr,"room: fast-edit requires a sealed session, --dev and --listen 127.0.0.1\n");return 1;
  }
  atexit(coax_fast_close);
#endif
  signal(SIGPIPE, SIG_IGN);
  signal(SIGCHLD, SIG_IGN);   /* vitals_post's curls reap themselves */
  setvbuf(stdout, NULL, _IOLBF, 0);
  poster_start();   /* before the server's memory (vitals_post) */
  server = server_new();
  if (!server) { fprintf(stderr, "room: out of memory\n"); return 1; }
  started_ok = rm_start_check(view()) == 0;   /* readiness needs the started server to hold its defaults (#12797) */
  if (!dice_key_in()) { fprintf(stderr, "room: no OS random source for the dice (getentropy)\n"); return 1; }   /* #13258 */
  rm_vitals_gen(view(), wall_ms());   /* this process's generation, in every vitals operation ID (#12793) */
  { const char *dg = getenv("OA_ROOM_DIR_GEN"); if (dg && !strcmp(dg, "1")) rm_dir_gen(view(), true); }   /* the directory's fence (#12797) */
  signal(SIGTERM, on_term);
  if (dev_dir) { rm_dev_start(view()); dev_store_open(port); const char *da = getenv("OA_ROOM_DEV_ADMIN"); if (da && !strcmp(da, "0")) rm_dev_admin(view(), false); printf("room: --dev %s: the API's stand-in (seat tickets checked and required)\n", dev_dir); }
  /* Everglade II's admin lines (everglade/server/admin.coax): --dev and 127.0.0.1 only; PORT (Cloud Run) binds 0.0.0.0 */
  if (rm_admin_local(view(), (coax_string){address, (int64_t)strlen(address)})) printf("room: admin lines taken (--dev on 127.0.0.1)\n");
  if (native_local && !rm_native_local(view(), (coax_string){address,(int64_t)strlen(address)})) {
    fprintf(stderr,"room: --native-local requires --dev and --listen 127.0.0.1\n");return 1;
  }
  world_ask();   /* the realm's published world, for every room's plan (room-world.c world_edits) */
  /* the seat-ticket key (Secret Manager oa-room-ticket-key): set, an everglade join needs a ticket (room-ticket.coax) */
  const char *tkey = getenv("OA_ROOM_TICKET_KEY");
  const char *must = getenv("OA_ROOM_TICKET_REQUIRED");   /* "0": a join without a ticket sits with no id (while pages roll out) */
  bool required = !(must && !strcmp(must, "0"));
  content_load();   /* the content tables: loot, quests, dialogue, points (#13294) */
  /* which shard of the instance directory this is (oa-room 1, oa-room-K K: room-directory.coax) */
  const char *shard = getenv("OA_ROOM_SHARD");
  if (shard && *shard) rm_shard(view(), atoi(shard));
  /* a room VM's shard names where its pages connect, and reports its health every 5 s (#13179) */
  { const char *ep = getenv("OA_ROOM_ENDPOINT"); if (ep && *ep && !rm_endpoint(view(), (coax_string){ep, (int64_t)strlen(ep)})) fprintf(stderr, "room: OA_ROOM_ENDPOINT is not a wss:// URL ending in /: not named\n"); }
  printf("room: shard %lld\n", (long long)server->shard);
  /* the API's public keys for v5 tickets (#13260, docs/deployment/room-keys.md): one, or two while a key
     rotates. With them and no OA_ROOM_TICKET_KEY, only v5 tickets seat, and nothing this host holds can sign one. */
  const char *tpub = getenv("OA_ROOM_TICKET_PUBKEYS");
  if (dev_dir) {}
  else {
    bool hmac = tkey && strlen(tkey) >= 32, v5 = tpub && *tpub;
    if (hmac) rm_ticket_key(view(), (coax_string){tkey, (int64_t)strlen(tkey)}, required);
    if (v5 && !rm_ticket_pubkeys(view(), (coax_string){tpub, (int64_t)strlen(tpub)}, required)) {
      fprintf(stderr, "room: OA_ROOM_TICKET_PUBKEYS is not one or two x-only public keys in lowercase hex\n"); return 1;
    }
    /* this shard's own write key (#13260, docs/deployment/room-keys.md): "KID:KEYHEX", made from the API's
       write master for this shard only. With it every write is signed as this shard; without it, with the shared key. */
    const char *wkey = getenv("OA_ROOM_WRITE_KEY");
    if (wkey && *wkey) {
      if (!rm_write_key(view(), (coax_string){wkey, (int64_t)strlen(wkey)})) {
        fprintf(stderr, "room: OA_ROOM_WRITE_KEY is not KID:KEYHEX (8 and 64 lowercase hex digits), or OA_ROOM_SHARD is not 1 to 99\n"); return 1;
      }
      coax_string sg = rm_write_signer(view(), (coax_v_u8){(uint8_t *)write_signer, (int64_t)sizeof write_signer - 1});
      write_signer[sg.count] = 0;
      snprintf(write_signer_line, sizeof write_signer_line, "header = \"X-Room-Signer: %s\"\n", write_signer);
      printf("room: writes signed as shard %lld with its own key\n", (long long)server->shard);
    } else if (hmac) printf("room: writes signed with the shared OA_ROOM_TICKET_KEY\n");
    if (hmac || v5) printf("room: seat tickets checked (%s%s%s)%s\n", hmac ? "v2-v4" : "", hmac && v5 ? ", " : "", v5 ? "v5" : "", required ? ", required" : "");
    else printf("room: no OA_ROOM_TICKET_KEY or OA_ROOM_TICKET_PUBKEYS, seat tickets not checked\n");
  }
  crash_post();   /* the last process's crash, now that this shard can sign it */
  int lfd = socket(AF_INET, SOCK_STREAM, 0);
  int one = 1; setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0};
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  inet_pton(AF_INET, address, &a.sin_addr);
  if (bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 512)) { perror("room: listen"); return 1; }
  fcntl(lfd, F_SETFL, O_NONBLOCK); fcntl(lfd, F_SETFD, FD_CLOEXEC);
  for (int i = 0; i < CONNS; i++) conns[i].fd = -1;
  printf("room: listening on %s:%d\n", address, port);
  if (wt_cert && *wt_cert && wt_key && *wt_key) {
    const char *wp = getenv("OA_ROOM_WT_PORT");
    if (!wt_start(address, wp && *wp ? atoi(wp) : port, wt_cert, wt_key)) return 1;
  }
  const double tick = 1000.0 / 60.0;
  double next = (double)now_ms();
  int64_t ticks = 0, last_report = now_ms(), last_vitals = now_ms(), last_stats = now_ms(), drain_at = 0;
  for (;;) {
#ifdef COAX_FAST_EDIT_ENABLED
    coax_fast_poll();
#endif
    if (drain_phase > DRAIN_NONE && drain_phase < DRAIN_DONE && !term_asked) drain_step();
    /* SIGUSR2 (vm-deploy.sh crash, root on the VM only): a Coax trap on purpose, to prove the crash path (#13298) */
    if (crash_test_asked()) { printf("room: SIGUSR2: the crash test, a Coax trap on purpose\n"); (void)rm_crash_test(view(), 4 + (ticks & 3)); }
    /* SIGTERM: bounded final work only (Cloud Run kills at 10 s). A drained room has nothing left; one
       that wasn't drained tells its pages "restarting" (they sit elsewhere at once, not after a silent
       socket's wait), and every seat's vitals go out as the last step before exit. */
    if (term_asked && !drain_at) {
      drain_at = now_ms();
      if (drain_phase < DRAIN_TELLING) { int64_t told = rm_drain_tell(view()); relay_flush(); printf("room: SIGTERM before a drain: told %lld pages restarting\n", (long long)told); }
      rm_vitals_flush(view()); rm_mod_flush(view()); last_vitals = 0;
      printf("room: SIGTERM: saving %lld players' vitals before exit\n", (long long)rm_vitals_pending(view()));
    }
    if (drain_at) {
      int64_t left = rm_vitals_pending(view()) + rm_mod_pending(view());
      if ((left == 0 && !server->vit.fly && rm_checkpoint_clean(view())) || now_ms() - drain_at > DRAIN_MS) {
        printf("room: exit after %lld ms, %lld players' vitals with no receipt\n", (long long)(now_ms() - drain_at), (long long)left);
        fprintf(stderr,"room: exit: %lld unacknowledged character checkpoints\n",(long long)rm_checkpoint_pending(view()));
        return rm_checkpoint_clean(view()) ? 0 : 1;
      }
    }
    struct pollfd fds[CONNS + 2]; int map[CONNS + 2], n = 0;
    fds[n] = (struct pollfd){lfd, POLLIN, 0}; map[n++] = -1;
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0 && !conns[i].wt) { fds[n] = (struct pollfd){conns[i].fd, (short)(POLLIN | (conns[i].on ? POLLOUT : 0)), 0}; map[n++] = i; }
    if (wts) { fds[n] = (struct pollfd){wt_fd(wts), POLLIN, 0}; map[n++] = -2; }
    int wait = (int)(next - (double)now_ms());
    if (wts) { int wto = wt_timeout_ms(wts); if (wto < wait) wait = wto; }
    if (poll(fds, (nfds_t)n, wait > 0 ? wait : 0) < 0 && errno != EINTR) { perror("room: poll"); return 1; }
    int64_t now = now_ms();
    clock_in();
    for (int k = 0; k < n; k++) {
      if (map[k] >= 0 && (fds[k].revents & POLLOUT) && conns[map[k]].fd >= 0 && !pump(map[k])) conns[map[k]].doomed = true;
      if (!(fds[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (map[k] == -2) { wt_read(wts); relay_flush(); continue; }   /* WebTransport: every datagram waiting (room-wt.c) */
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
      /* each realm steps when its profile says (room.coax rm_profile: 0 the games on their own paths, room-games.coax; 1 Everglade) */
      if (rm_realm_due(view(), 0, ticks)) rm_tick(view());
      if (rm_realm_due(view(), 1, ticks)) { everglade_tick(); fault_step(); int64_t dt = now_us() - t_tick; tick_add(dt); st_hist_add(&st_work[STW_ALL], dt); st_ticks++; st_tick_us += dt; crash_tick(wall_ms(), dt, rm_players(view())); }
      for (int i = 0; i < CONNS; i++) {
        if (conns[i].fd < 0 || !conns[i].ws || conns[i].wt) continue;
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
      if (now - last_vitals >= 1000) { last_vitals = now; vitals_post(); directory_post(); stats_sample(); wt_note(); }
      if (now - last_stats >= ST_WINDOW_MS) { stats_window((now - last_stats) / 1000.0); last_stats = now; }
      control_ask();poster_hear();if(!dev_dir)world_ask();checkpoint_post();mod_post();items_post();
      if (now - last_report >= 60000) { printf("room: %lld ticks in %.1f s, %lld players, %lld frames over the rate limits, %lld queue full, %lld slow sockets closed, %lld bytes the most waiting\n", (long long)ticks, (now - last_report) / 1000.0, (long long)rm_players(view()), (long long)rm_relay_dropped(view()), (long long)rm_relay_queue_full(view()), (long long)slow_closed, (long long)out_peak);
        printf("room: everglade tick parts over the minute (ms):"); for (int q = 0; q < PARTS; q++) { printf(" %s %lld", part_name[q], (long long)(part_us[q] / 1000)); part_us[q] = 0; }
        printf(", %lld events with no room in their tick\n", (long long)rm_eg_dropped(view()));
        { coax_v_t_RmServer w = view();
          printf("room: vitals posts %lld (%lld retried, %lld failed), lines applied %lld, already applied %lld, stale %lld, refused %lld, late answers %lld, given up %lld, evicted unsaved %lld, %lld players unsaved now, fenced %lld (%lld seats ended)\n",
                 (long long)rm_vitals_stat(w, 0), (long long)rm_vitals_stat(w, 1), (long long)rm_vitals_stat(w, 2), (long long)rm_vitals_stat(w, 3), (long long)rm_vitals_stat(w, 4),
                 (long long)rm_vitals_stat(w, 5), (long long)rm_vitals_stat(w, 6), (long long)rm_vitals_stat(w, 7), (long long)rm_vitals_stat(w, 8), (long long)rm_vitals_stat(w, 9), (long long)rm_vitals_stat(w, 10),
                 (long long)rm_vitals_stat(w, 11), (long long)rm_vitals_stat(w, 12));
          /* travel between rooms (#12850, room-travel.coax) */
          printf("room: travel asked %lld, sent on %lld, full %lld, refused %lld, arrived %lld, called off %lld, intents dropped in a cutover %lld\n",
                 (long long)rm_travel_stat(w, 0), (long long)rm_travel_stat(w, 1), (long long)rm_travel_stat(w, 2), (long long)rm_travel_stat(w, 3),
                 (long long)rm_travel_stat(w, 4), (long long)rm_travel_stat(w, 5), (long long)rm_travel_stat(w, 6)); }
        ticks = 0; last_report = now; memcpy(fan_last, fan_now, sizeof fan_now); memset(fan_now, 0, sizeof fan_now); memcpy(tick_last, tick_now, sizeof tick_now); memset(tick_now, 0, sizeof tick_now); }
    }
    if (wts) { reap(); wt_flush(wts); }   /* every session's timers and waiting bytes (room-wt.c) */
  }
}
