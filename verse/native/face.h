// The browser face (coax/web/host.js, a realm's -face.c) as a native host
// calls it: the same C, built with gcc instead of to wasm32. Every pointer
// the face takes or hands back is a 32-bit offset, as in WebAssembly.Memory,
// so a native build keeps all the realm's memory below 4 GB (realm-host.c).
#ifndef VERSE_NATIVE_FACE_H
#define VERSE_NATIVE_FACE_H
#include <stdint.h>

uint32_t coaxial_layout(void);
uint32_t coaxial_state_size(void);
uint32_t coaxial_max_vertices(void);
uint32_t coaxial_vertex_floats(void);
uint32_t coaxial_uniform_floats(void);
void coaxial_golden(uint32_t on);
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still);
uint32_t coaxial_draws(uint32_t state);
uint32_t coaxial_draw_count(uint32_t state);
uint32_t coaxial_stores(void);
uint32_t coaxial_store_cap(int32_t k);
uint32_t coaxial_store(uint32_t state, int32_t k);
uint32_t coaxial_store_bytes(uint32_t state, int32_t k);
uint32_t coaxial_store_version(uint32_t state, int32_t k);
uint32_t coaxial_water_draws(uint32_t state);
uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods);
uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods);
void coaxial_blur(uint32_t state);
uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id);
void coaxial_wheel(uint32_t state, float dy);
uint32_t coaxial_inbox(uint32_t state);
uint32_t coaxial_inbox_cap(void);
void coaxial_message(uint32_t state, int32_t kind, uint32_t length);
uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes);
void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes);
void coaxial_hash(uint32_t state, uint32_t length);
int32_t coaxial_action(uint32_t state);
uint32_t coaxial_outbox(uint32_t state);
uint32_t coaxial_outbox_len(uint32_t state);
void coaxial_action_done(uint32_t state);
uint32_t coaxial_particle_cap(void);
uint32_t coaxial_particles(uint32_t state);
uint32_t coaxial_particle_bytes(uint32_t state);
uint32_t coaxial_particle_instances(uint32_t state);
uint32_t coaxial_grass_instances(uint32_t state);
#endif
