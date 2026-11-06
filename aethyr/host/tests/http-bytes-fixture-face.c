// Firm reason: 5 (thin ABI projection of the Coax request recorder into the actual HTTP imports).
#define HF_FIXTURE_HEADER "http-bytes-fixture.h"
#define coaxial_message http_bytes_base_message
#define HF_MESSAGE_EXPORT(name)
#include "fixture-face.c"
#undef coaxial_message
HF_IMPORT("coaxial_http") int32_t coaxial_http(HfAddress url,uint32_t n,HfAddress body,uint32_t body_n,int32_t flags);
HF_EXPORT("coaxial_message") void coaxial_message(HfAddress p,int32_t kind,uint32_t n){
 if(hfb_message(view(p),kind,(coax_string){(const char*)game(p)->inbox.e,n}))hf_result(view(p),6,coaxial_http((HfAddress)(uintptr_t)game(p)->inbox.e,n,0,0,4));
 else drain(p);
}
