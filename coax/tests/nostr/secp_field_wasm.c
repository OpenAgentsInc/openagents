/* Test-only flat ABI. All field arithmetic and validation are generated Coax. */
#include "secp_field.h"
#include <stdint.h>
#define E(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p){return(coax_v_u8){(uint8_t*)(uintptr_t)p,32};}
E("field") uint32_t field(int op,uint32_t ap,uint32_t bp,uint32_t out){
  coax_t_SecpFieldResult a=sf_load(bytes(ap)),b=sf_load(bytes(bp));
  coax_t_SecpField r=a.value;uint32_t status=0;
  if(op==1)r=sf_add(a.value,b.value);
  else if(op==2)r=sf_subtract(a.value,b.value);
  else if(op==3)r=sf_negate(a.value);
  else if(op==4)r=sf_multiply(a.value,b.value);
  else if(op==5)r=sf_square(a.value);
  else if(op==6){a=sf_inverse(a.value);r=a.value;status=a.ok?0:1;}
  else if(op==7){a=sf_sqrt(a.value);r=a.value;status=a.ok?0:1;}
  else if(op!=0)return 5;
  return sf_store(r,bytes(out))?status:9;
}
E("raw_product") uint32_t raw_product(uint32_t p,uint32_t q,uint32_t out){
  coax_t_SecpField a={0},b={0};
  for(int i=0;i<10;i++){a.limb.e[i]=((uint64_t*)(uintptr_t)p)[i];b.limb.e[i]=((uint64_t*)(uintptr_t)q)[i];}
  return sf_store(sf_multiply(a,b),bytes(out))?0:9;
}
E("raw_power") uint32_t raw_power(uint32_t p,int op,uint32_t out){
  coax_t_SecpField a={0};for(int i=0;i<10;i++)a.limb.e[i]=((uint64_t*)(uintptr_t)p)[i];
  coax_t_SecpFieldResult r=op==6?sf_inverse(a):sf_sqrt(a);
  return sf_store(r.value,bytes(out))?(r.ok?0:1):9;
}
E("raw") uint32_t raw(uint32_t p,int magnitude,uint32_t out){
  coax_t_SecpFieldDebug d={0};d.magnitude=magnitude;
  for(int i=0;i<10;i++)d.value.limb.e[i]=((uint64_t*)(uintptr_t)p)[i];
  d=sf_debug_normalize(d);
  return sf_store(d.value,bytes(out))?0:9;
}
