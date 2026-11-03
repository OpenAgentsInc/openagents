/* Firm reason: 2 (independent native Metal/CUDA/CPU target and refused-output oracle).
 * Links the generated Coax reference and production Coax scheduler/kernels.
 * Build with -DPSIONIC_PARAKEET_CUDA and parakeet-cuda.o/cuda-host.o for the
 * CUDA profile, otherwise with parakeet-metal.o/metal-host.o for Metal. */
#include "parakeet-ref.h"
#ifdef PSIONIC_PARAKEET_CUDA
#include "parakeet-cuda.h"
#define PK_GPU_NAME "CUDA"
#define pk_gpu_available pk_cuda_available
#define pk_gpu_close pk_cuda_close
#define pk_gpu_encode_shape pk_cuda_encode_shape
#define pk_gpu_error pk_cuda_error
#define pk_gpu_seconds pk_cuda_seconds
#else
#include "parakeet-metal.h"
#define PK_GPU_NAME "Metal"
#define pk_gpu_available pk_metal_available
#define pk_gpu_close pk_metal_close
#define pk_gpu_encode_shape pk_metal_encode_shape
#define pk_gpu_error pk_metal_error
#define pk_gpu_seconds pk_metal_seconds
#endif
#include <math.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int actual_slices(const char *model,const char *ranges_path,const char *pcm_path) {
 static char names[800][160];static int64_t begin[800],end[800];int count=0;long long b,e;
 FILE *rf=fopen(ranges_path,"r");if(!rf)return 10;while(count<800 && fscanf(rf,"%159s %lld %lld",names[count],&b,&e)==3){begin[count]=b;end[count]=e;count++;}fclose(rf);
 char path[4096];snprintf(path,sizeof path,"%s/model.safetensors",model);int fd=open(path,O_RDONLY);if(fd<0)return 10;struct stat st;if(fstat(fd,&st)||st.st_size!=2508311120LL)return 10;const uint8_t *map=mmap(0,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(map==MAP_FAILED)return 10;
 int64_t ne=pkr_encoder_weights();float *w=calloc((size_t)ne,sizeof(float));if(!w)return 10;
 for(int64_t i=0;i<pkr_tensors();i++){
  int64_t part=pkr_part(i);if(part!=0)continue;uint8_t name[160];int64_t n=pkr_name(i,(coax_v_u8){name,sizeof name});int j=0;while(j<count && (strlen(names[j])!=(size_t)n || memcmp(names[j],name,(size_t)n)))j++;
  int64_t at=pkr_at(i),nk=pkr_count(i);if(j==count || end[j]-begin[j]!=nk*4 || begin[j]<0 || end[j]>st.st_size || at<0 || at+nk>ne)return 11;memcpy(w+at,map+begin[j],(size_t)nk*4);
 }
 munmap((void *)map,(size_t)st.st_size);
 FILE *pf=fopen(pcm_path,"rb");if(!pf)return 10;fseek(pf,0,SEEK_END);long bytes=ftell(pf);rewind(pf);int64_t samples=bytes/2;int16_t *pcm=malloc((size_t)bytes);if(!pcm || fread(pcm,1,(size_t)bytes,pf)!=(size_t)bytes)return 10;fclose(pf);
 int64_t nf=samples/160+1,encoded=(nf+7)/8,no=encoded*1024;int64_t n64=pkr_work_f64(samples),n32=pkr_work_f32(samples);
 double *wave=calloc((size_t)samples,sizeof(double)),*scratch64=calloc((size_t)n64,sizeof(double));float *scratch32=calloc((size_t)n32,sizeof(float));float *features=calloc((size_t)(128*nf),sizeof(float));float *cpu[3],*gpu=calloc((size_t)no,sizeof(float));for(int i=0;i<3;i++)cpu[i]=calloc((size_t)no,sizeof(float));
 if(!wave||!scratch64||!scratch32||!features||!gpu||!cpu[0]||!cpu[1]||!cpu[2])return 10;
 if(pkr_stages((coax_v_f32){w,ne},(coax_v_s16){pcm,samples},(coax_v_f64){wave,samples},(coax_v_f64){scratch64,n64},(coax_v_f32){scratch32,n32},(coax_v_f32){features,128*nf},(coax_v_f32){cpu[0],no},(coax_v_f32){cpu[1],no},(coax_v_f32){cpu[2],no})!=encoded)return 12;
 int64_t shape[9]={128,256,1024,8,4096,9,24,1,0};
 for(int stage=0;stage<3;stage++){
  if(pk_gpu_encode_shape(shape,w,ne,features,128*nf,nf,gpu,no,stage)!=1){fprintf(stderr,"real GPU: %s\n",pk_gpu_error());return 13;}
  double largest=0,top=0,squared=0;for(int64_t i=0;i<no;i++){double diff=fabs((double)gpu[i]-cpu[stage][i]);if(diff>largest)largest=diff;if(fabs(cpu[stage][i])>top)top=fabs(cpu[stage][i]);squared+=diff*diff;if(!isfinite(gpu[i]))return 14;}
  double limit=0.001*(1+top);printf("original frames=%lld encoded=%lld layers=%d values=%lld max_error=%.9g rms=%.9g limit=%.9g gpu_ms=%.6f\n",(long long)nf,(long long)encoded,stage,(long long)no,largest,sqrt(squared/no),limit,pk_gpu_seconds()*1e3);fflush(stdout);if(largest>limit)return 15;
 }
 pk_gpu_close();free(w);free(pcm);free(wave);free(scratch64);free(scratch32);free(features);free(gpu);for(int i=0;i<3;i++)free(cpu[i]);return 0;
}
int main(int argc,char **argv) {
 if(argc==4)return actual_slices(argv[1],argv[2],argv[3]);
 if(argc!=1){fprintf(stderr,"usage: parakeet-device [MODEL RANGES PCM]\n");return 2;}
 if(!pk_gpu_available()){fprintf(stderr,"GPU unavailable: %s\n",pk_gpu_error());return 1;}
 coax_t_CfShape s=pkref_shape();int64_t shape[9]={s.mels,s.channels,s.dim,s.heads,s.hidden,s.kernel,s.layers,s.vocab,s.xscale};
 int64_t nw=pkref_weights(s);float *w=calloc((size_t)nw,sizeof(float));if(!w || !pkref_init(s,(coax_v_f32){w,nw}))return 2;
 int frames[3]={1,17,33};int cases=0;
 for(int f=0;f<3;f++)for(int prefix=0;prefix<=1;prefix++){
  int64_t n=frames[f],nx=n*s.mels,no=((n+7)/8)*s.dim,work=pkref_work(s,n);
  float *x=calloc((size_t)nx,sizeof(float)),*y=calloc((size_t)(no+1),sizeof(float)),*ref=calloc((size_t)no,sizeof(float)),*scratch=calloc((size_t)work,sizeof(float));
  if(!x||!y||!ref||!scratch)return 2;for(int64_t i=0;i<nx;i++)x[i]=(float)((i*7)%19-9)*0.13f;y[no]=1234;
  if(!pkref_encode(s,(coax_v_f32){w,nw},(coax_v_f32){x,nx},n,(coax_v_f32){scratch,work},(coax_v_f32){ref,no},prefix))return 3;
  if(pk_gpu_encode_shape(shape,w,nw,x,nx,n,y,no+1,prefix)!=1){fprintf(stderr,"GPU: %s\n",pk_gpu_error());return 4;}
  double max=0,top=0;for(int64_t i=0;i<no;i++){double diff=fabs((double)y[i]-ref[i]);if(diff>max)max=diff;if(fabs(ref[i])>top)top=fabs(ref[i]);if(!isfinite(y[i]))return 5;}
  if(max>0.001*(1+top)||y[no]!=1234){fprintf(stderr,"mismatch f=%lld prefix=%d max=%.9g top=%.9g sentinel=%g\n",(long long)n,prefix,max,top,y[no]);return 6;}
  printf("tiny frames=%lld layers=%d values=%lld max_error=%.9g limit=%.9g gpu_ms=%.6f\n",(long long)n,prefix,(long long)no,max,0.001*(1+top),pk_gpu_seconds()*1e3);
  y[0]=9876;if(pk_gpu_encode_shape(shape,w,nw-1,x,nx,n,y,no,prefix)!=-1||y[0]!=9876)return 7;
  float keep=x[0];x[0]=NAN;
  if(pk_gpu_encode_shape(shape,w,nw,x,nx,n,y,no,prefix)!=-1 || y[0]!=9876)return 8;
  x[0]=keep;cases++;free(x);free(y);free(ref);free(scratch);
 }
 pk_gpu_close();free(w);printf("Parakeet " PK_GPU_NAME ": %d CPU-reference cases and capacity refusals passed\n",cases);return 0;
}
