// Crew's C face for the browser (coax/web/host.js): crew.coax is the page (the scene of example
// characters and its labels); this file decides nothing. It keeps the store the scene draws from
// (store 0, written when what is shown changes; store 1 is Anime Lab's live store, which Crew
// leaves empty), and asks animelab-style.coax for the anime style's block each frame, which the
// page copies into the uniforms. Every pointer is an offset into the host's WebAssembly.Memory.
#include "crew.h"
#include "Animelab-style.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Crew *crew(uint32_t state) { return (coax_t_Crew *)(uintptr_t)state; }
static coax_v_t_Crew at(uint32_t state) { return (coax_v_t_Crew){crew(state), 1}; }
static uint8_t inbox[8192];          /* the page's facts (kind 600), a document (701), what to show (702), a storage answer (768) */
#define WORLD_WORDS (230000u * 6u)   /* crew.coax CR_WORLD_VERTICES × VERTEX_WORDS */
static uint32_t world[WORLD_WORDS];
static uint32_t live[6];
static float style[128];             /* STYLE_FLOATS */
static bool golden;

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Crew) * 1000003u + offsetof(coax_t_Crew, draws) * 13u + 0x43524557u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Crew); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Crew(crew(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_crew_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_crew_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_crew_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

// This browser's copy of the character (coax/web/STORAGE.md): the page says when to read it and when to keep
// it; the host answers through the inbox (768 + slot 0).
__attribute__((import_module("env"), import_name("coaxial_storage_put"))) int32_t coaxial_storage_put(int32_t slot, int32_t token, uint32_t space, int32_t space_len, int32_t version, uint32_t key, int32_t key_len, uint32_t bytes, int32_t bytes_len);
__attribute__((import_module("env"), import_name("coaxial_storage_get"))) int32_t coaxial_storage_get(int32_t slot, int32_t token, uint32_t space, int32_t space_len, int32_t version, uint32_t key, int32_t key_len);
static const char store_space[] = "crew", store_key[] = "character";   /* crew.coax CR_STORE_SPACE, CR_STORE_KEY */
static void store(uint32_t state) {
  int64_t wants = crew_store_wants(at(state));
  if (wants == 1) {
    crew_store_asked(at(state), 1, coaxial_storage_get(0, 1, (uint32_t)(uintptr_t)store_space, 4, 1, (uint32_t)(uintptr_t)store_key, 9) == 0);
  } else if (wants == 2) {
    coax_string text = crew_store_text(at(state));
    crew_store_asked(at(state), 2, coaxial_storage_put(0, 2, (uint32_t)(uintptr_t)store_space, 4, 1, (uint32_t)(uintptr_t)store_key, 9, (uint32_t)(uintptr_t)text.data, (int32_t)text.count) == 0);
  }
}
// The pane beside the scene (coax/web/host.js, "A page's own pane"): its module's name, and what it is told.
EXPORT("coaxial_pane") uint32_t coaxial_pane(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)crew_pane().data; }
EXPORT("coaxial_pane_len") uint32_t coaxial_pane_len(uint32_t state) { (void)state; return (uint32_t)crew_pane().count; }
EXPORT("coaxial_pane_rev") int32_t coaxial_pane_rev(uint32_t state) { return (int32_t)crew_pane_rev(at(state)); }
EXPORT("coaxial_pane_model") uint32_t coaxial_pane_model(uint32_t state) { return (uint32_t)(uintptr_t)crew_pane_model(at(state)).data; }
EXPORT("coaxial_pane_model_len") uint32_t coaxial_pane_model_len(uint32_t state) { return (uint32_t)crew_pane_model(at(state)).count; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_crew_max_vertices() * coaxial_crew_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_crew_uniform_floats()};
  for (int i = 0; i < 128; i++) style[i] = 0;
  animelab_style_block(crew_variant(at(state)), animelab_phone(width, dpr), crew_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = crew_frame(at(state), out, u, width, height, dpr, dt, (still != 0) || golden, (coax_v_u32){world, WORLD_WORDS}, (coax_v_f32){style, 128});
  store(state);
  return n < 0 ? 0u : (uint32_t)n;
}

// The scene: one draw of store 0's vertices in order (no index).
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)crew(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)crew_draw_count(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof world : (uint32_t)sizeof live; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; return k == 0 ? (uint32_t)(uintptr_t)world : (uint32_t)(uintptr_t)live; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  int64_t n = k == 0 ? crew_world_n(at(state)) : 0;
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
// The store is copied to the GPU when the scene is written again.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { return k == 0 ? (uint32_t)crew_version(at(state)) : 0u; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  crew_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
  store(state);
}
// A fact of the page for a headless check (coaxial.fieldProbe).
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(uint32_t state, int32_t field) { return (int32_t)crew_fact(at(state), field); }
