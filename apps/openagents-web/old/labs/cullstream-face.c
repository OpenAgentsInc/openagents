/* Firm reason: 1 (scalar browser ABI; the streaming and residency are Coax, apps/openagents-web/old/labs/cullstream.coax). */
#include "cullstream.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_CullStream *at(uint32_t p){return (coax_t_CullStream *)(uintptr_t)p;}
static coax_v_t_CullStream view(uint32_t p){return (coax_v_t_CullStream){at(p),1};}
EXPORT("cs_state_size") uint32_t state_size(void){return sizeof(coax_t_CullStream);}
EXPORT("cs_init") void init(uint32_t p){__builtin_memset(at(p),0,sizeof(coax_t_CullStream));}  /* every field starts at zero */
EXPORT("cs_links") uint32_t links(uint32_t p){return (uint32_t)(uintptr_t)at(p)->links.e;}
EXPORT("cs_requests") uint32_t requests(uint32_t p){return (uint32_t)(uintptr_t)at(p)->requests.e;}
EXPORT("cs_installed") uint32_t installed(uint32_t p){return (uint32_t)(uintptr_t)at(p)->installed.e;}
EXPORT("cs_io") uint32_t io(uint32_t p){return (uint32_t)(uintptr_t)at(p)->io.e;}
EXPORT("cs_io_size") uint32_t io_size(void){return (uint32_t)sizeof(((coax_t_CullStream *)0)->io);}
EXPORT("cs_tri_out") uint32_t tri_out(uint32_t p){return (uint32_t)(uintptr_t)at(p)->tri_out.e;}
EXPORT("cs_vert_out") uint32_t vert_out(uint32_t p){return (uint32_t)(uintptr_t)at(p)->vert_out.e;}
EXPORT("cs_first_out") uint32_t first_out(uint32_t p){return (uint32_t)(uintptr_t)at(p)->first_out.e;}
EXPORT("cs_ntri") int32_t ntri(uint32_t p){return (int32_t)at(p)->ntri;}
EXPORT("cs_nvert") int32_t nvert(uint32_t p){return (int32_t)at(p)->nvert;}
EXPORT("cs_nfirst") int32_t nfirst(uint32_t p){return (int32_t)at(p)->nfirst;}
EXPORT("cs_refused") int32_t refused(uint32_t p){return (int32_t)at(p)->refused;}
EXPORT("cs_fetched") int32_t fetched(uint32_t p){return (int32_t)at(p)->fetched;}
EXPORT("cs_installs") int32_t installs(uint32_t p){return (int32_t)at(p)->installs;}
EXPORT("cs_evictions") int32_t evictions(uint32_t p){return (int32_t)at(p)->evictions;}
EXPORT("cs_open") uint32_t open_(uint32_t p,int32_t pages,int32_t slots,int32_t nlinks){return cs_open(view(p),pages,slots,nlinks)?1u:0u;}
EXPORT("cs_need") int32_t need(uint32_t p,int32_t n){return (int32_t)cs_need(view(p),n);}
EXPORT("cs_pack") uint32_t pack(uint32_t p,int32_t n){return cs_pack(view(p),n)?1u:0u;}
EXPORT("cs_page_at") int32_t page_at(uint32_t p,int32_t k){return (int32_t)cs_page_at(view(p),k);}
EXPORT("cs_page_stored") int32_t page_stored(uint32_t p,int32_t k){return (int32_t)cs_page_stored(view(p),k);}
EXPORT("cs_arrive") uint32_t arrive(uint32_t p,int32_t page,int32_t n,uint32_t track){return cs_arrive(view(p),page,n,track!=0)?1u:0u;}
EXPORT("cs_frame") int32_t frame(uint32_t p){return (int32_t)cs_frame(view(p));}
EXPORT("cs_fetches") int32_t fetches(uint32_t p){return (int32_t)cs_fetches(view(p));}
EXPORT("cs_fetch") int32_t fetch_(uint32_t p,int32_t i){return (int32_t)cs_fetch(view(p),i);}
EXPORT("cs_done_page") int32_t done_page(uint32_t p,int32_t i){return (int32_t)cs_done_page(view(p),i);}
EXPORT("cs_done_slot") int32_t done_slot(uint32_t p,int32_t i){return (int32_t)cs_done_slot(view(p),i);}
EXPORT("cs_done_evicted") int32_t done_evicted(uint32_t p,int32_t i){return (int32_t)cs_done_evicted(view(p),i);}
EXPORT("cs_place") uint32_t place(uint32_t p,int32_t i){return cs_place(view(p),i)?1u:0u;}
EXPORT("cs_place_page") uint32_t place_page(uint32_t p,int32_t page,int32_t slot){return cs_place_page(view(p),page,slot)?1u:0u;}
EXPORT("cs_valid") uint32_t valid(uint32_t p){return cs_valid(view(p))?1u:0u;}
EXPORT("cs_count") int32_t count(uint32_t p,int32_t k){return (int32_t)cs_count(view(p),k);}
