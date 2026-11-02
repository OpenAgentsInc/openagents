/* Firm reason: 2 (independent public-ABI clear/reuse integration oracle).
 * One known recording, twice, on one warm original-weight engine. */
#include "psionic-parakeet.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static float *read_pcm(const char*path,int64_t*n){FILE*f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long bytes=ftell(f);assert(bytes>0&&bytes%2==0);rewind(f);unsigned char*p=malloc(bytes);float*x=malloc(bytes/2*4);assert(p&&x&&fread(p,1,bytes,f)==(size_t)bytes&&!fclose(f));*n=bytes/2;for(int64_t i=0;i<*n;i++){int v=p[i*2]|(p[i*2+1]<<8);if(v>=32768)v-=65536;x[i]=(float)v/32768.0f;}free(p);return x;}
static void cleared(psionic_parakeet*s){char text[64]="caller snapshot unchanged";float stage=17;int64_t t=18,f=19;psionic_parakeet_info info={0};info.samples=20;assert(!psionic_parakeet_last_info(s,&info)&&info.samples==20);assert(!psionic_parakeet_json(s,text,sizeof text)&&!strcmp(text,"caller snapshot unchanged"));assert(psionic_parakeet_stage(s,0,&stage,1)==-1&&stage==17);assert(psionic_parakeet_stage(s,1,&stage,1)==-1&&stage==17);assert(psionic_parakeet_tokens(s,&t,&f,1)==-1&&t==18&&f==19);}
int main(int argc,char**argv){if(argc!=3)return 2;char error[1024],first[8193],second[8193];int64_t n;float*x=read_pcm(argv[2],&n);assert(!psionic_parakeet_clear(NULL));psionic_parakeet*s=psionic_parakeet_open(argv[1],1,error,sizeof error);if(!s){fprintf(stderr,"open: %s\n",error);return 1;}
 int threads=psionic_parakeet_decoder_threads(s);assert(threads>=1&&threads<=8);assert(psionic_parakeet_clear(s));cleared(s);
 assert(psionic_parakeet_text(s,x,n,first,sizeof first,error,sizeof error));assert(!strcmp(first,"I am convinced of what I say, said the Count."));
 psionic_parakeet_info info;assert(psionic_parakeet_last_info(s,&info)&&info.gpu&&info.samples==n);assert(psionic_parakeet_clear(s));cleared(s);assert(!strcmp(first,"I am convinced of what I say, said the Count."));
 assert(psionic_parakeet_decoder_threads(s)==threads);assert(psionic_parakeet_text(s,x,n,second,sizeof second,error,sizeof error));assert(!strcmp(first,second));assert(psionic_parakeet_clear(s));assert(psionic_parakeet_clear(s));cleared(s);
 psionic_parakeet_close(s);free(x);puts("Parakeet Metal clear/reuse: same engine and worker pool, known exact text twice, snapshots invalidated after each clear");return 0;}
