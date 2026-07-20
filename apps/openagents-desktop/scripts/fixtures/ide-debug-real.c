#include <stdio.h>

static int square(int value) {
  return value * value;
}

int main(void) {
  int result = square(7);
  printf("IDE11_C_RESULT=%d\n", result);
  return result == 49 ? 0 : 1;
}
