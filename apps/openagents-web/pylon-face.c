/* Byte/GPU effects only. Original event bytes reach the shared Coax
   verifier before projection. No public export accepts an admitted flag. */
#include "pylon.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"),import_name(name)))
IMPORT("coaxial_socket_open") int32_t socket_open(uint32_t p,uint32_t n);
IMPORT("coaxial_socket_send") void socket_send(int32_t id,uint32_t p,uint32_t n);
IMPORT("coaxial_socket_close") void socket_close(int32_t id);
extern int64_t lr_beacon(coax_string,int64_t,coax_v_u8);
extern _Bool lr_verify_public(coax_string);
extern int64_t lr_npub(coax_string,coax_v_u8);
static uint32_t golden_mode;
static unsigned char admitted[270336],npub[64];
static coax_t_Pylon *s(uint32_t p){return (coax_t_Pylon *)(uintptr_t)p;}
static coax_v_t_Pylon at(uint32_t p){return (coax_v_t_Pylon){s(p),1};}
static void drain(uint32_t p){
  coax_t_Pipe *q=&s(p)->pipe.e[0];
  if(!golden_mode) for(int64_t i=0;i<q->n;i++){
    uint32_t a=(uint32_t)(uintptr_t)(q->buf.e+q->from.e[i]),n=(uint32_t)q->len.e[i];
    if(q->op.e[i]==1)s(p)->socket=socket_open(a,n);
    else if(q->op.e[i]==2)socket_send((int32_t)q->id.e[i],a,n);
    else if(q->op.e[i]==3)socket_close((int32_t)q->id.e[i]);
  }
  q->n=0;q->used=0;
}
EXPORT("coaxial_golden") void golden(uint32_t on){golden_mode=on!=0;}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_Pylon)*1000003u+0x50594c4eu;}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_Pylon);}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return 300000;}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return 16;}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return 16;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){
  int64_t n=pl_frame(at(p),(coax_v_f32){(float *)(uintptr_t)v,300000*16},(coax_v_f32){(float *)(uintptr_t)u,16},w,h,d,dt,still||golden_mode);drain(p);return n>0?(uint32_t)n:0;
}
EXPORT("coaxial_inbox") uint32_t inbox(uint32_t p){return (uint32_t)(uintptr_t)s(p)->inbox.e;}
EXPORT("coaxial_inbox_cap") uint32_t inbox_cap(uint32_t p){(void)p;return 270336;}
EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,int32_t n){
  if(golden_mode&&kind>=256&&kind<272)return;
  int64_t k=pl_receive(at(p),kind,n);
  coax_string original={(const char *)s(p)->inbox.e+s(p)->pending_from,s(p)->pending_n};
  if(k==30200){
    int64_t count=lr_beacon(original,pl_now(at(p)),(coax_v_u8){admitted,sizeof admitted});
    if(count>=0){
      coax_string content={(const char *)admitted,count};
      int64_t nn=lr_npub(pl_provider(content),(coax_v_u8){npub,sizeof npub});
      if(nn>=0)pl_observed(at(p),content,(coax_string){(const char *)npub,nn});
    }else s(p)->refused++;
  }else if(k==26910||k==27010){
    if(lr_verify_public(original))pl_activity(at(p),original);else s(p)->refused++;
  }
  drain(p);
}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){(void)kind;(void)id;return pl_pointer(at(p),x,y,phase);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){pl_wheel(at(p),dy);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t key,int32_t mods){(void)p;(void)key;(void)mods;return 0;}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p){(void)p;return 1;}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)s(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)s(p)->route.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p){return (uint32_t)s(p)->route_n;}
EXPORT("coaxial_action_done") void action_done(uint32_t p){s(p)->action=0;}
/* Observation-only probe ABI for end-to-end page tests. */
EXPORT("coaxial_pylon_count") int32_t count(uint32_t p){return (int32_t)s(p)->count;}
EXPORT("coaxial_pylon_answers") int32_t answers(uint32_t p){return (int32_t)s(p)->answers;}
EXPORT("coaxial_pylon_statuses") int32_t statuses(uint32_t p){return (int32_t)s(p)->statuses;}
EXPORT("coaxial_pylon_refused") int32_t refused(uint32_t p){return (int32_t)s(p)->refused;}
