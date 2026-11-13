// Splice's C face for a native host (aethyr/export/unreal/aethyr_host.h): the same decisions as
// splice-face.c makes for the browser, which is none. It keeps the two stores and asks
// animelab-style.coax for the anime block each frame. Pointers are pointers here.
#include "splice.h"
#include "Animelab-style.h"
#include "aethyr_host.h"
#include <stdbool.h>

#define WORLD_WORDS (150000u * 6u)   /* splice.coax SP_WORLD_WORDS × VERTEX_WORDS */
#define LIVE_WORDS (150000u * 6u)
static uint32_t world[WORLD_WORDS];
static uint32_t live[LIVE_WORDS];
static float style[128];            /* STYLE_FLOATS */
static uint32_t frames;

static coax_v_t_Splice at(void *state) { return (coax_v_t_Splice){(coax_t_Splice *)state, 1}; }

size_t aethyr_state_size(void) { return sizeof(coax_t_Splice); }
// Splice's state declares no defaults: zeroed memory is its start.
void aethyr_state_init(void *state) { (void)state; }
uint32_t aethyr_max_vertices(void) { return (uint32_t)coaxial_splice_max_vertices(); }
uint32_t aethyr_vertex_floats(void) { return (uint32_t)coaxial_splice_vertex_floats(); }
uint32_t aethyr_uniform_floats(void) { return (uint32_t)coaxial_splice_uniform_floats(); }

uint32_t aethyr_frame(void *state, float *vertices, float *uniforms, float width, float height, float dpr, float dt, int still) {
  coax_v_f32 out = {vertices, coaxial_splice_max_vertices() * coaxial_splice_vertex_floats()};
  coax_v_f32 u = {uniforms, coaxial_splice_uniform_floats()};
  for (int i = 0; i < 128; i++) style[i] = 0;
  animelab_style_block(splice_variant(at(state)), animelab_phone(width, dpr), splice_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = splice_frame(at(state), out, u, width, height, dpr, dt, still != 0,
                           (coax_v_u32){world, WORLD_WORDS}, (coax_v_u32){live, LIVE_WORDS}, (coax_v_f32){style, 128});
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

uint32_t aethyr_stores(void) { return 2; }
const uint32_t *aethyr_store(void *state, int k) { (void)state; return k == 0 ? world : live; }
uint32_t aethyr_store_bytes(void *state, int k) {
  int64_t n = k == 0 ? splice_world_n(at(state)) : splice_live_n(at(state));
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
uint32_t aethyr_store_version(void *state, int k) { return k == 0 ? (uint32_t)splice_world_version(at(state)) : frames; }
uint32_t aethyr_draw_count(void *state) { return (uint32_t)splice_draw_count(at(state)); }

int aethyr_key(void *state, int key, int down) { return splice_key(at(state), key, down != 0) ? 1 : 0; }
int aethyr_pointer(void *state, float x, float y, int phase) { return splice_pointer(at(state), x, y, phase) ? 1 : 0; }
int aethyr_mouselook(void *state) { return (int)splice_mouselook(at(state)); }
void aethyr_look_delta(void *state, float dx, float dy) { splice_look(at(state), dx, dy); }
void aethyr_pointer_lock(void *state, int status) { splice_lock(at(state), status); }
void aethyr_message(void *state, int kind, const char *body, size_t length) {
  splice_message(at(state), kind, (coax_string){body, (int64_t)length});
}
