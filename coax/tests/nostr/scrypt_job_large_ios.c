/* Full NIP-49-sized scrypt and erase in an explicitly selected simulator. */
#include "scrypt-job.h"
#include "zeroize_large.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
int main(void){
  const int64_t n=64LL*1024*1024;
  uint8_t password[5]={'n','o','s','t','r'},salt[16],result[32];
  const uint8_t expected[32]={0x16,0x29,0xc8,0xcc,0xd6,0x05,0xa8,0xc5,0x76,0xb2,0x5d,0xb7,0x76,0xcf,0x57,0x0c,0x59,0x98,0x0a,0xe9,0xbe,0xcb,0x9b,0x76,0x3e,0xf4,0x06,0xba,0x44,0x64,0x73,0x6d};
  for(int i=0;i<16;i++)salt[i]=(uint8_t)i;
  uint8_t *allocation=malloc((size_t)n+2);if(!allocation)return 1;
  allocation[0]=0xa5;allocation[n+1]=0xa5;uint8_t *memory=allocation+1;
  coax_t_ScryptJob session={0};coax_v_t_ScryptJob s={&session,1};
  if(!sj_start(s,(coax_v_u8){password,5},(coax_v_u8){salt,16},(coax_v_u8){memory,n},16,0))return 2;
  for(int i=0;i<2048 && sj_next(s)==-2;i++)if(!sj_step(s,(coax_v_u8){memory,n},64,0))return 3;
  if(sj_next(s)!=1)return 15;
  coax_large_wipe wiping={0};if(coax_large_wipe_begin(&wiping,memory,n))return 4;
  uint32_t status;int chunks=0;do{status=coax_large_wipe_step(&wiping);chunks++;}while(status==9);
  if(status || chunks!=64 || coax_large_wipe_reset(&wiping) || !sj_ack(s,1,1))return 5;
  for(int64_t i=0;i<n;i++)if(memory[i])return 6;
  if(allocation[0]!=0xa5 || allocation[n+1]!=0xa5)return 10;
  if(coax_secure_zero(&session.secret,sizeof session.secret,0,sizeof session.secret) || !sj_ack(s,2,1))return 7;
  const uint8_t *secret=(const uint8_t *)&session.secret;
  for(size_t i=0;i<sizeof session.secret;i++)if(secret[i])return 11;
  if(!sj_publish(s,(coax_v_u8){result,32},0) || memcmp(result,expected,32))return 8;
  if(coax_secure_zero(&session.key,sizeof session.key,0,sizeof session.key) || coax_secure_zero(result,sizeof result,0,sizeof result) || coax_secure_zero(password,sizeof password,0,sizeof password))return 9;
  for(size_t i=0;i<sizeof session.key;i++)if(session.key.e[i])return 12;
  for(size_t i=0;i<sizeof result;i++)if(result[i])return 13;
  for(size_t i=0;i<sizeof password;i++)if(password[i])return 14;
  free(allocation);return 0;
}
