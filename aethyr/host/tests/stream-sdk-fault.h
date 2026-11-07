// Test-carrier effects only. Shared production SDK interfaces stay unchanged.
#ifndef STREAM_SDK_FAULT_H
#define STREAM_SDK_FAULT_H
#include <stdint.h>
int32_t sw_sdk_fault_open(int32_t kind,int32_t domain,int32_t phase);
void sw_sdk_scope(int32_t domain,int32_t kind,int32_t row,int64_t generation,int64_t epoch);
int64_t sw_sdk_fault_info(int32_t field);
// Native carrier's allocation observer: the active Coax grant precedes
// actual SDK allocation and remains owned through the observed free.
int sw_vk_backing_memory_allowed(uint64_t bytes,uint32_t type);
void sw_vk_backing_memory_created(uintptr_t memory,uint64_t bytes,uint32_t type);
void sw_vk_backing_memory_freed(uintptr_t memory);
#endif
