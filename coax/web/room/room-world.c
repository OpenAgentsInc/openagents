/* The room server's Everglade worlds (GitHub #12705; spec docs/verse/2026-11-03-everglade-multiplayer-spec.md
   section 3.1): each simulated everglade room (room-everglade.coax) has its own headless Everglade
   (apps/openagents-web/everglade.coax: the city's ground, walls, bodies and floors, nothing drawn),
   and each of its seats a controller (everglade-step.coax, the unit that carries aethyr/player.coax).
   A seat's input is stepped by the movement step the page predicts with, in the order written at the
   top of everglade-step.coax, so the page and the server put the player in the same place.
   The town runs here too (world_town): everyone's day, the watch against every seat, crimes and WANTED.
   Not yet the server's: the river (everglade-river: you walk its bed here, where a page swims), and
   the page's per-player states the world half reads (a blast's throw, held, slowed, the ice). Those
   stay at rest until the world's step owns them per seat.
   This file includes only the Everglade units' headers; room-host.c calls it with plain numbers. */
#include "everglade.h"
#include "everglade-step.h"
#include "room-world.h"
#include <stdlib.h>
#include <string.h>

static coax_t_Everglade *world[WORLD_ROOMS];
static coax_t_Player seat_p[WORLD_ROOMS][WORLD_SEATS];

static coax_v_t_Everglade at(int k) { return (coax_v_t_Everglade){world[k], 1}; }

/* Room k's world, made (and its city built) the first time; kept for the next room in that slot. */
static bool world_of(int k) {
  if (k < 0 || k >= WORLD_ROOMS) return false;
  if (!world[k]) {
    world[k] = calloc(1, sizeof *world[k]);
    if (!world[k]) return false;
    (void)everglade_walls(at(k));   /* starts it: the ground, the town, the walls and bodies */
  }
  return true;
}

static void pose_of(coax_t_Player p, bool moved, float out[WORLD_POSE]) {
  out[0] = p.pos.x; out[1] = p.pos.y; out[2] = p.pos.z; out[3] = verse_player_face(p);
  out[4] = p.vel.x; out[5] = p.vel.y; out[6] = p.vel.z;
  out[7] = p.on_ground ? 1 : 0;            /* G: 1 on the ground (2 swimming, 4 in a boat: the river's, not yet) */
  out[8] = (float)p.act;                   /* CLIP: the controller's act (aethyr/walk.coax Act) */
  out[9] = (float)(p.clock.carry_ns / 100000);   /* CARRY: the fixed step's leftover, tenths of a ms */
  (void)moved;
}

bool world_place(int k, int s, float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Point spawn = everglade_spawn(at(k));
  seat_p[k][s] = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(k)));
  pose_of(seat_p[k][s], false, out);
  return true;
}

bool world_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Everglade *g = world[k];
  coax_t_Player p = seat_p[k][s];
  const float dt = 1.0f / WORLD_HZ, frame = dt;
  /* in: SEQ KX KY PX PY THETA B (cm-scaled sticks, centiradians, bits 1 run 2 jump 4 dive) */
  coax_t_EgMoveIn input = {(float)in[1] / 100.0f, (float)in[2] / 100.0f, (float)in[3] / 100.0f, (float)in[4] / 100.0f,
                           (in[6] & 1) != 0, (in[6] & 2) != 0, (float)in[5] / 100.0f};
  coax_t_EgMoveWorld mw = everglade_move_world(at(k), p.pos.x, p.pos.y, p.pos.z, 0, 1.0f, 0, 0, false, 0, 0, 0, (coax_v_f32){NULL, 0});
  p = eg_step_begin(p, input, mw, dt);
  coax_t_EgBody before = eg_step_body(p);
  coax_v_f32 near = everglade_move_bodies(at(k));
  p = eg_step_advance(p, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){g->walls.e, mw.walls * 6}, (coax_v_f32){g->floors.e, mw.floors * 10}, frame);
  coax_t_EgBody moved = everglade_move_after(at(k), mw, before, eg_step_body(p), dt);
  p = eg_step_put(p, moved);
  seat_p[k][s] = p;
  pose_of(p, true, out);
  return true;
}

float world_ground(int k, float x, float z) { return world_of(k) ? everglade_height(at(k), x, z) : 0.0f; }

void world_folk_hp(int k, int j, int hp, int temp, int by) { if (world_of(k)) everglade_room_folk_hp(at(k), j, hp, temp, by); }
void world_crime(int k, int by, float x, float z) { if (world_of(k)) (void)everglade_room_crime(at(k), by, x, z); }
void world_town(int k, float dt, double world_s, const float seats[WORLD_SEATS * WORLD_SEAT_TOWN], float took[WORLD_SEATS]) {
  if (!world_of(k)) return;
  everglade_room_town(at(k), dt, world_s, (coax_v_f32){(float *)seats, WORLD_SEATS * WORLD_SEAT_TOWN}, WORLD_SEATS, (coax_v_f32){took, WORLD_SEATS});
}
int world_people(int k, float out[WORLD_FOLK * WORLD_PEOPLE]) {
  return world_of(k) ? (int)everglade_room_people(at(k), (coax_v_f32){out, WORLD_FOLK * WORLD_PEOPLE}) : 0;
}
bool world_view(int k, int i, float out[WORLD_VIEW]) { return world_of(k) && everglade_room_view(at(k), i, (coax_v_f32){out, WORLD_VIEW}); }
int world_wanted(int k, int s) { return world_of(k) ? (int)everglade_room_wanted(at(k), s) : 0; }

/* The drawing host's calls, which grid.coax's thumbs make and a headless world never does. */
void host_line(int64_t h, double ax, double ay, double bx, double by, double la, double lb, double w) { (void)h; (void)ax; (void)ay; (void)bx; (void)by; (void)la; (void)lb; (void)w; }
void host_dot(int64_t h, double x, double y, double size, double light, double soft, double crisp) { (void)h; (void)x; (void)y; (void)size; (void)light; (void)soft; (void)crisp; }
void host_slot(int64_t h, double x, double y, double w, double hh, double press, double flash, double sel, double icon) { (void)h; (void)x; (void)y; (void)w; (void)hh; (void)press; (void)flash; (void)sel; (void)icon; }
