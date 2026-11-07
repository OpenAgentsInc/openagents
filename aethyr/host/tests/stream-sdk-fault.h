// Test-carrier effects only. Shared production SDK interfaces stay unchanged.
#ifndef STREAM_SDK_FAULT_H
#define STREAM_SDK_FAULT_H
#include <stdint.h>
int32_t sw_sdk_fault_open(int32_t kind,int32_t domain,int32_t phase);
void sw_sdk_scope(int32_t domain,int32_t kind,int32_t row,int64_t generation,int64_t epoch);
int64_t sw_sdk_fault_info(int32_t field);
#endif
