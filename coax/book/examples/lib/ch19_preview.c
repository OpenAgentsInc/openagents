/* A C host for ch17_glow.coax: draw one glowing dot as text. */
#include <stdio.h>
#include "ch17_glow.h"

int main(void) {
  const char *ramp = " .:-=+*#%@";
  for (int row = 0; row < 11; row++) {
    for (int col = 0; col < 23; col++) {
      float x = (col - 11) / 11.0f, y = (row - 5) / 5.0f;
      float c = glow_tone(glow_dot(x, y, 2.0f));
      putchar(ramp[(int)(c * 9.99f)]);
    }
    putchar('\n');
  }
  return 0;
}
