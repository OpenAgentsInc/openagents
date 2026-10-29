/* Test-only flat memory face. All point policy/arithmetic remains Coax. */
#include "secp_group.h"
#include <stdint.h>
#define E(n) __attribute__((export_name(n)))
static coax_t_SgPoint point(uint32_t p){return *(coax_t_SgPoint*)(uintptr_t)p;}
static uint32_t result(coax_t_SgResult r,uint32_t out){*(coax_t_SgPoint*)(uintptr_t)out=r.value;return r.ok?0:1;}
E("group") uint32_t group(int op,uint32_t p,uint32_t q,uint32_t out){
 if(op==0)return result(sg_add(point(p),point(q)),out);
 if(op==1)return result(sg_double(point(p)),out);
 if(op==2)return result(sg_negate(point(p)),out);
 return result(sg_mixed_add(point(p),*(coax_t_SgAffine*)(uintptr_t)q),out);
}
E("decode") uint32_t decode(uint32_t p,int n,int lift,uint32_t out){coax_v_u8 b={(uint8_t*)(uintptr_t)p,n};return result(lift?sg_lift_x_public(b):sg_decode_public(b),out);}
E("pick") uint32_t pick(uint32_t p,int n,uint64_t which,uint32_t out){return result(sg_pick((coax_v_t_SgPoint){(coax_t_SgPoint*)(uintptr_t)p,n},which),out);}
E("encode") int encode(uint32_t p,uint32_t out){coax_t_SgEncoded r=sg_encode_public(point(p));for(int i=0;i<33;i++)((uint8_t*)(uintptr_t)out)[i]=r.bytes.e[i];return r.ok?(int)r.length:-1;}
