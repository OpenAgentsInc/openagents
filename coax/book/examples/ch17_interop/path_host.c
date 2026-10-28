#include <stdio.h>
#include "path.h"

int main(void) {
  coax_t_Point a = {0, 0, 0}, b = {3, 4, 0};
  coax_t_Point m = path_mix(a, b, 0.5f);
  printf("mix: %g %g %g\n", m.x, m.y, m.z);

  coax_t_Point points[] = {a, b, {3, 4, 12}};
  coax_v_t_Point view = {points, 3};
  printf("length: %g\n", path_length(view));

  coax_t_Mode next = path_next(COAX_Mode_orbit);
  printf("after orbit: %s\n", next == COAX_Mode_dolly ? "dolly" : "something else");
  return 0;
}
