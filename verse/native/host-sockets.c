// Firm reason: 5 (thin realm/transport projection over the shared native worker SDK).
// Linux and Metal compile the same portable workers. Coax owns protocol,
// queue limits, callback ownership and bounded main-thread drains.
#define _GNU_SOURCE
#include "HostNetwork.h"
#include "realm-host.h"
#include "face.h"
#include "metal-sockets.h"
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
extern int64_t htr_http_target(coax_string,coax_v_u8) __attribute__((weak));
extern int64_t htr_local_target(coax_string,coax_string,coax_v_u8) __attribute__((weak));
extern int64_t hfj_attach(int64_t,coax_string,coax_string,coax_v_u8) __attribute__((weak));
extern bool hap_span(uint64_t,uint64_t,uint64_t,uint64_t,bool) __attribute__((weak));
int rhn_action_span(uint64_t address,uint64_t capacity,uint64_t bytes,uint64_t limit,int present){return hap_span&&hap_span(address,capacity,bytes,limit,present!=0);}
int64_t rhn_http_image_body(int32_t flags,const char *body,size_t bytes,const uint8_t *image,size_t image_bytes,uint8_t *out,size_t capacity){
 if(!hfj_attach||bytes>INT64_MAX||image_bytes>INT64_MAX||capacity>INT64_MAX)return -1;
 return hfj_attach(flags,(coax_string){body,(int64_t)bytes},(coax_string){(const char*)image,(int64_t)image_bytes},(coax_v_u8){out,(int64_t)capacity});
}
static void *context;
static struct {MNSocketOwner owner;int active;} owners[4];
int rhn_http_path(const char *bytes,size_t n){uint8_t target[12288];return n<=INT64_MAX&&htr_http_target&&htr_http_target((coax_string){bytes,(int64_t)n},(coax_v_u8){target,sizeof target})>=0;}
int rhn_utf8(const char *bytes,size_t n){return n<=INT64_MAX&&hns_utf8((coax_string){bytes,(int64_t)n});}
int32_t rhn_socket_open(const char *url,uint32_t n){
 uint8_t target[4096];if(!mn_uri((const uint8_t*)url,n)){
  const char *bound=getenv("VERSE_NATIVE_LOCAL_ROOM");if(!bound||!htr_local_target)return -1;
  int64_t count=htr_local_target((coax_string){url,n},(coax_string){bound,(int64_t)strlen(bound)},(coax_v_u8){target,sizeof target});
  if(count<0)return -1;url=(const char*)target;n=(uint32_t)count;
 }
 signal(SIGPIPE,SIG_IGN);if(!context)context=mn_open(coaxial_inbox_cap());if(!context)return -1;
 int32_t id=coaxial_host_socket_reserve();if(id<0)return -1;
 MNSocketOwner o={id,coaxial_host_owner(1),coaxial_host_owner(2)};owners[id].owner=o;owners[id].active=1;
 (void)mn_start(context,o,(const uint8_t*)url,n);return id;
}
void rhn_socket_send(int32_t id,const void *bytes,uint32_t n){
 if(id<0||id>=4||!context||!owners[id].active)return;MNSocketOwner o=owners[id].owner;
 if(coaxial_host_socket_send(id,o.generation,o.engine))(void)mn_send(context,o,bytes,n);
}
void rhn_socket_close(int32_t id){
 if(id<0||id>=4||!context||!owners[id].active)return;MNSocketOwner o=owners[id].owner;
 if(coaxial_host_socket_close(id,o.generation,o.engine)){owners[id].active=0;mn_socket_close(context,o);}
}
static int64_t now_ms(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
void rhn_socket_poll(RealmHost *h){
 if(!h||!context)return;void *current=context;int64_t start=now_ms();int handled=0;
 while(context==current&&mn_poll_more(handled,start,now_ms())){
  MNSocketEvent e;if(!mn_next(current,&e))break;
  if(coaxial_host_socket_event((int32_t)e.owner.id,e.owner.generation,e.owner.engine,(int32_t)e.event,(int64_t)e.count,coaxial_inbox_cap())){
   if(e.event==2&&owners[e.owner.id].owner.generation==e.owner.generation&&owners[e.owner.id].owner.engine==e.owner.engine)owners[e.owner.id].active=0;
   rh_send(h,(int)coaxial_host_delivery(0),(const char*)e.bytes,(size_t)coaxial_host_delivery(1));
  }mn_done(e.token);handled++;
 }
}
int64_t rhn_socket_queue_info(int32_t field){return context?mn_info(context,field):0;}
int64_t rhn_socket_pool_info(int32_t field){return mn_pool_info(field);}
void rhn_socket_shutdown(void){coaxial_host_transport_reset();for(int i=0;i<4;i++)owners[i].active=0;if(context){void *old=context;context=NULL;mn_close(old);}}
// The pool is process-owned; engine replacement retires only its context.
void rhn_socket_pool_shutdown(void){if(context)rhn_socket_shutdown();mn_shutdown();}
__attribute__((destructor)) static void pool_close(void){rhn_socket_pool_shutdown();}
