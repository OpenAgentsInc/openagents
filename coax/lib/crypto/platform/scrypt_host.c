/* Platform effects and address observations; Coax owns lifecycle decisions. */
#include "scrypt_host.h"
#include <stddef.h>
#include <stdint.h>
#include <limits.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
#ifdef COAX_SCRYPT_TEST_EFFECT
extern uint32_t coax_scrypt_test_zero(void *,int64_t,int64_t,int64_t);
#define HOST_ZERO coax_scrypt_test_zero
#else
#define HOST_ZERO coax_secure_zero
#endif
static coax_v_t_ScryptJob view(coax_scrypt_host *h){return (coax_v_t_ScryptJob){&h->job,1};}
static uint64_t addr(const void *p){return (uint64_t)(uintptr_t)p;}
static int admit_start(coax_scrypt_host *h,const uint8_t *p,int64_t pn,const uint8_t *salt,int64_t sn,uint8_t *memory,int64_t n,int64_t cost){
#if UINTPTR_MAX == UINT32_MAX
    return sa_start32(addr(h),sizeof *h,addr(memory),n,addr(p),pn,addr(salt),sn,cost);
#elif UINTPTR_MAX == UINT64_MAX
    return sa_start(addr(h),sizeof *h,addr(memory),n,addr(p),pn,addr(salt),sn,cost);
#else
#error unsupported pointer width
#endif
}
static int admit_output(coax_scrypt_host *h,uint8_t *out,int64_t n){
#if UINTPTR_MAX == UINT32_MAX
    return sa_output32(addr(h),sizeof *h,addr(h->scratch),h->scratch_count,addr(out),n);
#else
    return sa_output(addr(h),sizeof *h,addr(h->scratch),h->scratch_count,addr(out),n);
#endif
}
int coax_scrypt_host_start(coax_scrypt_host *h,const uint8_t *p,int64_t pn,const uint8_t *salt,int64_t sn,uint8_t *memory,int64_t n,int64_t cost){
    if(!h || h->job.active || h->wiping.active || !admit_start(h,p,pn,salt,sn,memory,n,cost))return 1;
    h->scratch=memory;h->scratch_count=n;
    return sj_start(view(h),(coax_v_u8){(uint8_t *)p,pn},(coax_v_u8){(uint8_t *)salt,sn},(coax_v_u8){memory,n},cost,0)?0:1;
}
int64_t coax_scrypt_host_status(coax_scrypt_host *h){return h?sj_next(view(h)):-1;}
int coax_scrypt_host_step(coax_scrypt_host *h,int64_t budget){
    if(!h)return 1;
    return sj_step(view(h),(coax_v_u8){h->scratch,h->scratch_count},budget,0)?0:1;
}
int coax_scrypt_host_cancel(coax_scrypt_host *h){return h && sj_cancel(view(h))?0:1;}
int coax_scrypt_host_clean(coax_scrypt_host *h){
    if(!h)return 2;
    int64_t region=sj_next(view(h));if(region==0)return 0;if(region<0)return 2;
    if(region==1){
        if(!h->wiping.active && coax_large_wipe_begin(&h->wiping,h->scratch,h->scratch_count))return 3;
        uint32_t status=coax_large_wipe_step(&h->wiping);
        if(status==9)return 1;
        if(status || coax_large_wipe_reset(&h->wiping))return 3;
    }else if(region==2){
        if(HOST_ZERO(&h->job.secret,sizeof h->job.secret,0,sizeof h->job.secret))return 3;
    }else if(region==4){
        if(HOST_ZERO(&h->job.key,sizeof h->job.key,0,sizeof h->job.key))return 3;
    }else return 2;
    if(!sj_ack(view(h),region,1))return 3;
    return sj_next(view(h))==0?0:1;
}
int coax_scrypt_host_publish(coax_scrypt_host *h,uint8_t *out,int64_t n){
    if(!h || !admit_output(h,out,n) || !sj_publish(view(h),(coax_v_u8){out,n},0))return 1;
    if(HOST_ZERO(&h->job.key,sizeof h->job.key,0,sizeof h->job.key)){
        (void)HOST_ZERO(out,n,0,32);return 2;
    }
    return 0;
}
int coax_scrypt_host_close(coax_scrypt_host *h){
    if(!h || h->wiping.active)return 1;
    int64_t status=sj_next(view(h));
    if(status!=0 && status!=-3)return 1;
    /* Abandon an unpublished success through the Coax lifecycle; it must
     * return to key cleanup before the host can be cleared. */
    if(status==0){
        if(!sj_cancel(view(h)) || sj_next(view(h))!=0)return 1;
    }
    return HOST_ZERO(h,sizeof *h,0,sizeof *h)?1:0;
}
