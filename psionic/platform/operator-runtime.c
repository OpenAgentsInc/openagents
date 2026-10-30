/* Bind trusted Coax CPU callbacks and driver kernels to admitted schemas. */
#include "operator-runtime.h"
#include "cuda-host.h"
#ifdef __APPLE__
#include "metal-host.h"
#endif
#include <stdlib.h>
#include <string.h>
#define OR(name) COAX_TiStatus_##name
struct PorEntry {PorCPU forward,reference;PcModule module;PcKernel kernel;
#ifdef __APPLE__
    PmModule metal;PmKernel mk;
#endif
};
struct Por {coax_t_OrRegistry state;coax_t_TdDevice device;struct PorEntry entries[8];};
coax_t_TiStatus por_open(coax_t_TdDevice device,coax_t_TiTarget target,Por **out){if(!out || *out)return OR(state);coax_t_TiStatus z=or_open_admit(target.compiler,device);if(z)return z;Por *p=calloc(1,sizeof *p);if(!p)return OR(capacity);p->state=or_empty(target.compiler);p->device=device;*out=p;return OR(ok);}
static void por_unload(Por *p,struct PorEntry *e){if(p->device==COAX_TdDevice_cuda && e->module)pc_unload(e->module);
#ifdef __APPLE__
    if(p->device==COAX_TdDevice_metal){if(e->mk)pm_kernel_free(e->mk);if(e->metal)pm_unload(e->metal);}
#endif
}
coax_t_TiStatus por_register(Por *p,coax_t_OrSchema schema,PorCPU forward,PorCPU reference,const char *source,size_t bytes,int64_t *id){
    if(!p || !id || bytes>INT64_MAX)return OR(state);
    coax_t_TiStatus z=or_source((int64_t)bytes,source!=NULL,forward&&reference);if(z)return z;coax_a4_u64 hash=ti_identity_words((coax_v_u8){(uint8_t*)source,(int64_t)bytes});coax_t_OrUpdate change=or_register(p->state,schema,hash);if(change.status)return change.status;
    struct PorEntry entry={.forward=forward,.reference=reference};char name[49];memcpy(name,schema.entry.e,(size_t)schema.entry_count);name[schema.entry_count]=0;
    if(p->device==COAX_TdDevice_cuda){PcDeviceInfo info;double seconds;if(pc_open(&info)!=PC_OK || pc_compile(source,bytes,"registered-operator.cu",&entry.module,&seconds)!=PC_OK || pc_kernel(entry.module,name,&entry.kernel)!=PC_OK){por_unload(p,&entry);return OR(driver);}}
#ifdef __APPLE__
    if(p->device==COAX_TdDevice_metal){PmDeviceInfo info;double seconds;if(pm_open(&info)!=PM_OK || pm_compile(source,bytes,"registered-operator.metal",&entry.metal,&seconds)!=PM_OK || pm_kernel(entry.metal,name,&entry.mk)!=PM_OK){por_unload(p,&entry);return OR(driver);}}
#else
    if(p->device==COAX_TdDevice_metal)return OR(absent);
#endif
    p->entries[change.id]=entry;p->state=change.registry;*id=change.id;return OR(ok);
}
coax_t_OrRegistry por_info(const Por *p){return p?p->state:(coax_t_OrRegistry){0};}
static coax_t_TiStatus por_admit(void *ctx,coax_t_TiPlan plan){Por *p=ctx;coax_t_TiStatus z=or_device(p->device,plan.target.device);return z?z:or_admit(p->state,plan);}
static coax_t_TiStatus por_acquire(void *ctx){Por *p=ctx;coax_t_TiLifeResult r=ti_module_borrow(p->state.life);if(!r.status)p->state.life=r.state;return r.status;}
static void por_release(void *ctx){Por *p=ctx;coax_t_TiLifeResult r=ti_module_return(p->state.life);if(!r.status)p->state.life=r.state;}
static void por_cpu(void *ctx,const coax_t_TiLaunch *launch,bool reference,coax_v_f32 input,coax_v_f32 work,coax_v_f32 out,coax_v_u8 flags){Por *p=ctx;struct PorEntry *e=&p->entries[launch->meta.e[31]];PorCPU run=reference?e->reference:e->forward;for(int64_t i=0;i<launch->items;i++)run(i,(coax_v_s64){(int64_t*)launch->meta.e,64},launch->epsilon,input,work,out,flags);}
static void *por_kernel(void *ctx,int64_t id){Por *p=ctx;if(p->device==COAX_TdDevice_cuda)return p->entries[id].kernel;
#ifdef __APPLE__
    if(p->device==COAX_TdDevice_metal)return p->entries[id].mk;
#endif
    return NULL;
}
coax_t_TiStatus por_compile(Por *p,coax_t_TiProgram graph,coax_t_TiTarget target,bool reference,const char *module,size_t bytes,Pti **out){if(!p || !out || *out)return OR(state);coax_t_TiPlan plan=ti_compile(graph,target,!reference);coax_t_TiStatus z=por_admit(p,plan);if(z)return z;Pti *compiled=NULL;z=pti_open(graph,target,reference,module,bytes,&compiled);if(z)return z;PtiExtensions hooks={.context=p,.admit=por_admit,.acquire=por_acquire,.release=por_release,.cpu=por_cpu,.kernel=por_kernel};coax_a4_u64 digest=or_fingerprint(p->state);memcpy(hooks.digest,digest.e,sizeof hooks.digest);z=pti_attach(compiled,&hooks);if(z){pti_close(&compiled);return z;}*out=compiled;return OR(ok);}
coax_t_TiStatus por_close(Por **out){if(!out || !*out)return OR(state);Por *p=*out;coax_t_TiStatus z=ti_close_admit(p->state.life);if(z)return z;for(int64_t i=0;i<p->state.count;i++)por_unload(p,&p->entries[i]);free(p);*out=NULL;return OR(ok);}
