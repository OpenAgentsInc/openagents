// Firm reason: 1 (browser SDK: concrete Wasm storage and scalar checked pack projections).
#include "pack-carrier.h"
#include <stdbool.h>
#include <string.h>
#define PACK_EXPORT(name) __attribute__((export_name(name)))
extern unsigned char __heap_base;
static coax_t_HostInboxQueue storage;
static coax_t_HostInboxGrant ticket;
static coax_t_HostPackStream stream;
static coax_t_HostPackChunk chunk;
static coax_t_HostPackAllocation allocation;
static coax_t_HostPrivatePack private_plan;
static coax_t_PackRead pack;
static uint8_t prefix[12],metadata[4096];
static coax_v_t_HostInboxQueue storage_view(void){return(coax_v_t_HostInboxQueue){&storage,1};}
static coax_v_t_HostPackStream stream_view(void){return(coax_v_t_HostPackStream){&stream,1};}
PACK_EXPORT("pack_storage_open") void pack_storage_open(void){storage=hpk_storage_open();}
PACK_EXPORT("pack_storage_reserve") uint32_t pack_storage_reserve(int64_t bytes){ticket=hpk_storage_reserve(storage_view(),bytes);return ticket.status;}
PACK_EXPORT("pack_storage_ticket") int64_t pack_storage_ticket(int32_t field){switch(field){case 0:return ticket.lease.slot;case 1:return ticket.lease.serial;case 2:return ticket.lease.epoch;default:return -1;}}
PACK_EXPORT("pack_storage_release") uint32_t pack_storage_release(int64_t slot,int64_t serial,int64_t epoch){return hiq_release(storage_view(),(coax_t_HostInboxLease){.slot=slot,.serial=serial,.epoch=epoch});}
PACK_EXPORT("pack_storage_info") int64_t pack_storage_info(int32_t field){coax_t_HostPackStorageOffer p=hpk_storage_offer(storage_view());switch(field){case 0:return p.count;case 1:return p.bytes;case 2:return p.capacity;case 3:return p.per_allocation;default:return -1;}}
PACK_EXPORT("pack_stream_open") uint32_t pack_stream_open(int64_t expected,uint32_t encoded){stream=hpk_stream_open(expected,encoded!=0);return stream.valid;}
PACK_EXPORT("pack_prefix_ptr") uint32_t pack_prefix_ptr(void){return(uint32_t)(uintptr_t)prefix;}
PACK_EXPORT("pack_prefix_capacity") uint32_t pack_prefix_capacity(void){return sizeof prefix;}
PACK_EXPORT("pack_stream_offer") uint32_t pack_stream_offer(int64_t bytes,uint32_t prefix_bytes){if(prefix_bytes>sizeof prefix)return 0;chunk=hpk_stream_offer(stream_view(),(coax_string){(char*)prefix,prefix_bytes},bytes);return chunk.accepted;}
PACK_EXPORT("pack_stream_chunk") int64_t pack_stream_chunk(int32_t field){switch(field){case 0:return chunk.at;case 1:return chunk.bytes;case 2:return chunk.total;default:return -1;}}
PACK_EXPORT("pack_stream_finish") uint32_t pack_stream_finish(void){return hpk_stream_finish(stream_view());}
PACK_EXPORT("pack_stream_head_ptr") uint32_t pack_stream_head_ptr(void){return(uint32_t)(uintptr_t)stream.head.e;}
PACK_EXPORT("pack_arena") uint32_t pack_arena(int64_t bytes){allocation=hpk_allocation((int64_t)(uintptr_t)&__heap_base,bytes);return allocation.valid;}
PACK_EXPORT("pack_arena_info") int64_t pack_arena_info(int32_t field){switch(field){case 0:return allocation.address;case 1:return allocation.pages;case 2:return allocation.memory_bytes;default:return -1;}}
PACK_EXPORT("pack_metadata_ptr") uint32_t pack_metadata_ptr(void){return(uint32_t)(uintptr_t)metadata;}
PACK_EXPORT("pack_metadata_capacity") uint32_t pack_metadata_capacity(void){return sizeof metadata;}
PACK_EXPORT("pack_metadata_offer") uint32_t pack_metadata_offer(int64_t loaded,int64_t bytes){return hpk_metadata_offer(loaded,bytes);}
PACK_EXPORT("pack_private_parse") uint32_t pack_private_parse(uint32_t bytes){if(bytes>sizeof metadata)return 0;private_plan=hpk_private((coax_string){(char*)metadata,bytes});return private_plan.valid;}
PACK_EXPORT("pack_private_info") int64_t pack_private_info(int32_t field){switch(field){case 0:return private_plan.source_bytes;case 1:return private_plan.plain_bytes;case 2:return private_plan.url_bytes;case 3:return(uintptr_t)private_plan.url.e;case 4:return(uintptr_t)private_plan.key.e;case 5:return(uintptr_t)private_plan.iv.e;default:return -1;}}
PACK_EXPORT("pack_admit") uint32_t pack_admit(uint32_t required_private){uint64_t measured=(uint64_t)__builtin_wasm_memory_size(0)*65536;if(!htb_copy_span((uint64_t)allocation.address,(uint64_t)(allocation.memory_bytes-allocation.address),(uint64_t)stream.loaded,measured))return 0;return hpk_stream_pack((coax_string){(char*)(uintptr_t)allocation.address,stream.loaded},stream,private_plan,required_private!=0,(coax_v_t_PackRead){&pack,1});}
PACK_EXPORT("pack_entry_count") uint32_t pack_entry_count(void){return(uint32_t)pack.count;}
PACK_EXPORT("pack_entry") int64_t pack_entry(int32_t index,int32_t field){if(index<0||index>=pack.count)return -1;coax_t_PackEntry e=pack.entries.e[index];switch(field){case 0:return e.kind;case 1:return e.licence;case 2:return e.at;case 3:return e.n;case 4:return(uintptr_t)e.name.data;case 5:return e.name.count;default:return -1;}}
PACK_EXPORT("pack_close") void pack_close(void){stream=hpk_stream_open(-1,false);private_plan=hpk_private((coax_string){"",0});coax_default_t_PackRead(&pack);allocation=hpk_allocation(-1,0);memset(metadata,0,sizeof metadata);memset(prefix,0,sizeof prefix);}
