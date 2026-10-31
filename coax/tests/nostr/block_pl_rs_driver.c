// Native/Wasm driver for pla_read (-DPL) or rsa_read: stdin is the wire, stdout "consistent original-bytes".
#include "block_pl_rs.h"
#include <stdint.h>
#include <stdio.h>
static uint8_t in[1048576];
int main(void) {
  size_t n=fread(in,1,sizeof in,stdin);if (!feof(stdin) || ferror(stdin)) return 3;
#ifdef PL
  coax_t_PlaLease r=pla_read((coax_v_u8){in,(int64_t)n});
#else
  coax_t_RsaReadState r=rsa_read((coax_v_u8){in,(int64_t)n});
#endif
  printf("%d %lld\n",r.consistent?1:0,(long long)r.original.count);return 0;
}
