/* Allocation/driver/address effects for Coax's immutable tensor IR plans. */
#include "tensor-ir-runtime.h"
#include "cuda-host.h"
#include "spans.h"
#ifdef __APPLE__
#include "metal-host.h"
#endif
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define TI(name) COAX_TiStatus_##name
#define DEVICE(name) COAX_TdDevice_##name
struct Pti {
    coax_t_TiPlan plan;coax_t_TiBuffers sizes;coax_t_TiLife life;
    coax_t_TiLaunch *launches;int64_t *metadata;
    PcBuffer input,workspace,stage,flags,meta;
    float *readback;uint8_t *host_flags;
    PcModule module;PcKernel kernel;PcStream stream;
#ifdef __APPLE__
    PmModule metal;PmKernel mk;
#endif
    PtiStats stats;
};
static coax_v_u8 ti_bytes(const void *p,int64_t n){return(coax_v_u8){(uint8_t*)p,n};}
static coax_v_f32 ti_floats(const void *p,int64_t n){return(coax_v_f32){(float*)p,n};}
static uint8_t *ti_pointer(Pti *p,PcBuffer b){if(p->plan.target.device==DEVICE(cpu))return(uint8_t*)(uintptr_t)b;
#ifdef __APPLE__
    if(p->plan.target.device==DEVICE(metal))return pm_contents((PmBuffer)(uintptr_t)b);
#endif
    return NULL;
}
static coax_t_TiStatus ti_alloc(Pti *p,int64_t bytes,PcBuffer *out){
    coax_t_TdDevice d=p->plan.target.device;
    if(d==DEVICE(cpu)){void *b=calloc((size_t)bytes,1);if(!b)return TI(capacity);*out=(PcBuffer)(uintptr_t)b;}
    else if(d==DEVICE(cuda)){if(pc_alloc((size_t)bytes,out)!=PC_OK)return TI(driver);}
#ifdef __APPLE__
    else {PmBuffer b=NULL;if(pm_alloc((size_t)bytes,&b)!=PM_OK)return TI(driver);*out=(PcBuffer)(uintptr_t)b;}
#else
    else return TI(absent);
#endif
    p->stats.allocations++;p->stats.buffer_bytes+=bytes;if(out==&p->workspace)p->stats.workspace_bytes=bytes;return TI(ok);
}
static void ti_free(Pti *p,PcBuffer b){if(!b)return;if(p->plan.target.device==DEVICE(cpu))free((void*)(uintptr_t)b);else if(p->plan.target.device==DEVICE(cuda))pc_free(b);
#ifdef __APPLE__
    else pm_free((PmBuffer)(uintptr_t)b);
#endif
}
static void ti_dispose(Pti *p){if(!p)return;ti_free(p,p->input);ti_free(p,p->workspace);ti_free(p,p->stage);ti_free(p,p->flags);ti_free(p,p->meta);
    if(p->plan.target.device==DEVICE(cuda)){if(p->stream)pc_destroy_stream(p->stream);if(p->module)pc_unload(p->module);}
#ifdef __APPLE__
    if(p->plan.target.device==DEVICE(metal)){if(p->mk)pm_kernel_free(p->mk);if(p->metal)pm_unload(p->metal);}
#endif
    free(p->readback);free(p->host_flags);free(p->metadata);free(p->launches);free(p);
}
coax_t_TiStatus pti_open(coax_t_TiProgram graph,coax_t_TiTarget target,bool reference,const char *source,size_t bytes,Pti **out){
    if(!out || bytes>INT64_MAX)return TI(state);
    coax_t_TiStatus z=ti_open_admit(graph,target,reference,(int64_t)bytes,source!=NULL,*out==NULL);if(z)return z;
    Pti *p=calloc(1,sizeof *p);if(!p)return TI(capacity);p->plan=ti_compile(graph,target,!reference);p->life=ti_initial_life();
    char backend[256];snprintf(backend,sizeof backend,"cpu:%s:%s",sizeof(void*)==8?"64":"32",__VERSION__);
    if(target.device==DEVICE(cuda)){
        PcDeviceInfo info;if(pc_open(&info)!=PC_OK){ti_dispose(p);return TI(absent);}
        snprintf(backend,sizeof backend,"cuda:%s:cc=%d.%d:driver=%d",info.name,info.major,info.minor,info.driver_version);
        double seconds;if(pc_compile(source,bytes,"tensor-ir.cu",&p->module,&seconds)!=PC_OK || pc_kernel(p->module,"tik_run",&p->kernel)!=PC_OK || pc_stream(&p->stream)!=PC_OK){ti_dispose(p);return TI(driver);}
    }
#ifdef __APPLE__
    else if(target.device==DEVICE(metal)){
        PmDeviceInfo info;if(pm_open(&info)!=PM_OK){ti_dispose(p);return TI(absent);}snprintf(backend,sizeof backend,"metal:%s:family=%d:MSL3.1:safe-v1",info.name,info.apple_family);
        double seconds;if(pm_compile(source,bytes,"tensor-ir.metal",&p->metal,&seconds)!=PM_OK || pm_kernel(p->metal,"tik_run",&p->mk)!=PM_OK){ti_dispose(p);return TI(driver);}
    }
#else
    else if(target.device==DEVICE(metal)){ti_dispose(p);return TI(absent);}
#endif
    target.backend=ti_identity_words(ti_bytes(backend,(int64_t)strlen(backend)));target.kernel=ti_identity_words(ti_bytes(source,(int64_t)bytes));p->plan=ti_compile(graph,target,!reference);p->sizes=ti_buffers(p->plan);

    p->launches=calloc((size_t)p->plan.launches,sizeof *p->launches);p->metadata=malloc((size_t)p->sizes.metadata);p->readback=malloc((size_t)p->sizes.stage);p->host_flags=malloc((size_t)p->sizes.flags);
    if(!p->launches || !p->metadata || !p->readback || !p->host_flags){ti_dispose(p);return TI(capacity);}
    for(int64_t i=0;i<p->plan.launches;i++){p->launches[i]=ti_lower(p->plan,i);memcpy(p->metadata+64*i,p->launches[i].meta.e,64*sizeof(int64_t));}
    PcBuffer *buffers[]={&p->input,&p->workspace,&p->stage,&p->flags,&p->meta};int64_t sizes[]={p->sizes.input,p->sizes.workspace,p->sizes.stage,p->sizes.flags,p->sizes.metadata};
    for(int i=0;i<5;i++){z=ti_alloc(p,sizes[i],buffers[i]);if(z){ti_dispose(p);return z;}}
    if(target.device==DEVICE(cuda)){PcStatus copied=pc_upload(p->meta,p->metadata,(size_t)p->sizes.metadata,p->stream);PcStatus waited=pc_sync_stream(p->stream);if(copied!=PC_OK || waited!=PC_OK){ti_dispose(p);return TI(driver);}}
    else memcpy(ti_pointer(p,p->meta),p->metadata,(size_t)p->sizes.metadata);
    *out=p;return TI(ok);
}
coax_t_TiPlan pti_info(const Pti *p){return p?p->plan:(coax_t_TiPlan){.status=TI(state)};}
PtiStats pti_stats(const Pti *p){return p?p->stats:(PtiStats){0};}
bool pti_matches(const Pti *p,coax_t_TiProgram graph,coax_t_TiTarget target){return p && ti_cache_ready(p->life) && ti_cache_matches(p->plan,graph,target);}
static bool ti_overlap(Pti *p,const float *in,int64_t n,float *out,int64_t capacity){
    if(psionic_spans_overlap(in,n,sizeof(float),out,capacity,sizeof(float)) || psionic_spans_overlap(out,capacity,sizeof(float),p,1,sizeof *p))return true;
    const void *owned[]={p->launches,p->metadata,p->readback,p->host_flags};int64_t sizes[]={p->plan.launches*(int64_t)sizeof *p->launches,p->sizes.metadata,p->sizes.stage,p->sizes.flags};
    for(int i=0;i<4;i++)if(psionic_spans_overlap(out,capacity,sizeof(float),owned[i],sizes[i],1) || psionic_spans_overlap(in,n,sizeof(float),owned[i],sizes[i],1))return true;
    PcBuffer buffers[]={p->input,p->workspace,p->stage,p->flags,p->meta};int64_t lengths[]={p->sizes.input,p->sizes.workspace,p->sizes.stage,p->sizes.flags,p->sizes.metadata};
    for(int i=0;i<5;i++){uint8_t *address=ti_pointer(p,buffers[i]);if(address && (psionic_spans_overlap(out,capacity,sizeof(float),address,lengths[i],1) || psionic_spans_overlap(in,n,sizeof(float),address,lengths[i],1)))return true;}return false;
}
static coax_t_TiStatus ti_gpu_run(Pti *p,const float *input){
    int cuda=p->plan.target.device==DEVICE(cuda);coax_t_TiStatus z=TI(ok);
    if(cuda){if(p->plan.input_elements && pc_upload(p->input,input,(size_t)p->plan.input_elements*sizeof(float),p->stream)!=PC_OK){pc_sync_stream(p->stream);return TI(driver);}}
    else if(p->plan.input_elements)memcpy(ti_pointer(p,p->input),input,(size_t)p->plan.input_elements*sizeof(float));
#ifdef __APPLE__
    PmBatch batch=NULL;if(!cuda && pm_begin(&batch)!=PM_OK)return TI(driver);
#endif
    for(int64_t i=0;i<p->plan.launches;i++){
        coax_t_TiLaunch launch=p->launches[i];if(!launch.items)continue;
        if(cuda){PcArg args[]={{.kind=PC_ARG_VIEW,.data=p->meta+(uint64_t)i*64*8,.count=64},{.kind=PC_ARG_F32,.f=launch.epsilon},{.kind=PC_ARG_VIEW,.data=p->input,.count=p->plan.input_elements},{.kind=PC_ARG_VIEW,.data=p->workspace,.count=p->plan.workspace},{.kind=PC_ARG_VIEW,.data=p->stage,.count=p->plan.output_elements},{.kind=PC_ARG_VIEW,.data=p->flags,.count=p->plan.flag_elements}};
            if(pc_launch(p->kernel,launch.items,256,args,6,p->stream)!=PC_OK){z=TI(driver);break;}}
#ifdef __APPLE__
        else {PmArg args[]={{.kind=PM_ARG_VIEW,.buffer=(PmBuffer)(uintptr_t)p->meta,.offset=i*64*8,.count=64},{.kind=PM_ARG_F32,.f=launch.epsilon},{.kind=PM_ARG_VIEW,.buffer=(PmBuffer)(uintptr_t)p->input,.count=p->plan.input_elements},{.kind=PM_ARG_VIEW,.buffer=(PmBuffer)(uintptr_t)p->workspace,.count=p->plan.workspace},{.kind=PM_ARG_VIEW,.buffer=(PmBuffer)(uintptr_t)p->stage,.count=p->plan.output_elements},{.kind=PM_ARG_VIEW,.buffer=(PmBuffer)(uintptr_t)p->flags,.count=p->plan.flag_elements}};
            if(pm_launch(batch,p->mk,launch.items,1,256,args,6)!=PM_OK){z=TI(driver);break;}}
#endif
    }
    if(cuda){if(pc_sync_stream(p->stream)!=PC_OK)z=TI(driver);if(!z){if(p->plan.output_elements && pc_download(p->readback,p->stage,(size_t)p->plan.output_elements*4,p->stream)!=PC_OK)z=TI(driver);if(!z && p->plan.flag_elements && pc_download(p->host_flags,p->flags,(size_t)p->plan.flag_elements,p->stream)!=PC_OK)z=TI(driver);if(pc_sync_stream(p->stream)!=PC_OK)z=TI(driver);}}
#ifdef __APPLE__
    else {if(pm_wait(batch,NULL)!=PM_OK)z=TI(driver);if(!z){memcpy(p->readback,ti_pointer(p,p->stage),(size_t)p->plan.output_elements*4);memcpy(p->host_flags,ti_pointer(p,p->flags),(size_t)p->plan.flag_elements);}}
#endif
    return z;
}
coax_t_TiStatus pti_run(Pti *p,const float *input,int64_t count,float *out,int64_t capacity){
    if(!p)return TI(state);
    coax_t_TiStatus z=ti_inputs(p->plan,ti_floats(input,count),capacity,ti_overlap(p,input,count,out,capacity));if(z)return z;
    coax_t_TiLifeResult begin=ti_begin(p->life);if(begin.status)return begin.status;p->life=begin.state;
    if(p->plan.target.device==DEVICE(cpu)){
        if(!p->plan.reuse){coax_t_TiResult r=ti_cpu_checked(p->plan.program,false,ti_floats(input,count),ti_floats(ti_pointer(p,p->workspace),p->plan.workspace),ti_floats(ti_pointer(p,p->stage),p->plan.output_elements),ti_floats(out,capacity),ti_bytes(ti_pointer(p,p->flags),p->plan.flag_elements),false);z=r.status;}
        else {
            for(int64_t command=0;command<p->plan.launches;command++){coax_t_TiLaunch *launch=&p->launches[command];for(int64_t i=0;i<launch->items;i++)tik_run(i,(coax_v_s64){launch->meta.e,64},launch->epsilon,ti_floats(input,count),ti_floats(ti_pointer(p,p->workspace),p->plan.workspace),ti_floats(ti_pointer(p,p->stage),p->plan.output_elements),ti_bytes(ti_pointer(p,p->flags),p->plan.flag_elements));}
            z=ti_publish(ti_bytes(ti_pointer(p,p->flags),p->plan.flag_elements),ti_floats(ti_pointer(p,p->stage),p->plan.output_elements),ti_floats(out,capacity));
        }
    }else {z=ti_gpu_run(p,input);if(!z)z=ti_publish(ti_bytes(p->host_flags,p->plan.flag_elements),ti_floats(p->readback,p->plan.output_elements),ti_floats(out,capacity));}
    p->life=ti_end(p->life,z);p->stats.runs++;return z;
}
coax_t_TiStatus pti_close(Pti **out){if(!out || !*out)return TI(state);coax_t_TiStatus z=ti_close_admit((*out)->life);if(z)return z;ti_dispose(*out);*out=NULL;return TI(ok);}
