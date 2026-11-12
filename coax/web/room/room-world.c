/* The room server's Everglade world (GitHub #12705, #13053; apps/openagents-web/everglade2-move.coax):
   Everglade II's land, path and woods, one planting for every room, and each seat a controller
   (everglade-step.coax, the unit that carries aethyr/player.coax) stepped as the page steps it, so
   the page and the server put the player in the same place. Every room is Everglade II's
   (docs/decisions/0005-production-room-runs-everglade-ii.md, #13267): v1's city, town, river and
   pieces are the frozen build's (everglade/server/legacy/v1.sh). This file includes only the units'
   headers; room-host.c calls it with plain numbers. */
#include <stdlib.h>
#include <string.h>
#include "everglade-step.h"
#include "room.h"   /* Everglade II's movement world (apps/openagents-web/everglade2-move.coax, the room's unit) */
#include "room-world.h"

static coax_t_Player seat_p[WORLD_ROOMS][WORLD_SEATS];
/* Each seat moved in one jump since the room last asked (placed, teleported, come to on the bank, into
   or out of a boat): its next row is a WARP however short the jump (world_cut_take, #12835). */
static bool seat_cut[WORLD_ROOMS][WORLD_SEATS];
bool world_cut_take(int k, int s) {
  if (k < 0 || k >= WORLD_ROOMS || s < 0 || s >= WORLD_SEATS || !seat_cut[k][s]) return false;
  seat_cut[k][s] = false;
  return true;
}

static void pose_of(coax_t_Player p, bool moved, float out[WORLD_POSE]) {
  out[0] = p.pos.x; out[1] = p.pos.y; out[2] = p.pos.z; out[3] = verse_player_face(p);
  out[4] = p.vel.x; out[5] = p.vel.y; out[6] = p.vel.z;
  out[7] = p.on_ground ? 1 : 0;            /* G: 1 on the ground, 2 swimming, 4 in a boat (world_step adds them) */
  out[8] = (float)p.act;                   /* CLIP: the controller's act (aethyr/walk.coax Act) */
  out[9] = (float)(p.clock.carry_ns / 100000);   /* CARRY: the fixed step's leftover, tenths of a ms */
  (void)moved;
}

/* The realm's published world (#12731) is v1's city's: Everglade II's land is its own planting, so
   the text is only kept as the host's readiness signal. */
static bool edits_laid;
void world_edits(const char *text, int64_t n) { (void)text; edits_laid = n > 0; }
/* Readiness (#12797): the published world is laid and Everglade II's land is planted. */
bool world_ready(void) { return edits_laid && world2_prepare(); }

/* ---- Everglade II's rooms (#13053; apps/openagents-web/everglade2-move.coax): its land, path and woods,
   one planting for every room (a pure function of the land), and each seat a controller stepped as the
   page steps it (everglade2-face.c e2_step_once): the world round it, Everglade II's pace, the
   controller's two halves over the trunks and the rocks' hulls, then kept where it may stand. No town, river,
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
void *world2_ground(void) { return world2_prepare() ? (void *)e2_ground : NULL; }
bool world2_contact(const float from[3], const float to[3]) {
  if (!world2_prepare()) return false;
  coax_t_EgMoveWorld w = e2mg_move_world(e2mg(), from[0], from[2], false);
  coax_v_f32 bodies = e2mg_bodies(e2mg()), hulls = e2mg_hulls(e2mg());
  return eg_contact_clear_among((coax_v_f32){bodies.data, w.bodies * 7}, (coax_v_f32){hulls.data, w.hulls}, from[0], from[1], from[2], to[0], to[1], to[2]);
}
/* at (x, z, facing): where the room puts the seat back (room-everglade.coax rm_e2_place, #13057); NULL the spawn */
bool world2_place(int k, int s, const float *at, float out[WORLD_POSE]) {
  if (k < 0 || k >= WORLD_ROOMS || s < 0 || s >= WORLD_SEATS || !world2_prepare()) return false;
  seat_p[k][s] = at ? verse_player_place(at[0], 0, at[1], at[2]) : verse_player_place(e2mg_spawn_x(), 0, e2mg_spawn_z(), e2mg_spawn_face()); seat_cut[k][s] = true;
  pose_of(seat_p[k][s], false, out);
  return true;
}
/* The path guard's body this tick (everglade/server/guard.coax rm_e2_guard_body), which every seat's step walks round. */
static bool guard_on[WORLD_ROOMS]; static float guard_xz[WORLD_ROOMS][2];
void world2_guard(int k, bool on, float x, float z) { if (k >= 0 && k < WORLD_ROOMS) { guard_on[k] = on; guard_xz[k][0] = x; guard_xz[k][1] = z; } }
/* The trunks and round props near (x, z) as circles (x, z, radius each) into out, at most cap of them:
   what the guard steers round (everglade/server/guard.coax rm_e2_guard_near, for the shared steering service). */
int world2_near(float x, float z, float *out, int cap) {
  if (!world2_prepare()) return 0;
  coax_t_EgMoveWorld w = e2mg_move_world(e2mg(), x, z, false);
  coax_v_f32 bodies = e2mg_bodies(e2mg());
  int n = 0;
  for (int64_t q = 0; q < w.bodies && n < cap; q++, n++) {
    out[n * 3] = bodies.data[q * 7]; out[n * 3 + 1] = bodies.data[q * 7 + 2]; out[n * 3 + 2] = bodies.data[q * 7 + 6];
  }
  return n;
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
  if (guard_on[k]) mw.bodies = e2mg_guard(e2mg(), mw.bodies, p.pos.x, p.pos.z, guard_xz[k][0], guard_xz[k][1]);
  p = eg_step_begin(p, input, mw, dt);
  coax_v_f32 near = e2mg_bodies(e2mg()), hulls = e2mg_hulls(e2mg());
  p = eg_step_advance_among(p, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){0, 0}, (coax_v_f32){0, 0}, (coax_v_f32){hulls.data, mw.hulls}, dt);
  coax_t_E2Spot kept = e2m_keep(p.pos.x, p.pos.z, fx, fz);
  if (kept.x != p.pos.x || kept.z != p.pos.z) p = verse_player_constrain(p, kept.x, kept.z);
  seat_p[k][s] = p;
  pose_of(p, true, out);
  return true;
}
