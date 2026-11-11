// /components' C face for the browser: scalar Wasm exports over
// components/gallery/page.coax (the component library's page), and the atlas
// calls coax/web/gpui.js makes (coaxial_gpui_*), as /chat's face has them.
#include "components.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_GalleryState at(uint32_t state) { return (coax_v_t_GalleryState){(coax_t_GalleryState *)(uintptr_t)state, 1}; }
static coax_t_GalleryState *raw(uint32_t state) { return (coax_t_GalleryState *)(uintptr_t)state; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_GalleryState) * 7u + 0x434D50u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_GalleryState); }
// GalleryState declares no defaults: a new state is zeros, and the first frame sets it up.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { (void)state; }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_components_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_components_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return 4u; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  (void)still;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_components_max_vertices() * coaxial_components_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, 4};
  int64_t n = coaxial_components_frame(at(state), out, u, width, height, dpr, dt);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  return coaxial_components_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_components_wheel(at(state), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_components_cursor(at(state)); }

// The atlas (coax/web/gpui.js): what to rasterize, then where it went.
EXPORT("coaxial_gpui_glyphs") int32_t gpui_glyphs(void) { return (int32_t)coaxial_components_glyph_count(); }
EXPORT("coaxial_gpui_glyph_code") int32_t gpui_glyph_code(int32_t g) { return (int32_t)coaxial_components_glyph_code(g); }
EXPORT("coaxial_gpui_fonts") int32_t gpui_fonts(void) { return (int32_t)coaxial_components_font_count(); }
EXPORT("coaxial_gpui_font_size") float gpui_font_size(int32_t k) { return coaxial_components_font_size(k); }
EXPORT("coaxial_gpui_font_weight") int32_t gpui_font_weight(int32_t k) { return (int32_t)coaxial_components_font_weight(k); }
EXPORT("coaxial_gpui_icons") int32_t gpui_icons(void) { return (int32_t)coaxial_components_icon_count(); }
EXPORT("coaxial_gpui_icon_size") float gpui_icon_size(int32_t i) { return coaxial_components_icon_size(i); }
EXPORT("coaxial_gpui_icon_name") int32_t gpui_icon_name(uint32_t state, int32_t i) { return (int32_t)coaxial_components_icon_name(at(state), i); }
EXPORT("coaxial_gpui_scratch") uint32_t gpui_scratch(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->scratch.e; }
EXPORT("coaxial_gpui_bytes") uint32_t gpui_bytes(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->sbytes.e; }
EXPORT("coaxial_gpui_set_font") void gpui_set_font(uint32_t state, int32_t slot) { coaxial_components_set_font(at(state), slot); }
EXPORT("coaxial_gpui_set_icon") void gpui_set_icon(uint32_t state, int32_t i, float x, float y, float w, float h) { coaxial_components_set_icon(at(state), i, x, y, w, h); }
EXPORT("coaxial_gpui_set_atlas") void gpui_set_atlas(uint32_t state, float w, float h, int32_t levels) { coaxial_components_set_atlas(at(state), w, h, levels); }
EXPORT("coaxial_gpui_set_dynamic") void gpui_set_dynamic(uint32_t state, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_w, float color_h) {
  coaxial_components_set_dynamic(at(state), size, pen_x, pen_y, y0, cols, color_cols, color_w, color_h);
}
EXPORT("coaxial_gpui_next_cluster") int32_t gpui_next_cluster(uint32_t state) { return (int32_t)coaxial_components_next_cluster(at(state)); }
EXPORT("coaxial_gpui_cluster") int32_t gpui_cluster(uint32_t state, int32_t slot) { return (int32_t)coaxial_components_cluster(at(state), slot); }
EXPORT("coaxial_gpui_measured") void gpui_measured(uint32_t state, int32_t slot, float advance, int32_t color) { coaxial_components_measured(at(state), slot, advance, color != 0); }
EXPORT("coaxial_gpui_next_cell") int32_t gpui_next_cell(uint32_t state) { return (int32_t)coaxial_components_next_cell(at(state)); }
EXPORT("coaxial_gpui_cell") int32_t gpui_cell(uint32_t state, int32_t cell) { return coaxial_components_cell(at(state), cell) ? 1 : 0; }
EXPORT("coaxial_gpui_cell_ready") void gpui_cell_ready(uint32_t state, int32_t cell) { coaxial_components_cell_ready(at(state), cell); }
