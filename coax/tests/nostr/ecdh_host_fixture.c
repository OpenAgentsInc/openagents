#include "ecdh_host.h"
#include <assert.h>
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static int fail_next;
uint32_t coax_ecdh_test_zero(void *p,int64_t cap,int64_t off,int64_t n){if(fail_next>0){fail_next--;return 7;}return coax_secure_zero(p,cap,off,n);}
static int cleared(const void *p,size_t n){const uint8_t *b=p;for(size_t i=0;i<n;i++)if(b[i])return 0;return 1;}
int main(void){
 const uint8_t peer[32]={0x79,0xbe,0x66,0x7e,0xf9,0xdc,0xbb,0xac,0x55,0xa0,0x62,0x95,0xce,0x87,0x0b,0x07,0x02,0x9b,0xfc,0xdb,0x2d,0xce,0x28,0xd9,0x59,0xf2,0x81,0x5b,0x16,0xf8,0x17,0x98};
 uint8_t key[32]={0};key[31]=1;uint8_t output[34];coax_t_EcdhScratch work;
 memset(&work,0xa5,sizeof work);memset(output,0xa5,sizeof output);
 assert(coax_ecdh_raw(key,32,peer,32,output+1,32,&work)==0);assert(!memcmp(output+1,peer,32));assert(output[0]==0xa5&&output[33]==0xa5);assert(cleared(&work,sizeof work));
 memset(&work,0xa5,sizeof work);memset(output,0xa5,sizeof output);memset(key,0,32);
 assert(coax_ecdh_raw(key,32,peer,32,output+1,32,&work)==3);assert(cleared(&work,sizeof work));for(int i=0;i<34;i++)assert(output[i]==0xa5);
 memset(&work,0xa5,sizeof work);memset(output,0xa5,sizeof output);uint8_t invalid[32];memset(invalid,0xff,32);key[31]=1;
 assert(coax_ecdh_raw(key,32,invalid,32,output+1,32,&work)==2);assert(cleared(&work,sizeof work));for(int i=0;i<34;i++)assert(output[i]==0xa5);
 memset(&work,0xa5,sizeof work);assert(coax_ecdh_raw(key,31,peer,32,output+1,32,&work)==4);assert(!cleared(&work,sizeof work));
 assert(coax_ecdh_raw(key,32,peer,32,key,32,&work)==4);
 assert(coax_ecdh_raw(key,32,key,32,output+1,32,&work)==4);
 assert(coax_ecdh_raw((const uint8_t *)(uintptr_t)(UINTPTR_MAX-15),32,peer,32,output+1,32,&work)==4);
 assert(coax_ecdh_raw(key,32,peer,32,(uint8_t *)&work,32,&work)==4);
 assert(coax_ecdh_raw(key,32,peer,32,output+1,-1,&work)==4);
 memset(&work,0xa5,sizeof work);memset(output,0xa5,sizeof output);fail_next=1;
 assert(coax_ecdh_raw(key,32,peer,32,output+1,32,&work)==5);assert(cleared(output+1,32));assert(!cleared(&work,sizeof work));
 assert(coax_ecdh_cleanup(&work,output+1,32)==0);assert(cleared(&work,sizeof work)&&cleared(output+1,32));
 memset(&work,0xa5,sizeof work);memset(output,0xa5,sizeof output);fail_next=2;
 assert(coax_ecdh_raw(key,32,peer,32,output+1,32,&work)==5);assert(!cleared(&work,sizeof work));assert(!memcmp(output+1,peer,32));
 assert(coax_ecdh_cleanup(&work,output+1,32)==0);assert(cleared(&work,sizeof work)&&cleared(output+1,32));
 static uint8_t large[1048577];memset(large,0xa5,sizeof large);memset(&work,0xa5,sizeof work);fail_next=1;
 assert(coax_ecdh_raw(key,32,peer,32,large,sizeof large,&work)==5);assert(cleared(large,32)&&large[32]==0xa5);
 assert(coax_ecdh_cleanup(&work,large,sizeof large)==0);assert(cleared(&work,sizeof work)&&cleared(large,32));
 return 0;
}
