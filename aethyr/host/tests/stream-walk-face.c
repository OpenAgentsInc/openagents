// SDK effects for the shared streamed walker: concrete malloc/free and thin
// scalar ABI projections. All decoding, clocks and admission stay in Coax.
#ifndef SW_HEADER
#define SW_HEADER "stream-walk.h"
#endif
#include SW_HEADER
#include <stdint.h>
#include <stdbool.h>
#include <errno.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>
#ifdef __wasm__
#define SW_API(name) __attribute__((export_name(name)))
#else
#define SW_API(name)
#endif
typedef struct {
  coax_t_ResourceLoadKey key;
  uint8_t *source,*decoded,*work,*upload;
  coax_t_ResourcePlan *plan;
  coax_t_ResourceCostStatus status;
  int64_t encoded_ticket,pcm_ticket;
  bool occupied,finished,image_ticket;
} SwPhysical;
static coax_t_SwWalk state;
static coax_t_ResourceManifest manifest;
static coax_t_HostAudioMemory audio_memory;
static coax_t_HostImageMemory image_memory;
static SwPhysical objects[64];
static uint8_t configuration[32768],root_hex[64];
static size_t physical_bytes,physical_peak;
static coax_t_ResourceBorrowRef borrows[64];
static bool borrowed[64];
static float uniforms[64];
static coax_t_RfHandle fixed[16];
static bool fixed_live[16];
static coax_t_SwDraws drawing;
static int64_t physical_trace[1024][6];
static int32_t trace_n;
static bool trace_overflow;
static size_t failed_bytes;
static int allocation_errno;
static coax_v_t_SwWalk sv(void){return(coax_v_t_SwWalk){&state,1};}
static coax_v_t_ResourceManifest mv(void){return(coax_v_t_ResourceManifest){&manifest,1};}
static coax_v_t_HostAudioMemory av(void){return(coax_v_t_HostAudioMemory){&audio_memory,1};}
static coax_v_t_HostImageMemory iv(void){return(coax_v_t_HostImageMemory){&image_memory,1};}
static void observe(int32_t row,int32_t field,int32_t freed,size_t n){if(trace_n==1024){trace_overflow=true;return;}int64_t *event=physical_trace[trace_n++];event[0]=freed;event[1]=row;event[2]=objects[row].key.generation;event[3]=objects[row].key.epoch;event[4]=field;event[5]=(int64_t)n;}
static void *allocation(int32_t row,int32_t field,size_t n){if(!n)return NULL;void *p=calloc(1,n);if(p){physical_bytes+=n;if(physical_bytes>physical_peak)physical_peak=physical_bytes;observe(row,field,0,n);}else{failed_bytes=n;allocation_errno=errno;}return p;}
static void discard(int32_t row,int32_t field,void **p,size_t n){if(*p){free(*p);*p=NULL;physical_bytes-=n;observe(row,field,1,n);}}
static bool same(SwPhysical *p,int32_t row,int64_t generation,int64_t epoch){return row>=0&&row<64&&p->occupied&&p->key.generation==generation&&p->key.epoch==epoch;}
SW_API("sw_config_ptr") uintptr_t sw_config_ptr(void){return(uintptr_t)configuration;}
SW_API("sw_root_ptr") uintptr_t sw_root_ptr(void){return(uintptr_t)root_hex;}
SW_API("sw_host_auth") int32_t sw_host_auth(int32_t bytes){return bytes>=0&&bytes<=(int32_t)sizeof configuration&&sw_auth((coax_v_u8){configuration,(uint64_t)bytes},(coax_string){(char*)root_hex,64});}
SW_API("sw_host_start") int32_t sw_host_start(int32_t bytes,int32_t tier,int32_t backend){
  if(bytes<0||bytes>(int32_t)sizeof configuration||backend<0||backend>1)return 0;
  if(!state.valid)coax_default_t_SwWalk(&state);
  return sw_load_manifest(sv(),mv(),(coax_v_u8){configuration,(uint64_t)bytes},(coax_string){(char*)root_hex,64},tier,(coax_t_ResourceBackend)backend);
}
SW_API("sw_host_frame") int32_t sw_host_frame(int64_t ns,float direction){return sw_frame(sv(),mv(),ns,direction);}
SW_API("sw_host_ack") void sw_host_ack(void){sw_ack(sv());}
SW_API("sw_host_close") int32_t sw_host_close(void){return sw_close(sv(),mv());}
SW_API("sw_host_info") int64_t sw_host_info(int32_t field){switch(field){
  case 0:return state.tick;case 1:return state.load_n;case 2:return state.retire_n;case 3:return state.record_n;
  case 4:return state.active;case 5:return state.drawn;case 6:return state.updated;case 7:return state.voices;
  case 8:return state.stream.e[0].budget.e[0].total;case 9:return state.stream.e[0].budget.e[0].peak;
  case 10:return state.queue.e[0].bytes;case 11:return state.queue.e[0].count;
  case 12:return state.registry.e[0].live;case 13:return state.registry.e[0].words;
  case 14:return(int64_t)physical_bytes;case 15:return(int64_t)physical_peak;
  case 16:return state.blocked;case 17:return state.audio_deferred;case 18:return state.stream.e[0].refused;
  case 19:return state.cells.e[0].counts.e[5];case 20:return state.stream.e[0].requests;case 21:return state.stream.e[0].destroyed;
  case 22:return audio_memory.total*4;case 23:return image_memory.total;
  case 24:return state.stream.e[0].closing&&state.stream.e[0].destroyed==state.stream.e[0].requests;
  case 25:return state.peak_active;case 26:return state.player.clock.frames;
  case 27:return sizeof state+sizeof manifest+sizeof audio_memory+sizeof image_memory+sizeof objects+sizeof configuration+sizeof root_hex+sizeof borrows+sizeof borrowed+sizeof uniforms+sizeof fixed+sizeof fixed_live+sizeof drawing+sizeof physical_bytes+sizeof physical_peak+sizeof physical_trace+sizeof trace_n+sizeof trace_overflow;
  default:return -1;}}
SW_API("sw_host_player") float sw_host_player(int32_t field){switch(field){case 0:return state.player.pos.x;case 1:return state.player.pos.y;case 2:return state.player.pos.z;case 3:return state.player.theta;case 4:return state.player.phi;case 5:return state.player.dist;case 6:return state.player.target.x;case 7:return state.player.target.y;case 8:return state.player.target.z;default:return 0;}}
SW_API("sw_host_command") int64_t sw_host_command(int32_t retiring,int32_t index,int32_t field){
  int64_t n=retiring?state.retire_n:state.load_n;if(index<0||index>=n)return -1;
  coax_t_ResourceLoadKey k=retiring?state.retires.e[index]:state.loads.e[index];
  if(field==0)return k.owner;if(field==1)return k.generation;if(field==2)return k.epoch;if(field>=3&&field<11)return k.content.words.e[field-3];return -1;
}
SW_API("sw_host_record") int64_t sw_host_record(int32_t index,int32_t field){
  if(index<0||index>=state.record_n)return -1;coax_t_SwRecord r=state.records.e[index];
  switch(field){case 0:return r.tick;case 1:return r.kind;case 2:return r.key.owner;case 3:return r.key.generation;case 4:return r.key.epoch;case 5:return r.decoded;case 6:return r.status;}
  if(field>=7&&field<15)return r.key.content.words.e[field-7];if(field>=15&&field<23)return r.source.words.e[field-15];return -1;
}
SW_API("sw_host_row") int64_t sw_host_row(int32_t row,int32_t field){
  if(row<0||row>=64)return -1;coax_t_ResourcePlan p=manifest.entries.e[row].plan;coax_t_ResourceOwner o=state.stream.e[0].owners.e[row];
  switch(field){case 0:return p.spec.format;case 1:return p.cost.encoded;case 2:return p.cost.decoded;case 3:return p.cost.gpu;case 4:return p.cost.width;case 5:return p.cost.height;case 6:return p.cost.levels;case 7:return p.cost.elements;case 8:return o.phase;case 9:return state.stream.e[0].wanted.e[row];case 10:return o.generation;case 11:return o.admission.e[0].gpu_handle.slot;case 12:return o.admission.e[0].gpu_handle.generation;case 13:return p.cost.decode_scratch;case 14:return p.cost.upload_scratch;case 15:return p.spec.category;default:return -1;}
}
SW_API("sw_host_allocate") int32_t sw_host_allocate(int32_t row,int64_t generation,int64_t epoch){
  if(row<0||row>=64||objects[row].occupied)return 0;
  coax_t_ResourceOwner owner=state.stream.e[0].owners.e[row];
  if(owner.phase!=COAX_ResourceOwnerPhase_loading||owner.generation!=generation||state.stream.e[0].epoch!=epoch)return 0;
  SwPhysical *p=&objects[row];memset(p,0,sizeof *p);p->occupied=true;p->key=(coax_t_ResourceLoadKey){.owner=row,.generation=generation,.epoch=epoch,.content=owner.content};p->encoded_ticket=p->pcm_ticket=-1;
  coax_t_ResourcePlan plan=manifest.entries.e[row].plan;p->status=COAX_ResourceCostStatus_capacity;
  if(plan.spec.format==COAX_ResourceFormat_wav_pcm16){
    p->encoded_ticket=ham_reserve(av(),hae_words(plan.cost.encoded,false));if(p->encoded_ticket<0)return 0;
    p->pcm_ticket=ham_reserve(av(),plan.cost.decoded/4);if(p->pcm_ticket<0)return 0;
  }
  if(plan.spec.format==COAX_ResourceFormat_image_mips){p->image_ticket=hic_reserve(iv(),row,plan.cost.decoded);if(!p->image_ticket)return 0;}
  p->source=allocation(row,0,(size_t)plan.cost.encoded);p->decoded=allocation(row,1,(size_t)plan.cost.decoded);p->plan=allocation(row,2,sizeof plan);
  p->work=allocation(row,3,(size_t)plan.cost.decode_scratch);p->upload=allocation(row,4,(size_t)plan.cost.upload_scratch);
  if(!p->source||!p->plan||(plan.cost.decoded&&!p->decoded)||(plan.cost.decode_scratch&&!p->work)||(plan.cost.upload_scratch&&!p->upload))return 0;
  *p->plan=plan;return 1;
}
SW_API("sw_host_pointer") uintptr_t sw_host_pointer(int32_t row,int32_t field){if(row<0||row>=64)return 0;SwPhysical *p=&objects[row];switch(field){case 0:return(uintptr_t)p->source;case 1:return(uintptr_t)p->decoded;case 2:return(uintptr_t)p->work;case 3:return(uintptr_t)p->upload;case 4:return(uintptr_t)p->plan;default:return 0;}}
SW_API("sw_host_decode") int32_t sw_host_decode(int32_t row,int64_t generation,int64_t epoch){
  if(row<0||row>=64)return COAX_ResourceCostStatus_invalid;
  SwPhysical *p=&objects[row];if(!same(p,row,generation,epoch)||!p->plan||!p->source)return COAX_ResourceCostStatus_capacity;
  coax_t_ResourceCost c=p->plan->cost;
  p->status=sw_codec(*p->plan,(coax_v_u8){p->source,c.encoded},(coax_v_u8){p->work,c.decode_scratch},(coax_v_u8){p->decoded,c.decoded},(coax_v_f32){(float*)p->decoded,c.decoded/4},(coax_v_t_CellBody){(coax_t_CellBody*)p->decoded,c.decoded/sizeof(coax_t_CellBody)});
  return p->status;
}
SW_API("sw_host_finish") int32_t sw_host_finish(int32_t row,int64_t generation,int64_t epoch,int32_t status){
  if(row<0||row>=64)return 0;SwPhysical *p=&objects[row];if(!same(p,row,generation,epoch)||p->finished)return 0;
  coax_t_ResourceCost c=manifest.entries.e[row].plan.cost;
  discard(row,3,(void**)&p->work,(size_t)c.decode_scratch);discard(row,4,(void**)&p->upload,(size_t)c.upload_scratch);
  if(status!=0)p->status=COAX_ResourceCostStatus_decode;
  coax_t_SwFact f;coax_default_t_SwFact(&f);f.key=p->key;f.decoded=p->status;
  f.source=(coax_v_u8){p->source,p->source?(uint64_t)c.encoded:0};f.values=(coax_v_f32){(float*)p->decoded,p->decoded?(uint64_t)c.decoded/4:0};f.bodies=(coax_v_t_CellBody){(coax_t_CellBody*)p->decoded,p->decoded?(uint64_t)c.decoded/sizeof(coax_t_CellBody):0};
  p->finished=sw_offer(sv(),f);return p->finished;
}
SW_API("sw_host_destroy") int32_t sw_host_destroy(int32_t row,int64_t generation,int64_t epoch){
  if(row<0||row>=64)return 0;SwPhysical *p=&objects[row];if(!same(p,row,generation,epoch))return 0;
  coax_t_ResourceCost c=manifest.entries.e[row].plan.cost;
  discard(row,0,(void**)&p->source,(size_t)c.encoded);discard(row,1,(void**)&p->decoded,(size_t)c.decoded);discard(row,2,(void**)&p->plan,sizeof(coax_t_ResourcePlan));discard(row,3,(void**)&p->work,(size_t)c.decode_scratch);discard(row,4,(void**)&p->upload,(size_t)c.upload_scratch);
  if(p->encoded_ticket>=0){ham_release(av(),p->encoded_ticket);p->encoded_ticket=-1;}if(p->pcm_ticket>=0){ham_release(av(),p->pcm_ticket);p->pcm_ticket=-1;}if(p->image_ticket){hic_release(iv(),row);p->image_ticket=false;}
  coax_t_SwFact f;coax_default_t_SwFact(&f);f.kind=COAX_SwFactKind_destroyed;f.key=p->key;
  if(!sw_offer(sv(),f))return 0;p->occupied=false;return 1;
}
SW_API("sw_host_borrow") int32_t sw_host_borrow(int32_t row){if(row<0||row>=64||borrowed[row])return 0;borrows[row]=sw_borrow(sv(),row);borrowed[row]=borrows[row].slot>=0;return borrowed[row];}
SW_API("sw_host_return") int32_t sw_host_return(int32_t row){if(row<0||row>=64||!borrowed[row])return 0;bool ok=sw_return(sv(),mv(),borrows[row]);if(ok)borrowed[row]=false;return ok;}
SW_API("sw_host_cell") int64_t sw_host_cell(int32_t cell,int32_t field){if(cell<0||cell>=12)return -1;switch(field){case 0:return state.cells.e[0].cells.e[cell].level;case 1:return state.actor.e[cell].id;case 2:return state.actor.e[cell].words.e[0];case 3:return state.entity.e[0].ids.e[cell];case 4:return state.nav_walk.e[cell];default:return -1;}}
SW_API("sw_host_uniforms") uintptr_t sw_host_uniforms(int32_t cell,float aspect){return sw_uniforms(sv(),cell,aspect,(coax_v_f32){uniforms,64})?(uintptr_t)uniforms:0;}
SW_API("sw_host_gpu_live") int32_t sw_host_gpu_live(int32_t row){return sw_gpu_live(sv(),row);}
SW_API("sw_host_fixed") int32_t sw_host_fixed(int32_t index,int32_t kind,int64_t words){if(index<0||index>=16||fixed_live[index])return 0;coax_t_RrGrant g=sw_gpu_acquire(sv(),kind,words);if(g.status!=COAX_RrStatus_ok)return 0;fixed[index]=g.handle;fixed_live[index]=true;return 1;}
SW_API("sw_host_fixed_release") int32_t sw_host_fixed_release(int32_t index){if(index<0||index>=16||!fixed_live[index])return 0;if(sw_gpu_release(sv(),fixed[index])!=COAX_RrStatus_ok)return 0;fixed_live[index]=false;return 1;}
SW_API("sw_host_draw_count") int32_t sw_host_draw_count(void){drawing=sw_draws(sv(),mv());return drawing.count;}
SW_API("sw_host_draw") int64_t sw_host_draw(int32_t index,int32_t field){if(index<0||index>=drawing.count)return -1;coax_t_SwDraw d=drawing.rows.e[index];switch(field){case 0:return d.geometry;case 1:return d.indices;case 2:return d.texture;case 3:return d.lighting;case 4:return d.cell;case 5:return d.count;case 6:return d.fallback;default:return -1;}}
SW_API("sw_host_trace_count") int32_t sw_host_trace_count(void){return trace_overflow?-1:trace_n;}
SW_API("sw_host_trace") int64_t sw_host_trace(int32_t index,int32_t field){if(index<0||index>=trace_n||field<0||field>=6)return -1;return physical_trace[index][field];}
SW_API("sw_host_trace_ack") void sw_host_trace_ack(void){trace_n=0;trace_overflow=false;}
SW_API("sw_host_allocator") int64_t sw_host_allocator(int32_t field){
  if(field==0)return failed_bytes;if(field==1)return allocation_errno;
#ifdef __wasm__
  extern unsigned char __heap_base;
  if(field==2)return(uintptr_t)&__heap_base;if(field==3)return(uint64_t)__builtin_wasm_memory_size(0)*65536;if(field==4)return(uintptr_t)sbrk(0);
#endif
  return -1;
}
// Native tool conveniences preserve Coax view ABI and copy only into the
// already granted physical allocation. They make no admission decisions.
bool sw_sdk_start(coax_v_u8 source,coax_v_u8 root,int64_t tier,int64_t backend){
  if(source.count<0||(uint64_t)source.count>sizeof configuration||root.count!=64||tier<0||tier>1||backend<0||backend>1)return false;
  memcpy(configuration,source.data,(size_t)source.count);memcpy(root_hex,root.data,64);
  return sw_host_start((int32_t)source.count,(int32_t)tier,(int32_t)backend)!=0;
}
bool sw_sdk_copy(int64_t row,int64_t generation,int64_t epoch,coax_v_u8 source){
  if(row<0||row>=64)return false;SwPhysical *p=&objects[row];
  if(!same(p,(int32_t)row,generation,epoch)||!p->source||!p->plan||source.count!=p->plan->cost.encoded)return false;
  memcpy(p->source,source.data,(size_t)source.count);return true;
}
#ifdef COAX_coax_t_SwRecoveryReport_DEFINED
static coax_t_SwRecoveryReport recovery_report;
SW_API("sw_host_recover") int32_t sw_host_recover(void){
  recovery_report=sw_recover_gpu(sv(),mv());
  if(recovery_report.status!=COAX_SwRecoveryStatus_ok)return 0;
  memset(fixed_live,0,sizeof fixed_live);return 1;
}
SW_API("sw_host_recover_info") int64_t sw_host_recover_info(int32_t field){
  switch(field){case 0:return recovery_report.status;case 1:return recovery_report.restored;case 2:return recovery_report.epoch;default:return -1;}
}
// The SDK has ended this attempt's decode/upload and retained its exact
// source/decoded spans. Readiness publication still belongs to Coax.
SW_API("sw_host_uploadable") int32_t sw_host_uploadable(int32_t row){
  if(row<0||row>=64)return 0;SwPhysical *p=&objects[row];
  return p->occupied&&p->finished&&p->status==COAX_ResourceCostStatus_ok&&p->plan&&sw_gpu_live(sv(),row);
}
#endif
