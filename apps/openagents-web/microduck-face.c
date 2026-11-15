/* Firm reason: 1 (scalar browser ABI only; camera, input and drawing are Coax). */
#include "microduck.h"
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include "face-address.h"
#define EXPORT(name) __attribute__((export_name(name)))
static uint8_t catalog[65536], inbox[4096], menu[32768], lab[312], run[1472];
static uint32_t run_len;
static uint32_t menu_len;
static coax_t_Microduck *state(cx_face_addr p) { return (coax_t_Microduck *)(uintptr_t)p; }
static coax_v_t_Microduck view(cx_face_addr p) { return (coax_v_t_Microduck){state(p),1}; }
EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return sizeof(coax_t_Microduck)*1000003u+0x4d443031u; }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return sizeof(coax_t_Microduck); }
EXPORT("coaxial_state_init") void coaxial_state_init(cx_face_addr p) { coax_default_t_Microduck(state(p));
#if defined(COAXIAL_NATIVE_ADDRESS)
state(p)->native=true;
#endif
 }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return 32768; }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return 12; }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return 1600; }
EXPORT("coaxial_frame") uint32_t coaxial_frame(cx_face_addr p,cx_face_addr v,cx_face_addr u,float w,float h,float d,float dt,uint32_t still) {
  (void)still; md_advance(view(p),dt);
  return md_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,32768*12},(coax_v_f32){(float *)(uintptr_t)u,1600},w,h,d);
}
__attribute__((import_module("env"),import_name("coaxial_storage_get"))) int32_t coaxial_storage_get(int32_t,int32_t,cx_face_addr,int32_t,int32_t,cx_face_addr,int32_t);
__attribute__((import_module("env"),import_name("coaxial_storage_put"))) int32_t coaxial_storage_put(int32_t,int32_t,cx_face_addr,int32_t,int32_t,cx_face_addr,int32_t,cx_face_addr,int32_t);
static void storage(cx_face_addr p) {
  coax_t_MdRunStorage r=md_run_storage(view(p));
  if (!r.operation) return;
  int32_t result=r.operation==1 ? coaxial_storage_put(0,(int32_t)r.token,(cx_face_addr)(uintptr_t)r.space.data,(int32_t)r.space.count,(int32_t)r.version,(cx_face_addr)(uintptr_t)r.key.data,(int32_t)r.key.count,(cx_face_addr)(uintptr_t)r.data.data,(int32_t)r.data.count) : coaxial_storage_get(0,(int32_t)r.token,(cx_face_addr)(uintptr_t)r.space.data,(int32_t)r.space.count,(int32_t)r.version,(cx_face_addr)(uintptr_t)r.key.data,(int32_t)r.key.count);
  md_run_storage_refused(view(p),result);
}
EXPORT("coaxial_key") uint32_t coaxial_key(cx_face_addr p,int32_t k,int32_t mods) { (void)mods;uint32_t done=md_key(view(p),k);storage(p);return done; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(cx_face_addr p,float x,float y,int32_t phase,int32_t kind,int32_t id) { return md_pointer(view(p),x,y,phase,kind,id); }
EXPORT("coaxial_wheel") void coaxial_wheel(cx_face_addr p,float delta) { md_wheel(view(p),delta); }
EXPORT("coaxial_blur") void coaxial_blur(cx_face_addr p) { state(p)->drag=false; }
EXPORT("coaxial_scene_ready") uint32_t coaxial_scene_ready(cx_face_addr p) { return state(p)->ready; }
EXPORT("coaxial_asset") cx_face_addr coaxial_asset(cx_face_addr p,int32_t i,uint32_t n) { (void)p; return i==0 && n<=sizeof catalog ? (cx_face_addr)(uintptr_t)catalog:0; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(cx_face_addr p,int32_t i,uint32_t n) { if(i==0 && n<=sizeof catalog) md_asset(view(p),(coax_string){(const char *)catalog,n}); }
EXPORT("coaxial_draws") cx_face_addr coaxial_draws(cx_face_addr p) { return (cx_face_addr)(uintptr_t)state(p)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(cx_face_addr p) { return state(p)->ready?(uint32_t)state(p)->assembly.e[0].parts:0u; }
EXPORT("coaxial_pack_gate_version") uint32_t coaxial_pack_gate_version(void) { return 1; }
EXPORT("coaxial_pack_digest") cx_face_addr coaxial_pack_digest(cx_face_addr p) { return (cx_face_addr)(uintptr_t)state(p)->digest.e; }
EXPORT("coaxial_pack_digest_capacity") uint32_t coaxial_pack_digest_capacity(void) { return 32; }
EXPORT("coaxial_pack_admit") uint32_t coaxial_pack_admit(cx_face_addr p,uint32_t bytes) { return md_gate(view(p),bytes); }
EXPORT("coaxial_guard_probe") int32_t coaxial_guard_probe(cx_face_addr p,int32_t field) { return field==0?state(p)->ready:field==1?state(p)->mode:field==2?state(p)->draws.e[1]:0; }

EXPORT("coaxial_pinch") uint32_t coaxial_pinch(cx_face_addr p,float ratio) { return md_pinch(view(p),ratio); }

EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(cx_face_addr p,int32_t i) { (void)p; return i==0 ? sizeof catalog:0; }
EXPORT("coaxial_view_menu_len_v1") uint32_t coaxial_view_menu_len_v1(cx_face_addr p) { int64_t n=md_menu(view(p),(coax_v_u8){menu,sizeof menu}); menu_len=n<0?0:(uint32_t)n;return menu_len; }
EXPORT("coaxial_view_menu_v1") cx_face_addr coaxial_view_menu_v1(cx_face_addr p) { (void)p;return menu_len?(cx_face_addr)(uintptr_t)menu:0; }

EXPORT("coaxial_inbox") cx_face_addr coaxial_inbox(cx_face_addr p) { (void)p; return (cx_face_addr)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(cx_face_addr p,int32_t kind,uint32_t n) { if(n<=sizeof inbox) md_run_inbox(view(p),kind,(coax_v_u8){inbox,n}); memset(inbox,0,sizeof inbox); }
EXPORT("coaxial_document_reply_v1") void coaxial_document_reply_v1(cx_face_addr p,int32_t token,int32_t status,uint32_t n) { if(n<=sizeof inbox) md_run_document_reply(view(p),token,status,(coax_v_u8){inbox,n});memset(inbox,0,sizeof inbox); }
EXPORT("coaxial_action") int32_t coaxial_action(cx_face_addr p) {return (int32_t)state(p)->files.e[0].action;}
EXPORT("coaxial_outbox") cx_face_addr coaxial_outbox(cx_face_addr p) {return (cx_face_addr)(uintptr_t)state(p)->files.e[0].out.e;}
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(cx_face_addr p) {return (uint32_t)state(p)->files.e[0].out_n;}
EXPORT("coaxial_outbox_capacity") uint32_t coaxial_outbox_capacity(cx_face_addr p) {(void)p;return 4096;}
EXPORT("coaxial_action_done") void coaxial_action_done(cx_face_addr p) {state(p)->files.e[0].action=0;}
EXPORT("coaxial_servo_run_len_v1") uint32_t coaxial_servo_run_len_v1(cx_face_addr p) {int64_t n=md_run_packet(view(p),(coax_v_u8){run,sizeof run});run_len=n<0?0:(uint32_t)n;return run_len;}
EXPORT("coaxial_servo_run_v1") cx_face_addr coaxial_servo_run_v1(cx_face_addr p) {(void)p;return (cx_face_addr)(uintptr_t)run;}

EXPORT("coaxial_servo_lab_len_v1") uint32_t coaxial_servo_lab_len_v1(cx_face_addr p) { return md_lab_packet(view(p),(coax_v_u8){lab,sizeof lab}) ? sizeof lab : 0; }
EXPORT("coaxial_servo_lab_v1") cx_face_addr coaxial_servo_lab_v1(cx_face_addr p) { (void)p;return (cx_face_addr)(uintptr_t)lab; }
