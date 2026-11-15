// Firm reason: 1 (flat browser ABI and shared socket/HTTP SDK imports).
#include "messages.h"
#include <string.h>
#define EXPORT(n) __attribute__((export_name(n)))
#define IMPORT(n) __attribute__((import_module("env"),import_name(n)))
IMPORT("coaxial_socket_open") int32_t socket_open(uint32_t p,uint32_t n);
IMPORT("coaxial_socket_send") void socket_send(int32_t id,uint32_t p,uint32_t n);
IMPORT("coaxial_socket_close") void socket_close(int32_t id);
IMPORT("coaxial_random") uint32_t coaxial_random(uint32_t out,int32_t length);
IMPORT("coaxial_now_seconds") int64_t now_seconds(void);
IMPORT("dom_stream") void dom_stream(uint32_t p,uint32_t n,uint32_t body,uint32_t len);
IMPORT("dom_go") void dom_go(uint32_t p,uint32_t n,uint32_t push);
#define COAX_ENTROPY_HEADER "messages.h"
#include "../../coax/platform/entropy-wasm.c"
static coax_t_MessagesWeb *raw(uint32_t s){return (void *)(uintptr_t)s;}
static coax_v_t_MessagesWeb at(uint32_t s){return (coax_v_t_MessagesWeb){raw(s),1};}
static void drain(uint32_t s){
 coax_t_MessagesWeb *a=raw(s);
 if(a->need_entropy){uint8_t bytes[32]={0};if(coaxial_random((uintptr_t)bytes,32)==0)wm_random(at(s),(coax_v_u8){bytes,32},now_seconds());memset(bytes,0,sizeof bytes);}coax_t_Pipe *p=&a->carrier.e[0].pipe.e[0];
 for(int64_t i=0;i<p->n;i++){
  uint32_t b=(uint32_t)(uintptr_t)&p->buf.e[p->from.e[i]],n=(uint32_t)p->len.e[i];
  if(p->op.e[i]==1)wm_opened(at(s),p->id.e[i],socket_open(b,n));
  else if(p->op.e[i]==2)socket_send((int32_t)p->id.e[i],b,n);
  else if(p->op.e[i]==3)socket_close((int32_t)p->id.e[i]);
 }p->n=0;p->used=0;
 if(a->asking){a->asking=0;dom_stream((uint32_t)(uintptr_t)a->path.e,(uint32_t)a->path_n,(uint32_t)(uintptr_t)a->request.e,(uint32_t)a->request_n);}
 if(a->go_n>0){uint32_t n=(uint32_t)a->go_n;a->go_n=0;dom_go((uint32_t)(uintptr_t)a->go.e,n,0);}
}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_MessagesWeb);}
EXPORT("coaxial_state_init") void state_init(uint32_t s){coax_default_t_MessagesWeb(raw(s));}
EXPORT("coaxial_socket_slots") uint32_t socket_slots(void){return 6;}
EXPORT("coaxial_socket_text_limit") uint32_t text_limit(void){return 131072;}
EXPORT("coaxial_socket_pause_background") uint32_t pause_background(void){return 1;}
EXPORT("dom_frame") int32_t frame(uint32_t s,float dt,uint32_t reduced){int32_t n=(int32_t)wm_frame(at(s),dt,reduced!=0,now_seconds());drain(s);return n;}
EXPORT("dom_out") uint32_t output(uint32_t s){return (uintptr_t)raw(s)->dom.e[0].out.e;}
EXPORT("dom_busy") uint32_t busy(uint32_t s){return raw(s)->dirty||raw(s)->ui.e[0].busy||raw(s)->messenger.e[0].active;}
EXPORT("dom_inbox") uint32_t inbox(uint32_t s){return (uintptr_t)raw(s)->inbox.e;}
EXPORT("dom_inbox_cap") uint32_t cap(void){return 131072;}
EXPORT("dom_event") void event(uint32_t s,int32_t k,int32_t slot,int32_t n){wm_event(at(s),k,slot,n);drain(s);}
EXPORT("dom_size") void size(uint32_t s,int32_t slot,float w,float h){wm_size(at(s),slot,w,h);}
EXPORT("dom_clip") uint32_t clip(uint32_t s){return (uintptr_t)raw(s)->clip.e;}
EXPORT("dom_clip_len") uint32_t clip_len(uint32_t s){return (uint32_t)raw(s)->clip_n;}
EXPORT("dom_clip_done") void clip_done(uint32_t s){raw(s)->clip_n=0;}
