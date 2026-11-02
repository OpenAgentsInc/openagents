#include "native16.h"
#include <stdio.h>
int main(void) { int cases=0;
for (int i=0;i<10;i++) { if (!path16_probe_0(i)) return 1; cases++; }
for (int i=0;i<4;i++) { if (!path16_probe_1(i)) return 2; cases++; }
for (int i=0;i<3;i++) { if (!path16_probe_2(i)) return 3; cases++; }
for (int i=0;i<4;i++) { if (!path16_probe_3(i)) return 4; cases++; }
for (int i=0;i<1;i++) { if (!path16_probe_4()) return 5; cases++; }
for (int i=0;i<3;i++) { if (!path16_probe_5(i)) return 6; cases++; }
for (int i=0;i<8;i++) { if (!path16_probe_6(i)) return 7; cases++; }
printf("path16 native cases: %d PASS\n", cases); return 0; }
