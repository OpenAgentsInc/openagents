// Splice's C face for the browser (coax/web/host.js): splice.coax is the page (the Mars world and
// its frame); this file decides nothing. It keeps the two stores the scene draws from, and asks
// animelab-style.coax (Anime Lab's style library) for the anime block each frame, which the page
// copies into the uniforms. Every pointer is an offset into the host's WebAssembly.Memory.
#include "splice.h"
#include "Animelab-style.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Splice *splice_state(uint32_t state) { return (coax_t_Splice *)(uintptr_t)state; }
static coax_v_t_Splice at(uint32_t state) { return (coax_v_t_Splice){splice_state(state), 1}; }
#define WORLD_WORDS (150000u * 6u)   /* splice.coax SP_WORLD_WORDS × VERTEX_WORDS */
#define LIVE_WORDS (150000u * 6u)
static uint32_t world[WORLD_WORDS];
static uint32_t live[LIVE_WORDS];
static float style[128];            /* STYLE_FLOATS */
static uint32_t frames;
static bool golden;

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Splice) * 1000003u + offsetof(coax_t_Splice, world_n) * 13u + 0x53504c43u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Splice); }
// Splice's state declares no defaults: the zeroed memory the host hands it is its start.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { (void)state; }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_splice_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_splice_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_splice_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_splice_max_vertices() * coaxial_splice_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_splice_uniform_floats()};
  for (int i = 0; i < 128; i++) style[i] = 0;
  /* each scene picks its own look: its anime variant (verse/style-preset.coax) and clock */
  animelab_style_block(splice_variant(at(state)), animelab_phone(width, dpr), splice_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = splice_frame(at(state), out, u, width, height, dpr, dt, (still != 0) || golden,
                           (coax_v_u32){world, WORLD_WORDS}, (coax_v_u32){live, LIVE_WORDS}, (coax_v_f32){style, 128});
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

// The scene: two draws of vertices in order (no index), store 0 the Mars world, store 1 nothing.
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)splice_state(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)splice_draw_count(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof world : (uint32_t)sizeof live; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; return k == 0 ? (uint32_t)(uintptr_t)world : (uint32_t)(uintptr_t)live; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  int64_t n = k == 0 ? splice_world_n(at(state)) : splice_live_n(at(state));
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
// The world is written once and copied to the GPU once.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)splice_world_version(at(state)) : frames;
}

// Keys and the pointer go to the game (WASD, E, F, the edit bay's keys and buttons).
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return splice_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return splice_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)kind; (void)id; return splice_pointer(at(state), x, y, phase) ? 1u : 0u;
}
// The interface is ink on paper: the overlay blends source over (gpu/splice.coax fx_fs).
EXPORT("coaxial_fx_alpha_v1") uint32_t coaxial_fx_alpha_v1(void) { return 1; }

static uint8_t inbox[1024];   /* the page's facts (kind 600), read for ?scene= */
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  splice_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
}
