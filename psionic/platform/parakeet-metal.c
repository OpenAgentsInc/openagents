/* Metal effects only. Coax owns all arithmetic, plans and dispatch order. */
#include "parakeet-fast.h"
#include "parakeet-metal.h"
#include "metal-host.h"
#include "parakeet-metal-source.h"
#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static pthread_mutex_t mu=PTHREAD_MUTEX_INITIALIZER;
static PmModule mod;
static int ready;
static PmKernel kernels[13];
static const char *names[13]={"pkd_feature","wd_linear","wd_norm","pkd_conv","pkd_relu","pkd_gather","pkd_silu","pkd_add","pkd_glu","pkd_dwbn","pkd_attend","pkd_scale","pkd_copy"};
static PmBuffer bufs[4];
static float *bases[4];
static size_t sizes[4];
static const float *loaded;
static int64_t loaded_count;
static PmBatch batch;
static double seconds;
static char error_text[512];
static int fail(const char *operation) {snprintf(error_text,sizeof error_text,"%s: %s",operation,pm_log());return -1;}
static void clear_locked(void){
 for(int i=0;i<4;i++){if(bufs[i])pm_free(bufs[i]);bufs[i]=0;bases[i]=0;sizes[i]=0;}
 for(int i=0;i<13;i++){if(kernels[i])pm_kernel_free(kernels[i]);kernels[i]=0;}
 if(mod)pm_unload(mod);mod=0;ready=0;loaded=0;loaded_count=0;
}
static int open_locked(void) {
 if(ready)return 1;
 if(mod)clear_locked();
 PmDeviceInfo info;double time;
 if(pm_open(&info)!=PM_OK)return fail("open Metal");
 if(pm_compile((const char *)parakeet_metal_source,parakeet_metal_source_len,"parakeet",&mod,&time)!=PM_OK)return fail("compile Parakeet kernels");
 for(int i=0;i<13;i++)if(pm_kernel(mod,names[i],&kernels[i])!=PM_OK){fail("bind Parakeet kernel");clear_locked();return -1;}
 ready=1;return 1;
}
static int reserve(int slot,int64_t count) {
 if(count<1 || (uint64_t)count>SIZE_MAX/sizeof(float)){snprintf(error_text,sizeof error_text,"invalid GPU buffer extent");return -1;}
 size_t bytes=(size_t)count*sizeof(float);
 if(bufs[slot] && sizes[slot]>=bytes)return 1;
 if(bufs[slot]){pm_free(bufs[slot]);bufs[slot]=0;bases[slot]=0;sizes[slot]=0;}
 if(pm_alloc_unset(bytes,&bufs[slot])!=PM_OK)return fail("allocate Parakeet buffer");
 bases[slot]=pm_contents(bufs[slot]);sizes[slot]=bytes;return 1;
}
static PmArg integer(int64_t value){PmArg a={0};a.kind=PM_ARG_INT;a.i=value;return a;}
static PmArg real(float value){PmArg a={0};a.kind=PM_ARG_F32;a.f=value;return a;}
static int view(coax_v_f32 value,PmArg *arg) {
 uintptr_t at=(uintptr_t)value.data;
 if(value.count<0 || (uint64_t)value.count>SIZE_MAX/sizeof(float))return 0;
 size_t bytes=(size_t)value.count*sizeof(float);
 for(int i=0;i<4;i++){
  uintptr_t base=(uintptr_t)bases[i];
  if(bufs[i] && at>=base && at-base<=sizes[i] && bytes<=sizes[i]-(at-base)){
   *arg=(PmArg){.kind=PM_ARG_VIEW,.buffer=bufs[i],.offset=(int64_t)(at-base),.count=value.count};return 1;
  }
 }
 snprintf(error_text,sizeof error_text,"Coax launch supplied a view outside its GPU buffers");return 0;
}
/* This is a fixed binding ABI, not a scheduler. Every count, dimension,
 * view range, scale and launch sequence is supplied by fast.coax. */
_Bool pkq_launch(int64_t kind,int64_t items,int64_t n0,int64_t n1,int64_t n2,int64_t n3,int64_t n4,int64_t n5,int64_t n6,int64_t n7,float scale,coax_v_f32 a,coax_v_f32 b,coax_v_f32 c,coax_v_f32 d,coax_v_f32 e,coax_v_f32 f) {
 PmArg args[16];int count=0,lanes=1,block=256;int64_t ns[8]={n0,n1,n2,n3,n4,n5,n6,n7};coax_v_f32 vs[6]={a,b,c,d,e,f};
 if(kind<0 || kind>=13 || !batch)return 0;
 int ni=0,nv=0;
 switch(kind){
  case 0:ni=2;nv=2;break;
  case 1:ni=3;nv=4;lanes=128;block=128;break;
  case 2:ni=1;nv=4;lanes=128;block=128;break;
  case 3:ni=8;nv=4;break;
  case 4:case 6:nv=1;break;
  case 5:ni=3;nv=2;break;
  case 7:nv=2;args[count++]=real(scale);break;
  case 8:ni=1;nv=2;break;
  case 9:ni=3;nv=5;break;
  case 10:ni=5;nv=6;break;
  case 11:nv=1;args[count++]=real(scale);break;
  case 12:nv=2;break;
 }
 for(int i=0;i<ni;i++)args[count++]=integer(ns[i]);
 if(kind==1){args[count++]=integer(0);args[count++]=integer(n3);if(n3==0)vs[2]=d;}
 for(int i=0;i<nv;i++)if(!view(vs[i],&args[count++]))return 0;
 if(pm_launch(batch,kernels[kind],items,lanes,block,args,count)!=PM_OK){fail("launch Parakeet kernel");return 0;}
 return 1;
}
static int encode_locked(coax_t_CfShape shape,const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *out,int64_t no,int64_t prefix) {
 coax_t_PkfPlan p=pkf_plan(shape,frames);
 if(!p.ok || !pkf_admit(shape,frames,ne,nx,p.end,no) || !ew || !x || !out){snprintf(error_text,sizeof error_text,"Parakeet encoder shape/capacity refusal");return -1;}
 if(open_locked()<1 || reserve(0,ne)<1 || reserve(1,nx)<1 || reserve(2,p.end)<1 || reserve(3,p.output)<1)return -1;
 if(loaded!=ew || loaded_count!=ne){memcpy(bases[0],ew,(size_t)ne*sizeof(float));loaded=ew;loaded_count=ne;}
 memcpy(bases[1],x,(size_t)nx*sizeof(float));
 if(pm_begin(&batch)!=PM_OK)return fail("begin Parakeet encoder");
 _Bool submitted=pkf_encode(shape,(coax_v_f32){bases[0],ne},(coax_v_f32){bases[1],nx},frames,(coax_v_f32){bases[2],p.end},(coax_v_f32){bases[3],p.output},prefix);
 PmStatus status=pm_wait(batch,&seconds);batch=0;
 if(!submitted || status!=PM_OK)return fail("execute Parakeet encoder");
 if(!pkf_output_valid(shape,frames,(coax_v_f32){bases[3],p.output})){snprintf(error_text,sizeof error_text,"Parakeet GPU encoded values refused");return -1;}
 memcpy(out,bases[3],(size_t)p.output*sizeof(float));return 1;
}
int pk_metal_available(void){pthread_mutex_lock(&mu);int result=open_locked();pthread_mutex_unlock(&mu);return result==1;}
int pk_metal_encode(const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *work,int64_t nw,float *out,int64_t no) {
 (void)work;(void)nw;pthread_mutex_lock(&mu);coax_t_CfShape shape=pkf_shape();int result=encode_locked(shape,ew,ne,x,nx,frames,out,no,shape.layers);pthread_mutex_unlock(&mu);return result;
}
int pk_metal_encode_shape(const int64_t s[9],const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *out,int64_t no,int64_t prefix) {
 coax_t_CfShape shape={.mels=s[0],.channels=s[1],.dim=s[2],.heads=s[3],.hidden=s[4],.kernel=s[5],.layers=s[6],.vocab=s[7],.xscale=s[8]!=0};pthread_mutex_lock(&mu);int result=encode_locked(shape,ew,ne,x,nx,frames,out,no,prefix);pthread_mutex_unlock(&mu);return result;
}
const char *pk_metal_error(void){return error_text;}
double pk_metal_seconds(void){return seconds;}
void pk_metal_close(void){pthread_mutex_lock(&mu);clear_locked();pthread_mutex_unlock(&mu);}
