#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { float *data; int64_t count; } coax_v_f32;
extern int64_t collision_checked(coax_v_f32, coax_v_f32, coax_v_f32, coax_v_f32, float, float, coax_v_f32);
extern void collision_legacy(coax_v_f32, coax_v_f32, coax_v_f32, coax_v_f32, float, float, coax_v_f32);

static coax_v_f32 view(float *p, int64_t n) { return (coax_v_f32){p, n}; }

static void parity(const char *name, float *boxes, int nb, float *bodies, int nc,
                   float *floors, int nf, float *state, float dx, float dz) {
  float checked[8] = {0}, legacy[6] = {0};
  int64_t status = collision_checked(view(boxes, nb), view(bodies, nc), view(floors, nf),
                                     view(state, 6), dx, dz, view(checked, 8));
  collision_legacy(view(boxes, nb), view(bodies, nc), view(floors, nf), view(state, 6), dx, dz, view(legacy, 6));
  if (status != 0 || memcmp(checked, legacy, sizeof legacy)) {
    fprintf(stderr, "%s: status %lld\n", name, (long long)status);
    for (int i = 0; i < 6; ++i) fprintf(stderr, "  %d: new %.9g, published %.9g\n", i, checked[i], legacy[i]);
    assert(0 && "published contact parity");
  }
}

int main(void) {
  float state[6] = {0, 0, 0.65f, 2, 3, 4};
  float wall[6] = {-2, 0, 1, 2, 2, 1.01f};
  parity("thin wall", wall, 6, NULL, 0, NULL, 0, state, 0, 0.1f);
  parity("free step", NULL, 0, NULL, 0, NULL, 0, state, 0.1171875f, 0);
  wall[4] = 0.55f; parity("step threshold", wall, 6, NULL, 0, NULL, 0, state, 0, 0.1f);
  wall[4] = 0.56f; parity("high step", wall, 6, NULL, 0, NULL, 0, state, 0, 0.1f);
  float post[7] = {0, 0, 1, 0, 2, 1, 0.4f}; state[2] = 0.25f;
  parity("upright capsule", NULL, 0, post, 7, NULL, 0, state, 0, 0.1f);
  state[2] = 1; parity("on capsule axis", NULL, 0, post, 7, NULL, 0, state, 0, 0);
  post[1] = 0.2f; post[4] = 0.2f; post[6] = 0.2f;
  parity("capsule cap", NULL, 0, post, 7, NULL, 0, state, 0, 0);
  float roof[10] = {1, -1, 1, 2, 0, 0, 2, 1, 0, 0}; state[2] = 0.95f;
  parity("roof side", NULL, 0, NULL, 0, roof, 10, state, 0.05f, 0.1f);
  float invalid[6] = {1, 0, -1, -1, 2, 1};
  parity("reversed corners skipped", invalid, 6, NULL, 0, NULL, 0, state, 0.1f, 0);
  parity("partial tail skipped", wall, 5, post, 6, roof, 9, state, 0.1f, 0);
  post[0] = 0; post[1] = 0.1f; post[2] = 1;
  post[3] = 0; post[4] = 0.1005f; post[5] = 1; post[6] = 0.2f;
  state[0] = 0.25f; state[2] = 1;
  parity("legacy short-axis guard", NULL, 0, post, 7, NULL, 0, state, 0, 0);
  post[0] = -0.000375f; post[1] = 0.25f; post[3] = 0.000125f; post[4] = 1.25f;
  state[0] = 0;
  parity("legacy near-parallel guard", NULL, 0, post, 7, NULL, 0, state, 0, 0);
  state[2] = 0.65f; state[3] = -0.0f; state[5] = 0; wall[4] = 2;
  parity("wall signed zero", wall, 6, NULL, 0, NULL, 0, state, 0, 0.1f);
  post[0] = 0; post[1] = 0; post[3] = 0; post[4] = 2;
  state[0] = 0.25f; state[2] = 1; state[5] = -0.0f;
  parity("capsule signed zero", NULL, 0, post, 7, NULL, 0, state, 0, 0);

  float checked[8] = {0};
  assert(collision_checked(view(wall, 6), view(NULL, 0), view(NULL, 0), view(state, 6), 0, 0.15001f, view(checked, 8)) == 4);
  assert(!memcmp(checked, state, sizeof state));
  float trap[12] = {-2, 0, -2, -0.2f, 2, 2, 0.2f, 0, -2, 2, 2, 2};
  state[2] = 0;
  assert(collision_checked(view(trap, 12), view(NULL, 0), view(NULL, 0), view(state, 6), 0, 0, view(checked, 8)) == 5);
  assert(!memcmp(checked, state, sizeof state) && checked[6] > 0.1f);
  puts("14 published contacts match bit for bit; travel and unresolved refusals preserve state");
}
