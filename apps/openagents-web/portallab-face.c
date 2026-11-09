/* Firm reason: 1 (scalar browser ABI; the scene, camera and drawing are Coax in portallab.coax). */
#include "portallab.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_PortalLab *at(uint32_t p){return (coax_t_PortalLab *)(uintptr_t)p;}
static coax_v_t_PortalLab view(uint32_t p){return (coax_v_t_PortalLab){at(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_PortalLab)*1000003u+offsetof(coax_t_PortalLab,mesh)*7u+0x504c3031u;}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_PortalLab);}
EXPORT("coaxial_state_init") void state_init(uint32_t p){coax_default_t_PortalLab(at(p));}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return 6;}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return 4;}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return 64;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){return (uint32_t)pl_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,6*4},(coax_v_f32){(float *)(uintptr_t)u,64},w,h,d,dt,still!=0);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return pl_pointer(view(p),x,y,phase,kind,id,phase==0?0:-1);}
EXPORT("coaxial_pointer_buttons_v1") uint32_t pointer_buttons(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id,int32_t button){return pl_pointer(view(p),x,y,phase,kind,id,button);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){pl_wheel(view(p),dy);}
EXPORT("coaxial_blur") void blur(uint32_t p){pl_blur(view(p));}
/* Asset 0, the GLB (local:), lands in the state's buffer; asset 1 is the baked texture. */
EXPORT("coaxial_asset") uint32_t asset(uint32_t p,int32_t i,uint32_t n){return i==0&&n>0&&n<=sizeof(at(p)->glb.e)?(uint32_t)(uintptr_t)at(p)->glb.e:0;}
EXPORT("coaxial_asset_capacity") uint32_t capacity(uint32_t p,int32_t i){return i==0?(uint32_t)sizeof(at(p)->glb.e):0;}
EXPORT("coaxial_asset_ready") void ready(uint32_t p,int32_t i,uint32_t n){pl_asset(view(p),i,(int64_t)n);}
EXPORT("coaxial_scene_ready") uint32_t scene_ready(uint32_t p){return pl_ready(view(p));}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return (uint32_t)(uintptr_t)at(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p){return (uint32_t)pl_draw_count(view(p));}
EXPORT("coaxial_draw_blend_v1") uint32_t draw_blend(uint32_t p,int32_t i){(void)p;return (uint32_t)pl_draw_blend(i);}
EXPORT("coaxial_stores") uint32_t stores(void){return 1;}
EXPORT("coaxial_store_cap") uint32_t store_cap(int32_t k){(void)k;return sizeof(at(0)->mesh.e);}
EXPORT("coaxial_store") uint32_t store(uint32_t p,int32_t k){(void)k;return (uint32_t)(uintptr_t)at(p)->mesh.e;}
EXPORT("coaxial_store_bytes") uint32_t store_bytes(uint32_t p,int32_t k){(void)k;return (uint32_t)at(p)->mesh_n*8u*4u;}
EXPORT("coaxial_store_version") uint32_t store_version(uint32_t p,int32_t k){(void)k;return at(p)->revision;}
