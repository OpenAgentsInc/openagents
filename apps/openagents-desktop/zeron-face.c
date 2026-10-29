// The C face over zeron/screen.coax, the Zeron-fork chat screen, for the
// Swift host (Sources/Zeron.swift). Based on Zeron by Wing, MIT.
#include "zeron.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_ZeronState at(void *state) { return (coax_v_t_ZeronState){(coax_t_ZeronState *)state, 1}; }

void *zr_new(void) { return calloc(1, sizeof(coax_t_ZeronState)); }
int32_t zr_vertex_floats(void) { return (int32_t)coaxial_zeron_vertex_floats(); }
int32_t zr_max_vertices(void) { return (int32_t)coaxial_zeron_max_vertices(); }
int32_t zr_glyph_count(void) { return (int32_t)coaxial_zeron_glyph_count(); }
uint32_t zr_glyph_code(int32_t g) { return (uint32_t)coaxial_zeron_glyph_code(g); }
int32_t zr_font_count(void) { return (int32_t)coaxial_zeron_font_count(); }
float zr_font_size(int32_t slot) { return coaxial_zeron_font_size(slot); }
int32_t zr_font_weight(int32_t slot) { return (int32_t)coaxial_zeron_font_weight(slot); }
int32_t zr_icon_count(void) { return (int32_t)coaxial_zeron_icon_count(); }
float zr_icon_size(int32_t i) { return coaxial_zeron_icon_size(i); }
int32_t zr_icon_name(int32_t i, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_icon_name(i, (coax_v_u8){out, cap}); }
void zr_set_font(void *state, int32_t slot, const float *header, const float *advance, const float *kern) {
  int64_t g = coaxial_zeron_glyph_count();
  coaxial_zeron_set_font(at(state), slot, (coax_v_f32){(float *)header, 7}, (coax_v_f32){(float *)advance, g}, (coax_v_f32){(float *)kern, g * g});
}
void zr_set_icon(void *state, int32_t i, float x, float y, float w, float h) { coaxial_zeron_set_icon(at(state), i, x, y, w, h); }
void zr_set_atlas(void *state, float width, float height) { coaxial_zeron_set_atlas(at(state), width, height); }
int32_t zr_frame(void *state, float *vertices, float width, float height, float dpr, float dt) {
  coax_v_f32 out = {vertices, coaxial_zeron_max_vertices() * coaxial_zeron_vertex_floats()};
  int64_t n = coaxial_zeron_frame(at(state), out, width, height, dpr, dt);
  return n < 0 ? 0 : (int32_t)n;
}
int32_t zr_pointer(void *state, float x, float y, int32_t phase, int32_t clicks, int32_t mods) { return coaxial_zeron_pointer(at(state), x, y, phase, clicks, mods) ? 1 : 0; }
int32_t zr_wheel(void *state, float dy) { return coaxial_zeron_wheel(at(state), dy) ? 1 : 0; }
int32_t zr_key(void *state, int32_t key, int32_t mods) { return coaxial_zeron_key(at(state), key, mods) ? 1 : 0; }
void zr_text(void *state, const uint8_t *bytes, int32_t length) { coaxial_zeron_text(at(state), (coax_string){(const char *)bytes, length}); }
int32_t zr_wants_text(void *state) { return coaxial_zeron_wants_text(at(state)) ? 1 : 0; }
int32_t zr_cursor(void *state) { return (int32_t)coaxial_zeron_cursor(at(state)); }
static coax_string str(const uint8_t *bytes, int32_t length) { return (coax_string){(const char *)bytes, length}; }
void zr_undo(void *state) { coaxial_zeron_undo(at(state)); }
void zr_redo(void *state) { coaxial_zeron_redo(at(state)); }
int32_t zr_can_undo(void *state) { return coaxial_zeron_can_undo(at(state)) ? 1 : 0; }
int32_t zr_can_redo(void *state) { return coaxial_zeron_can_redo(at(state)) ? 1 : 0; }
void zr_select_all(void *state) { coaxial_zeron_select_all(at(state)); }
int32_t zr_selected(void *state, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_selected(at(state), (coax_v_u8){out, cap}); }
int32_t zr_selected_count(void *state) { return (int32_t)coaxial_zeron_selected_count(at(state)); }
void zr_cut(void *state) { coaxial_zeron_cut(at(state)); }
void zr_paste(void *state, const uint8_t *bytes, int32_t length) { coaxial_zeron_paste(at(state), str(bytes, length)); }
void zr_replace(void *state, int32_t from16, int32_t to16, const uint8_t *bytes, int32_t length) { coaxial_zeron_replace(at(state), from16, to16, str(bytes, length)); }
void zr_mark(void *state, int32_t from16, int32_t to16, const uint8_t *bytes, int32_t length, int32_t sel_from16, int32_t sel_to16) {
  coaxial_zeron_mark(at(state), from16, to16, str(bytes, length), sel_from16, sel_to16);
}
void zr_unmark(void *state) { coaxial_zeron_unmark(at(state)); }
int32_t zr_range16(void *state, int32_t which) { return (int32_t)coaxial_zeron_range16(at(state), which); }
int32_t zr_text16(void *state, int32_t from16, int32_t to16, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_text16(at(state), from16, to16, (coax_v_u8){out, cap}); }
int32_t zr_rect16(void *state, int32_t at16, float *out) { return coaxial_zeron_rect16(at(state), at16, (coax_v_f32){out, 4}) ? 1 : 0; }
int32_t zr_index16(void *state, float x, float y) { return (int32_t)coaxial_zeron_index16(at(state), x, y); }
void zr_set_dynamic(void *state, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_width, float color_height) {
  coaxial_zeron_set_dynamic(at(state), size, pen_x, pen_y, y0, cols, color_cols, color_width, color_height);
}
int32_t zr_next_cluster(void *state) { return (int32_t)coaxial_zeron_next_cluster(at(state)); }
int32_t zr_cluster(void *state, int32_t slot, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_cluster(at(state), slot, (coax_v_u8){out, cap}); }
void zr_measured(void *state, int32_t slot, float advance, int32_t color) { coaxial_zeron_measured(at(state), slot, advance, color != 0); }
int32_t zr_next_cell(void *state) { return (int32_t)coaxial_zeron_next_cell(at(state)); }
int32_t zr_cell(void *state, int32_t cell, float *out) { return coaxial_zeron_cell(at(state), cell, (coax_v_f32){out, 6}) ? 1 : 0; }
void zr_cell_ready(void *state, int32_t cell) { coaxial_zeron_cell_ready(at(state), cell); }
int32_t zr_can_cut(void *state) { return coaxial_zeron_can_cut(at(state)) ? 1 : 0; }
int32_t zr_take_copy(void *state, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_take_copy(at(state), (coax_v_u8){out, cap}); }
void zr_fixture(void *state, int32_t open) { coaxial_zeron_fixture(at(state), open != 0); }
void zr_scroll_to(void *state, float px) { coaxial_zeron_scroll_to(at(state), px); }
// The chat's engine (zeron/claude.coax): its buffers, the folder Claude Code
// works in, and what the host does for it.
#define ZR_POOL (8 << 20)
#define ZR_LINE (1 << 20)
#define ZR_ARENA (1 << 20)
#define ZR_OUT (256 << 10)
#define ZR_SCRATCH (64 << 10)
void zr_attach(void *state, const uint8_t *cwd, int32_t length) {
  uint8_t *pool = malloc(ZR_POOL), *line = malloc(ZR_LINE), *arena = malloc(ZR_ARENA), *out = malloc(ZR_OUT), *scratch = malloc(ZR_SCRATCH);
  if (!pool || !line || !arena || !out || !scratch) return;
  coaxial_zeron_attach(at(state), (coax_v_u8){pool, ZR_POOL}, (coax_v_u8){line, ZR_LINE}, (coax_v_u8){arena, ZR_ARENA},
                       (coax_v_u8){out, ZR_OUT}, (coax_v_u8){scratch, ZR_SCRATCH}, str(cwd, length));
}
void zr_demo(void *state) { coaxial_zeron_demo(at(state)); }
void zr_choose(void *state, int32_t which, int32_t index) { coaxial_zeron_choose(at(state), which, index); }
int32_t zr_live(void *state) { return coaxial_zeron_live(at(state)) ? 1 : 0; }
int32_t zr_take_action(void *state) { return (int32_t)coaxial_zeron_take_action(at(state)); }
void zr_quit(void *state) { coaxial_zeron_quit(at(state)); }
