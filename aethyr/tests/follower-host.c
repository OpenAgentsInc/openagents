/* Test driver only: data fixtures, calls to the actual Player/realm C
   boundaries, and byte serialization. All motion and clock arithmetic
   stays in the separately compiled production Coax units. */
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#if defined(FOLLOWER_GARDEN)
#include "garden.h"
typedef coax_t_Garden Scene;
#define scene_default coax_default_t_Garden
#define scene_walls garden_walls
#define scene_place garden_place_camera
static const float origin_x = -30, origin_z = -30;
static const char realm[] = "garden";
static coax_v_t_Garden scene_view(Scene *s) { return (coax_v_t_Garden){s, 1}; }
#else
#include "villa.h"
typedef coax_t_Villa Scene;
#define scene_default coax_default_t_Villa
#define scene_walls villa_walls
#define scene_place villa_place_camera
static const float origin_x = 30, origin_z = 30;
static const char realm[] = "hecates-cemetery";
static coax_v_t_Villa scene_view(Scene *s) { return (coax_v_t_Villa){s, 1}; }
#endif
#include "Player.h"

#if defined(__wasm__)
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static Scene scene;
static coax_t_Player player;
static coax_t_AdvanceTrace walked;
static float vertices[48000 * 16], uniforms[1024];
static uint32_t words[61];
static int64_t drawn_vertices;

EXPORT("follower_init") void follower_init(void) {
  scene_default(&scene);
  scene_walls(scene_view(&scene));
  for (size_t i = 0; i < sizeof scene.heights.e / sizeof(float); ++i) scene.heights.e[i] = 2.5f;
  scene.golden = true;
  scene_place(scene_view(&scene), verse_place(origin_x, 0, origin_z, 0, 1.234567f, 0.4f, 6, 0, -1, 1000));
  scene.fetch.dog = (coax_t_Follower){.pos = {origin_x, 2.5f, origin_z - 3}};
  scene.fetch.ball.held = 0;
  scene.fetch.ball.thrown = false;
  scene.follower_previous = (coax_t_FetchPose){.ready = true, .dog = scene.fetch.dog.pos,
    .face = scene.fetch.dog.face, .ball = scene.fetch.ball.pos, .spin = scene.fetch.ball.spin};
#if !defined(FOLLOWER_GARDEN)
  villa_on_realm_clock(scene_view(&scene));
#endif
  player = verse_player_input(verse_player_place(origin_x, 0, origin_z, 0), 1, 0, 0, 0, true, false, 0, 0);
  player.theta = 1.234567f;
  memset(&walked, 0, sizeof walked);
  drawn_vertices = 0;
}

static void draw_frame(int fault, float counted) {
  coax_t_Vec3 feet = verse_player_feet(player);
  coax_v_f32 out = {vertices, sizeof vertices / sizeof(float)};
  coax_v_f32 u = {uniforms, sizeof uniforms / sizeof(float)};
  coax_v_t_StepTarget targets = {walked.targets.e, walked.count};
#if defined(FOLLOWER_GARDEN)
  if (fault == 2) {
    drawn_vertices = garden_frame(scene_view(&scene), out, u, 1280, 800, 1, counted, true,
      feet.x, feet.y, feet.z, verse_player_face(player), verse_player_act(player), player.vel.x, player.vel.z,
      false, 0, 0, 0, 0);
  } else {
    drawn_vertices = garden_frame_on_clock(scene_view(&scene), out, u, 1280, 800, 1, player.clock, targets, true,
      feet.x, feet.y, feet.z, verse_player_face(player), verse_player_act(player), player.vel.x, player.vel.z,
      false, 0, 0, 0, 0);
  }
#else
  if (fault == 2) villa_step(scene_view(&scene), counted, feet.x, feet.y, feet.z, verse_player_face(player),
      verse_player_act(player), player.vel.x, player.vel.z);
  else villa_step_on_clock(scene_view(&scene), player.clock, targets, feet.x, feet.y, feet.z,
      verse_player_face(player), verse_player_act(player), player.vel.x, player.vel.z);
  drawn_vertices = villa_draw(scene_view(&scene), out, u, 1280, 800, 1, false, 0, 0, 0, 0);
#endif
}

/* Opcodes configure fixtures or call the actual production boundaries. The
   Python oracle supplies each recorded f32 frame; this driver computes no
   due steps, carry, alpha, follower motion, or expected coordinates. */
EXPORT("follower_apply") void follower_apply(int32_t operation, float frame, int32_t fault) {
  if (operation == 0) {
    float counted = verse_player_counts(player, frame);
    walked = verse_player_advance_in_realm_traced(player, frame, (coax_v_f32){0, 0},
      (coax_v_f32){scene.walls.e, scene.wall_n * 6}, (coax_v_f32){0, 0}, (coax_string){realm, sizeof realm - 1});
    player = walked.player;
    if (fault == 1 && walked.count > 0) {
      coax_t_StepTarget final = walked.targets.e[walked.count - 1];
      for (int64_t i = 0; i < walked.count; ++i) walked.targets.e[i] = final;
    }
    if (fault == 3) for (int64_t i = 0; i < walked.count; ++i) walked.targets.e[i].feet.y -= 2.5f;
    draw_frame(fault, counted);
  } else if (operation == 1) {
    draw_frame(0, 0);
  } else if (operation == 2) {
    player.vel.x = 100; /* a finite incoming velocity whose move is refused */
  } else if (operation == 3) {
    player = verse_player_input(verse_player_place(origin_x + 2, 0, origin_z, 0), 1, 0, 0, 0, true, false, 0, 0);
    scene_place(scene_view(&scene), verse_place(origin_x + 2, 0, origin_z, 0, 0, 0.4f, 6, 0, -1, 1000));
  } else if (operation == 4) {
    scene.follower_tuning.acceleration = 0;
#if defined(FOLLOWER_GARDEN)
    scene.use = true;
#else
    scene.pet.e[0].doing = 0; /* DO_THROW */
#endif
  } else if (operation == 5) {
    scene.follower_tuning.acceleration = 14;
  }
}

static void put_float(size_t *at, float value) { memcpy(&words[(*at)++], &value, sizeof value); }
static void put_point(size_t *at, coax_t_Point p) { put_float(at, p.x); put_float(at, p.y); put_float(at, p.z); }
EXPORT("follower_snapshot") uintptr_t follower_snapshot(void) {
  size_t at = 0;
  put_float(&at, player.pos.x); put_float(&at, player.pos.y); put_float(&at, player.pos.z);
  put_float(&at, player.vel.x); put_float(&at, player.vel.y); put_float(&at, player.vel.z);
  put_float(&at, player.face); words[at++] = player.on_ground; words[at++] = player.act;
  words[at++] = (uint32_t)scene.fetch.mode;
  put_point(&at, scene.fetch.dog.pos); put_point(&at, scene.fetch.dog.vel);
  put_float(&at, scene.fetch.dog.face); words[at++] = scene.fetch.dog.walking;
  put_point(&at, scene.fetch.ball.pos); put_point(&at, scene.fetch.ball.vel); put_float(&at, scene.fetch.ball.radius);
  words[at++] = (uint32_t)scene.fetch.ball.held; words[at++] = scene.fetch.ball.thrown; words[at++] = scene.fetch.ball.resting;
  put_float(&at, scene.fetch.ball.spin); words[at++] = scene.follower_previous.ready;
  put_point(&at, scene.follower_previous.dog); put_float(&at, scene.follower_previous.face);
  put_point(&at, scene.follower_previous.ball); put_float(&at, scene.follower_previous.spin);
#if defined(FOLLOWER_GARDEN)
  put_float(&at, scene.dog_stride); put_float(&at, scene.dog_previous_stride);
#else
  put_float(&at, 0); put_float(&at, 0);
#endif
  words[at++] = (uint32_t)player.clock.ticks; words[at++] = (uint32_t)player.clock.frames;
  words[at++] = (uint32_t)player.clock.ran; words[at++] = (uint32_t)player.clock.carry_ns;
  words[at++] = (uint32_t)player.clock.counted_ns; words[at++] = (uint32_t)player.clock.refused;
  words[at++] = (uint32_t)player.clock.overlong; words[at++] = (uint32_t)player.clock.stalls;
  words[at++] = (uint32_t)player.clock.dropped_ns; words[at++] = (uint32_t)scene.follower_cursor.ticks;
  words[at++] = (uint32_t)scene.follower_cursor.frames; words[at++] = (uint32_t)scene.follower_steps;
  words[at++] = scene.follower_status; put_float(&at, scene.follower_alpha);
#if defined(FOLLOWER_GARDEN)
  words[at++] = scene.use; words[at++] = 0;
#else
  words[at++] = scene.pet.e[0].doing >= 0; words[at++] = (uint32_t)scene.pet.e[0].ev_n;
#endif
  words[at++] = player.collision_status;
  coax_t_Point drawn = {99999, 99999, 99999};
#if defined(FOLLOWER_GARDEN)
  /* First golden-coat sphere is the actual dog body emitted by dog(). */
  for (int64_t i = 160; i < 160 + scene.counts.e[2]; ++i) {
    float *r = scene.objects.e + i * 16;
    if (r[12] == 0.78f && r[13] == 0.55f && r[14] == 0.3f) { drawn = (coax_t_Point){r[3], r[7], r[11]}; break; }
  }
#else
  drawn = villa_draw_dog(scene_view(&scene)).pos; /* actual face's pose consumer */
#endif
  put_point(&at, drawn); words[at++] = (uint32_t)drawn_vertices;
  return (uintptr_t)words;
}

#if !defined(__wasm__)
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  FILE *input = fopen(argv[1], "rb"); if (!input) return 3;
  follower_init();
  struct { int32_t operation; float frame; int32_t fault; } record;
  while (fread(&record, sizeof record, 1, input) == 1) {
    follower_apply(record.operation, record.frame, record.fault);
    follower_snapshot();
    for (size_t i = 0; i < sizeof words / sizeof words[0]; ++i) printf("%u%c", words[i], i + 1 == sizeof words / sizeof words[0] ? '\n' : ' ');
  }
  fclose(input); return 0;
}
#endif
