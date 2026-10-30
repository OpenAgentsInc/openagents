#include "fixture.h"
#include <stdint.h>
#include <stdio.h>
/* Every value of a u8 source and the u32 boundaries widen exactly. */
int main(void) {
  for (uint32_t v = 0; v < 256; v++) {
    uint8_t b = (uint8_t)v;
    if (widen_u8_u32(b) != v || widen_u8_u64(b) != v) return 1;
  }
  const uint32_t edges[] = {0, 1, 255, 256, 65535, 65536, 2147483647u, 2147483648u, 4294967294u, 4294967295u};
  for (unsigned i = 0; i < sizeof edges / sizeof edges[0]; i++)
    if (widen_u32_u64(edges[i]) != edges[i]) return 2;
  puts("ok");
  return 0;
}
