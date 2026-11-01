/* The legacy C boundary must keep its layout while its sampled geometry is
   owned by aethyr/ground.coax. These checks call the production exports. */
#include "terrain-native.h"
#include <assert.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

_Static_assert(sizeof(coax_t_Ground) == 40, "legacy LP64 Ground size");
_Static_assert(offsetof(coax_t_Ground, n) == 0, "legacy Ground.n");
_Static_assert(offsetof(coax_t_Ground, cell) == 8, "legacy Ground.cell");
_Static_assert(offsetof(coax_t_Ground, west) == 12, "legacy Ground.west");
_Static_assert(offsetof(coax_t_Ground, south) == 16, "legacy Ground.south");
_Static_assert(offsetof(coax_t_Ground, rim_from) == 20, "legacy Ground.rim_from");
_Static_assert(offsetof(coax_t_Ground, rim) == 24, "legacy Ground.rim");
_Static_assert(offsetof(coax_t_Ground, rows) == 32, "legacy Ground.rows");

static uint32_t word(float value) {
  uint32_t out;
  memcpy(&out, &value, sizeof out);
  return out;
}

int main(void) {
  coax_t_Ground g;
  memset(&g, 0xa5, sizeof g);
  coax_default_t_Ground(&g);
  assert(g.n == 64 && g.cell == 1.5f && g.west == -48 && g.south == -48);
  assert(g.rim_from == 32 && g.rim == 0.022f && g.rows == 0);
  g.n = 13; g.rows = 2; g.cell = 0.1f; g.west = 100.1f; g.south = -12.3f;
  float heights[42], before[42];
  for (int j = 0; j < 3; ++j)
    for (int i = 0; i < 14; ++i) heights[j * 14 + i] = (float)i * 17 + (float)j * 0.3f;
  memcpy(before, heights, sizeof before);
  for (int j = 0; j < 3; ++j) {
    for (int i = 0; i < 14; ++i) {
      float x = g.west + (float)i * g.cell, z = g.south + (float)j * g.cell;
      float y = verse_terrain_height(g, (coax_v_f32){heights, 42}, x, z);
      assert(word(y) == word(heights[j * 14 + i]));
    }
  }
  assert(!memcmp(before, heights, sizeof before));

  g.n = 1; g.rows = 1; g.cell = 1; g.west = 0; g.south = 0;
  float corners[4] = {-93.68108367919922f, -72.09828186035156f, -0.0f, 5.823012351989746f};
  for (int i = 0; i < 4; ++i)
    assert(word(verse_terrain_height(g, (coax_v_f32){corners, 4}, (float)(i % 2), (float)(i / 2))) == word(corners[i]));
  float plane[4] = {0, 1, 2, 4};
  g.cell = 2; g.west = -3; g.south = 5;
  coax_t_Point normal = verse_terrain_normal(g, (coax_v_f32){plane, 4}, -1.5f, 5.5f);
  assert(fabs((double)normal.x + .5 / sqrt(3.5)) < .000001);
  assert(fabs((double)normal.y - 1 / sqrt(3.5)) < .000001);
  assert(fabs((double)normal.z + 1.5 / sqrt(3.5)) < .000001);
  normal = verse_terrain_normal(g, (coax_v_f32){plane, 4}, -10, 6);
  assert(normal.x == 0 && fabs((double)normal.y - 1 / sqrt(2.0)) < .000001);
  assert(fabs((double)normal.z + 1 / sqrt(2.0)) < .000001);
  normal = verse_terrain_normal(g, (coax_v_f32){plane, 4}, 100, 100);
  assert(normal.x == 0 && normal.y == 1 && normal.z == 0);
  assert(verse_terrain_height(g, (coax_v_f32){NULL, 0}, -1.5f, 5.5f) == 0);
  normal = verse_terrain_normal(g, (coax_v_f32){plane, 3}, -1.5f, 5.5f);
  assert(normal.x == 0 && normal.y == 1 && normal.z == 0);
  g.rows = -1;
  assert(verse_terrain_height(g, (coax_v_f32){plane, 4}, -1.5f, 5.5f) == 0);
  puts("legacy terrain: LP64 layout/defaults, exact nonbinary vertices, corner words, slopes, constant edge normals and refused views pass");
  return 0;
}
