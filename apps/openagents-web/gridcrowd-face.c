// The Grid crowd's C face for the browser (coax/web/host.js): gridcrowd.coax
// is the page (the stage, the crowd, the camera and the draws). grid.data
// (asset 0) goes to the page; the other GPU assets are the GPU's and the page
// is only told they came. Cast.bin (asset 5, and 6 as the GPU's copy; 7 its
// texture sheets) is opened here with verse/cast.coax, which plays each
// player's clips on the crowd's own clock and writes their joint palettes,
// scaled to their height, into store 1 after every player's look (the
// page's colours for them, four vec4s each). It decides nothing and sends nothing: the crowd is simulated, not
// anyone's presence. Every pointer is an offset into the host's
// WebAssembly.Memory; the page's state is the host's (coaxial_state_size).
#include "gridcrowd.h"
#include "Cast.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static uint8_t data[65536];
// Cast.bin: the character, read by verse/cast.coax
#define CAST_CAP (6u << 20)
static uint8_t cast_bytes[CAST_CAP];
static uint32_t cast_n;
static coax_t_Cast cast_set;
static bool cast_open;
#define PEOPLE_MAX 1024   /* gridcrowd.coax PEOPLE */
#define JOINTS_MAX 65
static coax_t_Anim anims[PEOPLE_MAX];
static float anim_whose[PEOPLE_MAX];
#define LOOK_FLOATS 16    /* gridcrowd.coax LOOK_FLOATS */
#define PERSON_FLOATS 7  /* gridcrowd.coax PERSON_FLOATS */
// store 1: the looks (vec4 0..4095, as gpu/gridcrowd.coax reads them), then the palettes
static struct { float looks[PEOPLE_MAX * LOOK_FLOATS]; coax_t_Joint palette[PEOPLE_MAX * JOINTS_MAX]; } store1;
#define palette store1.palette
static uint32_t palette_bytes;

static coax_t_GridCrowd *crowd(uint32_t state) { return (coax_t_GridCrowd *)(uintptr_t)state; }
static coax_v_t_GridCrowd at(uint32_t state) { return (coax_v_t_GridCrowd){crowd(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_GridCrowd) * 1000003u + offsetof(coax_t_GridCrowd, robots) * 7u + offsetof(coax_t_GridCrowd, draws) * 13u + 0x43524f57u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_GridCrowd); }
// A new state's declared defaults. Its state declares none, so the zeroed memory the host
// hands it is its start; the host calls this first all the same (coax/web/state-defaults.coax).
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { (void)state; }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_gridcrowd_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_gridcrowd_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_gridcrowd_uniform_floats(); }

// Each player the page lists, posed: their clips moved on for how they move (verse_cast_update) and
// their joints placed where they stand (verse_cast_pose) and scaled to their height about their feet,
// into store 1 after the looks. A slot that holds someone new starts their clips afresh. The clock is
// the crowd's, so a clip is the same every time.
static void people_out(uint32_t state) {
  coax_t_GridCrowd *g = crowd(state);
  palette_bytes = 0;
  if (!cast_open) return;
  coax_v_u8 b = {cast_bytes, cast_n};
  int64_t n = g->people_n, joints = cast_set.joints;
  if (n > PEOPLE_MAX) n = PEOPLE_MAX;
  if (joints <= 0 || joints > JOINTS_MAX) return;
  double t = gridcrowd_time(at(state));
  for (int64_t i = 0; i < n; i++) {
    const float *p = &g->people.e[i * PERSON_FLOATS];
    if (anim_whose[i] != p[5]) { anims[i] = (coax_t_Anim){0}; anims[i].fade_start = -1; anims[i].start = t; anim_whose[i] = p[5]; }
    int64_t act = (int64_t)p[4];
    anims[i] = verse_cast_update(b, cast_set, anims[i], act, act == 3 && p[1] > 0.05f, t);
    coax_t_Joint *js = &palette[i * joints];
    if (!verse_cast_pose(b, cast_set, anims[i], t, p[0], p[1], p[2], p[3], (coax_v_t_Joint){js, joints})) return;
    float k = p[6] > 0.5f && p[6] < 1.5f ? p[6] : 1.0f, foot[3] = {p[0], p[1], p[2]};
    if (k != 1.0f) for (int64_t j = 0; j < joints; j++)
      for (int r = 0; r < 3; r++) {
        float *m = &js[j].m.e[r * 4];
        m[0] *= k; m[1] *= k; m[2] *= k; m[3] = k * m[3] + (1.0f - k) * foot[r];
      }
  }
  memcpy(store1.looks, g->looks.e, (size_t)n * LOOK_FLOATS * sizeof(float));
  palette_bytes = (uint32_t)(sizeof store1.looks + n * joints * sizeof(coax_t_Joint));
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_gridcrowd_max_vertices() * coaxial_gridcrowd_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_gridcrowd_uniform_floats()};
  int64_t n = gridcrowd_frame(at(state), out, u, w, h, scale, dt, still != 0);
  people_out(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)crowd(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return crowd(state)->started ? (uint32_t)gridcrowd_draws(at(state)) : 0u; }
// Store 0 the robots, store 1 the players' joint palettes.
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 1 ? (uint32_t)sizeof store1 : (uint32_t)sizeof(((coax_t_GridCrowd *)0)->robots); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { return k == 1 ? (uint32_t)(uintptr_t)&store1 : (uint32_t)(uintptr_t)crowd(state)->robots.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return k == 1 ? palette_bytes : (uint32_t)gridcrowd_store_bytes(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  gridcrowd_message(at(state), kind, (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  if (i == 5) return bytes <= CAST_CAP ? (uint32_t)(uintptr_t)cast_bytes : 0u;
  return i == 0 && bytes <= sizeof data ? (uint32_t)(uintptr_t)data : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0) {
    if (bytes > 0 && bytes <= sizeof data) gridcrowd_data(at(state), (coax_string){(const char *)data, bytes});
    else gridcrowd_arrived(at(state), false);
  } else if (i == 5) {
    // Cast.bin: admitted whole (verse_cast_open checks every buffer, index and joint), or no player draws
    cast_n = bytes <= CAST_CAP ? bytes : 0;
    cast_set = verse_cast_open((coax_v_u8){cast_bytes, cast_n});
    cast_open = cast_n > 0 && cast_set.ok && cast_set.joints <= JOINTS_MAX;
    gridcrowd_cast(at(state), cast_open, cast_set.body_vertices.offset / 4, cast_set.body_indices.offset / 4, cast_set.body_indices.count, cast_set.joints);
  } else if (i >= 1 && i <= 7) gridcrowd_arrived(at(state), bytes > 0);
}
