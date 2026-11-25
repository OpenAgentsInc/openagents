// Firm reason: 6 (thin C wrapper over gamebuilder-core.o exports for Swift Bridging Header).
// Embeds gamebuilder/core/ inside OpenAgents Desktop (GitHub #14043) while keeping contexts.txt acyclic.
#ifndef OA_GB_HOST_H
#define OA_GB_HOST_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void *oa_gb_alloc(float w, float h);
void oa_gb_free(void *handle);
void oa_gb_set_size(void *handle, float w, float h);
void oa_gb_account(void *handle, const char *email);

// Single command-table choke point (100% routed through GB_COMMANDS / GB_CONTROLS)
int oa_gb_exec_verb(void *handle, const char *verb, const char *args_json, const int32_t *levels, int nlevels, char *out_json, int out_cap);
int oa_gb_exec_ctrl(void *handle, const char *ctrl_id, const char *args_json, const int32_t *levels, int nlevels, char *out_json, int out_cap);
int oa_gb_get_table_digest(char out_hex9[9]);

// Persistence
int oa_gb_load_level_buf(void *handle, int32_t level_n, const uint8_t *bytes, int len);
int oa_gb_load_game_buf(void *handle, int32_t game_idx, const uint8_t *bytes, int len);
int oa_gb_save_level_buf(void *handle, uint8_t *out, int cap);
int oa_gb_is_save_due(void *handle);
void oa_gb_clear_save_due(void *handle);
int32_t oa_gb_poll_more_due(void *handle);
int oa_gb_save_more_buf(void *handle, uint8_t *out, int cap);
int32_t oa_gb_poll_open_due(void *handle);
int32_t oa_gb_poll_open_game_due(void *handle);

// Input & Simulation
void oa_gb_mouse_down(void *handle, int32_t button, float x, float y, int32_t mods);
void oa_gb_mouse_move(void *handle, float x, float y);
void oa_gb_mouse_up(void *handle, int32_t button, float x, float y);
void oa_gb_scroll_wheel(void *handle, float notches);
void oa_gb_send_key(void *handle, int32_t key, int down, int32_t mods);
int oa_gb_step_tick(void *handle, float dt);
int oa_gb_consume_dirty(void *handle);
void oa_gb_set_play_look(void *handle, float dx, float dy);
void oa_gb_set_play_fire(void *handle, int down);
void oa_gb_trigger_play_use(void *handle);
void oa_gb_toggle_play_view(void *handle);
int oa_gb_configure_play(void *handle, int32_t mode, int32_t team_size);
void oa_gb_focus_cm(void *handle, int32_t x_cm, int32_t z_cm);

// Render Pipeline 1: Instance stream + .gbm meshes
int oa_gb_get_cube(float *out, int float_cap);
int oa_gb_get_camera(void *handle, float out20[20]);
int oa_gb_get_instance_max(void);
int oa_gb_get_draw_instances(void *handle, float *out, int float_cap);
int oa_gb_load_gbm(void *handle, const uint8_t *bytes, int len);
int oa_gb_get_mesh_count(void *handle);
int64_t oa_gb_get_mesh_vertices_at(void *handle);
int oa_gb_get_mesh_range(void *handle, int32_t id, int32_t out2[2]);
int oa_gb_get_play_hud(void *handle, float *out, int float_cap);
int oa_gb_get_sky(void *handle, float out12[12]);

// Render Pipeline 2 & 3: SPLICEBOTS UE Farm & world.coax (9 web worlds)
int oa_gb_load_farm_pack(void *handle, const uint8_t *bytes, int len);
int oa_gb_is_farm_shown(void *handle);
int oa_gb_get_farm_draws(void *handle);
void oa_gb_set_world_attached(void *handle, int on);
int oa_gb_is_world_drawn(void *handle);
int oa_gb_is_world_playing(void *handle);
int oa_gb_step_world(void *handle, float w, float h, float dpr, float dt);

// CoreAudio Play Sound Synthesis
int oa_gb_get_sound_rate(void);
int oa_gb_get_sound_count(void);
int oa_gb_get_sound_pcm(int32_t id, float *out, int sample_cap);
int oa_gb_poll_sound_events(void *handle, int32_t *out, int cap);

// Live --attach Unix Socket, Claims, Pictures & UI Snapshot
int oa_gb_listen_attach(void *handle, const char *socket_path);
int oa_gb_poll_attach(void *handle, const int32_t *levels, int nlevels);
void oa_gb_close_attach(void *handle);
void oa_gb_set_claims_clock(void *handle, int64_t now_sec);
int oa_gb_get_pic_due(void *handle);
int oa_gb_submit_pic(void *handle, const uint8_t *px, int len, int32_t w, int32_t h, int32_t stride, int bgra, int64_t made);
int oa_gb_snapshot_ui(void *handle, int32_t out_i26[26], int64_t out_cam9[9], char *out_label, int label_cap, char *out_note, int note_cap);
int oa_gb_get_tree_tsv(void *handle, char *out, int cap);

#ifdef __cplusplus
}
#endif

#endif
