/* Public input fixtures; the test observes workspace, host and scratch wipes. */
#include "scrypt_text_host.h"
#include <stdint.h>
#include <string.h>
static coax_scrypt_host host;
static struct {uint8_t before;coax_scrypt_text_workspace work;uint8_t after;} guarded;
static uint8_t memory[2050],salt[16],output[32];
static const uint8_t expected[32]={0x95,0x8f,0x3e,0x9f,0x16,0xc3,0x3f,0xe6,0x60,0x42,0x5a,0x09,0x8b,0x1d,0xa4,0xd4,0x02,0x84,0xba,0x2e,0xa0,0xed,0x8c,0x8e,0x83,0xc6,0x52,0xa0,0x6f,0xf5,0x7e,0xfe};
static int zero(const void *p,size_t n){const uint8_t *b=p;for(size_t i=0;i<n;i++)if(b[i])return 0;return 1;}
#ifdef COAX_SCRYPT_TEXT_TEST_EFFECT
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static int fail_work;
uint32_t coax_scrypt_text_test_zero(void *p,int64_t n,int64_t off,int64_t len){if(p==&guarded.work && fail_work){fail_work=0;return 8;}return coax_secure_zero(p,n,off,len);}
#endif
static void reset(void){memset(&host,0,sizeof host);memset(&guarded,165,sizeof guarded);memset(memory,165,sizeof memory);memset(output,165,sizeof output);for(int i=0;i<16;i++)salt[i]=(uint8_t)i;}
static int start(const uint8_t *p,int64_t n){return coax_scrypt_text_start(&host,&guarded.work,p,n,salt,16,memory+1,2048,1);}
static int cleared(void){return guarded.before==165 && guarded.after==165 && zero(&guarded.work,sizeof guarded.work);}
int scrypt_text_host_fixture(void){
 const uint8_t compat[3]={0xe2,0x84,0xab},canonical[2]={0xc3,0x85};
 for(int k=0;k<2;k++){
  reset();if(start(k?canonical:compat,k?2:3) || !cleared())return 1;
  memset(&guarded.work,165,sizeof guarded.work);
  if(start(k?canonical:compat,k?2:3)!=1)return 15;
  const uint8_t *active=(const uint8_t *)&guarded.work;for(size_t i=0;i<sizeof guarded.work;i++)if(active[i]!=165)return 16;
  memset(&guarded.work,0,sizeof guarded.work);if(coax_scrypt_host_step(&host,4))return 17;
  if(coax_scrypt_host_clean(&host)!=1||coax_scrypt_host_clean(&host)!=0)return 2;
  if(coax_scrypt_host_publish(&host,output,32)||memcmp(output,expected,32)||coax_scrypt_host_close(&host))return 3;
  if(memory[0]!=165||memory[2049]!=165||!zero(memory+1,2048))return 4;
 }
 reset();const uint8_t bad[2]={0xc0,0x80};if(start(bad,2)!=1||!cleared()||coax_scrypt_host_status(&host)!=-1)return 5;
 reset();uint8_t expanded[180];for(int i=0;i<60;i++){expanded[3*i]=0xef;expanded[3*i+1]=0xb7;expanded[3*i+2]=0xba;}
 if(start(expanded,sizeof expanded)!=1||!cleared()||coax_scrypt_host_status(&host)!=-1)return 6;
 reset();if(!coax_scrypt_text_start(&host,&guarded.work,(const uint8_t *)&guarded.work,1,salt,16,memory+1,2048,1))return 7;
 const uint8_t *untouched=(const uint8_t *)&guarded.work;for(size_t i=0;i<sizeof guarded.work;i++)if(untouched[i]!=165)return 14;
 if(!coax_scrypt_text_start(&host,(coax_scrypt_text_workspace *)&host,canonical,2,salt,16,memory+1,2048,1))return 8;
#ifdef COAX_SCRYPT_TEXT_TEST_EFFECT
 reset();fail_work=1;if(start(compat,3)!=2 || zero(&guarded.work,sizeof guarded.work) || coax_scrypt_host_status(&host)!=1)return 9;
 if(coax_secure_zero(&guarded.work,sizeof guarded.work,0,sizeof guarded.work)||!cleared())return 10;
 if(coax_scrypt_host_clean(&host)!=1||coax_scrypt_host_clean(&host)!=1||coax_scrypt_host_clean(&host)!=0||coax_scrypt_host_close(&host))return 11;
 reset();fail_work=1;if(start(bad,2)!=2||zero(&guarded.work,sizeof guarded.work)||coax_scrypt_host_status(&host)!=-1)return 12;
 if(coax_secure_zero(&guarded.work,sizeof guarded.work,0,sizeof guarded.work)||!cleared())return 13;
#endif
 return 0;
}
#ifdef NATIVE_MAIN
int main(void){return scrypt_text_host_fixture();}
#endif
