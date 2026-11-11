/* /settings/agents' C face for the browser: scalar Wasm exports over agents.coax (an owner's
   agents and grants, #13211), the atlas calls coax/web/gpui.js makes (coaxial_gpui_*) as
   components-face.c has them, and the page's one request handed to the host's HTTP import
   (coax/web/host.js). It records no analytics: the page can show a new API key. */
#include "agents.h"
#include <stddef.h>
#include <stdint.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);

static unsigned char inbox[65536];
static coax_t_AgentsPage *raw(uint32_t p) { return (coax_t_AgentsPage *)(uintptr_t)p; }
static coax_v_t_AgentsPage at(uint32_t p) { return (coax_v_t_AgentsPage){raw(p), 1}; }
/* The request agents.coax asks for, once the host has a free slot (flag 4: answer wanted). */
static void send_ask(uint32_t p) {
  coax_t_AgentsPage *s = raw(p);
  if (!s->want) return;
  int32_t id = coaxial_http((uint32_t)(uintptr_t)s->url.e, (uint32_t)s->url_n, (uint32_t)(uintptr_t)s->body.e, (uint32_t)s->body_n, (int32_t)s->flags);
  if (id >= 0) ap_sent(at(p), id);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_AgentsPage) * 7u + 0x41474Eu); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_AgentsPage); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t p) { (void)p; }   /* zeros; the first frame sets it up */
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_agents_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_agents_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return 4u; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t p, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  (void)still;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_agents_max_vertices() * coaxial_agents_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, 4};
  int64_t n = coaxial_agents_frame(at(p), out, u, width, height, dpr, dt);
  send_ask(p);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t p, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)kind; (void)id;
  uint32_t used = coaxial_agents_pointer(at(p), x, y, phase) ? 1u : 0u;
  send_ask(p);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t p, float dy) { coaxial_agents_wheel(at(p), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t p) { return (int32_t)coaxial_agents_cursor(at(p)); }
/* The page's asks of the host: 1 go to the outbox's path (log in), 2 copy (a new key), 3 the hash. */
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t p) { return (int32_t)coaxial_agents_action(at(p)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t p) { return (uint32_t)(uintptr_t)raw(p)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t p) { return (uint32_t)raw(p)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t p) { coaxial_agents_action_done(at(p)); }
EXPORT("coaxial_inbox") uint32_t incoming(uint32_t p) { (void)p; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t incoming_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void message(uint32_t p, int32_t kind, int32_t n) {
  if (n < 0 || n > (int32_t)sizeof inbox) return;
  ap_message(at(p), kind, (coax_string){(const char *)inbox, n});
  send_ask(p);
}
EXPORT("coaxial_hash") void hash(uint32_t p, uint32_t n) {
  if (n > sizeof inbox) return;
  ap_hash(at(p), (coax_string){(const char *)inbox, (int64_t)n});
  send_ask(p);
}

/* The atlas (coax/web/gpui.js): what to rasterize, then where it went. */
EXPORT("coaxial_gpui_glyphs") int32_t gpui_glyphs(void) { return (int32_t)coaxial_agents_glyph_count(); }
EXPORT("coaxial_gpui_glyph_code") int32_t gpui_glyph_code(int32_t g) { return (int32_t)coaxial_agents_glyph_code(g); }
EXPORT("coaxial_gpui_fonts") int32_t gpui_fonts(void) { return (int32_t)coaxial_agents_font_count(); }
EXPORT("coaxial_gpui_font_size") float gpui_font_size(int32_t k) { return coaxial_agents_font_size(k); }
EXPORT("coaxial_gpui_font_weight") int32_t gpui_font_weight(int32_t k) { return (int32_t)coaxial_agents_font_weight(k); }
EXPORT("coaxial_gpui_icons") int32_t gpui_icons(void) { return (int32_t)coaxial_agents_icon_count(); }
EXPORT("coaxial_gpui_icon_size") float gpui_icon_size(int32_t i) { return coaxial_agents_icon_size(i); }
EXPORT("coaxial_gpui_icon_name") int32_t gpui_icon_name(uint32_t p, int32_t i) { return (int32_t)coaxial_agents_icon_name(at(p), i); }
EXPORT("coaxial_gpui_scratch") uint32_t gpui_scratch(uint32_t p) { return (uint32_t)(uintptr_t)raw(p)->scratch.e; }
EXPORT("coaxial_gpui_bytes") uint32_t gpui_bytes(uint32_t p) { return (uint32_t)(uintptr_t)raw(p)->sbytes.e; }
EXPORT("coaxial_gpui_set_font") void gpui_set_font(uint32_t p, int32_t slot) { coaxial_agents_set_font(at(p), slot); }
EXPORT("coaxial_gpui_set_icon") void gpui_set_icon(uint32_t p, int32_t i, float x, float y, float w, float h) { coaxial_agents_set_icon(at(p), i, x, y, w, h); }
EXPORT("coaxial_gpui_set_atlas") void gpui_set_atlas(uint32_t p, float w, float h, int32_t levels) { coaxial_agents_set_atlas(at(p), w, h, levels); }
EXPORT("coaxial_gpui_set_dynamic") void gpui_set_dynamic(uint32_t p, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_w, float color_h) {
  coaxial_agents_set_dynamic(at(p), size, pen_x, pen_y, y0, cols, color_cols, color_w, color_h);
}
EXPORT("coaxial_gpui_next_cluster") int32_t gpui_next_cluster(uint32_t p) { return (int32_t)coaxial_agents_next_cluster(at(p)); }
EXPORT("coaxial_gpui_cluster") int32_t gpui_cluster(uint32_t p, int32_t slot) { return (int32_t)coaxial_agents_cluster(at(p), slot); }
EXPORT("coaxial_gpui_measured") void gpui_measured(uint32_t p, int32_t slot, float advance, int32_t color) { coaxial_agents_measured(at(p), slot, advance, color != 0); }
EXPORT("coaxial_gpui_next_cell") int32_t gpui_next_cell(uint32_t p) { return (int32_t)coaxial_agents_next_cell(at(p)); }
EXPORT("coaxial_gpui_cell") int32_t gpui_cell(uint32_t p, int32_t cell) { return coaxial_agents_cell(at(p), cell) ? 1 : 0; }
EXPORT("coaxial_gpui_cell_ready") void gpui_cell_ready(uint32_t p, int32_t cell) { coaxial_agents_cell_ready(at(p), cell); }
