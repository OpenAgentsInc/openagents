/* File path, owned worker buffers and incremental C API effects only.
 * Coax owns submission/publication ordering and all capture/output policy. */
#define _POSIX_C_SOURCE 200809L
#include "dictation.h"
#include "entropy-foreign.h"
#include "psionic-incremental.h"
#include <pthread.h>
#include <stdint.h>
#include <string.h>

enum { STREAM_BUFFER = 2560, STREAM_TEXT = 8000 };
static psionic_incremental *stream_engine;
static pthread_mutex_t stream_lock = PTHREAD_MUTEX_INITIALIZER;
static float stream_audio[STREAM_BUFFER];
static uint8_t stream_text[STREAM_TEXT];
static int64_t stream_n, stream_text_n;
static uint64_t stream_epoch, stream_sequence;
static int stream_final, stream_state; /* 0 free, 1 running, 2 completed, 3 failed */
static void stream_wipe(void *p,size_t n) { memset(p,0,n);__asm__ __volatile__("" : : "r"(p) : "memory"); }
int64_t cxdict_stream_open(coax_v_u8 dir,coax_v_u8 err) {
  char path[4096],reason[512]={0};
  if(dir.count<1 || dir.count>=(int64_t)sizeof path || memchr(dir.data,0,(size_t)dir.count))return -1;
  memcpy(path,dir.data,(size_t)dir.count);path[dir.count]=0;
  stream_engine=psionic_incremental_open(path,reason,sizeof reason);
  if(stream_engine)return 0;
  int64_t n=(int64_t)strlen(reason);if(n>err.count)n=err.count;
  memcpy(err.data,reason,(size_t)n);return -(n+1);
}
int64_t cxdict_stream_reset(int64_t epoch) {return stream_engine && psionic_incremental_reset(stream_engine,(uint64_t)epoch);}
static void *stream_run(void *unused) {
  (void)unused;
  int ok=psionic_incremental_push(stream_engine,stream_epoch,stream_sequence,stream_audio,stream_n,stream_final)==0;
  int64_t n=ok?psionic_incremental_text(stream_engine,stream_epoch,stream_text,STREAM_TEXT):-1;
  pthread_mutex_lock(&stream_lock);
  stream_text_n=n;
  if(n<0){const char *e=psionic_incremental_error(stream_engine);size_t k=strlen(e);if(k>STREAM_TEXT)k=STREAM_TEXT;memcpy(stream_text,e,k);stream_text_n=(int64_t)k;}
  stream_wipe(stream_audio,sizeof stream_audio);stream_state=n>=0?2:3;
  pthread_mutex_unlock(&stream_lock);return NULL;
}
int64_t cxdict_stream_submit(int64_t epoch,int64_t sequence,coax_v_f32 audio,int64_t final) {
  if(!stream_engine || audio.count<0 || audio.count>STREAM_BUFFER)return -1;
  pthread_mutex_lock(&stream_lock);
  if(stream_state!=0){pthread_mutex_unlock(&stream_lock);return -1;}
  memcpy(stream_audio,audio.data,(size_t)audio.count*sizeof(float));stream_n=audio.count;
  stream_epoch=(uint64_t)epoch;stream_sequence=(uint64_t)sequence;stream_final=final!=0;stream_state=1;
  pthread_t thread;pthread_attr_t at;pthread_attr_init(&at);pthread_attr_setdetachstate(&at,PTHREAD_CREATE_DETACHED);
  int bad=pthread_create(&thread,&at,stream_run,NULL);pthread_attr_destroy(&at);
  if(bad){stream_state=0;stream_wipe(stream_audio,sizeof stream_audio);}
  pthread_mutex_unlock(&stream_lock);return bad?-1:0;
}
/* -1 pending; >=0 copied text bytes; <=-2 failed, with -(n+2) error bytes. */
int64_t cxdict_stream_poll(coax_v_u8 out) {
  pthread_mutex_lock(&stream_lock);int64_t n=-1;
  if(stream_state==2 || stream_state==3){
    if(stream_text_n>out.count)n=-2;
    else{memcpy(out.data,stream_text,(size_t)stream_text_n);n=stream_state==2?stream_text_n:-(stream_text_n+2);}
    stream_wipe(stream_text,sizeof stream_text);stream_text_n=0;stream_state=0;
  }
  pthread_mutex_unlock(&stream_lock);return n;
}
int64_t cxdict_stream_close(void) {
  pthread_mutex_lock(&stream_lock);int busy=stream_state==1;pthread_mutex_unlock(&stream_lock);
  /* No caller stack is borrowed by the detached worker. On cancellation
     the process ends without waiting for an in-flight GPU call, just as
     the existing final-only dictation host does. The worker never prints. */
  if(stream_engine && !busy){psionic_incremental_close(stream_engine);stream_engine=NULL;stream_wipe(stream_text,sizeof stream_text);stream_wipe(stream_audio,sizeof stream_audio);}
  return 0;
}
