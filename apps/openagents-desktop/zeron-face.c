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
int32_t zr_pointer(void *state, float x, float y, int32_t phase) { return coaxial_zeron_pointer(at(state), x, y, phase) ? 1 : 0; }
int32_t zr_key(void *state, int32_t key, int32_t mods) { return coaxial_zeron_key(at(state), key, mods) ? 1 : 0; }
void zr_text(void *state, const uint8_t *bytes, int32_t length) { coaxial_zeron_text(at(state), (coax_string){(const char *)bytes, length}); }
int32_t zr_wants_text(void *state) { return coaxial_zeron_wants_text(at(state)) ? 1 : 0; }
int32_t zr_cursor(void *state) { return (int32_t)coaxial_zeron_cursor(at(state)); }
