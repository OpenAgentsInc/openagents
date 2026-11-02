/* OS effects and address observation only. Admission/completion/copy policy is Coax. */
#include "entropy-native.h"
#include <stddef.h>
#include <stdint.h>
#if defined(COAX_ENTROPY_TEST_PROVIDER)
/* This symbol exists only in explicitly compiled test adapters. No production
   API accepts a seed or injected provider. */
extern void coax_entropy_test_provider(uint8_t *,int64_t,int *,int *,int64_t *);
#elif defined(__APPLE__)
#include <Security/SecRandom.h>
#elif defined(__linux__)
#include <sys/random.h>
#include <errno.h>
#endif
static int spans_valid(uint8_t *out,uint8_t *scratch,int64_t requested) {
  if(requested<=0) return 1;
  if(!out||!scratch||(uint64_t)requested>UINTPTR_MAX) return 0;
  uintptr_t last=(uintptr_t)requested-1;
  return (uintptr_t)out<=UINTPTR_MAX-last && (uintptr_t)scratch<=UINTPTR_MAX-last;
}
static int spans_overlap(uint8_t *a,uint8_t *b,int64_t requested) {
  if(requested<=0) return 0;
  uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;
  return x<=y ? y-x<(uint64_t)requested : x-y<(uint64_t)requested;
}
static void os_fill(uint8_t *scratch,int64_t requested,int *available,int *ok,int64_t *written) {
  *available=1;*ok=0;*written=0;
#if defined(COAX_ENTROPY_TEST_PROVIDER)
  coax_entropy_test_provider(scratch,requested,available,ok,written);
#elif defined(__APPLE__)
  if(SecRandomCopyBytes(kSecRandomDefault,(size_t)requested,scratch)==0) {*ok=1;*written=requested;}
#elif defined(__linux__)
  while(*written<requested) {
    ssize_t got=getrandom(scratch+(size_t)*written,(size_t)(requested-*written),0);
    if(got<0) { if(errno==EINTR)continue; if(errno==ENOSYS)*available=0; return; }
    if(got==0) return;
    *written+=(int64_t)got;
  }
  *ok=1;
#else
  (void)scratch;(void)requested;*available=0;
#endif
}
coax_t_Entropy_Result coax_entropy_native(uint8_t *out,int64_t capacity,
                                        uint8_t *scratch,int64_t scratch_capacity,
                                        int64_t requested) {
  int valid=spans_valid(out,scratch,requested),overlap=spans_overlap(out,scratch,requested);
  uint32_t error=entropy_admit(requested,capacity,scratch_capacity,valid,overlap);
  if(error) return (coax_t_Entropy_Result){0,error};
  if(requested==0) return (coax_t_Entropy_Result){0,0};
  int available,ok;int64_t written;
  os_fill(scratch,requested,&available,&ok,&written);
  return entropy_finish(requested,(coax_v_u8){out,capacity},(coax_v_u8){scratch,scratch_capacity},
                        valid,overlap,available,ok,written);
}
