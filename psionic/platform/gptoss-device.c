/* Device effects executing a Coax-authored program. No tensor arithmetic,
   routing, shape policy, cache indexing or token selection is written here. */
#include "gptoss-device.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct PgSession {
    coax_t_GShape shape;coax_t_GDPlan plan;coax_t_GDParam *params;int64_t param_count;
    coax_t_GDOp *ops;coax_t_GDArg *args;PcBuffer *buffers;PcModule module;PcKernel *kernels;
    PcStream stream;int cpu,poisoned;char error[256];
#ifdef PSIONIC_DEVICE_TEST
    int profile;double *seconds;int *launches;
#endif
};
static void *pg_address(PgSession *s,coax_t_GDRef r){return (void*)(uintptr_t)(s->buffers[r.buffer]+(uint64_t)r.offset);}
#ifdef PSIONIC_DEVICE_TEST
#include "gptoss-device-cpu.h"
#endif
static PcStatus pg_allocate(PgSession *s,int id,size_t bytes){
#ifdef PSIONIC_DEVICE_TEST
    if(s->cpu){void *p=calloc(bytes?bytes:1,1);if(!p)return PC_MEMORY;s->buffers[id]=(PcBuffer)(uintptr_t)p;return PC_OK;}
#endif
    return pc_alloc(bytes,&s->buffers[id]);
}
static PcStatus pg_upload(PgSession *s,PcBuffer dst,const void *src,size_t bytes){
#ifdef PSIONIC_DEVICE_TEST
    if(s->cpu){memcpy((void*)(uintptr_t)dst,src,bytes);return PC_OK;}
#endif
    return pc_upload(dst,src,bytes,s->stream);
}
static PcStatus pg_create(coax_t_GProfile profile,coax_v_t_GFile files,const char *source,size_t source_bytes,size_t budget,int cpu,PgSession **out,coax_t_GResult *why){
    *out=NULL;coax_t_GBlock blocks[36],draft[36];coax_t_GModel model;
    *why=g_bind_profile(profile,files,(coax_v_t_GBlock){blocks,36},(coax_v_t_GBlock){draft,36},(coax_v_t_GModel){&model,1},0);if(why->status)return PC_OK;
    PgSession *s=calloc(1,sizeof *s);if(!s)return PC_MEMORY;s->cpu=cpu;s->shape=model.shape;PcStatus code=PC_OK;size_t available=budget,total=budget;
    if(!cpu){PcDeviceInfo info;code=pc_open(&info);if(code)goto failed;double seconds;code=pc_compile(source,source_bytes,"gptoss.cu",&s->module,&seconds);if(code)goto failed;code=pc_memory(&available,&total);if(code)goto failed;code=pc_stream(&s->stream);if(code)goto failed;}
    s->plan=gd_plan(model.shape,profile.pins,(int64_t)available,(int64_t)budget);if(s->plan.status){why->status=s->plan.status;goto refused;}
    s->buffers=calloc((size_t)(16+gd_buffer_count()),sizeof *s->buffers);
    s->param_count=profile.specs.count;s->params=calloc((size_t)s->param_count,sizeof *s->params);s->ops=calloc((size_t)s->plan.max_ops,sizeof *s->ops);s->args=calloc((size_t)s->plan.max_args,sizeof *s->args);s->kernels=calloc((size_t)gd_kernel_count(),sizeof *s->kernels);
#ifdef PSIONIC_DEVICE_TEST
    s->seconds=calloc((size_t)gd_kernel_count(),sizeof *s->seconds);s->launches=calloc((size_t)gd_kernel_count(),sizeof *s->launches);if(!s->seconds || !s->launches){code=PC_MEMORY;goto failed;}
#endif
    if(!s->buffers || !s->params || !s->ops || !s->args || !s->kernels){code=PC_MEMORY;goto failed;}
    *why=gd_params(profile,(coax_v_t_GDParam){s->params,s->param_count});if(why->status)goto refused;
    if(!gd_packed_fits(s->plan,why->bytes)){why->status=COAX_GStatus_capacity;goto refused;}
    if(!cpu)for(int64_t i=0;i<gd_kernel_count();i++){coax_string name=gd_kernel_name(i);char label[128];if(name.count<1 || name.count>=127){code=PC_ARGUMENT;goto failed;}memcpy(label,name.data,(size_t)name.count);label[name.count]=0;code=pc_kernel(s->module,label,&s->kernels[i]);if(code)goto failed;}
    code=pg_allocate(s,0,(size_t)s->plan.upload.e[0]);if(code)goto failed;
    for(int64_t i=0;i<s->param_count;i++){coax_t_GDParam p=s->params[i];code=pg_upload(s,s->buffers[0]+(uint64_t)p.device,files.data[p.file].data.data+p.begin,(size_t)p.bytes);if(code)goto failed;}
    for(int i=0;i<gd_buffer_count();i++){code=pg_allocate(s,16+i,(size_t)(s->plan.counts.e[i]*s->plan.width.e[i]));if(code)goto failed;}
    size_t rope_bytes=(size_t)s->plan.counts.e[25]*4;float *rope=malloc(rope_bytes);if(!rope){code=PC_MEMORY;goto failed;}
    *why=gd_rope(s->shape.capacity,(coax_v_f32){rope,s->plan.counts.e[25]});if(why->status){free(rope);goto refused;}code=pg_upload(s,s->buffers[41],rope,rope_bytes);if(!code)code=pg_wait(s);free(rope);if(code)goto failed;
    code=pg_reset(s);if(code)goto failed;why->status=COAX_GStatus_ok;*out=s;return PC_OK;
refused:pg_close(s);return PC_OK;
failed:pg_close(s);return code;
}
PcStatus pg_open(coax_t_GProfile p,coax_v_t_GFile f,const char *source,size_t bytes,size_t budget,PgSession **out,coax_t_GResult *why){PcDeviceInfo info;PcStatus code=pc_open(&info);if(code){*out=NULL;return code;}return pg_create(p,f,source,bytes,budget,0,out,why);}
#ifdef PSIONIC_DEVICE_TEST
PcStatus pg_open_cpu(coax_t_GProfile p,coax_v_t_GFile f,PgSession **out,coax_t_GResult *why){return pg_create(p,f,NULL,0,1073741824,1,out,why);}
#endif
PcStatus pg_reset(PgSession *s){
    if(s->poisoned)return PC_DRIVER;
    /* Immutable coefficients and the rotary table survive request resets. */
    for(int i=0;i<gd_buffer_count();i++){if(gd_preserve(i))continue;size_t bytes=(size_t)(s->plan.counts.e[i]*s->plan.width.e[i]);
#ifdef PSIONIC_DEVICE_TEST
        if(s->cpu){memset((void*)(uintptr_t)s->buffers[16+i],0,bytes);continue;}
#endif
        PcStatus code=pc_fill(s->buffers[16+i],0,bytes/4,s->stream);if(code)return code;
    }return pg_wait(s);
}
static PcStatus pg_execute(PgSession *s,coax_t_GDProgram p){
    for(int64_t i=0;i<p.ops;i++){coax_t_GDOp op=s->ops[i];coax_t_GDArg *a=s->args+op.first;
#ifdef PSIONIC_DEVICE_TEST
        if(s->cpu){for(int64_t item=0;item<op.items;item++)pg_cpu_launch(s,(int)op.kind,item,a);continue;}
#endif
        PcArg arguments[24];for(int64_t j=0;j<op.count;j++){arguments[j]=(PcArg){.kind=a[j].view?PC_ARG_VIEW:PC_ARG_INT,.i=a[j].value,.data=a[j].view?(PcBuffer)(uintptr_t)pg_address(s,a[j].ref):0,.count=a[j].ref.count};}
#ifdef PSIONIC_DEVICE_TEST
        PcEvent begin=NULL,end=NULL;if(s->profile){PcStatus z=pc_event(&begin);if(z)return z;z=pc_event(&end);if(z)return z;z=pc_record(begin,s->stream);if(z)return z;}
#endif
        PcStatus code=pc_launch(s->kernels[op.kind],op.items,256,arguments,(int)op.count,s->stream);if(code){s->poisoned=1;return code;}
#ifdef PSIONIC_DEVICE_TEST
        if(s->profile){double elapsed;code=pc_record(end,s->stream);if(!code)code=pc_sync_stream(s->stream);if(!code)code=pc_event_seconds(begin,end,&elapsed);pc_destroy_event(begin);pc_destroy_event(end);if(code)return code;s->seconds[op.kind]+=elapsed;s->launches[op.kind]++;}
#endif
    }
    return PC_OK;
}
PcStatus pg_step(PgSession *s,int64_t token,int64_t position,int choose,int64_t limit,int wait,int64_t control[8]){
    if(s->poisoned)return PC_DRIVER;coax_t_GDProgram p=gd_program(s->shape,s->plan,(coax_v_t_GDParam){s->params,s->param_count},position,token,choose!=0,limit,(coax_v_t_GDOp){s->ops,s->plan.max_ops},(coax_v_t_GDArg){s->args,s->plan.max_args});
    if(p.status){snprintf(s->error,sizeof s->error,"Coax device program refused: %u",p.status);return PC_ARGUMENT;}
    PcStatus code=pg_execute(s,p);if(code)return code;
    if(wait){PcStatus code=pg_wait(s);if(code)return code;return pg_read(s,24,control,8*8);}return PC_OK;
}
PcStatus pg_prefill(PgSession *s,const int64_t *tokens,int64_t count,int64_t position,int choose,int64_t limit,int64_t control[8]){
    if(s->poisoned)return PC_DRIVER;
    coax_t_GResult admitted=gd_prompt(s->shape,position,(coax_v_s64){(int64_t*)tokens,count});
    if(admitted.status){snprintf(s->error,sizeof s->error,"Coax prompt refused: %u",admitted.status);return PC_ARGUMENT;}
    PcStatus code=pg_upload(s,s->buffers[42],tokens,(size_t)count*8);if(code){s->poisoned=1;return code;}
    for(int64_t first=0;first<count;){int64_t n=gd_chunk(first,count,s->plan.batch);
        coax_t_GDProgram p=gd_batch_program(s->shape,s->plan,(coax_v_t_GDParam){s->params,s->param_count},position+first,first,n,choose && first+n==count,limit,(coax_v_t_GDOp){s->ops,s->plan.max_ops},(coax_v_t_GDArg){s->args,s->plan.max_args});
        if(p.status){snprintf(s->error,sizeof s->error,"Coax prompt program refused: %u",p.status);return PC_ARGUMENT;}
        code=pg_execute(s,p);if(code)return code;code=pg_read(s,24,control,64);if(code)return code;
        if(control[0])return PC_OK;first+=n;
    }return PC_OK;
}
PcStatus pg_read(PgSession *s,int slot,void *out,size_t bytes){if(slot<0 || slot>=gd_buffer_count() || bytes>(size_t)(s->plan.counts.e[slot]*s->plan.width.e[slot]))return PC_ARGUMENT;
#ifdef PSIONIC_DEVICE_TEST
    if(s->cpu){memcpy(out,(void*)(uintptr_t)s->buffers[16+slot],bytes);return PC_OK;}
#endif
    PcStatus code=pc_download(out,s->buffers[16+slot],bytes,s->stream);if(code){s->poisoned=1;return code;}return pg_wait(s);
}
#ifdef PSIONIC_DEVICE_TEST
PcStatus pg_patch(PgSession *s,size_t offset,const void *bytes,size_t count){if(offset>(size_t)s->plan.upload.e[0] || count>(size_t)s->plan.upload.e[0]-offset)return PC_ARGUMENT;PcStatus code=pg_upload(s,s->buffers[0]+offset,bytes,count);return code?code:pg_wait(s);}
#endif
PcStatus pg_wait(PgSession *s){
#ifdef PSIONIC_DEVICE_TEST
    if(s->cpu)return PC_OK;
#endif
    PcStatus code=pc_sync_stream(s->stream);if(code)s->poisoned=1;return code;
}
void pg_close(PgSession *s){if(!s)return;
#ifdef PSIONIC_DEVICE_TEST
    if(s->cpu){if(s->buffers)for(int i=0;i<16+gd_buffer_count();i++)free((void*)(uintptr_t)s->buffers[i]);}else
#endif
    {if(s->stream)pc_sync_stream(s->stream);if(s->buffers)for(int i=0;i<16+gd_buffer_count();i++)if(s->buffers[i])pc_free(s->buffers[i]);if(s->module)pc_unload(s->module);if(s->stream)pc_destroy_stream(s->stream);}
#ifdef PSIONIC_DEVICE_TEST
    free(s->seconds);free(s->launches);
#endif
    free(s->buffers);free(s->params);free(s->ops);free(s->args);free(s->kernels);free(s);
}
coax_t_GDPlan pg_plan(const PgSession *s){return s->plan;}
coax_t_GShape pg_shape(const PgSession *s){return s->shape;}
const char *pg_error(const PgSession *s){return s->error;}

#ifdef PSIONIC_DEVICE_TEST
void pg_profile(PgSession *s,int enable){
    if(enable){memset(s->seconds,0,(size_t)gd_kernel_count()*sizeof *s->seconds);memset(s->launches,0,(size_t)gd_kernel_count()*sizeof *s->launches);}
    else {for(int64_t i=0;i<gd_kernel_count();i++)if(s->launches[i]){coax_string name=gd_kernel_name(i);printf("kernel %.*s launches=%d device_seconds=%.9f\n",(int)name.count,name.data,s->launches[i],s->seconds[i]);}}
    s->profile=enable;
}
#endif
