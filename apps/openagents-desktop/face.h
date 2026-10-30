// The Mac app's C face over chat.coax (apps/openagents-web/chat-hud.coax): the
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
int32_t zr_pointer(void *state, float x, float y, int32_t phase, int32_t clicks, int32_t mods);
int32_t zr_wheel(void *state, float dy);
void zr_undo(void *state);
void zr_redo(void *state);
int32_t zr_can_undo(void *state);
int32_t zr_can_redo(void *state);
void zr_select_all(void *state);
int32_t zr_selected(void *state, uint8_t *out, int32_t cap);
int32_t zr_selected_count(void *state);
void zr_cut(void *state);
void zr_paste(void *state, const uint8_t *bytes, int32_t length);
void zr_replace(void *state, int32_t from16, int32_t to16, const uint8_t *bytes, int32_t length);
void zr_mark(void *state, int32_t from16, int32_t to16, const uint8_t *bytes, int32_t length, int32_t sel_from16, int32_t sel_to16);
void zr_unmark(void *state);
int32_t zr_range16(void *state, int32_t which);
int32_t zr_text16(void *state, int32_t from16, int32_t to16, uint8_t *out, int32_t cap);
int32_t zr_rect16(void *state, int32_t at16, float *out);
int32_t zr_index16(void *state, float x, float y);
void zr_set_dynamic(void *state, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_width, float color_height);
int32_t zr_next_cluster(void *state);
int32_t zr_cluster(void *state, int32_t slot, uint8_t *out, int32_t cap);
void zr_measured(void *state, int32_t slot, float advance, int32_t color);
int32_t zr_next_cell(void *state);
int32_t zr_cell(void *state, int32_t cell, float *out);
void zr_cell_ready(void *state, int32_t cell);
int32_t zr_key(void *state, int32_t key, int32_t mods);
void zr_text(void *state, const uint8_t *bytes, int32_t length);
int32_t zr_wants_text(void *state);
int32_t zr_cursor(void *state);
// The transcript (zeron/transcript.coax): whether Cut applies (the composer's
// selection), a code block's pressed Copy, the fixture capture's open state
// and a scroll from the top in px (the --shot steps).
int32_t zr_can_cut(void *state);
int32_t zr_take_copy(void *state, uint8_t *out, int32_t cap);
void zr_fixture(void *state, int32_t open);
void zr_scroll_to(void *state, float px);
// The chat's engine (zeron/claude.coax): the user's own Claude Code. attach
// gives it its buffers and the folder it works in (without it a message
// only joins the transcript); demo shows Zeron's demo conversation instead;
// choose picks a chip's choice (1 model: 0 Opus, 1 Sonnet, 2 Haiku; 2
// effort: 0 Low .. 4 Max; 3 access: 0 Full access, 1 Auto-accept edits,
// 2 Supervised); live says a turn is running; take_action is a notice's
// button for the host (1 log in, 2 install), once; quit ends Claude Code.
void zr_attach(void *state, const uint8_t *cwd, int32_t length);
void zr_demo(void *state);
// Cmd+N: a new, empty chat (a running turn stops; the engines start a new conversation).
void zr_new_chat(void *state);
void zr_choose(void *state, int32_t which, int32_t index);
int32_t zr_live(void *state);
int32_t zr_take_action(void *state);
void zr_quit(void *state);
// The composer's menus and attachments (zeron/picker.coax, attachments.coax):
// choose_named picks a model or effort by id or name (1 model, 2 effort),
// open_menu opens a chip's menu (1 model, 3 access); take_action 3 asks for
// files to attach; attach_file hands over one the host staged (its staged
// path, its name, its bytes up to 5 MB, or failure 1 too large, 2 not a
// file, 3 unreadable).
int32_t zr_choose_named(void *state, int32_t which, const uint8_t *name, int32_t length);
void zr_open_menu(void *state, int32_t which);
void zr_attach_file(void *state, const uint8_t *path, int32_t path_len, const uint8_t *name, int32_t name_len, const uint8_t *bytes, int32_t length, int32_t failure);
