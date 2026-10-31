#include "panda-session.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
static size_t checks;
#define NEED(x) do{checks++;if(!(x)){fprintf(stderr,"runtime gate line%d\n",__LINE__);exit(1);}}while(0)
static void read_bytes(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}
static void equal_string(coax_string s,const char*value){size_t n=strlen(value);NEED(s.count==(int64_t)n&&!memcmp(s.data,value,n));}
static void hex(const uint8_t b[32],char s[65]){for(int i=0;i<32;i++)snprintf(s+2*i,3,"%02x",(unsigned)b[i]);s[64]=0;}
int main(int argc,char**argv){
 NEED(argc==2);FILE*f=fopen(argv[1],"rb");NEED(f);uint32_t head[2];read_bytes(f,head,sizeof head);NEED(head[0]==UINT32_C(0x314e5150)&&head[1]==66846);unsigned char*xml=malloc(66847);NEED(xml);read_bytes(f,xml,66846);uint8_t ids[3][33]={{0}},assets[2721]={0};for(int i=0;i<3;i++)read_bytes(f,ids[i],32);read_bytes(f,assets,2720);int64_t ranges[85][2];read_bytes(f,ranges,sizeof ranges);NEED(fgetc(f)==EOF&&!fclose(f));
 for(int role=0;role<3;role++){char text[65];hex(ids[role],text);equal_string(pqn_expected(role),text);}equal_string(pqn_expected(-1),"");equal_string(pqn_expected(3),"");
 for(int i=0;i<85;i++){coax_t_PqnRange r=pqn_asset_range(i);NEED(r.offset==ranges[i][0]&&r.count==ranges[i][1]);coax_string path=pqn_asset_path((coax_v_u8){xml,66846},i);NEED(path.count==r.count&&!memcmp(path.data,xml+r.offset,(size_t)r.count));char text[65];hex(assets+32*i,text);equal_string(pqn_asset_digest(i),text);}
 NEED(!pqn_asset_range(-1).offset&&!pqn_asset_range(-1).count&&!pqn_asset_range(85).offset&&!pqn_asset_range(85).count);equal_string(pqn_asset_digest(-1),"");equal_string(pqn_asset_digest(85),"");equal_string(pqn_asset_path((coax_v_u8){xml,66845},0),"");equal_string(pqn_asset_path((coax_v_u8){xml,66846},-1),"");equal_string(pqn_asset_path((coax_v_u8){xml,66846},85),"");
 const char*bad[]={"","sdk","owned","owned-shadow-v2"};for(int i=0;i<4;i++){coax_t_PqnDescriptorV1 d=pqn_descriptor((coax_string){bad[i],(int64_t)strlen(bad[i])});NEED(d.status==COAX_PqnStatus_mode&&!d.query_kind&&!d.assets&&!d.scalar_libm&&!d.profile.count&&!d.schema.count&&!d.query_profile.count&&!d.policy_profile.count&&!d.profile.data&&!d.schema.data&&!d.query_profile.data&&!d.policy_profile.data);}
 coax_t_PqnDescriptorV1 d=pqn_descriptor((coax_string){"owned-shadow-v1",15});NEED(!d.status&&d.query_kind==1&&d.assets==85&&d.scalar_libm);equal_string(d.profile,"panda-libero-mujoco331-gr00t-wide-fixed-noise-owned-query-shadow-v1");equal_string(d.schema,"psionic-panda-owned-query-closed-loop-v1");equal_string(d.query_profile,"panda-owned-query-shadow-v1");equal_string(d.policy_profile,"panda-libero-mujoco331-gr00t-wide-fixed-noise-v1");
 int64_t lengths[4]={32,32,32,2720};NEED(!pqn_admit((coax_v_u8){ids[0],32},(coax_v_u8){ids[1],32},(coax_v_u8){ids[2],32},(coax_v_u8){assets,2720},0));
 for(int role=0;role<4;role++){int64_t n=lengths[role],wrong[]={-1,0,n-1,n+1};for(int i=0;i<4;i++){lengths[role]=wrong[i];NEED(pqn_admit((coax_v_u8){ids[0],lengths[0]},(coax_v_u8){ids[1],lengths[1]},(coax_v_u8){ids[2],lengths[2]},(coax_v_u8){assets,lengths[3]},1)==COAX_PqnStatus_shape);}lengths[role]=n;}
 for(int role=0;role<88;role++){uint8_t*p=role<3?ids[role]:assets+32*(role-3);for(int i=0;i<32;i++){p[i]^=1;NEED(pqn_admit((coax_v_u8){ids[0],32},(coax_v_u8){ids[1],32},(coax_v_u8){ids[2],32},(coax_v_u8){assets,2720},0)==COAX_PqnStatus_identity);if(role<3)NEED(pqn_model((coax_v_u8){ids[0],32},(coax_v_u8){ids[1],32},(coax_v_u8){ids[2],32},0)==COAX_PqnStatus_identity);p[i]^=1;}}
 ids[0][0]^=1;NEED(pqn_admit((coax_v_u8){ids[0],32},(coax_v_u8){ids[1],32},(coax_v_u8){ids[2],32},(coax_v_u8){assets,2720},1)==COAX_PqnStatus_effect);free(xml);printf("{\"passed\":true,\"assertions\":%zu,\"asset_paths\":85,\"digest_bitflips\":2816,\"SDK_calls\":0,\"NN_calls\":0}\n",checks);return 0;
}
