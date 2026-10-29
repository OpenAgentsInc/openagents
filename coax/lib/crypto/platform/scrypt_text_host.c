/* Pointer observations, synchronous effects and cleanup only. Unicode,
 * address admission and scrypt lifecycle are decided by the Coax modules. */
#include "scrypt_text_host.h"
#include <stddef.h>
#include <stdint.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
#ifdef COAX_SCRYPT_TEXT_TEST_EFFECT
extern uint32_t coax_scrypt_text_test_zero(void *,int64_t,int64_t,int64_t);
#define TEXT_ZERO coax_scrypt_text_test_zero
#else
#define TEXT_ZERO coax_secure_zero
#endif
static uint64_t address(const void *p){return (uint64_t)(uintptr_t)p;}
static int admitted(void *owner,int64_t owner_n,uint8_t *memory,int64_t n,const uint8_t *password,int64_t pn,const uint8_t *salt,int64_t sn,int64_t cost){
#if UINTPTR_MAX == UINT32_MAX
 return sa_start32(address(owner),owner_n,address(memory),n,address(password),pn,address(salt),sn,cost);
#elif UINTPTR_MAX == UINT64_MAX
 return sa_start(address(owner),owner_n,address(memory),n,address(password),pn,address(salt),sn,cost);
#else
#error unsupported pointer width
#endif
}
static int separate(coax_scrypt_host *h,uint8_t *memory,int64_t n,coax_scrypt_text_workspace *w){
#if UINTPTR_MAX == UINT32_MAX
 return sa_output32(address(h),sizeof *h,address(memory),n,address(w),sizeof *w);
#else
 return sa_output(address(h),sizeof *h,address(memory),n,address(w),sizeof *w);
#endif
}
int coax_scrypt_text_start(coax_scrypt_host *h,coax_scrypt_text_workspace *w,const uint8_t *password,int64_t pn,const uint8_t *salt,int64_t sn,uint8_t *memory,int64_t n,int64_t cost){
 if(!h || !w || h->job.active || h->wiping.active)return 1;
 if(!admitted(h,sizeof *h,memory,n,password,pn,salt,sn,cost) || !admitted(w,sizeof *w,memory,n,password,pn,salt,sn,cost) || !separate(h,memory,n,w))return 1;
 coax_t_UnResult r=un_nfkc((coax_string){(const char *)password,pn},(coax_v_u32){w->scratch,18432},(coax_v_u8){w->normalized,1024},0);
 int status=1;
 if(r.error==0)status=coax_scrypt_host_start(h,w->normalized,r.written,salt,sn,memory,n,cost);
 if(TEXT_ZERO(w,sizeof *w,0,sizeof *w)){
   if(h->job.active)(void)coax_scrypt_host_cancel(h);
   return 2;
 }
 return status;
}
