// Firm reason: 6 (fixed native worker threads, socket effects and leased byte storage).
// Coax owns RFC6455, UTF-8, fragmentation, queue limits and failure semantics.
#include "metal-sockets.h"
#include "HostNetwork.h"
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct MNContext MNContext;
typedef struct MNCommand {struct MNCommand *next;coax_t_HostInboxLease lease;size_t count;uint8_t bytes[];} MNCommand;
typedef struct MNEvent {struct MNEvent *next;MNContext *context;coax_t_HostInboxLease lease;MNSocketOwner owner;int64_t event;size_t count;uint8_t bytes[];} MNEvent;
typedef struct MNJob {MNContext *context;MNSocketOwner owner;char *url;size_t url_count;bool stop,failure,suppress;MNCommand *head,*tail,*active;} MNJob;
typedef struct {MNSocketOwner owner;int64_t first,last;} MNTerminal;
struct MNContext {
 coax_t_HostSocketQueuePlan plan;coax_t_HostInboxQueue incoming,outgoing;
 bool closed;size_t references;MNEvent *head,*tail;MNTerminal terminal[4];
 int64_t peak_events,peak_bytes,waits,emitted,delivered;
};
typedef struct {pthread_t thread;bool started;MNJob *current,*pending;} MNSlot;
static pthread_once_t mn_once=PTHREAD_ONCE_INIT;
static pthread_mutex_t mn_queue=PTHREAD_MUTEX_INITIALIZER,mn_io=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t mn_changed=PTHREAD_COND_INITIALIZER;
static MNSlot mn_slots[4];static bool mn_ready,mn_stopping;
static int64_t mn_contexts,mn_jobs,mn_running,mn_message_bytes;
static bool owner_equal(MNSocketOwner a,MNSocketOwner b){return a.id==b.id&&a.generation==b.generation&&a.engine==b.engine;}
static coax_v_t_HostInboxQueue incoming(MNContext *c){return(coax_v_t_HostInboxQueue){&c->incoming,1};}
static coax_v_t_HostInboxQueue outgoing(MNContext *c){return(coax_v_t_HostInboxQueue){&c->outgoing,1};}
static void unref(MNContext *c){if(--c->references==0){mn_contexts--;free(c);}}
static void command_free(MNJob *j,MNCommand *p){if(p){hiq_release(outgoing(j->context),p->lease);free(p);}}
static void clear_commands(MNJob *j){MNCommand *p=j->head;j->head=j->tail=NULL;while(p){MNCommand *next=p->next;command_free(j,p);p=next;}}
static void job_free(MNJob *j){if(!j)return;clear_commands(j);command_free(j,j->active);free(j->url);mn_jobs--;unref(j->context);free(j);}
static void terminal_failure(MNContext *c,MNSocketOwner o){coax_t_HostSocketSend f=hsq_failure();c->terminal[o.id]=(MNTerminal){o,f.error,f.close};}
static MNJob *job_for(MNContext *c,MNSocketOwner o){if(o.id<0||o.id>=4)return NULL;MNSlot *s=&mn_slots[o.id];if(s->pending&&s->pending->context==c&&owner_equal(s->pending->owner,o))return s->pending;if(s->current&&s->current->context==c&&owner_equal(s->current->owner,o))return s->current;return NULL;}
void hni_lock(void){pthread_mutex_lock(&mn_io);}
void hni_unlock(void){pthread_mutex_unlock(&mn_io);}
void hni_pause(int64_t ms){struct timespec t={ms/1000,(ms%1000)*1000000};nanosleep(&t,NULL);}
int64_t hni_take(int64_t context){MNJob *j=(void*)(uintptr_t)context;pthread_mutex_lock(&mn_queue);int64_t result=0;if(j->stop||j->context->closed||mn_stopping)result=2;else if(j->failure)result=3;else if(j->head){j->active=j->head;j->head=j->head->next;if(!j->head)j->tail=NULL;result=1;}pthread_mutex_unlock(&mn_queue);return result;}
int64_t hni_size(int64_t context){MNJob *j=(void*)(uintptr_t)context;return j->active?(int64_t)j->active->count:-1;}
int64_t hni_byte(int64_t context,int64_t at){MNJob *j=(void*)(uintptr_t)context;return j->active&&at>=0&&(uint64_t)at<j->active->count?j->active->bytes[at]:-1;}
void hni_done(int64_t context){MNJob *j=(void*)(uintptr_t)context;pthread_mutex_lock(&mn_queue);command_free(j,j->active);j->active=NULL;pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);}
void hni_emit(int64_t context,int64_t event,coax_v_u8 bytes){
 MNJob *j=(void*)(uintptr_t)context;MNContext *c=j->context;pthread_mutex_lock(&mn_queue);
 while(!c->closed&&!j->stop&&!j->suppress&&!mn_stopping){
  coax_t_HostInboxGrant g=hiq_reserve(incoming(c),bytes.count);
  if(g.status==COAX_HostInboxStatus_full){c->waits++;pthread_cond_wait(&mn_changed,&mn_queue);continue;}
  if(g.status!=COAX_HostInboxStatus_ok){terminal_failure(c,j->owner);j->stop=j->suppress=true;break;}
  MNEvent *p=malloc(sizeof*p+(size_t)bytes.count);
  if(!p){hiq_release(incoming(c),g.lease);terminal_failure(c,j->owner);j->stop=j->suppress=true;break;}
  *p=(MNEvent){.context=c,.lease=g.lease,.owner=j->owner,.event=event,.count=(size_t)bytes.count};
  if(bytes.count)memcpy(p->bytes,bytes.data,(size_t)bytes.count);c->references++;
  if(c->tail)c->tail->next=p;else c->head=p;c->tail=p;c->emitted++;
  if(c->incoming.count>c->peak_events)c->peak_events=c->incoming.count;
  if(c->incoming.bytes>c->peak_bytes)c->peak_bytes=c->incoming.bytes;break;
 }
 pthread_mutex_unlock(&mn_queue);
}
static void *worker(void *slot){
 MNSlot *s=slot;pthread_mutex_lock(&mn_queue);
 while(!mn_stopping){while(!mn_stopping&&!s->pending)pthread_cond_wait(&mn_changed,&mn_queue);if(mn_stopping)break;
  MNJob *j=s->pending;s->pending=NULL;s->current=j;mn_running++;
  size_t capacity=(size_t)j->context->plan.input_bytes;pthread_mutex_unlock(&mn_queue);
  uint8_t *message=malloc(capacity?capacity:1);
  if(message){pthread_mutex_lock(&mn_queue);mn_message_bytes+=(int64_t)capacity;pthread_mutex_unlock(&mn_queue);
   hns_worker((int64_t)(uintptr_t)j,(coax_string){j->url,(int64_t)j->url_count},(coax_v_u8){message,(int64_t)capacity});
   free(message);pthread_mutex_lock(&mn_queue);mn_message_bytes-=(int64_t)capacity;
  }else{pthread_mutex_lock(&mn_queue);if(!j->context->closed)terminal_failure(j->context,j->owner);}
  s->current=NULL;mn_running--;job_free(j);pthread_cond_broadcast(&mn_changed);
 }
 pthread_mutex_unlock(&mn_queue);return NULL;
}
static void initialize(void){
 pthread_attr_t attributes;pthread_attr_init(&attributes);pthread_attr_setstacksize(&attributes,16u*1024u*1024u);mn_ready=true;
 for(int i=0;i<4;i++){int error=pthread_create(&mn_slots[i].thread,&attributes,worker,&mn_slots[i]);mn_slots[i].started=!error;if(error)mn_ready=false;}
 pthread_attr_destroy(&attributes);
}
void *mn_open(int64_t capacity){
 coax_t_HostSocketQueuePlan p=hsq_plan(capacity);if(!p.valid||p.workers!=4)return NULL;pthread_once(&mn_once,initialize);
 pthread_mutex_lock(&mn_queue);if(!mn_ready||mn_stopping){pthread_mutex_unlock(&mn_queue);return NULL;}
 MNContext *c=calloc(1,sizeof*c);if(c){c->plan=p;c->incoming=hiq_open(p.input_bytes);c->outgoing=hiq_open(p.output_bytes);c->references=1;mn_contexts++;}pthread_mutex_unlock(&mn_queue);return c;
}
bool mn_start(void *context,MNSocketOwner o,const uint8_t *url,size_t count){
 MNContext *c=context;if(!c||o.id<0||o.id>=4||count>INT64_MAX||!hns_uri((coax_string){(const char*)url,(int64_t)count}))return false;
 pthread_mutex_lock(&mn_queue);if(c->closed||mn_stopping){pthread_mutex_unlock(&mn_queue);return false;}
 MNJob *j=calloc(1,sizeof*j);if(j)j->url=malloc(count+1);
 if(!j||!j->url){if(j)free(j);terminal_failure(c,o);pthread_mutex_unlock(&mn_queue);return false;}
 memcpy(j->url,url,count);j->url[count]=0;j->url_count=count;j->context=c;j->owner=o;c->references++;mn_jobs++;
 MNSlot *s=&mn_slots[o.id];if(s->current)s->current->stop=true;job_free(s->pending);s->pending=j;
 pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);return true;
}
bool mn_send(void *context,MNSocketOwner o,const uint8_t *bytes,size_t count){
 MNContext *c=context;if(!c)return false;pthread_mutex_lock(&mn_queue);MNJob *j=job_for(c,o);
 if(c->closed||!j||j->stop||j->failure){pthread_mutex_unlock(&mn_queue);return false;}
 coax_t_HostSocketSend g=hsq_send(outgoing(c),count<=INT64_MAX?(int64_t)count:-1);
 if(!g.accepted){j->failure=true;clear_commands(j);pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);return false;}
 MNCommand *p=malloc(sizeof*p+count);if(!p){hiq_release(outgoing(c),g.lease);j->failure=true;clear_commands(j);pthread_mutex_unlock(&mn_queue);return false;}
 *p=(MNCommand){.lease=g.lease,.count=count};if(count)memcpy(p->bytes,bytes,count);
 if(j->tail)j->tail->next=p;else j->head=p;j->tail=p;pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);return true;
}
void mn_socket_close(void *context,MNSocketOwner o){
 MNContext *c=context;if(!c||o.id<0||o.id>=4)return;pthread_mutex_lock(&mn_queue);
 if(!c->closed){MNJob *j=job_for(c,o);if(j){j->stop=true;clear_commands(j);}c->terminal[o.id]=(MNTerminal){o,hsq_failure().close,0};pthread_cond_broadcast(&mn_changed);}pthread_mutex_unlock(&mn_queue);
}
bool mn_next(void *context,MNSocketEvent *out){
 MNContext *c=context;*out=(MNSocketEvent){0};if(!c)return false;pthread_mutex_lock(&mn_queue);if(c->closed){pthread_mutex_unlock(&mn_queue);return false;}
 MNEvent *p=c->head;if(p){c->head=p->next;if(!c->head)c->tail=NULL;*out=(MNSocketEvent){p->owner,p->event,p->bytes,p->count,p};c->delivered++;pthread_mutex_unlock(&mn_queue);return true;}
 for(int i=0;i<4;i++)if(c->terminal[i].first){MNTerminal *t=&c->terminal[i];*out=(MNSocketEvent){.owner=t->owner,.event=t->first};t->first=t->last;t->last=0;c->delivered++;pthread_mutex_unlock(&mn_queue);return true;}
 pthread_mutex_unlock(&mn_queue);return false;
}
void mn_done(void *token){MNEvent *p=token;if(!p)return;pthread_mutex_lock(&mn_queue);MNContext *c=p->context;hiq_release(incoming(c),p->lease);free(p);unref(c);pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);}
void mn_close(void *context){
 MNContext *c=context;if(!c)return;pthread_mutex_lock(&mn_queue);c->closed=true;
 for(int i=0;i<4;i++){MNSlot *s=&mn_slots[i];if(s->current&&s->current->context==c){s->current->stop=true;clear_commands(s->current);}if(s->pending&&s->pending->context==c){job_free(s->pending);s->pending=NULL;}}
 MNEvent *p=c->head;c->head=c->tail=NULL;while(p){MNEvent *next=p->next;hiq_release(incoming(c),p->lease);free(p);unref(c);p=next;}unref(c);pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);
}
int64_t mn_info(void *context,int32_t field){MNContext *c=context;if(!c)return -1;pthread_mutex_lock(&mn_queue);int64_t result=-1;switch(field){case 0:result=c->incoming.count;break;case 1:result=c->incoming.bytes;break;case 2:result=c->outgoing.count;break;case 3:result=c->outgoing.bytes;break;case 4:result=c->peak_events;break;case 5:result=c->peak_bytes;break;case 6:result=c->waits;break;case 7:result=c->emitted;break;case 8:result=c->delivered;break;case 9:result=c->closed;break;}pthread_mutex_unlock(&mn_queue);return result;}
int64_t mn_pool_info(int32_t field){pthread_mutex_lock(&mn_queue);int64_t result=-1;switch(field){case 0:result=mn_contexts;break;case 1:result=mn_jobs;break;case 2:result=mn_running;break;case 3:result=mn_message_bytes;break;case 4:result=0;for(int i=0;i<4;i++)if(mn_slots[i].started)result++;break;}pthread_mutex_unlock(&mn_queue);return result;}
void mn_shutdown(void){pthread_mutex_lock(&mn_queue);mn_stopping=true;for(int i=0;i<4;i++){if(mn_slots[i].current)mn_slots[i].current->stop=true;job_free(mn_slots[i].pending);mn_slots[i].pending=NULL;}pthread_cond_broadcast(&mn_changed);pthread_mutex_unlock(&mn_queue);for(int i=0;i<4;i++)if(mn_slots[i].started){pthread_join(mn_slots[i].thread,NULL);pthread_mutex_lock(&mn_queue);mn_slots[i].started=false;pthread_mutex_unlock(&mn_queue);}}
