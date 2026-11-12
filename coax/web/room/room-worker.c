/* The room server's local host (GitHub #12784; Aethyr book chapter 95): the same server oa-room runs
   (room.coax, the Everglade rooms of room-everglade.coax and the world of room-world.c), built to
   wasm32 (worker.coax) and run in the browser by coax/web/room-worker.js, which moves the frames over
   a MessagePort where room-host.c moves them over TCP. Offline play is a one-seat room on it: the page
   is always a client, and a step added to the shared tick (room-tick.c) reaches both hosts.
   Every decision is room.coax's. This file starts the server as room-host.c does (every declared
   default, then rm_init), hands it each join, frame and close, runs the 30 Hz everglade tick, and
   hands the frames it writes back to the script. The script gives the clock and the world document. */
#include "room.h"
#include "room-world.h"
#include <stdlib.h>
#include <string.h>
#include <sys/random.h>   /* getentropy (room-tick.c dice_key_in) */

#define EXPORT(name) __attribute__((export_name(name)))
__attribute__((import_module("env"), import_name("room_now_us"))) extern double host_now_us(void);

static coax_t_RmServer *server;
static int64_t wall_now;
static coax_v_t_RmServer view(void) { return (coax_v_t_RmServer){server, 1}; }
static int64_t now_us(void) { return (int64_t)host_now_us(); }
static int64_t wall_ms(void) { return wall_now; }
#include "room-tick.c"

static char *buf; static int64_t buf_cap;

/* The server with every field at its declared default, then rm_init, as room-host.c server_new. */
EXPORT("rw_start") int32_t rw_start(void) {
  if (server) return 1;
  server = malloc(sizeof *server);
  if (!server) return 0;
  coax_default_t_RmServer(server);
  rm_init(view());
  rm_control_local(view());   /* offline play: the room's control lines are its own player's (room-control.coax) */
  if (rm_start_check(view()) != 0) return 0;
  return dice_key_in() ? 1 : 0;   /* the dice from WASI random_get (room-worker.js), #13258 */
}
EXPORT("rw_size") int32_t rw_size(void) { return (int32_t)sizeof *server; }
/* n bytes the script writes before a call that reads them (a request, a frame, a document). */
EXPORT("rw_buf") uint32_t rw_buf(int32_t n) {
  if (n + 1 > buf_cap) { char *b = realloc(buf, (size_t)n + 1); if (!b) return 0; buf = b; buf_cap = n + 1; }
  return (uint32_t)(uintptr_t)buf;
}
static coax_string text(int32_t n) { return (coax_string){buf, n}; }
EXPORT("rw_clock") void rw_clock(double mono_ms, double wall) {
  static bool gen;
  wall_now = (int64_t)wall; rm_clock(view(), (int64_t)mono_ms, (int64_t)wall);
  if (!gen) { gen = true; rm_vitals_gen(view(), wall_now); }   /* this host's generation, as room-host.c main (#12793) */
}
/* Once a second: the players' vitals the room notes, kept by the room itself as room-host.c does under
   --dev with no vitals URL (room-vitals.coax rm_vitals_local): offline play has no API to post to. */
EXPORT("rw_vitals") void rw_vitals(void) {
  if (rm_vitals_due(view()) > 0) rm_vitals_local(view());
  /* the item ledger's operations (room-items.coax): offline, the room's stand-in store answers them */
  for (int i = 0; i < 16 && rm_items_due(view()) > 0; i++) rm_items_local(view());
}
/* The realm's published world (its JSON), laid over the room's plan, as room-host.c world_ask. */
EXPORT("rw_edits") void rw_edits(int32_t n) { world_edits(buf, n); }
/* The API's word on the rooms (room-control.coax lines), as room-host.c control_ask under --dev. */
EXPORT("rw_control") void rw_control(int32_t n) { rm_control(view(), text(n)); }
EXPORT("rw_join") int32_t rw_join(int32_t conn, int32_t n) { return (int32_t)rm_join(view(), conn, text(n)); }
EXPORT("rw_heard") void rw_heard(int32_t conn, int32_t n) { rm_heard(view(), conn, text(n)); }
EXPORT("rw_left") void rw_left(int32_t conn) { rm_left(view(), conn); }
EXPORT("rw_kicked") int32_t rw_kicked(int32_t conn) { return rm_relay_kicked(view(), conn) ? 1 : 0; }
/* One everglade tick (30 Hz): every live room's step, its D frames queued. */
EXPORT("rw_tick") int32_t rw_tick(void) {
  int32_t live = 0;
#ifdef COAX_ROOM_PROFILE
  int64_t phase_start=now_us();
#endif
  rm_tick(view());
  ROOM_PROFILE_PHASE(6,phase_start);
  for (int k = 0; k < WORLD_ROOMS; k++) if (everglade_room_tick(k)) live++;
  return live;
}
/* What the room has scheduled: casts begun and not yet resolved (room-casts.coax; the agent clock's settle, #13096). */
EXPORT("rw_scheduled") int32_t rw_scheduled(void) { return (int32_t)rm_scheduled(view()); }
/* The frames queued, one at a time: its length (0 none), its bytes at rw_frame, rw_to whom. */
EXPORT("rw_next") int32_t rw_next(void) { return (int32_t)rm_relay_next(view()); }
EXPORT("rw_frame") uint32_t rw_frame(void) { return (uint32_t)(uintptr_t)server->rout.e; }
EXPORT("rw_to") int32_t rw_to(int32_t conn) { return rm_relay_to(view(), conn) ? 1 : 0; }
/* Where the tick's time went, microseconds since the start (room-tick.c PART_*). */
EXPORT("rw_part_us") double rw_part_us(int32_t q) { return q >= 0 && q < PARTS ? (double)part_us[q] : 0; }
#ifdef COAX_ROOM_PROFILE
EXPORT("rw_profile_us") double rw_profile_us(int32_t q) { return q>=0 && q<7 ? (double)profile_us[q] : 0; }
#endif
