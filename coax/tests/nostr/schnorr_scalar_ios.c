#include <stdint.h>
#include <string.h>
uint8_t *ss_input(void),*ss_output(void);uint64_t ss_valid(void);int ss_run(int),ss_refusal(void);
int main(void){
 const uint8_t n[32]={0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xfe,0xba,0xae,0xdc,0xe6,0xaf,0x48,0xa0,0x3b,0xbf,0xd2,0x5e,0x8c,0xd0,0x36,0x41,0x41};
 memset(ss_input(),0,32);ss_input()[31]=3;if(!ss_run(0)||ss_valid()!=1||ss_output()[31]!=3)return 1;
 if(!ss_run(1)||ss_valid()!=1||memcmp(ss_output(),n,31)||ss_output()[31]!=(uint8_t)(n[31]-3))return 2;
 memcpy(ss_input(),n,32);if(!ss_run(0)||ss_valid()!=0||ss_output()[31]!=1||!ss_refusal())return 3;return 0;
}
