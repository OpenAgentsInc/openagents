// The real Cast.bin through metareal/cast.coax under ASan and UBSan: it is
// admitted with aethyr's counts, a minute of walking, running and jumping
// and the robots' patrols give finite palettes, and damaged copies,
// including non-finite numbers the Coax evaluator cannot hold, are refused.
// The dance plays and a step ends it. Then metareal/player.coax walks you at the Foreman among the robots'
// bodies: its leg stops you, and you can walk under it between its legs.
#include "Cast.h"
#include "Player.h"
#include <assert.h>
#include <stdbool.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t at32(const uint8_t *b, size_t at) { uint32_t v; memcpy(&v, b + at, 4); return v; }
static int refused(const uint8_t *bytes, size_t size, size_t at, uint32_t bits) {
  uint8_t *copy = malloc(size);
  memcpy(copy, bytes, size);
  memcpy(copy + at, &bits, 4);
  coax_t_Cast c = metareal_cast_open((coax_v_u8){copy, (int64_t)size});
  free(copy);
  return !c.ok;
}

int main(int argc, char **argv) {
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  fseek(f, 0, SEEK_END);
  size_t size = (size_t)ftell(f);
  fseek(f, 0, SEEK_SET);
  uint8_t *bytes = malloc(size);
  assert(fread(bytes, 1, size, f) == size);
  fclose(f);
  coax_v_u8 view = {bytes, (int64_t)size};
  coax_t_Cast c = metareal_cast_open(view);
  assert(c.ok && c.joints == 65 && c.bones == 67 && c.rate == 24 && c.scale == 1.5f);
  assert(metareal_cast_robots(c) == 6 && metareal_cast_palette(c) == 65 + 6 * 67);
  assert(c.body_indices.count % 3 == 0 && c.robot_lines.count % 2 == 0);

  int64_t joints = metareal_cast_palette(c);
  coax_t_Joint *palette = calloc((size_t)joints, sizeof *palette);
  coax_t_Anim a = {0};
  a.fade_start = -1;
  int seen[6] = {0};
  for (int i = 0; i < 3600; ++i) {
    double now = i / 60.0;
    int phase = (i / 180) % 5;  // idle, walk, run, jump, walk
    int act = phase == 3 ? ((i % 180) < 90 ? 3 : 0) : phase == 4 ? 1 : phase;
    a = metareal_cast_update(view, c, a, act, act == 3 && (i % 180) == 0, now);
    seen[a.move] = 1;
    assert(metareal_cast_pose(view, c, a, now, 1.0f, 0, -2.0f, (float)(i * 0.01), (coax_v_t_Joint){palette, joints}));
    assert(metareal_cast_robots_pose(view, c, now, (coax_v_t_Joint){palette, joints}));
    for (int64_t j = 0; j < joints; ++j)
      for (int k = 0; k < 12; ++k) assert(isfinite(palette[j].m.e[k]) && fabsf(palette[j].m.e[k]) < 1000);
  }
  for (int m = 0; m < 6; ++m) assert(seen[m]);
  // The dance: standing, the hotbar's button plays Dance_Loop for a while,
  // finite throughout; a step ends it.
  a = metareal_cast_dance(a);
  for (int i = 0; i < 600; ++i) {
    double now = 60 + i / 60.0;
    a = metareal_cast_update(view, c, a, 0, false, now);
    assert(a.dance);
    assert(metareal_cast_pose(view, c, a, now, 1.0f, 0, -2.0f, 0, (coax_v_t_Joint){palette, joints}));
    for (int64_t j = 0; j < c.joints; ++j)
      for (int k = 0; k < 12; ++k) assert(isfinite(palette[j].m.e[k]) && fabsf(palette[j].m.e[k]) < 1000);
  }
  assert(a.move == 6);
  a = metareal_cast_update(view, c, a, 1, false, 71);
  assert(!a.dance && a.move == 1);

  // Non-finite numbers: a vertex weight, a pose, a bind position, a sample.
  uint32_t nan = 0x7fc00000, inf = 0x7f800000;
  assert(refused(bytes, size, (size_t)c.body_vertices.offset + 44, nan));
  assert(refused(bytes, size, (size_t)c.robot_vertices.offset + 64 * 7 + 4, inf));
  assert(refused(bytes, size, (size_t)c.poses.offset + 32 * 100 + 16, nan));
  assert(refused(bytes, size, (size_t)c.robot_frames.offset + 32 * 9 + 8, inf));
  assert(refused(bytes, size, (size_t)c.skeleton.offset + 16 * 3 + 8, nan));
  assert(refused(bytes, size, (size_t)c.patrol_samples.offset + 20 * 9 + 8, nan));
  assert(refused(bytes, size, (size_t)c.robot_setup.offset + 4, nan));
  assert(refused(bytes, size, (size_t)c.robot_clips.offset + 8, inf));
  // An index past the vertices, and every length cut short.
  assert(refused(bytes, size, (size_t)c.body_indices.offset + 4 * 7, (uint32_t)c.body_vertices.count));
  assert(at32(bytes, 0) == 0x3143524D);
  for (size_t cut = 0; cut < size; cut += size / 97 + 1)
    assert(!metareal_cast_open((coax_v_u8){bytes, (int64_t)cut}).ok);
  // Bytes other than those admitted draw nothing.
  assert(!metareal_cast_pose((coax_v_u8){bytes, (int64_t)size - 1}, c, a, 0, 0, 0, 0, 0, (coax_v_t_Joint){palette, joints}));
  // Walk at the Foreman's left leg for four seconds, its bodies placed every
  // frame: the leg stops you. At its middle you walk under it, between its
  // legs; with no bodies, straight through the leg.
  int64_t count = metareal_cast_robots(c) * c.robot_bodies.count;
  assert(c.robot_bodies.count > 10 && c.robot_bodies.count <= c.bones);
  float *bodies = calloc((size_t)count * 7, sizeof *bodies);
  const float lx = 2.78f, lz = 3.62f;
  float nearest[3] = {1e9f, 1e9f, 1e9f}, far[3] = {0};
  for (int run = 0; run < 3; ++run) {
    float tx = run == 1 ? 2.8f : lx, tz = run == 1 ? 2.5f : lz;
    coax_t_Player p = metareal_player_place(0, 0, 0, atan2f(tx, tz));
    p = metareal_player_input(p, 0, 0, 0, 1, false, false, 0, 0);
    for (int i = 0; i < 240; ++i) {
      assert(metareal_cast_robots_pose(view, c, i / 60.0, (coax_v_t_Joint){palette, joints}));
      assert(metareal_cast_bodies(view, c, (coax_v_t_Joint){palette, joints}, (coax_v_f32){bodies, count * 7}) == count);
      p = metareal_player_advance_among(p, 1 / 60.0f, (coax_v_f32){bodies, run == 2 ? 0 : count * 7}, (coax_v_f32){0, 0});
      assert(isfinite(p.pos.x) && isfinite(p.pos.z));
      float d = hypotf(p.pos.x - tx, p.pos.z - tz);
      if (d < nearest[run]) nearest[run] = d;
    }
    far[run] = hypotf(p.pos.x, p.pos.z);
  }
  printf("Foreman: a leg stops you %.2f m from it; under it %.2f m from its middle, on to %.1f m\n",
         nearest[0], nearest[1], far[1]);
  assert(nearest[0] > 0.55f && nearest[0] < 1.2f);   // then you slide round it
  assert(nearest[1] < 0.3f && far[1] > 8);
  assert(nearest[2] < 0.1f && far[2] > 8);
  // A body's non-finite number is skipped, not followed.
  coax_t_Player p = metareal_player_place(lx, 0, lz - 0.9f, 0);
  bodies[3] = NAN;
  p = metareal_player_advance_among(p, 1 / 60.0f, (coax_v_f32){bodies, count * 7}, (coax_v_f32){0, 0});
  assert(isfinite(p.pos.x) && isfinite(p.pos.z));
  assert(refused(bytes, size, (size_t)c.robot_bodies.offset + 32 + 12, nan));
  assert(refused(bytes, size, (size_t)c.robot_bodies.offset + 4, inf));
  free(bodies);
  free(palette);
  free(bytes);
  printf("cast: %lld joints, %lld robots, 3600 frames finite, damaged copies refused\n", (long long)joints,
         (long long)metareal_cast_robots(c));
  return 0;
}
