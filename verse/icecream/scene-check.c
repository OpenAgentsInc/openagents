// scene.coax's check "the house and every model are stored once, chunk by chunk"
// on the default (Normal) house, natively: it and the models are over the
// compile-time storage cap, so the Coax check stores an Easy house and this one
// the house the page builds. verse/icecream/test.sh builds and runs it.
#include "Scene.h"
#include <stdio.h>
#include <stdlib.h>

enum { MAX_SCENE = 600000, SCENE_WORDS = 6, CHUNK = 1020, MODELS = 38, OBJ_FLOATS = 24, TRACE_CAP = 3000000 };

#define CHECK(c) do { if (!(c)) { fprintf(stderr, "scene-check: failed: %s (line %d)\n", #c, __LINE__); return 1; } } while (0)

int main(void) {
  coax_t_IceScene *s = calloc(1, sizeof *s);
  uint32_t *verts = calloc((size_t)MAX_SCENE * SCENE_WORDS, sizeof *verts);
  uint8_t *trace = calloc(TRACE_CAP, 1);
  if (!s || !verts || !trace) return 2;
  coax_v_t_IceScene v = {s, 1};
  icecream_attach(v, (coax_v_u32){verts, (int64_t)MAX_SCENE * SCENE_WORDS}, (coax_v_u8){trace, TRACE_CAP});
  icecream_hash(v, (coax_string){"", 0});   // the address's defaults: Normal, as a page with no options
  CHECK(s->plays.e[0].options.difficulty == 1);
  icecream_frame(v, 1600, 1000, 1, 0.016f);
  CHECK(s->built && s->vert_n % CHUNK == 0 && s->vert_n < MAX_SCENE && s->house_chunks * CHUNK > 60000);
  for (int m = 0; m < MODELS; m++) CHECK(s->model_count.e[m] > 0 && s->model_first.e[m] >= s->house_chunks);
  CHECK((verts[4] >> 24) == 0);
  CHECK(icecream_scene_store_bytes(v, 0) == s->vert_n * SCENE_WORDS * 4);
  CHECK(icecream_scene_store_bytes(v, 0) > 0 && icecream_scene_store_bytes(v, 0) > 0 && icecream_scene_store_bytes(v, 0) == 0);
  CHECK(icecream_scene_store_bytes(v, 1) == s->obj_n * OBJ_FLOATS * 4);
  printf("scene-check: the default house and %d models stored, %lld vertices in %lld chunks\n", MODELS, (long long)s->vert_n, (long long)(s->vert_n / CHUNK));
  return 0;
}
