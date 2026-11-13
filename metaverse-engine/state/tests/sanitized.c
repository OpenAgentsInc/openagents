// Native SDK harness only; the fixture and runtime contract remain Coax.
#include <stdbool.h>

#include "fixture.h"
#include <stdio.h>
static coax_t_StateLab s;
int main(void){uint8_t out[64];if(!state_lab_contract_hex(60,(coax_v_u8){out,64}))return 2;if(!state_lab_trial((coax_v_t_StateLab){&s,1},60,false,true))return 1;printf("sanitized replay: %lld ticks\n",(long long)s.clock.ticks);return 0;}
