#include "arithmetic.h"
#include <stdint.h>
#include <stdlib.h>
int main(int argc,char **argv){
 if(argc!=2)return 2;switch(atoi(argv[1])){
  case 0: return checked_add(UINT64_MAX)!=0;
  case 1: return checked_sub(0)!=UINT64_MAX;
  case 2: return checked_mul(UINT64_MAX)!=UINT64_MAX-1;
  case 3: return checked_neg(UINT64_C(9223372036854775808))!=UINT64_C(9223372036854775808);
  case 4: return compound(UINT64_MAX)!=0;
  case 5: (void)secret_shift(64);return 3;
  case 6: (void)secret_div(0);return 3;
  case 7: return checked_add(41)!=42;
  case 8: return wrapping_add(UINT64_MAX)!=0;
  case 9: {coax_a1_u64 a={{UINT64_MAX}};return array_compound(a,1)!=0;}
  default:return 2;
 }
}
