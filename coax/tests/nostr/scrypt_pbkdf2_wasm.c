#include "scrypt_pbkdf2.h"
#include <stdint.h>
static uint8_t password[1024],salt[1024],output[1025];
uint8_t *pb_password(void) { return password; }
uint8_t *pb_salt(void) { return salt; }
uint8_t *pb_output(void) { return output; }
int pb_run(int pn,int sn,int n) {
  output[n]=165;
  return scrypt_pbkdf2_one((coax_v_u8){password,pn},(coax_v_u8){salt,sn},n,(coax_v_u8){output,n+1},0) && output[n]==165;
}
int pb_refusal(int mode) {
  for(int i=0;i<1025;i++) output[i]=165;
  int ok=scrypt_pbkdf2_one((coax_v_u8){password,mode==0?-1:32},(coax_v_u8){salt,mode==1?-1:16},
      mode==2?0:32,(coax_v_u8){output,mode==3?31:1025},mode==4);
  if(ok) return 0;
  for(int i=0;i<1025;i++) if(output[i]!=165) return 0;
  return 1;
}
