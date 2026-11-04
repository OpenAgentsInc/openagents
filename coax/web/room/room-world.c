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
#include <stdlib.h>
#include <string.h>
#include "everglade-step.h"
#include "river.h"
#include "room-world.h"
#include <stdlib.h>
#include <string.h>

static coax_t_Everglade *world[WORLD_ROOMS];
static coax_t_DemoWorld *pieces[WORLD_ROOMS];
extern const int room_catalog_n;               /* room-catalog.c (room-catalog.py): the pack's catalog */
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
/* the ground's grid as the page's (everglade5e-face.c): EG_GRID cells of 2 m a side from -EG_HALF */
#define EG_GRID 176
#define EG_HALF 176
static coax_v_t_River rv_of(int k) { return (coax_v_t_River){river[k], 1}; }
static void seat_dry(int k, int s) {
  seat_w[k][s] = (seat_water){0, 0, false, 0, 0, 0};
  seat_swim[k][s] = (coax_t_Swim){0}; seat_swim[k][s].pace = 1; seat_swim[k][s].breath = 1; seat_swim[k][s].surface = -1000;
}

static coax_v_t_Everglade at(int k) { return (coax_v_t_Everglade){world[k], 1}; }

/* Room k's world, made (and its city built) the first time; kept for the next room in that slot. */
/* The realm's published world (an admin's placed things: GET /api/playtest/world/everglade-5e), laid over
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
    if (pieces[k])
      everglade_room_spells_start(at(k), (coax_v_t_DemoWorld){pieces[k], 1}, (coax_string){(const char *)room_catalog, room_catalog_n});
    if (edits_text) everglade_world(at(k), (coax_string){edits_text, edits_n});
  }
  return true;
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
  out[7] = p.on_ground ? 1 : 0;            /* G: 1 on the ground, 2 swimming (world_step adds it), 4 in a boat (not yet) */
  out[8] = (float)p.act;                   /* CLIP: the controller's act (aethyr/walk.coax Act) */
  out[9] = (float)(p.clock.carry_ns / 100000);   /* CARRY: the fixed step's leftover, tenths of a ms */
  (void)moved;
}

bool world_place(int k, int s, float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Point spawn = everglade_spawn(at(k));
  seat_p[k][s] = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(k)));
  if (river[k]) { seat_dry(k, s); seat_w[k][s].breath = river[k]->breath; }
  pose_of(seat_p[k][s], false, out);
  return true;
}

bool world_place_at(int k, int s, float x, float y, float z, float yaw, float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  if (!(x == x) || !(y == y) || !(z == z) || !(yaw == yaw)) return world_place(k, s, out);
  seat_p[k][s] = verse_player_place(x, y, z, yaw);
  if (river[k]) { seat_dry(k, s); seat_w[k][s].breath = river[k]->breath; }
  pose_of(seat_p[k][s], false, out);
  return true;
}

bool world_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]) {
  if (!world_of(k) || s < 0 || s >= WORLD_SEATS) return false;
  coax_t_Everglade *g = world[k];
  coax_t_Player p = seat_p[k][s];
  const float dt = 1.0f / WORLD_HZ, frame = dt;
  /* in: SEQ KX KY PX PY THETA B (cm-scaled sticks, centiradians, bits 1 run 2 jump 4 dive 8 rise 16 sink) */
  coax_t_EgMoveIn input = {(float)in[1] / 100.0f, (float)in[2] / 100.0f, (float)in[3] / 100.0f, (float)in[4] / 100.0f,
                           (in[6] & 1) != 0, (in[6] & 2) != 0, (float)in[5] / 100.0f};
  everglade_room_seat(at(k), s);   /* its conditions, slow and throw are what the step reads */
  everglade_room_rise(at(k), s, (in[6] & 8) != 0, (in[6] & 16) != 0, p.pos.x, p.pos.z, p.pos.y);   /* its Levitate's keys */
  coax_t_Swim sw = seat_swim[k][s];
  coax_t_EgMoveWorld mw = everglade_move_world(at(k), p.pos.x, p.pos.y, p.pos.z, sw.medium, sw.pace, sw.flow_x, sw.flow_z, false, 0, 0, 0, (coax_v_f32){NULL, 0});
  p = eg_step_begin(p, input, mw, dt);
  coax_t_EgBody before = eg_step_body(p);
  coax_v_f32 near = everglade_move_bodies(at(k));
  static float wall_near[WALL_MAX * 6];
  int wn = walls_near(g->walls.e, (int)mw.walls, p.pos.x, p.pos.z, wall_near);
  p = eg_step_advance(p, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){wall_near, wn * 6}, (coax_v_f32){g->floors.e, mw.floors * 10}, frame);
  coax_t_EgBody moved = everglade_move_after(at(k), mw, before, eg_step_body(p), dt);
  p = eg_step_put(p, moved);
  /* the river after the step: the water this seat is in now, for its next step */
  if (river[k]) {
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
      p = verse_player_place(seat_swim[k][s].bank_x, 0, seat_swim[k][s].bank_z, verse_player_face(p));
      seat_swim[k][s].medium = 0; seat_swim[k][s].pace = 1;
    }
    *me = (seat_water){r->medium, r->feet, r->afloat, r->breath, r->out, r->levels};
  }
  seat_p[k][s] = p;
  pose_of(p, true, out);
  if (seat_swim[k][s].medium >= 2 && seat_swim[k][s].medium <= 3) out[7] = (float)(((int)out[7]) | 2);   /* G: swimming */
  return true;
}

float world_ground(int k, float x, float z) { return world_of(k) ? everglade_height(at(k), x, z) : 0.0f; }

void world_folk_hp(int k, int j, int hp, int temp, int by) { if (world_of(k)) everglade_room_folk_hp(at(k), j, hp, temp, by); }
void world_crime(int k, int by, float x, float z) { if (world_of(k)) (void)everglade_room_crime(at(k), by, x, z); }
void world_town(int k, float dt, double world_s, const float seats[WORLD_SEATS * WORLD_SEAT_TOWN], float took[2 * WORLD_SEATS]) {
  if (!world_of(k)) return;
  everglade_room_town(at(k), dt, world_s, (coax_v_f32){(float *)seats, WORLD_SEATS * WORLD_SEAT_TOWN}, WORLD_SEATS, (coax_v_f32){took, 2 * WORLD_SEATS});
}
int world_people(int k, float out[WORLD_FOLK * WORLD_PEOPLE]) {
  return world_of(k) ? (int)everglade_room_people(at(k), (coax_v_f32){out, WORLD_FOLK * WORLD_PEOPLE}) : 0;
}
bool world_view(int k, int i, float out[WORLD_VIEW]) { return world_of(k) && everglade_room_view(at(k), i, (coax_v_f32){out, WORLD_VIEW}); }
int world_wanted(int k, int s) { return world_of(k) ? (int)everglade_room_wanted(at(k), s) : 0; }
int world_cond(int k, int s) { return world_of(k) ? (int)everglade_room_cond(at(k), s) : 0; }

void world_strike(int k, int by, int slot, const float from[3], const float to[3], int seed) {
  if (!world_of(k)) return;
  /* the Warlock's Dimension Door (room-everglade.coax EG_WL_DOOR): the seat stands where it teleported */
  if (slot == 69 && by >= 0 && by < WORLD_SEATS) {
    seat_p[k][by] = verse_player_place(to[0], to[1], to[2], verse_player_face(seat_p[k][by]));
    if (river[k]) { seat_dry(k, by); seat_w[k][by].breath = river[k]->breath; }
    return;
  }
  everglade_room_strike(at(k), by, slot, from[0], from[1], from[2], to[0], to[1], to[2], seed);
}
int world_spells(int k, float dt) { return world_of(k) ? (int)everglade_room_spells(at(k), dt) : 0; }
bool world_event(int k, int q, float out[WORLD_EVENT]) { return world_of(k) && everglade_room_event(at(k), q, (coax_v_f32){out, WORLD_EVENT}); }
void world_fresh(int k) { if (world_of(k)) everglade_room_fresh(at(k)); }
void world_rebuild(int k) { if (world_of(k)) everglade_room_rebuild(at(k)); }

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
