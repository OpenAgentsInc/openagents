// Thin ABI transport. Coax owns setup parsing, labels, choices and readiness.
#include "session.h"
#include "face.h"
static coax_t_ForgeSetup setup;
static coax_t_ForgeChrome chrome;
static uint8_t input[65536];
static coax_v_t_ForgeSetup view(void) { return (coax_v_t_ForgeSetup){&setup, 1}; }
void fgs_reset(void) { fsu_reset(view()); }
int64_t fgs_input_byte(int64_t i, int64_t v) {
  if (i < 0 || i >= (int64_t)sizeof input || v < 0 || v > 255) return 0;
  input[i] = (uint8_t)v; return 1;
}
int64_t fgs_accept(int64_t kind, int64_t n) {
  if (n < 0 || n > (int64_t)sizeof input) return 0;
  return fsu_read(view(), kind, (coax_string){(const char *)input, n}) ? 1 : 0;
}
bool fgs_can_enter(bool scene) { return fsu_can_enter(view(), scene); }
bool fgs_completed(void) { return fsu_completed(view()); }
void fgs_microphone(int64_t status, bool pending) { fsu_microphone(view(), status, pending); }
void fgs_voice_ready(int64_t stage) { fsu_voice_ready(view(), stage); }
void fgs_choose(int64_t kind) { fsu_choose(view(), kind); }
int64_t fgs_count(int64_t kind) { return fsu_count(view(), kind); }
int64_t fgs_selected(void) { return fsu_selected(view()); }
int64_t fgs_status(int64_t kind) { return fsu_status(view(), kind); }
int64_t fgs_text_byte(int64_t field, int64_t index) { return fsu_text_byte(view(), field, index); }
int64_t fgs_settings_byte(int64_t index) { return fsu_settings_byte(view(), index); }

static coax_v_t_ForgeChrome ui(void) {return (coax_v_t_ForgeChrome){&chrome,1};}
void *fgu_state(void) {return &chrome;}
void fgu_update(bool scene,bool checking,bool verifying,bool status,bool paused,const uint8_t *error,int64_t n) {
  fui_update(ui(),scene,checking,verifying,status,paused,(coax_string){(const char *)error,n});
}
int64_t fgu_draw(float *out,int64_t cap,float w,float h,float d) {return fui_draw(ui(),view(),(coax_v_f32){out,cap},w,h,d);}
int64_t fgu_hit(float w,float h,float x,float y) {return fui_hit(ui(),view(),w,h,x,y);}
float fgu_box(int64_t kind,float w,float h,int64_t field) {return fui_box_field(kind,w,h,field);}
int32_t fgu_glyph_count(void) {return 98;}
uint32_t fgu_glyph_code(int32_t g) {return fui_glyph_code(g);}
int32_t fgu_font_count(void) {return 4;}
float fgu_font_size(int32_t slot) {return fui_font_size(slot);}
int32_t fgu_font_weight(int32_t slot) {return (int32_t)fui_font_weight(slot);}
int32_t fgu_icon_count(void) {return 0;}
float fgu_icon_size(int32_t i) {(void)i;return 0;}
int32_t fgu_icon_name(int32_t i,uint8_t *out,int32_t cap) {(void)i;(void)out;(void)cap;return 0;}
void fgu_set_icon(void *s,int32_t i,float x,float y,float w,float h) {(void)s;(void)i;(void)x;(void)y;(void)w;(void)h;}
void fgu_set_font(void *s,int32_t slot,const float *h,const float *a,const float *k) {(void)s;fui_set_font(ui(),slot,(coax_v_f32){(float *)h,7},(coax_v_f32){(float *)a,98},(coax_v_f32){(float *)k,9604});}
void fgu_set_atlas(void *s,float w,float h) {(void)s;fui_set_atlas(ui(),w,h);}
void fgu_set_dynamic(void *s,float size,float px,float py,float y,int32_t cols,int32_t cc,float cw,float ch) {(void)s;fui_set_dynamic(ui(),size,px,py,y,cols,cc,cw,ch);}
int32_t fgu_next_cluster(void *s) {(void)s;return (int32_t)fui_next_cluster(ui());}
int32_t fgu_cluster(void *s,int32_t slot,uint8_t *b,int32_t cap) {(void)s;return (int32_t)fui_cluster(ui(),slot,(coax_v_u8){b,cap});}
void fgu_measured(void *s,int32_t slot,float width,int32_t color) {(void)s;fui_measured(ui(),slot,width,color!=0);}
int32_t fgu_next_cell(void *s) {(void)s;return (int32_t)fui_next_cell(ui());}
int32_t fgu_cell(void *s,int32_t cell,float *out) {(void)s;return fui_cell(ui(),cell,(coax_v_f32){out,6});}
void fgu_cell_ready(void *s,int32_t cell) {(void)s;fui_cell_ready(ui(),cell);}
#ifdef __APPLE__
#include <libproc.h>
#include <mach/mach_time.h>
uint64_t forge_process_cpu_ns(int32_t pid) {
  struct rusage_info_v2 r={0};mach_timebase_info_data_t t;
  if(proc_pid_rusage(pid,RUSAGE_INFO_V2,(rusage_info_t *)&r)!=0 || mach_timebase_info(&t)!=0)return 0;
  return (uint64_t)(((__uint128_t)r.ri_user_time+r.ri_system_time)*t.numer/t.denom);
}
uint64_t forge_process_resident_bytes(int32_t pid) {
  struct rusage_info_v2 r={0};return proc_pid_rusage(pid,RUSAGE_INFO_V2,(rusage_info_t *)&r)==0?r.ri_resident_size:0;
}
#endif
