/* Firm reason: 1 (scalar browser ABI; the residency is Coax, apps/openagents-web/cullstream.coax). */
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
EXPORT("cs_open") uint32_t open_(uint32_t p,int32_t pages,int32_t slots,int32_t nlinks){return cs_open(view(p),pages,slots,nlinks)?1u:0u;}
EXPORT("cs_frame") int32_t frame(uint32_t p){return (int32_t)cs_frame(view(p));}
EXPORT("cs_installs") int32_t installs(uint32_t p){return (int32_t)at(p)->installs;}
EXPORT("cs_evictions") int32_t evictions(uint32_t p){return (int32_t)at(p)->evictions;}
EXPORT("cs_valid") uint32_t valid(uint32_t p){return cs_valid(view(p))?1u:0u;}
