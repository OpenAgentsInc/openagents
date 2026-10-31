#include "panda-session.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
static size_t checks;
#define NEED(x)do{checks++;if(!(x)){fprintf(stderr,"hash bridge line%d\n",__LINE__);exit(1);}}while(0)
int main(int argc,char**argv){
 NEED(argc==2);FILE*f=fopen(argv[1],"rb");NEED(f);uint32_t head[2];NEED(fread(head,4,2,f)==2&&head[0]==UINT32_C(0x31485150)&&head[1]==10);uint64_t lengths[10];uint8_t expected[10][32];NEED(fread(lengths,8,10,f)==10&&fread(expected,32,10,f)==10);unsigned char*data=malloc(16777216);NEED(data&&fread(data,1,16777216,f)==16777216&&fgetc(f)==EOF&&!fclose(f));
 for(int i=0;i<10;i++){uint8_t out[33],old[33];memset(out,0xa5,33);memcpy(old,out,33);NEED(!pqn_file_hash((coax_v_u8){data,(int64_t)lengths[i]},(coax_v_u8){out,32},0)&&!memcmp(out,expected[i],32)&&out[32]==0xa5);NEED(pqn_file_hash((coax_v_u8){data,(int64_t)lengths[i]},(coax_v_u8){out,32},1)==5);}
 uint8_t out[33],prior[33];memset(out,0xa5,33);memcpy(prior,out,33);int64_t bad[]={-1,16777217};for(int i=0;i<2;i++){NEED(pqn_file_hash((coax_v_u8){data,bad[i]},(coax_v_u8){out,32},0)==2&&!memcmp(out,prior,33));}NEED(pqn_file_hash((coax_v_u8){data,1},(coax_v_u8){out,31},0)==3&&!memcmp(out,prior,33));NEED(pqn_file_hash((coax_v_u8){data,1},(coax_v_u8){out,33},0)==3&&!memcmp(out,prior,33));free(data);printf("{\"passed\":true,\"vectors\":10,\"max_file_bytes\":16777216,\"assertions\":%zu,\"SDK_API_calls\":0,\"NN_calls\":0}\n",checks);return 0;
}
