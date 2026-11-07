// SDK projections for the additive HTP1 carrier; Coax admits every binding.
#define SW_TRANSPORT 1
#define SW_PRESENTATION 1
#define SW_LOSS_POLICY 1
#define SW_HEADER "stream-walk-transport.h"
#include "stream-walk-face.c"
static coax_t_StreamTransport transport_state;
static uint8_t transport_input[128];
static coax_t_DiagnosticTrace diagnostic_trace;
static coax_t_HostCapturePlan capture_cost;
SW_API("sw_capture_plan") int32_t sw_capture_plan(int64_t width,int64_t height){capture_cost=capture_plan(width,height);return capture_cost.valid;}
SW_API("sw_capture_info") int64_t sw_capture_info(int32_t field){switch(field){case 0:return capture_cost.row_bytes;case 1:return capture_cost.source;case 2:return capture_cost.copy;case 3:return capture_cost.encoded;case 4:return capture_cost.utf16_peak;default:return -1;}}
SW_API("sw_capture_observed") int32_t sw_capture_observed(int64_t source,int64_t encoded,int64_t text){return capture_observed(capture_cost,source,encoded,text);}
static coax_v_t_DiagnosticTrace dv(void){return(coax_v_t_DiagnosticTrace){&diagnostic_trace,1};}
SW_API("sw_trace_open") int64_t sw_trace_open(int64_t capacity){return dt_open(dv(),capacity)?dt_plan(capacity).bytes:0;}
SW_API("sw_trace_offer") int64_t sw_trace_offer(void){return dt_offer(dv());}
SW_API("sw_trace_close") int32_t sw_trace_close(void){return dt_close(dv());}
SW_API("sw_trace_info") int64_t sw_trace_info(int32_t field){switch(field){case 0:return diagnostic_trace.capacity;case 1:return diagnostic_trace.count;case 2:return diagnostic_trace.dropped;case 3:return diagnostic_trace.exhausted;case 4:return dt_complete(dv());case 5:return dt_plan(diagnostic_trace.capacity).reserved;default:return -1;}}
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
SW_API("sw_transport_info") int64_t sw_transport_info(int32_t field){switch(field){case 0:return transport_state.phase;case 1:return sizeof transport_state+sizeof transport_input+sizeof diagnostic_trace+sizeof capture_cost;case 2:return transport_state.manifest.e[0].count;default:return -1;}}
