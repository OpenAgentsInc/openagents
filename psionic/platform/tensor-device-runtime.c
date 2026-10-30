/* Native ownership/effects for Coax placement plans and generated kernels.
   No dtype arithmetic, RNG, shape inference, ownership policy or fallback. */
#include "tensor-device-runtime.h"
#include "cuda-host.h"
#include "spans.h"
#ifdef __APPLE__
#include "metal-host.h"
#endif
#include <stdlib.h>
#include <string.h>
#define OK COAX_TdStatus_ok
#define CPU COAX_TdDevice_cpu
#define CUDA COAX_TdDevice_cuda
#define METAL COAX_TdDevice_metal
#define F32 COAX_TdDtype_f32
#define ERR(name) COAX_TdStatus_##name
struct PtdSession {coax_t_TdSessionState state;PcModule module;PcKernel kernels[3];PcStream stream;
#ifdef __APPLE__
    PmModule metal;PmKernel mk[3];
#endif
};
typedef struct PtdAllocation PtdAllocation;
struct PtdAllocation {PtdSession *session;coax_t_TdState state;coax_t_TdAllocation plan;PcBuffer data,temp;int64_t meta[15];
    PtdAllocation *deps[2];int weight[2];void *host_staging;coax_t_TdStatus effect;int queued;
#ifdef __APPLE__
    PmBatch batch;
#endif
};
struct PtdTensor {PtdAllocation *root;coax_t_TvView view;};
static coax_t_TdIds identities;static int initialized;
static coax_v_u8 bytes(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_s64 longs(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_t_TdStatus issue(int session,int64_t *out){if(!initialized){identities=td_initial_ids();initialized=1;}coax_t_TdIssued r=td_issue(identities,session!=0);if(r.status)return r.status;identities=r.next;*out=r.identity;return OK;}
static coax_t_TdStatus error(PtdSession *s,coax_t_TdStatus status){if(status==ERR(driver))s->state=td_poison(s->state);return status;}
static coax_t_TdStatus allocate_buffer(PtdSession *s,size_t n,PcBuffer *out){
    if(!n){*out=0;return OK;}if(s->state.device==CPU){void *p=calloc(n,1);if(!p)return ERR(memory);*out=(PcBuffer)(uintptr_t)p;return OK;}
    if(s->state.device==CUDA){PcStatus z=pc_alloc(n,out);return z==PC_OK?OK:error(s,z==PC_MEMORY?ERR(memory):ERR(driver));}
#ifdef __APPLE__
    PmBuffer b=NULL;PmStatus z=pm_alloc(n,&b);if(z!=PM_OK)return error(s,z==PM_MEMORY?ERR(memory):ERR(driver));*out=(PcBuffer)(uintptr_t)b;return OK;
#else
    return ERR(absent);
#endif
}
static coax_t_TdStatus free_buffer(PtdSession *s,PcBuffer b){if(!b)return OK;if(s->state.device==CPU){free((void*)(uintptr_t)b);return OK;}if(s->state.device==CUDA)return pc_free(b)==PC_OK?OK:error(s,ERR(driver));
#ifdef __APPLE__
    return pm_free((PmBuffer)(uintptr_t)b)==PM_OK?OK:error(s,ERR(driver));
#else
    return ERR(absent);
#endif
}
static uint8_t *host_pointer(PtdSession *s,PcBuffer b){if(s->state.device==CPU)return(uint8_t*)(uintptr_t)b;
#ifdef __APPLE__
    if(s->state.device==METAL)return pm_contents((PmBuffer)(uintptr_t)b);
#endif
    return NULL;
}
static coax_t_TdStatus upload(PtdSession *s,PcBuffer b,int64_t offset,const void *source,size_t n){if(!n)return OK;if(s->state.device==CUDA)return pc_upload(b+(uint64_t)offset,source,n,s->stream)==PC_OK?OK:error(s,ERR(driver));memcpy(host_pointer(s,b)+offset,source,n);return OK;}
static void unload(PtdSession *s){if(s->state.device==CUDA){if(s->stream)pc_destroy_stream(s->stream);if(s->module)pc_unload(s->module);}
#ifdef __APPLE__
    if(s->state.device==METAL){for(int i=0;i<3;i++)if(s->mk[i])pm_kernel_free(s->mk[i]);if(s->metal)pm_unload(s->metal);}
#endif
}
coax_t_TdStatus ptd_open(coax_t_TdDevice device,int64_t budget,const char *source,size_t n,PtdSession **out){
    if(!out)return ERR(state);
    int64_t identity;coax_t_TdStatus z=issue(1,&identity);if(z)return z;uint32_t endian=1;
    coax_t_TdSessionResult r=td_session(identity,device,budget,true,*(uint8_t*)&endian==1);if(r.status)return r.status;if(*out)return ERR(state);
    
    PtdSession *s=calloc(1,sizeof *s);if(!s)return ERR(memory);s->state=r.state;
    if(device==CUDA){PcDeviceInfo info;PcStatus p=pc_open(&info);if(p!=PC_OK){free(s);return p==PC_ABSENT?ERR(absent):ERR(driver);}double seconds;p=pc_compile(source,n,"tensor-device.cu",&s->module,&seconds);if(p!=PC_OK){unload(s);free(s);return ERR(driver);}if(pc_stream(&s->stream)!=PC_OK){unload(s);free(s);return ERR(driver);}}
#ifdef __APPLE__
    if(device==METAL){PmDeviceInfo info;if(pm_open(&info)!=PM_OK){free(s);return ERR(absent);}double seconds;if(pm_compile(source,n,"tensor-device.metal",&s->metal,&seconds)!=PM_OK){unload(s);free(s);return ERR(driver);}}
#else
    if(device==METAL){free(s);return ERR(absent);}
#endif
    if(device!=CPU)for(int i=0;i<3;i++){coax_string name=td_entry(i);char label[64];memcpy(label,name.data,(size_t)name.count);label[name.count]=0;
        if(device==CUDA){if(pc_kernel(s->module,label,&s->kernels[i])!=PC_OK){unload(s);free(s);return ERR(driver);}}
#ifdef __APPLE__
        else if(pm_kernel(s->metal,label,&s->mk[i])!=PM_OK){unload(s);free(s);return ERR(driver);}
#endif
    }
    *out=s;return OK;
}
coax_t_TdStatus ptd_close(PtdSession **ptr){if(!ptr || !*ptr)return ERR(state);
    PtdSession *s=*ptr;coax_t_TdStatus z=td_close_plan(s->state);if(z)return z;unload(s);free(s);*ptr=NULL;return OK;}
coax_t_TdSessionState ptd_stats(const PtdSession *s){return s?s->state:(coax_t_TdSessionState){0};}
PtdInfo ptd_info(const PtdTensor *t){return t?(PtdInfo){t->root->state,t->view}:(PtdInfo){0};}
static coax_t_TdStatus make(PtdSession *s,coax_t_TvShape shape,coax_t_TdDtype dtype,int64_t temporary,PtdTensor **out){
    coax_t_TdAllocation p=td_allocate_plan(s->state,shape,dtype,temporary,*out==NULL);if(p.status)return p.status;int64_t id;coax_t_TdStatus z=issue(0,&id);if(z)return z;
    PtdAllocation *a=calloc(1,sizeof *a);PtdTensor *t=calloc(1,sizeof *t);if(!a || !t){free(a);free(t);return ERR(memory);}a->session=s;a->plan=p;
    z=allocate_buffer(s,(size_t)p.resident,&a->data);if(!z)z=allocate_buffer(s,(size_t)p.temporary,&a->temp);if(z){free_buffer(s,a->temp);free_buffer(s,a->data);free(a);free(t);return z;}
    coax_t_TdChange c=td_commit(s->state,shape,dtype,temporary,id);if(!c.applied){free_buffer(s,a->temp);free_buffer(s,a->data);free(a);free(t);return c.status;}s->state=c.session;a->state=c.state;t->root=a;t->view=tv_contiguous(a->state.owner,shape,false).view;*out=t;return OK;
}
#define I(x) (PcArg){.kind=PC_ARG_INT,.i=(x)}
#define U(x) (PcArg){.kind=PC_ARG_U32,.i=(x)}
#define V(buffer,offset,n) (PcArg){.kind=PC_ARG_VIEW,.data=(buffer)+(uint64_t)(offset),.count=(n)}
static coax_t_TdStatus launch(PtdTensor *out,int kind,int64_t source_dtype,int64_t second_dtype,PtdSession *as,PcBuffer ab,int64_t an,PtdSession *bs,PcBuffer bb,int64_t bn,coax_t_TdDropout r){
    PtdAllocation *a=out->root;PtdSession *s=a->session;int64_t n=a->state.owner.elements;if(!n)return OK;
    PcArg args[13];int count=0;
    if(kind==0){args[count++]=I(source_dtype);args[count++]=I(a->state.dtype);args[count++]=V(a->data,a->plan.meta,15);args[count++]=V(ab,0,an);}
    else if(kind==1){args[count++]=I(a->state.dtype);args[count++]=U(r.seed);args[count++]=U(r.stream);args[count++]=I(r.first);}
    else {args[count++]=I(source_dtype);args[count++]=I(second_dtype);args[count++]=I(a->state.dtype);args[count++]=U(r.seed);args[count++]=U(r.stream);args[count++]=I(r.first);args[count++]=I(r.keep);args[count++]=I(r.training?1:0);args[count++]=V(a->data,a->plan.meta,15);args[count++]=V(ab,0,an);args[count++]=V(bb,0,bn);}
    args[count++]=V(a->data,0,a->plan.payload);args[count++]=V(a->data,a->plan.flags,n);
    if(s->state.device==CPU){coax_v_u8 av=bytes(host_pointer(as,ab),an),bv=bytes(bs?host_pointer(bs,bb):NULL,bn),ov=bytes(host_pointer(s,a->data),a->plan.payload),flags=bytes(host_pointer(s,a->data)+a->plan.flags,n);coax_v_s64 meta=longs(a->meta,15);
        for(int64_t i=0;i<n;i++){if(kind==0)tdk_cast(i,source_dtype,a->state.dtype,meta,av,ov,flags);else if(kind==1)tdk_uniform(i,a->state.dtype,r.seed,r.stream,r.first,ov,flags);else tdk_program(i,source_dtype,second_dtype,a->state.dtype,r.seed,r.stream,r.first,r.keep,r.training?1:0,meta,av,bv,ov,flags);}return OK;
    }
    coax_t_TdStatus z=upload(s,a->data,a->plan.meta,a->meta,sizeof a->meta);if(z){a->effect=z;return z;}
    if(s->state.device==CUDA){PcStatus p=pc_launch(s->kernels[kind],n,256,args,count,s->stream);a->queued=1;if(p!=PC_OK){a->effect=ERR(driver);return error(s,a->effect);}return OK;}
#ifdef __APPLE__
    PmArg ma[13];for(int i=0;i<count;i++){ma[i]=(PmArg){.kind=(PmArgKind)args[i].kind,.i=args[i].i};if(args[i].kind==PC_ARG_VIEW){/* Rebind byte offsets to the one native MTLBuffer handle. */
        if((kind==0 && i==2) || (kind==2 && i==8)){ma[i].buffer=(PmBuffer)(uintptr_t)a->data;ma[i].offset=a->plan.meta;ma[i].count=15;}
        else if(i==count-2){ma[i].buffer=(PmBuffer)(uintptr_t)a->data;ma[i].count=a->plan.payload;}
        else if(i==count-1){ma[i].buffer=(PmBuffer)(uintptr_t)a->data;ma[i].offset=a->plan.flags;ma[i].count=n;}
        else if((kind==0 && i==3) || (kind==2 && i==9)){ma[i].buffer=(PmBuffer)(uintptr_t)ab;ma[i].count=an;}
        else {ma[i].buffer=(PmBuffer)(uintptr_t)bb;ma[i].count=bn;}
    }}
    PmStatus p=pm_begin(&a->batch);if(p==PM_OK){a->queued=1;p=pm_launch(a->batch,s->mk[kind],n,1,256,ma,count);}if(p!=PM_OK){a->effect=ERR(driver);return error(s,a->effect);}return OK;
#else
    return ERR(absent);
#endif
}
static coax_t_TdStatus finish(PtdTensor *t,coax_t_TdStatus requested){
    if(!t)return ERR(state);
    PtdAllocation *a=t->root;PtdSession *s=a->session;if(!a->state.pending)return td_access(s->state,a->state);
    coax_t_TdStatus effect=a->effect?a->effect:requested;uint8_t *flags=NULL,*allocated=NULL;int64_t n=a->state.owner.elements;
    if(s->state.device==CUDA){if(pc_sync_stream(s->stream)!=PC_OK)effect=ERR(driver);if(effect==OK && n){allocated=malloc((size_t)n);if(!allocated)effect=ERR(memory);else if(pc_download(allocated,a->data+(uint64_t)a->plan.flags,(size_t)n,s->stream)!=PC_OK || pc_sync_stream(s->stream)!=PC_OK)effect=ERR(driver);}flags=allocated;}
#ifdef __APPLE__
    else if(s->state.device==METAL){if(a->queued && pm_wait(a->batch,NULL)!=PM_OK)effect=ERR(driver);a->batch=NULL;flags=host_pointer(s,a->data)+a->plan.flags;}
#endif
    else flags=host_pointer(s,a->data)+a->plan.flags;
    a->queued=0;coax_t_TdStatus freed=free_buffer(s,a->temp);a->temp=0;if(freed)effect=freed;free(a->host_staging);a->host_staging=NULL;
    coax_t_TdChange c=td_finish(s->state,a->state,bytes(flags,n),effect);free(allocated);if(!c.applied)return c.status;s->state=c.session;a->state=c.state;
    for(int i=0;i<2;i++)if(a->deps[i]){coax_t_TdStateResult r=td_return(a->deps[i]->state,a->weight[i]);if(r.status)return r.status;a->deps[i]->state=r.state;a->deps[i]=NULL;}
    return c.status;
}
coax_t_TdStatus ptd_wait(PtdTensor *t){return finish(t,OK);}
coax_t_TdStatus ptd_cancel(PtdTensor *t){return finish(t,ERR(cancelled));}
coax_t_TdStatus ptd_release(PtdTensor **ptr){if(!ptr || !*ptr)return ERR(state);
    PtdTensor *t=*ptr;PtdAllocation *a=t->root;coax_t_TdChange c=td_drop(a->session->state,a->state);if(c.status)return c.status;a->session->state=c.session;a->state=c.state;coax_t_TdStatus z=OK;
    if(!a->state.refs){z=free_buffer(a->session,a->data);free(a);}free(t);*ptr=NULL;return z;
}
coax_t_TdStatus ptd_view(PtdTensor *source,coax_t_TvView view,PtdTensor **out){if(!source || !out)return ERR(state);
    PtdAllocation *a=source->root;coax_t_TdStateResult r=td_retain(a->session->state,a->state,view,*out==NULL);if(r.status)return r.status;PtdTensor *t=calloc(1,sizeof *t);if(!t)return ERR(memory);t->root=a;t->view=view;a->state=r.state;*out=t;return OK;}
static coax_t_TdStatus discard(PtdTensor *t,coax_t_TdStatus original){if(t){if(t->root->state.pending){t->root->effect=original;ptd_wait(t);}ptd_release(&t);}return original;}
static coax_t_TdStatus imported(PtdSession *s,coax_t_TvShape shape,coax_t_TdDtype sd,const uint8_t *values,int64_t count,int float_units,coax_t_TdDtype dtype,PtdTensor **out){
    if(!s || !out)return ERR(state);
    coax_t_TdAllocation p=td_import_plan(s->state,shape,sd,dtype,count,float_units!=0,values!=NULL,*out==NULL);if(p.status)return p.status;PtdTensor *t=NULL;coax_t_TdStatus z=make(s,shape,dtype,p.temporary,&t);if(z)return z;
    coax_t_TvStorage owner=tv_owner(1,1,p.elements).storage;coax_t_TvView v=tv_contiguous(owner,shape,false).view;z=td_bind(owner,v,owner,v,longs(t->root->meta,15));if(z)return discard(t,z);
    PtdSession cpu={.state={.device=CPU}};PtdSession *input=&cpu;PcBuffer buffer=(PcBuffer)(uintptr_t)values;
    if(s->state.device!=CPU){input=s;buffer=t->root->temp;z=upload(s,buffer,0,values,(size_t)p.input_bytes);if(z)return discard(t,z);}
    z=launch(t,0,sd,0,input,buffer,p.input_bytes,NULL,0,0,(coax_t_TdDropout){0});if(z)return discard(t,z);z=ptd_wait(t);if(z)return discard(t,z);*out=t;return OK;
}
coax_t_TdStatus ptd_from_f32(PtdSession *s,coax_t_TvShape shape,coax_t_TdDtype dtype,const float *values,int64_t n,PtdTensor **out){return imported(s,shape,F32,(const uint8_t*)values,n,1,dtype,out);}
coax_t_TdStatus ptd_from_bytes(PtdSession *s,coax_t_TvShape shape,coax_t_TdDtype sd,const uint8_t *values,int64_t n,coax_t_TdDtype dtype,PtdTensor **out){return imported(s,shape,sd,values,n,0,dtype,out);}
coax_t_TdStatus ptd_to(PtdTensor *source,PtdSession *target,coax_t_TdDtype dtype,PtdTensor **out){
    if(!source || !target || !out)return ERR(state);
    PtdAllocation *src=source->root;coax_t_TdCopyPlan p=td_copy_plan(target->state,src->session->state,src->state,source->view,dtype,*out==NULL);if(p.status)return p.status;
    PtdTensor *t=NULL;coax_t_TdStatus z=make(target,source->view.shape,dtype,p.temporary,&t);if(z)return z;PtdAllocation *a=t->root;
    z=td_bind(src->state.owner,source->view,src->state.owner,source->view,longs(a->meta,15));if(z)return discard(t,z);
    coax_t_TdStateResult borrow=td_borrow(src->state,1);if(borrow.status)return discard(t,borrow.status);src->state=borrow.state;a->deps[0]=src;a->weight[0]=1;
    PcBuffer input=src->data;PtdSession *is=src->session;
    if(!p.direct){uint8_t *host=host_pointer(src->session,src->data);if(src->session->state.device==CUDA){a->host_staging=malloc((size_t)src->plan.payload? (size_t)src->plan.payload:1);if(!a->host_staging)return discard(t,ERR(memory));host=a->host_staging;
        if(pc_download(host,src->data,(size_t)src->plan.payload,src->session->stream)!=PC_OK || pc_sync_stream(src->session->stream)!=PC_OK)return discard(t,error(src->session,ERR(driver)));}
        z=upload(target,a->temp,0,host,(size_t)src->plan.payload);if(z)return discard(t,z);input=a->temp;is=target;
    }
    z=launch(t,0,src->state.dtype,0,is,input,src->plan.payload,NULL,0,0,(coax_t_TdDropout){0});if(z)return discard(t,z);z=ptd_wait(t);if(z)return discard(t,z);*out=t;return OK;
}
coax_t_TdStatus ptd_random(PtdSession *s,coax_t_TvShape shape,coax_t_TdDtype dtype,coax_t_TdRng rng,PtdTensor **out){
    if(!s || !out)return ERR(state);
    coax_t_TvCount n=tv_elements(shape);if(n.status)return ERR(shape);coax_t_TdRange range=td_rng_reserve(rng,n.elements);if(range.status)return range.status;
    PtdTensor *t=NULL;coax_t_TdStatus z=make(s,shape,dtype,0,&t);if(z)return z;coax_t_TdDropout r={.seed=rng.seed,.stream=rng.stream,.first=rng.next};
    z=launch(t,1,0,0,s,0,0,NULL,0,0,r);if(z)return discard(t,z);z=ptd_wait(t);if(z)return discard(t,z);*out=t;return OK;
}
coax_t_TdStatus ptd_program(PtdSession *s,PtdTensor *ta,PtdTensor *tb,coax_t_TdDropout r,PtdTensor **out){
    if(!s || !ta || !tb || !out)return ERR(state);
    PtdAllocation *a=ta->root,*b=tb->root;coax_t_TdProgramPlan p=td_program_plan(s->state,a->state,ta->view,b->state,tb->view,r,*out==NULL);if(p.status)return p.status;
    PtdTensor *t=NULL;coax_t_TdStatus z=make(s,p.shape,p.dtype,0,&t);if(z)return z;PtdAllocation *d=t->root;
    z=td_bind(a->state.owner,ta->view,b->state.owner,tb->view,longs(d->meta,15));if(z)return discard(t,z);
    coax_t_TdStateResult borrow=td_borrow(a->state,p.borrow_a);if(borrow.status)return discard(t,borrow.status);a->state=borrow.state;d->deps[0]=a;d->weight[0]=(int)p.borrow_a;
    if(p.borrow_b){borrow=td_borrow(b->state,p.borrow_b);if(borrow.status)return discard(t,borrow.status);b->state=borrow.state;d->deps[1]=b;d->weight[1]=(int)p.borrow_b;}
    z=launch(t,2,a->state.dtype,b->state.dtype,s,a->data,a->plan.payload,s,b->data,b->plan.payload,r);if(z)return discard(t,z);*out=t;return OK;
}
static bool output_overlap(PtdTensor *t,uint8_t *out,int64_t n){PtdAllocation *a=t->root;return psionic_spans_overlap(out,n,1,t,1,sizeof *t) || psionic_spans_overlap(out,n,1,a,1,sizeof *a) || psionic_spans_overlap(out,n,1,a->session,1,sizeof *a->session) || (host_pointer(a->session,a->data)!=NULL && psionic_spans_overlap(out,n,1,host_pointer(a->session,a->data),a->plan.resident,1));}
coax_t_TdStatus ptd_export(PtdTensor *t,uint8_t *out,int64_t capacity){
    if(!t)return ERR(state);
    PtdAllocation *a=t->root;PtdSession *s=a->session;coax_t_TdStatus z=td_export_plan(s->state,a->state,t->view,capacity,output_overlap(t,out,capacity));if(z)return z;
    coax_t_TvLayout layout=tv_layout(a->state.owner,t->view);coax_t_TdAllocation p=td_export_layout(a->state,t->view);if(p.status)return p.status;
    uint8_t *raw=host_pointer(s,a->data),*download=NULL,*stage=malloc((size_t)p.payload? (size_t)p.payload:1),*flags=malloc((size_t)layout.elements? (size_t)layout.elements:1);int64_t meta[15];
    if(!stage || !flags){free(stage);free(flags);return ERR(memory);}z=td_bind(a->state.owner,t->view,a->state.owner,t->view,longs(meta,15));if(z){free(stage);free(flags);return z;}
    if(s->state.device==CUDA){download=malloc((size_t)a->plan.payload? (size_t)a->plan.payload:1);if(!download){free(stage);free(flags);return ERR(memory);}if(pc_download(download,a->data,(size_t)a->plan.payload,s->stream)!=PC_OK || pc_sync_stream(s->stream)!=PC_OK){free(download);free(stage);free(flags);return error(s,ERR(driver));}raw=download;}
    for(int64_t i=0;i<layout.elements;i++)tdk_cast(i,a->state.dtype,a->state.dtype,longs(meta,15),bytes(raw,a->plan.payload),bytes(stage,p.payload),bytes(flags,layout.elements));
    z=td_export_commit(bytes(flags,layout.elements),bytes(stage,p.payload),bytes(out,capacity));free(download);free(stage);free(flags);return z;
}
