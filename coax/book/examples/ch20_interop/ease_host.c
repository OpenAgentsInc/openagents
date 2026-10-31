#include <stdio.h>
#include "ease.h"

int main(void) {
  for (int i = 0; i <= 4; i++) {
    float x = i / 4.0f;
    printf("ease(%g) = %g\n", x, ease(x));
  }
  return 0;
}
