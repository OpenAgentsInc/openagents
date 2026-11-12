/* The room server's Everglade world (room-world.c), in plain numbers for room-host.c. Every room is
   Everglade II's (docs/decisions/0005-production-room-runs-everglade-ii.md, #13267). */
#ifndef ROOM_WORLD_H
#define ROOM_WORLD_H
#include <stdbool.h>
#include <stdint.h>
#define WORLD_ROOMS 16   /* everglade/server/seat.coax EG_ROOMS */
#define WORLD_SEATS 16   /* EG_SEATS */
#define WORLD_HZ 30      /* EG_HZ */
#define WORLD_IN 7       /* EG_IN_FIELDS: SEQ KX KY PX PY THETA B */
#define WORLD_POSE 10    /* X Y Z YAW VX VY VZ G CLIP CARRY */
/* The realm's published world (its JSON answer): kept as the host's readiness signal. */
void world_edits(const char *text, int64_t n);
/* readiness: the published world laid and Everglade II's land planted (#12797) */
bool world_ready(void);
/* seat s of room k moved in one jump since asked: its next row is a WARP (#12835) */
bool world_cut_take(int k, int s);
/* Everglade II's rooms (#13053): one planting for all, each seat placed at the glade and stepped on its land */
bool world2_contact(const float from[3], const float to[3]);
bool world2_prepare(void);
/* the planting world2_prepare made (an E2MoveGround), which the room's arrows fly among too (everglade/server/arrows.coax rm_e2_land) */
void *world2_ground(void);
bool world2_place(int k, int s, const float *at, float out[WORLD_POSE]);
bool world2_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]);
void world2_guard(int k, bool on, float x, float z);
int world2_near(float x, float z, float *out, int cap);
#endif
