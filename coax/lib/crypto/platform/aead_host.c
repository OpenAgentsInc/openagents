/* Platform pointer observations and effects; Coax owns admission and policy. */
#include "aead_host.h"
#include <stdint.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
#ifdef COAX_AEAD_TEST_EFFECT
extern uint32_t coax_aead_test_zero(void *,int64_t,int64_t,int64_t);
#define HOST_ZERO coax_aead_test_zero
#else
#define HOST_ZERO coax_secure_zero
#endif
static uint64_t address(const void *p){return (uint64_t)(uintptr_t)p;}
static coax_v_t_AeadOperation state(coax_aead_host *h){return (coax_v_t_AeadOperation){&h->operation,1};}
static coax_v_t_AeadScratch work(coax_aead_host *h){return (coax_v_t_AeadScratch){h->workspace,1};}
int coax_aead_host_start(coax_aead_host *h,coax_t_AeadScratch *w,const uint8_t *key,int64_t kn,const uint8_t *nonce,int64_t nn,const uint8_t *aad,int64_t an,const uint8_t *input,int64_t n,uint8_t *out,int64_t capacity,int seal){
 if(!h || !w || ((uintptr_t)h % _Alignof(coax_aead_host)) || ((uintptr_t)w % _Alignof(coax_t_AeadScratch)))return 1;
 if(h->operation.active || (seal!=0 && seal!=1))return 1;
 coax_t_AeadSpan spans[7]={{address(h),sizeof *h},{address(w),sizeof *w},{address(key),kn},{address(nonce),nn},{address(aad),an},{address(input),n},{address(out),capacity}};
#if UINTPTR_MAX == UINT32_MAX
 const int narrow=1;
#elif UINTPTR_MAX == UINT64_MAX
 const int narrow=0;
#else
#error unsupported pointer width
#endif
 if(!aa_admit((coax_v_t_AeadSpan){spans,7},seal,narrow))return 1;
 h->workspace=w;h->output=out;h->capacity=capacity;
 return aw_start(state(h),work(h),(coax_v_u8){(uint8_t *)key,kn},(coax_v_u8){(uint8_t *)nonce,nn},(coax_v_u8){(uint8_t *)aad,an},(coax_v_u8){(uint8_t *)input,n},seal,0)?0:1;
}
int64_t coax_aead_host_status(coax_aead_host *h){return h?aw_next(state(h)):-1;}
int coax_aead_host_cancel(coax_aead_host *h){return h&&aw_cancel(state(h))?0:1;}
int coax_aead_host_clean(coax_aead_host *h){
 if(!h)return 2;int64_t region=aw_next(state(h));if(region==0)return 0;if(region<0)return 2;
 void *buffer;int64_t n;if(region==1){buffer=h->workspace;n=sizeof *h->workspace;}else if(region==2){buffer=&h->operation.staged;n=sizeof h->operation.staged;}else return 2;
 if(HOST_ZERO(buffer,n,0,n))return 3;
 if(!aw_ack(state(h),region,1))return 3;
 return aw_next(state(h))==0?0:1;
}
int coax_aead_host_publish(coax_aead_host *h){
 if(!h || !aw_publish(state(h),(coax_v_u8){h->output,h->capacity},0))return 1;
 if(HOST_ZERO(&h->operation.staged,sizeof h->operation.staged,0,sizeof h->operation.staged)){
  (void)HOST_ZERO(h->output,h->capacity,0,h->operation.count);return 2;
 }
 return 0;
}
int coax_aead_host_close(coax_aead_host *h){
 if(!h)return 1;int64_t status=aw_next(state(h));if(status!=0 && status!=-3)return 1;
 if(status==0){if(!aw_cancel(state(h)) || aw_next(state(h))!=0)return 1;}
 return HOST_ZERO(h,sizeof *h,0,sizeof *h)?1:0;
}
