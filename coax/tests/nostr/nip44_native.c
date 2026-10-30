/* Native ASan/UBSan observation of the nip44-memory.coax exports.
   Usage: nip44_native N PROFILE > payload.b64  (N bytes of 'a', the pinned
   NIP's extended-table conversation key and nonce). Encrypts, writes the
   payload to stdout, decrypts it back, and checks the overlap refusals.
   N above the profile maximum must be refused as capacity (exit 3). */
#include "nip44.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define MAXN 917505
#define WORK 1835200
#define CHARS 1223440
static uint8_t plain[MAXN],work[WORK],out[CHARS],back[MAXN],dwork[WORK];
static coax_v_u8 bytes(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
int main(int argc,char **argv){
  assert(argc==3);int64_t n=atoll(argv[1]);uint32_t profile=(uint32_t)atoi(argv[2]);assert(n>=1&&n<=MAXN);
  static const uint8_t ckc[32]={0xc4,0x1c,0x77,0x53,0x56,0xfd,0x92,0xea,0xdc,0x63,0xff,0x5a,0x0d,0xc1,0xda,0x21,0x1b,0x26,0x8c,0xbe,0xa2,0x23,0x16,0x76,0x70,0x95,0xb2,0x87,0x1e,0xa1,0x41,0x2d};
  uint8_t ck[32],nonce[32]={0};memcpy(ck,ckc,32);nonce[31]=1;
  memset(plain,'a',(size_t)n);memset(work,165,sizeof work);memset(out,165,sizeof out);
  coax_t_Nip44Result r=nip44_encrypt(profile,bytes(ck,32),bytes(nonce,32),bytes(plain,n),bytes(work,WORK),bytes(out,CHARS));
  if(r.error==7){for(int i=0;i<CHARS;i++)assert(out[i]==165);return 3;}
  assert(r.error==0&&r.written==nip44_payload_size(profile,n));
  /* overlap: out inside work */
  coax_t_Nip44Result a=nip44_encrypt(profile,bytes(ck,32),bytes(nonce,32),bytes(plain,n),bytes(work,WORK),bytes(work+8,CHARS));
  assert(a.error==10);
  coax_string text={(const char*)out,r.written};
  coax_t_Nip44Result d=nip44_decrypt(profile,bytes(ck,32),text,bytes(dwork,WORK),bytes(back,MAXN));
  assert(d.error==0&&d.written==n&&memcmp(back,plain,(size_t)n)==0);
  coax_string inside={(const char*)dwork,r.written};memcpy(dwork,out,(size_t)r.written);
  coax_t_Nip44Result b=nip44_decrypt(profile,bytes(ck,32),inside,bytes(dwork,WORK),bytes(back,MAXN));
  assert(b.error==10);
  out[r.written-2]^=1; /* flip a MAC character: refused before plaintext */
  memset(back,165,sizeof back);
  coax_t_Nip44Result m=nip44_decrypt(profile,bytes(ck,32),text,bytes(dwork,WORK),bytes(back,MAXN));
  assert(m.error==4||m.error==3);for(int64_t i=0;i<n;i++)assert(back[i]==165);
  out[r.written-2]^=1;
  fwrite(out,1,(size_t)r.written,stdout);
  return 0;
}
