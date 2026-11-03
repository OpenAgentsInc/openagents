/* The room server's Everglade worlds (room-world.c), in plain numbers for room-host.c. */
#ifndef ROOM_WORLD_H
#define ROOM_WORLD_H
#include <stdbool.h>
#include <stdint.h>
#define WORLD_ROOMS 16   /* room-everglade.coax EG_ROOMS */
#define WORLD_SEATS 16   /* EG_SEATS */
#define WORLD_HZ 30      /* EG_HZ */
#define WORLD_IN 7       /* EG_IN_FIELDS: SEQ KX KY PX PY THETA B */
#define WORLD_POSE 10    /* X Y Z YAW VX VY VZ G CLIP CARRY */
/* Seat s of room k stands at the gate: its pose. */
bool world_place(int k, int s, float out[WORLD_POSE]);
/* Seat s of room k steps one input (1 / WORLD_HZ s): its pose after it. */
bool world_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]);
/* The ground's height at (x, z) in room k's world. */
float world_ground(int k, float x, float z);
/* The town (everglade-watch.coax everglade_room_town, GitHub #12713). */
#define WORLD_FOLK 38          /* watch.coax WATCH_ACTORS, everglade5e-rules.coax E5R_FOLK */
#define WORLD_SEAT_TOWN 7      /* a seat for the town: X Y Z LIVE HP MAX COND */
#define WORLD_PEOPLE 8         /* a person for the rules: HERE X Z HP TEMP MAX AC BLOCK */
#define WORLD_VIEW 7           /* a person for the pages: X Z YAW CLIP MODE HP SHOWN */
/* Person j's Hit Points as the rules have them, harmed by seat `by` (-1 none). */
void world_folk_hp(int k, int j, int hp, int temp, int by);
/* Seat `by` harmed someone of the town at (x, z): a crime. */
void world_crime(int k, int by, float x, float z);
/* One tick of room k's town at the shared clock `world_s` (s): took[s], what its guards took from seat s,
   took[WORLD_SEATS + s] who did (a watch actor, -1 none). */
void world_town(int k, float dt, double world_s, const float seats[WORLD_SEATS * WORLD_SEAT_TOWN], float took[2 * WORLD_SEATS]);
/* The people for the rules (WORLD_PEOPLE each): how many. */
int world_people(int k, float out[WORLD_FOLK * WORLD_PEOPLE]);
/* Person i for the pages; seat s's WANTED level. */
bool world_view(int k, int i, float out[WORLD_VIEW]);
int world_wanted(int k, int s);
/* Seat s's conditions from the watch (wire WC_*: Hold Person's hold, Ray of Frost's slow). */
int world_cond(int k, int s);
/* The town's pieces: a strike of Everglade's four spells from seat `by`; dt of the strikes landing and
   fire, the count of its events; event q: KIND PIECE A B C X Z (KIND 1 a break thrown at A B C m/s, 2
   alight: A 1 on); the rebuild. */
#define WORLD_EVENT 7
void world_strike(int k, int by, int slot, const float from[3], const float to[3], int seed);
int world_spells(int k, float dt);
bool world_event(int k, int q, float out[WORLD_EVENT]);
void world_rebuild(int k);
/* The hotbar abilities the world runs (apps/openagents-web/everglade-room-abilities.coax, #12718): a cast
   or an end, BY AB X Y Z YAW ON (room-abilities.coax rm_ra_cast); a step; Reverse Gravity's cylinder ON
   X Y Z T BY; how high person i is; the falls' damage, q's PERSON AMOUNT SEAT. */
#define WORLD_CAST 8
#define WORLD_RG 6
void world_ability(int k, const float c[WORLD_CAST]);
void world_abilities(int k, float dt);
bool world_rg(int k, float out[WORLD_RG]);
float world_air(int k, int i);
int world_hurts(int k);
bool world_hurt(int k, int q, float out[3]);
/* A new room in slot k: its town whole and nobody known. */
void world_fresh(int k);
#endif
