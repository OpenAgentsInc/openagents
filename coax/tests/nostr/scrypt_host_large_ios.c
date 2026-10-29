/* Full-size public-vector end-to-end host bridge test on a simulator. */
#include "scrypt_host.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
int main(void){
 const int64_t n=64LL*1024*1024;
 const uint8_t expected[32]={0x16,0x29,0xc8,0xcc,0xd6,0x05,0xa8,0xc5,0x76,0xb2,0x5d,0xb7,0x76,0xcf,0x57,0x0c,0x59,0x98,0x0a,0xe9,0xbe,0xcb,0x9b,0x76,0x3e,0xf4,0x06,0xba,0x44,0x64,0x73,0x6d};
 uint8_t password[5]={'n','o','s','t','r'},salt[16],result[32];for(int i=0;i<16;i++)salt[i]=(uint8_t)i;
 uint8_t *allocation=malloc((size_t)n+2);if(!allocation)return 1;allocation[0]=allocation[n+1]=165;uint8_t *memory=allocation+1;
 coax_scrypt_host host={0};if(coax_scrypt_host_start(&host,password,5,salt,16,memory,n,16))return 2;
 for(int i=0;i<2048&&coax_scrypt_host_status(&host)==-2;i++)if(coax_scrypt_host_step(&host,64))return 3;
 if(coax_scrypt_host_status(&host)!=1)return 4;
 for(int i=0;i<64;i++)if(coax_scrypt_host_clean(&host)!=1)return 5;
 if(coax_scrypt_host_clean(&host)!=0)return 6;
 for(int64_t i=0;i<n;i++)if(memory[i])return 7;
 if(allocation[0]!=165||allocation[n+1]!=165)return 8;
 if(coax_scrypt_host_publish(&host,result,32)||memcmp(result,expected,32))return 9;
 if(coax_scrypt_host_close(&host))return 10;
 const uint8_t *h=(const uint8_t *)&host;for(size_t i=0;i<sizeof host;i++)if(h[i])return 11;
 free(allocation);return 0;
}
