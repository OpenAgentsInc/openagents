/* The room server's Everglade worlds (room-world.c), in plain numbers for room-host.c. */
#ifndef ROOM_WORLD_H
#define ROOM_WORLD_H
#include <stdbool.h>
#include <stdint.h>
#define WORLD_ROOMS 16   /* room-everglade.coax EG_ROOMS */
#define WORLD_SEATS 16   /* EG_SEATS */
#define WORLD_HZ 30      /* EG_HZ */
#define WORLD_TREES 192
#define WORLD_TREE 16
bool world_tree(int k, int q, float out[WORLD_TREE]);
#define WORLD_IN 7       /* EG_IN_FIELDS: SEQ KX KY PX PY THETA B */
#define WORLD_POSE 10    /* X Y Z YAW VX VY VZ G CLIP CARRY */
/* Seat s of room k stands at the gate: its pose. */
/* The realm's published world (its JSON answer), laid over every room's plan and each new room's. */
void world_edits(const char *text, int64_t n);
/* readiness: the published world laid and a room's world built from it (#12797) */
bool world_ready(void);
bool world_prepared(int k);
bool world_prepare(int k);
bool world_place(int k, int s, float out[WORLD_POSE]);
/* seat s of room k where its player stood (x, y, z, facing yaw), as world_place does at the gate */
bool world_place_at(int k, int s, float x, float y, float z, float yaw, float out[WORLD_POSE]);
/* seat s of room k at the glade's clearing, where Character Create stands you (apps/openagents-web/everglade-glade.coax) */
bool world_place_glade(int k, int s, float out[WORLD_POSE]);
/* Seat s of room k steps one input (1 / WORLD_HZ s): its pose after it. */
bool world_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]);
/* the rowboats (#12861): the room's floats a tick on; the n-th boat (WORLD_BOAT: x y z yaw rower oars float); a seat gone */
#define WORLD_BOAT 7
void world_floats(int k, float dt);
bool world_boat(int k, int n, float out[WORLD_BOAT]);
void world_gone(int k, int s);
void world_seat_owner(int k, int s, uint64_t owner);
/* seat s of room k on the bank beside its n-th rowboat (room-control.coax "place"): false none there */
bool world_place_boat(int k, int s, int n);
/* whether seat s of room k moved in one jump since last asked (placed, teleported, into or out of a boat), once */
bool world_cut_take(int k, int s);
/* The ground's height at (x, z) in room k's world. */
float world_ground(int k, float x, float z);
bool world_forge_open(int k, float x, float z);
void world_forge_prepare(int k);
void world_forge_step(int k, const float in[6], float out[5]);
/* The town (everglade-watch.coax everglade_room_town, GitHub #12713). */
#define WORLD_FOLK 47          /* watch.coax WATCH_ACTORS, everglade5e-rules.coax E5R_FOLK */
#define WORLD_SEAT_TOWN 7      /* a seat for the town: X Y Z LIVE HP MAX COND */
#define WORLD_PEOPLE 8         /* a person for the rules: HERE X Z HP TEMP MAX AC BLOCK */
#define WORLD_VIEW 12          /* a person for the pages: X Z YAW CLIP MODE HP SHOWN CASTING KIND FOE RANK MAX */
#define WORLD_MISSILE 11       /* a watch spell in the air: KIND CASTER FOE AX AY AZ BX BY BZ FLIGHT LIFE */
/* Person j's Hit Points and conditions as the rules have them, harmed by seat `by` (-1 none). */
void world_folk_hp(int k, int j, int hp, int temp, int by, int cond);
/* Person j's save the rules rolled against seat `seat`'s ability `ab`: that ability's copy takes it (#12874). */
void world_folk_save(int k, int j, int ab, int seat, bool saved);
/* Seat `by` harmed someone of the town at (x, z): a crime. */
void world_crime(int k, int by, float x, float z);
/* One tick of room k's town at the shared clock `world_s` (s): took[s], what its guards took from seat s,
   took[WORLD_SEATS + s] who did (a watch actor, -1 none). */
void world_town(int k, float dt, double world_s, const float seats[WORLD_SEATS * WORLD_SEAT_TOWN], float took[2 * WORLD_SEATS]);
/* The people for the rules (WORLD_PEOPLE each): how many. */
int world_people(int k, float out[WORLD_FOLK * WORLD_PEOPLE]);
/* Person i for the pages; seat s's WANTED level. */
bool world_view(int k, int i, float out[WORLD_VIEW]);
/* The next watch spell that left its caster's hands since the last asked (false: none). */
bool world_missile(int k, float out[WORLD_MISSILE]);
int world_wanted(int k, int s);
int world_gates(int k);                           /* 1 while the town's gates are shut (WANTED 4, #12794) */
void world_seed_wanted(int k, int s, int level);   /* a seat back WANTED at its saved level (#12794) */
/* Seat s's conditions from the watch (wire WC_*: Hold Person's hold, Ray of Frost's slow). */
int world_cond(int k, int s);
void world_hold(int k, int s);
void world_dice(int k, bool fixed);   /* the world's dice fixed as the room's rules' are (room-control.coax "dice fixed") */   /* seat s held by the watch's Hold Person, 3 s (a scripted hold, room-control.coax) */
float world_cond_left(int k, int s, int cause);   /* seconds left of seat s's watch effect by cause (WE_*), 0 none */
/* The town's pieces: a strike of Everglade's four spells from seat `by`; dt of the strikes landing and
   fire, the count of its events; event q: KIND PIECE A B C X Z (KIND 1 a break thrown at A B C m/s, 2
   alight: A 1 on); the rebuild. */
#define WORLD_EVENT 7
bool world_strike(int k, int source, int by, int slot, const float from[3], const float to[3], int seed);
int world_effect_free(int k,int source);
int world_spells(int k, float dt);
bool world_event(int k, int q, float out[WORLD_EVENT]);
int world_pieces(int k);              /* the town's pieces, and whether piece i stands whole (a Repair's) */
bool world_piece_up(int k, int i);
void world_rebuild(int k);
/* The hotbar abilities the world runs (apps/openagents-web/everglade-room-abilities.coax, #12718, #12733),
   each seat its own copy: a cast or an end, BY AB X Y Z YAW ON AX AY AZ (room-abilities.coax rm_ra_cast);
   a step (every seat's copy where the seat stands); seat s's ability ab, ON X Y Z YAW EPOCH; how high
   person i is; the falls' damage, q's PERSON AMOUNT SEAT. */
#define WORLD_CAST 11
#define WORLD_LIFE 6
#define WORLD_ABS 48
void world_ability(int k, const float c[WORLD_CAST]);
void world_abilities(int k, float dt);
bool world_life(int k, int s, int ab, float out[WORLD_LIFE]);
float world_air(int k, int i);
int world_hurts(int k);
bool world_hurt(int k, int q, float out[3]);
/* Whether a caster with feet at `from` sees `to` in room k's world: its standing pieces and ground
   (apps/openagents-web/everglade-cast-sight.coax, GitHub #12790). */
bool world_sight(int k, const float from[3], const float to[3]);
/* A new room in slot k: its town whole and nobody known. */
void world_fresh(int k);
bool world_use(int k, int kind, int s, int i);   /* seat s uses i (WE_DOOR: opens or shuts door i; WE_TALK: talks to spot i, -1 lets go): false when it can't */
int world_doors(int k, int w);          /* the doors open, word w of them (WK_DOORS) */
/* Everglade II's rooms (#13053): one planting for all, each seat placed at the glade and stepped on its land */
bool world2_prepare(void);
bool world2_place(int k, int s, float out[WORLD_POSE]);
bool world2_step(int k, int s, const int64_t in[WORLD_IN], float out[WORLD_POSE]);
#endif
