// Optional face effects for a realm that does not publish the corresponding
// capability. Include once in a static host, or compile once inside a closed
// producer DSO. Executable fallbacks must not preempt that DSO's real face.
#ifndef METAVERSE_NATIVE_FACE_DEFAULTS_H
#define METAVERSE_NATIVE_FACE_DEFAULTS_H
#include "face.h"
#define NONE __attribute__((weak))
NONE void coaxial_state_init(cx_face_addr s) { (void)s; }
NONE int32_t coaxial_action(cx_face_addr s) { (void)s; return 0; }
NONE cx_face_addr coaxial_outbox(cx_face_addr s) { (void)s; return 0; }
NONE uint32_t coaxial_outbox_len(cx_face_addr s) { (void)s; return 0; }
NONE void coaxial_action_done(cx_face_addr s) { (void)s; }
NONE cx_face_addr coaxial_asset(cx_face_addr s,int32_t i,uint32_t b) { (void)s;(void)i;(void)b;return 0; }
NONE uint32_t coaxial_asset_capacity(cx_face_addr s,int32_t i) { (void)s;(void)i;return 0; }
NONE void coaxial_asset_ready(cx_face_addr s,int32_t i,uint32_t b) { (void)s;(void)i;(void)b; }
NONE void coaxial_hash(cx_face_addr s,uint32_t n) { (void)s;(void)n; }
NONE cx_face_addr coaxial_draws(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_draw_capacity(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_draw_count(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_water_draws(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_grass_instances(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_surface_vertices(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_caster_vertices(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_caster_instances(cx_face_addr s) { (void)s;return 0; }
NONE void coaxial_wheel(cx_face_addr s,float dy) { (void)s;(void)dy; }
// host.js calls coaxial_blur only when the face exports it; a face without one (splice-face.c) keeps no held keys to drop (#13574).
NONE void coaxial_blur(cx_face_addr s) { (void)s; }
NONE void coaxial_page(cx_face_addr s,int32_t p) { (void)s;(void)p; }
NONE void coaxial_golden(uint32_t on) { (void)on; }
NONE uint32_t coaxial_stores(void) { return 0; }
NONE uint32_t coaxial_store_cap(int32_t k) { (void)k;return 0; }
NONE cx_face_addr coaxial_store(cx_face_addr s,int32_t k) { (void)s;(void)k;return 0; }
NONE uint32_t coaxial_store_bytes(cx_face_addr s,int32_t k) { (void)s;(void)k;return 0; }
// An absent version means copy every frame, as in the browser adapter.
NONE uint32_t coaxial_store_version(cx_face_addr s,int32_t k) { static uint32_t calls;(void)s;(void)k;return ++calls; }
NONE uint32_t coaxial_particle_cap(void) { return 0; }
NONE cx_face_addr coaxial_particles(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_particle_bytes(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_particle_instances(cx_face_addr s) { (void)s;return 0; }
NONE cx_face_addr coaxial_media(cx_face_addr s) { (void)s;return 0; }
NONE uint32_t coaxial_media_len(cx_face_addr s) { (void)s;return 0; }
#undef NONE
#endif
