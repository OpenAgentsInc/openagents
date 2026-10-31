// The C face over zeron/screen.coax, the Zeron-fork chat screen, for the
// Swift host (Sources/Zeron.swift). Based on Zeron by Wing, MIT.
#include "zeron.h"
#include "face.h"
#include <stdlib.h>
#include <string.h>
#include <time.h>

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
void zr_fixture(void *state, int32_t open) { if (open == 2) coaxial_zeron_fixture_table(at(state)); else coaxial_zeron_fixture(at(state), open != 0); }
void zr_scroll_to(void *state, float px) { coaxial_zeron_scroll_to(at(state), px); }
// The chat's engine (zeron/claude.coax): its buffers, the folder Claude Code
// works in, and what the host does for it.
#define ZR_POOL (8 << 20)
#define ZR_LINE (1 << 20)
#define ZR_ARENA (1 << 20)
#define ZR_OUT (32 << 20)      /* a message's frame: up to sixteen 5 MB images as base64 */
#define ZR_STORE (48 << 20)    /* the attachments' bytes, and the message as its engine reads it */
#define ZR_SCRATCH (64 << 10)
void zr_attach(void *state, const uint8_t *cwd, int32_t length) {
  uint8_t *pool = malloc(ZR_POOL), *line = malloc(ZR_LINE), *arena = malloc(ZR_ARENA), *out = malloc(ZR_OUT), *scratch = malloc(ZR_SCRATCH);
  if (!pool || !line || !arena || !out || !scratch) return;
  coaxial_zeron_attach(at(state), (coax_v_u8){pool, ZR_POOL}, (coax_v_u8){line, ZR_LINE}, (coax_v_u8){arena, ZR_ARENA},
                       (coax_v_u8){out, ZR_OUT}, (coax_v_u8){scratch, ZR_SCRATCH}, str(cwd, length));
  uint8_t *store = malloc(ZR_STORE);
  if (store) coaxial_zeron_attach_store(at(state), (coax_v_u8){store, ZR_STORE});
  // The Codex engine (zeron/codex.coax): itself and its buffers, kept for the app's life.
  coax_t_CxEngine *codex = calloc(1, sizeof(coax_t_CxEngine));
  int64_t cmem = coaxial_zeron_codex_mem();
  uint8_t *cline = malloc(ZR_LINE), *carena = malloc(ZR_ARENA), *cout = malloc(ZR_LINE), *cm = malloc((size_t)cmem);
  if (codex && cline && carena && cout && cm)
    coaxial_zeron_attach_codex(at(state), (coax_v_t_CxEngine){codex, 1}, (coax_v_u8){cline, ZR_LINE}, (coax_v_u8){carena, ZR_ARENA},
                               (coax_v_u8){cout, ZR_LINE}, (coax_v_u8){cm, cmem});
  // Microcoder for the Lab's other route (zeron/microcoder.coax): its loop, plan and buffers, kept for the app's life.
  coax_t_LabMc *mc = calloc(1, sizeof(coax_t_LabMc));
  if (mc) coaxial_zeron_attach_mc(at(state), (coax_v_t_LabMc){mc, 1});
}
// The clock Codex's usage-limit notices count from (zeron/codex.coax).
int64_t zr_unix_s(void) { return (int64_t)time(NULL); }
int64_t zr_random_u64(void) { int64_t v; arc4random_buf(&v, sizeof v); return v; }   // microcoder.coax: aux for our route's signatures
int64_t zr_lab_unix_s(void) { return (int64_t)time(NULL); }

// The Lab's NIP-98 signer (lab-sign.coax) for microcoder.coax, a byte at a time.
typedef struct { const char *data; int64_t count; } zr_sign_string;
typedef struct { uint8_t *data; int64_t count; } zr_sign_bytes;
int64_t coaxial_lab_nip98(zr_sign_bytes key, zr_sign_bytes aux, int64_t now_s, zr_sign_string tags, zr_sign_bytes out);
static uint8_t zr_sk[32], zr_st[700], zr_so[2048];
static int64_t zr_stn, zr_son;
void zr_sign_reset(void) { memset(zr_sk, 0, sizeof zr_sk); zr_stn = 0; zr_son = 0; }
void zr_sign_key(int64_t i, int64_t b) { if (i >= 0 && i < 32) zr_sk[i] = (uint8_t)b; }
_Bool zr_sign_tag(int64_t b) { if (zr_stn >= (int64_t)sizeof zr_st) return 0; zr_st[zr_stn++] = (uint8_t)b; return 1; }
int64_t zr_sign_run(int64_t now_s, int64_t a0, int64_t a1, int64_t a2, int64_t a3) {
  uint8_t aux[32]; int64_t a[4] = {a0, a1, a2, a3};
  memcpy(aux, a, sizeof aux);
  zr_son = coaxial_lab_nip98((zr_sign_bytes){zr_sk, 32}, (zr_sign_bytes){aux, 32}, now_s, (zr_sign_string){(const char *)zr_st, zr_stn}, (zr_sign_bytes){zr_so, sizeof zr_so});
  memset(zr_sk, 0, sizeof zr_sk); memset(aux, 0, sizeof aux);
  return zr_son;
}
int64_t zr_sign_byte(int64_t i) { return i >= 0 && i < zr_son ? zr_so[i] : 0; }
// Attachments (zeron/attachments.coax): a file the host staged, its bytes (up to 5 MB) or a failure.
void zr_attach_file(void *state, const uint8_t *path, int32_t path_len, const uint8_t *name, int32_t name_len, const uint8_t *bytes, int32_t length, int32_t failure) {
  coaxial_zeron_attach_file(at(state), str(path, path_len), str(name, name_len), (coax_v_u8){(uint8_t *)bytes, bytes ? length : 0}, failure);
}
int32_t zr_choose_named(void *state, int32_t which, const uint8_t *name, int32_t length) { return coaxial_zeron_choose_named(at(state), which, str(name, length)) ? 1 : 0; }
void zr_open_menu(void *state, int32_t which) { coaxial_zeron_open_menu(at(state), which); }
void zr_demo(void *state) { coaxial_zeron_demo(at(state)); }
void zr_new_chat(void *state) { coaxial_zeron_new_chat(at(state)); }
void zr_choose(void *state, int32_t which, int32_t index) { coaxial_zeron_choose(at(state), which, index); }
int32_t zr_live(void *state) { return coaxial_zeron_live(at(state)) ? 1 : 0; }
int32_t zr_take_action(void *state) { return (int32_t)coaxial_zeron_take_action(at(state)); }
void zr_quit(void *state) { coaxial_zeron_quit(at(state)); }

// The desk (zeron/desk.coax): every session's chat, the sidebar and the tabs.
static coax_v_t_Desk desk_at(void *desk) { return (coax_v_t_Desk){(coax_t_Desk *)desk, 1}; }
void *dsk_new(void) { return calloc(1, sizeof(coax_t_Desk)); }
int32_t dsk_add(void *desk, void *state, const uint8_t *cwd, int32_t length) { return (int32_t)coaxial_desk_add(desk_at(desk), at(state), str(cwd, length)); }
void *dsk_active(void *desk) {
  coax_t_Desk *k = desk;
  return k->n > 0 ? (void *)k->chats.e[k->active].s.data : NULL;
}
void *dsk_chat(void *desk, int32_t i) {
  coax_t_Desk *k = desk;
  return i >= 0 && i < k->n ? (void *)k->chats.e[i].s.data : NULL;
}
int32_t dsk_count(void *desk) { return (int32_t)coaxial_desk_count(desk_at(desk)); }
int32_t dsk_frame(void *desk, float *vertices, float width, float height, float dpr, float dt) {
  coax_v_f32 out = {vertices, coaxial_zeron_max_vertices() * coaxial_zeron_vertex_floats()};
  int64_t n = coaxial_desk_frame(desk_at(desk), out, width, height, dpr, dt);
  return n < 0 ? 0 : (int32_t)n;
}
int32_t dsk_pointer(void *desk, float x, float y, int32_t phase, int32_t clicks, int32_t mods) { return coaxial_desk_pointer(desk_at(desk), x, y, phase, clicks, mods) ? 1 : 0; }
int32_t dsk_wheel(void *desk, float dy) { return coaxial_desk_wheel(desk_at(desk), dy) ? 1 : 0; }
int32_t dsk_key(void *desk, int32_t key, int32_t mods) { return coaxial_desk_key(desk_at(desk), key, mods) ? 1 : 0; }
int32_t dsk_close_tab(void *desk) { return coaxial_desk_close_tab(desk_at(desk)) ? 1 : 0; }
void dsk_toggle(void *desk) { coaxial_desk_toggle(desk_at(desk)); }
float dsk_side(void *desk) { return coaxial_desk_side(desk_at(desk)); }
int32_t dsk_cursor(void *desk) { return (int32_t)coaxial_desk_cursor(desk_at(desk)); }
int32_t dsk_take_action(void *desk) { return (int32_t)coaxial_desk_take_action(desk_at(desk)); }
int32_t dsk_cwd(void *desk, uint8_t *out, int32_t cap) { return (int32_t)coaxial_desk_cwd(desk_at(desk), (coax_v_u8){out, cap}); }
void dsk_quit(void *desk) { coaxial_desk_quit(desk_at(desk)); }
int32_t dsk_tabs(void *desk) { return (int32_t)coaxial_desk_tabs(desk_at(desk)); }
int32_t dsk_tab(void *desk, int32_t t) { return (int32_t)coaxial_desk_tab(desk_at(desk), t); }
int32_t dsk_collapsed(void *desk) { return coaxial_desk_collapsed(desk_at(desk)) ? 1 : 0; }
int32_t zr_answer(void *state, int32_t allow) { return coaxial_zeron_answer(at(state), allow != 0) ? 1 : 0; }
int32_t zr_status_text(void *state, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_status_text(at(state), (coax_v_u8){out, cap}); }
int32_t zr_last_text(void *state, uint8_t *out, int32_t cap) { return (int32_t)coaxial_zeron_last_text(at(state), (coax_v_u8){out, cap}); }
void dsk_set_room(void *desk, const uint8_t *exe, int32_t exe_n, const uint8_t *sock, int32_t sock_n) { coaxial_desk_set_room(desk_at(desk), str(exe, exe_n), str(sock, sock_n)); }
int32_t dsk_room_line(void *desk, int32_t conn, const uint8_t *line, int32_t n) { return coaxial_desk_room_line(desk_at(desk), conn, str(line, n)) ? 1 : 0; }
int32_t dsk_room_out(void *desk, int32_t *conn, uint8_t *out, int32_t cap) {
  int64_t c = -1;
  int32_t n = (int32_t)coaxial_desk_room_out(desk_at(desk), (coax_v_s64){&c, 1}, (coax_v_u8){out, cap});
  *conn = (int32_t)c; return n;
}
int32_t room_mcp(const uint8_t *line, int32_t n, int32_t agent, uint8_t *out, int32_t cap, int32_t *len) {
  int64_t l = 0;
  int32_t kind = (int32_t)coaxial_room_mcp(str(line, n), agent, (coax_v_u8){out, cap}, (coax_v_s64){&l, 1});
  *len = (int32_t)l; return kind;
}
int32_t room_mcp_result(const uint8_t *reply, int32_t n, uint8_t *out, int32_t cap) { return (int32_t)coaxial_room_mcp_result(str(reply, n), (coax_v_u8){out, cap}); }
static coax_v_t_Settings ds_at(void *g) { return (coax_v_t_Settings){(coax_t_Settings *)g, 1}; }
void *ds_new(void) { return calloc(1, sizeof(coax_t_Settings)); }
int32_t ds_save(void *desk, float x, float y, float w, float h, uint8_t *out, int32_t cap) { return (int32_t)coaxial_ds_save(desk_at(desk), x, y, w, h, (coax_v_u8){out, cap}); }
int32_t ds_load(void *g, void *desk, const uint8_t *json, int32_t n) { return (int32_t)coaxial_ds_load(ds_at(g), desk_at(desk), str(json, n)); }
int32_t ds_window(void *g, float *out) { return coaxial_ds_window(ds_at(g), (coax_v_f32){out, 4}) ? 1 : 0; }
int32_t ds_restore_cwd(void *g, int32_t i, uint8_t *out, int32_t cap) { return (int32_t)coaxial_ds_restore_cwd(ds_at(g), i, (coax_v_u8){out, cap}); }
void ds_restore(void *g, void *desk, int32_t i, int32_t c) { coaxial_ds_restore(ds_at(g), desk_at(desk), i, c); }
void ds_restore_tabs(void *g, void *desk, int32_t base) { coaxial_ds_restore_tabs(ds_at(g), desk_at(desk), base); }
void ds_defaults(void *g, void *state) { coaxial_ds_defaults(ds_at(g), at(state)); }
int32_t dsk_room_shown(void *desk) { return coaxial_desk_room_shown(desk_at(desk)) ? 1 : 0; }
void dsk_show_room(void *desk) { coaxial_desk_show_room(desk_at(desk)); }
void dsk_set_store(void *desk, int32_t i, const uint8_t *id, int32_t n) { coaxial_desk_set_store(desk_at(desk), i, str(id, n)); }
int32_t dsk_store_id(void *desk, int32_t i, uint8_t *out, int32_t cap) { return (int32_t)coaxial_desk_store_id(desk_at(desk), i, (coax_v_u8){out, cap}); }
int32_t dsk_store_take(void *desk, int32_t i, int32_t force, uint8_t *out, int32_t cap) { return (int32_t)coaxial_desk_store_take(desk_at(desk), i, force != 0, (coax_v_u8){out, cap}); }
void dsk_store_load(void *desk, int32_t i, const uint8_t *text, int32_t n) { coaxial_desk_store_load(desk_at(desk), i, str(text, n)); }
void dsk_still(void *desk) { coaxial_desk_still(desk_at(desk)); }
int32_t dsk_shortcut(void *desk, int32_t j, uint8_t *out, int32_t cap) { return (int32_t)coaxial_desk_shortcut(desk_at(desk), j, (coax_v_u8){out, cap}); }
