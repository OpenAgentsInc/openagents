/* Platform ABI only: transport effects, canvas input and observation. */
#include "explorer.h"
#include <stdint.h>
#define EXPORT(n) __attribute__((export_name(n)))
#define IMPORT(n) __attribute__((import_module("env"),import_name(n)))
IMPORT("coaxial_http_bounded") int32_t http(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,int64_t);
IMPORT("coaxial_http_cancel") int32_t cancel(int32_t);
int64_t cxexplorer_http(coax_v_u8 url,int64_t timeout){return http((uint32_t)(uintptr_t)url.data,(uint32_t)url.count,0,0,4,timeout);}
int64_t cxexplorer_cancel(int64_t id){return cancel((int32_t)id);}
static coax_t_ExplorerUi *s(uint32_t p){return (coax_t_ExplorerUi *)(uintptr_t)p;}
static coax_v_t_ExplorerUi at(uint32_t p){return (coax_v_t_ExplorerUi){s(p),1};}
static unsigned char accessible[32768];
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_ExplorerUi)*1000003u+0x45585031u;}
EXPORT("coaxial_state_size") uint32_t size(void){return sizeof(coax_t_ExplorerUi);}
EXPORT("coaxial_state_init") void init(uint32_t p){coax_default_t_ExplorerUi(s(p));}
EXPORT("coaxial_max_vertices") uint32_t vertices(void){return 50400;}
EXPORT("coaxial_vertex_floats") uint32_t floats(void){return 16;}
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void){return 16;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){(void)still;return (uint32_t)eu_frame(at(p),(coax_v_f32){(float *)(uintptr_t)v,50400*16},(coax_v_f32){(float *)(uintptr_t)u,16},w,h,d,dt);}
EXPORT("coaxial_inbox") uint32_t inbox(uint32_t p){return (uint32_t)(uintptr_t)s(p)->inbox.e;}
EXPORT("coaxial_inbox_cap") uint32_t cap(uint32_t p){(void)p;return 16777216;}
EXPORT("coaxial_message") void message(uint32_t p,int32_t k,int32_t n){eu_receive(at(p),k,n);}
EXPORT("coaxial_hash") void hash(uint32_t p,int32_t n){eu_hash(at(p),n);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){(void)kind;(void)id;return eu_pointer(at(p),x,y,phase);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){eu_wheel(at(p),dy);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){return eu_key(at(p),k,mods);}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p){return s(p)->focus?3:1;}
EXPORT("coaxial_wants_text") uint32_t wants(uint32_t p){return s(p)->focus;}
EXPORT("coaxial_field") uint32_t field(uint32_t p){return (uint32_t)(uintptr_t)s(p)->field_box.e;}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)s(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)s(p)->route.e;}
EXPORT("coaxial_outbox_len") uint32_t outlen(uint32_t p){return (uint32_t)s(p)->route_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){s(p)->action=0;}
EXPORT("coaxial_a11y") uint32_t a11y(uint32_t p){return (uint32_t)eu_a11y(at(p),(coax_v_u8){accessible,sizeof accessible});}
EXPORT("coaxial_a11y_buf") uint32_t a11ybuf(void){return (uint32_t)(uintptr_t)accessible;}
EXPORT("coaxial_a11y_do") void a11ydo(uint32_t p,int32_t id){eu_do(at(p),id);}
EXPORT("coaxial_a11y_focus") void a11yfocus(uint32_t p,int32_t id){s(p)->focused_hit=id;}
/* Read-only end-to-end probes use the exact displayed state. */
EXPORT("coaxial_explorer_ready") uint32_t ready(uint32_t p){return s(p)->model.e[0].ready;}
EXPORT("coaxial_explorer_height") int32_t height(uint32_t p){return (int32_t)s(p)->model.e[0].snapshot.height;}
EXPORT("coaxial_explorer_view") int32_t view(uint32_t p){return (int32_t)s(p)->view;}
EXPORT("coaxial_explorer_error") int32_t error(uint32_t p){return (int32_t)s(p)->model.e[0].error;}
EXPORT("coaxial_explorer_balance") int64_t balance(uint32_t p){return s(p)->balance;}
EXPORT("coaxial_field_probe") double probe(uint32_t p,int32_t field){switch(field){case 0:return ready(p);case 1:return height(p);case 2:return view(p);case 3:return error(p);case 4:return (double)balance(p);case 5:return (double)s(p)->focused_hit;case 6:return (double)eu_output_count(at(p),0);case 7:return (double)eu_output_count(at(p),1);default:return -1;}}
