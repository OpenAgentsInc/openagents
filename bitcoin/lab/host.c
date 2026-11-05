/* Native allocation effect, matching the explorer's host. Coax owns all
   parsing, state admission, routes and the decision to publish a response. */
#include "service.h"
#include <stdlib.h>
int64_t tool_main(void){
  coax_t_LabState *s=malloc(sizeof *s);if(!s)return 1;
  coax_default_t_LabState(s);
  int64_t code=ls_main((coax_v_t_LabState){s,1});free(s);return code;
}
