// /chat's C face for the browser: scalar Wasm exports over chat.coax (the
// Zeron components, apps/shared/zeron/), the requests it asks for (a read of
// the state, or one post) handed to the host's HTTP import
// (coax/web/host.js) with their answers wanted, and the atlas calls
// coax/web/gpui.js makes (coaxial_gpui_*). The page records no analytics:
// what it shows is private.
#include "chat.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);

// The host keeps one WcHost: the WcState and the buffers it sees, bound again before every call.
static coax_v_t_WcState at(uint32_t state) {
  coax_t_WcHost *h = (coax_t_WcHost *)(uintptr_t)state;
  coaxial_chat_bind((coax_v_t_WcHost){h, 1});
  return (coax_v_t_WcState){h->s.e, 1};
}
static coax_t_WcState *raw(uint32_t state) { return at(state).data; }

// What chat.coax asks for, once there's a free slot (flag 4: answer wanted; 1: POST).
static void send_asks(uint32_t state) {
  coax_t_WcState *s = raw(state);
  if (s->ask_n > 0) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->ask.e, (uint32_t)s->ask_n, 0, 0, 4);
    if (id >= 0) coaxial_chat_asked(at(state), id);
  }
  if (s->post_n > 0) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->post.e, (uint32_t)s->post_n, (uint32_t)(uintptr_t)s->body.e, (uint32_t)s->body_n, 5);
    if (id >= 0) coaxial_chat_posted(at(state), id);
  }
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_WcHost) * 7u + offsetof(coax_t_WcHost, pool) * 3u + sizeof(coax_t_WcState) * 5u + 0x5A524Eu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_WcHost); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_chat_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_chat_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return 4u; }

// Golden mode (host.js, before any frame): the caret holds still.
static int golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  (void)still;
  if (golden) raw(state)->golden = 1;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_chat_max_vertices() * coaxial_chat_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, 4};
  int64_t n = coaxial_chat_frame(at(state), out, u, width, height, dpr, dt);
  send_asks(state);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  uint32_t used = coaxial_chat_key(at(state), key, mods) ? 1u : 0u;
  send_asks(state);
  return used;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return coaxial_chat_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { raw(state)->shift = 0; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  uint32_t used = coaxial_chat_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
  send_asks(state);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_chat_wheel(at(state), dy); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.data; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_chat_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { coaxial_chat_message(at(state), kind, length); send_asks(state); }
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) { coaxial_chat_hash(at(state), (int64_t)length); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return coaxial_chat_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->field.e; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_chat_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_chat_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)raw(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_chat_action_done(at(state)); }

// The atlas (coax/web/gpui.js): what to rasterize, then where it went.
EXPORT("coaxial_gpui_glyphs") int32_t gpui_glyphs(void) { return (int32_t)coaxial_chat_glyph_count(); }
EXPORT("coaxial_gpui_glyph_code") int32_t gpui_glyph_code(int32_t g) { return (int32_t)coaxial_chat_glyph_code(g); }
EXPORT("coaxial_gpui_fonts") int32_t gpui_fonts(void) { return (int32_t)coaxial_chat_font_count(); }
EXPORT("coaxial_gpui_font_size") float gpui_font_size(int32_t k) { return coaxial_chat_font_size(k); }
EXPORT("coaxial_gpui_font_weight") int32_t gpui_font_weight(int32_t k) { return (int32_t)coaxial_chat_font_weight(k); }
EXPORT("coaxial_gpui_icons") int32_t gpui_icons(void) { return (int32_t)coaxial_chat_icon_count(); }
EXPORT("coaxial_gpui_icon_size") float gpui_icon_size(int32_t i) { return coaxial_chat_icon_size(i); }
EXPORT("coaxial_gpui_icon_name") int32_t gpui_icon_name(uint32_t state, int32_t i) { return (int32_t)coaxial_chat_icon_name(at(state), i); }
EXPORT("coaxial_gpui_scratch") uint32_t gpui_scratch(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->scratch.e; }
EXPORT("coaxial_gpui_bytes") uint32_t gpui_bytes(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->sbytes.e; }
EXPORT("coaxial_gpui_set_font") void gpui_set_font(uint32_t state, int32_t slot) { coaxial_chat_set_font(at(state), slot); }
EXPORT("coaxial_gpui_set_icon") void gpui_set_icon(uint32_t state, int32_t i, float x, float y, float w, float h) { coaxial_chat_set_icon(at(state), i, x, y, w, h); }
EXPORT("coaxial_gpui_set_atlas") void gpui_set_atlas(uint32_t state, float w, float h, int32_t levels) { coaxial_chat_set_atlas(at(state), w, h, levels); }
EXPORT("coaxial_gpui_set_dynamic") void gpui_set_dynamic(uint32_t state, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_w, float color_h) {
  coaxial_chat_set_dynamic(at(state), size, pen_x, pen_y, y0, cols, color_cols, color_w, color_h);
}
EXPORT("coaxial_gpui_next_cluster") int32_t gpui_next_cluster(uint32_t state) { return (int32_t)coaxial_chat_next_cluster(at(state)); }
EXPORT("coaxial_gpui_cluster") int32_t gpui_cluster(uint32_t state, int32_t slot) { return (int32_t)coaxial_chat_cluster(at(state), slot); }
EXPORT("coaxial_gpui_measured") void gpui_measured(uint32_t state, int32_t slot, float advance, int32_t color) { coaxial_chat_measured(at(state), slot, advance, color != 0); }
EXPORT("coaxial_gpui_next_cell") int32_t gpui_next_cell(uint32_t state) { return (int32_t)coaxial_chat_next_cell(at(state)); }
EXPORT("coaxial_gpui_cell") int32_t gpui_cell(uint32_t state, int32_t cell) { return coaxial_chat_cell(at(state), cell) ? 1 : 0; }
EXPORT("coaxial_gpui_cell_ready") void gpui_cell_ready(uint32_t state, int32_t cell) { coaxial_chat_cell_ready(at(state), cell); }
