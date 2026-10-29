/* Public vectors; a test host supplies observable wipes and acknowledgments. */
#include "aead-workspace.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],nonce[24],aad[1024],input[4112],output[4113];
static coax_t_AeadOperation operation;static coax_t_AeadScratch scratch;
static coax_v_t_AeadOperation state(void){return (coax_v_t_AeadOperation){&operation,1};}
static coax_v_t_AeadScratch work(void){return (coax_v_t_AeadScratch){&scratch,1};}
static int zero(const void *p,size_t n){const uint8_t *b=p;for(size_t i=0;i<n;i++)if(b[i])return 0;return 1;}
static int wipe(int region){
 void *p=region==1?(void *)&scratch:(void *)&operation.staged;
 size_t n=region==1?sizeof scratch:sizeof operation.staged;
 return !coax_secure_zero(p,n,0,n)&&zero(p,n)&&aw_ack(state(),region,1);
}
static int calculate(int an,int n,int seal,int capacity){
 memset(&operation,0,sizeof operation);memset(&scratch,0,sizeof scratch);
 int ok=aw_start(state(),work(),(coax_v_u8){key,32},(coax_v_u8){nonce,24},(coax_v_u8){aad,an},(coax_v_u8){input,n},seal,0);
 if(aw_next(state())!=1||!wipe(1))return -1;
 if(!ok){if(aw_next(state())!=2||!wipe(2)||aw_next(state())!=0)return -1;return 0;}
 if(aw_next(state())!=0||!aw_publish(state(),(coax_v_u8){output,capacity},0))return -1;
 if(coax_secure_zero(&operation.staged,sizeof operation.staged,0,sizeof operation.staged)||!zero(&operation.staged,sizeof operation.staged))return -1;
 return 1;
}
uint8_t *aead_key(void){return key;} uint8_t *aead_nonce(void){return nonce;} uint8_t *aead_aad(void){return aad;} uint8_t *aead_input(void){return input;} uint8_t *aead_output(void){return output;}
int aead_seal(int an,int n){output[n+16]=165;return calculate(an,n,1,n+17)==1&&output[n+16]==165;}
int aead_open(int an,int n){output[n-16]=165;return calculate(an,n,0,n-15)==1&&output[n-16]==165;}
int aead_tamper(int an,int n,int at){
 memset(output,165,sizeof output);input[at]^=1;int ok=calculate(an,n,0,n-16);input[at]^=1;
 if(ok!=0)return 0;for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;
}
static int unchanged(void){for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;}
int aead_nonce_tamper(int an,int n){
 memset(output,165,sizeof output);nonce[0]^=1;int r=calculate(an,n,0,n-16);nonce[0]^=1;return r==0&&unchanged();
}
int aead_aad_tamper(int an,int n){
 if(an<=0)return 0;memset(output,165,sizeof output);aad[0]^=1;int r=calculate(an,n,0,n-16);aad[0]^=1;return r==0&&unchanged();
}
