// SDK projections for the additive HTP1 carrier; Coax admits every binding.
#define SW_TRANSPORT 1
#define SW_PRESENTATION 1
#define SW_LOSS_POLICY 1
#define SW_HEADER "stream-walk-transport.h"
#include "stream-walk-face.c"
static coax_t_StreamTransport transport_state;
static uint8_t transport_input[128];
static coax_v_t_StreamTransport tv(void){return(coax_v_t_StreamTransport){&transport_state,1};}
static coax_t_ResourceLoadKey tk(int32_t row,int64_t generation,int64_t epoch){
  coax_t_ResourceLoadKey k;coax_default_t_ResourceLoadKey(&k);
  k.owner=row;k.generation=generation;k.epoch=epoch;
  if(row>=0&&row<64)k.content=manifest.entries.e[row].plan.content;return k;
}
static bool sw_transport_live_key(int32_t row,int64_t generation,int64_t epoch){return st_live(tv(),tk(row,generation,epoch));}
static bool sw_transport_checked_key(int32_t row,int64_t generation,int64_t epoch){return st_checked(tv(),tk(row,generation,epoch));}
static bool sw_transport_idle_row(int32_t row){return st_idle(tv(),row);}
static bool sw_transport_drained(void){return st_drained(tv());}
SW_API("sw_transport_input") uintptr_t sw_transport_input(void){return(uintptr_t)transport_input;}
SW_API("sw_transport_scene") int32_t sw_transport_scene(int32_t bytes){
  return bytes>=0&&bytes<=(int32_t)sizeof configuration&&st_scene(tv(),(coax_string){(char*)configuration,(uint64_t)bytes},(coax_string){(char*)root_hex,64});
}
SW_API("sw_transport_open") int32_t sw_transport_open(int32_t bytes){
  return bytes>=0&&bytes<=(int32_t)sizeof configuration&&st_open(tv(),manifest,(coax_v_u8){configuration,(uint64_t)bytes},(coax_string){(char*)root_hex,64});
}
SW_API("sw_transport_begin") int32_t sw_transport_begin(int32_t row,int64_t generation,int64_t epoch,int64_t offset,int64_t bytes,int64_t raw){
  if(row<0||row>=64)return 0;
  return st_begin_hex(tv(),state.stream.e[0].owners.e[row],state.stream.e[0].epoch,state.stream.e[0].wanted.e[row],tk(row,generation,epoch),(coax_string){(char*)transport_input,64},(coax_string){(char*)transport_input+64,64},offset,bytes,raw);
}
SW_API("sw_transport_source") int32_t sw_transport_source(int32_t row,int64_t generation,int64_t epoch,int32_t bytes){
  return bytes>=0&&bytes<=(int32_t)sizeof configuration&&st_source(tv(),manifest,tk(row,generation,epoch),(coax_v_u8){configuration,(uint64_t)bytes});
}
SW_API("sw_transport_end") int32_t sw_transport_end(int32_t row,int64_t generation,int64_t epoch,int32_t joined,int32_t success){return st_end(tv(),tk(row,generation,epoch),joined!=0,success!=0);}
SW_API("sw_transport_close") int32_t sw_transport_close(void){return st_close(tv());}
SW_API("sw_transport_row") int64_t sw_transport_row(int32_t row,int32_t field){
  if(row<0||row>=transport_state.manifest.e[0].count)return -1;
  coax_t_HptSource p=transport_state.manifest.e[0].entries.e[row];
  switch(field){case 0:return p.raw;case 1:return p.container_bytes;case 2:return p.offset;case 3:return p.bytes;case 4:return transport_state.runs.e[row].live;case 5:return transport_state.runs.e[row].checked;default:return -1;}
}
SW_API("sw_transport_info") int64_t sw_transport_info(int32_t field){switch(field){case 0:return transport_state.phase;case 1:return sizeof transport_state+sizeof transport_input;case 2:return transport_state.manifest.e[0].count;default:return -1;}}
