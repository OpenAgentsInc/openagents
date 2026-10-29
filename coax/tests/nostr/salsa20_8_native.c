#include "salsa20_8.h"
#include <assert.h>
#include <stdint.h>

int main(void) {
  uint32_t input[16] = {0x219a877e,0x86c93e4f,0xe640a97c,0x268f7141,0x5b55eeba,0xb5c1618c,0x1146f80d,0x1d3bcd6d,0x19f324ee,0x853d9bdf,0x4b1e1214,0x32aac55a,0x291d0276,0x2948c709,0x8dc6ebed,0x5ec2b8b8};
  uint32_t expected[16] = {0x9c851fa4,0x99cc0866,0xcbca813b,0x05ef0c02,0x81214b04,0x7d33fda2,0x631c7bfd,0x292f6896,0x683139b4,0xbce6c9e3,0xb7c56bfe,0xba966da0,0x10cc24e4,0x5c74912c,0x3d67ad24,0x818f61c7};
  uint32_t output[17];
  for (int i=0; i<17; i++) output[i]=0xa5a5a5a5;
  assert(!salsa20_8_core((coax_v_u32){input,15},(coax_v_u32){output,17},0));
  assert(!salsa20_8_core((coax_v_u32){input,16},(coax_v_u32){output,15},0));
  assert(!salsa20_8_core((coax_v_u32){input,16},(coax_v_u32){output,17},1));
  for (int i=0; i<17; i++) assert(output[i]==0xa5a5a5a5);
  assert(salsa20_8_core((coax_v_u32){input,16},(coax_v_u32){output,17},0));
  for (int i=0; i<16; i++) assert(output[i]==expected[i]);
  assert(output[16]==0xa5a5a5a5);
}
