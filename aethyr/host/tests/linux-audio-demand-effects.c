// Firm reason: 1 (SDK fixture: real lazy cache, allocations, copies and retained owners).
#include "NativeAudio.h"
#include "audio-host.h"
#include "audio-opus.h"
#include <stdio.h>
#include <string.h>
#include <pthread.h>
#include <time.h>
#include <errno.h>
static RhAudio *audio;
static RhAudioMemory *memory;
static RhAudioSamples *held;
static int eager;
int64_t hld_open(coax_v_u8 root,bool warm){char path[4097];if(root.count<1||root.count>4096)return 0;memcpy(path,root.data,(size_t)root.count);path[root.count]=0;audio=rha_open();if(!audio||!rhao_resources(audio,path)){rha_close(audio);audio=NULL;return 0;}memory=rha_memory_keep(audio);eager=warm;return 1;}
int64_t hld_register(coax_v_u8 name,coax_v_u8 wav){char line[128];if(name.count<1||name.count>48)return -1;int n=snprintf(line,sizeof line,"%.*s %llu %lld",(int)name.count,name.data,(unsigned long long)(uintptr_t)wav.data,(long long)wav.count);return eager?rha_bank(audio,(uint8_t*)line,(size_t)n,wav.data,(size_t)wav.count):rha_bank_lazy(audio,(uint8_t*)line,(size_t)n,wav.data,(size_t)wav.count);}
int64_t hld_command(coax_v_u8 line,bool started){return rha_command(audio,line.data,(size_t)line.count,started);}
int64_t hld_info(int64_t field){return rha_info(audio,(int)field);}
int64_t hld_warm(void){return rha_warm_bank(audio);}
int64_t hld_probe(coax_v_u8 name,int64_t field){RhAudioSamples *p=rha_cached_keep(audio,name.data,(size_t)name.count);int64_t value=field==0?rhas_count(p):field==1?rhas_channels(p):rhas_ticket(p);rhas_close(p);return value;}
int64_t hld_copy(coax_v_u8 name,coax_v_u8 out){RhAudioSamples *p=rha_cached_keep(audio,name.data,(size_t)name.count);size_t n=(size_t)rhas_count(p)*sizeof(float);int64_t result=-1;if(p&&out.count>=0&&n<=(uint64_t)out.count){memcpy(out.data,rhas_data(p),n);result=(int64_t)n;}rhas_close(p);return result;}
int64_t hld_block(coax_v_u8 out){float block[2048];if(out.count<sizeof block||!rha_block(audio,block))return -1;memcpy(out.data,block,sizeof block);return sizeof block;}
void hld_reset(void){rha_reset(audio);}
void hld_keep(coax_v_u8 name){rhas_close(held);held=rha_cached_keep(audio,name.data,(size_t)name.count);}
int64_t hld_held(void){return rhas_count(held);}
void hld_retire(void){if(audio){rha_session_event(audio,4);rha_close(audio);audio=NULL;}}
int64_t hld_total(void){return rham_total(memory);}
void hld_drop(void){rhas_close(held);held=NULL;}
void hld_close(void){hld_retire();hld_drop();rham_close(memory);memory=NULL;}
typedef struct {pthread_mutex_t mutex;pthread_cond_t cond;RhAudio *core;bool waiting,released,callback_done;int64_t result;bool rendered;} HeldDecode;
static struct timespec held_deadline(void){struct timespec until;clock_gettime(CLOCK_REALTIME,&until);until.tv_sec+=2;return until;}
static RhAudioSamples *held_resolve(void *context,RhAudioMemory *owner,const uint8_t *name,size_t bytes){
 (void)name;(void)bytes;HeldDecode *d=context;pthread_mutex_lock(&d->mutex);d->waiting=true;pthread_cond_broadcast(&d->cond);while(!d->released)pthread_cond_wait(&d->cond,&d->mutex);pthread_mutex_unlock(&d->mutex);return rhas_open(owner,128,1);
}
static void *held_command(void *context){HeldDecode *d=context;const uint8_t line[]="probe 1 0 0 20000 1 0 7";d->result=rha_command_current(d->core,line,sizeof line-1);return NULL;}
static void *held_callback(void *context){HeldDecode *d=context;float block[2048];d->rendered=rha_render(d->core,block,1024,true);pthread_mutex_lock(&d->mutex);d->callback_done=true;pthread_cond_broadcast(&d->cond);pthread_mutex_unlock(&d->mutex);return NULL;}
bool hld_concurrency(coax_v_s64 out){
 if(out.count!=10)return false;memset(out.data,0,10*sizeof(int64_t));
 for(int pass=0;pass<2;pass++){
  HeldDecode d={0};pthread_t command_thread,callback_thread;bool callback_started=false;
  if(pthread_mutex_init(&d.mutex,NULL)||pthread_cond_init(&d.cond,NULL))return false;
  d.core=rha_open();if(!d.core)return false;RhAudioMemory *owner=rha_memory_keep(d.core);rha_resolver(d.core,held_resolve,&d,NULL);
  if(pthread_create(&command_thread,NULL,held_command,&d))return false;
  pthread_mutex_lock(&d.mutex);struct timespec until=held_deadline();while(!d.waiting){if(pthread_cond_timedwait(&d.cond,&d.mutex,&until)==ETIMEDOUT)break;}bool waiting=d.waiting;pthread_mutex_unlock(&d.mutex);
  if(pass==0){out.data[0]=waiting;rha_session_event(d.core,1);callback_started=pthread_create(&callback_thread,NULL,held_callback,&d)==0;
   pthread_mutex_lock(&d.mutex);until=held_deadline();while(callback_started&&!d.callback_done){if(pthread_cond_timedwait(&d.cond,&d.mutex,&until)==ETIMEDOUT)break;}out.data[1]=d.callback_done;pthread_mutex_unlock(&d.mutex);
  }else {out.data[5]=waiting;rha_session_event(d.core,4);}
  pthread_mutex_lock(&d.mutex);d.released=true;pthread_cond_broadcast(&d.cond);pthread_mutex_unlock(&d.mutex);pthread_join(command_thread,NULL);if(callback_started)pthread_join(callback_thread,NULL);
  if(pass==0){out.data[2]=d.rendered?rha_info(d.core,0):-1;out.data[3]=d.result;const uint8_t next[]="probe 1 0 0 20000 1 0 8";out.data[4]=rha_command_current(d.core,next,sizeof next-1);}
  else {out.data[6]=d.result;out.data[7]=rha_info(d.core,11);out.data[8]=rham_total(owner);}
  rha_close(d.core);out.data[9]+=rham_total(owner);rham_close(owner);pthread_cond_destroy(&d.cond);pthread_mutex_destroy(&d.mutex);
 }return true;
}
