/* Firm reason: 1 (scalar browser ABI; the HUD is Coax). */
// gamebuilder.org's world and HUD face for the browser: scalar Wasm exports
// over hud.coax, for coax/web/host.js. The host keeps one Hud, calls
// coaxial_frame each frame for the strokes and the engine's frame block, draws
// the scene's draws and grass from its stores (the Metaverse Engine's stylized
// shading, web/gpu/hud.coax), and hands over the page's messages (hud-glue.js:
// the panels' places, the pointer, the keys, the blocks, the wheel) and the window's pointers. It decides nothing.
#include "hud.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_Hud at(uint32_t state) { return (coax_v_t_Hud){(coax_t_Hud *)(uintptr_t)state, 1}; }
static coax_t_Hud *raw(uint32_t state) { return (coax_t_Hud *)(uintptr_t)state; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_Hud) * 7u + 0x485544u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Hud); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Hud(raw(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)hud_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)hud_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)hud_uniform_floats(); }
EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  (void)still;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, hud_max_vertices() * hud_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, hud_uniform_floats()};
  int64_t n = hud_frame(at(state), out, u, width, height, dpr, dt);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)hud_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { hud_message(at(state), kind, length); }
// The world's camera (the engine's orbit, hud.coax): the host hands over every pointer of the window
// (device pixels; phase 0 down, 1 moved, 2 up, 3 cancelled; kind 0 a mouse, else a finger; the
// button), and says when the window lost the keys.
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  hud_pointer(at(state), x, y, phase, kind, id, button);
  return 0;
}
// First person's held pointer (the engine's mouselook; hud.coax decides): what the host should do with
// the browser's Pointer Lock, the mouse's movement while it is held (CSS pixels), and what the browser did.
EXPORT("coaxial_mouselook") int32_t coaxial_mouselook(uint32_t state) { return (int32_t)hud_mouselook(at(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(uint32_t state, float dx, float dy) { hud_look_delta(at(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(uint32_t state, int32_t status) { hud_pointer_lock(at(state), status); }
// The village's pack (world-village.coax; the host's pack: asset, coax/web/host.js). The host holds it
// back while coaxial_pack_wait_v1 answers 1 (until the village is picked), hashes what it fetched
// into the village's digest and asks whether it is admitted (the engine's pack gate,
// coax/web/PACK-GATE.md), copies the catalog where coaxial_asset says, and says as each of the four
// parts arrives. Every part is optional: the world draws without the buildings until they are in.
EXPORT("coaxial_pack_wait_v1") uint32_t coaxial_pack_wait_v1(uint32_t state) { return (uint32_t)hud_pack_wait(at(state)); }
EXPORT("coaxial_pack_gate_version") uint32_t coaxial_pack_gate_version(void) { return 1u; }
EXPORT("coaxial_pack_digest") uint32_t coaxial_pack_digest(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->village.e[0].digest.e; }
EXPORT("coaxial_pack_digest_capacity") uint32_t coaxial_pack_digest_capacity(void) { return 32u; }
EXPORT("coaxial_pack_admit") uint32_t coaxial_pack_admit(uint32_t state, uint32_t bytes) { return (uint32_t)hud_pack_admit(at(state), (int64_t)bytes); }
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  return i == 0 && bytes <= (uint32_t)hud_catalog_cap() ? (uint32_t)(uintptr_t)raw(state)->village.e[0].catalog.e : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) { (void)state; return i == 0 ? (uint32_t)hud_catalog_cap() : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) { hud_asset_ready(at(state), i, (int64_t)bytes); }
EXPORT("coaxial_asset_optional_v1") uint32_t coaxial_asset_optional_v1(int32_t i) { (void)i; return 1u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { hud_blur(at(state)); }
// What the HUD tells the page (the host hands it to the page's scripts as a "coaxial" event): where the character is.
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)hud_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)hud_outbox_len(at(state)); }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { hud_action_done(at(state)); }

// The scene: its draws (the ground, the blocks, the character), which of them is blended (the
// character: the engine's ghost), the grass's blades, and the six stores, each copied to the GPU
// when its generation changes: the kit's vertices, the records, the ground table, the character's
// skinned vertices, its triangles, and the ring's pictures (web/ring.coax: the home page's portals).
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->host_draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)hud_draw_count(at(state)); }
EXPORT("coaxial_draw_blend_v1") int32_t coaxial_draw_blend_v1(uint32_t state, int32_t i) { return (int32_t)hud_draw_blend(at(state), i); }
EXPORT("coaxial_grass_instances") uint32_t coaxial_grass_instances(uint32_t state) { return (uint32_t)hud_grass(at(state)); }
EXPORT("coaxial_grass_vertices_v1") uint32_t coaxial_grass_vertices_v1(uint32_t state) { return (uint32_t)hud_grass_vertices(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 6; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)hud_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_Hud *h = raw(state);
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)h->mesh.e : k == 1 ? (void *)h->records.e : k == 2 ? (void *)h->table.e : k == 3 ? (void *)h->skin.e : k == 4 ? (void *)h->tris.e : (void *)h->pics.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)hud_store_bytes(at(state), k); }
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { return (uint32_t)hud_store_version(at(state), k); }
