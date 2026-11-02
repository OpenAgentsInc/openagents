#include "native256.h"
#include <stdio.h>
int main(void) { int cases=0;
for (int i=0;i<2;i++) { if (!summary256_probe_0(i)) return 1; cases++; }
for (int i=0;i<5;i++) { if (!summary256_probe_1(i)) return 2; cases++; }
for (int i=0;i<1;i++) { if (!summary256_probe_2()) return 3; cases++; }
for (int i=0;i<3;i++) { if (!summary256_probe_3(i)) return 4; cases++; }
printf("summary256 native cases: %d PASS\n", cases); return 0; }
