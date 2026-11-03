/* File/memory/mutex/CUDA effects only. Arithmetic, extents, cache movement,
 * decoder decisions and transcript admission are generated from Coax. */
#define _POSIX_C_SOURCE 200809L
#include "incremental-runtime.h"
#include "incremental-cuda-source.h"
#include "psionic-incremental.h"
#include "cuda-host.h"
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#define IAS_HEADER_BYTES (ias_extents().header)
#define IAS_WEIGHT_COUNT (ias_extents().weights)
#define IAS_WORK (ias_extents().work)
#define IAS_STATE (ias_extents().state)
#define IAS_H (ias_extents().encoded)
#define IAS_PROJ (ias_extents().projected)

struct psionic_incremental {
 pthread_mutex_t lock;
 void *file,*json;size_t file_n,json_n;
 coax_t_IasStream stream_state;
 float *work,*state;double *bank,*window;
 coax_t_JvNode *nodes;uint8_t *vocab_bytes;coax_string pieces[1024];
 PcModule module;PcStream gpu_stream;PcKernel kernels[15];PcBuffer buffers[3];
 const float *base[3];int64_t count[3];
 PcBuffer staged[2];int staging,staged_view;uint64_t resident_words;
 char error[512];
};
static _Thread_local psionic_incremental *active;
static const char *kernel_names[15]={"ias_k_dense","wd_norm","ias_k_sub","ias_k_save_sub","ias_k_point","ias_k_gather","ias_k_glu","ias_k_dw","ias_k_silu","ias_k_relu","ias_k_add","ias_k_copy","ias_k_zero","ias_k_attend","ias_k_cell"};
static coax_v_t_IasStream sv(psionic_incremental *p){return (coax_v_t_IasStream){&p->stream_state,1};}
static coax_v_f32 fv(float *p,int64_t n){return (coax_v_f32){p,n};}
static coax_v_f32 weights(psionic_incremental *p){return fv((float*)((uint8_t*)p->file+IAS_HEADER_BYTES),IAS_WEIGHT_COUNT);}
static void wipe(void *p,size_t n){if(p){memset(p,0,n);__asm__ __volatile__("" : : "r"(p) : "memory");}}
static int failure(psionic_incremental *p,const char *operation){snprintf(p->error,sizeof p->error,"%s: %s",operation,pc_log());return 0;}
static int map_input(const char *dir,const char *name,void **data,size_t *n){
 char path[4096];if(snprintf(path,sizeof path,"%s/%s",dir,name)>=(int)sizeof path)return 0;
 int fd=open(path,O_RDONLY|O_CLOEXEC);if(fd<0)return 0;struct stat st;
 if(fstat(fd,&st)||!S_ISREG(st.st_mode)||st.st_size<=0){close(fd);return 0;}
 *n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)return 0;*data=p;return 1;
}
static int bind_view(psionic_incremental *p,coax_v_f32 x,PcArg *a){
 if(x.count<0)return 0;uintptr_t at=(uintptr_t)x.data;
 for(int i=0;i<3;i++){uintptr_t b=(uintptr_t)p->base[i];if(at>=b&&(at-b)%4==0){uint64_t off=(at-b)/4;if(off<=(uint64_t)p->count[i]&&(uint64_t)x.count<=(uint64_t)p->count[i]-off){
  if(i==0&&p->staging&&off+(uint64_t)x.count>p->resident_words){if(p->staged_view>=2||(uint64_t)x.count>5000000)return 0;PcBuffer target=p->staged[p->staged_view++];if(x.count&&pc_upload(target,x.data,(size_t)x.count*4,p->gpu_stream)!=PC_OK)return failure(p,"stage original weights");*a=(PcArg){.kind=PC_ARG_VIEW,.data=target,.count=x.count};return 1;}
  *a=(PcArg){.kind=PC_ARG_VIEW,.data=p->buffers[i]+off*4,.count=x.count};return 1;}}}
 snprintf(p->error,sizeof p->error,"Coax GPU view outside the owned arenas");return 0;
}
_Bool ias_launch(int64_t kind,int64_t items,int64_t n0,int64_t n1,int64_t n2,int64_t n3,int64_t n4,int64_t n5,float scale,coax_v_f32 a,coax_v_f32 b,coax_v_f32 c,coax_v_f32 d,coax_v_f32 e,coax_v_f32 f){
 psionic_incremental *p=active;if(!p||kind<0||kind>=15||items<1)return 0;p->staged_view=0;
 static const int ints[15]={4,1,6,2,4,3,1,1,0,0,0,0,0,3,1};
 static const int views[15]={4,4,5,2,4,2,2,3,1,1,2,2,1,6,3};
 int64_t ns[6]={n0,n1,n2,n3,n4,n5};coax_v_f32 vs[6]={a,b,c,d,e,f};PcArg args[13];int n=0;
 for(int i=0;i<ints[kind];i++)args[n++]=(PcArg){.kind=PC_ARG_INT,.i=ns[i]};
 if(kind==10)args[n++]=(PcArg){.kind=PC_ARG_F32,.f=scale};
 for(int i=0;i<views[kind];i++)if(!bind_view(p,vs[i],&args[n++]))return 0;
 PcStatus s=(kind==0||kind==1)?pc_launch_warps(p->kernels[kind],items*4,128,args,n,p->gpu_stream):pc_launch(p->kernels[kind],items,256,args,n,p->gpu_stream);
 return s==PC_OK?1:failure(p,"launch generated incremental kernel");
}
_Bool ias_upload(coax_v_f32 x){PcArg a;psionic_incremental *p=active;if(!p||!bind_view(p,x,&a))return 0;return pc_upload(a.data,x.data,(size_t)x.count*4,p->gpu_stream)==PC_OK?1:failure(p,"upload incremental arena");}
_Bool ias_read(coax_v_f32 x,coax_v_f32 out){PcArg a;psionic_incremental *p=active;if(!p||out.count<x.count||!bind_view(p,x,&a))return 0;return pc_download(out.data,a.data,(size_t)x.count*4,p->gpu_stream)==PC_OK&&pc_sync_stream(p->gpu_stream)==PC_OK?1:failure(p,"read incremental arena");}

psionic_incremental *psionic_incremental_open(const char *directory,char *error,int64_t capacity){
 psionic_incremental *p=calloc(1,sizeof *p);if(!p)return NULL;pthread_mutex_init(&p->lock,NULL);active=p;
 if(!directory||!map_input(directory,"model.safetensors",&p->file,&p->file_n)||!map_input(directory,"tokenizer.json",&p->json,&p->json_n)){snprintf(p->error,sizeof p->error,"missing original model/tokenizer files");goto bad;}
 if(!ias_model_identity((coax_v_u8){p->file,p->file_n},(coax_v_u8){p->json,p->json_n})){snprintf(p->error,sizeof p->error,"incremental model identity refused");goto bad;}
 p->nodes=calloc(65536,sizeof *p->nodes);p->vocab_bytes=calloc(400216,1);p->work=calloc(IAS_WORK,4);p->state=calloc(IAS_STATE,4);p->bank=calloc(32896,sizeof(double));p->window=calloc(512,sizeof(double));
 if(!p->nodes||!p->vocab_bytes||!p->work||!p->state||!p->bank||!p->window){snprintf(p->error,sizeof p->error,"incremental host allocation failed");goto bad;}
 if(!ias_model_tokens((coax_v_u8){p->json,p->json_n},(coax_v_t_JvNode){p->nodes,65536},(coax_v_u8){p->vocab_bytes,400216},(coax_v_string){p->pieces,1024})){snprintf(p->error,sizeof p->error,"incremental vocabulary refused");goto bad;}
 ias_front_tables((coax_v_f64){p->bank,32896},(coax_v_f64){p->window,512});
 PcDeviceInfo info;if(pc_open(&info)!=PC_OK){failure(p,"open CUDA");goto bad;}
 double seconds;if(pc_stream(&p->gpu_stream)!=PC_OK||pc_compile((const char*)incremental_cuda_source,incremental_cuda_source_len,"incremental.cu",&p->module,&seconds)!=PC_OK){failure(p,"compile generated incremental CUDA");goto bad;}
 for(int i=0;i<15;i++)if(pc_kernel(p->module,kernel_names[i],&p->kernels[i])!=PC_OK){failure(p,"bind incremental kernel");goto bad;}
 p->base[0]=weights(p).data;p->count[0]=IAS_WEIGHT_COUNT;p->base[1]=p->work;p->count[1]=IAS_WORK;p->base[2]=p->state;p->count[2]=IAS_STATE;
 if(pc_alloc((size_t)p->count[0]*4,&p->buffers[0])==PC_OK)p->resident_words=p->count[0];
 else {
  p->staging=1;size_t available=0,total=0;
  /* Keep a bounded prefix on a shared GPU. Other original views use two
   * fixed transfer buffers, ordered on this stream. Only byte transport
   * changes; storage precision and arithmetic remain identical. */
  if(pc_memory(&available,&total)==PC_OK&&available>268435456){size_t bytes=(available-268435456)&~(size_t)1048575;if(bytes>1073741824)bytes=1073741824;if(pc_alloc(bytes,&p->buffers[0])==PC_OK)p->resident_words=bytes/4;}
  for(int i=0;i<2;i++)if(pc_alloc(20000000,&p->staged[i])!=PC_OK){failure(p,"allocate original-weight transfer buffers");goto bad;}
 }
 for(int i=1;i<3;i++)if(pc_alloc((size_t)p->count[i]*4,&p->buffers[i])!=PC_OK){failure(p,"allocate incremental GPU arena");goto bad;}
 if((p->resident_words&&pc_upload(p->buffers[0],p->base[0],(size_t)p->resident_words*4,p->gpu_stream)!=PC_OK)||!ias_device_prepare(weights(p),fv(p->work,IAS_WORK),fv(p->state,IAS_STATE))||pc_sync_stream(p->gpu_stream)!=PC_OK){failure(p,"prepare incremental model");goto bad;}
 active=NULL;return p;
bad:
 if(error&&capacity>0)snprintf(error,(size_t)capacity,"%s",p->error);psionic_incremental_close(p);return NULL;
}
int psionic_incremental_reset(psionic_incremental *p,uint64_t epoch){if(!p)return 0;pthread_mutex_lock(&p->lock);active=p;int result;if(pc_open(NULL)==PC_OK)result=ias_stream_reset(sv(p),epoch,fv(p->work,IAS_WORK),fv(p->state,IAS_STATE));else {ias_stream_backend_failure(sv(p));failure(p,"activate CUDA");result=0;}active=NULL;pthread_mutex_unlock(&p->lock);return result;}
int psionic_incremental_push(psionic_incremental *p,uint64_t epoch,uint64_t sequence,const float *samples,int64_t count,int final){
 if(!p||(!samples&&count))return 1;pthread_mutex_lock(&p->lock);active=p;
 int ready=pc_open(NULL)==PC_OK;if(!ready){ias_stream_backend_failure(sv(p));failure(p,"activate CUDA");}
 int result=ready?ias_stream_push(sv(p),epoch,sequence,fv((float*)samples,count),final!=0,weights(p),fv(p->work,IAS_WORK),fv(p->state,IAS_STATE),(coax_v_f64){p->bank,32896},(coax_v_f64){p->window,512},(coax_v_string){p->pieces,1024}):3;
 active=NULL;pthread_mutex_unlock(&p->lock);return result;
}
int psionic_incremental_cancel(psionic_incremental *p,uint64_t epoch){if(!p)return 0;pthread_mutex_lock(&p->lock);active=p;int ready=pc_open(NULL)==PC_OK;int result=ias_stream_cancel(sv(p),epoch,fv(p->work,IAS_WORK),fv(p->state,IAS_STATE));if(!ready){failure(p,"activate CUDA");result=0;}active=NULL;pthread_mutex_unlock(&p->lock);return result;}
int64_t psionic_incremental_text(psionic_incremental *p,uint64_t epoch,uint8_t *out,int64_t capacity){if(!p||(!out&&capacity))return -1;pthread_mutex_lock(&p->lock);int64_t result=ias_stream_text(sv(p),epoch,(coax_v_u8){out,capacity});pthread_mutex_unlock(&p->lock);return result;}
int psionic_incremental_info(psionic_incremental *p,int64_t out[6]){if(!p||!out)return 0;pthread_mutex_lock(&p->lock);out[0]=p->stream_state.gate.phase;out[1]=p->stream_state.gate.samples;out[2]=p->stream_state.front.e[0].frame;out[3]=p->stream_state.chunks;out[4]=p->stream_state.count;out[5]=p->stream_state.gate.sequence;pthread_mutex_unlock(&p->lock);return 1;}
const char *psionic_incremental_error(psionic_incremental *p){return p?p->error:"no incremental engine";}
int psionic_incremental_inspect(psionic_incremental *p,float *encoded,float *projected){if(!p||!encoded||!projected)return 0;pthread_mutex_lock(&p->lock);active=p;int ok=p->stream_state.chunks>0&&pc_open(NULL)==PC_OK&&ias_read(fv(p->work+IAS_H,2048),fv(encoded,2048))&&ias_read(fv(p->work+IAS_PROJ,1280),fv(projected,1280));active=NULL;pthread_mutex_unlock(&p->lock);return ok;}
int64_t psionic_incremental_tokens(psionic_incremental *p,int64_t *tokens,int64_t *positions,int64_t capacity){if(!p||!tokens||!positions)return -1;pthread_mutex_lock(&p->lock);int64_t n=p->stream_state.count;if(capacity<n||p->stream_state.gate.phase<1||p->stream_state.gate.phase>2)n=-1;else {memcpy(tokens,p->stream_state.tokens.e,(size_t)n*8);memcpy(positions,p->stream_state.positions.e,(size_t)n*8);}pthread_mutex_unlock(&p->lock);return n;}
int psionic_incremental_cache(psionic_incremental *p,int layer,float *keys,float *conv){if(!p||!keys||!conv)return -1;pthread_mutex_lock(&p->lock);active=p;int n=pc_open(NULL)==PC_OK?ias_cache_snapshot(fv(p->state,IAS_STATE),p->stream_state.chunks,layer,fv(keys,71680),fv(conv,8192)):-1;active=NULL;pthread_mutex_unlock(&p->lock);return n;}
int psionic_incremental_features(psionic_incremental *p,float *features){if(!p||!features)return -1;pthread_mutex_lock(&p->lock);int n=ias_feature_snapshot(sv(p),fv(p->work,IAS_WORK),fv(features,2048));pthread_mutex_unlock(&p->lock);return n;}
int psionic_incremental_memory(psionic_incremental *p,int64_t out[4]){if(!p||!out)return 0;pthread_mutex_lock(&p->lock);out[0]=(int64_t)p->resident_words*4;out[1]=p->staging?40000000:0;out[2]=IAS_WORK*4;out[3]=IAS_STATE*4;pthread_mutex_unlock(&p->lock);return 1;}
void psionic_incremental_close(psionic_incremental *p){if(!p)return;pc_open(NULL);if(p->gpu_stream)pc_sync_stream(p->gpu_stream);for(int i=0;i<3;i++)if(p->buffers[i])pc_free(p->buffers[i]);for(int i=0;i<2;i++)if(p->staged[i])pc_free(p->staged[i]);if(p->module)pc_unload(p->module);if(p->gpu_stream)pc_destroy_stream(p->gpu_stream);if(p->file)munmap(p->file,p->file_n);if(p->json)munmap(p->json,p->json_n);if(p->work){wipe(p->work,IAS_WORK*4);free(p->work);}if(p->state){wipe(p->state,IAS_STATE*4);free(p->state);}free(p->bank);free(p->window);free(p->nodes);free(p->vocab_bytes);pthread_mutex_destroy(&p->lock);wipe(p,sizeof *p);free(p);active=NULL;}
