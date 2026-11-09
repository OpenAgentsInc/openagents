/* The room server's Everglade worlds (GitHub #12705; spec docs/verse/2026-11-03-everglade-multiplayer-spec.md
   section 3.1): each simulated everglade room (room-everglade.coax) has its own headless Everglade
   (apps/openagents-web/everglade.coax: the city's ground, walls, bodies and floors, nothing drawn),
   and each of its seats a controller (everglade-step.coax, the unit that carries aethyr/player.coax).
   A seat's input is stepped by the movement step the page predicts with, in the order written at the
   top of everglade-step.coax, so the page and the server put the player in the same place.
   The town runs here too (world_town): everyone's day, the watch against every seat, crimes and WANTED.
   The river is the room's too: each seat swims in it, and rows a rowboat in it (world_use WE_BOAT,
   world_floats, #12861). Not yet the server's: the page's per-player states the world half reads (a
   blast's throw, held, slowed, the ice). Those stay at rest until the world's step owns them per seat.
   This file includes only the Everglade units' headers; room-host.c calls it with plain numbers. */
#include "everglade.h"
#include <stdlib.h>
#include <string.h>
#include "everglade-step.h"
#include "room.h"   /* Everglade II's movement world (apps/openagents-web/everglade2-move.coax, the room's unit) */
#include "river.h"
#include "room-world.h"
#include <stdlib.h>
#include <string.h>

static coax_t_Everglade *world[WORLD_ROOMS];
static coax_t_DemoWorld *pieces[WORLD_ROOMS];
extern const int room_catalog_n;               /* room-catalog.c (room-catalog.coax): the pack's catalog */
extern const unsigned char room_catalog[];
static coax_t_Player seat_p[WORLD_ROOMS][WORLD_SEATS];
/* The river (verse/everglade/river.coax): one a room, its water and current; each seat's own part of it
   (the water it is in, its feet, its breath), swapped in while that seat swims, and the swim it left,
   which its next step moves in as the page's does (everglade5e-face.c: the step, then the river). */
static coax_t_River *river[WORLD_ROOMS];
typedef struct { int64_t medium; float feet; bool afloat; double breath, out; int64_t levels; } seat_water;
static seat_water seat_w[WORLD_ROOMS][WORLD_SEATS];
static coax_t_Swim seat_swim[WORLD_ROOMS][WORLD_SEATS];
static double river_now[WORLD_ROOMS];
/* The rowboats (#12861): the float each seat rows (+ 1, 0 none), and the floats as the last tick left
   them (everglade_river_floats' 8 numbers each: x y z yaw tilt lean oars kind) */
static int seat_boat[WORLD_ROOMS][WORLD_SEATS];
#define WORLD_FLOATS 28
static float floats_out[WORLD_ROOMS][WORLD_FLOATS * 8];
static int floats_n[WORLD_ROOMS];
/* the ground's grid as the page's (everglade5e-face.c): EG_GRID cells of 2 m a side from -EG_HALF */
#define EG_GRID 176
#define EG_HALF 176
static coax_v_t_River rv_of(int k) { return (coax_v_t_River){river[k], 1}; }
static void seat_dry(int k, int s) {
  seat_w[k][s] = (seat_water){0, 0, false, 0, 0, 0};
  seat_swim[k][s] = (coax_t_Swim){0}; seat_swim[k][s].pace = 1; seat_swim[k][s].breath = 1; seat_swim[k][s].surface = -1000;
}

/* Each seat moved in one jump since the room last asked (placed, teleported, come to on the bank, into
   or out of a boat): its next row is a WARP however short the jump (world_cut_take, #12835). */
static bool seat_cut[WORLD_ROOMS][WORLD_SEATS];
bool world_cut_take(int k, int s) {
  if (k < 0 || k >= WORLD_ROOMS || s < 0 || s >= WORLD_SEATS || !seat_cut[k][s]) return false;
  seat_cut[k][s] = false;
  return true;
}
static coax_v_t_Everglade at(int k) { return (coax_v_t_Everglade){world[k], 1}; }
static coax_v_f32 heights_of(int k) { return (coax_v_f32){world[k]->heights.e, (EG_GRID + 1) * (EG_GRID + 1)}; }
/* seat s out of the boat it rows, where it is (a seat placed again, gone, or teleported): tied up there */
static void seat_ashore(int k, int s) {
  int b = seat_boat[k][s] - 1;
  seat_boat[k][s] = 0;
  if (b >= 0 && river[k] && everglade_river_crew_of(rv_of(k), b) == s + 1) everglade_river_crew_free(rv_of(k), b);
}
static void seat_aboard(int k, int s) {
  seat_swim[k][s] = (coax_t_Swim){0}; seat_swim[k][s].medium = 4; seat_swim[k][s].pace = 1; seat_swim[k][s].breath = 1; seat_swim[k][s].surface = -1000;
}

/* Room k's world, made (and its city built) the first time; kept for the next room in that slot. */
/* The realm's published world (an admin's placed things: GET /api/playtest/world/everglade), laid over
   every room's plan as each page lays it (everglade-voice.coax everglade_world). Without it a thing
   placed on a street stood in a page's way and not in the server's: the page's prediction stopped
   at it while the server walked on, and its next word snapped you past (#12731). */
static char *edits_text; static int64_t edits_n;
static bool world_of(int k);
void world_edits(const char *text, int64_t n) {
  /* as the page's answer comes: "200\n" and the JSON (verse/everglade/edits.coax eg_edits_answer) */
  char *copy = malloc((size_t)n + 5);
  if (!copy) return;
  memcpy(copy, "200\n", 4); memcpy(copy + 4, text, (size_t)n); copy[n + 4] = 0;
  free(edits_text); edits_text = copy; edits_n = n + 4;
  for (int k = 0; k < WORLD_ROOMS; k++) if (world[k]) everglade_world(at(k), (coax_string){edits_text, edits_n});
}
static bool world_of(int k) {
  if (k < 0 || k >= WORLD_ROOMS) return false;
  if (!world[k]) {
    world[k] = calloc(1, sizeof *world[k]);
    if (!world[k]) return false;
    coax_default_t_Everglade(world[k]);   /* every declared default, as the page's state starts (everglade-face.c coaxial_state_init) */
    (void)everglade_walls(at(k));   /* starts it: the ground, the town, the walls and bodies */
    /* its pieces, which break and burn as a page's (everglade-watch.coax everglade_room_spells_start) */
    river[k] = calloc(1, sizeof *river[k]);
    if (river[k]) {
      coax_default_t_River(river[k]);
      static float levels[EG_GRID + 1 + 16];
      everglade_levels(at(k), (coax_v_f32){levels, EG_GRID + 1 + 16});
      everglade_river_start(rv_of(k), (coax_v_f32){world[k]->heights.e, (EG_GRID + 1) * (EG_GRID + 1)}, (coax_v_f32){levels, EG_GRID + 1 + 16}, EG_GRID, 2, -EG_HALF, -EG_HALF);
      seat_water fresh = {0, 0, false, 0, 0, 0};
      fresh.breath = river[k]->breath; fresh.out = river[k]->out; fresh.levels = river[k]->levels;
      for (int s = 0; s < WORLD_SEATS; s++) { seat_dry(k, s); seat_w[k][s] = fresh; }
    }
    pieces[k] = calloc(1, sizeof *pieces[k]);
    if (pieces[k]) coax_default_t_DemoWorld(pieces[k]);
    if (pieces[k])
      everglade_room_spells_start(at(k), (coax_v_t_DemoWorld){pieces[k], 1}, (coax_string){(const char *)room_catalog, room_catalog_n});
    if (edits_text) everglade_world(at(k), (coax_string){edits_text, edits_n});
  }
  return true;
}
bool world_prepared(int k) { return k >= 0 && k < WORLD_ROOMS && world[k] && river[k] && pieces[k]; }
bool world_prepare(int k) { return world_of(k) && world_prepared(k); }
bool world_forge_open(int k, float x, float z) {
  return world_of(k) && everglade_forge_open(at(k), x, z);
}
void world_forge_prepare(int k) { if (world_of(k)) everglade_forge_prepare(at(k)); }
void world_forge_step(int k, const float in[6], float out[5]) {
  if (!world_of(k)) return;
  everglade_forge_step(at(k), (coax_v_f32){(float *)in, 6}, (coax_v_f32){out, 5});
}

/* The walls within WALL_NEAR m of (x, z), in their order, into near (6 floats each): how many. A wall
   acts only within the controller's radius (0.3 m) of the feet, and a step's three controller steps
   move them 0.45 m at most (max_travel 0.15 m each), so what is left out never touched you: the step
   is the same as over every wall (the page steps over every wall), and the controller tests 30 walls,
   not 256, in each of its passes (#12717). */
#define WALL_NEAR 4.0f
#define WALL_MAX (256 + 2)
static int walls_near(const float *walls, int n, float x, float z, float *near) {
  int m = 0;
  for (int i = 0; i < n && i < WALL_MAX; i++) {
    const float *w = walls + i * 6;
    /* a wall with a bad number goes in as it is: the controller refuses it as the page's would */
    bool bad = !(w[0] == w[0]) || !(w[2] == w[2]) || !(w[3] == w[3]) || !(w[5] == w[5]);
    if (!bad && (x < w[0] - WALL_NEAR || x > w[3] + WALL_NEAR || z < w[2] - WALL_NEAR || z > w[5] + WALL_NEAR)) continue;
    memcpy(near + m * 6, w, 6 * sizeof(float)); m++;
  }
  return m;
}

static void pose_of(coax_t_Player p, bool moved, float out[WORLD_POSE]) {
  out[0] = p.pos.x; out[1] = p.pos.y; out[2] = p.pos.z; out[3] = verse_player_face(p);
  out[4] = p.vel.x; out[5] = p.vel.y; out[6] = p.vel.z;
  out[7] = p.on_ground ? 1 : 0;            /* G: 1 on the ground, 2 swimming, 4 in a boat (world_step adds them) */
  out[8] = (float)p.act;                   /* CLIP: the controller's act (aethyr/walk.coax Act) */
  out[9] = (float)(p.clock.carry_ns / 100000);   /* CARRY: the fixed step's leftover, tenths of a ms */
  (void)moved;
}

bool world_place(int k, int s, float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Point spawn = everglade_spawn(at(k));
  seat_ashore(k, s);
  seat_p[k][s] = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(k))); seat_cut[k][s] = true;
  if (river[k]) { seat_dry(k, s); seat_w[k][s].breath = river[k]->breath; }
  pose_of(seat_p[k][s], false, out);
  return true;
}

bool world_place_at(int k, int s, float x, float y, float z, float yaw, float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  if (!(x == x) || !(y == y) || !(z == z) || !(yaw == yaw)) return world_place(k, s, out);
  seat_ashore(k, s);
  seat_p[k][s] = verse_player_place(x, y, z, yaw); seat_cut[k][s] = true;
  if (river[k]) { seat_dry(k, s); seat_w[k][s].breath = river[k]->breath; }
  pose_of(seat_p[k][s], false, out);
  return true;
}

bool world_place_glade(int k, int s, float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Point p = everglade_glade_spawn(at(k));
  return world_place_at(k, s, p.x, p.y, p.z, everglade_glade_face(at(k)), out);
}

bool world_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Everglade *g = world[k];
  coax_t_Player p = seat_p[k][s];
  const float dt = 1.0f / WORLD_HZ, frame = dt;
  /* in: SEQ KX KY PX PY THETA B (sticks -100..100, the heading in WIRE_FINE units (0.1 mrad), bits 1 run 2 jump 4 dive 8 rise 16 sink) */
  coax_t_EgMoveIn input = {(float)in[1] / 100.0f, (float)in[2] / 100.0f, (float)in[3] / 100.0f, (float)in[4] / 100.0f,
                           (in[6] & 1) != 0, (in[6] & 2) != 0, (float)in[5] / 10000.0f};
  everglade_room_seat(at(k), s);   /* its conditions, slow and throw are what the step reads */
  everglade_room_rise(at(k), s, (in[6] & 8) != 0, (in[6] & 16) != 0, p.pos.x, p.pos.z, p.pos.y);   /* its Levitate's keys */
  coax_t_Swim sw = seat_swim[k][s];
  /* in a rowboat: W and S row, A and D turn (a stick too), and it sits where the boat takes it, as the
     page's (everglade5e-face.c); the boat steps with the room's floats (world_floats) */
  int boat = river[k] && seat_boat[k][s] > 0 && everglade_river_crew_of(rv_of(k), seat_boat[k][s] - 1) == s + 1 ? seat_boat[k][s] - 1 : -1;
  coax_t_Seat seat = {0};
  if (boat >= 0) {
    everglade_river_crew_row(rv_of(k), boat, input.key_y + input.pad_y, input.key_x + input.pad_x);
    seat = everglade_river_crew_seat(rv_of(k), boat);
    seat_aboard(k, s); sw = seat_swim[k][s];
  }
  coax_t_EgMoveWorld mw = everglade_move_world(at(k), p.pos.x, p.pos.y, p.pos.z, sw.medium, sw.pace, sw.flow_x, sw.flow_z, boat >= 0, seat.x, seat.z, seat.yaw, (coax_v_f32){NULL, 0});
  input.link_flags = (int64_t)in[6] & 480;
  mw = eg_step_world(mw, input);
  p = eg_step_begin(p, input, mw, dt);
  coax_t_EgBody before = eg_step_body(p);
  coax_v_f32 near = everglade_move_bodies(at(k));
  static float wall_near[WALL_MAX * 6];
  int wn = walls_near(g->walls.e, (int)mw.walls, p.pos.x, p.pos.z, wall_near);
  p = eg_step_advance(p, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){wall_near, wn * 6}, (coax_v_f32){g->floors.e, mw.floors * 10}, frame);
  coax_t_EgBody moved = everglade_move_after(at(k), mw, before, eg_step_body(p), dt);
  p = eg_step_put(p, moved);
  /* the river after the step: the water this seat is in now, for its next step */
  if (river[k] && boat < 0) {
    coax_t_River *r = river[k];
    seat_water *me = &seat_w[k][s];
    r->medium = me->medium; r->feet = me->feet; r->afloat = me->afloat; r->breath = me->breath; r->out = me->out; r->levels = me->levels;
    r->gills = false; r->aboard = -1;
    river_now[k] += dt;
    if ((everglade_on_bridge(at(k), p.pos.x, p.pos.z) && sw.medium < 2) || everglade_parted(at(k), p.pos.x, p.pos.z)) {
      seat_swim[k][s] = (coax_t_Swim){0}; seat_swim[k][s].pace = 1; seat_swim[k][s].breath = 1; seat_swim[k][s].surface = -1000;
    } else {
      seat_swim[k][s] = everglade_river_swim_over(rv_of(k), (coax_v_f32){g->heights.e, (EG_GRID + 1) * (EG_GRID + 1)}, p.pos.x, p.pos.z,
                                                  p.pos.y + everglade_ice_lift(at(k), p.pos.x, p.pos.z), (in[6] & 4) != 0, dt, river_now[k]);
    }
    if (seat_swim[k][s].defeated) {   /* the sixth level of Exhaustion: it comes to on the nearer bank */
      p = verse_player_place(seat_swim[k][s].bank_x, 0, seat_swim[k][s].bank_z, verse_player_face(p)); seat_cut[k][s] = true;
      seat_swim[k][s].medium = 0; seat_swim[k][s].pace = 1;
    }
    *me = (seat_water){r->medium, r->feet, r->afloat, r->breath, r->out, r->levels};
  }
  seat_p[k][s] = p;
  pose_of(p, true, out);
  if (seat_swim[k][s].medium >= 2 && seat_swim[k][s].medium <= 3) out[7] = (float)(((int)out[7]) | 2);   /* G: swimming */
  if (boat >= 0) out[7] = (float)(((int)out[7]) | 4);   /* G: in a boat */
  return true;
}

float world_ground(int k, float x, float z) { return world_of(k) ? everglade_height(at(k), x, z) : 0.0f; }

void world_folk_hp(int k, int j, int hp, int temp, int by, int cond) {
  if (!world_of(k)) return;
  everglade_room_folk_hp(at(k), j, hp, temp, by);
  everglade_room_folk_cond(at(k), j, cond);
}
void world_folk_save(int k, int j, int ab, int seat, bool saved) { if (world_of(k)) everglade_room_folk_save(at(k), j, ab, seat, saved); }
void world_crime(int k, int by, float x, float z) { if (world_of(k)) (void)everglade_room_crime(at(k), by, x, z); }
void world_town(int k, float dt, double world_s, const float seats[WORLD_SEATS * WORLD_SEAT_TOWN], float took[2 * WORLD_SEATS]) {
  if (!world_of(k)) return;
  everglade_room_town(at(k), dt, world_s, (coax_v_f32){(float *)seats, WORLD_SEATS * WORLD_SEAT_TOWN}, WORLD_SEATS, (coax_v_f32){took, 2 * WORLD_SEATS});
}
int world_people(int k, float out[WORLD_FOLK * WORLD_PEOPLE]) {
  return world_of(k) ? (int)everglade_room_people(at(k), (coax_v_f32){out, WORLD_FOLK * WORLD_PEOPLE}) : 0;
}
bool world_view(int k, int i, float out[WORLD_VIEW]) { return world_of(k) && everglade_room_view(at(k), i, (coax_v_f32){out, WORLD_VIEW}); }
bool world_missile(int k, float out[WORLD_MISSILE]) { return world_of(k) && everglade_room_missile(at(k), (coax_v_f32){out, WORLD_MISSILE}); }
int world_wanted(int k, int s) { return world_of(k) ? (int)everglade_room_wanted(at(k), s) : 0; }
int world_gates(int k) { return world_of(k) ? (int)everglade_room_gates(at(k)) : 0; }
void world_seed_wanted(int k, int s, int level) { if (world_of(k)) everglade_room_seed_wanted(at(k), s, level); }
int world_cond(int k, int s) { return world_of(k) ? (int)everglade_room_cond(at(k), s) : 0; }
void world_hold(int k, int s) { if (world_of(k)) everglade_room_hold(at(k), s); }
void world_dice(int k, bool fixed) { if (world_of(k)) everglade_room_dice(at(k), fixed); }
float world_cond_left(int k, int s, int cause) { return world_of(k) ? everglade_room_cond_left(at(k), s, cause) : 0.0f; }

int world_effect_free(int k,int source) { return world_of(k) ? (int)everglade_room_effect_free(at(k),source) : 0; }
bool world_strike(int k, int source, int by, int slot, const float from[3], const float to[3], int seed) {
  if (!world_of(k)) return false;
  /* the Warlock's Dimension Door (room-everglade.coax EG_WL_DOOR): the seat stands where it teleported */
  if (slot == 69 && by >= 0 && by < WORLD_SEATS) {
    seat_ashore(k, by);
    seat_p[k][by] = verse_player_place(to[0], to[1], to[2], verse_player_face(seat_p[k][by])); seat_cut[k][by] = true;
    if (river[k]) { seat_dry(k, by); seat_w[k][by].breath = river[k]->breath; }
    return true;
  }
  return everglade_room_strike_try(at(k),source,by,slot,from[0],from[1],from[2],to[0],to[1],to[2],seed);
}
int world_spells(int k, float dt) { return world_of(k) ? (int)everglade_room_spells(at(k), dt) : 0; }
bool world_tree(int k, int q, float out[WORLD_TREE]) { return world_of(k) && everglade_room_tree(at(k), q, (coax_v_f32){out, WORLD_TREE}); }
bool world_event(int k, int q, float out[WORLD_EVENT]) { return world_of(k) && everglade_room_event(at(k), q, (coax_v_f32){out, WORLD_EVENT}); }
bool world_sight(int k, const float from[3], const float to[3]) { return !world_of(k) || everglade_room_sight(at(k), from[0], from[1], from[2], to[0], to[1], to[2]); }
void world_fresh(int k) { if (world_of(k)) everglade_room_fresh(at(k)); }
/* Seat s uses door i or talks to townsperson i where the room has them (everglade_room_door, everglade_room_talk). */
bool world_use(int k, int kind, int s, int i) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  float x = seat_p[k][s].pos.x, z = seat_p[k][s].pos.z;
  if (kind == 2) return everglade_room_door(at(k), i, x, z);      /* WE_DOOR */
  if (kind == 1) return everglade_room_talk(at(k), s, i, x, z);   /* WE_TALK */
  if (kind == 3 && river[k]) {                                       /* WE_BOAT: into a free boat in reach, or out of yours */
    int b = seat_boat[k][s] - 1;
    if (b >= 0) {
      coax_t_Seat off = everglade_river_crew_leave(rv_of(k), heights_of(k), river_now[k], b);
      seat_boat[k][s] = 0;
      if (off.on) seat_p[k][s] = verse_player_place(off.x, 0, off.z, off.yaw);
      seat_cut[k][s] = true;
      seat_dry(k, s); seat_w[k][s].breath = river[k]->breath;
      return true;
    }
    int near = everglade_river_crew_near(rv_of(k), x, z);
    if (near < 0 || (i >= 0 && near != i) || !everglade_river_crew_board(rv_of(k), near, s + 1)) return false;
    seat_boat[k][s] = near + 1;
    seat_aboard(k, s);
    seat_cut[k][s] = true;   /* onto the thwart in one move */
    return true;
  }
  return false;
}
/* The room's floats one tick on (ducks, pads, the rowed and moored boats), once a tick before the steps. */
void world_floats(int k, float dt) {
  if (!world_of(k) || !river[k]) return;
  floats_n[k] = (int)everglade_river_floats(rv_of(k), dt, (coax_v_f32){floats_out[k], WORLD_FLOATS * 8});
}
/* The n-th rowboat as the last tick left it: x y z yaw, its rower (a seat, -1 none), the oars' stroke (-1 shipped). */
bool world_boat(int k, int n, float out[WORLD_BOAT]) {
  if (!world_of(k) || !river[k]) return false;
  for (int i = 0, m = 0; i < floats_n[k]; i++) {
    const float *f = floats_out[k] + i * 8;
    if ((int)f[7] != 2) continue;
    if (m++ != n) continue;
    out[0] = f[0]; out[1] = f[1]; out[2] = f[2]; out[3] = f[3];
    out[4] = (float)(everglade_river_crew_of(rv_of(k), i) - 1); out[5] = f[6]; out[6] = (float)i;
    return true;
  }
  return false;
}
/* Seat s on the bank beside the n-th rowboat, in reach to board it (everglade_river_beside), facing along it: the
   boat check's seats at a pond (room-control.coax "place", #12861). Its next step starts there. */
bool world_place_boat(int k, int s, int n) {
  float bt[WORLD_BOAT];
  if (!world_boat(k, n, bt) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Seat at_bank = everglade_river_beside(rv_of(k), heights_of(k), river_now[k], (int64_t)bt[6]);
  if (!at_bank.on) return false;
  seat_ashore(k, s);
  seat_p[k][s] = verse_player_place(at_bank.x, at_bank.y, at_bank.z, at_bank.yaw); seat_cut[k][s] = true;
  seat_dry(k, s); seat_w[k][s].breath = river[k]->breath;
  return true;
}
/* Seat s, gone from the room: out of any boat it rowed. */
void world_gone(int k, int s) { if (world_of(k) && s >= 0 && s < WORLD_SEATS) seat_ashore(k, s); }
void world_seat_owner(int k, int s, uint64_t owner) { if(world_of(k)) everglade_room_seat_owner(at(k),s,owner); }
int world_doors(int k, int w) { return world_of(k) ? (int)everglade_room_doors(at(k), w) : 0; }
void world_rebuild(int k) { if (world_of(k)) everglade_room_rebuild(at(k)); }
int world_pieces(int k) { return world_of(k) ? (int)everglade_room_pieces(at(k)) : 0; }
bool world_piece_up(int k, int i) { return world_of(k) && everglade_room_piece_up(at(k), i); }

void world_ability(int k, const float c[WORLD_CAST]) {
  if (world_of(k)) everglade_room_ability_mode(at(k), (int)c[0], (int)c[1], c[2], c[3], c[4], c[5], c[6] > 0.5f, c[7], c[8], c[9], (int)c[10]);
}
void world_abilities(int k, float dt) {
  if (!world_of(k)) return;
  /* each seat's copy runs where the seat stands */
  for (int s = 0; s < WORLD_SEATS; s++) {
    coax_t_Player p = seat_p[k][s];
    everglade_room_ab_pose(at(k), s, p.pos.x, p.pos.y, p.pos.z, verse_player_face(p));
  }
  everglade_room_abilities(at(k), dt);
}
bool world_life(int k, int s, int ab, float out[WORLD_LIFE]) { return world_of(k) && everglade_room_life(at(k), s, ab, (coax_v_f32){out, WORLD_LIFE}); }
float world_air(int k, int i) { return world_of(k) ? everglade_room_air(at(k), i) : 0.0f; }
int world_hurts(int k) { return world_of(k) ? (int)everglade_room_hurts(at(k)) : 0; }
bool world_hurt(int k, int q, float out[3]) { return world_of(k) && everglade_room_hurt(at(k), q, (coax_v_f32){out, 3}); }

/* The drawing host's calls, which grid.coax's thumbs make and a headless world never does. */
void host_line(int64_t h, double ax, double ay, double bx, double by, double la, double lb, double w) { (void)h; (void)ax; (void)ay; (void)bx; (void)by; (void)la; (void)lb; (void)w; }
void host_dot(int64_t h, double x, double y, double size, double light, double soft, double crisp) { (void)h; (void)x; (void)y; (void)size; (void)light; (void)soft; (void)crisp; }
void host_slot(int64_t h, double x, double y, double w, double hh, double press, double flash, double sel, double icon) { (void)h; (void)x; (void)y; (void)w; (void)hh; (void)press; (void)flash; (void)sel; (void)icon; }

/* Readiness (#12797): the realm's published world is laid (world_edits), and a room's world builds from
   it: slot 0's (kept for the room that next takes it), its town peopled and its ground finite. */
bool world_ready(void) {
  static bool ready;
  if (ready) return true;
  if (!edits_text || room_catalog_n <= 0 || !world_of(0) || !river[0] || !pieces[0]) return false;
  static float people[WORLD_FOLK * WORLD_PEOPLE];
  float g = world_ground(0, 0.0f, 0.0f);
  ready = world_people(0, people) > 0 && g == g && g > -1000.0f && g < 1000.0f;
  return ready;
}

/* ---- Everglade II's rooms (#13053; apps/openagents-web/everglade2-move.coax): its land, path and woods,
   one planting for every room (a pure function of the land), and each seat a controller stepped as the
   page steps it (everglade2-face.c e2_step_once): the world round it, Everglade II's pace, the
   controller's two halves over the trunks and rocks, then kept where it may stand. No town, river,
   watch or pieces: room-tick.c's Everglade II tick asks none of them. */
static coax_t_E2MoveGround *e2_ground;
static coax_v_t_E2MoveGround e2mg(void) { return (coax_v_t_E2MoveGround){e2_ground, 1}; }
bool world2_prepare(void) {
  if (!e2_ground) {
    e2_ground = calloc(1, sizeof *e2_ground);
    if (!e2_ground) return false;
    /* every field of E2MoveGround is zero at its default: calloc is its default */
  }
  e2mg_plant(e2mg());
  return e2_ground->done;
}
bool world2_contact(const float from[3], const float to[3]) {
  if (!world2_prepare()) return false;
  coax_t_EgMoveWorld w = e2mg_move_world(e2mg(), from[0], from[2], false);
  coax_v_f32 bodies = e2mg_bodies(e2mg());
  return eg_contact_clear((coax_v_f32){bodies.data, w.bodies * 7}, from[0], from[1], from[2], to[0], to[1], to[2]);
}
/* at (x, z, facing): where the room puts the seat back (room-everglade.coax rm_e2_place, #13057); NULL the spawn */
bool world2_place(int k, int s, const float *at, float out[WORLD_POSE]) {
  if (k < 0 || k >= WORLD_ROOMS || s < 0 || s >= WORLD_SEATS || !world2_prepare()) return false;
  seat_p[k][s] = at ? verse_player_place(at[0], 0, at[1], at[2]) : verse_player_place(e2mg_spawn_x(), 0, e2mg_spawn_z(), e2mg_spawn_face()); seat_cut[k][s] = true;
  pose_of(seat_p[k][s], false, out);
  return true;
}
bool world2_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]) {
  if (k < 0 || k >= WORLD_ROOMS || s < 0 || s >= WORLD_SEATS || !world2_prepare()) return false;
  const float dt = 1.0f / WORLD_HZ;
  /* in: SEQ KX KY PX PY THETA B, as world_step reads them */
  coax_t_EgMoveIn input = {(float)in[1] / 100.0f, (float)in[2] / 100.0f, (float)in[3] / 100.0f, (float)in[4] / 100.0f,
                           (in[6] & 1) != 0, (in[6] & 2) != 0, (float)in[5] / 10000.0f};
  coax_t_Player p = seat_p[k][s];
  /* the same order as the page's step (everglade2-face.c e2_step_once) */
  float fx = p.pos.x, fz = p.pos.z;
  coax_t_EgMoveWorld mw = e2mg_move_world(e2mg(), p.pos.x, p.pos.z, input.run);
  p = eg_step_begin(p, input, mw, dt);
  coax_v_f32 near = e2mg_bodies(e2mg());
  p = eg_step_advance(p, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){0, 0}, (coax_v_f32){0, 0}, dt);
  coax_t_E2Spot kept = e2m_keep(p.pos.x, p.pos.z, fx, fz);
  if (kept.x != p.pos.x || kept.z != p.pos.z) p = verse_player_constrain(p, kept.x, kept.z);
  seat_p[k][s] = p;
  pose_of(p, true, out);
  return true;
}
