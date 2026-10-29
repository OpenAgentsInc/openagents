/* Flat test ABI around the unintegrated generated Coax JWKS parser. */
#include "generated.c"
#include <stdint.h>
__attribute__((export_name("parse")))
int parse(uint32_t input,uint32_t n,uint32_t output) {
 coax_t_FiParsedJwks s=fi_jwks_parse((coax_string){(const char *)(uintptr_t)input,n});
 if(!s.ok)return 0;
 uint8_t *out=(uint8_t *)(uintptr_t)output;
 for(int i=0;i<s.count;i++){
  coax_t_FiParsedKey *k=&s.keys.e[i];uint8_t *p=out+i*649;
  for(int j=0;j<128;j++)p[j]=k->kid.e[j];
  for(int j=0;j<512;j++)p[132+j]=k->modulus.e[j];
  uint32_t kn=(uint32_t)k->kid_count,mn=(uint32_t)k->modulus_count;
  for(int j=0;j<4;j++){p[128+j]=(uint8_t)(kn>>(8*j));p[644+j]=(uint8_t)(mn>>(8*j));}
  p[648]=k->has_key_ops?1:0;
 }
 return (int)s.count;
}
