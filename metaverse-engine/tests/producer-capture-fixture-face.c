/* WebAssembly ABI effects only. Fixture state, scheduling, bytes, counters,
   fault selection and arithmetic are authored in producer-capture-fixture.coax. */
#include "ProducerFixture.h"
#include <stdint.h>
#include <stddef.h>
#define EX(name) __attribute__((export_name(name)))
static coax_t_PfxTrace trace;
static uint32_t current;
static coax_v_t_PfxTrace tv(void){return (coax_v_t_PfxTrace){&trace,1};}
static coax_v_t_PfxState sv(uint32_t p){return (coax_v_t_PfxState){(coax_t_PfxState*)(uintptr_t)p,1};}
static uint32_t ptr(const void *p){return (uint32_t)(uintptr_t)p;}
__attribute__((import_module("env"),import_name("coaxial_now_ms"))) int64_t sdk_ms(void);
__attribute__((import_module("env"),import_name("coaxial_now_seconds"))) int64_t sdk_seconds(void);
int64_t pfx_clock_ms(void){return sdk_ms();}
int64_t pfx_clock_seconds(void){return sdk_seconds();}
int64_t pfx_memory_pages(void){return (int64_t)__builtin_wasm_memory_size(0);}
__attribute__((constructor)) static void initialize_fixture(void){
  (void)__builtin_wasm_memory_grow(0,(size_t)pfx_initialize(tv()));
}
EX("coaxial_vertex_floats") int32_t vertex_floats(void){return (int32_t)pfx_scalar(tv(),1,sizeof(coax_t_PfxState));}
EX("coaxial_max_vertices") int32_t max_vertices(void){return (int32_t)pfx_scalar(tv(),2,sizeof(coax_t_PfxState));}
EX("coaxial_uniform_floats") int32_t uniform_floats(void){return (int32_t)pfx_scalar(tv(),3,sizeof(coax_t_PfxState));}
EX("coaxial_state_size") int32_t state_size(void){return (int32_t)pfx_scalar(tv(),4,sizeof(coax_t_PfxState));}
EX("coaxial_state_init") void state_init(uint32_t s){current=s;pfx_init(sv(s),tv());}
EX("coaxial_golden") void golden(int32_t n){pfx_golden(sv(current),n);}
EX("coaxial_page") void page(uint32_t s,int32_t n){pfx_page(sv(s),n);}
EX("coaxial_inbox_cap") int32_t inbox_cap(void){return sizeof(((coax_t_PfxState*)0)->inbox);}
EX("coaxial_inbox") uint32_t inbox(uint32_t s){return ptr(&sv(s).data->inbox.e[0].e[0]);}
EX("coaxial_message") void message(uint32_t s,int32_t kind,int32_t n){pfx_message(sv(s),kind,n);}
EX("coaxial_asset") uint32_t asset(uint32_t s,int32_t i,int32_t n){(void)i;return (uint32_t)pfx_asset(sv(s),n,ptr(sv(s).data->catalog.e));}
EX("coaxial_asset_ready") void asset_ready(uint32_t s,int32_t i,int32_t n){pfx_ready(sv(s),i,n);}
#ifndef PFX_MINIMAL
EX("coaxial_hash") void hash(uint32_t s,int32_t n){pfx_hash(sv(s),n);}
EX("coaxial_action") int32_t action(uint32_t s){return (int32_t)pfx_action(sv(s));}
EX("coaxial_action_done") void action_done(uint32_t s){pfx_done(sv(s));}
#endif
#ifndef PFX_NO_KEY
EX("coaxial_key") void key(uint32_t s,int32_t k,int32_t flags){(void)flags;pfx_key(sv(s),k,1);}
EX("coaxial_key_up") void key_up(uint32_t s,int32_t k,int32_t flags){(void)flags;pfx_key(sv(s),k,0);}
#endif
EX("coaxial_pointer") void pointer(uint32_t s,double x,double y,int32_t phase,int32_t button,int32_t buttons){(void)button;(void)buttons;pfx_pointer(sv(s),x,y,phase);}
EX("coaxial_frame") int32_t frame(uint32_t s,uint32_t v,uint32_t u,int32_t w,int32_t h,float scale,float dt,float time){
  (void)dt;(void)time;return (int32_t)pfx_frame(sv(s),(coax_v_f32){(float*)(uintptr_t)v,pfx_vertex_words()},(coax_v_f32){(float*)(uintptr_t)u,pfx_uniform_words()},w,h,scale);
}
EX("coaxial_draw_count") int32_t draw_count(uint32_t s){(void)s;return (int32_t)pfx_count(0);}
#ifndef PFX_MINIMAL
EX("coaxial_water_draws") int32_t water(uint32_t s){(void)s;return (int32_t)pfx_count(1);}
EX("coaxial_grass_instances") int32_t grass(uint32_t s){(void)s;return (int32_t)pfx_count(2);}
#endif
EX("coaxial_particle_instances") int32_t particles(uint32_t s){(void)s;return (int32_t)pfx_count(3);}
#ifndef PFX_MINIMAL
EX("coaxial_surface_vertices") int32_t surface(uint32_t s){(void)s;return (int32_t)pfx_count(4);}
EX("coaxial_caster_vertices") int32_t caster_vertices(uint32_t s){(void)s;return (int32_t)pfx_count(5);}
EX("coaxial_caster_instances") int32_t caster_instances(uint32_t s){(void)s;return (int32_t)pfx_count(6);}
#endif
EX("coaxial_draws") uint32_t draws(uint32_t s){return (uint32_t)pfx_draw_pointer(sv(s),ptr(sv(s).data->draws.e));}
#ifndef PFX_MINIMAL
EX("coaxial_stores") int32_t stores(void){return (int32_t)pfx_store_count();}
EX("coaxial_store_cap") int32_t store_cap(int32_t i){return (int32_t)pfx_capacity(i);}
#ifndef PFX_NO_VERSION
EX("coaxial_store_version") int64_t store_version(uint32_t s,int32_t i){return pfx_version(sv(s),i);}
#endif
EX("coaxial_store_bytes") int32_t store_bytes(uint32_t s,int32_t i){return (int32_t)pfx_store_bytes(sv(s),i);}
EX("coaxial_store") uint32_t store(uint32_t s,int32_t i){return (uint32_t)pfx_store_pointer(sv(s),i,ptr(sv(s).data->stores.e[0].e),ptr(sv(s).data->stores.e[1].e));}
EX("coaxial_particle_cap") int32_t particle_cap(void){return sizeof(((coax_t_PfxState*)0)->puffs.e);}
EX("coaxial_particle_bytes") int32_t particle_bytes(uint32_t s){return (int32_t)pfx_particle_bytes(sv(s));}
EX("coaxial_particles") uint32_t particle_pointer(uint32_t s){return ptr(sv(s).data->puffs.e);}
#endif
