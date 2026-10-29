/* Public test data; tests observed erasure, publication and refusal paths. */
#include "scrypt_host.h"
#include <stdint.h>
#include <string.h>
static coax_scrypt_host host;
static uint8_t small[2050],large[2097154],output[34],password[5]={'n','o','s','t','r'},salt[16];
#ifdef COAX_SCRYPT_TEST_EFFECT
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static int fail_key,fail_output,fail_state;
uint32_t coax_scrypt_test_zero(void *p,int64_t n,int64_t off,int64_t len){
  if(p==&host.job.key && fail_key){fail_key=0;return 8;}
  if(p==output+1 && fail_output){fail_output=0;return 8;}
  if(p==&host.job.secret && fail_state){fail_state=0;return 8;}
  return coax_secure_zero(p,n,off,len);
}
#endif
static const uint8_t expected[32]={0x34,0xb8,0x3a,0xe9,0xbd,0x8d,0x9d,0x3b,0xbb,0x6a,0xa6,0xf7,0x3a,0xc6,0xb4,0x22,0x9f,0xb5,0xbb,0x49,0x04,0x43,0x17,0xd3,0x92,0xc7,0x32,0x35,0x1a,0xb5,0x1c,0x17};
static int allzero(const void *p,size_t n){const uint8_t *b=p;for(size_t i=0;i<n;i++)if(b[i])return 0;return 1;}
static int start(uint8_t *mem,int64_t n,int log){return !coax_scrypt_host_start(&host,password,5,salt,16,mem,n,log);}
int scrypt_host_fixture(void){
  for(int i=0;i<16;i++)salt[i]=(uint8_t)i;
  memset(&host,0,sizeof host);memset(small,165,sizeof small);memset(output,165,sizeof output);
  if(coax_scrypt_host_close(&host)!=1 || coax_scrypt_host_clean(&host)!=2)return 1;
  if(!start(small+1,2048,1) || coax_scrypt_host_status(&host)!=-2)return 2;
  if(!coax_scrypt_host_start(&host,password,5,salt,16,small+1,2048,1))return 3;
  if(!coax_scrypt_host_publish(&host,output+1,32) || !coax_scrypt_host_close(&host))return 4;
  if(coax_scrypt_host_step(&host,4) || coax_scrypt_host_status(&host)!=1)return 5;
  if(coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=0)return 6;
  if(!allzero(small+1,2048)||small[0]!=165||small[2049]!=165||!allzero(&host.job.secret,sizeof host.job.secret))return 7;
  if(!coax_scrypt_host_publish(&host,(uint8_t *)&host,32) || !coax_scrypt_host_publish(&host,small+1,32))return 8;
  if(coax_scrypt_host_publish(&host,output+1,32) || memcmp(output+1,expected,32))return 9;
  if(output[0]!=165||output[33]!=165||!allzero(&host.job.key,sizeof host.job.key))return 10;
  if(coax_scrypt_host_close(&host) || !allzero(&host,sizeof host))return 11;
  if(!start(small+1,2048,1) || !coax_scrypt_host_step(&host,0))return 12;
  if(coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=0)return 13;
  if(!coax_scrypt_host_publish(&host,output+1,32) || coax_scrypt_host_close(&host))return 14;
  if(!start(small+1,2048,1) || coax_scrypt_host_step(&host,4))return 15;
  if(coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=0)return 16;
  if(!coax_scrypt_host_close(&host) || coax_scrypt_host_status(&host)!=4)return 17;
  if(coax_scrypt_host_clean(&host)!=0 || coax_scrypt_host_close(&host))return 18;
  memset(large,165,sizeof large);
  if(!start(large+1,2097152,11) || coax_scrypt_host_step(&host,1) || coax_scrypt_host_cancel(&host))return 19;
  if(coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=0)return 20;
  if(!allzero(large+1,2097152)||large[0]!=165||large[2097153]!=165||coax_scrypt_host_close(&host))return 21;
  if(!coax_scrypt_host_start(&host,password,5,salt,15,small+1,2048,1))return 22;
  if(!coax_scrypt_host_start(&host,(const uint8_t *)&host,5,salt,16,small+1,2048,1))return 23;
  if(!coax_scrypt_host_start(&host,password,5,salt,16,(uint8_t *)&host,2048,1))return 24;
  if(!coax_scrypt_host_start(&host,password,5,salt,16,(uint8_t *)(UINTPTR_MAX-1),2048,1))return 33;
#ifdef COAX_SCRYPT_TEST_EFFECT
  if(!start(small+1,2048,1) || coax_scrypt_host_step(&host,4) || coax_scrypt_host_clean(&host)!=1)return 25;
  fail_state=1;if(coax_scrypt_host_clean(&host)!=3 || coax_scrypt_host_status(&host)!=2)return 26;
  if(coax_scrypt_host_clean(&host)!=0)return 27;
  fail_key=1;fail_output=1;
  if(coax_scrypt_host_publish(&host,output+1,32)!=2 || memcmp(output+1,expected,32))return 28;
  if(coax_scrypt_host_status(&host)!=-3 || coax_scrypt_host_close(&host) || !allzero(&host,sizeof host))return 29;
  memset(output+1,0,32);
  if(!start(small+1,2048,1) || coax_scrypt_host_step(&host,4) || coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=0)return 30;
  fail_key=1;if(coax_scrypt_host_publish(&host,output+1,32)!=2 || !allzero(output+1,32))return 31;
  if(coax_scrypt_host_close(&host) || !allzero(&host,sizeof host))return 32;
  if(!start(small+1,2048,1) || coax_scrypt_host_cancel(&host) || coax_scrypt_host_clean(&host)!=1 || coax_scrypt_host_clean(&host)!=1)return 34;
  fail_key=1;if(coax_scrypt_host_clean(&host)!=3 || coax_scrypt_host_status(&host)!=4)return 35;
  if(coax_scrypt_host_clean(&host)!=0 || coax_scrypt_host_close(&host))return 36;
#endif
  return 0;
}
#ifdef NATIVE_MAIN
int main(void){return scrypt_host_fixture();}
#endif
