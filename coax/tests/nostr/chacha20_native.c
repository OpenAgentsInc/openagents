/* Bounded binary public-fixture reader and sanitizer ABI observations only. */
#include "chacha20.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define LIMIT 1048576
static uint8_t key[128],nonce[32],input[LIMIT+16],output[LIMIT+16],expected[LIMIT],back[LIMIT];
static coax_v_u8 bytes(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
static uint32_t little(const uint8_t *p){return(uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;}
int main(void){
  uint8_t header[9];unsigned records=0;
  while(fread(header,1,9,stdin)==9){
    uint32_t n=little(header),counter=little(header+4);assert(n<=LIMIT);
    assert(fread(key,1,32,stdin)==32&&fread(nonce,1,12,stdin)==12);
    assert(fread(input,1,n,stdin)==n&&fread(expected,1,n,stdin)==n);
    memset(output,165,sizeof output);int64_t cap=n>LIMIT-8?LIMIT:n+8;
    coax_t_ChaChaResult r=header[8]?chacha20_block(bytes(key,32),bytes(nonce,12),counter,bytes(output,cap)):chacha20_xor(bytes(key,32),bytes(nonce,12),counter,bytes(input,n),bytes(output,cap));
    assert(!r.error&&r.written==n&&memcmp(output,expected,n)==0);
    for(unsigned i=n;i<n+8;i++)assert(output[i]==165);
    if(!header[8]){r=chacha20_xor(bytes(key,32),bytes(nonce,12),counter,bytes(output,n),bytes(back,n));assert(!r.error&&r.written==n&&memcmp(input,back,n)==0);}
    records++;
  }
  memset(output,165,sizeof output);memset(key,165,sizeof key);
  uint8_t input_guard[128],nonce_guard[32];memcpy(input_guard,input,128);memcpy(nonce_guard,nonce,32);
  coax_t_ChaChaResult failures[]={
    chacha20_block(bytes(key,31),bytes(nonce,12),0,bytes(output,64)),
    chacha20_block(bytes(key,32),bytes(nonce,11),0,bytes(output,64)),
    chacha20_block(bytes(key,32),bytes(nonce,12),0,bytes(output,63)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(input,-1),bytes(output,64)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(input,LIMIT+1),bytes(output,LIMIT)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),UINT32_MAX,bytes(input,65),bytes(output,65)),
    chacha20_block(bytes(key,32),bytes(nonce,12),0,bytes(key,64)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(key+32,65),bytes(key+33,65)),
    chacha20_xor(bytes(key,-1),bytes(nonce,12),0,bytes(input,1),bytes(output,1)),
    chacha20_xor(bytes(key,32),bytes(nonce,-1),0,bytes(input,1),bytes(output,1)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(input,1),bytes(output,-1)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(input,1),bytes(output,LIMIT+1)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(input,65),bytes(input,65)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0,bytes(input,1),bytes(nonce,12)),
    chacha20_xor(bytes(key,32),bytes(nonce,12),0xffffc001,bytes(input,LIMIT),bytes(output,LIMIT))
  };
  const uint32_t wanted[]={1,2,4,3,3,5,6,6,1,2,4,4,6,6,5};
  for(unsigned i=0;i<sizeof wanted/sizeof wanted[0];i++)assert(failures[i].error==wanted[i]&&failures[i].written==0);
  for(unsigned i=0;i<sizeof output;i++)assert(output[i]==165);
  for(unsigned i=0;i<sizeof key;i++)assert(key[i]==165);
  assert(memcmp(input,input_guard,128)==0&&memcmp(nonce,nonce_guard,32)==0);
  printf("%u independent records and native refusal/alias boundaries passed\n",records);return 0;
}
