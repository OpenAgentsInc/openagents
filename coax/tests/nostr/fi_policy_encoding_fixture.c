/* Test-only selected configuration encoding; no policy ID or authority. */
#include "generated.c"
#include <stdint.h>
#include <string.h>
static int fi_test_policy_max(uint8_t *out,int64_t cap){
 char names_data[16][2048],aud[2048],uri[2048],owners_data[8][128],clients_data[8][128];coax_string names[16],owners[8],clients[8];
 memset(names_data,'x',sizeof(names_data));memset(aud,'x',sizeof(aud));memset(uri,'x',sizeof(uri));
 memcpy(names_data[0],"https://issuer.test/",20);memcpy(aud,"wss://relay.test/",17);memcpy(uri,"https://keys.test/",18);
 for(int i=0;i<16;i++){if(i)names_data[i][0]=(char)('A'+i);names[i]=(coax_string){names_data[i],2048};}
 for(int i=0;i<8;i++){memset(owners_data[i],'y',128);memset(clients_data[i],'z',128);owners_data[i][0]=(char)('A'+i);clients_data[i][0]=(char)('K'+i);owners[i]=(coax_string){owners_data[i],128};clients[i]=(coax_string){clients_data[i],128};}
 coax_t_FiCommunity c={0};c.host=(coax_string){"relay.test",10};c.audience=(coax_string){aud,2048};c.issuers=(coax_v_string){names,16};
 coax_t_FiIssuer p={0};p.issuer=names[0];p.jwks_uri=(coax_string){uri,2048};p.transport=1;p.token_class=2;p.maximum_age=10;p.connection_lifetime=10;p.refresh=1;p.hard_deadline=30;p.marker_name=(coax_string){"kind",4};p.owner_values=(coax_v_string){owners,8};p.client_values=(coax_v_string){clients,8};
 coax_t_FiPolicyEncoding r=fi_policy_encoding(c,p);if(!r.ok)return -1;if(cap<r.count)return -2;memcpy(out,r.bytes.e,(size_t)r.count);return (int)r.count;
}
int fi_test_policy_encoding(int id,uint8_t *out,int64_t cap){
 if(id==21)return fi_test_policy_max(out,cap);
 coax_string names[3]={{"https://issuer.test",19},{"https://other.test",18},{"https://third.test",18}},owners[2]={{"human",5},{"person",6}},clients[2]={{"service",7},{"worker",6}};
 coax_t_FiCommunity c={0};c.host=(coax_string){"relay.test",10};c.audience=(coax_string){"wss://relay.test",16};c.issuers=(coax_v_string){names,2};
 coax_t_FiIssuer p={0};p.issuer=(coax_string){"https://issuer.test",19};p.jwks_uri=(coax_string){"https://keys.test",17};p.transport=1;p.token_class=2;p.maximum_age=10;p.skew=1;p.connection_lifetime=10;p.refresh=1;p.hard_deadline=30;p.marker_name=(coax_string){"kind",4};p.owner_values=(coax_v_string){owners,2};p.client_values=(coax_v_string){clients,2};p.noncollision=1;
 if(id==1){coax_string t=names[0];names[0]=names[1];names[1]=t;t=owners[0];owners[0]=owners[1];owners[1]=t;t=clients[0];clients[0]=clients[1];clients[1]=t;}
 if(id==2)c.host=(coax_string){"other.test",10};if(id==3)c.audience=(coax_string){"wss://other.test",16};if(id==4)c.issuers.count=3;
 if(id==5){p.issuer=names[1];}if(id==6)p.jwks_uri=(coax_string){"https://else.test",17};
 if(id==7){p.transport=2;p.jwks_uri=(coax_string){"http://keys.local",17};}
 if(id==8){p.token_class=1;p.marker_name=(coax_string){"",0};p.owner_values.count=0;p.client_values.count=0;p.noncollision=0;}
 if(id==9)p.maximum_age=11;if(id==10)p.skew=2;if(id==11)p.connection_lifetime=11;if(id==12)p.refresh=2;if(id==13)p.hard_deadline=31;
 if(id==14)p.marker_name=(coax_string){"type",4};if(id==15)owners[1]=(coax_string){"other",5};if(id==16)clients[1]=(coax_string){"daemon",6};
 if(id==17)p.allow_client=1;if(id==18)p.noncollision=0;if(id==19)names[1]=names[0];if(id==20)p.refresh=0;
 coax_t_FiPolicyEncoding r=fi_policy_encoding(c,p);if(!r.ok)return -1;if(cap<r.count)return -2;memcpy(out,r.bytes.e,(size_t)r.count);return (int)r.count;
}
#ifdef FI_POLICY_WASM
__attribute__((export_name("encode"))) int encode(int id,uint32_t out,uint32_t cap){return fi_test_policy_encoding(id,(uint8_t *)(uintptr_t)out,cap);}
#endif
