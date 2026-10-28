// The real Cast.bin through metareal/cast.coax under ASan and UBSan: it is
// admitted with aethyr's counts, a minute of walking, running and jumping
// and the robots' patrols give finite palettes, and damaged copies,
// including non-finite numbers the Coax evaluator cannot hold, are refused.
#include "Cast.h"
#include <assert.h>
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
  free(palette);
  free(bytes);
  printf("cast: %lld joints, %lld robots, 3600 frames finite, damaged copies refused\n", (long long)joints,
         (long long)metareal_cast_robots(c));
  return 0;
}
