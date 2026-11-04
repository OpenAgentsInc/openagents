/* Native allocation effect; all capacities and entry decisions are Coax. */
#include "explorer-node.h"
#include <stdlib.h>
int64_t tool_main(void) {
  coax_t_NativeExplorer *s=malloc(sizeof *s);if(!s)return 1;
  coax_default_t_NativeExplorer(s);int64_t cap=node_arena_capacity();
  if(cap<=0){free(s);return 2;}
  uint8_t *arena=calloc((size_t)cap,1);if(!arena){free(s);return 1;}
  s->live=(coax_v_u8){arena,cap};int64_t result=node_main((coax_v_t_NativeExplorer){s,1});
  free(arena);free(s);return result;
}
