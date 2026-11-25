// Firm reason: 6 (thin C wrapper mapping Swift pointers to Coax slice ABI in gamebuilder-core.o).
#include "gb_host.h"
#include "gamebuilder-core.h"
#include <stdlib.h>
#include <string.h>

static coax_v_t_GbDesktopState gb_view(void *h) {
  return (coax_v_t_GbDesktopState){(coax_t_GbDesktopState *)h, h ? 1 : 0};
}

static coax_string gb_str(const char *s) {
  if (!s) return (coax_string){NULL, 0};
  return (coax_string){s, (int64_t)strlen(s)};
}

void *oa_gb_alloc(float w, float h) {
  int64_t sz = oa_gb_state_size();
  if (sz <= 0) return NULL;
  void *p = calloc(1, (size_t)sz);
  if (p) oa_gb_init(gb_view(p), w, h);
  return p;
}

void oa_gb_free(void *handle) {
  if (!handle) return;
  oa_gb_attach_close(gb_view(handle));
  free(handle);
}

void oa_gb_set_size(void *handle, float w, float h) {
  if (handle) oa_gb_resize(gb_view(handle), w, h);
}

void oa_gb_account(void *handle, const char *email) {
  if (handle && email) oa_gb_set_account(gb_view(handle), gb_str(email));
}

int oa_gb_exec_verb(void *handle, const char *verb, const char *args_json, const int32_t *levels, int nlevels, char *out_json, int out_cap) {
  if (!handle || !verb || !out_json || out_cap < 2) return 0;
  coax_v_s32 lv = {(int32_t *)levels, (levels && nlevels > 0) ? (int64_t)nlevels : 0};
  int64_t r = oa_gb_run_verb(gb_view(handle), gb_str(verb), gb_str(args_json ? args_json : ""), lv, (coax_v_u8){(uint8_t *)out_json, (int64_t)(out_cap - 1)});
  int64_t n = r < 0 ? -r : r;
  if (n >= out_cap) n = out_cap - 1;
  out_json[n] = '\0';
  return (int)r;
}

int oa_gb_exec_ctrl(void *handle, const char *ctrl_id, const char *args_json, const int32_t *levels, int nlevels, char *out_json, int out_cap) {
  if (!handle || !ctrl_id || !out_json || out_cap < 2) return 0;
  coax_v_s32 lv = {(int32_t *)levels, (levels && nlevels > 0) ? (int64_t)nlevels : 0};
  int64_t r = oa_gb_press_ctrl(gb_view(handle), gb_str(ctrl_id), gb_str(args_json ? args_json : ""), lv, (coax_v_u8){(uint8_t *)out_json, (int64_t)(out_cap - 1)});
  int64_t n = r < 0 ? -r : r;
  if (n >= out_cap) n = out_cap - 1;
  out_json[n] = '\0';
  return (int)r;
}

int oa_gb_get_table_digest(char out_hex9[9]) {
  if (!out_hex9) return 0;
  int n = (int)oa_gb_table_digest((coax_v_u8){(uint8_t *)out_hex9, 8});
  if (n < 0) n = 0;
  if (n > 8) n = 8;
  out_hex9[n] = '\0';
  return n;
}

int oa_gb_load_level_buf(void *handle, int32_t level_n, const uint8_t *bytes, int len) {
  if (!handle) return 0;
  return oa_gb_open_level(gb_view(handle), level_n, (coax_v_u8){(uint8_t *)bytes, (bytes && len > 0) ? (int64_t)len : 0}) ? 1 : 0;
}

int oa_gb_load_game_buf(void *handle, int32_t game_idx, const uint8_t *bytes, int len) {
  if (!handle) return 0;
  return oa_gb_open_game(gb_view(handle), game_idx, (coax_v_u8){(uint8_t *)bytes, (bytes && len > 0) ? (int64_t)len : 0}) ? 1 : 0;
}

int oa_gb_save_level_buf(void *handle, uint8_t *out, int cap) {
  if (!handle || !out || cap <= 0) return 0;
  return (int)oa_gb_save_level(gb_view(handle), (coax_v_u8){out, (int64_t)cap});
}

int oa_gb_is_save_due(void *handle) { return handle && oa_gb_save_due(gb_view(handle)) ? 1 : 0; }
void oa_gb_clear_save_due(void *handle) { if (handle) oa_gb_mark_saved(gb_view(handle)); }
int32_t oa_gb_poll_more_due(void *handle) { return handle ? oa_gb_more_due(gb_view(handle)) : 0; }
int oa_gb_save_more_buf(void *handle, uint8_t *out, int cap) {
  if (!handle || !out || cap <= 0) return 0;
  return (int)oa_gb_more_save(gb_view(handle), (coax_v_u8){out, (int64_t)cap});
}
int32_t oa_gb_poll_open_due(void *handle) { return handle ? oa_gb_open_due(gb_view(handle)) : -1; }
int32_t oa_gb_poll_open_game_due(void *handle) { return handle ? oa_gb_open_game_due(gb_view(handle)) : -1; }

void oa_gb_mouse_down(void *handle, int32_t button, float x, float y, int32_t mods) { if (handle) oa_gb_down(gb_view(handle), button, x, y, mods); }
void oa_gb_mouse_move(void *handle, float x, float y) { if (handle) oa_gb_move(gb_view(handle), x, y); }
void oa_gb_mouse_up(void *handle, int32_t button, float x, float y) { if (handle) oa_gb_up(gb_view(handle), button, x, y); }
void oa_gb_scroll_wheel(void *handle, float notches) { if (handle) oa_gb_wheel(gb_view(handle), notches); }
void oa_gb_send_key(void *handle, int32_t key, int down, int32_t mods) { if (handle) oa_gb_key(gb_view(handle), key, down != 0, mods); }
int oa_gb_step_tick(void *handle, float dt) { return handle && oa_gb_tick(gb_view(handle), dt) ? 1 : 0; }
int oa_gb_consume_dirty(void *handle) { return handle && oa_gb_take_dirty(gb_view(handle)) ? 1 : 0; }
void oa_gb_set_play_look(void *handle, float dx, float dy) { if (handle) oa_gb_play_look(gb_view(handle), dx, dy); }
void oa_gb_set_play_fire(void *handle, int down) { if (handle) oa_gb_play_fire(gb_view(handle), down != 0); }
void oa_gb_trigger_play_use(void *handle) { if (handle) oa_gb_play_use(gb_view(handle)); }
void oa_gb_toggle_play_view(void *handle) { if (handle) oa_gb_play_view(gb_view(handle)); }
int oa_gb_configure_play(void *handle, int32_t mode, int32_t team_size) { return handle && oa_gb_play_setup(gb_view(handle), mode, team_size) ? 1 : 0; }
void oa_gb_focus_cm(void *handle, int32_t x_cm, int32_t z_cm) { if (handle) oa_gb_focus_world_pos(gb_view(handle), x_cm, z_cm); }

int oa_gb_get_cube(float *out, int float_cap) {
  if (!out || float_cap <= 0) return 0;
  return (int)oa_gb_cube_verts((coax_v_f32){out, (int64_t)float_cap});
}

int oa_gb_get_camera(void *handle, float out20[20]) {
  if (!handle || !out20) return 0;
  return oa_gb_camera(gb_view(handle), (coax_v_f32){out20, 20}) ? 1 : 0;
}

int oa_gb_get_instance_max(void) { return (int)oa_gb_instance_max(); }

int oa_gb_get_draw_instances(void *handle, float *out, int float_cap) {
  if (!handle || !out || float_cap <= 0) return 0;
  return (int)oa_gb_draw(gb_view(handle), (coax_v_f32){out, (int64_t)float_cap});
}

int oa_gb_load_gbm(void *handle, const uint8_t *bytes, int len) {
  if (!handle || !bytes || len <= 0) return 0;
  return oa_gb_open_meshes(gb_view(handle), (coax_v_u8){(uint8_t *)bytes, (int64_t)len}) ? 1 : 0;
}

int oa_gb_get_mesh_count(void *handle) { return handle ? (int)oa_gb_mesh_count(gb_view(handle)) : 0; }
int64_t oa_gb_get_mesh_vertices_at(void *handle) { return handle ? oa_gb_mesh_vertices_at(gb_view(handle)) : 0; }

int oa_gb_get_mesh_range(void *handle, int32_t id, int32_t out2[2]) {
  if (!handle || !out2) return 0;
  return oa_gb_mesh_range(gb_view(handle), id, (coax_v_s32){out2, 2}) ? 1 : 0;
}

int oa_gb_get_play_hud(void *handle, float *out, int float_cap) {
  if (!handle || !out || float_cap <= 0) return 0;
  return oa_gb_play_hud(gb_view(handle), (coax_v_f32){out, (int64_t)float_cap}) ? 1 : 0;
}

int oa_gb_get_sky(void *handle, float out12[12]) {
  if (!handle || !out12) return 0;
  return oa_gb_sky(gb_view(handle), (coax_v_f32){out12, 12}) ? 1 : 0;
}

int oa_gb_load_farm_pack(void *handle, const uint8_t *bytes, int len) {
  if (!handle || !bytes || len <= 0) return 0;
  return oa_gb_farm_open(gb_view(handle), (coax_v_u8){(uint8_t *)bytes, (int64_t)len}) ? 1 : 0;
}

int oa_gb_is_farm_shown(void *handle) { return handle && oa_gb_farm_shown(gb_view(handle)) ? 1 : 0; }
int oa_gb_get_farm_draws(void *handle) { return handle ? (int)oa_gb_farm_draws(gb_view(handle)) : 0; }
void oa_gb_set_world_attached(void *handle, int on) { if (handle) oa_gb_world_attach(gb_view(handle), on != 0); }
int oa_gb_is_world_drawn(void *handle) { return handle && oa_gb_world_draws(gb_view(handle)) ? 1 : 0; }
int oa_gb_is_world_playing(void *handle) { return handle && oa_gb_world_playing(gb_view(handle)) ? 1 : 0; }
int oa_gb_step_world(void *handle, float w, float h, float dpr, float dt) { return handle ? (int)oa_gb_world_step(gb_view(handle), w, h, dpr, dt) : 0; }

int oa_gb_get_sound_rate(void) { return (int)oa_gb_sound_rate(); }
int oa_gb_get_sound_count(void) { return (int)oa_gb_sound_count(); }

int oa_gb_get_sound_pcm(int32_t id, float *out, int sample_cap) {
  if (!out || sample_cap <= 0) return 0;
  return (int)oa_gb_sound_pcm(id, (coax_v_f32){out, (int64_t)sample_cap});
}

int oa_gb_poll_sound_events(void *handle, int32_t *out, int cap) {
  if (!handle || !out || cap <= 0) return 0;
  return (int)oa_gb_take_events(gb_view(handle), (coax_v_s32){out, (int64_t)cap});
}

int oa_gb_listen_attach(void *handle, const char *socket_path) {
  return (handle && socket_path) && oa_gb_attach_listen(gb_view(handle), gb_str(socket_path)) ? 1 : 0;
}

int oa_gb_poll_attach(void *handle, const int32_t *levels, int nlevels) {
  if (!handle) return 0;
  coax_v_s32 lv = {(int32_t *)levels, (levels && nlevels > 0) ? (int64_t)nlevels : 0};
  return oa_gb_attach_poll(gb_view(handle), lv) ? 1 : 0;
}

void oa_gb_close_attach(void *handle) { if (handle) oa_gb_attach_close(gb_view(handle)); }
void oa_gb_set_claims_clock(void *handle, int64_t now_sec) { if (handle) oa_gb_claims_clock(gb_view(handle), now_sec); }
int oa_gb_get_pic_due(void *handle) { return handle ? (int)oa_gb_pic_due(gb_view(handle)) : 0; }

int oa_gb_submit_pic(void *handle, const uint8_t *px, int len, int32_t w, int32_t h, int32_t stride, int bgra, int64_t made) {
  if (!handle || !px || len <= 0) return 0;
  return oa_gb_pic_put(gb_view(handle), (coax_v_u8){(uint8_t *)px, (int64_t)len}, w, h, stride, bgra != 0, made) ? 1 : 0;
}

int oa_gb_snapshot_ui(void *handle, int32_t out_i26[26], int64_t out_cam9[9], char *out_label, int label_cap, char *out_note, int note_cap) {
  if (!handle || !out_i26 || !out_cam9 || !out_label || label_cap < 2 || !out_note || note_cap < 2) return 0;
  return oa_gb_ui_sync(gb_view(handle), (coax_v_s32){out_i26, 26}, (coax_v_s64){out_cam9, 9},
                       (coax_v_u8){(uint8_t *)out_label, (int64_t)label_cap},
                       (coax_v_u8){(uint8_t *)out_note, (int64_t)note_cap}) ? 1 : 0;
}

int oa_gb_get_tree_tsv(void *handle, char *out, int cap) {
  if (!handle || !out || cap < 2) return 0;
  int n = (int)oa_gb_tree_tsv(gb_view(handle), (coax_v_u8){(uint8_t *)out, (int64_t)(cap - 1)});
  if (n < 0) n = 0;
  if (n >= cap) n = cap - 1;
  out[n] = '\0';
  return n;
}
