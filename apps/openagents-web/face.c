// The splash's C face for the browser: scalar Wasm exports that wrap the
// Coax procedures, which take views. Every pointer is an offset into the
// host's WebAssembly.Memory, where the host keeps the state, vertex and
// uniform regions. Logic stays in splash.coax.
#include "splash.h"

#define EXPORT(name) __attribute__((export_name(name)))

// The state's layout fingerprint: the host keeps state across a code swap
// only when this matches, and starts the state over when it doesn't.
EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_SplashState) * 1000003u + _Alignof(coax_t_SplashState) * 31u
                    + offsetof(coax_t_SplashState, frames));
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_SplashState); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_splash_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_splash_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_splash_uniform_floats(); }

// Which page to draw: 0 the splash, 1 the not-found page.
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) {
  ((coax_t_SplashState *)(uintptr_t)state)->page = page;
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_t_SplashState s = {(coax_t_SplashState *)(uintptr_t)state, 1};
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_splash_max_vertices() * coaxial_splash_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_splash_uniform_floats()};
  int64_t n = coaxial_splash_frame(s, out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}
