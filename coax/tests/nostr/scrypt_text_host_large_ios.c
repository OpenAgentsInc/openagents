/* Full public NFKC+scrypt vector, simulator host bridge. */
#include "scrypt_text_host.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static coax_scrypt_host host;static coax_scrypt_text_workspace work;
int main(void){
 const int64_t n=64LL*1024*1024;const uint8_t raw[3]={0xe2,0x84,0xab};uint8_t salt[16],out[32];for(int i=0;i<16;i++)salt[i]=(uint8_t)i;
 const uint8_t expected[32]={0x7b,0xe8,0x55,0xa3,0x67,0x23,0xa3,0x48,0x29,0x5c,0x59,0x81,0x6d,0xa5,0x32,0x37,0x5e,0xc3,0xf8,0xd9,0xac,0x89,0x74,0x91,0x82,0x31,0xde,0x9a,0x81,0x3a,0x6e,0xcd};
 uint8_t *allocation=malloc((size_t)n+2);if(!allocation)return 1;allocation[0]=allocation[n+1]=165;uint8_t *memory=allocation+1;
 if(coax_scrypt_text_start(&host,&work,raw,3,salt,16,memory,n,16))return 2;
 const uint8_t *w=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(w[i])return 3;
 for(int i=0;i<2048&&coax_scrypt_host_status(&host)==-2;i++)if(coax_scrypt_host_step(&host,64))return 4;
 if(coax_scrypt_host_status(&host)!=1)return 5;
 for(int i=0;i<64;i++)if(coax_scrypt_host_clean(&host)!=1)return 6;
 if(coax_scrypt_host_clean(&host)!=0)return 7;
 for(int64_t i=0;i<n;i++)if(memory[i])return 8;
 if(allocation[0]!=165||allocation[n+1]!=165)return 9;
 if(coax_scrypt_host_publish(&host,out,32)||memcmp(out,expected,32)||coax_scrypt_host_close(&host))return 10;
 const uint8_t *h=(const uint8_t *)&host;for(size_t i=0;i<sizeof host;i++)if(h[i])return 11;
 free(allocation);return 0;
}
