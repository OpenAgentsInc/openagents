/* Firm reason: 1 (scalar browser ABI; the page is Coax). */
// The Speedrun HUD's C face for the browser (coax/web/host.js): speedrunhud.coax
// is the page (the stages, the Grid floor and the camera). It decides nothing
// and sends nothing; the stage comes from the page's url (message 600).
#include "speedrunhud.h"
#include <stddef.h>
#include <stdint.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];

static coax_t_SpeedrunHud *hud(uint32_t state) { return (coax_t_SpeedrunHud *)(uintptr_t)state; }
static coax_v_t_SpeedrunHud at(uint32_t state) { return (coax_v_t_SpeedrunHud){hud(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_SpeedrunHud) * 1000003u + offsetof(coax_t_SpeedrunHud, panel) * 7u + 0x53485544u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_SpeedrunHud); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_speedrunhud_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_speedrunhud_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_speedrunhud_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_speedrunhud_max_vertices() * coaxial_speedrunhud_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_speedrunhud_uniform_floats()};
  int64_t n = speedrunhud_frame(at(state), out, u, w, h, scale, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  speedrunhud_message(at(state), kind, (coax_string){(const char *)inbox, length});
}
