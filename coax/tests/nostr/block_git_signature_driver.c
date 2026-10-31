// Native/Wasm driver for gsa_read: stdin is a GitSignature wire, stdout "consistent owner armor-bytes".
#include "block_git_signature.h"
#include <stdint.h>
#include <stdio.h>
static uint8_t in[1048576];
int main(void) {
  size_t n=fread(in,1,sizeof in,stdin);if (!feof(stdin) || ferror(stdin)) return 3;
  coax_t_GsaSignature r=gsa_read((coax_v_u8){in,(int64_t)n});
  printf("%d %d %lld\n",r.consistent?1:0,r.owner?1:0,(long long)r.armor.count);return 0;
}
