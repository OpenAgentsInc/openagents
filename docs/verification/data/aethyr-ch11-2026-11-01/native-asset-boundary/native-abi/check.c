#include "fixture.h"
#include <stdint.h>
uint32_t coaxial_asset_capacity(uint32_t,int32_t);
uint32_t coaxial_asset(uint32_t,int32_t,uint32_t);
int main(void){uint32_t cap=coaxial_asset_capacity(0,0);if(cap!=32768||coaxial_asset_capacity(0,-1)||coaxial_asset_capacity(0,1))return 1;uint64_t p=coaxial_asset(0,0,cap);if(!p||coaxial_asset(0,0,cap+1))return 2;if(!aethyr_native_span(p,cap,p,cap)||!aethyr_native_span(p+cap,0,p,cap)||aethyr_native_span(p+cap,1,p,cap)||aethyr_native_span(p-1,1,p,cap))return 3;if(!aethyr_native_span(UINT64_MAX,0,UINT64_MAX-5,5)||aethyr_native_span(UINT64_MAX,1,UINT64_MAX-5,5))return 4;return 0;}
