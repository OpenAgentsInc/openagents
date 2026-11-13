/* Firm reason: 6 (native Wayland/Vulkan ABI; all application behavior is Coax). */
#include "linux-native.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
static coax_t_EditorNative *at(uint32_t p){return (coax_t_EditorNative *)(uintptr_t)p;}
static coax_v_t_EditorNative view(uint32_t p){return (coax_v_t_EditorNative){at(p),1};}
uint32_t coaxial_layout(void){return sizeof(coax_t_EditorNative)*1000003u+0x46474e31u;}
uint32_t coaxial_state_size(void){return sizeof(coax_t_EditorNative);}
void coaxial_state_init(uint32_t p){coax_default_t_EditorNative(at(p));fgn_boot(view(p));}
uint32_t coaxial_max_vertices(void){return 144000;}
uint32_t coaxial_vertex_floats(void){return 12;}
uint32_t coaxial_uniform_floats(void){return 88;}
uint32_t coaxial_frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){(void)still;return fgn_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,144000*12},(coax_v_f32){(float *)(uintptr_t)u,88},w,h,d,dt);}
uint32_t coaxial_key(uint32_t p,int32_t k,int32_t mods){return fgn_key(view(p),k,true,mods);}
uint32_t coaxial_key_up(uint32_t p,int32_t k,int32_t mods){return fgn_key(view(p),k,false,mods);}
void coaxial_blur(uint32_t p){fgn_blur(view(p));}
uint32_t coaxial_pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return fgn_pointer(view(p),x,y,phase,kind,id);}
void coaxial_hover(uint32_t p,float x,float y){fgn_hover(view(p),x,y);}
uint32_t coaxial_inbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->inbox.e;}
uint32_t coaxial_inbox_cap(void){return 2097152;}
void coaxial_message(uint32_t p,int32_t kind,uint32_t n){fgn_message(view(p),kind,n);}
uint32_t coaxial_draws(uint32_t p){return (uint32_t)(uintptr_t)at(p)->lab.e[0].draws.e;}
uint32_t coaxial_draw_count(uint32_t p){return at(p)->lab.e[0].mesh_n>0?1:0;}
uint32_t coaxial_stores(void){return 1;}
uint32_t coaxial_store_cap(int32_t k){(void)k;return 32768*4;}
uint32_t coaxial_store(uint32_t p,int32_t k){(void)k;return (uint32_t)(uintptr_t)at(p)->lab.e[0].mesh.e;}
uint32_t coaxial_store_bytes(uint32_t p,int32_t k){(void)k;return at(p)->lab.e[0].mesh_n*8*4;}
uint32_t coaxial_store_version(uint32_t p,int32_t k){(void)k;return at(p)->frame;}
int32_t coaxial_action(uint32_t p){fgn_selection_save(view(p));return at(p)->lab.e[0].action;}
uint32_t coaxial_outbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->lab.e[0].outbox.e;}
uint32_t coaxial_outbox_len(uint32_t p){return at(p)->lab.e[0].out_n;}
void coaxial_action_done(uint32_t p){at(p)->lab.e[0].action=0;at(p)->lab.e[0].out_n=0;}
const char *coaxial_window_title(void){return "Aethyrforge · Bitcoin Lab";}
const char *coaxial_window_app_id(void){return "com.openagents.aethyrforge";}
int coaxial_should_close(uint32_t p){return at(p)->quit;}
void coaxial_close(uint32_t p){fgn_close(view(p));}
