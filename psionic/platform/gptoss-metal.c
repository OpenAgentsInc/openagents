/* gpt-oss on an Apple GPU: the pg_* API of gptoss-device.c, performing the
   same Coax-authored launch programs (psionic/lib/gptoss/device-plan.coax and
   device-prefill.coax) through metal-host.m. Device effects only: no tensor
   arithmetic, routing, shape policy, cache indexing or token selection is
   written here. Unified memory keeps every weight resident in one shared
   buffer; nothing streams, and there is no CPU fallback: without a Metal GPU
   pg_open reports PC_ABSENT. The PcStatus codes are the CUDA build's, so the
   Responses server and the chat shell link either engine unchanged. */
#include "gptoss-device.h"
#include "metal-host.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
struct PgSession {
    coax_t_GShape shape;coax_t_GDPlan plan;coax_t_GDParam *params;int64_t param_count;
    coax_t_GDOp *ops;coax_t_GDArg *args;PmBuffer *buffers;PmModule module;PmKernel *kernels;
    int poisoned;char error[256];
#ifdef PSIONIC_DEVICE_TEST
    int profile;double *seconds;int *launches;
    int qualify;uint8_t **shadow;int64_t norms,checked,exact,worst_kind,worst_wide_kind;double worst,worst_wide;
#endif
};
static PcStatus pg_status(PgSession *s,PmStatus m){
    if(m==PM_OK)return PC_OK;
    if(s){snprintf(s->error,sizeof s->error,"Metal: %s",pm_log());if(m==PM_DEVICE || m==PM_CHECK)s->poisoned=1;}
    switch(m){case PM_ABSENT:return PC_ABSENT;case PM_COMPILE:return PC_COMPILE;case PM_ARGUMENT:return PC_ARGUMENT;case PM_MEMORY:return PC_MEMORY;default:return PC_DRIVER;}
}
static uint8_t *pg_bytes(PgSession *s,int buffer){return pm_contents(s->buffers[buffer]);}
#ifdef PSIONIC_DEVICE_TEST
/* The CPU repeat (pg_qualify) runs the same Coax kernels compiled to C on
   copies of the workspace buffers; weights are read in place. */
static void *pg_address(PgSession *s,coax_t_GDRef r){
    if(s->shadow && r.buffer>=16 && s->shadow[r.buffer])return s->shadow[r.buffer]+r.offset;
    return pg_bytes(s,(int)r.buffer)+r.offset;
}
#include "gptoss-device-cpu.h"
#endif
static size_t pg_slot_bytes(PgSession *s,int slot){return (size_t)(s->plan.counts.e[slot]*s->plan.width.e[slot]);}
PcStatus pg_open(coax_t_GProfile profile,coax_v_t_GFile files,const char *source,size_t source_bytes,size_t budget,PgSession **out,coax_t_GResult *why){
    *out=NULL;PmDeviceInfo info;PmStatus m=pm_open(&info);if(m)return pg_status(NULL,m);
    /* Unified memory: what the GPU may keep resident is the recommended working set less what is already allocated. */
    int64_t available=(int64_t)info.working_set_bytes-(int64_t)info.allocated_bytes;
    coax_t_GDPlan preflight=gd_preflight(profile,available,(int64_t)budget);if(preflight.status){*why=(coax_t_GResult){.status=preflight.status};return PC_OK;}
    coax_t_GBlock blocks[36],draft[36];coax_t_GModel model;
    *why=g_bind_profile(profile,files,(coax_v_t_GBlock){blocks,36},(coax_v_t_GBlock){draft,36},(coax_v_t_GModel){&model,1},0);if(why->status)return PC_OK;
    PgSession *s=calloc(1,sizeof *s);if(!s)return PC_MEMORY;s->shape=model.shape;PcStatus code=PC_OK;
    double seconds=0;m=pm_compile(source,source_bytes,"gptoss.metal",&s->module,&seconds);if(m){code=pg_status(s,m);goto failed;}
    m=pm_memory(&info);if(m){code=pg_status(s,m);goto failed;}available=(int64_t)info.working_set_bytes-(int64_t)info.allocated_bytes;
    s->plan=gd_plan(model.shape,profile.pins,available,(int64_t)budget);if(s->plan.status){why->status=s->plan.status;goto refused;}
    if((uint64_t)s->plan.upload.e[0]>(uint64_t)info.max_buffer_bytes){snprintf(s->error,sizeof s->error,"the weights need one %lld-byte buffer; this GPU allows %zu",(long long)s->plan.upload.e[0],info.max_buffer_bytes);why->status=COAX_GStatus_capacity;goto refused;}
    s->buffers=calloc((size_t)(16+gd_buffer_count()),sizeof *s->buffers);
    s->param_count=profile.specs.count;s->params=calloc((size_t)s->param_count,sizeof *s->params);s->ops=calloc((size_t)s->plan.max_ops,sizeof *s->ops);s->args=calloc((size_t)s->plan.max_args,sizeof *s->args);s->kernels=calloc((size_t)gd_kernel_count(),sizeof *s->kernels);
#ifdef PSIONIC_DEVICE_TEST
    s->seconds=calloc((size_t)gd_kernel_count(),sizeof *s->seconds);s->launches=calloc((size_t)gd_kernel_count(),sizeof *s->launches);if(!s->seconds || !s->launches){code=PC_MEMORY;goto failed;}
#endif
    if(!s->buffers || !s->params || !s->ops || !s->args || !s->kernels){code=PC_MEMORY;goto failed;}
    *why=gd_params(profile,(coax_v_t_GDParam){s->params,s->param_count});if(why->status)goto refused;
    if(!gd_packed_fits(s->plan,why->bytes)){why->status=COAX_GStatus_capacity;goto refused;}
    for(int64_t i=0;i<gd_kernel_count();i++){coax_string name=gd_kernel_name(i);char label[128];if(name.count<1 || name.count>=127){code=PC_ARGUMENT;goto failed;}memcpy(label,name.data,(size_t)name.count);label[name.count]=0;m=pm_kernel(s->module,label,&s->kernels[i]);if(m){code=pg_status(s,m);goto failed;}}
    /* Every original byte is copied once into one shared buffer: the GPU reads it where the CPU wrote it. */
    m=pm_alloc((size_t)s->plan.upload.e[0],&s->buffers[0]);if(m){code=pg_status(s,m);goto failed;}
    for(int64_t i=0;i<s->param_count;i++){coax_t_GDParam p=s->params[i];memcpy(pg_bytes(s,0)+p.device,files.data[p.file].data.data+p.begin,(size_t)p.bytes);}
    for(int i=0;i<gd_buffer_count();i++){m=pm_alloc(pg_slot_bytes(s,i),&s->buffers[16+i]);if(m){code=pg_status(s,m);goto failed;}}
    /* The rotary table is built by Coax on the host (chapter 43's YaRN, binary64 trigonometry) and never recomputed on the GPU. */
    *why=gd_rope(s->shape.capacity,(coax_v_f32){(float*)pg_bytes(s,41),s->plan.counts.e[25]});if(why->status)goto refused;
    code=pg_reset(s);if(code)goto failed;why->status=COAX_GStatus_ok;*out=s;return PC_OK;
refused:pg_close(s);return PC_OK;
failed:pg_close(s);return code;
}
/* Immutable coefficients and the rotary table survive request resets. The
   GPU is idle here: every program is waited for before its call returns. */
PcStatus pg_reset(PgSession *s){
    if(s->poisoned)return PC_DRIVER;
    for(int i=0;i<gd_buffer_count();i++){if(gd_preserve(i))continue;memset(pg_bytes(s,16+i),0,pg_slot_bytes(s,i));}
    return PC_OK;
}
static PcStatus pg_encode(PgSession *s,PmBatch batch,int64_t i){
    coax_t_GDOp op=s->ops[i];coax_t_GDArg *a=s->args+op.first;PmArg arguments[24];if(op.count>16){snprintf(s->error,sizeof s->error,"operation %lld has %lld arguments",(long long)i,(long long)op.count);return PC_ARGUMENT;}
    for(int64_t j=0;j<op.count;j++){
        if(a[j].view)arguments[j]=(PmArg){.kind=PM_ARG_VIEW,.buffer=s->buffers[a[j].ref.buffer],.offset=a[j].ref.offset,.count=a[j].ref.count};
        else arguments[j]=(PmArg){.kind=PM_ARG_INT,.i=a[j].value};
        if(a[j].view && !arguments[j].buffer){snprintf(s->error,sizeof s->error,"operation %lld names buffer %lld",(long long)i,(long long)a[j].ref.buffer);return PC_ARGUMENT;}
    }
    return pg_status(s,pm_launch(batch,s->kernels[op.kind],op.items,1,(int)op.block,arguments,(int)op.count));
}
#ifdef PSIONIC_DEVICE_TEST
static PcStatus pg_execute_one(PgSession *s,int64_t i);
#endif
/* The whole program in one command buffer, in order; the wait reports a
   failed command buffer or a failed Coax check and poisons the session. */
static PcStatus pg_execute(PgSession *s,coax_t_GDProgram p){
#ifdef PSIONIC_DEVICE_TEST
    if(s->profile || s->qualify){s->norms=0;for(int64_t i=0;i<p.ops;i++){PcStatus code=pg_execute_one(s,i);if(code)return code;}return PC_OK;}
#endif
    PmBatch batch;PmStatus m=pm_begin(&batch);if(m)return pg_status(s,m);PcStatus code=PC_OK;
    for(int64_t i=0;i<p.ops && !code;i++)code=pg_encode(s,batch,i);
    m=pm_wait(batch,NULL);if(code){s->poisoned=1;return code;}return pg_status(s,m);
}
PcStatus pg_step(PgSession *s,int64_t token,int64_t position,int choose,int64_t limit,int wait,int64_t control[8]){
    if(s->poisoned)return PC_DRIVER;coax_t_GDProgram p=gd_program(s->shape,s->plan,(coax_v_t_GDParam){s->params,s->param_count},position,token,choose!=0,limit,(coax_v_t_GDOp){s->ops,s->plan.max_ops},(coax_v_t_GDArg){s->args,s->plan.max_args});
    if(p.status){snprintf(s->error,sizeof s->error,"Coax device program refused: %u",p.status);return PC_ARGUMENT;}
    PcStatus code=pg_execute(s,p);if(code)return code;
    if(wait)return pg_read(s,24,control,8*8);return PC_OK;
}
PcStatus pg_prefill_poll(PgSession *s,const int64_t *tokens,int64_t count,int64_t position,int choose,int64_t limit,int64_t control[8],PgPoll poll,void *context,int *interrupted){
    *interrupted=0;
    if(s->poisoned)return PC_DRIVER;
    coax_t_GResult admitted=gd_prompt(s->shape,position,(coax_v_s64){(int64_t*)tokens,count});
    if(admitted.status){snprintf(s->error,sizeof s->error,"Coax prompt refused: %u",admitted.status);return PC_ARGUMENT;}
    if((size_t)count*8>pg_slot_bytes(s,26))return PC_ARGUMENT;
    memcpy(pg_bytes(s,42),tokens,(size_t)count*8);
    for(int64_t first=0;first<count;){if(poll && poll(context)){*interrupted=1;return PC_OK;}int64_t n=gd_chunk(first,count,s->plan.batch);
        coax_t_GDProgram p=gd_batch_program(s->shape,s->plan,(coax_v_t_GDParam){s->params,s->param_count},position+first,first,n,choose && first+n==count,limit,(coax_v_t_GDOp){s->ops,s->plan.max_ops},(coax_v_t_GDArg){s->args,s->plan.max_args});
        if(p.status){snprintf(s->error,sizeof s->error,"Coax prompt program refused: %u",p.status);return PC_ARGUMENT;}
        PcStatus code=pg_execute(s,p);if(code)return code;code=pg_read(s,24,control,64);if(code)return code;
        if(control[0])return PC_OK;first+=n;
    }return PC_OK;
}
PcStatus pg_prefill(PgSession *s,const int64_t *tokens,int64_t count,int64_t position,int choose,int64_t limit,int64_t control[8]){
    int interrupted;return pg_prefill_poll(s,tokens,count,position,choose,limit,control,NULL,NULL,&interrupted);
}
PcStatus pg_read(PgSession *s,int slot,void *out,size_t bytes){
    if(slot<0 || slot>=gd_buffer_count() || bytes>pg_slot_bytes(s,slot))return PC_ARGUMENT;
    memcpy(out,pg_bytes(s,16+slot),bytes);return PC_OK;
}
PcStatus pg_wait(PgSession *s){return s->poisoned?PC_DRIVER:PC_OK;}
void pg_close(PgSession *s){if(!s)return;
    if(s->kernels)for(int64_t i=0;i<gd_kernel_count();i++)pm_kernel_free(s->kernels[i]);
    if(s->buffers)for(int i=0;i<16+gd_buffer_count();i++)pm_free(s->buffers[i]);
    pm_unload(s->module);
#ifdef PSIONIC_DEVICE_TEST
    free(s->seconds);free(s->launches);
    if(s->shadow){for(int i=0;i<16+gd_buffer_count();i++)free(s->shadow[i]);free(s->shadow);}
#endif
    free(s->buffers);free(s->params);free(s->ops);free(s->args);free(s->kernels);free(s);
}
coax_t_GDPlan pg_plan(const PgSession *s){return s->plan;}
coax_t_GShape pg_shape(const PgSession *s){return s->shape;}
const char *pg_error(const PgSession *s){return s->error;}

#ifdef PSIONIC_DEVICE_TEST
PcStatus pg_patch(PgSession *s,size_t offset,const void *bytes,size_t count){if(offset>(size_t)s->plan.upload.e[0] || count>(size_t)s->plan.upload.e[0]-offset)return PC_ARGUMENT;memcpy(pg_bytes(s,0)+offset,bytes,count);return PC_OK;}
void pg_profile(PgSession *s,int enable){
    if(enable){memset(s->seconds,0,(size_t)gd_kernel_count()*sizeof *s->seconds);memset(s->launches,0,(size_t)gd_kernel_count()*sizeof *s->launches);}
    else {double total=0;for(int64_t i=0;i<gd_kernel_count();i++)total+=s->seconds[i];
        for(int64_t i=0;i<gd_kernel_count();i++)if(s->launches[i]){coax_string name=gd_kernel_name(i);printf("kernel %.*s launches=%d device_seconds=%.9f share=%.1f%%\n",(int)name.count,name.data,s->launches[i],s->seconds[i],total>0?100*s->seconds[i]/total:0);}}
    s->profile=enable;
}
static double ff_value(const uint8_t *p){float f[2];memcpy(f,p,8);return (double)f[0]+(double)f[1];}
/* PSIONIC_DEVICE_QUALIFY: every launch of layers 0 and 1 (a windowed and a
   full-attention layer) and of the readout after the last norm is repeated on
   the CPU by the same Coax kernels compiled to C, from the GPU's own inputs.
   f64 views hold float-float pairs on the GPU; the CPU copy widens them to
   binary64 before its run and compares its binary64 results with the pairs'
   sums. Every other byte of every touched workspace buffer is compared as f32
   words (integers compare exactly that way too). */
void pg_qualify(PgSession *s,int enable){
    s->qualify=enable;s->checked=s->exact=0;s->worst=s->worst_wide=0;s->worst_kind=s->worst_wide_kind=-1;
    if(!enable){if(s->shadow){for(int i=0;i<16+gd_buffer_count();i++)free(s->shadow[i]);free(s->shadow);s->shadow=NULL;}return;}
}
void pg_qualify_report(PgSession *s,int64_t *checked,int64_t *exact,double *worst,int *worst_kind,double *worst_wide,int *worst_wide_kind){*checked=s->checked;*exact=s->exact;*worst=s->worst;*worst_kind=(int)s->worst_kind;*worst_wide=s->worst_wide;*worst_wide_kind=(int)s->worst_wide_kind;}
static double now_seconds(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static PcStatus pg_execute_one(PgSession *s,int64_t i){
    coax_t_GDOp op=s->ops[i];coax_t_GDArg *a=s->args+op.first;
    if(op.kind==3 || op.kind==25)s->norms++;
    int64_t layer=(s->norms-1)/2;int last=s->norms==2*s->shape.layers+1;
    int repeat=s->qualify && (layer==0 || layer==1 || last);
    int touched[64]={0};
    if(repeat){
        if(!s->shadow){s->shadow=calloc((size_t)(16+gd_buffer_count()),sizeof *s->shadow);if(!s->shadow)return PC_MEMORY;}
        for(int64_t j=0;j<op.count;j++)if(a[j].view && a[j].ref.buffer>=16)touched[a[j].ref.buffer]=1;
        for(int b=16;b<16+gd_buffer_count();b++)if(touched[b]){size_t n=pg_slot_bytes(s,b-16);free(s->shadow[b]);s->shadow[b]=malloc(n?n:1);if(!s->shadow[b])return PC_MEMORY;memcpy(s->shadow[b],pg_bytes(s,b),n);}
        for(int64_t j=0;j<op.count;j++)if(a[j].view && pg_cpu_f64((int)op.kind,(int)j)){uint8_t *p=s->shadow[a[j].ref.buffer]+a[j].ref.offset;for(int64_t w=0;w<a[j].ref.count;w++){double v=ff_value(p+8*w);memcpy(p+8*w,&v,8);}}
    }
    PmBatch batch;PmStatus m=pm_begin(&batch);if(m)return pg_status(s,m);PcStatus code=pg_encode(s,batch,i);double gpu=0;m=pm_wait(batch,&gpu);if(code){s->poisoned=1;return code;}code=pg_status(s,m);if(code)return code;
    if(s->profile){s->seconds[op.kind]+=gpu;s->launches[op.kind]++;}
    if(!repeat)return PC_OK;
    double t0=now_seconds();for(int64_t item=0;item<op.items;item++)pg_cpu_launch(s,(int)op.kind,item,a);(void)t0;
    int64_t differ=0;double max_rel=0,max_wide=0;
    for(int b=16;b<16+gd_buffer_count();b++){if(!touched[b])continue;size_t n=pg_slot_bytes(s,b-16);const uint8_t *gpu_bytes=pg_bytes(s,b),*cpu_bytes=s->shadow[b];
        /* Mark this launch's f64 regions in this buffer. */
        size_t words=n/8;uint8_t *wide=calloc(words+1,1);if(!wide)return PC_MEMORY;
        for(int64_t j=0;j<op.count;j++)if(a[j].view && a[j].ref.buffer==b && pg_cpu_f64((int)op.kind,(int)j))memset(wide+a[j].ref.offset/8,1,(size_t)a[j].ref.count);
        for(size_t w=0;w<words;w++){
            if(wide[w]){double want,got=ff_value(gpu_bytes+8*w);memcpy(&want,cpu_bytes+8*w,8);if(want==got)continue;differ++;double rel=fabs(want-got)/(fabs(want)>1e-300?fabs(want):1e-300);if(rel>max_wide)max_wide=rel;continue;}
            for(int h=0;h<2;h++){if(!memcmp(cpu_bytes+8*w+4*h,gpu_bytes+8*w+4*h,4))continue;float fu,fv;memcpy(&fu,cpu_bytes+8*w+4*h,4);memcpy(&fv,gpu_bytes+8*w+4*h,4);differ++;double rel=fabs((double)fu-(double)fv)/(fabs((double)fu)>1e-30?fabs((double)fu):1e-30);if(rel>max_rel)max_rel=rel;}
        }
        for(size_t t=words*8;t<n;t++)if(cpu_bytes[t]!=gpu_bytes[t]){differ++;if(max_rel<1)max_rel=1;}
        free(wide);
    }
    s->checked++;if(!differ)s->exact++;
    if(max_rel>s->worst){s->worst=max_rel;s->worst_kind=op.kind;}if(max_wide>s->worst_wide){s->worst_wide=max_wide;s->worst_wide_kind=op.kind;}
    if(differ){coax_string name=gd_kernel_name(op.kind);printf("  qualify op %4lld %-22.*s layer %2lld: %lld values differ, largest relative difference %.3g (f32) %.3g (f64)\n",(long long)i,(int)name.count,name.data,last?-1LL:(long long)layer,(long long)differ,max_rel,max_wide);}
    return PC_OK;
}
#endif
