/* Process/file/memory/Metal effects for the resident Coax Parakeet engine.
 * Model admission, WAV/audio rules, token rendering and JSON are Coax. */
#define _POSIX_C_SOURCE 200809L
#ifdef __APPLE__
#define __STDC_WANT_LIB_EXT1__ 1
#endif
#include "parakeet-runtime.h"
#include "psionic-parakeet.h"
#include "parakeet-threads.h"
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#ifdef PSIONIC_PARAKEET_METAL
#include "parakeet-metal.h"
#endif
#define TOKENS 2560
#define TEXT 8193
#define JSON_MAX 131072
#define NODES 65536
#define VOCAB 8192
#define BYTES 262144
#define MAX_SAMPLES 327520
struct psionic_parakeet {
 float *ew,*dw,*w32,*features,*encoded,*last_features,*last_encoded;double *wave,*w64;int64_t ne,nd,n32,n64;
 coax_string *pieces;uint8_t *bytes;
 int64_t tokens[TOKENS],frames[TOKENS],draft_tokens[TOKENS],draft_frames[TOKENS],stats[4];
 char text[TEXT];int gpu,has_result;psionic_parakeet_info info;
};
static pthread_mutex_t lock=PTHREAD_MUTEX_INITIALIZER;
static psionic_parakeet *resident;
typedef struct {uint8_t *data;int64_t size;} mapped;
static double now_ms(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec*1000.0+t.tv_nsec/1e6;}
static void error(char *out,size_t cap,const char *message){if(out&&cap)snprintf(out,cap,"%s",message);}
static void status_error(char *out,size_t cap,const char *phase,int status){if(out&&cap)snprintf(out,cap,"Parakeet %s refused (status %d)",phase,status);}
static mapped map_file(const char *path){mapped m={0};int fd=open(path,O_RDONLY);if(fd<0)return m;struct stat st;if(fstat(fd,&st)||st.st_size<=0||(uint64_t)st.st_size>SIZE_MAX){close(fd);return m;}void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)return m;m.data=p;m.size=st.st_size;return m;}
static void unmap(mapped m){if(m.data)munmap(m.data,(size_t)m.size);}
static void wipe(void *p,size_t n){if(!p||!n)return;
#ifdef __APPLE__
 (void)memset_s(p,n,0,n);
#else
 volatile unsigned char *bytes=p;while(n--)*bytes++=0;
#endif
}
static void clear_work_locked(psionic_parakeet *s){
 wipe(s->w32,(size_t)s->n32*sizeof *s->w32);wipe(s->w64,(size_t)s->n64*sizeof *s->w64);wipe(s->wave,MAX_SAMPLES*sizeof *s->wave);
 wipe(s->features,128*2048*sizeof *s->features);wipe(s->encoded,256*1024*sizeof *s->encoded);
 wipe(s->last_features,128*2048*sizeof *s->last_features);wipe(s->last_encoded,256*1024*sizeof *s->last_encoded);
 wipe(s->tokens,sizeof s->tokens);wipe(s->frames,sizeof s->frames);wipe(s->draft_tokens,sizeof s->draft_tokens);wipe(s->draft_frames,sizeof s->draft_frames);
 wipe(s->stats,sizeof s->stats);wipe(s->text,sizeof s->text);wipe(&s->info,sizeof s->info);s->has_result=0;
#ifdef PSIONIC_PARAKEET_METAL
 if(s->gpu)pk_metal_clear_work();
#endif
}
static void release(psionic_parakeet *s){if(!s)return;free(s->ew);free(s->dw);free(s->w32);free(s->w64);free(s->wave);free(s->features);free(s->encoded);free(s->last_features);free(s->last_encoded);free(s->pieces);free(s->bytes);free(s);}
int64_t cxpk_encode(coax_v_f32 ew,coax_v_f32 x,int64_t frames,coax_v_f32 work,coax_v_f32 out){
#ifdef PSIONIC_PARAKEET_METAL
 return pk_metal_encode(ew.data,ew.count,x.data,x.count,frames,work.data,work.count,out.data,out.count);
#else
 (void)ew;(void)x;(void)frames;(void)work;(void)out;return -1;
#endif
}
#ifndef PSIONIC_PARAKEET_METAL
_Bool pkq_launch(int64_t kind,int64_t items,int64_t n0,int64_t n1,int64_t n2,int64_t n3,int64_t n4,int64_t n5,int64_t n6,int64_t n7,float scale,coax_v_f32 a,coax_v_f32 b,coax_v_f32 c,coax_v_f32 d,coax_v_f32 e,coax_v_f32 f){(void)kind;(void)items;(void)n0;(void)n1;(void)n2;(void)n3;(void)n4;(void)n5;(void)n6;(void)n7;(void)scale;(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0;}
#endif
psionic_parakeet *psionic_parakeet_open(const char *dir,int gpu,char *err,size_t errcap){
 pthread_mutex_lock(&lock);psionic_parakeet *s=NULL; mapped model={0},json={0};coax_t_JvNode *nodes=NULL;int64_t offsets[723];char path[4096];int gpu_effects=0;
 if(resident){error(err,errcap,"one Parakeet model is already open");goto fail;}
 if(!dir||snprintf(path,sizeof path,"%s/model.safetensors",dir)>=(int)sizeof path){error(err,errcap,"invalid model directory");goto fail;}
 if(gpu){
#ifdef PSIONIC_PARAKEET_METAL
 if(!pk_metal_available()){error(err,errcap,"Apple Metal is unavailable");goto fail;}gpu_effects=1;
#else
 error(err,errcap,"this build has no Apple Metal backend");goto fail;
#endif
 }
 model=map_file(path);if(!model.data){error(err,errcap,"cannot map model.safetensors");goto fail;}
 if(snprintf(path,sizeof path,"%s/tokenizer.json",dir)>=(int)sizeof path){error(err,errcap,"invalid tokenizer path");goto fail;}
 json=map_file(path);if(!json.data){error(err,errcap,"cannot map tokenizer.json");goto fail;}
 s=calloc(1,sizeof *s);nodes=calloc(NODES,sizeof *nodes);if(!s||!nodes){error(err,errcap,"out of memory");goto fail;}
 s->pieces=calloc(VOCAB,sizeof *s->pieces);s->bytes=calloc(BYTES,1);if(!s->pieces||!s->bytes){error(err,errcap,"out of memory");goto fail;}
 coax_t_PkRtStatus st=pkrt_open((coax_v_u8){model.data,model.size},(coax_v_u8){json.data,json.size},(coax_v_t_JvNode){nodes,NODES},(coax_v_u8){s->bytes,BYTES},(coax_v_string){s->pieces,VOCAB},(coax_v_s64){offsets,723});
 if(st!=COAX_PkRtStatus_ok){status_error(err,errcap,"model",st);goto fail;}
 s->ne=pkrt_encoder_count();s->nd=pkrt_decoder_count();s->ew=calloc((size_t)s->ne,4);s->dw=calloc((size_t)s->nd,4);
 s->n32=pkrt_work32(MAX_SAMPLES);s->n64=pkrt_work64(MAX_SAMPLES);s->w32=calloc((size_t)s->n32,4);s->w64=calloc((size_t)s->n64,8);s->wave=calloc(MAX_SAMPLES,8);s->features=calloc(128*2048,4);s->encoded=calloc(256*1024,4);s->last_features=calloc(128*2048,4);s->last_encoded=calloc(256*1024,4);
 if(!s->ew||!s->dw||!s->w32||!s->w64||!s->wave||!s->features||!s->encoded||!s->last_features||!s->last_encoded){error(err,errcap,"out of memory");goto fail;}
 for(int64_t i=0;i<723;i++){int64_t part=pkrt_part(i);if(part<0)continue;int64_t at=pkrt_at(i),count=pkrt_count(i);memcpy((part?s->dw:s->ew)+at,model.data+offsets[i],(size_t)count*4);}
 pk_threads_start(0);s->gpu=!!gpu;resident=s;unmap(model);unmap(json);free(nodes);pthread_mutex_unlock(&lock);return s;
fail:unmap(model);unmap(json);free(nodes);release(s);
#ifdef PSIONIC_PARAKEET_METAL
 if(gpu_effects)pk_metal_close();
#else
 (void)gpu_effects;
#endif
 pthread_mutex_unlock(&lock);return NULL;
}
static int valid(psionic_parakeet *s,char *err,size_t cap){if(!s||s!=resident){error(err,cap,"invalid or closed Parakeet handle");return 0;}return 1;}
static void commit(psionic_parakeet *s,const char *draft,const int64_t *stats,int64_t samples,double elapsed,int gpu_used){
 memcpy(s->text,draft,(size_t)stats[0]+1);memcpy(s->stats,stats,sizeof s->stats);memcpy(s->tokens,s->draft_tokens,(size_t)stats[1]*8);memcpy(s->frames,s->draft_frames,(size_t)stats[1]*8);s->has_result=1;
 if(samples>0){memcpy(s->last_features,s->features,(size_t)stats[2]*128*4);memcpy(s->last_encoded,s->encoded,(size_t)stats[3]*1024*4);}
 s->info=(psionic_parakeet_info){samples,stats[2],stats[3],stats[1],stats[0],gpu_used,elapsed};
}
int psionic_parakeet_text(psionic_parakeet *s,const float *samples,int64_t n,char *out,size_t cap,char *err,size_t errcap){
 pthread_mutex_lock(&lock);int ok=0;char draft[TEXT];int64_t stats[4]={0};double start=now_ms();
 if(!valid(s,err,errcap))goto done;if(!samples||!out||cap>INT64_MAX||!pkrt_audio_count(n)){error(err,errcap,"audio must contain320..327520 mono16kHz samples");goto done;}
 coax_t_PkRtStatus st=pkrt_transcribe((coax_v_f32){s->ew,s->ne},(coax_v_f32){s->dw,s->nd},(coax_v_string){s->pieces,VOCAB},(coax_v_f32){(float*)samples,n},s->gpu,(coax_v_f64){s->wave,MAX_SAMPLES},(coax_v_f64){s->w64,s->n64},(coax_v_f32){s->w32,s->n32},(coax_v_f32){s->features,128*2048},(coax_v_f32){s->encoded,256*1024},(coax_v_s64){s->draft_tokens,TOKENS},(coax_v_s64){s->draft_frames,TOKENS},(coax_v_u8){(uint8_t*)draft,cap<TEXT?(int64_t)cap:TEXT},(coax_v_s64){stats,4});
 if(st!=COAX_PkRtStatus_ok){status_error(err,errcap,"transcription",st);goto done;}
 commit(s,draft,stats,n,now_ms()-start,s->gpu);memcpy(out,draft,(size_t)stats[0]+1);ok=1;
done:pthread_mutex_unlock(&lock);return ok;
}
int psionic_parakeet_decode(psionic_parakeet *s,const float *encoded,int64_t frames,char *out,size_t cap,char *err,size_t errcap){
 pthread_mutex_lock(&lock);int ok=0;char draft[TEXT];int64_t stats[4]={0};double start=now_ms();
 if(!valid(s,err,errcap))goto done;if(!encoded||!out||frames<1||frames>256||cap>INT64_MAX){error(err,errcap,"encoded frames must be 1..256 rows of1024");goto done;}
 coax_t_PkRtStatus st=pkrt_decode((coax_v_f32){s->dw,s->nd},(coax_v_string){s->pieces,VOCAB},(coax_v_f32){(float*)encoded,frames*1024},frames,(coax_v_f32){s->w32,s->n32},(coax_v_s64){s->draft_tokens,TOKENS},(coax_v_s64){s->draft_frames,TOKENS},(coax_v_u8){(uint8_t*)draft,cap<TEXT?(int64_t)cap:TEXT},(coax_v_s64){stats,4});
 if(st!=COAX_PkRtStatus_ok){status_error(err,errcap,"decode",st);goto done;}stats[3]=frames;commit(s,draft,stats,0,now_ms()-start,0);memcpy(out,draft,(size_t)stats[0]+1);ok=1;
done:pthread_mutex_unlock(&lock);return ok;
}
int psionic_parakeet_last_info(psionic_parakeet *s,psionic_parakeet_info *out){pthread_mutex_lock(&lock);int ok=s&&s==resident&&s->has_result&&out;if(ok)*out=s->info;pthread_mutex_unlock(&lock);return ok;}
int psionic_parakeet_decoder_threads(psionic_parakeet *s){pthread_mutex_lock(&lock);int n=s&&s==resident?pk_threads_count():0;pthread_mutex_unlock(&lock);return n;}
int psionic_parakeet_clear(psionic_parakeet *s){pthread_mutex_lock(&lock);int ok=s&&s==resident;if(ok)clear_work_locked(s);pthread_mutex_unlock(&lock);return ok;}
int64_t psionic_parakeet_tokens(psionic_parakeet *s,int64_t *tokens,int64_t *frames,size_t cap){pthread_mutex_lock(&lock);int64_t n=-1;if(s&&s==resident&&s->has_result&&tokens&&frames&&cap>=(size_t)s->stats[1]){n=s->stats[1];memcpy(tokens,s->tokens,(size_t)n*8);memcpy(frames,s->frames,(size_t)n*8);}pthread_mutex_unlock(&lock);return n;}
int psionic_parakeet_json(psionic_parakeet *s,char *out,size_t cap){pthread_mutex_lock(&lock);int ok=0;if(s&&s==resident&&s->has_result&&out&&cap<=INT64_MAX){uint8_t draft[JSON_MAX];int64_t n=pkrt_json((coax_string){s->text,s->stats[0]},s->info.gpu,s->info.samples,(coax_v_s64){s->stats,4},(coax_v_s64){s->tokens,s->stats[1]},(coax_v_s64){s->frames,s->stats[1]},(coax_v_u8){draft,sizeof draft});if(n>=0&&(size_t)n<cap){memcpy(out,draft,(size_t)n);out[n]=0;ok=1;}}pthread_mutex_unlock(&lock);return ok;}
int64_t psionic_parakeet_stage(psionic_parakeet *s,int stage,float *out,size_t cap){pthread_mutex_lock(&lock);int64_t n=-1;if(s&&s==resident&&s->has_result&&s->info.samples>0&&out&&(stage==0||stage==1)){int64_t size=stage==0?s->info.feature_frames*128:s->info.encoded_frames*1024;if(cap>=(size_t)size){memcpy(out,stage==0?s->last_features:s->last_encoded,(size_t)size*4);n=size;}}pthread_mutex_unlock(&lock);return n;}
void psionic_parakeet_close(psionic_parakeet *s){pthread_mutex_lock(&lock);if(s&&s==resident){clear_work_locked(s);resident=NULL;
 pk_threads_stop();
#ifdef PSIONIC_PARAKEET_METAL
 pk_metal_close();
#endif
 release(s);}pthread_mutex_unlock(&lock);}
#ifndef PSIONIC_PARAKEET_LIBRARY
static int write_bytes(const char *path,const void *data,size_t bytes){FILE*f=fopen(path,"wb");if(!f)return 0;int ok=fwrite(data,1,bytes,f)==bytes;return fclose(f)==0&&ok;}
static int dump(psionic_parakeet *s,const char *prefix){char path[4096],json[JSON_MAX];if(!psionic_parakeet_json(s,json,sizeof json))return 0;
 snprintf(path,sizeof path,"%s.tokens.json",prefix);if(!write_bytes(path,json,strlen(json)))return 0;
 if(s->info.samples){snprintf(path,sizeof path,"%s.features.f32",prefix);if(!write_bytes(path,s->last_features,(size_t)s->info.feature_frames*128*4))return 0;snprintf(path,sizeof path,"%s.encoded.f32",prefix);if(!write_bytes(path,s->last_encoded,(size_t)s->info.encoded_frames*1024*4))return 0;}return 1;}
int main(int argc,char **argv){
 if(argc<4||strcmp(argv[1],"transcribe")){fprintf(stderr,"usage: psionic-parakeet transcribe MODEL_DIR WAV [--gpu|--cpu] [--text|--json] [--dump-prefix PATH]\n");return 2;}
 int gpu=0,json_out=0;const char *prefix=NULL;for(int i=4;i<argc;i++){if(!strcmp(argv[i],"--gpu"))gpu=1;else if(!strcmp(argv[i],"--cpu"))gpu=0;else if(!strcmp(argv[i],"--text"))json_out=0;else if(!strcmp(argv[i],"--json"))json_out=1;else if(!strcmp(argv[i],"--dump-prefix")&&i+1<argc)prefix=argv[++i];else {fprintf(stderr,"unsupported Parakeet option: %s\n",argv[i]);return 2;}}
 mapped wav=map_file(argv[3]);if(!wav.data){fprintf(stderr,"cannot map WAV\n");return 65;}coax_t_WvInfo info=pkrt_wav_info((coax_v_u8){wav.data,wav.size});int64_t n=info.status==COAX_WvStatus_ok?pkrt_wav_length(info.frames,info.rate):-1;
 if(!pkrt_audio_count(n)){fprintf(stderr,"WAV refused: need320..327520 resampled16kHz mono samples (20.47s maximum)\n");unmap(wav);return 65;}
 float *mono=calloc((size_t)info.frames,4),*samples=calloc((size_t)n,4);if(!mono||!samples||pkrt_wav((coax_v_u8){wav.data,wav.size},(coax_v_f32){mono,info.frames},(coax_v_f32){samples,n})!=n){fprintf(stderr,"WAV conversion refused\n");free(mono);free(samples);unmap(wav);return 65;}free(mono);unmap(wav);
 char err[512],text[TEXT],json[JSON_MAX];psionic_parakeet *s=psionic_parakeet_open(argv[2],gpu,err,sizeof err);if(!s){fprintf(stderr,"%s\n",err);free(samples);return 1;}
 int ok=psionic_parakeet_text(s,samples,n,text,sizeof text,err,sizeof err);free(samples);if(!ok){fprintf(stderr,"%s\n",err);psionic_parakeet_close(s);return 1;}
 if(prefix&&!dump(s,prefix)){fprintf(stderr,"cannot write dump prefix\n");psionic_parakeet_close(s);return 1;}
 if(json_out){if(!psionic_parakeet_json(s,json,sizeof json)){psionic_parakeet_close(s);return 1;}puts(json);}else puts(text);
 psionic_parakeet_close(s);return 0;
}
#endif
