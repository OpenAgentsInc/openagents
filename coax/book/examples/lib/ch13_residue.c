#include <stdio.h>

float residue(float x, float c) { return x * x - c; }

int main(void) {
  volatile float x = 1.0f + 1.0f / 4096.0f, c = 1.0f + 1.0f / 2048.0f;
  printf("%g\n", residue(x, c));
  return 0;
}
