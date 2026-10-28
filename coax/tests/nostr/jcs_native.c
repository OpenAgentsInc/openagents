/* Bounded public-fixture input and sanitizer/span observations, no JSON policy. */
#include "jcs.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define LIMIT 524288
static uint8_t input[LIMIT+32],out[LIMIT+32],expected[LIMIT+32],snapshot[LIMIT+32];
static uint32_t little(const uint8_t *p){return(uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;}
static coax_t_JcsResult call(int64_t n,int64_t cap){return jcs_canonicalize((coax_string){input,n},(coax_v_u8){out,cap});}
int main(void){
 uint8_t header[12];unsigned records=0;
 while(fread(header,1,12,stdin)==12){
  uint32_t n=little(header),m=little(header+4),error=little(header+8);assert(n<=LIMIT+1&&m<=LIMIT);
  assert(fread(input,1,n,stdin)==n&&fread(expected,1,m,stdin)==m);memcpy(snapshot,input,n);memset(out,165,sizeof out);
  coax_t_JcsResult r=call(n,error?LIMIT:m);assert(r.error==error&&r.written==(error?0:m));
  if(error){for(unsigned i=0;i<sizeof out;i++)assert(out[i]==165);}
  else {assert(memcmp(out,expected,m)==0);for(unsigned i=m;i<m+8;i++)assert(out[i]==165);memset(out,165,sizeof out);r=call(n,m-1);assert(r.error==5&&r.written==0);for(unsigned i=0;i<sizeof out;i++)assert(out[i]==165);}
  assert(memcmp(input,snapshot,n)==0);records++;
 }
 memcpy(input,"{\"a\":1}",7);memcpy(snapshot,input,32);
 coax_t_JcsResult a=jcs_canonicalize((coax_string){input,7},(coax_v_u8){input,7});assert(a.error==6&&!a.written);
 a=jcs_canonicalize((coax_string){input,7},(coax_v_u8){input+1,7});assert(a.error==6&&!a.written);assert(memcmp(input,snapshot,32)==0);
 a=call(-1,LIMIT);assert(a.error==1&&!a.written);a=call(7,-1);assert(a.error==5&&!a.written);a=call(7,LIMIT+1);assert(a.error==5&&!a.written);
 printf("%u native independent/limit records, transactional short outputs and span/alias cases passed\n",records);return 0;
}
