#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
// Blocking: run away from the UI thread. Only protobuf AgentRequest/AgentReply
// cross this boundary. Device secrets stay in the platform's protected store.
typedef int64_t (*openagents_phone_network_io)(uint64_t,const uint8_t*,size_t,uint8_t*,size_t);
uint8_t* openagents_phone_host_call(const uint8_t*,size_t,const char*,uint64_t,openagents_phone_network_io,size_t*);
void openagents_phone_host_free(uint8_t*,size_t);
char* openagents_phone_host_key(void);
void openagents_phone_host_key_free(char*);
#ifdef __cplusplus
}
#endif
