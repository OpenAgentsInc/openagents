/* Explicit issuer-only/dedicated/access parser-mode test fixture. */
#include "generated.c"
#include <stdint.h>
int fi_test_profile(const char *text,int64_t tn,const char *marker,int64_t mn,int64_t mode){
 coax_t_FiClaimsProjection r=coax_p_fi_claims_profile((coax_string){text,tn},(coax_string){marker,mn},mode);
 return (r.ok?1:0)|(r.has_client?2:0)|(r.has_marker?4:0);
}
#ifdef FI_CLAIMS_WASM
__attribute__((export_name("profile"))) int profile(uint32_t t,uint32_t tn,uint32_t m,uint32_t mn,int64_t mode){return fi_test_profile((const char *)(uintptr_t)t,tn,(const char *)(uintptr_t)m,mn,mode);}
#endif
