// The water lab's C face for the browser (coax/web/host.js): scalar Wasm
// exports over waterlab.coax. The host keeps one Waterlab; this file decides
// nothing. No assets, no scene: waterlab.wgsl's bg_fs traces the sea from
// the uniforms waterlab_frame writes (docs/verse/water.md, "The water lab").
#include "waterlab.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Waterlab *state_at(uint32_t state) { return (coax_t_Waterlab *)(uintptr_t)state; }
static coax_v_t_Waterlab at(uint32_t state) { return (coax_v_t_Waterlab){state_at(state), 1}; }
static uint8_t inbox[16384];

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_Waterlab) * 1000003u + 0x57415452u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Waterlab); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_waterlab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_waterlab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_waterlab_uniform_floats(); }

// Golden mode (host.js coaxial_golden): the sea holds one moment.
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_waterlab_max_vertices() * coaxial_waterlab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_waterlab_uniform_floats()};
  waterlab_golden(at(state), golden);
  int64_t n = waterlab_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return waterlab_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return waterlab_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { waterlab_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return waterlab_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { waterlab_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  waterlab_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
}
