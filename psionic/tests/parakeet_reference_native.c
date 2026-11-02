/* Firm reason: 2 (independent public-C-ABI artifact and token/frame snapshot oracle).
 * No model arithmetic: the host supplies bytes/storage and records the actual
 * resident engine outputs on already recorded independent NeMo encoder frames. */
#include "psionic-parakeet.h"
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>
#include <math.h>
static double seconds(void) { struct timespec t; assert(clock_gettime(CLOCK_MONOTONIC,&t)==0); return t.tv_sec+t.tv_nsec*1e-9; }
static int same_info(psionic_parakeet_info a,psionic_parakeet_info b) { return a.samples==b.samples&&a.feature_frames==b.feature_frames&&a.encoded_frames==b.encoded_frames&&a.tokens==b.tokens&&a.text_bytes==b.text_bytes&&a.gpu==b.gpu&&a.inference_ms==b.inference_ms; }
static void refused_inputs_preserve_snapshot(psionic_parakeet *engine) {
 psionic_parakeet_info before,after;int64_t ids[4096],at[4096],ids_after[4096],at_after[4096];char snapshot[131072],current[131072],output[64],expected[64],error[512];float signal[320]={0};
 assert(psionic_parakeet_last_info(engine,&before));int64_t n=psionic_parakeet_tokens(engine,ids,at,4096);assert(n>=0&&psionic_parakeet_json(engine,snapshot,sizeof snapshot));memset(expected,0x5a,sizeof expected);
 const int64_t bad_counts[3]={-1,327521,0};
 for(int c=0;c<4;c++) {
  memcpy(output,expected,sizeof output);if(c==3)signal[0]=NAN;int64_t length=c==3?320:bad_counts[c];
  assert(!psionic_parakeet_text(engine,signal,length,output,sizeof output,error,sizeof error));assert(!memcmp(output,expected,sizeof output));
  assert(psionic_parakeet_last_info(engine,&after)&&same_info(before,after));assert(psionic_parakeet_tokens(engine,ids_after,at_after,4096)==n);assert(!memcmp(ids,ids_after,(size_t)n*sizeof *ids)&&!memcmp(at,at_after,(size_t)n*sizeof *at));assert(psionic_parakeet_json(engine,current,sizeof current)&&!strcmp(snapshot,current));
 }
 puts("REAL_MODEL_FAILURE_SNAPSHOTS 4 passed");fflush(stdout);
}
int main(int argc,char **argv) {
 if(argc!=4){fprintf(stderr,"usage: reference-native MODEL LIST.tsv OUTPUT_DIR\n");return 2;}
 FILE *list=fopen(argv[2],"rb");assert(list);char error[1024]={0};double begin=seconds();psionic_parakeet *engine=psionic_parakeet_open(argv[1],0,error,sizeof error);
 if(!engine){fprintf(stderr,"open refused: %s\n",error);return 1;}printf("MODEL_OPEN_SECONDS %.6f\n",seconds()-begin);fflush(stdout);assert(!psionic_parakeet_open(argv[1],0,error,sizeof error));puts("SECOND_OWNER_REFUSED");
 char line[4096],text[65536],json[131072],path[4096];int clips=0;int64_t ids[4096],at[4096];
 while(fgets(line,sizeof line,list)){
  char *id=strtok(line,"\t"),*input=strtok(NULL,"\t"),*count=strtok(NULL,"\t\r\n");assert(id&&input&&count&&!strchr(id,'/'));char *end=NULL;long long frames=strtoll(count,&end,10);assert(end&&!*end&&frames>0&&frames<=256);
  FILE *f=fopen(input,"rb");assert(f);size_t bytes=(size_t)frames*1024*sizeof(float);float *encoded=malloc(bytes);assert(encoded);assert(fread(encoded,1,bytes,f)==bytes&&fgetc(f)==EOF);assert(!fclose(f));
  double started=seconds();if(!psionic_parakeet_decode(engine,encoded,frames,text,sizeof text,error,sizeof error)){fprintf(stderr,"decode %s refused: %s\n",id,error);return 1;}
  int64_t n=psionic_parakeet_tokens(engine,ids,at,4096);assert(n>=0&&n<=2560);psionic_parakeet_info info;assert(psionic_parakeet_last_info(engine,&info)&&info.tokens==n&&info.encoded_frames==frames);
  assert(psionic_parakeet_json(engine,json,sizeof json));int written=snprintf(path,sizeof path,"%s/%s.tokens.json",argv[3],id);assert(written>0&&(size_t)written<sizeof path);FILE *o=fopen(path,"wb");assert(o&&fwrite(json,1,strlen(json),o)==strlen(json)&&fwrite("\n",1,1,o)==1);assert(!fclose(o));
  printf("DECODE %s frames=%lld tokens=%lld seconds=%.6f\n",id,frames,(long long)n,seconds()-started);fflush(stdout);if(clips==0)refused_inputs_preserve_snapshot(engine);free(encoded);clips++;
 }
 assert(!ferror(list)&&!fclose(list));psionic_parakeet_close(engine);printf("DONE clips=%d seconds=%.6f\n",clips,seconds()-begin);return clips==29?0:1;
}
