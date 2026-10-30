/* Allocation, byte transport, observed addresses and producer deleters only.
 * Coax decides formats, layout admission, bounds and lease transitions.
 */
#include "interchange.h"
#include "interchange-generated.h"
#include <stdlib.h>
#include <string.h>
#include <float.h>
#include <stdbool.h>
_Static_assert(sizeof(float)==4 && FLT_RADIX==2 && FLT_MANT_DIG==24 && FLT_MAX_EXP==128,"binary32 host");
struct PsTensorHandleV1 {coax_t_IcLease lease;coax_t_IcDlPlan plan;const void *data;void *owned,*producer;uint32_t kind;};
static uint64_t live;
static bool overlap(const void *a,uint64_t na,const void *b,uint64_t nb){if(!a || !b || !na || !nb)return false;uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;return x<=y?y-x<na:x-y<nb;}
static PsResultV1 result(uint32_t status,uint64_t written){return (PsResultV1){status,0,status?0:written};}
static uint32_t host(void){const uint32_t one=1;return *(const uint8_t*)&one==1?PS_OK:PS_VERSION;}
uint32_t psionic_interchange_abi_version(void){return 1;}
static uint32_t params(const PsParameterV1 *p,uint64_t count,coax_t_IcTensor t[16],coax_t_IcPlan *plan){
    if(host())return host();if(count>INT64_MAX || !ic_count((int64_t)count))return PS_CAPACITY;if(!p)return PS_ARGUMENT;
    for(uint64_t i=0;i<count;i++){
        const PsTensorV1 *d=&p[i].tensor;uint32_t z=ic_fields(d->abi_version,d->dtype,d->rank,p[i].name_bytes,d->byte_count,d->flags,d->data!=NULL);if(z)return z;if(!p[i].name)return PS_ARGUMENT;
        t[i]=(coax_t_IcTensor){.name_count=(int64_t)p[i].name_bytes,.shape={.rank=d->rank},.dtype=d->dtype,.bytes=(int64_t)d->byte_count,.present=true,.version=d->abi_version,.flags=d->flags};
        memcpy(t[i].name.e,p[i].name,p[i].name_bytes);memcpy(t[i].shape.dims.e,d->dims,sizeof d->dims);
    }
    *plan=ic_plan((coax_v_t_IcTensor){t,(int64_t)count});return plan->status;
}
PsResultV1 psionic_weights_size_v1(const PsParameterV1 *p,uint64_t n){coax_t_IcTensor t[16];coax_t_IcPlan plan;uint32_t z=params(p,n,t,&plan);return result(z,z?0:(uint64_t)plan.total);}
PsResultV1 psionic_weights_write_v1(const PsParameterV1 *p,uint64_t n,void *out,uint64_t cap){
    coax_t_IcTensor t[16];coax_t_IcPlan plan;uint32_t z=params(p,n,t,&plan);if(z)return result(z,0);
    bool aliases=overlap(out,cap,p,n*sizeof *p);for(uint64_t i=0;i<n;i++)aliases|=overlap(out,cap,p[i].name,p[i].name_bytes)||overlap(out,cap,p[i].tensor.data,p[i].tensor.byte_count);
    z=ic_write_destination(plan,cap,out!=NULL,aliases);if(z)return result(z,0);
    uint8_t *data=malloc((size_t)plan.payload),*stage=malloc((size_t)plan.total);if(!data || !stage){free(data);free(stage);return result(PS_MEMORY,0);}
    for(uint64_t i=0;i<n;i++)memcpy(data+plan.source.e[i],p[i].tensor.data,(size_t)t[i].bytes);
    coax_t_IcResult r=ic_write((coax_v_t_IcTensor){t,(int64_t)n},(coax_v_u8){data,plan.payload},(coax_v_u8){out,plan.total},(coax_v_u8){stage,plan.total},false);
    free(stage);free(data);return result(r.status,(uint64_t)r.written);
}
PsResultV1 psionic_weights_inspect_v1(const void *file,uint64_t size,PsParameterInfoV1 *out,uint64_t cap){
    if(host())return result(host(),0);if(!file || !out)return result(PS_ARGUMENT,0);if(size>INT64_MAX || cap>16)return result(PS_CAPACITY,0);bool aliases=overlap(file,size,out,cap*sizeof *out);
    coax_t_IcTensor t[16];coax_t_IcResult r=ic_inspect((coax_v_u8){(uint8_t*)file,(int64_t)size},(coax_v_t_IcTensor){t,(int64_t)cap},aliases);if(r.status)return result(r.status,0);
    for(int64_t i=0;i<r.written;i++){PsParameterInfoV1 x={.name_bytes=(uint32_t)t[i].name_count,.dtype=t[i].dtype,.rank=(int32_t)t[i].shape.rank,.data_offset=(uint64_t)t[i].source,.byte_count=(uint64_t)t[i].bytes};memcpy(x.name,t[i].name.e,(size_t)t[i].name_count);memcpy(x.dims,t[i].shape.dims.e,sizeof x.dims);out[i]=x;}
    return result(0,(uint64_t)r.written);
}
static void producer_drop(void *p,uint32_t kind){if(!p)return;if(kind==PS_DLPACK_LEGACY){DLManagedTensor *t=p;if(t->deleter)t->deleter(t);}else{DLManagedTensorVersioned *t=p;if(t->deleter)t->deleter(t);}}
static void drop(PsTensorHandleV1 *h){coax_t_IcLeaseChange r=ic_release(h->lease);if(r.status)return;h->lease=r.next;if(r.dispose){producer_drop(h->producer,h->kind);free(h->owned);free(h);live--;}}
static PsTensorHandleV1 *handle(coax_t_IcDlPlan plan){PsTensorHandleV1 *h=calloc(1,sizeof *h);if(h){h->lease=(coax_t_IcLease){.refs=1,.live=true};h->plan=plan;live++;}return h;}
uint32_t psionic_tensor_copy_v1(const PsTensorV1 *d,PsTensorHandleV1 **out){
    if(!d || !out || *out)return PS_ARGUMENT;if(host())return host();coax_t_TvShape shape={.rank=d->rank};memcpy(shape.dims.e,d->dims,sizeof d->dims);
    coax_t_IcDlPlan plan=ic_descriptor(d->abi_version,d->dtype,shape,d->byte_count,d->data!=NULL,d->flags);if(plan.status)return plan.status;
    PsTensorHandleV1 *h=handle(plan);if(!h)return PS_MEMORY;if(plan.bytes){h->owned=malloc((size_t)plan.bytes);if(!h->owned){drop(h);return PS_MEMORY;}memcpy(h->owned,d->data,(size_t)plan.bytes);h->data=h->owned;}*out=h;return PS_OK;
}
uint32_t psionic_tensor_view_v1(const PsTensorHandleV1 *h,PsTensorV1 *out){if(!h || !out)return PS_ARGUMENT;uint32_t z=ic_access(h->lease);if(z)return z;z=ic_destination(true,overlap(h,sizeof *h,out,sizeof *out)||overlap(h->data,(uint64_t)h->plan.bytes,out,sizeof *out));if(z)return z;PsTensorV1 d={.abi_version=1,.rank=(int32_t)h->plan.shape.rank,.flags=PS_READONLY,.data=h->data,.byte_count=(uint64_t)h->plan.bytes};memcpy(d.dims,h->plan.shape.dims.e,sizeof d.dims);*out=d;return PS_OK;}
uint32_t psionic_tensor_retain_v1(PsTensorHandleV1 *h,PsTensorHandleV1 **out){if(!h || !out || *out)return PS_ARGUMENT;coax_t_IcLeaseChange r=ic_retain(h->lease);if(r.status)return r.status;h->lease=r.next;*out=h;return PS_OK;}
uint32_t psionic_tensor_release_v1(PsTensorHandleV1 **out){if(!out || !*out)return PS_ARGUMENT;uint32_t z=ic_access((*out)->lease);if(z)return z;PsTensorHandleV1 *h=*out;*out=NULL;drop(h);return PS_OK;}
uint32_t psionic_dlpack_import_v1(void *carrier,uint32_t kind,PsTensorHandleV1 **out){
    if(!carrier || !out || *out || kind>1)return PS_ARGUMENT;
    uint32_t major=0,minor=0;DLTensor *t;uint64_t flags=0;
    if(kind==PS_DLPACK_VERSIONED){DLManagedTensorVersioned *v=carrier;major=v->version.major;minor=v->version.minor;}
    uint32_t z=ic_dl_version(kind,major,minor);if(!z)z=host();if(z){producer_drop(carrier,kind);return z;}
    if(kind==PS_DLPACK_LEGACY)t=&((DLManagedTensor*)carrier)->dl_tensor;else{DLManagedTensorVersioned *v=carrier;t=&v->dl_tensor;flags=v->flags;}
    z=ic_dl_prefix(kind,minor,t->device.device_type,t->device.device_id,t->ndim,t->dtype.code,t->dtype.bits,t->dtype.lanes,flags,t->shape!=NULL,t->strides!=NULL);
    if(z){producer_drop(carrier,kind);return z;}
    coax_t_TvShape shape={.rank=t->ndim};coax_a4_s64 strides={{0}};for(int i=0;i<t->ndim;i++){shape.dims.e[i]=t->shape[i];if(t->strides)strides.e[i]=t->strides[i];}
    coax_t_IcDlPlan plan=ic_dl_layout(shape,strides,t->strides!=NULL);uintptr_t base=(uintptr_t)t->data;bool span=t->byte_offset<=UINTPTR_MAX-base;uintptr_t at=span?base+(uintptr_t)t->byte_offset:0;
    span=span && (uint64_t)plan.bytes<=UINTPTR_MAX-at;z=ic_pointer(plan,t->byte_offset,t->data!=NULL,at%4==0,span);if(z){producer_drop(carrier,kind);return z;}
    PsTensorHandleV1 *h=handle(plan);if(!h){producer_drop(carrier,kind);return PS_MEMORY;}h->data=plan.bytes?(const void*)at:NULL;h->producer=carrier;h->kind=kind;*out=h;return PS_OK;
}
struct Export {DLManagedTensorVersioned tensor;int64_t shape[4],strides[4];};
static void exported_drop(DLManagedTensorVersioned *t){PsTensorHandleV1 *h=t->manager_ctx;drop(h);free(t);}
uint32_t psionic_tensor_dlpack_v1(PsTensorHandleV1 *h,DLManagedTensorVersioned **out){
    if(!h || !out || *out)return PS_ARGUMENT;coax_t_IcLeaseChange r=ic_retain(h->lease);if(r.status)return r.status;
    struct Export *e=calloc(1,sizeof *e);if(!e)return PS_MEMORY;h->lease=r.next;memcpy(e->shape,h->plan.shape.dims.e,sizeof e->shape);
    memcpy(e->strides,h->plan.strides.e,sizeof e->strides);
    e->tensor=(DLManagedTensorVersioned){.version={1,0},.manager_ctx=h,.deleter=exported_drop,.flags=DLPACK_FLAG_BITMASK_READ_ONLY,.dl_tensor={.data=(void*)h->data,.device={kDLCPU,0},.ndim=(int32_t)h->plan.shape.rank,.dtype={kDLFloat,32,1},.shape=e->shape,.strides=e->strides}};
    *out=&e->tensor;return PS_OK;
}
void psionic_dlpack_dispose_v1(DLManagedTensorVersioned *t){if(t && t->deleter)t->deleter(t);}
uint64_t psionic_tensor_live_v1(void){return live;}
