/* Firm reason: 2 (independent public-C-ABI corpus/tensor/timing recorder).
 * No model code: PCM bytes become exact float inputs; the public resident
 * engine supplies every feature, encoded frame, token and text byte. */
#define _POSIX_C_SOURCE 200809L
#include "psionic-parakeet.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static double seconds(void){struct timespec t;assert(clock_gettime(CLOCK_MONOTONIC,&t)==0);return t.tv_sec+t.tv_nsec*1e-9;}
static int write_bytes(const char*path,const void*bytes,size_t n){FILE*f=fopen(path,"wb");if(!f)return 0;int ok=fwrite(bytes,1,n,f)==n;return fclose(f)==0&&ok;}
int main(int argc,char**argv){
 if(argc!=4){fprintf(stderr,"usage: runtime-gpu MODEL LIST.tsv OUTPUT_DIR\n");return 2;}
 FILE*list=fopen(argv[2],"rb");assert(list);char error[1024],text[8193],json[131072],line[4096],path[4096];double started=seconds();
 psionic_parakeet*s=psionic_parakeet_open(argv[1],1,error,sizeof error);if(!s){fprintf(stderr,"open: %s\n",error);return 1;}printf("MODEL_OPEN_SECONDS %.6f\n",seconds()-started);fflush(stdout);
 float *stage=malloc(128*2048*4);assert(stage);int count=0;
 while(fgets(line,sizeof line,list)){
  char*id=strtok(line,"\t"),*input=strtok(NULL,"\t\r\n");assert(id&&input&&!strchr(id,'/'));
  FILE*f=fopen(input,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0&&size%2==0&&size<=327520*2);assert(!fseek(f,0,SEEK_SET));
  unsigned char*pcm=malloc((size_t)size);float*audio=malloc((size_t)size/2*4);assert(pcm&&audio);assert(fread(pcm,1,(size_t)size,f)==(size_t)size&&!fclose(f));
  for(long i=0;i<size/2;i++){int value=pcm[2*i]|(pcm[2*i+1]<<8);if(value>=32768)value-=65536;audio[i]=(float)value/32768.0f;}
  double begin=seconds();if(!psionic_parakeet_text(s,audio,size/2,text,sizeof text,error,sizeof error)){fprintf(stderr,"clip%s refused:%s\n",id,error);return 1;}
  psionic_parakeet_info info;assert(psionic_parakeet_last_info(s,&info)&&info.gpu==1&&info.samples==size/2);
  assert(psionic_parakeet_json(s,json,sizeof json));snprintf(path,sizeof path,"%s/%s.tokens.json",argv[3],id);assert(write_bytes(path,json,strlen(json)));
  int64_t n=psionic_parakeet_stage(s,0,stage,128*2048);assert(n==128*info.feature_frames);snprintf(path,sizeof path,"%s/%s.features.f32",argv[3],id);assert(write_bytes(path,stage,(size_t)n*4));
  n=psionic_parakeet_stage(s,1,stage,128*2048);assert(n==1024*info.encoded_frames);snprintf(path,sizeof path,"%s/%s.encoded.f32",argv[3],id);assert(write_bytes(path,stage,(size_t)n*4));
  printf("GPU %s audio=%.6f frames=%lld encoded=%lld tokens=%lld inference_ms=%.6f total_seconds=%.6f\n",id,(double)info.samples/16000,(long long)info.feature_frames,(long long)info.encoded_frames,(long long)info.tokens,info.inference_ms,seconds()-begin);fflush(stdout);free(audio);free(pcm);count++;
 }
 assert(!ferror(list)&&!fclose(list));free(stage);psionic_parakeet_close(s);printf("DONE clips=%d seconds=%.6f\n",count,seconds()-started);return count==29?0:1;
}
