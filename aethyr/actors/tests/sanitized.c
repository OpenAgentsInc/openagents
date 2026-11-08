// SDK projection harness: simulation and conformance arithmetic remain Coax.
#include <stdbool.h>
#include "actors.h"
#include <stdio.h>
static coax_t_ActorLab lab;
int main(void) {
  uint8_t hash[64];
  if (!actor_contract_hex(60,(coax_v_u8){hash,64})) return 2;
  if (!actor_lab_open((coax_v_t_ActorLab){&lab,1},0) ||
      !actor_lab_trial((coax_v_t_ActorLab){&lab,1},60,false,true)) return 1;
  printf("sanitized actor replay: %lld ticks, %lld controls\n",
         (long long)lab.world.e[0].clock.ticks,(long long)lab.control_n);
  return 0;
}
