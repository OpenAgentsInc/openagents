/* Shared native/Wasm test-only serialization; this is not an authority API. */
#include "generated.c"
#include <stdint.h>
#include <string.h>
static void put_text(uint8_t *out,int *at,coax_t_FiClaimText *s){out[(*at)++]=(uint8_t)(s->count>>8);out[(*at)++]=(uint8_t)s->count;memcpy(out+*at,s->bytes.e,(size_t)s->count);*at+=(int)s->count;}
static void put_int(uint8_t *out,int *at,int64_t value){for(int i=0;i<8;i++)out[(*at)++]=(uint8_t)((uint64_t)value>>(i*8));}
int fi_test_claims(const char *text,int64_t tn,const char *marker,int64_t mn,uint8_t *out,int64_t cap){
 coax_t_FiClaimsProjection r=fi_claims_parse((coax_string){text,tn},(coax_string){marker,mn});if(!r.ok)return -1;
 if(cap<40000)return -2;int at=0;put_text(out,&at,&r.issuer);put_text(out,&at,&r.subject);
 out[at++]=(uint8_t)r.audience_count;for(int i=0;i<r.audience_count;i++)put_text(out,&at,&r.audiences.e[i]);
 put_text(out,&at,&r.key);out[at++]=(r.has_iat?1:0)|(r.has_exp?2:0)|(r.has_nbf?4:0)|(r.has_owner?8:0)|(r.has_client?16:0)|(r.has_marker?32:0);
 put_int(out,&at,r.iat);put_int(out,&at,r.exp);put_int(out,&at,r.nbf);
 put_text(out,&at,&r.owner);put_text(out,&at,&r.client);put_text(out,&at,&r.marker);return at;
}
#ifdef FI_CLAIMS_WASM
__attribute__((export_name("project"))) int project(uint32_t t,uint32_t tn,uint32_t m,uint32_t mn,uint32_t out,uint32_t cap){return fi_test_claims((const char *)(uintptr_t)t,tn,(const char *)(uintptr_t)m,mn,(uint8_t *)(uintptr_t)out,cap);}
#endif
