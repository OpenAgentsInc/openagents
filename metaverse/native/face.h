// The browser face (coax/web/host.js, a realm's -face.c) as a native host
// calls it: the same C, built with gcc instead of to wasm32. Pointer width
// follows the explicitly selected face ABI. Counts and GPU/wire words stay
// u32; legacy native faces retain their below-4-GB allocation requirements.
#ifndef VERSE_NATIVE_FACE_H
#define VERSE_NATIVE_FACE_H
#include <stdint.h>
#include "face-address.h"

uint32_t coaxial_layout(void);
void coaxial_state_init(cx_face_addr state);
uint32_t coaxial_state_size(void);
uint32_t coaxial_max_vertices(void);
uint32_t coaxial_vertex_floats(void);
uint32_t coaxial_uniform_floats(void);
// Optional synthesized audio. The storage pair names the complete physical
// allocation before any NAME ADDRESS BYTES metadata span is copied.
cx_face_addr coaxial_sound_bank(cx_face_addr state);
uint32_t coaxial_sound_bank_len(cx_face_addr state);
uint32_t coaxial_sound_bank_capacity(cx_face_addr state);
cx_face_addr coaxial_sound_bank_storage(cx_face_addr state);
uint32_t coaxial_sound_bank_storage_bytes(cx_face_addr state);
cx_face_addr coaxial_sounds(cx_face_addr state);
uint32_t coaxial_sounds_capacity(cx_face_addr state);
uint32_t coaxial_sounds_len(cx_face_addr state);
void coaxial_golden(uint32_t on);
uint32_t coaxial_frame(cx_face_addr state, cx_face_addr vertices, cx_face_addr uniforms, float w, float h, float scale, float dt, uint32_t still);
cx_face_addr coaxial_draws(cx_face_addr state);
// Optional measured allocation for faces whose draw records live outside state.
// Zero retains the caller's measured state-arena source allocation.
uint32_t coaxial_draw_capacity(cx_face_addr state);
uint32_t coaxial_draw_count(cx_face_addr state);
uint32_t coaxial_stores(void);
uint32_t coaxial_store_cap(int32_t k);
cx_face_addr coaxial_store(cx_face_addr state, int32_t k);
uint32_t coaxial_store_bytes(cx_face_addr state, int32_t k);
uint32_t coaxial_store_version(cx_face_addr state, int32_t k);
uint32_t coaxial_water_draws(cx_face_addr state);
uint32_t coaxial_key(cx_face_addr state, int32_t key, int32_t mods);
uint32_t coaxial_key_up(cx_face_addr state, int32_t key, int32_t mods);
void coaxial_blur(cx_face_addr state);
uint32_t coaxial_pointer(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id);
void coaxial_wheel(cx_face_addr state, float dy);
cx_face_addr coaxial_inbox(cx_face_addr state);
uint32_t coaxial_inbox_cap(void);
void coaxial_message(cx_face_addr state, int32_t kind, uint32_t length);
cx_face_addr coaxial_asset(cx_face_addr state, int32_t i, uint32_t bytes);
// Actual allocation size of the destination returned by coaxial_asset.
uint32_t coaxial_asset_capacity(cx_face_addr state, int32_t i);
void coaxial_asset_ready(cx_face_addr state, int32_t i, uint32_t bytes);
void coaxial_hash(cx_face_addr state, uint32_t length);
int32_t coaxial_action(cx_face_addr state);
cx_face_addr coaxial_outbox(cx_face_addr state);
uint32_t coaxial_outbox_len(cx_face_addr state);
uint32_t coaxial_outbox_capacity(cx_face_addr state);
void coaxial_action_done(cx_face_addr state);
uint32_t coaxial_particle_cap(void);
cx_face_addr coaxial_particles(cx_face_addr state);
uint32_t coaxial_particle_bytes(cx_face_addr state);
uint32_t coaxial_particle_instances(cx_face_addr state);
cx_face_addr coaxial_media(cx_face_addr state);
uint32_t coaxial_media_len(cx_face_addr state);
uint32_t coaxial_media_capacity(cx_face_addr state);
uint32_t coaxial_grass_instances(cx_face_addr state);
// a traced page's surface (the water lab's sea): vertices drawn after bg, three a triangle
uint32_t coaxial_surface_vertices(cx_face_addr state);
uint32_t coaxial_caster_vertices(cx_face_addr state);
uint32_t coaxial_caster_instances(cx_face_addr state);
// which page of the app to draw (host.js's data-page: the galaxy is the verse app's page 1)
void coaxial_page(cx_face_addr state, int32_t page);
#endif
