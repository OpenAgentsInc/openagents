#include "ecdh_owned.h"
#include <assert.h>
#include <stdint.h>
#include <string.h>
#if !defined(__wasi__) && defined(COAX_ECDH_OWNED_TEST_EFFECT)
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
static atomic_int pause_effect, entered_effect, release_effect;
#endif
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
#ifdef COAX_ECDH_OWNED_TEST_EFFECT
static int fail_next, reenter;static uint64_t current;static uint8_t *last_work;static int64_t work_size;
uint32_t coax_ecdh_owned_test_zero(void *p,int64_t cap,int64_t off,int64_t n){
 if(n>32){last_work=p;work_size=n;}
#ifndef __wasi__
 if(atomic_load(&pause_effect)){
  atomic_store(&entered_effect,1);
  while(!atomic_load(&release_effect))sched_yield();
  atomic_store(&pause_effect,0);
 }
#endif
 if(reenter){reenter=0;assert(coax_ecdh_owned_close(current)==COAX_ECDH_OWNED_BUSY);}
 if(fail_next){fail_next--;return 7;}return coax_secure_zero(p,cap,off,n);
}
#endif
static int filled(const uint8_t *p,uint8_t b,int n){for(int i=0;i<n;i++)if(p[i]!=b)return 0;return 1;}
#if !defined(__wasi__) && defined(COAX_ECDH_OWNED_TEST_EFFECT)
typedef struct {uint64_t h;const uint8_t *key,*peer;uint32_t result;} Start;
static void *concurrent_start(void *p){Start *s=p;s->result=coax_ecdh_owned_start(s->h,s->key,32,s->peer,32);return 0;}
#endif
int main(void){
 const uint8_t peer[32]={0x79,0xbe,0x66,0x7e,0xf9,0xdc,0xbb,0xac,0x55,0xa0,0x62,0x95,0xce,0x87,0x0b,0x07,0x02,0x9b,0xfc,0xdb,0x2d,0xce,0x28,0xd9,0x59,0xf2,0x81,0x5b,0x16,0xf8,0x17,0x98};
 uint8_t key[32]={0},output[32],invalid[32];key[31]=1;memset(invalid,255,32);memset(output,0xa5,32);
 uint64_t h=coax_ecdh_owned_acquire();assert(h);assert(!coax_ecdh_owned_acquire());
 assert(coax_ecdh_owned_start(h,key,31,peer,32)==COAX_ECDH_OWNED_INVALID);
 assert(coax_ecdh_owned_start(h,key,32,key,32)==COAX_ECDH_OWNED_INVALID);
 assert(coax_ecdh_owned_start(h,0,32,peer,32)==COAX_ECDH_OWNED_INVALID);
 assert(coax_ecdh_owned_start(h,(const uint8_t *)(uintptr_t)(UINTPTR_MAX-15),32,peer,32)==COAX_ECDH_OWNED_INVALID);
#ifdef COAX_ECDH_OWNED_TEST_EFFECT
 current=h;reenter=1;
#endif
 assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_collect(h,key,32)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_collect(h,0,32)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_collect(h,(uint8_t *)(uintptr_t)(UINTPTR_MAX-15),32)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_collect(h,output,31)==COAX_ECDH_OWNED_PENDING);assert(filled(output,0xa5,32));
 assert(coax_ecdh_owned_close(h)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_collect(h,output,32)==0);assert(!memcmp(output,peer,32));assert(coax_ecdh_owned_close(h)==0);
 h=coax_ecdh_owned_acquire();assert(h);assert(coax_ecdh_owned_start(h,key,31,peer,32)==COAX_ECDH_OWNED_INVALID);
 assert(coax_ecdh_owned_cancel(h)==COAX_ECDH_OWNED_CANCELLED);assert(coax_ecdh_owned_close(h)==COAX_ECDH_OWNED_INVALID);
 uint64_t old=h;h=coax_ecdh_owned_acquire();assert(h && h!=old);assert(coax_ecdh_owned_start(old,key,32,peer,32)==COAX_ECDH_OWNED_INVALID);
 assert(coax_ecdh_owned_start(h,key,32,invalid,32)==COAX_ECDH_OWNED_PEER);assert(coax_ecdh_owned_close(h)==0);
#ifdef COAX_ECDH_OWNED_TEST_EFFECT
 assert(last_work && work_size>32 && filled(last_work,0,(int)work_size));
#endif
 h=coax_ecdh_owned_acquire();memset(key,0,32);assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_SCALAR);assert(coax_ecdh_owned_close(h)==0);key[31]=1;
#ifdef COAX_ECDH_OWNED_TEST_EFFECT
 assert(filled(last_work,0,(int)work_size));
 h=coax_ecdh_owned_acquire();fail_next=1;assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);
 assert(last_work && !filled(last_work,0,(int)work_size));
 assert(coax_ecdh_owned_close(h)==COAX_ECDH_OWNED_PENDING);assert(!coax_ecdh_owned_acquire());assert(coax_ecdh_owned_retry(h)==COAX_ECDH_OWNED_EFFECT);assert(coax_ecdh_owned_close(h)==0);
 assert(filled(last_work,0,(int)work_size));
 h=coax_ecdh_owned_acquire();assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);
 memset(output,0xa5,32);fail_next=1;assert(coax_ecdh_owned_collect(h,output,32)==COAX_ECDH_OWNED_PENDING);assert(!memcmp(output,peer,32));
 uint8_t other[32];memset(other,0xa5,32);assert(coax_ecdh_owned_collect(h,other,32)==COAX_ECDH_OWNED_PENDING);assert(filled(other,0xa5,32));
 assert(coax_ecdh_owned_close(h)==COAX_ECDH_OWNED_PENDING);fail_next=1;assert(coax_ecdh_owned_retry(h)==COAX_ECDH_OWNED_PENDING);assert(!coax_ecdh_owned_acquire());
 assert(coax_ecdh_owned_retry(h)==COAX_ECDH_OWNED_EFFECT);assert(filled(output,0,32));assert(coax_ecdh_owned_close(h)==0);
 h=coax_ecdh_owned_acquire();assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);
 fail_next=1;assert(coax_ecdh_owned_cancel(h)==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_close(h)==COAX_ECDH_OWNED_PENDING);assert(coax_ecdh_owned_retry(h)==COAX_ECDH_OWNED_EFFECT);assert(coax_ecdh_owned_close(h)==0);
 h=coax_ecdh_owned_acquire();assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);
 memset(output,0xa5,32);fail_next=1;assert(coax_ecdh_owned_collect(h,output,32)==COAX_ECDH_OWNED_PENDING);assert(!memcmp(output,peer,32));
 assert(coax_ecdh_owned_cancel(h)==COAX_ECDH_OWNED_CANCELLED);assert(filled(output,0,32));assert(coax_ecdh_owned_close(h)==0);
#endif
 h=coax_ecdh_owned_acquire();assert(coax_ecdh_owned_start(h,key,32,peer,32)==COAX_ECDH_OWNED_PENDING);assert(coax_ecdh_owned_cancel(h)==COAX_ECDH_OWNED_CANCELLED);assert(coax_ecdh_owned_close(h)==0);
#if !defined(__wasi__) && defined(COAX_ECDH_OWNED_TEST_EFFECT)
 h=coax_ecdh_owned_acquire();Start run={h,key,peer,99};pthread_t thread;
 atomic_store(&entered_effect,0);atomic_store(&release_effect,0);atomic_store(&pause_effect,1);
 assert(!pthread_create(&thread,0,concurrent_start,&run));
 for(int i=0;i<5000000 && !atomic_load(&entered_effect);i++)sched_yield();assert(atomic_load(&entered_effect));
 memset(output,0xa5,32);assert(!coax_ecdh_owned_acquire());assert(coax_ecdh_owned_cancel(h)==COAX_ECDH_OWNED_BUSY);
 assert(coax_ecdh_owned_close(h)==COAX_ECDH_OWNED_BUSY);assert(coax_ecdh_owned_retry(h)==COAX_ECDH_OWNED_BUSY);
 assert(coax_ecdh_owned_collect(h,output,32)==COAX_ECDH_OWNED_BUSY && filled(output,0xa5,32));
 atomic_store(&release_effect,1);assert(!pthread_join(thread,0));assert(run.result==COAX_ECDH_OWNED_PENDING);
 assert(coax_ecdh_owned_cancel(h)==COAX_ECDH_OWNED_CANCELLED);assert(coax_ecdh_owned_close(h)==0);
#endif
 return 0;
}
