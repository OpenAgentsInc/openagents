/* Test adapter and C ABI observations only. Never linked by an app. */
#include "entropy-native.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define MAX_BYTES 1048576
static uint8_t output[MAX_BYTES+8],scratch[MAX_BYTES+8];
#if defined(COAX_ENTROPY_TEST_PROVIDER)
static int mode,calls;
void coax_entropy_test_provider(uint8_t *out,int64_t n,int *available,int *ok,int64_t *written) {
  calls++;*available=mode!=1;*ok=mode!=1&&mode!=2;*written=n;
  if(mode==1){*written=0;return;}
  if(mode==2)*written=n/2;
  if(mode==3)*written=n-1;
  if(mode==4)*written=n+1;
  if(mode==5)*written=-1;
  int64_t fill=mode==2?n/2:mode==3?n-1:mode==5?n/2:n;
  for(int64_t i=0;i<fill;i++)out[i]=(uint8_t)((i+7)%251);
}
#endif
static int reset(void) {memset(output,77,sizeof output);memset(scratch,88,sizeof scratch);return 0;}
static int unchanged(void) {for(size_t i=0;i<sizeof output;i++)if(output[i]!=77||scratch[i]!=88)return 0;return 1;}
int main(void) {
  const int64_t requests[]={0,1,32,65535,65536,65537,MAX_BYTES};
  for(size_t k=0;k<sizeof requests/sizeof requests[0];k++) {
    int64_t n=requests[k];reset();
    coax_t_Entropy_Result r=coax_entropy_native(output,sizeof output,scratch,sizeof scratch,n);
    if(r.error||r.written!=n)return 10;
    for(int64_t i=0;i<n;i++){
      if(scratch[i]!=0)return 11;
#if defined(COAX_ENTROPY_TEST_PROVIDER)
      if(output[i]!=(uint8_t)((i+7)%251))return 12;
#endif
    }
    for(size_t i=(size_t)n;i<sizeof output;i++)if(output[i]!=77||scratch[i]!=88)return 13;
  }
#if defined(COAX_ENTROPY_TEST_PROVIDER)
  for(mode=1;mode<=5;mode++) {
    reset();coax_t_Entropy_Result r=coax_entropy_native(output,sizeof output,scratch,sizeof scratch,32);
    uint32_t wanted=mode==1?5:mode==2?6:7;
    if(r.error!=wanted||r.written)return 20+mode;
    for(size_t i=0;i<sizeof output;i++)if(output[i]!=77)return 30+mode;
    for(int i=0;i<32;i++)if(scratch[i]!=0)return 40+mode;
    if(scratch[32]!=88)return 50+mode;
  }
  mode=0;int before=calls;
#endif
  reset();
  coax_t_Entropy_Result cases[]={
    coax_entropy_native(output,sizeof output,scratch,sizeof scratch,-1),
    coax_entropy_native(output,sizeof output,scratch,sizeof scratch,MAX_BYTES+1),
    coax_entropy_native(output,31,scratch,sizeof scratch,32),
    coax_entropy_native(output,sizeof output,scratch,31,32),
    coax_entropy_native(NULL,32,scratch,sizeof scratch,32),
    coax_entropy_native(output,sizeof output,NULL,32,32),
    coax_entropy_native(output,sizeof output,output,sizeof output,32),
    coax_entropy_native(output,sizeof output,output+1,sizeof output-1,32),
    coax_entropy_native((uint8_t *)(uintptr_t)(UINTPTR_MAX-1),32,scratch,sizeof scratch,32),
    coax_entropy_native(output,-1,scratch,sizeof scratch,0)
  };
  const uint32_t errors[]={1,1,2,2,3,3,4,4,3,2};
  for(size_t i=0;i<sizeof cases/sizeof cases[0];i++)if(cases[i].error!=errors[i]||cases[i].written)return 60+(int)i;
  if(!unchanged())return 80;
  coax_t_Entropy_Result z=coax_entropy_native(NULL,0,NULL,0,0);
  if(z.error||z.written)return 81;
#if defined(COAX_ENTROPY_TEST_PROVIDER)
  if(calls!=before)return 82;
  puts("test provider: success, partial/short/unavailable failures and no-call admission passed");
#else
  puts("OS CSPRNG: real requests and native boundary cases passed");
#endif
  return 0;
}
