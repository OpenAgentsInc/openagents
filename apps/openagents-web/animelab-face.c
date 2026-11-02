// Anime Lab's C face for the browser (coax/web/host.js): animelab.coax is the
// page (the director's take, the stage, the overlay); this file decides
// nothing. It keeps the two stores the scene draws from (store 0, the
// academy, written once; store 1, the cast and the effects, every frame), and
// asks animelab-style.coax (its own library) for the anime style's block each
// frame, which the page copies into the uniforms. Every pointer is an offset
// into the host's WebAssembly.Memory; the page's state is the host's.
#include "animelab.h"
#include "Animelab-style.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Animelab *lab(uint32_t state) { return (coax_t_Animelab *)(uintptr_t)state; }
static coax_v_t_Animelab at(uint32_t state) { return (coax_v_t_Animelab){lab(state), 1}; }
static uint8_t inbox[65536]; // full take64 URL facts; Coax admits at most 32 KiB decoded
#define WORLD_WORDS (150000u * 6u)   /* animelab.coax WORLD_VERTICES × VERTEX_WORDS */
#define LIVE_WORDS (150000u * 6u)    /* LIVE_VERTICES × VERTEX_WORDS */
static uint32_t world[WORLD_WORDS];
static uint32_t live[LIVE_WORDS];
static float style[128];            /* STYLE_FLOATS */
static uint32_t frames;

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Animelab) * 1000003u + offsetof(coax_t_Animelab, draws) * 13u + 0x414e494du);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Animelab); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Animelab(lab(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_animelab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_animelab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_animelab_uniform_floats(); }

// Golden mode (host.js coaxial_golden): the take held at one moment.
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_animelab_max_vertices() * coaxial_animelab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_animelab_uniform_floats()};
  animelab_golden(at(state), golden);
  for (int i = 0; i < 128; i++) style[i] = 0;
  animelab_style_block(animelab_variant(at(state)), animelab_phone(width, dpr), animelab_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = animelab_frame(at(state), out, u, width, height, dpr, dt, still != 0,
                             (coax_v_u32){world, WORLD_WORDS}, (coax_v_u32){live, LIVE_WORDS}, (coax_v_f32){style, 128});
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

// The scene: two draws of vertices in order (no index), store 0 the academy, store 1 the stage.
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)lab(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)animelab_draw_count(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof world : (uint32_t)sizeof live; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; return k == 0 ? (uint32_t)(uintptr_t)world : (uint32_t)(uintptr_t)live; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  int64_t n = k == 0 ? animelab_world_n(at(state)) : animelab_live_n(at(state));
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
// The academy is written once: copied to the GPU once; the stage every frame.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)animelab_world_n(at(state)) : frames;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return animelab_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return animelab_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return animelab_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  animelab_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
}

EXPORT("coaxial_probe") int32_t coaxial_probe(uint32_t state) { return (int32_t)animelab_take_status(at(state)); }
EXPORT("coaxial_guard_probe") int32_t coaxial_guard_probe(uint32_t state, int32_t field) {
  return (int32_t)animelab_take_probe(at(state), field);
}
