#include "aead_host.h"
#include <stdint.h>
#include <string.h>
static coax_aead_host host;static coax_t_AeadScratch scratch;
static uint8_t key[32],nonce[24],aad[12],input[130],output[131];
static const uint8_t expected[130]={0xbd,0x6d,0x17,0x9d,0x3e,0x83,0xd4,0x3b,0x95,0x76,0x57,0x94,0x93,0xc0,0xe9,0x39,0x57,0x2a,0x17,0x00,0x25,0x2b,0xfa,0xcc,0xbe,0xd2,0x90,0x2c,0x21,0x39,0x6c,0xbb,0x73,0x1c,0x7f,0x1b,0x0b,0x4a,0xa6,0x44,0x0b,0xf3,0xa8,0x2f,0x4e,0xda,0x7e,0x39,0xae,0x64,0xc6,0x70,0x8c,0x54,0xc2,0x16,0xcb,0x96,0xb7,0x2e,0x12,0x13,0xb4,0x52,0x2f,0x8c,0x9b,0xa4,0x0d,0xb5,0xd9,0x45,0xb1,0x1b,0x69,0xb9,0x82,0xc1,0xbb,0x9e,0x3f,0x3f,0xac,0x2b,0xc3,0x69,0x48,0x8f,0x76,0xb2,0x38,0x35,0x65,0xd3,0xff,0xf9,0x21,0xf9,0x66,0x4c,0x97,0x63,0x7d,0xa9,0x76,0x88,0x12,0xf6,0x15,0xc6,0x8b,0x13,0xb5,0x2e,0xc0,0x87,0x59,0x24,0xc1,0xc7,0x98,0x79,0x47,0xde,0xaf,0xd8,0x78,0x0a,0xcf,0x49};
static const char *text="Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
static int zero(const void *p,size_t n){const uint8_t *b=p;for(size_t i=0;i<n;i++)if(b[i])return 0;return 1;}
static int sentinel(const void *p,size_t n){const uint8_t *b=p;for(size_t i=0;i<n;i++)if(b[i]!=165)return 0;return 1;}
#ifdef COAX_AEAD_TEST_EFFECT
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static int fail_work,fail_stage,fail_output;
uint32_t coax_aead_test_zero(void *p,int64_t n,int64_t off,int64_t len){
 if(p==&scratch && fail_work){fail_work=0;return 8;}if(p==&host.operation.staged && fail_stage){fail_stage=0;return 8;}if(p==output && fail_output){fail_output=0;return 8;}
 return coax_secure_zero(p,n,off,len);
}
#endif
static void setup(void){
 memset(&host,0,sizeof host);memset(&scratch,165,sizeof scratch);memset(output,165,sizeof output);
 for(int i=0;i<32;i++)key[i]=(uint8_t)(128+i);for(int i=0;i<24;i++)nonce[i]=(uint8_t)(64+i);
 const uint8_t a[12]={0x50,0x51,0x52,0x53,0xc0,0xc1,0xc2,0xc3,0xc4,0xc5,0xc6,0xc7};memcpy(aad,a,12);
}
static int start(int seal,int n,int cap){return coax_aead_host_start(&host,&scratch,key,32,nonce,24,aad,12,input,n,output,cap,seal);}
int aead_host_fixture(void){
 setup();memcpy(input,text,114);if(start(1,114,130)||coax_aead_host_status(&host)!=1)return 1;
 if(!coax_aead_host_publish(&host)||output[0]!=165)return 2;
 if(coax_aead_host_clean(&host)||!zero(&scratch,sizeof scratch))return 3;
 if(coax_aead_host_publish(&host)||memcmp(output,expected,130)||!zero(&host.operation.staged,sizeof host.operation.staged))return 4;
 if(coax_aead_host_close(&host)||!zero(&host,sizeof host))return 5;
 for(int i=114;i<130;i++){
  setup();memcpy(input,expected,130);input[i]^=1;
  if(start(0,130,114)!=1||coax_aead_host_status(&host)!=1)return 6;
  if(coax_aead_host_clean(&host)!=1||coax_aead_host_clean(&host)||!zero(&scratch,sizeof scratch)||!zero(&host.operation.staged,sizeof host.operation.staged))return 7;
  if(!coax_aead_host_publish(&host)||output[0]!=165||coax_aead_host_close(&host))return 8;
 }
 for(int which=0;which<3;which++){
  setup();memcpy(input,expected,130);if(which==0)input[0]^=1;else if(which==1)nonce[0]^=1;else aad[0]^=1;
  if(start(0,130,114)!=1||coax_aead_host_clean(&host)!=1||coax_aead_host_clean(&host)||!sentinel(output,sizeof output)||coax_aead_host_close(&host))return 24;
 }
 setup();memcpy(input,expected,130);if(start(0,130,114)||coax_aead_host_clean(&host))return 9;
 if(coax_aead_host_publish(&host)||memcmp(output,text,114)||coax_aead_host_close(&host))return 10;
 setup();memcpy(input,text,114);if(start(1,114,130)||coax_aead_host_clean(&host)||!coax_aead_host_close(&host))return 11;
 if(coax_aead_host_status(&host)!=2||coax_aead_host_clean(&host)||coax_aead_host_close(&host))return 12;
 setup();if(!coax_aead_host_start(&host,&scratch,key,32,nonce,24,aad,12,input,114,(uint8_t *)&scratch,130,1)||!sentinel(&scratch,sizeof scratch))return 13;
 if(!coax_aead_host_start((coax_aead_host *)((uint8_t *)&host+1),&scratch,key,32,nonce,24,aad,12,input,114,output,130,1))return 14;
 if(!coax_aead_host_start(&host,&scratch,key,32,nonce,24,aad,12,(const uint8_t *)(UINTPTR_MAX-1),114,output,130,1))return 15;
#ifdef COAX_AEAD_TEST_EFFECT
 setup();memcpy(input,text,114);if(start(1,114,130))return 16;
 fail_work=1;if(coax_aead_host_clean(&host)!=3||coax_aead_host_status(&host)!=1||coax_aead_host_clean(&host))return 17;
 fail_stage=1;fail_output=1;if(coax_aead_host_publish(&host)!=2||memcmp(output,expected,130))return 18;
 if(coax_aead_host_close(&host)||!zero(&host,sizeof host))return 19;
 setup();memcpy(input,text,114);if(start(1,114,130)||coax_aead_host_clean(&host))return 20;
 fail_stage=1;if(coax_aead_host_publish(&host)!=2||!zero(output,130)||coax_aead_host_close(&host))return 21;
 setup();memcpy(input,expected,130);input[129]^=1;if(start(0,130,114)!=1||coax_aead_host_clean(&host)!=1)return 22;
 fail_stage=1;if(coax_aead_host_clean(&host)!=3||coax_aead_host_status(&host)!=2||coax_aead_host_clean(&host)||coax_aead_host_close(&host))return 23;
#endif
 return 0;
}
#ifdef NATIVE_MAIN
int main(void){return aead_host_fixture();}
#endif
