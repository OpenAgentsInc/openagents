#include "fixture.h"
#include <stdint.h>
#include <stdlib.h>
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  switch (atoi(argv[1])) {
    case 0: return checked_u(UINT64_C(4294967295)) != UINT32_MAX;
    case 1: (void)checked_u(UINT64_C(4294967296)); return 3;
    case 2: return trunc_u(UINT64_C(4294967296)) != 0;
    case 3: return checked_s(255) != 255;
    case 4: (void)checked_s(-1); return 3;
    case 5: (void)checked_s(256); return 3;
    case 6: return checked_us(INT64_MAX) != INT64_MAX;
    case 7: (void)checked_us(UINT64_C(9223372036854775808)); return 3;
    default: return 2;
  }
}
