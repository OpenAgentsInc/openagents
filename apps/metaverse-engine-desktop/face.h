// Scalar Swift bridge to the Coax setup state. No borrowed Coax storage escapes.
#include <stdbool.h>
#include <stdint.h>
#include "metal-host.h"
int64_t editor_world_backend_bytes(const uint8_t *bytes,int64_t count);
int64_t fwp_target(int64_t launch,int64_t current,bool ready,bool busy);
int64_t fwp_label_byte(int64_t current,int64_t index);
int64_t editor_url_byte(int64_t page, int64_t room, int64_t index);
int64_t editor_offline_url_byte(int64_t page,int64_t index);
int64_t editor_native_url_byte(int64_t page,int64_t index);
int64_t editor_startup_budget(int64_t remaining, int64_t elapsed, bool visible);
int64_t editor_startup_phase(int64_t remaining, int64_t elapsed, bool visible);
bool editor_world_paused(bool ready, bool setup, bool visible, bool minimized, bool occluded);
void *fgu_state(void);
void fgu_update(bool scene,bool checking,bool verifying,bool status,bool paused,const uint8_t *error,int64_t n);
int64_t fgu_draw(float *out,int64_t cap,float w,float h,float d);
int64_t fgu_hit(float w,float h,float x,float y);
float fgu_box(int64_t kind,float w,float h,int64_t field);
// The setup panel's rows for coax drive and VoiceOver (chrome.coax fui_tree, fui_element, fui_about).
int64_t fgu_tree(float w,float h,uint8_t *out,int64_t cap);
int64_t fgu_element(const char *id,int64_t n);
int64_t fgu_about(int64_t button,uint8_t *out,int64_t cap);
int32_t fgu_glyph_count(void);
uint32_t fgu_glyph_code(int32_t g);
int32_t fgu_font_count(void);
float fgu_font_size(int32_t slot);
int32_t fgu_font_weight(int32_t slot);
int32_t fgu_icon_count(void);
float fgu_icon_size(int32_t i);
int32_t fgu_icon_name(int32_t i,uint8_t *out,int32_t cap);
void fgu_set_icon(void *s,int32_t i,float x,float y,float w,float h);
void fgu_set_font(void *s,int32_t slot,const float *h,const float *a,const float *k);
void fgu_set_atlas(void *s,float w,float h);
void fgu_set_dynamic(void *s,float size,float px,float py,float y,int32_t cols,int32_t cc,float cw,float ch);
int32_t fgu_next_cluster(void *s);
int32_t fgu_cluster(void *s,int32_t slot,uint8_t *b,int32_t cap);
void fgu_measured(void *s,int32_t slot,float width,int32_t color);
int32_t fgu_next_cell(void *s);
int32_t fgu_cell(void *s,int32_t cell,float *out);
void fgu_cell_ready(void *s,int32_t cell);
uint64_t editor_process_cpu_ns(int32_t pid);
uint64_t editor_process_resident_bytes(int32_t pid);
int64_t editor_benchmark_url_byte(int64_t page,int64_t index);
void fgs_reset(void);
int64_t fgs_input_byte(int64_t index, int64_t value);
int64_t fgs_accept(int64_t kind, int64_t length);
bool fgs_can_enter(bool scene);
bool fgs_completed(void);
void fgs_microphone(int64_t status, bool pending);
void fgs_voice_ready(int64_t stage);
bool fm_setup_request(bool visible, bool active, bool shown, bool completed, int64_t status, bool pending, bool interactive);
bool editor_high_refresh(int64_t display_hz);
bool editor_voice_warm(bool scene, bool visible, bool authorized, bool installed, bool interactive, bool pending, bool ready);
void fgs_choose(int64_t kind);
int64_t fgs_count(int64_t kind);
int64_t fgs_selected(void);
int64_t fgs_status(int64_t kind);
int64_t fgs_text_byte(int64_t field, int64_t index);
int64_t fgs_settings_byte(int64_t index);
void fgc_reset(void);
bool fm_permission(bool trusted, bool audio, bool authorized);
bool fm_capture(bool ready, bool visible, bool active);
int64_t fgv_arm(void);
void fgv_cancel(void);
int64_t fgv_message(const uint8_t *p, int64_t n, bool available, bool busy);
int64_t fgv_copy(uint8_t *p, int64_t cap);
// The voice page's two functions that post aethyrforge.voice-coder.v1 (voice-coder.coax fv_page_script).
int64_t fgv_page_script(uint8_t *out, int64_t cap);
// The scene session file's bytes, aethyrforge.scene-session.v1 (scene-session.coax fs_session_put); -1 when it doesn't fit.
int64_t fgc_scene_session(int64_t port, int64_t x_cm, int64_t z_cm, int64_t pid, int64_t observed_ms, uint8_t *out, int64_t cap);
int64_t fgc_read(const uint8_t *p, int64_t n);
int64_t fgc_copy(int64_t field, uint8_t *p, int64_t cap);
bool fgc_send(bool busy, const uint8_t *p, int64_t n);
int64_t fgc_router_default(uint8_t *out, int64_t cap);
int64_t fc_chat_hold_ms(void);
int64_t fgc_chat_mode(bool opened, bool busy, int64_t now_ms);
void fgc_feed_event(int64_t kind, const uint8_t *p, int64_t n, int64_t now_ms);
int64_t fgc_feed_copy(uint8_t *p,int64_t cap);
// A chat turn on the shared turn runner, in this process (chat-run.coax): start
// it, poll it each frame, read its session lines (fgc_read reads each), stop it.
bool fgc_run_start(const uint8_t *text,int64_t tn,const uint8_t *parent,int64_t pn,const uint8_t *cwd,int64_t cn,const uint8_t *context,int64_t xn,const uint8_t *engine,int64_t en);
bool fgc_run_poll(void);
int64_t fgc_run_line(uint8_t *out,int64_t cap);
void fgc_run_stop(void);
int64_t fgc_scene_instructions(uint8_t *out,int64_t cap);
int64_t fgc_feed_draw(float *out,int64_t cap,float w,float h,float d);
int64_t fgc_enter_route(bool ready, bool opened, int64_t key, bool down, bool repeated, bool modified, bool held);
bool fgc_answer(bool busy, bool pending, const uint8_t *p, int64_t n);
int64_t fgc_catalog(int64_t i, bool label, uint8_t *out, int64_t cap);
void fgc_models_reset(void);
bool fgc_models(const uint8_t *p, int64_t n);
int64_t fgc_model(int64_t i, int64_t effort, uint8_t *out, int64_t cap);
float fgc_box(int64_t kind, float w, float h, int64_t component);
int64_t fgc_draw(float *out, int64_t cap, float w, float h, float d, bool busy);
