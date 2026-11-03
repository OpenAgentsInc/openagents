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
#endif
