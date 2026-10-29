#include "field_guard_adapter.h"
#include <assert.h>
#include <stdint.h>

int main(void) {
  uint64_t in64[12]={0},out64[12]={0};in64[0]=in64[11]=UINT64_C(0x1122334455667788);out64[0]=out64[11]=UINT64_C(0x8877665544332211);
  in64[1]=1;
  assert(field_guard_normalize((coax_v_u64){in64+1,10},(coax_v_u64){out64+1,10}));
  assert(out64[1]==1 && out64[0]==UINT64_C(0x8877665544332211) && out64[11]==UINT64_C(0x8877665544332211));
  assert(in64[0]==UINT64_C(0x1122334455667788) && in64[11]==UINT64_C(0x1122334455667788));
  assert(!field_guard_normalize((coax_v_u64){in64+1,9},(coax_v_u64){out64+1,10}));
  uint32_t a[10]={0},b[10]={0},out[10]={0};a[0]=a[9]=0x13572468u;b[0]=b[9]=0x24681357u;out[0]=out[9]=0xdeadbeefu;a[1]=b[1]=1;
  assert(field_guard_add32((coax_v_u32){a+1,8},(coax_v_u32){b+1,8},(coax_v_u32){out+1,8}) && out[1]==2);
  assert(out[0]==0xdeadbeefu && out[9]==0xdeadbeefu && a[0]==0x13572468u && a[9]==0x13572468u && b[0]==0x24681357u && b[9]==0x24681357u);
  assert(field_guard_mul32((coax_v_u32){a+1,8},(coax_v_u32){b+1,8},(coax_v_u32){out+1,8}) && out[1]==1);
  assert(!field_guard_mul32((coax_v_u32){a+1,8},(coax_v_u32){b+1,8},(coax_v_u32){out+1,7}));
  return 0;
}
