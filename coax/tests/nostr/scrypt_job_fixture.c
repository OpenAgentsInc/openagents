/* End-to-end prototype: host supplies actual erase effects and acknowledgments. */
#include "scrypt-job.h"
#include "zeroize_large.h"
#include <stddef.h>
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static coax_t_ScryptJob state;
static uint8_t scratch[2050],large[2097154],output[34],password[5]={'n','o','s','t','r'},salt[16];
static coax_v_t_ScryptJob S(void){return (coax_v_t_ScryptJob){&state,1};}
static coax_v_u8 M(void){return (coax_v_u8){scratch+1,2048};}
static int wipe(int region){
  void *p=region==1?(void *)(scratch+1):region==2?(void *)&state.secret:(void *)&state.key;
  size_t n=region==1?2048:region==2?sizeof state.secret:sizeof state.key;
  if(coax_secure_zero(p,n,0,n))return 0;
  return sj_ack(S(),region,1);
}
static int clean(void){
  int region;
  while((region=(int)sj_next(S()))>0)if(!wipe(region))return 0;
  if(region!=0)return 0;
  if(scratch[0]!=0xa5 || scratch[2049]!=0xa5)return 0;
  for(size_t i=1;i<2049;i++)if(scratch[i])return 0;
  const uint8_t *secret=(const uint8_t *)&state.secret;
  for(size_t i=0;i<sizeof state.secret;i++)if(secret[i])return 0;
  if(state.policy.reason!=1)for(size_t i=0;i<sizeof state.key;i++)if(state.key.e[i])return 0;
  return 1;
}
static int start(void){
  memset(&state,0,sizeof state);memset(scratch,0xa5,sizeof scratch);memset(output,0xa5,sizeof output);
  for(int i=0;i<16;i++)salt[i]=(uint8_t)i;
  return sj_start(S(),(coax_v_u8){password,5},(coax_v_u8){salt,16},M(),1,0);
}
int scrypt_job_fixture(void){
  static const uint8_t expected[32]={0x34,0xb8,0x3a,0xe9,0xbd,0x8d,0x9d,0x3b,0xbb,0x6a,0xa6,0xf7,0x3a,0xc6,0xb4,0x22,0x9f,0xb5,0xbb,0x49,0x04,0x43,0x17,0xd3,0x92,0xc7,0x32,0x35,0x1a,0xb5,0x1c,0x17};
  if(!start() || sj_publish(S(),(coax_v_u8){output+1,32},0))return 1;
  if(!sj_step(S(),M(),4,0) || sj_next(S())!=1)return 2;
  if(sj_ack(S(),1,0) || sj_publish(S(),(coax_v_u8){output+1,32},0))return 3;
  if(!clean() || !sj_publish(S(),(coax_v_u8){output+1,32},0))return 4;
  if(memcmp(output+1,expected,32) || output[0]!=0xa5 || output[33]!=0xa5)return 5;
  if(coax_secure_zero(&state.key,sizeof state.key,0,sizeof state.key))return 6;
  if(!start() || !sj_step(S(),M(),4,0) || !wipe(1) || !sj_cancel(S()) || !clean())return 7;
  if(sj_publish(S(),(coax_v_u8){output+1,32},0))return 8;
  if(!start() || sj_step(S(),(coax_v_u8){scratch+1,1024},1,0) || !clean())return 9;
  if(sj_publish(S(),(coax_v_u8){output+1,32},0))return 10;
  memset(&state,0,sizeof state);memset(large,0xa5,sizeof large);
  coax_v_u8 lm={large+1,2097152};
  if(!sj_start(S(),(coax_v_u8){password,5},(coax_v_u8){salt,16},lm,11,0) || !sj_step(S(),lm,1,0) || !sj_cancel(S()))return 11;
  int wrote=0;for(size_t i=1;i<1025;i++)if(large[i]!=0xa5)wrote=1;if(!wrote)return 17;
  coax_large_wipe large_wipe={0};
  if(coax_large_wipe_begin(&large_wipe,large+1,2097152) || coax_large_wipe_step(&large_wipe)!=9)return 12;
  if(coax_large_wipe_step(&large_wipe) || coax_large_wipe_reset(&large_wipe) || !sj_ack(S(),1,1))return 13;
  for(size_t i=1;i<2097153;i++)if(large[i])return 14;
  if(large[0]!=0xa5 || large[2097153]!=0xa5 || !wipe(2) || !wipe(4))return 15;
  const uint8_t *secret=(const uint8_t *)&state.secret;
  for(size_t i=0;i<sizeof state.secret;i++)if(secret[i])return 18;
  for(size_t i=0;i<sizeof state.key;i++)if(state.key.e[i])return 19;
  if(sj_publish(S(),(coax_v_u8){output+1,32},0))return 16;
  return 0;
}
#ifdef NATIVE_MAIN
int main(void){return scrypt_job_fixture();}
#endif
