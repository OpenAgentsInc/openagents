/* Scalar transport and observations only. Envelope policy remains Coax. */
#include "envelope.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_ArtifactBuild build_result;
static coax_t_ArtifactInfo info;
EXPORT("build") uint32_t build(uint32_t p,int32_t n,uint32_t s,int32_t sn,int64_t issued,int64_t retain,uint32_t scratch,int32_t sc,uint32_t out,int32_t cap){
 build_result=envelope_build((coax_string){(void*)(uintptr_t)p,n},(coax_string){(void*)(uintptr_t)s,sn},issued,retain,(coax_v_u8){(void*)(uintptr_t)scratch,sc},(coax_v_u8){(void*)(uintptr_t)out,cap});return build_result.error;
}
EXPORT("check") uint32_t check(uint32_t p,int32_t n,uint32_t s,int32_t sn,int64_t wire,uint32_t scratch,int32_t sc){
 coax_string body={(void*)(uintptr_t)p,n},schema={(void*)(uintptr_t)s,sn};coax_v_u8 work={(void*)(uintptr_t)scratch,sc};
 info=wire==-1?envelope_check_body(body,schema,work):envelope_check_observed(body,schema,wire,work);return info.error;
}
EXPORT("written") int64_t written(void){return build_result.written;}
EXPORT("build_size") int64_t build_size(void){return build_result.canonical_size;}
EXPORT("from") int64_t from(void){return info.inline_from;}
EXPORT("to") int64_t to(void){return info.inline_to;}
EXPORT("size") int64_t size(void){return info.canonical_size;}
EXPORT("issued") int64_t issued(void){return info.issued_at;}
EXPORT("retain") int64_t retain(void){return info.retain_until;}
EXPORT("wire") int64_t wire(void){return info.wire_bytes;}
