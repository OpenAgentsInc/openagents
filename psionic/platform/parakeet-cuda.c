/* CUDA effects only. Coax owns all arithmetic, plans and dispatch order:
 * fast.coax's pkf_encode and device-tdt.coax's pkg_decode issue every launch
 * through pkq_launch, the fixed ABI parakeet-metal.c also binds (kinds 0..12;
 * 13..17 are the CUDA decoder's), and cuda.coax holds the kernels. */
#define _POSIX_C_SOURCE 200809L
#include "parakeet-device-tdt.h"
#include "parakeet-cuda.h"
#include "cuda-host.h"
#include "parakeet-cuda-source.h"
#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define KINDS 18
#define SLOTS 7
static pthread_mutex_t mu=PTHREAD_MUTEX_INITIALIZER;
static PcDeviceInfo device;
static PcModule mod;
static PcStream stream;
static int ready;
static PcKernel kernels[KINDS];
/* Kind IDs are fast.coax's and device-tdt.coax's private ABI; kind 1 is the
 * CUDA f32 product. */
static const char *names[KINDS]={"pkd_feature","pkc_linear","wd_norm","pkd_conv","pkd_relu","pkd_gather","pkd_silu","pkd_add","pkd_glu","pkd_dwbn","pkc_attend","pkd_scale","pkd_copy",
 "pkc_rows","pkc_joint_input","pkc_cell","pkc_pick","pkc_zero"};
/* Slot i maps the host view base[i] ..< base[i]+count[i] that Coax slices
 * onto the device buffer dev[i]. Encoder: 0 weights, 1 features, 2 work,
 * 3 output. Decoder: 4 weights, 5 encoded frames, 6 work. Only the running
 * call's slots have a base. */
static const float *base[SLOTS];
static int64_t count[SLOTS];
static PcBuffer dev[SLOTS];
static size_t capacity[SLOTS];
static float *mirror;            /* host side of the work arena */
static size_t mirror_count;
static int64_t pos_at,pos_end;   /* host-written relative-position table */
static int pos_pending;
static const float *loaded,*loaded_decoder;
static int64_t loaded_count,loaded_decoder_count;
static int encoding;
static double seconds,decode_seconds,compile_seconds,upload_seconds;
static char error_text[512];

static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (double)t.tv_sec+1e-9*(double)t.tv_nsec;}
/* memset, kept by an empty asm that may read *p (explicit_bzero's method). */
static void wipe(void *p,size_t n){if(!p||!n)return;memset(p,0,n);__asm__ __volatile__("" : : "r"(p) : "memory");}
static int fail(const char *operation){snprintf(error_text,sizeof error_text,"%s: %s",operation,pc_log());return -1;}
/* The engine may be called from any thread: make the primary context current
 * on this one (pc_open only does that once the device is open). */
static int bind_locked(void){return ready&&pc_open(NULL)==PC_OK;}
static void clear_locked(void){
 if(ready&&bind_locked())pc_sync_stream(stream);
 for(int i=0;i<SLOTS;i++){if(dev[i])pc_free(dev[i]);dev[i]=0;capacity[i]=0;base[i]=0;count[i]=0;}
 if(mod)pc_unload(mod);mod=0;for(int i=0;i<KINDS;i++)kernels[i]=0;
 if(stream)pc_destroy_stream(stream);stream=0;
 if(mirror){wipe(mirror,mirror_count*sizeof *mirror);free(mirror);}mirror=0;mirror_count=0;
 ready=0;loaded=0;loaded_count=0;loaded_decoder=0;loaded_decoder_count=0;
}
static int open_locked(void){
 if(ready){if(bind_locked())return 1;return fail("bind CUDA context");}
 clear_locked();
 PcStatus s=pc_open(&device);
 if(s==PC_ABSENT){snprintf(error_text,sizeof error_text,"no CUDA driver or device: %s",pc_log());return -1;}
 if(s!=PC_OK)return fail("open CUDA");
 if(pc_stream(&stream)!=PC_OK)return fail("create CUDA stream");
 s=pc_compile((const char *)parakeet_cuda_source,parakeet_cuda_source_len,"parakeet.cu",&mod,&compile_seconds);
 if(s==PC_NO_NVRTC){snprintf(error_text,sizeof error_text,"no NVRTC: %s",pc_log());clear_locked();return -1;}
 if(s!=PC_OK){fail("compile Parakeet kernels");clear_locked();return -1;}
 for(int i=0;i<KINDS;i++)if(pc_kernel(mod,names[i],&kernels[i])!=PC_OK){fail("bind Parakeet kernel");clear_locked();return -1;}
 ready=1;return 1;
}
static int reserve(int slot,int64_t n){
 if(n<1||(uint64_t)n>SIZE_MAX/sizeof(float)){snprintf(error_text,sizeof error_text,"invalid GPU buffer extent");return -1;}
 size_t bytes=(size_t)n*sizeof(float);
 if(dev[slot]&&capacity[slot]>=bytes)return 1;
 if(dev[slot]){pc_free(dev[slot]);dev[slot]=0;capacity[slot]=0;if(slot==0){loaded=0;loaded_count=0;}if(slot==4){loaded_decoder=0;loaded_decoder_count=0;}}
 if(pc_alloc(bytes,&dev[slot])!=PC_OK)return fail("allocate Parakeet device buffer");
 capacity[slot]=bytes;return 1;
}
static int upload_locked(int slot,const float *w,int64_t n);
static PcArg integer(int64_t value){PcArg a={0};a.kind=PC_ARG_INT;a.i=value;return a;}
static PcArg real(float value){PcArg a={0};a.kind=PC_ARG_F32;a.f=value;return a;}
static int view(coax_v_f32 value,PcArg *arg){
 uintptr_t at=(uintptr_t)value.data;
 if(value.count<0)return 0;
 for(int i=0;i<SLOTS;i++){
  uintptr_t b=(uintptr_t)base[i];
  if(base[i]&&at>=b&&(at-b)%sizeof(float)==0){
   int64_t offset=(int64_t)((at-b)/sizeof(float));
   if(offset<=count[i]&&value.count<=count[i]-offset){*arg=(PcArg){.kind=PC_ARG_VIEW,.data=dev[i]+(PcBuffer)offset*sizeof(float),.count=value.count};return 1;}
  }
 }
 snprintf(error_text,sizeof error_text,"Coax launch supplied a view outside its GPU buffers");return 0;
}
/* This is a fixed binding ABI, not a scheduler. Every count, dimension,
 * view range, scale and launch sequence is supplied by fast.coax. */
_Bool pkq_launch(int64_t kind,int64_t items,int64_t n0,int64_t n1,int64_t n2,int64_t n3,int64_t n4,int64_t n5,int64_t n6,int64_t n7,float scale,coax_v_f32 a,coax_v_f32 b,coax_v_f32 c,coax_v_f32 d,coax_v_f32 e,coax_v_f32 f){
 PcArg args[16];int n=0,ni=0,nv=0,group=0;int64_t ns[8]={n0,n1,n2,n3,n4,n5,n6,n7};coax_v_f32 vs[6]={a,b,c,d,e,f};
 if(kind<0||kind>=KINDS||!encoding||items<1)return 0;
 /* pkf_encode writes the position table on the host before its first launch. */
 if(pos_pending){if(pc_upload(dev[2]+(PcBuffer)pos_at*sizeof(float),mirror+pos_at,(size_t)(pos_end-pos_at)*sizeof(float),stream)!=PC_OK){fail("upload Parakeet positions");return 0;}pos_pending=0;}
 switch(kind){
  case 0:ni=2;nv=2;break;
  case 1:ni=4;nv=4;group=256;if(n3==0)vs[2]=d;break;
  case 2:ni=1;nv=4;group=128;break;
  case 3:ni=8;nv=4;break;
  case 4:case 6:nv=1;break;
  case 5:ni=3;nv=2;break;
  case 7:nv=2;args[n++]=real(scale);break;
  case 8:ni=1;nv=2;break;
  case 9:ni=3;nv=5;break;
  case 10:ni=5;nv=6;group=128;break;
  case 11:nv=1;args[n++]=real(scale);break;
  case 12:nv=2;break;
  case 13:ni=3;nv=5;group=128;break;
  case 14:nv=3;break;
  case 15:ni=1;nv=4;break;
  case 16:ni=2;nv=2;group=256;break;
  case 17:nv=1;break;
 }
 for(int i=0;i<ni;i++)args[n++]=integer(ns[i]);
 for(int i=0;i<nv;i++)if(!view(vs[i],&args[n++]))return 0;
 /* A group kernel is one block of `group` threads per item. */
 PcStatus s=group?pc_launch_warps(kernels[kind],items*(group/32),group,args,n,stream):pc_launch(kernels[kind],items,256,args,n,stream);
 if(s!=PC_OK){fail("launch Parakeet kernel");return 0;}
 return 1;
}
static int encode_locked(coax_t_CfShape shape,const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *out,int64_t no,int64_t prefix){
 coax_t_PkfPlan p=pkf_plan(shape,frames);
 if(!p.ok||!pkf_admit(shape,frames,ne,nx,p.end,no)||!ew||!x||!out){snprintf(error_text,sizeof error_text,"Parakeet encoder shape/capacity refusal");return -1;}
 if(open_locked()<1||reserve(0,ne)<1||reserve(1,nx)<1||reserve(2,p.end)<1||reserve(3,p.output)<1)return -1;
 if((size_t)p.end>mirror_count){
  if(mirror){wipe(mirror,mirror_count*sizeof *mirror);free(mirror);}
  mirror=calloc((size_t)p.end,sizeof *mirror);mirror_count=mirror?(size_t)p.end:0;
  if(!mirror){snprintf(error_text,sizeof error_text,"out of host memory");return -1;}
 }
 if(upload_locked(0,ew,ne)<1)return -1;
 double start=now();
 if(pc_upload(dev[1],x,(size_t)nx*sizeof(float),stream)!=PC_OK)return fail("upload Parakeet features");
 base[0]=ew;count[0]=ne;base[1]=x;count[1]=nx;base[2]=mirror;count[2]=p.end;base[3]=out;count[3]=p.output;
 pos_at=p.p;pos_end=p.rp;pos_pending=1;encoding=1;
 _Bool submitted=pkf_encode(shape,(coax_v_f32){(float *)ew,ne},(coax_v_f32){(float *)x,nx},frames,(coax_v_f32){mirror,p.end},(coax_v_f32){out,p.output},prefix);
 encoding=0;pos_pending=0;
 PcStatus status=pc_sync_stream(stream);
 if(!submitted||status!=PC_OK){for(int i=0;i<4;i++){base[i]=0;count[i]=0;}}
 if(!submitted){if(!error_text[0])snprintf(error_text,sizeof error_text,"Parakeet encoder schedule refused");return -1;}
 if(status!=PC_OK)return fail("execute Parakeet encoder");
 if(pc_download(out,dev[3],(size_t)p.output*sizeof(float),stream)!=PC_OK||pc_sync_stream(stream)!=PC_OK)return fail("download Parakeet encoded frames");
 seconds=now()-start;
 for(int i=0;i<4;i++){base[i]=0;count[i]=0;}
 if(!pkf_output_valid(shape,frames,(coax_v_f32){out,p.output})){snprintf(error_text,sizeof error_text,"Parakeet GPU encoded values refused");return -1;}
 return 1;
}
/* Wait for the stream, then copy the device's values of v back into v. */
_Bool pkq_read(coax_v_f32 v){
 PcArg a;if(!encoding||!view(v,&a))return 0;
 if(pc_download(v.data,a.data,(size_t)v.count*sizeof(float),stream)!=PC_OK||pc_sync_stream(stream)!=PC_OK){fail("read Parakeet decision");return 0;}
 return 1;
}
static int decode_locked(const coax_t_TdtShape *shape,const float *dw,int64_t nd,const float *enc,int64_t ne,int64_t frames,float *work,int64_t nw,int64_t *tokens,int64_t nt,int64_t *at,int64_t na,int64_t *result,int64_t nr){
 if(!dw||!enc||!work||!tokens||!at||!result||nr<2||nd<1||ne<1||nw<1){snprintf(error_text,sizeof error_text,"Parakeet decoder capacity refusal");return -1;}
 if(open_locked()<1||upload_locked(4,dw,nd)<1||reserve(5,ne)<1||reserve(6,nw)<1)return -1;
 double start=now();
 if(pc_upload(dev[5],enc,(size_t)ne*sizeof(float),stream)!=PC_OK)return fail("upload Parakeet encoded frames");
 base[4]=dw;count[4]=nd;base[5]=enc;count[5]=ne;base[6]=work;count[6]=nw;encoding=1;
 coax_t_TdtResult r=shape?pkg_decode(*shape,(coax_v_f32){(float *)dw,nd},(coax_v_f32){(float *)enc,ne},frames,(coax_v_f32){work,nw},(coax_v_s64){tokens,nt},(coax_v_s64){at,na}):pkg_decode_parakeet((coax_v_f32){(float *)dw,nd},(coax_v_f32){(float *)enc,ne},frames,(coax_v_f32){work,nw},(coax_v_s64){tokens,nt},(coax_v_s64){at,na});
 encoding=0;PcStatus status=pc_sync_stream(stream);
 for(int i=4;i<SLOTS;i++){base[i]=0;count[i]=0;}
 decode_seconds=now()-start;
 if(status!=PC_OK)return fail("execute Parakeet decoder");
 /* A refusal before or between launches is the decoder's status, not a
  * device failure, unless a launch or read itself failed. */
 if(r.status!=COAX_TdtStatus_ok&&error_text[0])return -1;
 result[0]=(int64_t)r.status;result[1]=r.written;return 1;
}
int pk_cuda_decode(const float *dw,int64_t nd,const float *enc,int64_t ne,int64_t frames,float *work,int64_t nw,int64_t *tokens,int64_t nt,int64_t *at,int64_t na,int64_t *result,int64_t nr){
 pthread_mutex_lock(&mu);error_text[0]=0;int r=decode_locked(NULL,dw,nd,enc,ne,frames,work,nw,tokens,nt,at,na,result,nr);pthread_mutex_unlock(&mu);return r;
}
int pk_cuda_decode_shape(const int64_t s[15],const float *dw,int64_t nd,const float *enc,int64_t ne,int64_t frames,float *work,int64_t nw,int64_t *tokens,int64_t nt,int64_t *at,int64_t na,int64_t *result,int64_t nr){
 coax_t_TdtShape shape={.vocab=s[0],.enc=s[1],.pred=s[2],.layers=s[3],.joint=s[4],.durations=s[5],.max_symbols=s[14]};
 for(int i=0;i<8;i++)shape.dur.e[i]=s[6+i];
 /* Verification callers may reuse a freed weight address for new values:
  * always upload. The resident engine's weights never change while open. */
 pthread_mutex_lock(&mu);error_text[0]=0;loaded_decoder=0;loaded_decoder_count=0;int r=decode_locked(&shape,dw,nd,enc,ne,frames,work,nw,tokens,nt,at,na,result,nr);pthread_mutex_unlock(&mu);return r;
}
double pk_cuda_decode_seconds(void){return decode_seconds;}
static int upload_locked(int slot,const float *w,int64_t n){
 if(reserve(slot,n)<1)return -1;
 const float **mark=slot==0?&loaded:&loaded_decoder;int64_t *marked=slot==0?&loaded_count:&loaded_decoder_count;
 if(*mark==w&&*marked==n)return 1;
 double t=now();
 if(pc_upload(dev[slot],w,(size_t)n*sizeof(float),stream)!=PC_OK||pc_sync_stream(stream)!=PC_OK)return fail("upload Parakeet weights");
 upload_seconds+=now()-t;*mark=w;*marked=n;return 1;
}
int pk_cuda_preload(const float *ew,int64_t ne,const float *dw,int64_t nd){
 pthread_mutex_lock(&mu);error_text[0]=0;int r=open_locked()==1&&upload_locked(0,ew,ne)==1&&upload_locked(4,dw,nd)==1;pthread_mutex_unlock(&mu);return r;
}
int pk_cuda_available(void){pthread_mutex_lock(&mu);int result=open_locked();pthread_mutex_unlock(&mu);return result==1;}
int pk_cuda_encode(const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *work,int64_t nw,float *out,int64_t no){
 (void)work;(void)nw;pthread_mutex_lock(&mu);error_text[0]=0;coax_t_CfShape shape=pkf_shape();int result=encode_locked(shape,ew,ne,x,nx,frames,out,no,shape.layers);pthread_mutex_unlock(&mu);return result;
}
int pk_cuda_encode_shape(const int64_t s[9],const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *out,int64_t no,int64_t prefix){
 coax_t_CfShape shape={.mels=s[0],.channels=s[1],.dim=s[2],.heads=s[3],.hidden=s[4],.kernel=s[5],.layers=s[6],.vocab=s[7],.xscale=s[8]!=0};
 pthread_mutex_lock(&mu);error_text[0]=0;loaded=0;loaded_count=0;int result=encode_locked(shape,ew,ne,x,nx,frames,out,no,prefix);pthread_mutex_unlock(&mu);return result;
}
const char *pk_cuda_error(void){return error_text;}
double pk_cuda_seconds(void){return seconds;}
double pk_cuda_compile_seconds(void){return compile_seconds;}
double pk_cuda_upload_seconds(void){return upload_seconds;}
const char *pk_cuda_device(void){return ready?device.name:"";}
void pk_cuda_clear_work(void){pthread_mutex_lock(&mu);
 /* encode_locked holds this lock through its final stream wait, so no
  * device command can still be using these arenas. */
 if(bind_locked()){for(int i=1;i<SLOTS;i++)if(i!=4&&dev[i]&&capacity[i])pc_fill(dev[i],0,capacity[i]/sizeof(float),stream);pc_sync_stream(stream);}
 if(mirror)wipe(mirror,mirror_count*sizeof *mirror);
 for(int i=1;i<SLOTS;i++){base[i]=0;count[i]=0;}
 seconds=0;decode_seconds=0;wipe(error_text,sizeof error_text);pthread_mutex_unlock(&mu);
}
void pk_cuda_close(void){pthread_mutex_lock(&mu);clear_locked();pthread_mutex_unlock(&mu);}
