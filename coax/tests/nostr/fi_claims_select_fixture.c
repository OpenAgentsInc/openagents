/* Test-only wrapper around untrusted claim selection, never a provider API. */
#include "generated.c"
#include <stdint.h>
#include <string.h>
int fi_test_select(const char *text,int64_t tn,const char *host,int64_t hn,const char *allow,int64_t an,const char *marker,int64_t mn,uint8_t *out){
 coax_string names[1]={{allow,an}},owners[1]={{"https://issuer.test",19}},clients[1]={{"service",7}};
 coax_t_FiCommunity community={0};community.host=(coax_string){"relay.test",10};community.audience=(coax_string){"wss://relay.test",16};community.issuers=(coax_v_string){names,1};
 coax_t_FiIssuer issuer={0};issuer.issuer=(coax_string){"https://issuer.test",19};issuer.jwks_uri=(coax_string){"https://keys.test",17};issuer.transport=1;issuer.token_class=2;
 issuer.maximum_age=10;issuer.connection_lifetime=10;issuer.refresh=1;issuer.hard_deadline=30;
 if(mn==14 && memcmp(marker,"invalid_config",14)==0)issuer.refresh=0;
 issuer.marker_name=(coax_string){marker,mn};issuer.owner_values=(coax_v_string){owners,1};issuer.client_values=(coax_v_string){clients,1};
 coax_t_FiClaimsSelection r=fi_claims_select((coax_v_t_FiCommunity){&community,1},(coax_v_t_FiIssuer){&issuer,1},(coax_string){host,hn},(coax_string){text,tn});
 if(r.status==0 && r.projection.has_marker){memcpy(out,r.projection.marker.bytes.e,(size_t)r.projection.marker.count);out[r.projection.marker.count]=0;}
 return (int)(r.status*2+(r.status==0 && r.projection.has_marker));
}
#ifdef FI_CLAIMS_WASM
__attribute__((export_name("select_claims"))) int select_claims(uint32_t t,uint32_t tn,uint32_t h,uint32_t hn,uint32_t a,uint32_t an,uint32_t m,uint32_t mn,uint32_t o){
 return fi_test_select((const char *)(uintptr_t)t,tn,(const char *)(uintptr_t)h,hn,(const char *)(uintptr_t)a,an,(const char *)(uintptr_t)m,mn,(uint8_t *)(uintptr_t)o);
}
#endif
