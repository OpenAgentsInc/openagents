// Firm reason: 6 (native TCP/TLS worker threads and owned byte queues).
// Coax writes RFC6455, drains/validates messages and admits every callback.
#define _GNU_SOURCE
#include "HostNetwork.h"
#include "realm-host.h"
#include "face.h"
#include <pthread.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

extern int32_t coaxial_host_socket_reserve(void);
extern int64_t coaxial_host_owner(int32_t);
extern uint32_t coaxial_host_socket_send(int32_t,int64_t,int64_t);
extern uint32_t coaxial_host_socket_close(int32_t,int64_t,int64_t);
extern uint32_t coaxial_host_socket_event(int32_t,int64_t,int64_t,int32_t,int64_t,int64_t);
extern int64_t coaxial_host_delivery(int32_t);
extern uint32_t coaxial_host_transport_reset(void);
typedef struct HnCommand {struct HnCommand *next;size_t n;uint8_t bytes[];} HnCommand;
typedef struct HnSocket {
  int id,stop;int64_t generation,engine;char *url;size_t url_n,capacity;
  uint8_t *message;HnCommand *head,*tail,*active;
} HnSocket;
typedef struct HnEvent {
  struct HnEvent *next;int id,event;int64_t generation,engine;size_t n;uint8_t bytes[];
} HnEvent;
static pthread_mutex_t hn_queue=PTHREAD_MUTEX_INITIALIZER,hn_io=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t hn_finished=PTHREAD_COND_INITIALIZER;
static HnSocket *hn_slots[4];static HnEvent *hn_head,*hn_tail;static int hn_running;
static struct {int active;int64_t generation,engine;} hn_owners[4];
extern uint8_t htr_http_path(coax_string) __attribute__((weak));
extern int64_t htr_http_target(coax_string,coax_v_u8) __attribute__((weak));
extern int64_t htr_local_target(coax_string,coax_string,coax_v_u8) __attribute__((weak));
int rhn_http_path(const char *bytes,size_t n) {uint8_t target[12288];return htr_http_target&&htr_http_target((coax_string){bytes,(int64_t)n},(coax_v_u8){target,sizeof target})>=0;}
int rhn_utf8(const char *bytes,size_t n) {return hns_utf8((coax_string){bytes,(int64_t)n});}
void hni_lock(void) {pthread_mutex_lock(&hn_io);}
void hni_unlock(void) {pthread_mutex_unlock(&hn_io);}
void hni_pause(int64_t ms) {struct timespec t={ms/1000,(ms%1000)*1000000};nanosleep(&t,NULL);}
int64_t hni_take(int64_t context) {
  HnSocket *s=(void *)(uintptr_t)context;pthread_mutex_lock(&hn_queue);
  int result=0;if(s->stop)result=2;else if(s->head){s->active=s->head;s->head=s->head->next;if(!s->head)s->tail=NULL;result=1;}
  pthread_mutex_unlock(&hn_queue);return result;
}
int64_t hni_size(int64_t context) {HnSocket *s=(void *)(uintptr_t)context;return s->active?(int64_t)s->active->n:-1;}
int64_t hni_byte(int64_t context,int64_t at) {HnSocket *s=(void *)(uintptr_t)context;return s->active&&at>=0&&(uint64_t)at<s->active->n?s->active->bytes[at]:-1;}
void hni_done(int64_t context) {HnSocket *s=(void *)(uintptr_t)context;free(s->active);s->active=NULL;}
void hni_emit(int64_t context,int64_t event,coax_v_u8 bytes) {
  HnSocket *s=(void *)(uintptr_t)context;
  HnEvent *e=malloc(sizeof *e+(size_t)bytes.count);if(!e)return;
  *e=(HnEvent){.id=s->id,.event=(int)event,.generation=s->generation,.engine=s->engine,.n=(size_t)bytes.count};
  if(bytes.count)memcpy(e->bytes,bytes.data,(size_t)bytes.count);
  pthread_mutex_lock(&hn_queue);if(hn_tail)hn_tail->next=e;else hn_head=e;hn_tail=e;pthread_mutex_unlock(&hn_queue);
}
static void *hn_thread(void *context) {
  HnSocket *s=context;hns_worker((int64_t)(uintptr_t)s,(coax_string){s->url,(int64_t)s->url_n},(coax_v_u8){s->message,(int64_t)s->capacity});
  pthread_mutex_lock(&hn_queue);if(hn_slots[s->id]==s)hn_slots[s->id]=NULL;
  HnCommand *commands=s->head;s->head=NULL;hn_running--;pthread_cond_broadcast(&hn_finished);pthread_mutex_unlock(&hn_queue);
  while(commands){HnCommand *next=commands->next;free(commands);commands=next;}
  free(s->active);free(s->message);free(s->url);free(s);return NULL;
}
int32_t rhn_socket_open(const char *url,uint32_t n) {
  uint8_t target[4096];
  if(!hns_uri((coax_string){url,n})){
    const char *bound=getenv("VERSE_NATIVE_LOCAL_ROOM");if(!bound||!htr_local_target)return -1;
    int64_t bytes=htr_local_target((coax_string){url,n},(coax_string){bound,(int64_t)strlen(bound)},(coax_v_u8){target,sizeof target});
    if(bytes<0)return -1;url=(const char *)target;n=(uint32_t)bytes;
  }
  signal(SIGPIPE,SIG_IGN);
  HnSocket *s=calloc(1,sizeof *s);if(!s)return -1;
  s->url=malloc((size_t)n+1);s->capacity=coaxial_inbox_cap();s->message=malloc(s->capacity?s->capacity:1);
  if(!s->url||!s->message){free(s->url);free(s->message);free(s);return -1;}
  memcpy(s->url,url,n);s->url[n]=0;s->url_n=n;s->id=coaxial_host_socket_reserve();
  if(s->id<0){free(s->url);free(s->message);free(s);return -1;}
  s->generation=coaxial_host_owner(1);s->engine=coaxial_host_owner(2);int id=s->id;
  pthread_mutex_lock(&hn_queue);hn_slots[id]=s;hn_owners[id].active=1;hn_owners[id].generation=s->generation;hn_owners[id].engine=s->engine;hn_running++;pthread_mutex_unlock(&hn_queue);
  pthread_attr_t attr;pthread_attr_init(&attr);pthread_attr_setdetachstate(&attr,PTHREAD_CREATE_DETACHED);pthread_attr_setstacksize(&attr,16u*1024u*1024u);
  pthread_t thread;int error=pthread_create(&thread,&attr,hn_thread,s);pthread_attr_destroy(&attr);
  if(error){
    hni_emit((int64_t)(uintptr_t)s,3,(coax_v_u8){NULL,0});hni_emit((int64_t)(uintptr_t)s,2,(coax_v_u8){NULL,0});
    pthread_mutex_lock(&hn_queue);hn_slots[id]=NULL;hn_running--;pthread_mutex_unlock(&hn_queue);free(s->url);free(s->message);free(s);
  }
  return id;
}
void rhn_socket_send(int32_t id,const void *bytes,uint32_t n) {
  if(id<0||id>=4)return;HnCommand *c=malloc(sizeof *c+(size_t)n);if(!c)return;
  *c=(HnCommand){.n=n};if(n)memcpy(c->bytes,bytes,n);
  pthread_mutex_lock(&hn_queue);HnSocket *s=hn_slots[id];
  if(s && coaxial_host_socket_send(id,s->generation,s->engine)) {if(s->tail)s->tail->next=c;else s->head=c;s->tail=c;c=NULL;}
  pthread_mutex_unlock(&hn_queue);free(c);
}
void rhn_socket_close(int32_t id) {
  if(id<0||id>=4)return;pthread_mutex_lock(&hn_queue);HnSocket *s=hn_slots[id];
  if(hn_owners[id].active && coaxial_host_socket_close(id,hn_owners[id].generation,hn_owners[id].engine)) {
    if(s)s->stop=1;hn_slots[id]=NULL;hn_owners[id].active=0;
    // Deferred main-thread delivery carries the old owner, even if reused.
    HnEvent *e=calloc(1,sizeof *e);if(e){e->id=id;e->event=2;e->generation=hn_owners[id].generation;e->engine=hn_owners[id].engine;if(hn_tail)hn_tail->next=e;else hn_head=e;hn_tail=e;}
  }
  pthread_mutex_unlock(&hn_queue);
}
static int64_t hn_now(void) {struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
void rhn_socket_poll(RealmHost *h) {
  if(!h)return;int64_t start=hn_now();int handled=0;
  while(htd_continue(handled,start,hn_now())) {
    pthread_mutex_lock(&hn_queue);HnEvent *e=hn_head;if(e){hn_head=e->next;if(!hn_head)hn_tail=NULL;}pthread_mutex_unlock(&hn_queue);if(!e)break;
    if(coaxial_host_socket_event(e->id,e->generation,e->engine,e->event,(int64_t)e->n,coaxial_inbox_cap())) {
      if(e->event==2 && hn_owners[e->id].generation==e->generation && hn_owners[e->id].engine==e->engine)hn_owners[e->id].active=0;
      rh_send(h,(int)coaxial_host_delivery(0),(const char *)e->bytes,(size_t)coaxial_host_delivery(1));
    }
    free(e);handled++;
  }
}
void rhn_socket_shutdown(void) {
  coaxial_host_transport_reset();pthread_mutex_lock(&hn_queue);
  for(int i=0;i<4;i++){hn_owners[i].active=0;if(hn_slots[i]){hn_slots[i]->stop=1;hn_slots[i]=NULL;}}
  while(hn_running)pthread_cond_wait(&hn_finished,&hn_queue);
  HnEvent *events=hn_head;hn_head=hn_tail=NULL;pthread_mutex_unlock(&hn_queue);
  while(events){HnEvent *next=events->next;free(events);events=next;}
}
