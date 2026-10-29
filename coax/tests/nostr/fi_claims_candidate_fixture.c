/* Semantic candidate fixture; never identity or signature authority. */
#include "generated.c"
#include <stdint.h>
int fi_test_candidate(const char *text,int64_t tn,const char *host,int64_t hn,const char *marker,int64_t mn,const char *algorithm,int64_t algn,const char *typ,int64_t typn,int64_t now){
 coax_string names[1]={{"https://issuer.test",19}},owners[1]={{"https://issuer.test",19}},clients[1]={{"service",7}};
 coax_t_FiCommunity community={0};community.host=(coax_string){"relay.test",10};community.audience=(coax_string){"wss://relay.test",16};community.issuers=(coax_v_string){names,1};
 coax_t_FiIssuer issuer={0};issuer.issuer=(coax_string){"https://issuer.test",19};issuer.jwks_uri=(coax_string){"https://keys.test",17};issuer.transport=1;issuer.token_class=2;
 issuer.maximum_age=10;issuer.connection_lifetime=10;issuer.refresh=1;issuer.hard_deadline=30;
 issuer.marker_name=(coax_string){marker,mn};issuer.owner_values=(coax_v_string){owners,1};issuer.client_values=(coax_v_string){clients,1};
 coax_t_FiHeader header={0};header.algorithm=(coax_string){algorithm,algn};header.typ=(coax_string){typ,typn};header.kid=(coax_string){"key",3};
 return (int)fi_claims_semantic_candidate((coax_v_t_FiCommunity){&community,1},(coax_v_t_FiIssuer){&issuer,1},(coax_string){host,hn},header,(coax_string){text,tn},now);
}
#ifdef FI_CLAIMS_WASM
__attribute__((export_name("candidate"))) int candidate(uint32_t t,uint32_t tn,uint32_t h,uint32_t hn,uint32_t m,uint32_t mn,uint32_t a,uint32_t an,uint32_t y,uint32_t yn,int64_t now){
 return fi_test_candidate((const char *)(uintptr_t)t,tn,(const char *)(uintptr_t)h,hn,(const char *)(uintptr_t)m,mn,(const char *)(uintptr_t)a,an,(const char *)(uintptr_t)y,yn,now);
}
#endif
