// The Mac app's C face over chat.coax (apps/openagents-web/chat.coax): the
// same calls host.js makes of chat.wasm, as plain C for Swift.
#pragma once
#include <stdint.h>

typedef int32_t (*dk_http_fn)(const uint8_t *url, int32_t url_len, const uint8_t *body, int32_t body_len, int32_t post);

void *dk_new(dk_http_fn http);
int32_t dk_vertex_floats(void);
int32_t dk_uniform_floats(void);
int32_t dk_max_vertices(void);
int32_t dk_frame(void *state, float *vertices, float *uniforms, float width, float height, float dpr, float dt);
int32_t dk_pointer(void *state, float x, float y, int32_t phase);
int32_t dk_key(void *state, int32_t key);
void dk_wheel(void *state, float dy);
void dk_message(void *state, int32_t kind, const uint8_t *bytes, int32_t length);
int32_t dk_wants_text(void *state);
int32_t dk_cursor(void *state);
// A navigation chat.coax asked for (its outbox), copied into url; 0 when none.
int32_t dk_take_action(void *state, uint8_t *url, int32_t cap);
// Mammoth's routing decision (router.coax): state is {"message", "environment"} JSON; the
// checked response goes in out and its length is returned, or a reason and its length negated.
int32_t dk_route(const uint8_t *state, int32_t length, uint8_t *out, int32_t cap);

// The Zeron-fork chat screen (zeron/screen.coax, zeron-face.c): what the
// host's atlas holds (glyphs, fonts, icons), its measurements in, and the
// frame and input calls.
void *zr_new(void);
int32_t zr_vertex_floats(void);
int32_t zr_max_vertices(void);
int32_t zr_glyph_count(void);
uint32_t zr_glyph_code(int32_t g);
int32_t zr_font_count(void);
float zr_font_size(int32_t slot);
int32_t zr_font_weight(int32_t slot);
int32_t zr_icon_count(void);
float zr_icon_size(int32_t i);
int32_t zr_icon_name(int32_t i, uint8_t *out, int32_t cap);
// header: ascent, descent, cell, pen x, pen y, first cell's y, columns; advance: per glyph; kern: glyphs x glyphs.
void zr_set_font(void *state, int32_t slot, const float *header, const float *advance, const float *kern);
void zr_set_icon(void *state, int32_t i, float x, float y, float w, float h);
void zr_set_atlas(void *state, float width, float height);
int32_t zr_frame(void *state, float *vertices, float width, float height, float dpr, float dt);
int32_t zr_pointer(void *state, float x, float y, int32_t phase);
int32_t zr_key(void *state, int32_t key, int32_t mods);
void zr_text(void *state, const uint8_t *bytes, int32_t length);
int32_t zr_wants_text(void *state);
int32_t zr_cursor(void *state);
