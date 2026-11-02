/* Firm reason: 2 (independent adversarial public-API lifetime/atomicity oracle).
 * The two different real clips force the work arenas to change. The middle
 * call fails only after inference, when one text byte cannot hold its result.
 * No private model state or generated Coax implementation is inspected. */
#include "psionic-parakeet.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define MAX_VALUES (128*2048)
#define MAX_TOKENS 2560
#define JSON_BYTES 131072
#define TEXT_BYTES 8193

typedef struct {
 psionic_parakeet_info info;
 int64_t tokens[MAX_TOKENS],frames[MAX_TOKENS],nt,nf,ne;
 float *features,*encoded;
 char json[JSON_BYTES];
} snapshot;
static float *audio(const char *path,int64_t *count){
 FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long bytes=ftell(f);assert(bytes>0&&bytes%2==0);rewind(f);
 unsigned char *raw=malloc((size_t)bytes);float *samples=malloc((size_t)(bytes/2)*sizeof(float));assert(raw&&samples);assert(fread(raw,1,(size_t)bytes,f)==(size_t)bytes&&!fclose(f));
 *count=bytes/2;for(int64_t i=0;i<*count;i++){int v=raw[2*i]|(raw[2*i+1]<<8);if(v>=32768)v-=65536;samples[i]=(float)v/32768.0f;}free(raw);return samples;
}
static void capture(psionic_parakeet *s,snapshot *v){
 memset(v,0,sizeof *v);v->features=malloc(MAX_VALUES*sizeof(float));v->encoded=malloc(MAX_VALUES*sizeof(float));assert(v->features&&v->encoded);
 assert(psionic_parakeet_last_info(s,&v->info));assert(v->info.gpu==1);
 v->nt=psionic_parakeet_tokens(s,v->tokens,v->frames,MAX_TOKENS);assert(v->nt>0&&v->nt==v->info.tokens);
 assert(psionic_parakeet_json(s,v->json,sizeof v->json));
 v->nf=psionic_parakeet_stage(s,0,v->features,MAX_VALUES);v->ne=psionic_parakeet_stage(s,1,v->encoded,MAX_VALUES);
 assert(v->nf==128*v->info.feature_frames&&v->ne==1024*v->info.encoded_frames);
}
static void equal(const snapshot *a,const snapshot *b){
 assert(a->info.samples==b->info.samples&&a->info.feature_frames==b->info.feature_frames&&a->info.encoded_frames==b->info.encoded_frames);
 assert(a->info.tokens==b->info.tokens&&a->info.text_bytes==b->info.text_bytes&&a->info.gpu==b->info.gpu&&a->info.inference_ms==b->info.inference_ms);
 assert(a->nt==b->nt&&a->nf==b->nf&&a->ne==b->ne&&strcmp(a->json,b->json)==0);
 assert(memcmp(a->tokens,b->tokens,(size_t)a->nt*sizeof(int64_t))==0&&memcmp(a->frames,b->frames,(size_t)a->nt*sizeof(int64_t))==0);
 assert(memcmp(a->features,b->features,(size_t)a->nf*sizeof(float))==0&&memcmp(a->encoded,b->encoded,(size_t)a->ne*sizeof(float))==0);
}
static void drop(snapshot *v){free(v->features);free(v->encoded);}
int main(int argc,char **argv){
 if(argc!=4){fprintf(stderr,"usage: snapshot MODEL SHORT_A.pcm DIFFERENT_LONG_B.pcm\n");return 2;}
 char err[1024],textA[TEXT_BYTES],textB[TEXT_BYTES],sentinel[TEXT_BYTES];int64_t na,nb;float *a=audio(argv[2],&na),*b=audio(argv[3],&nb);assert(nb>na);
 psionic_parakeet *s=psionic_parakeet_open(argv[1],1,err,sizeof err);if(!s){fprintf(stderr,"open: %s\n",err);return 1;}
 assert(psionic_parakeet_text(s,a,na,textA,sizeof textA,err,sizeof err));snapshot A,after;capture(s,&A);assert(A.info.samples==na&&textA[0]);
 memset(textB,0x5a,sizeof textB);memcpy(sentinel,textB,sizeof textB);
 assert(!psionic_parakeet_text(s,b,nb,textB,1,err,sizeof err));assert(strstr(err,"transcription refused")&&strstr(err,"status 10"));
 assert(memcmp(textB,sentinel,sizeof textB)==0);capture(s,&after);equal(&A,&after);drop(&after);
 // A too-small stage copy cannot publish a prefix or disturb the snapshot.
 float *guard=malloc(MAX_VALUES*sizeof(float));assert(guard);for(int i=0;i<MAX_VALUES;i++)guard[i]=-1234.5f;
 assert(psionic_parakeet_stage(s,0,guard,(size_t)A.nf-1)==-1);for(int i=0;i<MAX_VALUES;i++)assert(guard[i]==-1234.5f);
 assert(psionic_parakeet_stage(s,1,guard,(size_t)A.ne-1)==-1);for(int i=0;i<MAX_VALUES;i++)assert(guard[i]==-1234.5f);
 char small[32],saved[32];memset(small,0x27,sizeof small);memcpy(saved,small,sizeof small);assert(!psionic_parakeet_json(s,small,1)&&memcmp(small,saved,sizeof small)==0);
 capture(s,&after);equal(&A,&after);drop(&after);
 assert(psionic_parakeet_text(s,b,nb,textB,sizeof textB,err,sizeof err));snapshot B;capture(s,&B);
 assert(B.info.samples==nb&&B.info.feature_frames>A.info.feature_frames&&B.info.encoded_frames>A.info.encoded_frames);
 assert(textB[0]&&strcmp(textA,textB)!=0&&strcmp(A.json,B.json)!=0);
 printf("PASS late-capacity snapshot: A_samples=%lld A_tokens=%lld A_features=%lld A_encoded=%lld; B_samples=%lld B_tokens=%lld B_features=%lld B_encoded=%lld; text/info/tokens/frames/JSON/features/encoder snapshot A unchanged after real B inference refusal; small stage/JSON copies atomic; successful B replaces snapshot\n",(long long)na,(long long)A.nt,(long long)A.nf,(long long)A.ne,(long long)nb,(long long)B.nt,(long long)B.nf,(long long)B.ne);
 psionic_parakeet_close(s);drop(&A);drop(&B);free(guard);free(a);free(b);return 0;
}
