// Firm reason: 1 (browser glue: concrete reactor storage and scalar Coax service projections).
#include "services-carrier.h"
#define SERVICES_EXPORT(name) __attribute__((export_name(name)))
static coax_t_HostAudioBank service_bank;
static coax_t_HostAudioMemory service_memory;
static uint8_t service_line[1024];
static coax_t_HostAudioBankSpan service_source;
static coax_t_HostInboxQueue service_audio_queue;
static coax_t_HostInboxGrant service_audio_grant;
static coax_v_t_HostAudioBank bank_view(void){return(coax_v_t_HostAudioBank){&service_bank,1};}
static coax_v_t_HostAudioMemory memory_view(void){return(coax_v_t_HostAudioMemory){&service_memory,1};}
static coax_v_t_HostInboxQueue queue_view(void){return(coax_v_t_HostInboxQueue){&service_audio_queue,1};}
SERVICES_EXPORT("services_line_ptr") uint32_t services_line_ptr(void){return(uint32_t)(uintptr_t)service_line;}
SERVICES_EXPORT("services_line_capacity") uint32_t services_line_capacity(void){return sizeof service_line;}
SERVICES_EXPORT("services_span") uint32_t services_span(uint64_t address,uint64_t capacity,uint64_t count,uint64_t memory_bytes){return htb_copy_span(address,capacity,count,memory_bytes);}
SERVICES_EXPORT("services_render_presentation") uint32_t services_render_presentation(int64_t version,int64_t profile){return hrm_presentation(version,profile);}
SERVICES_EXPORT("services_bank_insert") int64_t services_bank_insert(uint32_t count,uint64_t base,uint64_t capacity){if(count>sizeof service_line)return -1;return hab_insert(bank_view(),(coax_string){(const char*)service_line,count},base,capacity);}
SERVICES_EXPORT("services_bank_source") uint32_t services_bank_source(int64_t id){service_source=hab_source(bank_view(),id);return service_source.valid;}
SERVICES_EXPORT("services_bank_count") int64_t services_bank_count(void){return service_bank.count;}
SERVICES_EXPORT("services_bank_field") uint64_t services_bank_field(int32_t field){switch(field){case 0:return(uint64_t)(uintptr_t)service_source.name.data;case 1:return(uint64_t)service_source.name.count;case 2:return service_source.address;case 3:return(uint64_t)service_source.bytes;default:return 0;}}
SERVICES_EXPORT("services_reserve") int64_t services_reserve(int64_t words){return ham_reserve(memory_view(),words);}
SERVICES_EXPORT("services_extent") int64_t services_extent(int64_t frames,int64_t channels){return ham_extent(frames,channels);}
SERVICES_EXPORT("services_release") uint32_t services_release(int64_t ticket){return ham_release(memory_view(),ticket);}
SERVICES_EXPORT("services_words") int64_t services_words(void){return service_memory.total;}
SERVICES_EXPORT("services_audio_queue_open") uint32_t services_audio_queue_open(void){service_audio_queue=hiq_open(sizeof service_line);return service_audio_queue.valid;}
SERVICES_EXPORT("services_audio_queue_offer") int32_t services_audio_queue_offer(int64_t bytes){service_audio_grant=hiq_reserve(queue_view(),bytes);return service_audio_grant.status;}
SERVICES_EXPORT("services_audio_queue_lease") int64_t services_audio_queue_lease(int32_t field){switch(field){case 0:return service_audio_grant.lease.slot;case 1:return service_audio_grant.lease.serial;case 2:return service_audio_grant.lease.epoch;default:return -1;}}
SERVICES_EXPORT("services_audio_queue_release") uint32_t services_audio_queue_release(int64_t slot,int64_t serial,int64_t epoch){return hiq_release(queue_view(),(coax_t_HostInboxLease){.slot=slot,.serial=serial,.epoch=epoch});}
SERVICES_EXPORT("services_audio_queue_info") int64_t services_audio_queue_info(int32_t field){switch(field){case 0:return service_audio_queue.count;case 1:return service_audio_queue.bytes;default:return -1;}}
