#include <stdint.h>
#include <string.h>
uint8_t *sm_key(void),*sm_nonce(void),*sm_challenge(void),*sm_output(void);uint64_t sm_valid(void);int sm_run(int,int),sm_refusal(void);
int main(void){memset(sm_key(),0,32);memset(sm_nonce(),0,32);memset(sm_challenge(),0,32);sm_key()[31]=3;sm_nonce()[31]=5;sm_challenge()[31]=2;
 if(!sm_run(0,0)||sm_valid()!=1||sm_output()[31]!=11)return 1;if(!sm_run(0,1)||sm_output()[31]!=1)return 2;
 memset(sm_nonce(),0,32);if(!sm_run(0,0)||sm_valid()!=0||!sm_refusal())return 3;return 0;}
