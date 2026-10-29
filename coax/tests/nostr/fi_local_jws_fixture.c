/* Test-only local JWS candidate: caller-supplied JWKS is not authority. */
#include "generated.c"
#include <stdint.h>
int fi_test_local_class(const uint8_t *token,int64_t tn,const char *jwks,int64_t jn,const char *host,int64_t hn,int64_t now,int64_t token_class){
 coax_string names[1]={{"https://issuer.test",19}},owners[1]={{"https://issuer.test",19}},clients[1]={{"service",7}};
 coax_t_FiCommunity community={0};community.host=(coax_string){"relay.test",10};community.audience=(coax_string){"wss://relay.test",16};community.issuers=(coax_v_string){names,1};
 coax_t_FiIssuer issuer={0};issuer.issuer=(coax_string){"https://issuer.test",19};issuer.jwks_uri=(coax_string){"https://keys.test",17};issuer.transport=1;issuer.token_class=token_class;
 issuer.maximum_age=10;issuer.connection_lifetime=10;issuer.refresh=1;issuer.hard_deadline=30;if(token_class==2){issuer.marker_name=(coax_string){"iss",3};issuer.owner_values=(coax_v_string){owners,1};issuer.client_values=(coax_v_string){clients,1};}
 return fi_local_jws_candidate((coax_v_u8){(uint8_t *)token,tn},(coax_string){jwks,jn},(coax_v_t_FiCommunity){&community,1},(coax_v_t_FiIssuer){&issuer,1},(coax_string){host,hn},now);
}
int fi_test_local(const uint8_t *token,int64_t tn,const char *jwks,int64_t jn,const char *host,int64_t hn,int64_t now){return fi_test_local_class(token,tn,jwks,jn,host,hn,now,2);}
#ifdef FI_LOCAL_WASM
__attribute__((export_name("local_class"))) int local_class(uint32_t t,uint32_t tn,uint32_t j,uint32_t jn,uint32_t h,uint32_t hn,int64_t now,int64_t klass){return fi_test_local_class((const uint8_t *)(uintptr_t)t,tn,(const char *)(uintptr_t)j,jn,(const char *)(uintptr_t)h,hn,now,klass);}
__attribute__((export_name("local_candidate"))) int local_candidate(uint32_t t,uint32_t tn,uint32_t j,uint32_t jn,uint32_t h,uint32_t hn,int64_t now){return fi_test_local((const uint8_t *)(uintptr_t)t,tn,(const char *)(uintptr_t)j,jn,(const char *)(uintptr_t)h,hn,now);}
#endif
