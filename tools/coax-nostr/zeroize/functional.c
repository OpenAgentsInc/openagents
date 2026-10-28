/* Observable tests are separate from probe.c's unobserved erasure buffers. */
#include "zeroize.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
EXPORT("range") uint32_t range(uint32_t pointer,int32_t capacity,int32_t at,int32_t length){return coax_secure_zero((void *)(uintptr_t)pointer,capacity,at,length);}
EXPORT("scenario") int scenario(uint32_t reason){
    uint8_t a[10],b[10];for(int i=0;i<10;i++){a[i]=77;b[i]=88;}
    coax_secret_scope scope={0};
    if(coax_secret_register(&scope,1,a,8,4)||coax_secret_register(&scope,2,b,8,0))return 1;
    if(coax_secret_copy(&scope,1,2,4))return 2;
    if(coax_secret_close(&scope,reason)||!zero_scope_complete(scope.policy))return 3;
    for(int i=0;i<4;i++)if(a[i])return 4;
    for(int i=4;i<10;i++)if(a[i]!=77)return 5;
    for(int i=0;i<8;i++)if(b[i])return 6;
    for(int i=8;i<10;i++)if(b[i]!=88)return 7;
    return coax_secret_reset(&scope)?8:0;
}
